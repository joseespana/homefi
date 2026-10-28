//! The realtime equalizer: biquads in series, transposed direct form II,
//! one state per channel and band.

use super::{Biquad, EqBand, EqProfile, MAX_BANDS, MAX_PREAMP_DB, MIN_PREAMP_DB};

/// How long a profile change glides from the old filters to the new ones.
const RAMP_SECONDS: f64 = 0.02;

/// Frames a design change glides over at `sample_rate`: the preamp moves
/// linearly from the old gain to the new one, reaching it on the last of them
/// (the crossfade mirrors it to keep each song's level exact).
pub fn glide_frames(sample_rate: u32) -> u32 {
    ((sample_rate as f64 * RAMP_SECONDS) as u32).max(1)
}
/// Filter states below this are flushed to zero (no denormals after silence).
const DENORMAL: f64 = 1e-25;

/// The filters of a profile at one sample rate: fixed size and `Copy`, so it
/// can be handed to the audio callback without allocating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqDesign {
    stages: [Biquad; MAX_BANDS],
    len: usize,
    gain: f64,
}

impl EqDesign {
    /// Changes nothing; [`Equalizer`] skips the samples entirely.
    pub const BYPASS: Self = Self { stages: [Biquad::IDENTITY; MAX_BANDS], len: 0, gain: 1.0 };

    /// The enabled, non-neutral bands of `profile` (clamped) with `preamp_db`
    /// in front, at `sample_rate` Hz.
    pub fn new(profile: &EqProfile, preamp_db: f32, sample_rate: u32) -> Self {
        let mut d = Self::BYPASS;
        let bands = profile.bands.iter().take(MAX_BANDS).map(|b| b.clamped()).filter(|b| !b.is_neutral());
        for band in bands {
            d.stages[d.len] = Biquad::design(&band, sample_rate);
            d.len += 1;
        }
        let preamp = if preamp_db.is_nan() { 0.0 } else { preamp_db.clamp(MIN_PREAMP_DB, MAX_PREAMP_DB) };
        if preamp.abs() >= 0.01 {
            d.gain = 10f64.powf(preamp as f64 / 20.0);
        }
        d
    }

    /// One band alone, without preamp (handy for tests and previews).
    pub fn band(band: &EqBand, sample_rate: u32) -> Self {
        Self::new(&EqProfile { bands: vec![*band], ..Default::default() }, 0.0, sample_rate)
    }

    pub fn is_bypass(&self) -> bool {
        self.len == 0 && self.gain == 1.0
    }

    /// The filter stages, in order.
    pub fn stages(&self) -> &[Biquad] {
        &self.stages[..self.len]
    }

    /// Linear preamp gain.
    pub fn gain(&self) -> f64 {
        self.gain
    }

    /// `a` → `b` at `t` (0..=1). Missing stages are identities, so any two
    /// designs blend; the blend of two stable biquads is stable.
    fn lerp(a: &Self, b: &Self, t: f64) -> Self {
        let mut d = Self::BYPASS;
        d.len = a.len.max(b.len);
        for i in 0..d.len {
            d.stages[i] = Biquad::lerp(&a.stages[i], &b.stages[i], t);
        }
        d.gain = a.gain + (b.gain - a.gain) * t;
        d
    }
}

impl Default for EqDesign {
    fn default() -> Self {
        Self::BYPASS
    }
}

/// Runs a profile over interleaved samples. Changing the profile glides over
/// [`RAMP_SECONDS`] (no clicks). Nothing allocates after [`Equalizer::new`].
pub struct Equalizer {
    sample_rate: u32,
    channels: usize,
    /// What the filters are moving away from / towards.
    from: EqDesign,
    to: EqDesign,
    /// Frames left in the glide, of `ramp_len`.
    ramp_left: u32,
    ramp_len: u32,
    /// `[z1, z2]` per channel and stage: `state[ch * MAX_BANDS + stage]`.
    state: Vec<[f64; 2]>,
}

