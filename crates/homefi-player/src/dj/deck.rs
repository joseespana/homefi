//! One DJ deck: a whole song in RAM played back at any speed (4-point Hermite interpolation),
//! with cue points, loops, trim, a 3-band EQ with kill, a filter and a volume. Everything here
//! runs in the audio callback: no allocation, no locks, no I/O.

use std::sync::Arc;

use homefi_core::dj::mixer_math::{
    channel_eq_stages, db_to_gain, filter_coeffs, filter_wet, BiquadState, EqGains, FilterCoeffs, FilterState,
    EQ_KILL_DB, EQ_STAGES, MAX_TRIM_DB, MIN_TRIM_DB,
};
use homefi_core::dj::{Grid, MixerConfig, HOT_CUES};
use homefi_core::dsp::Biquad;

/// Frames between control updates (smoothing, EQ coefficients).
pub(crate) const CONTROL_BLOCK: usize = 32;
/// Time constant of the knob smoothing (seconds).
const SMOOTH_SECONDS: f64 = 0.010;
/// Shortest loop (frames).
const MIN_LOOP_FRAMES: f64 = 64.0;
/// A band within this many dB of flat is skipped.
const FLAT_DB: f32 = 0.05;
/// Values closer than this to their target snap to it.
const SETTLE: f32 = 1e-4;
/// Pitch bend of a full push, as a fraction of `bend_pct`.
const BEND_FULL: f64 = 1.0;
/// Within this many frames of the cue point counts as "on it".
const ON_CUE_FRAMES: f64 = 64.0;

/// A decoded song: interleaved stereo `f32` at the file's own rate.
pub struct DeckAudio {
    samples: Vec<f32>,
    rate: u32,
    grid: Option<Grid>,
    auto_gain_db: f32,
}

impl DeckAudio {
    /// `samples` is interleaved stereo (an odd trailing sample is dropped).
    pub fn new(mut samples: Vec<f32>, rate: u32, grid: Option<Grid>, auto_gain_db: f32) -> Self {
        samples.truncate(samples.len() / 2 * 2);
        Self { samples, rate: rate.max(1), grid, auto_gain_db }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn grid(&self) -> Option<Grid> {
        self.grid
    }

    pub fn auto_gain_db(&self) -> f32 {
        self.auto_gain_db
    }

    pub fn duration_ms(&self) -> f64 {
        self.frames() as f64 / self.rate as f64 * 1000.0
    }

    /// RAM held by the samples.
    pub fn bytes(&self) -> u64 {
        (self.samples.len() * std::mem::size_of::<f32>()) as u64
    }

    #[inline]
    fn frame(&self, i: i64) -> [f32; 2] {
        if i < 0 {
            return [0.0; 2];
        }
        match self.samples.get(i as usize * 2..i as usize * 2 + 2) {
            Some(s) => [s[0], s[1]],
            None => [0.0; 2],
        }
    }
}

/// 4-point, 3rd-order Hermite between `y1` and `y2` at `t` (0..1).
#[inline]
fn hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + y1
}

/// The song at a fractional frame. On a whole frame this is the sample itself (exact).
#[inline]
fn sample_at(audio: &DeckAudio, pos: f64) -> [f32; 2] {
    let floor = pos.floor();
    let t = (pos - floor) as f32;
    let i = floor as i64;
    if t == 0.0 {
        return audio.frame(i);
    }
    let (a, b, c, d) = (audio.frame(i - 1), audio.frame(i), audio.frame(i + 1), audio.frame(i + 2));
    [hermite(a[0], b[0], c[0], d[0], t), hermite(a[1], b[1], c[1], d[1], t)]
}

/// A value that glides to its target (one pole), stepped once per control block.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Smooth {
    pub cur: f32,
    pub target: f32,
}

impl Smooth {
    pub fn new(v: f32) -> Self {
        Self { cur: v, target: v }
    }

    /// Moves toward the target; returns `(before, after)` for a per-frame ramp.
    pub fn step(&mut self, coef: f32) -> (f32, f32) {
        let before = self.cur;
        self.cur += (self.target - self.cur) * coef;
        if (self.target - self.cur).abs() < SETTLE {
            self.cur = self.target;
        }
        (before, self.cur)
    }

