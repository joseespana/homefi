//! Voices the engine mixes: a [`Deck`] decodes one track at the open
//! output's rate and channels, a [`Fade`] sums the end of the outgoing track
//! with the start of the incoming one (crossfade).
//!
//! A deck is the unit a future DJ mode builds on: two decks, each with its own
//! gain envelope, summed by the engine thread before the ring buffer. Nothing
//! here runs in the realtime callback; the callback only sees mixed samples.

use homefi_core::dsp::crossfade::{gains, FadeCurve, SILENCE_THRESHOLD};
use rubato::{Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use serde::{Deserialize, Serialize};

use crate::decoder::Source;
use crate::error::{PlayerError, Result};

/// Resampler input chunk (frames).
pub(crate) const RESAMPLE_CHUNK: usize = 1024;
/// The soft limiter leaves everything below this untouched.
const LIMIT_KNEE: f32 = 0.9;

/// How carefully a track at another rate is converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ResamplerQuality {
    /// 256-tap sinc: inaudible, a few % of a core per stereo stream.
    #[default]
    High,
    /// 64-tap sinc: still clean (cut-off a bit lower), about a quarter of the CPU.
    Fast,
}

/// Converts between sample rates with a sinc filter.
pub(crate) struct Resampler {
    inner: SincFixedIn<f32>,
    channels: usize,
    input: Vec<Vec<f32>>,
}

impl Resampler {
    pub fn new(from: u32, to: u32, channels: usize, quality: ResamplerQuality) -> Result<Self> {
        let params = match quality {
            ResamplerQuality::High => SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            },
            ResamplerQuality::Fast => SincInterpolationParameters {
                sinc_len: 64,
                f_cutoff: 0.91,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::Blackman2,
            },
        };
        let channels = channels.max(1);
        let inner = SincFixedIn::new(to as f64 / from as f64, 1.0, params, RESAMPLE_CHUNK, channels)
            .map_err(|e| PlayerError::Resample(e.to_string()))?;
        Ok(Self { inner, channels, input: vec![Vec::with_capacity(RESAMPLE_CHUNK * 2); channels] })
    }

    /// Feeds interleaved samples, appends resampled interleaved samples to `out`.
    pub fn process(&mut self, interleaved: &[f32], out: &mut Vec<f32>) -> Result<()> {
        for frame in interleaved.chunks_exact(self.channels) {
            for (ch, s) in frame.iter().enumerate() {
                self.input[ch].push(*s);
            }
        }
        while self.input[0].len() >= RESAMPLE_CHUNK {
            let chunk: Vec<Vec<f32>> = self.input.iter_mut().map(|c| c.drain(..RESAMPLE_CHUNK).collect()).collect();
            let resampled = self.inner.process(&chunk, None).map_err(|e| PlayerError::Resample(e.to_string()))?;
            interleave(&resampled, out);
        }
        Ok(())
    }

    /// Pads the tail with silence so the last samples come out.
    pub fn flush(&mut self, out: &mut Vec<f32>) -> Result<()> {
        if self.input[0].is_empty() {
            return Ok(());
        }
        let pad = RESAMPLE_CHUNK - self.input[0].len();
        let silence = vec![0.0; pad * self.channels];
        self.process(&silence, out)
    }
}

fn interleave(channels: &[Vec<f32>], out: &mut Vec<f32>) {
    let frames = channels.first().map(Vec::len).unwrap_or(0);
    out.reserve(frames * channels.len());
    for i in 0..frames {
        for ch in channels {
            out.push(ch[i]);
        }
    }
}

/// What a stereo (or mono) track does on a device with more speakers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SurroundMode {
    /// Front left / right only, the other speakers silent (the samples untouched).
    #[default]
    FrontOnly,
    /// Front as is, the stereo copied to the side and rear speakers at -3 dB
    /// (no center, no subwoofer).
    AllSpeakers,
    /// Front as is, center = (L+R)/2 at -3 dB, subwoofer = the mono below
    /// ~120 Hz, rears = L / R at -6 dB.
    Upmix,
}

/// A speaker position, in the WAVEFORMATEXTENSIBLE order devices use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Speaker {
    Fl,
    Fr,
    Fc,
    Lfe,
    Bl,
    Br,
    Sl,
    Sr,
    Bc,
}

/// The usual layout for `n` interleaved channels (FLAC, WAV and Windows
/// agree: 5.1 = FL FR FC LFE BL BR, 7.1 = FL FR FC LFE BL BR SL SR).
fn layout(n: usize) -> &'static [Speaker] {
    use Speaker::*;
    match n {
        1 => &[Fc],
        2 => &[Fl, Fr],
        3 => &[Fl, Fr, Fc],
        4 => &[Fl, Fr, Bl, Br],
        5 => &[Fl, Fr, Fc, Bl, Br],
        6 => &[Fl, Fr, Fc, Lfe, Bl, Br],
        7 => &[Fl, Fr, Fc, Lfe, Bc, Sl, Sr],
        _ => &[Fl, Fr, Fc, Lfe, Bl, Br, Sl, Sr],
    }
}

