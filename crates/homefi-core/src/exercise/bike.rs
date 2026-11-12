//! The rider's bike: what its console shows and how many resistance levels its knob has. Targets are
//! always cadence (RPM) inside HomeFi; this only decides how they are *shown*. A spinning bike's speed
//! sensor sits on the flywheel, so speed is proportional to cadence whatever the resistance: one
//! calibration factor (km/h per RPM) is enough. Pure and lenient like the other settings.

use serde::{Deserialize, Serialize};

use super::model::{RpmRange, MAX_EFFORT, MIN_EFFORT};

pub const MAX_RESISTANCE_LEVELS: u8 = 32;
pub const DEFAULT_RESISTANCE_LEVELS: u8 = 8;
/// A believable km/h per RPM (spinning bikes: 0.2-0.35).
pub const MIN_KPH_PER_RPM: f32 = 0.05;
pub const MAX_KPH_PER_RPM: f32 = 1.0;
/// The calibration asks the rider to pedal at this cadence for this long.
pub const CALIBRATION_RPM: u16 = 80;
pub const CALIBRATION_SECONDS: u8 = 20;
const KPH_PER_MPH: f32 = 1.609_344;
/// Speeds are shown (and stored) with this many steps per unit: one decimal on screen, three on the factor.
const SPEED_DECIMALS_SCALE: f32 = 10.0;
const FACTOR_DECIMALS_SCALE: f32 = 1000.0;

/// What the bike's console reads out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleUnit {
    Rpm,
    Kph,
    Mph,
    /// No console, or not looked at: targets show in RPM.
    #[default]
    #[serde(other)]
    None,
}

/// The unit a target is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetUnit {
    Kph,
    Mph,
    #[default]
    #[serde(other)]
    Rpm,
}

impl TargetUnit {
    /// The label next to the number ("RPM", "km/h", "mph"): the same in every language.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rpm => "RPM",
            Self::Kph => "km/h",
            Self::Mph => "mph",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BikeProfile {
    pub console: ConsoleUnit,
    /// Positions of the resistance knob, 1..=32; 0 = unknown (no level is shown).
    pub resistance_levels: u8,
    /// km/h the console shows per RPM; 0 = not calibrated (targets show in RPM).
    pub kph_per_rpm: f32,
}

impl Default for BikeProfile {
    fn default() -> Self {
        Self { console: ConsoleUnit::None, resistance_levels: DEFAULT_RESISTANCE_LEVELS, kph_per_rpm: 0.0 }
    }
}

/// A target ready to show: `value` with `min`..`max` in `unit`, and the cadence it came from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ShownTarget {
    pub unit: TargetUnit,
    /// The big number: the center of the range.
    pub value: f32,
    pub min: f32,
    pub max: f32,
    /// The center cadence (RPM): "pedal at about 75 RPM" stays true whatever the unit.
    pub rpm: u16,
}

