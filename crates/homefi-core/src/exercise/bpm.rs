//! Does a song's tempo fit a cadence? One pedal stroke (one leg down) per beat at 1:1, one every two
//! beats at half-time (120..180 BPM songs make 60..90 RPM), two a beat at double-time (very slow
//! songs, off by default). Pure.

use serde::{Deserialize, Serialize};

use super::model::{RpmRange, Target};
use super::settings::ExerciseSettings;

/// Tempos outside this are not music we can ride to (a wrong octave or noise).
pub const MIN_USABLE_BPM: f32 = 40.0;
pub const MAX_USABLE_BPM: f32 = 220.0;
/// The ladder's steps: how much wider each rung accepts.
const LADDER_STEP_RPM: f32 = 3.0;

/// Where a song's tempo came from, most trusted first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BpmSource {
    /// Tidal's `bpm` field.
    Tidal,
    /// HomeFi's own analysis of the audio.
    Analysis,
    /// The `BPM` tag of the library file.
    #[default]
    #[serde(other)]
    Tag,
}

/// Pedal strokes per beat of the song.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BeatRatio {
    /// One stroke every two beats.
    Half,
    /// Two strokes every beat.
    Double,
    /// One stroke every beat.
    #[default]
    #[serde(other)]
    Whole,
}

impl Default for Fit {
    fn default() -> Self {
        Self { ratio: BeatRatio::Whole, rpm: 0.0, error_rpm: 0.0 }
    }
}

impl BeatRatio {
    pub fn strokes_per_beat(self) -> f32 {
        match self {
            Self::Whole => 1.0,
            Self::Half => 0.5,
            Self::Double => 2.0,
        }
    }
}

/// How loosely a song may match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// RPM outside the target range that still counts.
    pub rpm: f32,
    pub allow_half: bool,
    pub allow_double: bool,
}

impl Tolerance {
    pub fn from_settings(s: &ExerciseSettings) -> Self {
        let s = s.clamped();
        Self {
            rpm: f32::from(s.cadence_tolerance_rpm),
            allow_half: s.allow_half_time,
            allow_double: s.allow_double_time,
        }
    }

    fn ratios(&self) -> impl Iterator<Item = BeatRatio> {
        [(true, BeatRatio::Whole), (self.allow_half, BeatRatio::Half), (self.allow_double, BeatRatio::Double)]
            .into_iter()
            .filter_map(|(allowed, ratio)| allowed.then_some(ratio))
    }
}

/// A tempo that fits a cadence, and how well.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fit {
    pub ratio: BeatRatio,
    /// The cadence the song gives at that ratio.
    pub rpm: f32,
    /// How far it is outside the target range (0 inside).
    pub error_rpm: f32,
}

/// The best way to pedal to a song of `bpm` for `target`, `None` when no allowed ratio lands within the
/// tolerance of the range. Inside the range beats outside it; then the one nearest the range's center;
/// then the simplest ratio (1:1, half, double).
pub fn fit(bpm: f32, target: &Target, tolerance: &Tolerance) -> Option<Fit> {
    if !bpm.is_finite() || !(MIN_USABLE_BPM..=MAX_USABLE_BPM).contains(&bpm) {
        return None;
    }
    let range = target.cadence();
    let center = f32::from(range.center());
    tolerance
        .ratios()
        .map(|ratio| {
            let rpm = bpm * ratio.strokes_per_beat();
            Fit { ratio, rpm, error_rpm: range.distance(rpm) }
        })
        .filter(|f| f.error_rpm <= tolerance.rpm)
        .min_by(|a, b| {
            a.error_rpm.total_cmp(&b.error_rpm).then((a.rpm - center).abs().total_cmp(&(b.rpm - center).abs()))
        })
}

/// The song tempos (BPM, min..=max) that fit `range`: 1:1, then half-time and double-time when allowed.
/// For telling the AI what to look for.
pub fn bpm_windows(range: RpmRange, tolerance: &Tolerance) -> Vec<(u16, u16)> {
    let (low, high) = (f32::from(range.min_rpm) - tolerance.rpm, f32::from(range.max_rpm) + tolerance.rpm);
    tolerance
        .ratios()
        .map(|ratio| {
            let per = ratio.strokes_per_beat();
            let (a, b) = ((low / per).max(MIN_USABLE_BPM), (high / per).min(MAX_USABLE_BPM));
            (a.round() as u16, b.round() as u16)
        })
        .filter(|(a, b)| a <= b)
        .collect()
}

/// The ratio that gets closest to `range` among all three: for a rest phase inside a song that fits
/// the work phase. Never fails: the caller shows the pulse at whatever is nearest.
pub fn nearest_ratio(bpm: f32, range: RpmRange) -> BeatRatio {
    [BeatRatio::Whole, BeatRatio::Half, BeatRatio::Double]
        .into_iter()
        .min_by(|a, b| {
            range.distance(bpm * a.strokes_per_beat()).total_cmp(&range.distance(bpm * b.strokes_per_beat()))
        })
        .unwrap_or_default()
}

/// One step of the widening ladder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rung {
    pub tolerance: Tolerance,
    /// Look for more songs on Tidal before trying this rung.
    pub search_tidal: bool,
    /// Last resort: any song, with a metronome pulse instead of its beat.
    pub metronome: bool,
}