const MINUS_3DB: f32 = std::f32::consts::FRAC_1_SQRT_2;
const MINUS_6DB: f32 = 0.5;
/// Where the upmix's subwoofer channel stops.
const LFE_HZ: f32 = 120.0;
/// Most source / device channels handled (more are dropped).
const MAX_CH: usize = 8;

/// Puts a track's channels on the device's speakers: unchanged when they
/// match (bit-perfect), else by speaker position with the [`SurroundMode`]
/// for stereo on surround and a standard fold-down the other way.
pub(crate) struct ChannelMapper {
    src: usize,
    dst: usize,
    /// `gain[out][in]`.
    gain: [[f32; MAX_CH]; MAX_CH],
    /// The subwoofer channel of an upmix and its two one-pole low-pass states.
    lfe: Option<usize>,
    lfe_coef: f32,
    lfe_state: [f32; 2],
}

impl ChannelMapper {
    pub fn new(src: u16, dst: u16, mode: SurroundMode, rate: u32) -> Self {
        let (src, dst) = ((src as usize).clamp(1, MAX_CH), (dst as usize).clamp(1, MAX_CH));
        let mut gain = [[0f32; MAX_CH]; MAX_CH];
        let mut lfe = None;
        let (from, to) = (layout(src), layout(dst));
        let find = |s: Speaker| to.iter().position(|&t| t == s);
        if src == dst {
            for (i, row) in gain.iter_mut().enumerate().take(dst) {
                row[i] = 1.0;
            }
        } else if src <= 2 && dst >= 2 {
            // mono plays as stereo with L = R
            let (l, r) = if src == 1 { (0, 0) } else { (0, 1) };
            // L = R for mono, so (L + R) / 2 is the mono itself
            let mono = 0.5;
            for (o, sp) in to.iter().enumerate() {
                let (gl, gr) = match (sp, mode) {
                    (Speaker::Fl, _) => (1.0, 0.0),
                    (Speaker::Fr, _) => (0.0, 1.0),
                    (Speaker::Fc, SurroundMode::Upmix) => (mono * MINUS_3DB, mono * MINUS_3DB),
                    (Speaker::Lfe, SurroundMode::Upmix) => {
                        lfe = Some(o);
                        (mono, mono)
                    }
                    (Speaker::Bl | Speaker::Sl, SurroundMode::AllSpeakers) => (MINUS_3DB, 0.0),
                    (Speaker::Br | Speaker::Sr, SurroundMode::AllSpeakers) => (0.0, MINUS_3DB),
                    (Speaker::Bc, SurroundMode::AllSpeakers) => (mono * MINUS_3DB, mono * MINUS_3DB),
                    (Speaker::Bl | Speaker::Sl, SurroundMode::Upmix) => (MINUS_6DB, 0.0),
                    (Speaker::Br | Speaker::Sr, SurroundMode::Upmix) => (0.0, MINUS_6DB),
                    (Speaker::Bc, SurroundMode::Upmix) => (mono * MINUS_6DB, mono * MINUS_6DB),
                    _ => (0.0, 0.0),
                };
                if src == 1 {
                    gain[o][0] = gl + gr;
                } else {
                    gain[o][l] += gl;
                    gain[o][r] += gr;
                }
            }
        } else if dst == 1 {
            for i in 0..src {
                gain[0][i] = if from[i] == Speaker::Lfe { 0.0 } else { 1.0 / src as f32 };
            }
        } else {
            // surround on another layout: each speaker where it is, else folded into its neighbours
            for (i, &sp) in from.iter().enumerate() {
                let fold: &[(Speaker, f32)] = match sp {
                    Speaker::Fc => &[(Speaker::Fl, MINUS_3DB), (Speaker::Fr, MINUS_3DB)],
                    Speaker::Bl => &[(Speaker::Sl, 1.0)],
                    Speaker::Br => &[(Speaker::Sr, 1.0)],
                    Speaker::Sl => &[(Speaker::Bl, 1.0)],
                    Speaker::Sr => &[(Speaker::Br, 1.0)],
                    Speaker::Bc => &[(Speaker::Bl, MINUS_3DB), (Speaker::Br, MINUS_3DB)],
                    _ => &[],
                };
                if let Some(o) = find(sp) {
                    gain[o][i] += 1.0;
                } else if fold.iter().any(|(t, _)| find(*t).is_some()) {
                    for (t, g) in fold {
                        if let Some(o) = find(*t) {
                            gain[o][i] += g;
                        }
                    }
                } else if matches!(sp, Speaker::Bl | Speaker::Sl) {
                    gain[0][i] += MINUS_3DB; // no rear speakers: into the front
                } else if matches!(sp, Speaker::Br | Speaker::Sr) {
                    gain[1][i] += MINUS_3DB;
                }
                // a subwoofer channel without a subwoofer is dropped (it doubles the bass)
            }
            // folding down adds channels up: keep every speaker at or below full scale
            let worst = gain.iter().take(dst).map(|row| row.iter().sum::<f32>()).fold(1.0f32, f32::max);
            if worst > 1.0 {
                gain.iter_mut().flatten().for_each(|g| *g /= worst);
            }
        }
        let lfe_coef = 1.0 - (-2.0 * std::f32::consts::PI * LFE_HZ / rate.max(1) as f32).exp();
        Self { src, dst, gain, lfe, lfe_coef, lfe_state: [0.0; 2] }
    }