fn round_to(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

impl BikeProfile {
    /// Levels within 0..=32; a factor outside the believable range, or NaN, becomes "not calibrated".
    pub fn clamped(self) -> Self {
        let kph_per_rpm = if self.kph_per_rpm.is_finite() && self.kph_per_rpm > 0.0 {
            self.kph_per_rpm.clamp(MIN_KPH_PER_RPM, MAX_KPH_PER_RPM)
        } else {
            0.0
        };
        Self { resistance_levels: self.resistance_levels.min(MAX_RESISTANCE_LEVELS), kph_per_rpm, ..self }
    }

    /// km/h per RPM from one reading: the console showed `shown_speed` (in the console's unit) while the
    /// rider pedaled `rpm`. `None` for a console without speed, a bad reading or a factor nobody's bike has.
    pub fn calibrate(console: ConsoleUnit, rpm: u16, shown_speed: f32) -> Option<f32> {
        let kph = match console {
            ConsoleUnit::Kph => shown_speed,
            ConsoleUnit::Mph => shown_speed * KPH_PER_MPH,
            ConsoleUnit::Rpm | ConsoleUnit::None => return None,
        };
        if rpm == 0 || !kph.is_finite() || kph <= 0.0 {
            return None;
        }
        let factor = round_to(kph / f32::from(rpm), FACTOR_DECIMALS_SCALE);
        (MIN_KPH_PER_RPM..=MAX_KPH_PER_RPM).contains(&factor).then_some(factor)
    }

    /// The unit the targets show in: the console's speed once calibrated, else RPM.
    fn unit(&self) -> TargetUnit {
        match self.console {
            ConsoleUnit::Kph if self.kph_per_rpm > 0.0 => TargetUnit::Kph,
            ConsoleUnit::Mph if self.kph_per_rpm > 0.0 => TargetUnit::Mph,
            _ => TargetUnit::Rpm,
        }
    }

    pub fn target(&self, range: RpmRange) -> ShownTarget {
        let unit = self.unit();
        let rpm = range.center();
        let convert = |rpm: u16| -> f32 {
            let f = f32::from(rpm);
            match unit {
                TargetUnit::Rpm => f,
                TargetUnit::Kph => round_to(f * self.kph_per_rpm, SPEED_DECIMALS_SCALE),
                TargetUnit::Mph => round_to(f * self.kph_per_rpm / KPH_PER_MPH, SPEED_DECIMALS_SCALE),
            }
        };
        ShownTarget { unit, value: convert(rpm), min: convert(range.min_rpm), max: convert(range.max_rpm), rpm }
    }

    /// The knob position for an effort 1..=10, 1..=`resistance_levels`; `None` when the levels are unknown.
    pub fn level(&self, effort: u8) -> Option<u8> {
        let levels = self.resistance_levels.min(MAX_RESISTANCE_LEVELS);
        if levels == 0 {
            return None;
        }
        let effort = f32::from(effort.clamp(MIN_EFFORT, MAX_EFFORT));
        let level = (effort * f32::from(levels) / f32::from(MAX_EFFORT)).round() as u8;
        Some(level.clamp(1, levels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kph_bike() -> BikeProfile {
        BikeProfile { console: ConsoleUnit::Kph, kph_per_rpm: 0.25, ..BikeProfile::default() }
    }

    #[test]
    fn calibrate_turns_speed_into_a_factor_and_rejects_nonsense() {
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, 20.0), Some(0.25));
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 0, 20.0), None);
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, 0.0), None);
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, -5.0), None);
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, f32::NAN), None);
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, 2.0), None, "0.025 km/h per RPM: nobody's bike");
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Kph, 80, 200.0), None, "2.5 km/h per RPM");
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::Rpm, 80, 80.0), None, "an RPM console has no speed");
        assert_eq!(BikeProfile::calibrate(ConsoleUnit::None, 80, 20.0), None);
    }

    #[test]
    fn targets_show_rpm_until_calibrated() {
        let range = RpmRange::new(70, 80);
        let t = BikeProfile::default().target(range);
        assert_eq!((t.unit, t.value, t.min, t.max, t.rpm), (TargetUnit::Rpm, 75.0, 70.0, 80.0, 75));
        let uncalibrated = BikeProfile { console: ConsoleUnit::Kph, ..BikeProfile::default() };
        assert_eq!(uncalibrated.target(range).unit, TargetUnit::Rpm);
        let rpm_console = BikeProfile { console: ConsoleUnit::Rpm, kph_per_rpm: 0.25, ..BikeProfile::default() };
        assert_eq!(rpm_console.target(range).unit, TargetUnit::Rpm);
        let t = kph_bike().target(range);
        assert_eq!((t.unit, t.value, t.min, t.max, t.rpm), (TargetUnit::Kph, 18.8, 17.5, 20.0, 75));
    }

    #[test]
    fn mph_converts() {
        let bike = BikeProfile { console: ConsoleUnit::Mph, kph_per_rpm: 0.4, ..BikeProfile::default() };
        let t = bike.target(RpmRange::new(80, 80));
        assert_eq!((t.unit, t.value), (TargetUnit::Mph, 19.9)); // 32 km/h
                                                                // a console reading in mph calibrates to the same km/h factor
        let f = BikeProfile::calibrate(ConsoleUnit::Mph, 80, 12.4274).unwrap();
        assert!((f - 0.25).abs() < 1e-3, "{f}");
        assert_eq!(TargetUnit::Kph.label(), "km/h");
    }

    #[test]
    fn levels_map_effort_1_10_to_1_n() {
        let bike = BikeProfile::default();
        assert_eq!([1, 5, 7, 10].map(|e| bike.level(e)), [Some(1), Some(4), Some(6), Some(8)]);
        assert_eq!(BikeProfile { resistance_levels: 0, ..bike }.level(7), None);
        assert_eq!(BikeProfile { resistance_levels: 1, ..bike }.level(10), Some(1));
        assert_eq!(bike.level(0), Some(1), "an out-of-range effort is pulled in");
        assert_eq!(bike.level(200), Some(8));
    }

    #[test]
    fn bike_json_is_lenient() {
        let b: BikeProfile = serde_json::from_str(r#"{"console":"kph","kph_per_rpm":0.27,"future":1}"#).unwrap();
        assert_eq!((b.console, b.resistance_levels, b.kph_per_rpm), (ConsoleUnit::Kph, 8, 0.27));
        let b: BikeProfile = serde_json::from_str(r#"{"console":"abacus"}"#).unwrap();
        assert_eq!(b.console, ConsoleUnit::None);
        assert_eq!(serde_json::from_str::<BikeProfile>("{}").unwrap(), BikeProfile::default());
        let wild = BikeProfile { console: ConsoleUnit::Kph, resistance_levels: 200, kph_per_rpm: f32::NAN }.clamped();
        assert_eq!((wild.resistance_levels, wild.kph_per_rpm), (MAX_RESISTANCE_LEVELS, 0.0));
        assert_eq!(BikeProfile { kph_per_rpm: 9.0, ..wild }.clamped().kph_per_rpm, MAX_KPH_PER_RPM);
        assert_eq!(BikeProfile { kph_per_rpm: 0.001, ..wild }.clamped().kph_per_rpm, MIN_KPH_PER_RPM);
        assert_eq!(serde_json::to_string(&ConsoleUnit::Mph).unwrap(), "\"mph\"");
    }
}
