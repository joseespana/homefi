//! The sampler's eight one-shots, synthesized here (no samples ship with HomeFi). Rendering is
//! deterministic: the same kind and rate always give the same samples.

use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

/// Longest one-shot (seconds).
pub const MAX_SFX_SECONDS: f32 = 3.0;
/// Peak every one-shot is scaled to.
pub const SFX_PEAK: f32 = 0.85;
/// Rate used when a caller passes 0.
const FALLBACK_RATE: u32 = 44_100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SfxKind {
    AirHorn,
    Siren,
    Laser,
    Rewind,
    Drop,
    Scratch,
    Riser,
    Hit,
}

impl SfxKind {
    /// Pad order: pad `i` plays `ALL[i]`.
    pub const ALL: [SfxKind; 8] =
        [Self::AirHorn, Self::Siren, Self::Laser, Self::Rewind, Self::Drop, Self::Scratch, Self::Riser, Self::Hit];

    pub fn seconds(self) -> f32 {
        match self {
            Self::AirHorn => 1.4,
            Self::Siren => 2.0,
            Self::Laser => 0.4,
            Self::Rewind => 1.2,
            Self::Drop => 1.6,
            Self::Scratch => 0.5,
            Self::Riser => 2.9,
            Self::Hit => 0.6,
        }
    }
}

/// Small deterministic noise source (xorshift32), `-1..1`.
pub(crate) struct Noise(pub(crate) u32);

impl Noise {
    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Phase accumulator for a frequency that changes over time.
#[derive(Default)]
pub(crate) struct Osc(pub(crate) f32);

impl Osc {
    pub(crate) fn sin(&mut self, hz: f32, rate: f32) -> f32 {
        self.0 = (self.0 + hz / rate).fract();
        (self.0 * TAU).sin()
    }
    pub(crate) fn saw(&mut self, hz: f32, rate: f32) -> f32 {
        self.0 = (self.0 + hz / rate).fract();
        self.0 * 2.0 - 1.0
    }
    pub(crate) fn square(&mut self, hz: f32, rate: f32) -> f32 {
        self.saw(hz, rate).signum()
    }
}

/// Mono samples of `kind` at `sample_rate` Hz, at most [`MAX_SFX_SECONDS`] long, peaking at
/// [`SFX_PEAK`], all finite, silent at both ends (no clicks).
pub fn render(kind: SfxKind, sample_rate: u32) -> Vec<f32> {
    let rate = if sample_rate == 0 { FALLBACK_RATE } else { sample_rate } as f32;
    let len = ((kind.seconds().min(MAX_SFX_SECONDS) * rate) as usize).max(1);
    let secs = kind.seconds();
    let mut noise = Noise(0x9E37_79B9);
    let (mut a, mut b, mut c) = (Osc::default(), Osc::default(), Osc::default());
    let mut lp = 0.0f32; // one-pole low-pass state for the noisy sounds

    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / rate; // seconds
            let u = t / secs; // 0..1
            match kind {
                SfxKind::AirHorn => {
                    // three stacked saw tones (a chord) with a fast attack and a short release
                    let env = (t / 0.02).min(1.0) * ((secs - t) / 0.15).clamp(0.0, 1.0);
                    let vibrato = 1.0 + 0.004 * (TAU * 6.0 * t).sin();
                    (a.saw(466.0 * vibrato, rate) + b.saw(587.0 * vibrato, rate) + c.saw(698.0 * vibrato, rate))
                        * 0.33
                        * env
                }
                SfxKind::Siren => {
                    // a triangle-ish sweep up and down twice
                    let sweep = 0.5 - 0.5 * (TAU * 2.0 * u).cos();
                    let hz = 600.0 + 800.0 * sweep;
                    let env = (t / 0.03).min(1.0) * ((secs - t) / 0.1).clamp(0.0, 1.0);
                    (a.sin(hz, rate) + 0.3 * b.sin(hz * 2.0, rate)) * env
                }
                SfxKind::Laser => {
                    let hz = 3_200.0 * (-6.0 * u).exp() + 250.0;
                    let env = (t / 0.003).min(1.0) * (1.0 - u).powi(2);
                    (a.square(hz, rate) * 0.6 + b.sin(hz * 1.5, rate) * 0.4) * env
                }
                SfxKind::Rewind => {
                    // a tape spinning backwards: falling pitch, flutter, hiss
                    let hz = 1_800.0 * (1.0 - u).powf(1.5) + 120.0;
                    let flutter = 0.6 + 0.4 * (TAU * 24.0 * t).sin();
                    let env = (t / 0.01).min(1.0) * ((secs - t) / 0.1).clamp(0.0, 1.0);
                    lp += 0.3 * (noise.next() - lp);
                    (a.saw(hz, rate) * 0.5 + lp * 0.5) * flutter * env
                }
                SfxKind::Drop => {
                    // a kick-like sine falling from 180 Hz to 35 Hz, with a click at the start
                    let hz = 35.0 + 145.0 * (-9.0 * t).exp();
                    let env = (t / 0.002).min(1.0) * (-2.2 * t).exp() * ((secs - t) / 0.1).clamp(0.0, 1.0);
                    let click = noise.next() * (-120.0 * t).exp() * 0.5;
                    a.sin(hz, rate) * env + click
                }
                SfxKind::Scratch => {
                    // noisy, band-limited, with the pitch of a record pushed back and forth
                    let wob = (TAU * 7.0 * t).sin();
                    let hz = 700.0 + 500.0 * wob;
                    let gate = (0.5 + 0.5 * (TAU * 7.0 * t + 1.2).sin()).powf(0.5);
                    lp += 0.45 * (noise.next() - lp);
                    let env = (t / 0.005).min(1.0) * ((secs - t) / 0.08).clamp(0.0, 1.0);
                    (a.saw(hz, rate) * 0.5 + lp) * gate * env
                }
                SfxKind::Riser => {
                    // rising tones under a swelling noise wash
                    let hz = 200.0 * (1.0 + 7.0 * u * u);
                    let env = u.powf(1.5) * ((secs - t) / 0.05).clamp(0.0, 1.0);
                    lp += (0.05 + 0.5 * u) * (noise.next() - lp);
                    (a.saw(hz, rate) * 0.35 + b.saw(hz * 1.005, rate) * 0.35 + lp * 1.2) * env
                }
                SfxKind::Hit => {
                    // an orchestral-style stab: noise burst over a low thump
                    let env = (t / 0.001).min(1.0) * (-7.0 * t).exp();
                    lp += 0.6 * (noise.next() - lp);
                    (lp * 0.8 + a.sin(70.0, rate) * 0.6 + c.sin(110.0, rate) * 0.3) * env
                }
            }
        })
        .collect();
    fade_ends(&mut out, rate);
    normalize(&mut out);
    out
}