impl Equalizer {
    /// An equalizer running `profile` (with its own `preamp_db`).
    pub fn new(profile: &EqProfile, sample_rate: u32, channels: usize) -> Self {
        Self::with_design(EqDesign::new(profile, profile.preamp_db, sample_rate), sample_rate, channels)
    }

    /// An equalizer that starts with `design` (no glide).
    pub fn with_design(design: EqDesign, sample_rate: u32, channels: usize) -> Self {
        let channels = channels.max(1);
        Self {
            sample_rate,
            channels,
            from: design,
            to: design,
            ramp_left: 0,
            ramp_len: glide_frames(sample_rate),
            state: vec![[0.0; 2]; channels * MAX_BANDS],
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// `true` when the samples go through untouched (bit-perfect).
    pub fn is_bypass(&self) -> bool {
        self.ramp_left == 0 && self.to.is_bypass()
    }

    /// Glides to `profile` (with its own `preamp_db`).
    pub fn set_profile(&mut self, profile: &EqProfile) {
        self.set_design(EqDesign::new(profile, profile.preamp_db, self.sample_rate));
    }

    /// Glides to `design`, from wherever the filters are now. Realtime safe.
    pub fn set_design(&mut self, design: EqDesign) {
        if design == self.to {
            return;
        }
        self.from = self.current();
        self.to = design;
        self.ramp_left = self.ramp_len;
    }

    /// Clears the filter memory (after a seek or a new track) and finishes any glide.
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|s| *s = [0.0; 2]);
        self.from = self.to;
        self.ramp_left = 0;
    }

    /// Filters `samples` (interleaved, `channels` per frame) in place. A trailing
    /// partial frame is left as is. Non-finite input is treated as silence.
    pub fn process_interleaved(&mut self, samples: &mut [f32]) {
        if self.is_bypass() {
            return;
        }
        let channels = self.channels;
        for frame in samples.chunks_exact_mut(channels) {
            let design = if self.ramp_left > 0 {
                self.ramp_left -= 1;
                let d = self.current();
                if self.ramp_left == 0 {
                    self.from = self.to;
                }
                d
            } else {
                self.to
            };
            let mut blew_up = false;
            for (ch, sample) in frame.iter_mut().enumerate() {
                let state = &mut self.state[ch * MAX_BANDS..ch * MAX_BANDS + design.len];
                let y = run(&design, state, *sample);
                if y.is_finite() {
                    *sample = y as f32;
                } else {
                    *sample = 0.0;
                    blew_up = true;
                }
            }
            if blew_up {
                self.state.iter_mut().for_each(|s| *s = [0.0; 2]);
            }
            if self.is_bypass() {
                // glided to bypass: start clean when filters come back
                self.state.iter_mut().for_each(|s| *s = [0.0; 2]);
                break;
            }
        }
        for s in &mut self.state {
            for z in s.iter_mut() {
                if z.abs() < DENORMAL {
                    *z = 0.0;
                }
            }
        }
    }

    /// The filters at this point of the glide.
    fn current(&self) -> EqDesign {
        if self.ramp_left == 0 {
            return self.to;
        }
        let t = 1.0 - self.ramp_left as f64 / self.ramp_len as f64;
        EqDesign::lerp(&self.from, &self.to, t)
    }
}