    /// The samples pass through as they are.
    pub fn is_identity(&self) -> bool {
        self.src == self.dst
    }

    pub fn source_channels(&self) -> usize {
        self.src
    }

    /// Appends `samples` (`src` channels) mapped to the device's channels.
    pub fn map(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        if self.is_identity() {
            out.extend_from_slice(samples);
            return;
        }
        out.reserve(samples.len() / self.src * self.dst);
        for frame in samples.chunks_exact(self.src) {
            for o in 0..self.dst {
                let row = &self.gain[o];
                let mut v: f32 = frame.iter().zip(row).map(|(x, g)| x * g).sum();
                if self.lfe == Some(o) {
                    // two one-pole low-passes: 12 dB / octave above LFE_HZ
                    self.lfe_state[0] += self.lfe_coef * (v - self.lfe_state[0]);
                    self.lfe_state[1] += self.lfe_coef * (self.lfe_state[0] - self.lfe_state[1]);
                    v = self.lfe_state[1];
                }
                out.push(v);
            }
        }
    }
}

/// `true` when some sample of the frame is heard (above -60 dBFS).
fn audible(frame: &[f32]) -> bool {
    frame.iter().any(|s| s.abs() > SILENCE_THRESHOLD)
}

/// Length (in samples, whole frames) of `samples` up to the end of the music:
/// trailing silence below -60 dBFS cut off. 0 when it is all silence.
pub(crate) fn music_end(samples: &[f32], channels: usize) -> usize {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    (0..frames).rev().find(|&f| audible(&samples[f * channels..(f + 1) * channels])).map_or(0, |f| (f + 1) * channels)
}

/// How a deck turns a track into what the device plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DeckFormat {
    pub rate: u32,
    pub channels: u16,
    pub surround: SurroundMode,
    pub quality: ResamplerQuality,
}

/// One track decoding at the output's rate and channel count. The resampler
/// runs at the track's own channel count (2 for stereo, not the 8 of a 7.1
/// device), then the channels are mapped to the speakers.
pub(crate) struct Deck {
    source: Source,
    format: DeckFormat,
    resampler: Option<Resampler>,
    mapper: ChannelMapper,
    raw: Vec<f32>,
    resampled: Vec<f32>,
    ended: bool,
    /// Frames of leading silence still allowed to be dropped (smart crossfade), 0 = none.
    lead_budget: u64,
    /// Leading silence dropped since the last [`Deck::take_skipped`] (frames).
    skipped: u64,
}

impl Deck {
    /// `lead_silence_frames`: how much leading silence may be dropped (0 for
    /// gapless and bit-perfect starts).
    pub fn new(source: Source, format: DeckFormat, lead_silence_frames: u64) -> Result<Self> {
        let format = DeckFormat { channels: format.channels.max(1), ..format };
        let src = source.channels.max(1);
        let resampler = Self::resampler_for(&source, src, &format)?;
        Ok(Self {
            mapper: ChannelMapper::new(src, format.channels, format.surround, format.rate),
            source,
            format,
            resampler,
            raw: Vec::with_capacity(16 * 1024),
            resampled: Vec::with_capacity(16 * 1024),
            ended: false,
            lead_budget: lead_silence_frames,
            skipped: 0,
        })
    }