/// When nothing fits: the tolerance, then +3 RPM, then +6, then double-time allowed, then more songs
/// from Tidal, then any song with a metronome. Always the same six rungs in the same order.
pub fn ladder(base: &Tolerance) -> [Rung; 6] {
    let widen = |by: f32| Tolerance { rpm: base.rpm + by, ..*base };
    let doubled = Tolerance { allow_double: true, ..widen(2.0 * LADDER_STEP_RPM) };
    [
        Rung { tolerance: *base, search_tidal: false, metronome: false },
        Rung { tolerance: widen(LADDER_STEP_RPM), search_tidal: false, metronome: false },
        Rung { tolerance: widen(2.0 * LADDER_STEP_RPM), search_tidal: false, metronome: false },
        Rung { tolerance: doubled, search_tidal: false, metronome: false },
        Rung { tolerance: doubled, search_tidal: true, metronome: false },
        Rung { tolerance: doubled, search_tidal: false, metronome: true },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(min: u16, max: u16) -> Target {
        Target::Cadence { min_rpm: min, max_rpm: max }
    }

    fn tol() -> Tolerance {
        Tolerance::from_settings(&ExerciseSettings::default())
    }

    #[test]
    fn half_time_for_170_bpm_at_85_rpm() {
        let f = fit(170.0, &target(80, 90), &tol()).unwrap();
        assert_eq!(f.ratio, BeatRatio::Half);
        assert_eq!((f.rpm, f.error_rpm), (85.0, 0.0));
    }

    #[test]
    fn whole_time_for_slow_songs_and_the_simplest_ratio_on_a_tie() {
        assert_eq!(fit(90.0, &target(85, 95), &tol()).unwrap().ratio, BeatRatio::Whole);
        // 120 BPM at 60 RPM could be half-time only: 120 is far from 60 at 1:1
        assert_eq!(fit(120.0, &target(55, 65), &tol()).unwrap().ratio, BeatRatio::Half);
        // a range that both 1:1 and half-time reach equally: 1:1 wins
        let wide = Target::Cadence { min_rpm: 30, max_rpm: 140 };
        assert_eq!(fit(100.0, &wide, &tol()).unwrap().ratio, BeatRatio::Whole);
    }

    #[test]
    fn double_time_is_refused_by_default_and_works_when_allowed() {
        assert_eq!(fit(55.0, &target(105, 115), &tol()), None);
        let allowed = Tolerance { allow_double: true, ..tol() };
        let f = fit(55.0, &target(105, 115), &allowed).unwrap();
        assert_eq!((f.ratio, f.rpm), (BeatRatio::Double, 110.0));
    }

    #[test]
    fn tolerance_and_garbage_tempos() {
        // 76 RPM is 4 under the 80..90 range: inside the default tolerance, outside a tight one
        assert!(fit(76.0, &target(80, 90), &tol()).is_some());
        assert!(fit(76.0, &target(80, 90), &Tolerance { rpm: 2.0, ..tol() }).is_none());
        for bad in [f32::NAN, 0.0, 10.0, 400.0, f32::INFINITY] {
            assert!(fit(bad, &target(80, 90), &tol()).is_none());
        }
        let no_half = Tolerance { allow_half: false, ..tol() };
        assert!(fit(170.0, &target(80, 90), &no_half).is_none());
    }

    #[test]
    fn the_ladder_widens_in_a_fixed_order() {
        let rungs = ladder(&tol());
        let widths: Vec<f32> = rungs.iter().map(|r| r.tolerance.rpm).collect();
        assert_eq!(widths, [4.0, 7.0, 10.0, 10.0, 10.0, 10.0]);
        let flags: Vec<(bool, bool, bool)> =
            rungs.iter().map(|r| (r.tolerance.allow_double, r.search_tidal, r.metronome)).collect();
        assert_eq!(
            flags,
            [
                (false, false, false),
                (false, false, false),
                (false, false, false),
                (true, false, false),
                (true, true, false),
                (true, false, true)
            ]
        );
        assert_eq!(ladder(&tol()), rungs, "deterministic");
        // a 100 BPM song for a 55..65 climb fits only once widening and double... no: half is 50, 5 under
        assert!(fit(100.0, &target(55, 65), &rungs[0].tolerance).is_none());
        assert!(fit(100.0, &target(55, 65), &rungs[1].tolerance).is_some());
    }

    #[test]
    fn windows_tell_the_ai_which_tempos_fit() {
        assert_eq!(bpm_windows(RpmRange::new(80, 90), &tol()), [(76, 94), (152, 188)]);
        let all = Tolerance { allow_double: true, ..tol() };
        assert_eq!(bpm_windows(RpmRange::new(55, 65), &all), [(51, 69), (102, 138)]);
        // double-time would need songs under 40 BPM, which are not usable: no window
    }

    #[test]
    fn the_nearest_ratio_serves_rest_phases() {
        assert_eq!(nearest_ratio(170.0, RpmRange::new(80, 90)), BeatRatio::Half);
        assert_eq!(nearest_ratio(85.0, RpmRange::new(80, 90)), BeatRatio::Whole);
    }

    #[test]
    fn sources_and_ratios_serialize_in_snake_case() {
        assert_eq!(serde_json::to_string(&BpmSource::Analysis).unwrap(), "\"analysis\"");
        assert_eq!(serde_json::to_string(&BeatRatio::Half).unwrap(), "\"half\"");
    }
}
