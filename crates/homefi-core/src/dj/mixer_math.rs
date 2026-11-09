//! The mixer's maths: channel EQ with kill, the bipolar filter, crossfader curves and auto-gain.
//! Pure functions over `dsp` building blocks; the real-time mixer (`homefi-player::dj`) and
//! the Android DJ screen both call them, so the sound is the same everywhere.

use serde::{Deserialize, Serialize};

use crate::dsp::crossfade::{self, FadeCurve};
use crate::dsp::{Biquad, EqBand, EqBandKind, EqProfile};

/// Gain of a killed band (dB). Two stages of -24 dB each, so a kill takes the band well
/// below -40 dB where it is plainly silent.
pub const EQ_KILL_DB: f32 = -48.0;
/// Corner of the low shelf, and the point the mid band starts to take over (Hz).
pub const EQ_LOW_HZ: f32 = 250.0;
pub const EQ_MID_HZ: f32 = 1_000.0;
/// Corner of the high shelf (Hz).
pub const EQ_HIGH_HZ: f32 = 2_500.0;
/// Width of the mid peak: wide, so its neighbours blend into the shelves.
const EQ_MID_Q: f32 = 0.6;
const EQ_SHELF_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Every band is two identical stages of half the gain (the range of one stage is +/-24 dB).
const STAGES_PER_BAND: usize = 2;
/// Biquad stages of one channel's EQ.
pub const EQ_STAGES: usize = 3 * STAGES_PER_BAND;
/// Gains below this (dB) leave a band out.
const NEUTRAL_DB: f32 = 0.05;

/// Filter knob: positions closer to the middle than this do nothing.
pub const FILTER_DEADZONE: f32 = 0.03;
/// How far past the dead zone the filter takes to be fully in (knob travel), so that it fades in
/// instead of switching on: a high-pass or low-pass changes the phase, not just the level.
pub const FILTER_BLEND: f32 = 0.06;
/// Corner of the low-pass at the knob's left end, of the high-pass at its right end (Hz).
pub const FILTER_LOW_END_HZ: f32 = 200.0;
pub const FILTER_HIGH_START_HZ: f32 = 20.0;
pub const FILTER_HIGH_END_HZ: f32 = 8_000.0;
pub const FILTER_OPEN_HZ: f32 = 20_000.0;
const FILTER_Q: f32 = 0.9;

/// Level every song is matched to by auto-gain (RMS dBFS of its loudest parts).
pub const AUTO_GAIN_TARGET_DB: f32 = -16.0;
pub const MIN_AUTO_GAIN_DB: f32 = -12.0;
pub const MAX_AUTO_GAIN_DB: f32 = 9.0;
/// Trim knob range (dB).
pub const MIN_TRIM_DB: f32 = -12.0;
pub const MAX_TRIM_DB: f32 = 12.0;

/// How far the cut curve's slopes reach into the fader (fraction of its travel).
const CUT_EDGE: f32 = 0.05;

/// Gain of each EQ band in dB ([`EQ_KILL_DB`] kills it).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EqGains {
    pub low_db: f32,
    pub mid_db: f32,
    pub high_db: f32,
}

impl EqGains {
    /// Gains inside `EQ_KILL_DB..=max_boost_db` (NaN becomes 0).
    pub fn clamped(self, max_boost_db: f32) -> Self {
        let f = |v: f32| if v.is_nan() { 0.0 } else { v.clamp(EQ_KILL_DB, max_boost_db.max(0.0)) };
        Self { low_db: f(self.low_db), mid_db: f(self.mid_db), high_db: f(self.high_db) }
    }

    pub fn is_flat(&self) -> bool {
        [self.low_db, self.mid_db, self.high_db].iter().all(|g| g.abs() < NEUTRAL_DB)
    }
}

/// The three bands as a `dsp` profile (six bands: two stages each), for any EQ engine.
pub fn channel_eq_profile(gains: EqGains) -> EqProfile {
    let mut profile = EqProfile::flat("DJ channel");
    profile.bands = channel_eq_bands(gains).to_vec();
    profile
}

/// The six filters of a channel EQ, neutral bands included (a 0 dB band is a unity gain).
fn channel_eq_bands(gains: EqGains) -> [EqBand; EQ_STAGES] {
    let half = |db: f32| db / STAGES_PER_BAND as f32;
    let low = EqBand::new(EqBandKind::LowShelf, EQ_LOW_HZ, half(gains.low_db), EQ_SHELF_Q);
    let mid = EqBand::new(EqBandKind::Peak, EQ_MID_HZ, half(gains.mid_db), EQ_MID_Q);
    let high = EqBand::new(EqBandKind::HighShelf, EQ_HIGH_HZ, half(gains.high_db), EQ_SHELF_Q);
    [low, low, mid, mid, high, high]
}