/// A few milliseconds of fade at both ends: no click when a pad starts or ends.
pub(crate) fn fade_ends(samples: &mut [f32], rate: f32) {
    let fade = ((0.003 * rate) as usize).clamp(1, samples.len() / 2);
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        samples[i] *= g;
        let last = samples.len() - 1 - i;
        samples[last] *= g;
    }
}

pub(crate) fn normalize(samples: &mut [f32]) {
    for s in samples.iter_mut() {
        if !s.is_finite() {
            *s = 0.0;
        }
    }
    let peak = samples.iter().fold(0.0f32, |p, s| p.max(s.abs()));
    if peak > 0.0 {
        let k = SFX_PEAK / peak;
        samples.iter_mut().for_each(|s| *s *= k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_short_finite_loud_enough_and_safe() {
        for rate in [22_050, 44_100, 48_000, 96_000] {
            for kind in SfxKind::ALL {
                let s = render(kind, rate);
                assert!(!s.is_empty());
                assert!(s.len() as f32 <= MAX_SFX_SECONDS * rate as f32 + 1.0, "{kind:?} at {rate}");
                assert!(s.iter().all(|x| x.is_finite()), "{kind:?}");
                let peak = s.iter().fold(0.0f32, |p, x| p.max(x.abs()));
                assert!(peak <= 0.9 && peak > 0.5, "{kind:?} peak {peak}");
                let rms = (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
                assert!(rms > 0.02, "{kind:?} is nearly silent ({rms})");
                assert!(s[0].abs() < 0.05 && s[s.len() - 1].abs() < 0.05, "{kind:?} clicks at the ends");
            }
        }
    }

    #[test]
    fn rendering_is_deterministic_and_the_sounds_differ() {
        for kind in SfxKind::ALL {
            assert_eq!(render(kind, 48_000), render(kind, 48_000));
        }
        for (i, a) in SfxKind::ALL.iter().enumerate() {
            for b in &SfxKind::ALL[i + 1..] {
                assert_ne!(render(*a, 48_000), render(*b, 48_000));
            }
        }
        assert!(!render(SfxKind::Hit, 0).is_empty(), "rate 0 falls back");
    }

    #[test]
    fn kinds_serialize_in_snake_case() {
        assert_eq!(serde_json::to_string(&SfxKind::AirHorn).unwrap(), "\"air_horn\"");
        assert_eq!(SfxKind::ALL.len(), crate::dj::PAD_COUNT);
    }
}