    fn resampler_for(source: &Source, channels: u16, format: &DeckFormat) -> Result<Option<Resampler>> {
        if source.sample_rate == format.rate {
            return Ok(None);
        }
        Resampler::new(source.sample_rate, format.rate, channels as usize, format.quality).map(Some)
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The track plays at another rate than its own.
    pub fn resampled(&self) -> bool {
        self.resampler.is_some()
    }

    /// The track's channels are spread over other speakers (not 1:1).
    pub fn remapped(&self) -> bool {
        !self.mapper.is_identity()
    }

    /// Decodes the next chunk and appends it (output rate and channels) to
    /// `out`. `Ok(false)` once the track is over (the resampler's tail is
    /// appended by that call).
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        if self.ended {
            return Ok(false);
        }
        let start = out.len();
        self.raw.clear();
        let more = match self.source.next_chunk() {
            Ok(Some(samples)) => {
                self.raw.extend_from_slice(samples);
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.ended = true;
                return Err(e);
            }
        };
        // the real channel count is only known after the first packet of some formats
        let src = self.source.channels.max(1);
        if src as usize != self.mapper.source_channels() {
            self.mapper = ChannelMapper::new(src, self.format.channels, self.format.surround, self.format.rate);
            self.resampler = Self::resampler_for(&self.source, src, &self.format)?;
        }
        let result = match &mut self.resampler {
            Some(r) => {
                self.resampled.clear();
                let done = r.process(&self.raw, &mut self.resampled).and_then(|()| {
                    if more {
                        Ok(())
                    } else {
                        r.flush(&mut self.resampled)
                    }
                });
                self.mapper.map(&self.resampled, out);
                done
            }
            None => {
                self.mapper.map(&self.raw, out);
                Ok(())
            }
        };
        if !more || result.is_err() {
            self.ended = true;
        }
        result?;
        if self.lead_budget > 0 {
            self.skip_lead(out, start);
        }
        Ok(more)
    }

    /// Drops the leading silence of what was just appended at `out[start..]`.
    fn skip_lead(&mut self, out: &mut Vec<f32>, start: usize) {
        let ch = self.format.channels as usize;
        let fresh = &out[start..];
        let frames = fresh.len() / ch;
        let first = (0..frames).find(|&f| audible(&fresh[f * ch..(f + 1) * ch]));
        let drop = first.unwrap_or(frames).min(self.lead_budget as usize);
        out.drain(start..start + drop * ch);
        self.skipped += drop as u64;
        self.lead_budget = if first.is_some() { 0 } else { self.lead_budget - drop as u64 };
    }

    /// Leading silence dropped since the last call (frames at the output rate).
    pub fn take_skipped(&mut self) -> u64 {
        std::mem::take(&mut self.skipped)
    }
}

/// The overlap of two tracks: the outgoing one's last samples (already
/// decoded) summed with the incoming deck as it decodes.
///
/// Each voice keeps its own track's level: the output equalizer (in the
/// realtime callback, after the mix) runs the outgoing track's filters until
/// the middle of the fade, then glides to the incoming one's. The preamp of
/// those filters carries the taste and loudness gains, so here each voice is
/// scaled by its own preamp over the one the callback runs at that frame:
/// both songs reach the device at their own loudness, and the filter shape
/// changes once, where both are at half power.
pub(crate) struct Fade {
    tail: Vec<f32>,
    /// Next sample of `tail` to mix.
    pos: usize,
    channels: usize,
    /// Frames the fade lasts / were mixed.
    pub len: usize,
    done: usize,
    curve: FadeCurve,
    /// First output frame of the fade (counted like `frames_played`).
    pub start_frame: u64,
    /// Fade frame at which the callback starts gliding to the incoming filters.
    pub switch_at: usize,
    glide: usize,
    /// Linear preamp (equalizer + taste + loudness) of each track.
    pub gain_from: f32,
    pub gain_to: f32,
    /// The outgoing track's filters (for [`crate::engine`]'s replan).
    pub from_design: homefi_core::dsp::EqDesign,
    pub from: std::path::PathBuf,
    pub to: std::path::PathBuf,
    /// A skip or a pick by the user (the short fade).
    pub manual: bool,
}

/// What a [`Fade`] needs besides the outgoing samples.
pub(crate) struct FadeSpec {
    pub curve: FadeCurve,
    pub start_frame: u64,
    pub glide: usize,
    pub from_design: homefi_core::dsp::EqDesign,
    pub gain_to: f32,
    pub from: std::path::PathBuf,
    pub to: std::path::PathBuf,
    pub manual: bool,
}

impl Fade {
    /// `tail`: the outgoing samples to fade out, interleaved (their length is the fade's).
    pub fn new(tail: Vec<f32>, channels: usize, spec: FadeSpec) -> Self {
        let channels = channels.max(1);
        let len = tail.len() / channels;
        Self {
            tail,
            pos: 0,
            channels,
            len,
            done: 0,
            curve: spec.curve,
            start_frame: spec.start_frame,
            switch_at: len / 2,
            glide: spec.glide.max(1),
            gain_from: spec.from_design.gain() as f32,
            gain_to: spec.gain_to,
            from_design: spec.from_design,
            from: spec.from,
            to: spec.to,
            manual: spec.manual,
        }
    }

    /// Output frame where the callback switches to the incoming filters.
    pub fn switch_frame(&self) -> u64 {
        self.start_frame + self.switch_at as u64
    }