/// Biquad coefficients of a channel EQ at `sample_rate`; flat bands are exact identities
/// (the mixer skips them).
pub fn channel_eq_stages(gains: EqGains, sample_rate: u32) -> [Biquad; EQ_STAGES] {
    let bands = channel_eq_bands(gains);
    std::array::from_fn(|i| {
        let band = bands[i].clamped();
        if band.is_neutral() {
            Biquad::IDENTITY
        } else {
            Biquad::design(&band, sample_rate)
        }
    })
}

/// The filter knob (-1 = closed low-pass, 0 = off, +1 = closed high-pass) as one band,
/// `None` in the dead zone around the middle.
pub fn filter_band(position: f32) -> Option<EqBand> {
    let p = if position.is_nan() { 0.0 } else { position.clamp(-1.0, 1.0) };
    if p.abs() < FILTER_DEADZONE {
        return None;
    }
    // exponential sweeps feel linear to the ear
    let freq = if p < 0.0 {
        FILTER_OPEN_HZ * (FILTER_LOW_END_HZ / FILTER_OPEN_HZ).powf(-p)
    } else {
        FILTER_HIGH_START_HZ * (FILTER_HIGH_END_HZ / FILTER_HIGH_START_HZ).powf(p)
    };
    let kind = if p < 0.0 { EqBandKind::LowPass } else { EqBandKind::HighPass };
    Some(EqBand::new(kind, freq, 0.0, FILTER_Q))
}

/// How much of the filter is heard at `position`: 0 in the dead zone, 1 from `FILTER_BLEND` past it.
pub fn filter_wet(position: f32) -> f32 {
    if position.is_nan() {
        return 0.0;
    }
    ((position.abs() - FILTER_DEADZONE) / FILTER_BLEND).clamp(0.0, 1.0)
}

/// Coefficients of the DJ filter (a state-variable filter in the trapezoidal form: unlike a
/// biquad it stays well behaved while its corner sweeps, which is exactly how a DJ uses it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterCoeffs {
    a1: f64,
    a2: f64,
    a3: f64,
    k: f64,
    low_pass: bool,
}

/// The filter knob at `sample_rate`, `None` when it does nothing.
pub fn filter_coeffs(position: f32, sample_rate: u32) -> Option<FilterCoeffs> {
    let band = filter_band(position)?;
    let rate = if sample_rate == 0 { 48_000.0 } else { sample_rate as f64 };
    let freq = (band.freq_hz as f64).min(rate * 0.45);
    let g = (std::f64::consts::PI * freq / rate).tan();
    let k = 1.0 / band.q as f64;
    let a1 = 1.0 / (1.0 + g * (g + k));
    Some(FilterCoeffs { a1, a2: g * a1, a3: g * g * a1, k, low_pass: band.kind == EqBandKind::LowPass })
}

/// State of [`FilterCoeffs`] on one channel.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterState {
    ic1: f64,
    ic2: f64,
}

impl FilterState {
    #[inline]
    pub fn process(&mut self, c: &FilterCoeffs, x: f64) -> f64 {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        let y = if c.low_pass { v2 } else { x - c.k * v1 - v2 };
        if y.is_finite() {
            y
        } else {
            *self = Self::default();
            0.0
        }
    }

