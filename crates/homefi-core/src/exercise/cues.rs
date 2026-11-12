//! The ride's sounds, synthesized here (nothing ships with HomeFi) with the DJ sampler's helpers.
//! Rendering is deterministic, at most a second long, finite, silent at both ends and peaks at 0.85.

use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

use super::ride::RideView;
use crate::dj::sfx::{fade_ends, normalize, Noise, Osc};

/// Rate used when a caller passes 0.
const FALLBACK_RATE_HZ: u32 = 44_100;
/// Longest cue (s).
pub const MAX_CUE_SECONDS: f32 = 1.0;
const PCM_BITS: u16 = 16;
const WAV_HEADER_BYTES: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Countdown3,
    Countdown2,
    Countdown1,
    /// A new segment, nothing special about the position.
    Change,
    /// Stand up.
    Standing,
    /// Sit down.
    Seated,
    SprintGo,
    /// A sprint's work phase ends: ease off.
    Recover,
    Finish,
    /// One pedal stroke of the metronome.
    Click,
    /// A climb asks for one more turn of resistance: two rising notes, apart from the stand-up glide.
    Resistance,
}

impl CueKind {
    pub const ALL: [CueKind; 11] = [
        Self::Countdown3,
        Self::Countdown2,
        Self::Countdown1,
        Self::Change,
        Self::Standing,
        Self::Seated,
        Self::SprintGo,
        Self::Recover,
        Self::Finish,
        Self::Click,
        Self::Resistance,
    ];

    pub fn seconds(self) -> f32 {
        match self {
            Self::Countdown3 | Self::Countdown2 | Self::Countdown1 => 0.2,
            Self::Change => 0.5,
            Self::Standing | Self::Seated => 0.45,
            Self::SprintGo => 0.7,
            Self::Recover => 0.6,
            Self::Finish => 0.95,
            Self::Click => 0.04,
            Self::Resistance => 0.5,
        }
    }
}

/// Notes (Hz) of the finish arpeggio: C5, E5, G5, C6.
const FINISH_NOTES: [f32; 4] = [523.25, 659.25, 783.99, 1046.5];
/// Cues sit well under the music: half of the DJ pads' peak (-6 dB), before the user's cue volume.
const CUE_LEVEL: f32 = 0.5;
/// Soft attack of the tonal cues: a quick swell instead of a hard edge that makes the rider jump.
const SOFT_ATTACK_S: f32 = 0.02;

/// Mono samples of `kind` at `sample_rate` Hz.
pub fn render(kind: CueKind, sample_rate: u32) -> Vec<f32> {
    let rate = if sample_rate == 0 { FALLBACK_RATE_HZ } else { sample_rate } as f32;
    let secs = kind.seconds().min(MAX_CUE_SECONDS);
    let len = ((secs * rate) as usize).max(2);
    let (mut a, mut b, mut c) = (Osc::default(), Osc::default(), Osc::default());
    let mut noise = Noise(0x2545_F491);
    let mut out: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / rate;
            let u = t / secs;
            let tail = ((secs - t) / 0.05).clamp(0.0, 1.0);
            match kind {
                CueKind::Countdown3 | CueKind::Countdown2 | CueKind::Countdown1 => {
                    let hz = match kind {
                        CueKind::Countdown3 => 523.25,
                        CueKind::Countdown2 => 587.33,
                        _ => 659.25,
                    };
                    a.sin(hz, rate) * (t / SOFT_ATTACK_S).min(1.0) * tail
                }
                CueKind::Change => {
                    // two soft notes, a third up (C5, E5)
                    let hz = if u < 0.4 { 523.25 } else { 659.25 };
                    (a.sin(hz, rate) + 0.12 * b.sin(hz * 2.0, rate))
                        * (t / SOFT_ATTACK_S).min(1.0)
                        * (-3.0 * u).exp()
                        * tail
                }
                CueKind::Standing => {
                    let hz = 440.0 + 220.0 * u;
                    (a.sin(hz, rate) + 0.12 * b.sin(hz * 2.0, rate)) * (t / SOFT_ATTACK_S).min(1.0) * tail
                }
                CueKind::Seated => {
                    let hz = 660.0 - 220.0 * u;
                    (a.sin(hz, rate) + 0.12 * b.sin(hz * 2.0, rate)) * (t / SOFT_ATTACK_S).min(1.0) * tail
                }
                CueKind::SprintGo => {
                    // a warm major chord (sines, a slight vibrato) that swells in: energy without a blast
                    let vibrato = 1.0 + 0.004 * (TAU * 6.0 * t).sin();
                    (a.sin(392.0 * vibrato, rate) + b.sin(493.88 * vibrato, rate) + c.sin(587.33 * vibrato, rate))
                        * (t / 0.06).min(1.0)
                        * tail
                }
                CueKind::Recover => {
                    // a soft low sigh: a falling sine
                    let hz = 440.0 - 110.0 * u;
                    a.sin(hz, rate) * (t / 0.03).min(1.0) * (1.0 - u).powf(0.7) * tail
                }
                CueKind::Finish => {
                    let step = ((u * FINISH_NOTES.len() as f32) as usize).min(FINISH_NOTES.len() - 1);
                    let into = u * FINISH_NOTES.len() as f32 - step as f32;
                    (a.sin(FINISH_NOTES[step], rate) + 0.3 * b.sin(FINISH_NOTES[step] * 2.0, rate))
                        * (1.0 - 0.6 * into)
                        * tail
                }
                CueKind::Resistance => {
                    // E5 then G5: a step up, each note a short bell-like swell
                    let first = u < 0.5;
                    let hz = if first { 659.25 } else { 783.99 };
                    let into = if first { u * 2.0 } else { u * 2.0 - 1.0 };
                    (a.sin(hz, rate) + 0.12 * b.sin(hz * 2.0, rate))
                        * (t / SOFT_ATTACK_S).min(1.0)
                        * (-2.5 * into).exp()
                        * tail
                }
                CueKind::Click => (a.sin(1_000.0, rate) * 0.9 + noise.next() * 0.1) * (-6.0 * u).exp(),
            }
        })
        .collect();
    fade_ends(&mut out, rate);
    normalize(&mut out);
    out.iter_mut().for_each(|s| *s *= CUE_LEVEL);
    out
}