/// One sample through the preamp and every stage of `design`.
#[inline]
fn run(design: &EqDesign, state: &mut [[f64; 2]], sample: f32) -> f64 {
    let mut x = if sample.is_finite() { sample as f64 * design.gain } else { 0.0 };
    for (q, z) in design.stages[..design.len].iter().zip(state.iter_mut()) {
        let y = q.b0 * x + z[0];
        z[0] = q.b1 * x - q.a1 * y + z[1];
        z[1] = q.b2 * x - q.a2 * y;
        x = y;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::super::{EqBandKind, Tilt};
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(freq: f64, frames: usize, channels: usize, amp: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = (2.0 * std::f64::consts::PI * freq * i as f64 / RATE as f64).sin() as f32 * amp;
                std::iter::repeat_n(v, channels)
            })
            .collect()
    }

    /// RMS in dB of channel `ch`, skipping the first `skip` frames (settling).
    fn rms_db(samples: &[f32], channels: usize, ch: usize, skip: usize) -> f64 {
        let v: Vec<f64> = samples.chunks_exact(channels).skip(skip).map(|f| f[ch] as f64).collect();
        let rms = (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
        20.0 * rms.log10()
    }

    fn peak(freq: f32, gain: f32, q: f32) -> EqProfile {
        EqProfile { bands: vec![EqBand::new(EqBandKind::Peak, freq, gain, q)], ..Default::default() }
    }

    #[test]
    fn flat_and_disabled_are_bit_perfect() {
        let input = sine(1000.0, 4800, 2, 0.5);
        for p in [EqProfile::default(), {
            let mut p = peak(1000.0, 6.0, 1.0);
            p.bands[0].enabled = false;
            p
        }] {
            let mut eq = Equalizer::new(&p, RATE, 2);
            assert!(eq.is_bypass());
            let mut out = input.clone();
            eq.process_interleaved(&mut out);
            assert_eq!(out, input);
        }
    }

    #[test]
    fn a_6_db_peak_measures_6_db_at_its_centre() {
        let mut eq = Equalizer::new(&peak(1000.0, 6.0, 1.0), RATE, 2);
        let input = sine(1000.0, RATE as usize, 2, 0.25);
        let mut out = input.clone();
        eq.process_interleaved(&mut out);
        for ch in 0..2 {
            let gain = rms_db(&out, 2, ch, 4800) - rms_db(&input, 2, ch, 4800);
            assert!((gain - 6.0).abs() < 0.05, "channel {ch}: {gain}");
        }
        // far from the centre: almost untouched
        let mut eq = Equalizer::new(&peak(1000.0, 6.0, 1.0), RATE, 1);
        let input = sine(80.0, RATE as usize, 1, 0.25);
        let mut out = input.clone();
        eq.process_interleaved(&mut out);
        assert!((rms_db(&out, 1, 0, 4800) - rms_db(&input, 1, 0, 4800)).abs() < 0.3);
    }

    #[test]
    fn shelf_lifts_the_bass() {
        let p = EqProfile { bands: vec![EqBand::new(EqBandKind::LowShelf, 200.0, 8.0, 0.7)], ..Default::default() };
        let mut eq = Equalizer::new(&p, RATE, 1);
        let input = sine(30.0, RATE as usize, 1, 0.1);
        let mut out = input.clone();
        eq.process_interleaved(&mut out);
        let gain = rms_db(&out, 1, 0, 9600) - rms_db(&input, 1, 0, 9600);
        assert!((gain - 8.0).abs() < 0.3, "{gain}");
    }

    #[test]
    fn auto_preamp_prevents_clipping() {
        // a heavy, summed bass boost on a full-scale bass tone
        let p = super::super::with_taste(&peak(60.0, 9.0, 0.8), Tilt { bass_db: 6.0, ..Default::default() });
        let pre = super::super::auto_preamp_db(&p);
        assert!(pre < -9.0, "{pre}");
        let mut eq = Equalizer::with_design(EqDesign::new(&p, pre, RATE), RATE, 1);
        let freqs = [30.0, 45.0, 60.0, 90.0, 120.0];
        for f in freqs {
            let mut out = sine(f, RATE as usize / 2, 1, 1.0);
            eq.process_interleaved(&mut out);
            let max = out[4800..].iter().fold(0f32, |m, s| m.max(s.abs()));
            assert!(max <= 1.0005, "{f} Hz peaks at {max}");
            eq.reset();
        }
    }

    #[test]
    fn device_taste_and_loudness_together_never_clip() {
        use super::super::{
            sound_plan, EqSettings, NowPlayingContext, TasteCreator, TasteRule, TasteScope, TasteSettings,
        };
        let mut eq = EqSettings { enabled: true, ..Default::default() };
        eq.per_output.insert("dac".into(), peak(60.0, 8.0, 0.8));
        let mut taste = TasteSettings::default();
        taste.upsert(TasteRule::new(
            TasteScope::Genre { name: "Dub".into() },
            Tilt { bass_db: 6.0, treble_db: 4.0, clarity_db: Some(3.0) },
            "",
            TasteCreator::Reaction,
        ));
        let ctx = NowPlayingContext { genres: vec!["Dub".into()], ..Default::default() };
        // a quiet master peaking at -6 dBFS, boosted to its peak by loudness
        let peak_amp = 0.5f32;
        let loud = super::super::LoudnessSettings { mode: super::super::LoudnessMode::Track, ..Default::default() };
        let rg =
            super::super::ReplayGain { track_gain_db: Some(9.0), track_peak: Some(peak_amp), ..Default::default() };
        let gain = loud.gain_db(&rg, false);
        assert!(gain > 5.9 && gain <= 6.03, "{gain}");
        let plan = sound_plan(&eq, &taste, "dac", &ctx, gain, RATE);
        let mut eq = Equalizer::with_design(plan.design, RATE, 1);
        for f in [30.0, 60.0, 120.0, 1000.0, 3000.0, 12_000.0, 18_000.0] {
            let mut out = sine(f, RATE as usize / 2, 1, peak_amp);
            eq.process_interleaved(&mut out);
            let max = out[4800..].iter().fold(0f32, |m, s| m.max(s.abs()));
            assert!(max <= 1.001, "{f} Hz peaks at {max}");
            eq.reset();
        }
    }

    #[test]
    fn changes_glide_without_clicks() {
        let input = sine(1000.0, RATE as usize, 1, 0.5);
        let mut eq = Equalizer::new(&EqProfile::default(), RATE, 1);
        let mut out = input.clone();
        let (a, b) = out.split_at_mut(RATE as usize / 2);
        eq.process_interleaved(a);
        eq.set_profile(&peak(1000.0, 12.0, 1.0));
        assert!(!eq.is_bypass());
        eq.process_interleaved(b);
        // no jump between two samples bigger than the 12 dB louder sine's own steepest slope
        let steepest = 4.0 * 0.5 * (2.0 * std::f32::consts::PI * 1000.0 / RATE as f32);
        let worst = out.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
        assert!(worst <= steepest * 1.1, "{worst} > {steepest}");
        // and it ends up at the new gain
        let tail = &out[RATE as usize - 4800..];
        let gain = rms_db(tail, 1, 0, 0) - rms_db(&input[..4800], 1, 0, 0);
        assert!((gain - 12.0).abs() < 0.1, "{gain}");

        // gliding back to flat ends bit-perfect
        eq.set_profile(&EqProfile::default());
        let mut more = input.clone();
        eq.process_interleaved(&mut more);
        assert!(eq.is_bypass());
        let mut again = input.clone();
        eq.process_interleaved(&mut again);
        assert_eq!(again, input);
    }

    #[test]
    fn nan_input_does_not_poison_the_filters() {
        let mut eq = Equalizer::new(&peak(100.0, 6.0, 2.0), RATE, 2);
        let mut buf = vec![f32::NAN, f32::INFINITY, 0.5, -0.5];
        eq.process_interleaved(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()), "{buf:?}");
        let mut more = sine(100.0, 4800, 2, 0.5);
        eq.process_interleaved(&mut more);
        assert!(more.iter().all(|s| s.is_finite()));
        // a NaN profile is treated as flat values
        let mut p = peak(f32::NAN, f32::NAN, f32::NAN);
        p.preamp_db = f32::NAN;
        let mut eq = Equalizer::new(&p, RATE, 1);
        let mut buf = sine(1000.0, 480, 1, 0.5);
        eq.process_interleaved(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn reset_clears_the_memory() {
        let mut eq = Equalizer::new(&peak(50.0, 12.0, 5.0), RATE, 1);
        let mut buf = sine(50.0, 4800, 1, 1.0);
        eq.process_interleaved(&mut buf);
        eq.reset();
        let mut silence = vec![0.0f32; 64];
        eq.process_interleaved(&mut silence);
        assert!(silence.iter().all(|&s| s == 0.0));
    }
}