    /// Starts as if the constant `x` had been going in for ever, so a low-pass engaged in the
    /// middle of a song does not step the signal. (A high-pass starts from rest, which is
    /// already seamless.)
    pub fn prime(&mut self, c: &FilterCoeffs, x: f64) {
        *self = Self::default();
        if c.low_pass {
            self.ic2 = x;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Direct-form II transposed state of one biquad on one channel.
#[derive(Debug, Clone, Copy, Default)]
pub struct BiquadState {
    z1: f64,
    z2: f64,
}

/// Filter states below this are flushed (no denormals after silence).
const DENORMAL: f64 = 1e-25;

impl BiquadState {
    #[inline]
    pub fn process(&mut self, q: &Biquad, x: f64) -> f64 {
        let y = q.b0 * x + self.z1;
        self.z1 = q.b1 * x - q.a1 * y + self.z2;
        self.z2 = q.b2 * x - q.a2 * y;
        if self.z1.abs() < DENORMAL {
            self.z1 = 0.0;
        }
        if self.z2.abs() < DENORMAL {
            self.z2 = 0.0;
        }
        y
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// How the crossfader blends the two decks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CrossfaderCurve {
    /// Straight volume ramps.
    Linear,
    /// Both decks at full volume until the very ends (scratch style).
    Cut,
    /// Equal power: constant loudness across the whole travel.
    #[default]
    #[serde(other)]
    Smooth,
}

impl CrossfaderCurve {
    /// Gains `(deck A, deck B)` at `position` (0 = all A, 1 = all B).
    pub fn gains(self, position: f32) -> (f32, f32) {
        let t = if position.is_nan() { 0.5 } else { position.clamp(0.0, 1.0) };
        match self {
            Self::Smooth => crossfade::gains(FadeCurve::EqualPower, t),
            Self::Linear => crossfade::gains(FadeCurve::Linear, t),
            Self::Cut => (((1.0 - t) / CUT_EDGE).min(1.0), (t / CUT_EDGE).min(1.0)),
        }
    }
}

/// Trim (dB) that brings a song of `loudness_db` to [`AUTO_GAIN_TARGET_DB`]; 0 when unknown.
pub fn auto_gain_db(loudness_db: f32) -> f32 {
    if loudness_db.is_finite() {
        (AUTO_GAIN_TARGET_DB - loudness_db).clamp(MIN_AUTO_GAIN_DB, MAX_AUTO_GAIN_DB)
    } else {
        0.0
    }
}

/// dB to an amplitude factor.
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn response_db(gains: EqGains, hz: f64) -> f64 {
        channel_eq_stages(gains, RATE).iter().map(|q| q.magnitude_db(hz, RATE)).sum()
    }

    #[test]
    fn flat_eq_is_transparent() {
        let flat = EqGains::default();
        assert!(flat.is_flat());
        assert!(channel_eq_stages(flat, RATE).iter().all(|q| *q == Biquad::IDENTITY));
        for hz in [60.0, 1000.0, 10_000.0] {
            assert!(response_db(flat, hz).abs() < 1e-9);
        }
    }

    #[test]
    fn kill_silences_its_band_and_leaves_the_others() {
        let low = EqGains { low_db: EQ_KILL_DB, ..Default::default() };
        assert!(response_db(low, 60.0) <= -26.0, "{}", response_db(low, 60.0));
        assert!(response_db(low, 10_000.0).abs() < 1.0);
        let high = EqGains { high_db: EQ_KILL_DB, ..Default::default() };
        assert!(response_db(high, 12_000.0) <= -26.0);
        assert!(response_db(high, 60.0).abs() < 1.0);
        let mid = EqGains { mid_db: EQ_KILL_DB, ..Default::default() };
        assert!(response_db(mid, 1000.0) <= -26.0);
    }

    #[test]
    fn boost_and_cut_land_where_asked() {
        let g = EqGains { low_db: 6.0, mid_db: -6.0, high_db: 3.0 };
        assert!((response_db(g, 40.0) - 6.0).abs() < 0.8);
        assert!((response_db(g, 1000.0) + 6.0).abs() < 0.5);
        assert!((response_db(g, 16_000.0) - 3.0).abs() < 0.8);
        assert_eq!(channel_eq_profile(g).bands.len(), EQ_STAGES);
        let clamped = EqGains { low_db: 40.0, mid_db: f32::NAN, high_db: -900.0 }.clamped(6.0);
        assert_eq!((clamped.low_db, clamped.mid_db, clamped.high_db), (6.0, 0.0, EQ_KILL_DB));
    }

    #[test]
    fn filter_sweeps_both_ways_with_a_dead_zone() {
        assert!(filter_band(0.0).is_none() && filter_band(0.02).is_none() && filter_band(f32::NAN).is_none());
        let lp = filter_band(-1.0).unwrap();
        assert_eq!(lp.kind, EqBandKind::LowPass);
        assert!((lp.freq_hz - FILTER_LOW_END_HZ).abs() < 1.0);
        let hp = filter_band(1.0).unwrap();
        assert_eq!(hp.kind, EqBandKind::HighPass);
        assert!((hp.freq_hz - FILTER_HIGH_END_HZ).abs() < 1.0);
        // further from the middle, further the corner moves
        assert!(filter_band(-0.8).unwrap().freq_hz < filter_band(-0.3).unwrap().freq_hz);
        assert!(filter_band(0.8).unwrap().freq_hz > filter_band(0.3).unwrap().freq_hz);
        // measured on a sine through the filter
        let gain_at = |position: f32, hz: f32| {
            let c = filter_coeffs(position, RATE).unwrap();
            let mut st = FilterState::default();
            let n = RATE as usize;
            let out: Vec<f64> = (0..n)
                .map(|i| st.process(&c, (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin() as f64))
                .collect();
            let peak = out[n / 2..].iter().fold(0f64, |p, v| p.max(v.abs()));
            20.0 * peak.log10()
        };
        assert!(gain_at(-1.0, 5000.0) < -20.0 && gain_at(-1.0, 50.0).abs() < 1.0);
        assert!(gain_at(1.0, 100.0) < -20.0 && gain_at(1.0, 16_000.0).abs() < 3.0);
        assert!(filter_coeffs(0.0, RATE).is_none());
        assert_eq!(filter_wet(0.0), 0.0);
        assert_eq!(filter_wet(-0.03), 0.0);
        assert!(filter_wet(0.06) > 0.4 && filter_wet(0.06) < 0.6);
        assert_eq!((filter_wet(-1.0), filter_wet(1.0), filter_wet(f32::NAN)), (1.0, 1.0, 0.0));
    }

    #[test]
    fn a_primed_low_pass_does_not_step_the_signal() {
        let c = filter_coeffs(-0.5, RATE).unwrap();
        let mut cold = FilterState::default();
        let mut primed = FilterState::default();
        primed.prime(&c, 0.3);
        // constant input: the primed filter outputs it from the first sample
        assert!((primed.process(&c, 0.3) - 0.3).abs() < 1e-9);
        assert!((cold.process(&c, 0.3) - 0.3).abs() > 0.01, "a cold start steps");
        // a high-pass starts from rest
        let hp = filter_coeffs(0.5, RATE).unwrap();
        let mut s = FilterState::default();
        s.prime(&hp, 0.3);
        assert!(s.process(&hp, 0.3) > 0.25);
    }

    #[test]
    fn a_sweeping_filter_stays_bounded() {
        let mut st = FilterState::default();
        let mut peak = 0f64;
        for i in 0..200_000usize {
            let pos = ((i as f32 / 3000.0).sin() * 1.1).clamp(-1.0, 1.0);
            let x = (std::f32::consts::TAU * 100.0 * i as f32 / RATE as f32).sin() as f64;
            let y = match filter_coeffs(pos, RATE) {
                Some(c) => st.process(&c, x),
                None => {
                    st.reset();
                    x
                }
            };
            peak = peak.max(y.abs());
        }
        assert!(peak < 1.6, "{peak}");
    }

    #[test]
    fn crossfader_curves_hit_the_ends_and_keep_power() {
        for curve in [CrossfaderCurve::Smooth, CrossfaderCurve::Linear, CrossfaderCurve::Cut] {
            let (a, b) = curve.gains(0.0);
            assert!((a - 1.0).abs() < 1e-6 && b.abs() < 1e-6, "{curve:?}");
            let (a, b) = curve.gains(1.0);
            assert!(a.abs() < 1e-6 && (b - 1.0).abs() < 1e-6, "{curve:?}");
            assert_eq!(curve.gains(f32::NAN), curve.gains(0.5));
            assert_eq!(curve.gains(-3.0), curve.gains(0.0));
        }
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let (a, b) = CrossfaderCurve::Smooth.gains(t);
            assert!((a * a + b * b - 1.0).abs() < 1e-5, "power at {t}");
            let (a, b) = CrossfaderCurve::Linear.gains(t);
            assert!((a + b - 1.0).abs() < 1e-5, "sum at {t}");
        }
        assert_eq!(CrossfaderCurve::Cut.gains(0.5), (1.0, 1.0));
        let (a, b) = CrossfaderCurve::Smooth.gains(0.5);
        assert!((a - b).abs() < 1e-6);
    }

    #[test]
    fn auto_gain_matches_levels_within_limits() {
        assert_eq!(auto_gain_db(-16.0), 0.0);
        assert_eq!(auto_gain_db(-20.0), 4.0);
        assert_eq!(auto_gain_db(-40.0), MAX_AUTO_GAIN_DB);
        assert_eq!(auto_gain_db(0.0), MIN_AUTO_GAIN_DB);
        assert_eq!(auto_gain_db(f32::NEG_INFINITY), 0.0);
        assert!((db_to_gain(-6.0) - 0.501).abs() < 0.01);
    }
}