/// `kind` as a mono 16-bit WAV file's bytes (the app plays it with `SoundPool` or WebAudio).
pub fn wav(kind: CueKind, sample_rate: u32) -> Vec<u8> {
    let rate = if sample_rate == 0 { FALLBACK_RATE_HZ } else { sample_rate };
    wav_16bit_mono(&render(kind, rate), rate)
}

/// Mono samples (-1..1) as a 16-bit PCM WAV file.
pub fn wav_16bit_mono(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(WAV_HEADER_BYTES + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&PCM_BITS.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

/// The cues of `view` that are still to come, with the moment each one is due (the view's own clock,
/// ms), in order. A cue already past (a view that arrived late) is left out.
pub fn schedule(view: &RideView) -> Vec<(u64, CueKind)> {
    view.cues.iter().filter(|c| c.in_ms >= 0).map(|c| (view.at_ms + c.in_ms as u64, c.kind)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::ride::Cue;

    #[test]
    fn every_cue_is_short_finite_loud_enough_and_safe() {
        for rate in [22_050, 44_100, 48_000] {
            for kind in CueKind::ALL {
                let s = render(kind, rate);
                assert!(s.len() as f32 <= MAX_CUE_SECONDS * rate as f32 + 1.0, "{kind:?}");
                assert!(s.iter().all(|x| x.is_finite()), "{kind:?}");
                let peak = s.iter().fold(0.0f32, |p, x| p.max(x.abs()));
                assert!(peak <= 0.85 * CUE_LEVEL + 1e-4 && peak > 0.5 * CUE_LEVEL, "{kind:?} peak {peak}");
                let rms = (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
                assert!(rms > 0.01, "{kind:?} is nearly silent ({rms})");
                assert!(s[0].abs() < 0.05 && s[s.len() - 1].abs() < 0.05, "{kind:?} clicks at the ends");
            }
        }
    }

    #[test]
    fn rendering_is_deterministic_and_the_cues_differ() {
        for kind in CueKind::ALL {
            assert_eq!(render(kind, 48_000), render(kind, 48_000));
        }
        for (i, a) in CueKind::ALL.iter().enumerate() {
            for b in &CueKind::ALL[i + 1..] {
                assert_ne!(render(*a, 48_000), render(*b, 48_000), "{a:?} and {b:?}");
            }
        }
        assert!(!render(CueKind::Click, 0).is_empty(), "rate 0 falls back");
    }

    #[test]
    fn the_wav_is_valid() {
        let bytes = wav(CueKind::Finish, 44_100);
        assert_eq!((&bytes[..4], &bytes[8..12]), (&b"RIFF"[..], &b"WAVE"[..]));
        let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), WAV_HEADER_BYTES + data);
        assert_eq!(serde_json::to_string(&CueKind::SprintGo).unwrap(), "\"sprint_go\"");
        assert_eq!(serde_json::to_string(&CueKind::Resistance).unwrap(), "\"resistance\"");
    }

    #[test]
    fn the_schedule_lists_what_is_still_to_come() {
        let view = RideView {
            at_ms: 10_000,
            cues: vec![
                Cue { id: 1, kind: CueKind::Change, in_ms: -300 },
                Cue { id: 2, kind: CueKind::Countdown1, in_ms: 0 },
                Cue { id: 3, kind: CueKind::Standing, in_ms: 1_000 },
            ],
            ..RideView::default()
        };
        assert_eq!(schedule(&view), [(10_000, CueKind::Countdown1), (11_000, CueKind::Standing)]);
    }
}