    /// Still summing (the outgoing samples aren't all used).
    pub fn mixing(&self) -> bool {
        self.done < self.len
    }

    /// How far the listener is (0..=1) when `played` frames reached the device.
    pub fn progress(&self, played: u64) -> f32 {
        (played.saturating_sub(self.start_frame) as f32 / self.len.max(1) as f32).clamp(0.0, 1.0)
    }

    /// `true` once the listener heard all of it.
    pub fn heard(&self, played: u64) -> bool {
        !self.mixing() && played >= self.start_frame + self.len as u64
    }

    /// Preamp the callback runs at fade frame `k` (it glides linearly, like the equalizer).
    fn callback_gain(&self, k: usize) -> f32 {
        if k < self.switch_at {
            return self.gain_from;
        }
        let t = ((k - self.switch_at + 1) as f32 / self.glide as f32).min(1.0);
        self.gain_from + (self.gain_to - self.gain_from) * t
    }

    /// Sums the next frames of the incoming track (`incoming`, in place) with
    /// the outgoing one; frames past the end of the fade are left as they are.
    pub fn mix(&mut self, incoming: &mut [f32]) {
        let ch = self.channels;
        for frame in incoming.chunks_exact_mut(ch) {
            if !self.mixing() {
                break;
            }
            let (out_g, in_g) = self.frame_gains();
            for s in frame.iter_mut() {
                let x = *s * in_g + self.tail[self.pos] * out_g;
                *s = soft_limit(x);
                self.pos += 1;
            }
            self.done += 1;
        }
    }

    /// The incoming track ended before the fade did: the rest of the outgoing
    /// one fades alone (appended to `out`).
    pub fn finish_alone(&mut self, out: &mut Vec<f32>) {
        while self.mixing() {
            let (out_g, _) = self.frame_gains();
            for _ in 0..self.channels {
                out.push(soft_limit(self.tail[self.pos] * out_g));
                self.pos += 1;
            }
            self.done += 1;
        }
    }

    /// `(outgoing, incoming)` gains of the current frame, preamp compensated.
    fn frame_gains(&self) -> (f32, f32) {
        let k = self.done;
        let (go, gi) = gains(self.curve, (k as f32 + 0.5) / self.len as f32);
        let now = self.callback_gain(k);
        if now <= 0.0 || !now.is_finite() {
            return (go, gi);
        }
        (go * self.gain_from / now, gi * self.gain_to / now)
    }
}

/// Lets two loud songs overlap without clipping: untouched up to the knee
/// (-0.9 dBFS), then a smooth curve that never passes full scale.
pub(crate) fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= LIMIT_KNEE {
        return x;
    }
    let room = 1.0 - LIMIT_KNEE;
    (LIMIT_KNEE + room * ((a - LIMIT_KNEE) / room).tanh()).copysign(x)
}

#[cfg(test)]
mod tests {
    use homefi_core::dsp::EqDesign;

    use super::*;

    fn spec(curve: FadeCurve) -> FadeSpec {
        FadeSpec {
            curve,
            start_frame: 1000,
            glide: 960,
            from_design: EqDesign::BYPASS,
            gain_to: 1.0,
            from: "a".into(),
            to: "b".into(),
            manual: false,
        }
    }