    #[cfg(test)]
    pub fn settled(&self) -> bool {
        self.cur == self.target
    }
}

pub(crate) fn smooth_coef(rate: u32) -> f32 {
    (1.0 - (-(CONTROL_BLOCK as f64) / (rate.max(1) as f64 * SMOOTH_SECONDS)).exp()) as f32
}

#[inline]
fn ramp(r: (f32, f32), k: f32) -> f32 {
    r.0 + (r.1 - r.0) * k
}

pub(crate) struct Deck {
    audio: Option<Arc<DeckAudio>>,
    out_rate: f64,
    coef: f32,
    /// Playhead in frames of the song.
    pub pos: f64,
    pub playing: bool,
    previewing: bool,
    pub cue: f64,
    pub hot: [Option<f64>; HOT_CUES],
    pub loop_: Option<(f64, f64)>,
    loop_in: Option<f64>,
    /// Tempo fader as a fraction (0.03 = 3 % faster).
    pub tempo: f64,
    pub bend: f32,
    pub synced: bool,
    pub user_trim_db: f32,
    pub eq_knob: [f32; 3],
    pub kill: [bool; 3],
    pub filter_knob: f32,
    pub volume_knob: f32,
    /// The automatic transition's hold on the low band (dB).
    pub low_override: Option<f32>,
    trim: Smooth,
    eq: [Smooth; 3],
    filter: Smooth,
    volume: Smooth,
    trim_ramp: (f32, f32),
    vol_ramp: (f32, f32),
    stages: [Biquad; EQ_STAGES],
    states: [[BiquadState; EQ_STAGES]; 2],
    eq_dirty: bool,
    filt: Option<FilterCoeffs>,
    filt_state: [FilterState; 2],
    filt_dirty: bool,
    wet_ramp: (f32, f32),
    /// The filter was just engaged: its states start from the signal (see `FilterState::prime`).
    filt_cold: bool,
    pub peak: [f32; 2],
}