    fn sine(freq: f32, rate: f32, frames: usize, amp: f32, phase0: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = amp * (2.0 * std::f32::consts::PI * freq * (i + phase0) as f32 / rate).sin();
                [v, v]
            })
            .collect()
    }

    fn mapped(src: u16, dst: u16, mode: SurroundMode, frame: &[f32]) -> Vec<f32> {
        let mut m = ChannelMapper::new(src, dst, mode, 48_000);
        let mut out = Vec::new();
        m.map(frame, &mut out);
        out
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn channel_mapping_basics() {
        // same count: untouched (bit-perfect)
        assert!(ChannelMapper::new(2, 2, SurroundMode::Upmix, 48_000).is_identity());
        assert_eq!(mapped(2, 2, SurroundMode::Upmix, &[0.1, 0.2]), [0.1, 0.2]);
        // mono → both sides
        assert_eq!(mapped(1, 2, SurroundMode::FrontOnly, &[0.1, 0.2]), [0.1, 0.1, 0.2, 0.2]);
        // stereo → mono: the average
        assert!(close(&mapped(2, 1, SurroundMode::FrontOnly, &[0.2, 0.4]), &[0.3]));
    }

    #[test]
    fn stereo_on_7_1_for_each_surround_mode() {
        let (l, r) = (0.4f32, 0.2f32);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        // FL FR FC LFE BL BR SL SR
        assert_eq!(mapped(2, 8, SurroundMode::FrontOnly, &[l, r]), [l, r, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let all = mapped(2, 8, SurroundMode::AllSpeakers, &[l, r]);
        assert!(close(&all, &[l, r, 0.0, 0.0, l * h, r * h, l * h, r * h]), "{all:?}");
        let up = mapped(2, 8, SurroundMode::Upmix, &[l, r]);
        assert!(close(&up[..3], &[l, r, 0.5 * (l + r) * h]), "{up:?}");
        assert!(close(&up[4..], &[l * 0.5, r * 0.5, l * 0.5, r * 0.5]), "{up:?}");
        // the subwoofer only gets the lows: a steady level gets through, a fast one doesn't
        let mut m = ChannelMapper::new(2, 8, SurroundMode::Upmix, 48_000);
        let mut out = Vec::new();
        m.map(&[0.5f32, 0.5].repeat(48_000), &mut out);
        assert!((out[out.len() - 8 + 3] - 0.5).abs() < 1e-3, "DC reaches the LFE");
        let mut m = ChannelMapper::new(2, 8, SurroundMode::Upmix, 48_000);
        let mut out = Vec::new();
        let fast: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let v = 0.5 * (2.0 * std::f32::consts::PI * 5000.0 * i as f32 / 48_000.0).sin();
                [v, v]
            })
            .collect();
        m.map(&fast, &mut out);
        let lfe_peak = out.chunks_exact(8).skip(1000).map(|f| f[3].abs()).fold(0f32, f32::max);
        assert!(lfe_peak < 0.01, "5 kHz stays out of the subwoofer: {lfe_peak}");
        // mono upmix: the center carries it at -3 dB
        let mono = mapped(1, 8, SurroundMode::Upmix, &[0.4]);
        assert!(close(&mono[..3], &[0.4, 0.4, 0.4 * h]), "{mono:?}");
    }

    #[test]
    fn surround_sources_land_on_their_speakers() {
        // 5.1 (FL FR FC LFE BL BR) on 7.1: 1:1, the sides silent
        let six = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        assert_eq!(mapped(6, 8, SurroundMode::Upmix, &six), [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.0, 0.0]);
        // 7.1 on 5.1: the sides join the rears, levels kept under full scale
        let eight = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let five = mapped(8, 6, SurroundMode::FrontOnly, &eight);
        assert_eq!(five.len(), 6);
        assert!((five[4] / five[0] - (0.5 + 0.7) / 0.1).abs() < 1e-3, "{five:?}");
        // 5.1 on stereo: a fold-down with the center at -3 dB, no subwoofer, never clipping
        let full = mapped(6, 2, SurroundMode::FrontOnly, &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        assert!(full.iter().all(|s| *s <= 1.0 + 1e-6 && *s > 0.9), "{full:?}");
        let center = mapped(6, 2, SurroundMode::FrontOnly, &[0.0, 0.0, 0.5, 0.9, 0.0, 0.0]);
        assert!((center[0] - center[1]).abs() < 1e-6 && center[0] > 0.0, "{center:?}");
    }

    #[test]
    fn resampling_before_or_after_the_mapping_sounds_the_same() {
        // the deck resamples at the track's 2 channels and then maps to 8;
        // mapping first (8 channels through the resampler) gives the same samples
        let input: Vec<f32> = (0..44_100)
            .flat_map(|i| {
                let t = i as f32 / 44_100.0;
                [
                    0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin(),
                    0.3 * (2.0 * std::f32::consts::PI * 660.0 * t).sin(),
                ]
            })
            .collect();
        for mode in [SurroundMode::FrontOnly, SurroundMode::AllSpeakers] {
            let mut r2 = Resampler::new(44_100, 48_000, 2, ResamplerQuality::High).unwrap();
            let mut two = Vec::new();
            r2.process(&input, &mut two).unwrap();
            let mut a = Vec::new();
            ChannelMapper::new(2, 8, mode, 48_000).map(&two, &mut a);

            let mut wide = Vec::new();
            ChannelMapper::new(2, 8, mode, 48_000).map(&input, &mut wide);
            let mut r8 = Resampler::new(44_100, 48_000, 8, ResamplerQuality::High).unwrap();
            let mut b = Vec::new();
            r8.process(&wide, &mut b).unwrap();
            assert_eq!(a.len(), b.len());
            let worst = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0f32, f32::max);
            assert!(worst < 1e-4, "{mode:?}: {worst}");
        }
    }

    /// Resampling 2 channels then mapping to 8 must cost about what stereo
    /// costs, far less than resampling all 8 (what broke the sound on a 7.1
    /// Windows device). Times are compared, not absolute, so it holds on slow CI.
    #[test]
    fn surround_devices_cost_about_the_same_cpu_as_stereo() {
        let input = vec![0.25f32; 2 * 44_100 * 3];
        let time = |f: &mut dyn FnMut()| {
            let t = std::time::Instant::now();
            f();
            t.elapsed()
        };
        let mut sink = Vec::with_capacity(8 * 48_000 * 4);
        let stereo_then_map = time(&mut || {
            let mut r = Resampler::new(44_100, 48_000, 2, ResamplerQuality::High).unwrap();
            let mut two = Vec::new();
            r.process(&input, &mut two).unwrap();
            sink.clear();
            ChannelMapper::new(2, 8, SurroundMode::AllSpeakers, 48_000).map(&two, &mut sink);
        });
        let mut wide = Vec::new();
        ChannelMapper::new(2, 8, SurroundMode::AllSpeakers, 48_000).map(&input, &mut wide);
        let all_eight = time(&mut || {
            let mut r = Resampler::new(44_100, 48_000, 8, ResamplerQuality::High).unwrap();
            sink.clear();
            r.process(&wide, &mut sink).unwrap();
        });
        assert!(stereo_then_map * 2 < all_eight, "{stereo_then_map:?} vs {all_eight:?}");
        // the fast resampler is lighter still
        let fast = time(&mut || {
            let mut r = Resampler::new(44_100, 48_000, 2, ResamplerQuality::Fast).unwrap();
            sink.clear();
            r.process(&input, &mut sink).unwrap();
        });
        let high = time(&mut || {
            let mut r = Resampler::new(44_100, 48_000, 2, ResamplerQuality::High).unwrap();
            sink.clear();
            r.process(&input, &mut sink).unwrap();
        });
        assert!(fast < high, "{fast:?} vs {high:?}");
    }

    #[test]
    fn resampler_changes_length_by_ratio() {
        let mut r = Resampler::new(44_100, 48_000, 2, ResamplerQuality::High).unwrap();
        let input = vec![0.25f32; 44_100 * 2];
        let mut out = Vec::new();
        r.process(&input, &mut out).unwrap();
        r.flush(&mut out).unwrap();
        let frames = out.len() / 2;
        assert!((47_000..=49_200).contains(&frames), "{frames}");
    }

    #[test]
    fn music_end_skips_trailing_silence() {
        let mut s = vec![0.5f32; 2 * 100];
        s.extend(vec![0.0005f32; 2 * 50]); // below -60 dBFS
        assert_eq!(music_end(&s, 2), 200);
        assert_eq!(music_end(&[0.0; 8], 2), 0);
        // one loud channel is enough
        assert_eq!(music_end(&[0.0, 0.0, 0.0, 0.2, 0.0, 0.0], 2), 4);
    }

    #[test]
    fn soft_limit_never_clips_and_leaves_normal_levels() {
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.9), -0.9);
        for x in [0.95f32, 1.2, 1.414, 2.0, 10.0] {
            let y = soft_limit(x);
            assert!(y <= 1.0 && y > 0.9, "{x} → {y}");
            assert_eq!(soft_limit(-x), -y);
        }
        // monotonic around the knee
        assert!(soft_limit(0.91) > soft_limit(0.9) && soft_limit(1.5) > soft_limit(1.2));
    }

    /// An offline render of a whole crossfade: two full-scale sines (the worst
    /// case: peaks add up), equal power, the incoming filters 6 dB quieter.
    #[test]
    fn offline_crossfade_has_no_clipping_or_clicks() {
        let rate = 48_000.0;
        let fade = 48_000; // 1 s
        let tail = sine(440.0, rate, fade, 0.99, 0);
        for curve in [FadeCurve::EqualPower, FadeCurve::Linear, FadeCurve::SCurve] {
            let mut f = Fade::new(tail.clone(), 2, FadeSpec { gain_to: 0.5, ..spec(curve) });
            // before the fade: the outgoing track at full level, continuous with the tail
            let mut render: Vec<f32> = (0..4800)
                .flat_map(|i| {
                    let v = 0.99 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 - 4800.0) / rate).sin();
                    [v, v]
                })
                .collect();
            // the incoming track arrives in uneven chunks, as decoders hand it over
            let incoming = sine(1000.0, rate, fade + 9600, 0.99, 0);
            for chunk in incoming.chunks(2 * 1153) {
                let mut c = chunk.to_vec();
                f.mix(&mut c);
                render.extend(c);
            }
            assert!(!f.mixing());
            assert!(render.iter().all(|s| s.abs() <= 1.0), "{curve:?}: no sample past full scale");
            let worst = render.windows(2).step_by(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
            // a 1 kHz full-scale sine moves at most 2π·1000/48000 ≈ 0.13 per sample
            assert!(worst < 0.14, "{curve:?}: no discontinuity ({worst})");
            // after the fade: the incoming track exactly as decoded
            let end = render.len() - 2 * 9600;
            assert_eq!(render[end..], incoming[2 * fade..], "{curve:?}");
        }
    }

    #[test]
    fn each_voice_keeps_its_own_loudness_across_the_filter_switch() {
        // outgoing preamp 1.0, incoming 0.5 (loudness -6 dB): the callback runs 1.0
        // until the switch, then glides to 0.5. What reaches the device for each
        // voice must stay at its curve gain × its own preamp all the way.
        let len = 10_000;
        let mut f = Fade::new(vec![0.0; 2 * len], 2, FadeSpec { glide: 480, gain_to: 0.5, ..spec(FadeCurve::Linear) });
        for k in 0..len {
            f.done = k;
            let (o, i) = f.frame_gains();
            let heard = f.callback_gain(k);
            let (go, gi) = gains(FadeCurve::Linear, (k as f32 + 0.5) / len as f32);
            assert!((o * heard - go).abs() < 1e-5, "outgoing at {k}");
            assert!((i * heard - gi * 0.5).abs() < 1e-5, "incoming at {k}");
        }
        assert_eq!(f.switch_frame(), 1000 + 5000);
    }

    #[test]
    fn a_short_incoming_track_lets_the_outgoing_one_finish_alone() {
        let mut f = Fade::new(vec![0.5; 2 * 100], 2, spec(FadeCurve::Linear));
        let mut part = vec![0.0; 2 * 30];
        f.mix(&mut part);
        let mut rest = Vec::new();
        f.finish_alone(&mut rest);
        assert_eq!(rest.len(), 2 * 70);
        assert!(!f.mixing());
        assert!(rest.last().unwrap().abs() < 0.01, "faded out");
        assert!(f.progress(1050) > 0.49 && f.progress(1050) < 0.51);
        assert!(f.heard(1100) && !f.heard(1099));
    }

    /// A 16-bit PCM WAV with a sine (`silence_ms` of silence first).
    fn wav(path: &std::path::Path, rate: u32, seconds: f32, silence_ms: u32) {
        let frames = (rate as f32 * seconds) as usize;
        let lead = (rate * silence_ms / 1000) as usize;
        let mut data = Vec::with_capacity(frames * 4);
        for i in 0..frames {
            let v = if i < lead { 0 } else { ((i as f32 * 0.05).sin() * 16_000.0) as i16 };
            data.extend_from_slice(&v.to_le_bytes());
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut f = Vec::new();
        f.extend_from_slice(b"RIFF");
        f.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        f.extend_from_slice(b"WAVEfmt ");
        f.extend_from_slice(&16u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&rate.to_le_bytes());
        f.extend_from_slice(&(rate * 4).to_le_bytes());
        f.extend_from_slice(&4u16.to_le_bytes());
        f.extend_from_slice(&16u16.to_le_bytes());
        f.extend_from_slice(b"data");
        f.extend_from_slice(&(data.len() as u32).to_le_bytes());
        f.extend_from_slice(&data);
        std::fs::write(path, f).unwrap();
    }

    fn fmt(rate: u32) -> DeckFormat {
        DeckFormat { rate, channels: 2, ..Default::default() }
    }

    fn read_all(deck: &mut Deck) -> Vec<f32> {
        let mut out = Vec::new();
        while deck.read(&mut out).unwrap() {}
        out
    }

    #[test]
    fn an_incoming_track_at_another_rate_is_resampled_to_the_device_rate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        wav(&path, 44_100, 1.0, 0);
        let mut deck = Deck::new(Source::open(&path).unwrap(), fmt(48_000), 0).unwrap();
        assert!(deck.resampled());
        let frames = read_all(&mut deck).len() / 2;
        assert!((47_000..=49_300).contains(&frames), "{frames}");
        // same rate: untouched
        let mut same = Deck::new(Source::open(&path).unwrap(), fmt(44_100), 0).unwrap();
        assert!(!same.resampled());
        assert_eq!(read_all(&mut same).len() / 2, 44_100);
    }

    #[test]
    fn smart_start_drops_leading_silence_up_to_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quiet-start.wav");
        wav(&path, 48_000, 1.0, 250);
        let mut deck = Deck::new(Source::open(&path).unwrap(), fmt(48_000), 48_000).unwrap();
        let out = read_all(&mut deck);
        let skipped = deck.take_skipped();
        // 250 ms of digital silence, and the sine's first sample (sin 0 = 0) too
        assert!((12_000..=12_002).contains(&skipped), "{skipped}");
        assert_eq!(out.len() / 2, 48_000 - skipped as usize);
        assert!(out[0].abs() > SILENCE_THRESHOLD);
        // a small budget stops early: the rest of the silence plays
        let mut deck = Deck::new(Source::open(&path).unwrap(), fmt(48_000), 1000).unwrap();
        let out = read_all(&mut deck);
        assert_eq!(deck.take_skipped(), 1000);
        assert_eq!(out.len() / 2, 47_000);
    }
}