impl Deck {
    pub fn new(out_rate: u32) -> Self {
        Self {
            audio: None,
            out_rate: out_rate.max(1) as f64,
            coef: smooth_coef(out_rate),
            pos: 0.0,
            playing: false,
            previewing: false,
            cue: 0.0,
            hot: [None; HOT_CUES],
            loop_: None,
            loop_in: None,
            tempo: 0.0,
            bend: 0.0,
            synced: false,
            user_trim_db: 0.0,
            eq_knob: [0.0; 3],
            kill: [false; 3],
            filter_knob: 0.0,
            volume_knob: 1.0,
            low_override: None,
            trim: Smooth::new(1.0),
            eq: [Smooth::new(0.0); 3],
            filter: Smooth::new(0.0),
            volume: Smooth::new(1.0),
            trim_ramp: (1.0, 1.0),
            vol_ramp: (1.0, 1.0),
            stages: [Biquad::IDENTITY; EQ_STAGES],
            states: [[BiquadState::default(); EQ_STAGES]; 2],
            eq_dirty: true,
            filt: None,
            filt_state: [FilterState::default(); 2],
            filt_dirty: true,
            wet_ramp: (0.0, 0.0),
            filt_cold: false,
            peak: [0.0; 2],
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.audio.is_some()
    }

    pub fn grid(&self) -> Option<Grid> {
        self.audio.as_ref().and_then(|a| a.grid())
    }

    pub fn src_rate(&self) -> f64 {
        self.audio.as_ref().map_or(self.out_rate, |a| a.rate() as f64)
    }

    pub fn frames(&self) -> f64 {
        self.audio.as_ref().map_or(0.0, |a| a.frames() as f64)
    }

    pub fn pos_ms(&self) -> f64 {
        self.pos / self.src_rate() * 1000.0
    }

    pub fn ms_to_frames(&self, ms: f64) -> f64 {
        ms / 1000.0 * self.src_rate()
    }

    /// Song frames advanced per output frame: the rate ratio, the tempo fader and the bend.
    pub fn step(&self, cfg: &MixerConfig) -> f64 {
        let bend = self.bend as f64 * BEND_FULL * cfg.bend_pct as f64 / 100.0;
        self.src_rate() / self.out_rate * (1.0 + self.tempo) * (1.0 + bend)
    }

    /// Tempo of the song as it plays (0 without a grid).
    pub fn current_bpm(&self) -> f64 {
        self.grid().map_or(0.0, |g| g.bpm() * (1.0 + self.tempo))
    }

    /// Puts `audio` on the deck and resets everything but the mixer knobs. The old song, if any, is
    /// returned so the caller can free it away from the audio thread.
    pub fn load(&mut self, audio: Arc<DeckAudio>, cfg: &MixerConfig) -> Option<Arc<DeckAudio>> {
        let first =
            audio.grid().map_or(0.0, |g| (g.first_beat_ms.rem_euclid(g.beat_ms)) / 1000.0 * audio.rate() as f64);
        let old = self.audio.replace(audio);
        self.pos = 0.0;
        self.playing = false;
        self.previewing = false;
        // a cue on the first beat: pressing Play after Cue starts on the beat
        self.cue = first;
        self.hot = [None; HOT_CUES];
        self.loop_ = None;
        self.loop_in = None;
        self.tempo = 0.0;
        self.bend = 0.0;
        self.synced = false;
        self.low_override = None;
        self.eq_dirty = true;
        self.filt_dirty = true;
        self.refresh_targets(cfg);
        // no glide from the previous song's trim
        self.trim.cur = self.trim.target;
        old
    }

    /// Unloads the deck; returns the song.
    pub fn eject(&mut self) -> Option<Arc<DeckAudio>> {
        let old = self.audio.take();
        self.pos = 0.0;
        self.playing = false;
        self.previewing = false;
        self.cue = 0.0;
        self.hot = [None; HOT_CUES];
        self.loop_ = None;
        self.loop_in = None;
        self.tempo = 0.0;
        self.synced = false;
        self.low_override = None;
        old
    }

    /// Snaps a position (frames) to the nearest beat when quantize is on and the song has a grid.
    pub fn snap(&self, pos: f64, cfg: &MixerConfig) -> f64 {
        match (cfg.quantize, self.grid()) {
            (true, Some(g)) => {
                let ms = g.nearest_beat_ms(pos / self.src_rate() * 1000.0);
                self.ms_to_frames(ms).clamp(0.0, self.frames())
            }
            _ => pos,
        }
    }

    pub fn seek_frames(&mut self, pos: f64) {
        self.pos = if pos.is_finite() { pos.clamp(0.0, self.frames()) } else { self.pos };
        // a jump out of the loop leaves it
        if let Some((a, b)) = self.loop_ {
            if self.pos < a || self.pos >= b {
                self.loop_ = None;
            }
        }
    }

    pub fn play(&mut self) {
        if self.audio.is_some() {
            if self.pos >= self.frames() {
                self.pos = 0.0;
            }
            self.playing = true;
            self.previewing = false;
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.previewing = false;
    }

    pub fn cue_button(&mut self, pressed: bool, cfg: &MixerConfig) {
        if self.audio.is_none() {
            return;
        }
        if pressed {
            if self.playing {
                self.playing = false;
                self.previewing = false;
                self.pos = self.cue;
            } else if (self.pos - self.cue).abs() <= ON_CUE_FRAMES {
                self.pos = self.cue;
                self.playing = true;
                self.previewing = true;
            } else {
                self.cue = self.snap(self.pos, cfg);
                self.pos = self.cue;
            }
        } else if self.previewing {
            self.playing = false;
            self.previewing = false;
            self.pos = self.cue;
        }
    }

    pub fn hot_cue(&mut self, index: usize, cfg: &MixerConfig) {
        match self.hot.get(index).copied() {
            Some(Some(at)) => self.seek_frames(at),
            Some(None) => self.set_hot_cue(index, cfg),
            None => {}
        }
    }

    pub fn set_hot_cue(&mut self, index: usize, cfg: &MixerConfig) {
        if self.audio.is_some() && index < HOT_CUES {
            self.hot[index] = Some(self.snap(self.pos, cfg));
        }
    }

    pub fn clear_hot_cue(&mut self, index: usize) {
        if let Some(slot) = self.hot.get_mut(index) {
            *slot = None;
        }
    }

    /// A loop of `beats` beats starting at the playhead (snapped to the beat).
    pub fn loop_beats(&mut self, beats: u32, cfg: &MixerConfig) {
        let Some(grid) = self.grid() else { return };
        if beats == 0 {
            return;
        }
        let start = self.snap(self.pos, cfg).floor();
        let len = (beats as f64 * self.ms_to_frames(grid.beat_ms)).round();
        self.set_loop(start, start + len);
    }

    pub fn loop_in(&mut self, cfg: &MixerConfig) {
        if self.audio.is_some() {
            self.loop_in = Some(self.snap(self.pos, cfg).floor());
        }
    }

    pub fn loop_out(&mut self, cfg: &MixerConfig) {
        if let Some(start) = self.loop_in {
            let end = self.snap(self.pos, cfg).floor();
            self.set_loop(start, end);
        }
    }

    pub fn loop_exit(&mut self) {
        self.loop_ = None;
        self.loop_in = None;
    }

    fn set_loop(&mut self, start: f64, end: f64) {
        let end = end.min(self.frames());
        if end - start >= MIN_LOOP_FRAMES {
            self.loop_ = Some((start, end));
            self.loop_in = Some(start);
            // a playhead already past the end would never get back in
            if self.pos >= end {
                self.pos = start + (self.pos - end) % (end - start);
            }
        }
    }

    pub fn set_tempo_pct(&mut self, percent: f32, cfg: &MixerConfig) {
        let range = cfg.tempo_range_pct;
        let p = if percent.is_nan() { 0.0 } else { percent.clamp(-range, range) };
        self.tempo = p as f64 / 100.0;
    }

    pub fn set_trim_db(&mut self, db: f32) {
        self.user_trim_db = if db.is_nan() { 0.0 } else { db.clamp(MIN_TRIM_DB, MAX_TRIM_DB) };
    }

    pub fn set_eq_db(&mut self, band: usize, db: f32, cfg: &MixerConfig) {
        if let Some(k) = self.eq_knob.get_mut(band) {
            *k = if db.is_nan() { 0.0 } else { db.clamp(EQ_KILL_DB, cfg.eq_max_boost_db) };
        }
    }

    pub fn set_filter(&mut self, v: f32) {
        self.filter_knob = if v.is_nan() { 0.0 } else { v.clamp(-1.0, 1.0) };
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume_knob = if v.is_nan() { 1.0 } else { v.clamp(0.0, 1.0) };
    }

    /// The EQ gains in force, kills and the transition's hold included.
    pub fn effective_eq_db(&self, cfg: &MixerConfig) -> [f32; 3] {
        std::array::from_fn(|b| {
            if self.kill[b] {
                EQ_KILL_DB
            } else if let (0, Some(hold)) = (b, self.low_override) {
                hold
            } else {
                self.eq_knob[b].clamp(EQ_KILL_DB, cfg.eq_max_boost_db)
            }
        })
    }

    /// Smoothed value of a band as the filters see it right now.
    #[cfg(test)]
    pub fn eq_now_db(&self, band: usize) -> f32 {
        self.eq[band].cur
    }

    fn refresh_targets(&mut self, cfg: &MixerConfig) {
        let auto = if cfg.auto_gain { self.audio.as_ref().map_or(0.0, |a| a.auto_gain_db()) } else { 0.0 };
        self.trim.target = db_to_gain(self.user_trim_db + auto);
        let eq = self.effective_eq_db(cfg);
        for (s, t) in self.eq.iter_mut().zip(eq) {
            s.target = t;
        }
        self.filter.target = self.filter_knob;
        self.volume.target = self.volume_knob;
    }

    /// One control step: knobs glide, EQ and filter coefficients follow. Call before `render`.
    pub fn tick(&mut self, cfg: &MixerConfig) {
        self.refresh_targets(cfg);
        let coef = self.coef;
        self.trim_ramp = self.trim.step(coef);
        self.vol_ramp = self.volume.step(coef);
        let mut moved = self.eq_dirty;
        for s in &mut self.eq {
            let (a, b) = s.step(coef);
            moved |= a != b;
        }
        if moved {
            self.eq_dirty = false;
            let gains = EqGains { low_db: self.eq[0].cur, mid_db: self.eq[1].cur, high_db: self.eq[2].cur };
            self.stages = channel_eq_stages(gains, self.out_rate as u32);
            for band in 0..3 {
                if self.eq[band].cur.abs() < FLAT_DB {
                    // a flat band is skipped: start clean when it comes back
                    for ch in 0..2 {
                        self.states[ch][band * 2].reset();
                        self.states[ch][band * 2 + 1].reset();
                    }
                }
            }
        }
        let (a, b) = self.filter.step(coef);
        self.wet_ramp = (filter_wet(a), filter_wet(b));
        if a != b || self.filt_dirty {
            self.filt_dirty = false;
            let was_off = self.filt.is_none();
            // while the filter fades out the block still runs it, with the corner it had
            let rate = self.out_rate as u32;
            self.filt = filter_coeffs(b, rate).or_else(|| filter_coeffs(a, rate).filter(|_| self.wet_ramp.0 > 0.0));
            match self.filt {
                None => self.filt_state.iter_mut().for_each(FilterState::reset),
                Some(_) => self.filt_cold |= was_off,
            }
        }
    }

    /// Renders `n` frames of stereo into `out` (overwritten), advancing the playhead.
    pub fn render(&mut self, cfg: &MixerConfig, n: usize, out: &mut [f32]) {
        let step = self.step(cfg);
        let inv = 1.0 / n.max(1) as f32;
        // the song is borrowed out of `self` for the block, so the loop can mutate the rest
        let audio = self.audio.take();
        for i in 0..n {
            let mut s = [0.0f32; 2];
            if let (true, Some(a)) = (self.playing, audio.as_deref()) {
                s = sample_at(a, self.pos);
                self.advance(a.frames() as f64, step);
            }
            let k = (i + 1) as f32 * inv;
            let trim = ramp(self.trim_ramp, k);
            let vol = ramp(self.vol_ramp, k);
            for (ch, v) in s.iter_mut().enumerate() {
                let mut x = (*v * trim) as f64;
                for (stage, q) in self.stages.iter().enumerate() {
                    if self.eq[stage / 2].cur.abs() >= FLAT_DB {
                        x = self.states[ch][stage].process(q, x);
                    }
                }
                if let Some(f) = &self.filt {
                    if self.filt_cold {
                        self.filt_state[ch].prime(f, x);
                    }
                    let y = self.filt_state[ch].process(f, x);
                    x += ramp(self.wet_ramp, k) as f64 * (y - x);
                }
                let y = x as f32 * vol;
                // a filter or EQ that blew up must never reach the speakers
                let y = if y.is_finite() { y } else { 0.0 };
                self.peak[ch] = self.peak[ch].max(y.abs());
                out[i * 2 + ch] = y;
            }
            self.filt_cold = false;
        }
        self.audio = audio;
    }

    #[inline]
    fn advance(&mut self, frames: f64, step: f64) {
        self.pos += step;
        if let Some((a, b)) = self.loop_ {
            let len = b - a;
            while self.pos >= b {
                self.pos -= len;
            }
        }
        if self.pos >= frames {
            self.pos = frames;
            self.playing = false;
            self.previewing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermite_is_exact_on_samples_and_smooth_between() {
        assert_eq!(hermite(0.0, 1.0, 2.0, 3.0, 0.0), 1.0);
        assert!((hermite(0.0, 1.0, 2.0, 3.0, 1.0) - 2.0).abs() < 1e-6);
        // a straight line stays straight
        assert!((hermite(0.0, 1.0, 2.0, 3.0, 0.25) - 1.25).abs() < 1e-6);
    }

    #[test]
    fn smoothing_settles_exactly() {
        let mut s = Smooth::new(0.0);
        s.target = 1.0;
        let coef = smooth_coef(48_000);
        for _ in 0..2000 {
            s.step(coef);
        }
        assert!(s.settled() && s.cur == 1.0);
    }

    #[test]
    fn audio_frames_outside_the_song_are_silent() {
        let a = DeckAudio::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], 44_100, None, 0.0);
        assert_eq!(a.frames(), 2, "an odd trailing sample is dropped");
        assert_eq!(a.frame(-1), [0.0; 2]);
        assert_eq!(a.frame(1), [3.0, 4.0]);
        assert_eq!(a.frame(2), [0.0; 2]);
        assert_eq!(a.bytes(), 16);
    }
}
