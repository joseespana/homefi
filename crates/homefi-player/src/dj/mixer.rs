//! The DJ mixer: two decks, the crossfader, the sampler, the master limiter and the automatic
//! transition. [`DjMixer::render`] is what the audio callback runs. It touches no allocator,
//! lock (only a `try_lock` to publish its state) or file, and depends on nothing but its own state,
//! so the tests drive it offline, sample by sample.

use std::sync::{Arc, Mutex};

use homefi_core::dj::mixer_math::EQ_KILL_DB;
use homefi_core::dj::settings::nearest_auto_mix_beats;
use homefi_core::dj::sync::{phase_offset_ms, tempo_for_sync};
use homefi_core::dj::{sfx, CrossfaderCurve, Grid, MixerConfig, SfxKind, DECK_COUNT, HOT_CUES, PAD_COUNT};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;

use super::control::DjControl;
use super::deck::{smooth_coef, Deck, DeckAudio, Smooth, CONTROL_BLOCK};
use crate::deck::soft_limit;

/// Beat length assumed for a transition when a song has no beat grid (120 BPM).
const FALLBACK_BEAT_SECONDS: f64 = 0.5;
/// Where in the transition the bass changes hands: it starts here and takes this long (fractions).
const BASS_SWAP_START: f32 = 0.4;
const BASS_SWAP_SPAN: f32 = 0.2;
/// Master volume range.
const MAX_MASTER: f32 = 1.5;
/// Output level above which the limiter counts as working.
const LIMITING_LEVEL: f32 = 0.9;

/// What travels through the lock-free command ring to the audio callback.
pub enum Command {
    Control(DjControl),
    Load { deck: usize, audio: Arc<DeckAudio> },
    Config(MixerConfig),
}

/// A deck as the screen needs it, copied out of the callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckView {
    pub loaded: bool,
    pub playing: bool,
    pub pos_frames: f64,
    pub src_rate: f64,
    pub cue_frames: f64,
    pub hot_frames: [Option<f64>; HOT_CUES],
    pub loop_frames: Option<(f64, f64)>,
    pub tempo: f64,
    pub bend: f32,
    pub synced: bool,
    pub current_bpm: f64,
    pub trim_db: f32,
    pub eq_db: [f32; 3],
    pub eq_kill: [bool; 3],
    pub filter: f32,
    pub volume: f32,
    /// Peak since the screen last took it.
    pub peak: [f32; 2],
}

impl Default for DeckView {
    fn default() -> Self {
        Self {
            loaded: false,
            playing: false,
            pos_frames: 0.0,
            src_rate: 44_100.0,
            cue_frames: 0.0,
            hot_frames: [None; HOT_CUES],
            loop_frames: None,
            tempo: 0.0,
            bend: 0.0,
            synced: false,
            current_bpm: 0.0,
            trim_db: 0.0,
            eq_db: [0.0; 3],
            eq_kill: [false; 3],
            filter: 0.0,
            volume: 1.0,
            peak: [0.0; 2],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoMixView {
    pub to: usize,
    pub beats: u32,
    pub armed: bool,
    pub progress: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixerView {
    pub decks: [DeckView; DECK_COUNT],
    pub crossfader: f32,
    pub curve: CrossfaderCurve,
    pub master: f32,
    pub peak: [f32; 2],
    pub limiting: bool,
    pub auto_mix: Option<AutoMixView>,
    pub sampler_volume: f32,
    pub pads_active: [bool; PAD_COUNT],
}

impl Default for MixerView {
    fn default() -> Self {
        let cfg = MixerConfig::default();
        Self {
            decks: [DeckView::default(); DECK_COUNT],
            crossfader: 0.5,
            curve: cfg.crossfader_curve,
            master: 1.0,
            peak: [0.0; 2],
            limiting: false,
            auto_mix: None,
            sampler_volume: cfg.sampler_volume,
            pads_active: [false; PAD_COUNT],
        }
    }
}

/// The mixer's state as the rest of the app reads it: the callback writes with `try_lock` (it
/// skips a block when the reader holds the lock), the reader takes the peaks it sees.
#[derive(Default)]
pub struct DjShared {
    view: Mutex<MixerView>,
}

impl DjShared {
    /// A copy of the state; the peaks it holds are reset (each reader sees a peak once).
    pub fn take(&self) -> MixerView {
        let mut guard = self.view.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let copy = *guard;
        for d in &mut guard.decks {
            d.peak = [0.0; 2];
        }
        guard.peak = [0.0; 2];
        guard.limiting = false;
        copy
    }

    fn publish(&self, mut next: MixerView) -> bool {
        let Ok(mut guard) = self.view.try_lock() else { return false };
        // peaks and the limiter flag accumulate until somebody reads them
        for (n, old) in next.decks.iter_mut().zip(&guard.decks) {
            n.peak = [n.peak[0].max(old.peak[0]), n.peak[1].max(old.peak[1])];
        }
        next.peak = [next.peak[0].max(guard.peak[0]), next.peak[1].max(guard.peak[1])];
        next.limiting |= guard.limiting;
        *guard = next;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AutoPhase {
    /// Waiting for the outgoing song's next beat (output frames left).
    Armed {
        wait: u64,
    },
    Running,
}

#[derive(Debug, Clone, Copy)]
struct AutoMix {
    to: usize,
    from: usize,
    beats: u32,
    phase: AutoPhase,
    total: u64,
    elapsed: u64,
    x_from: f32,
    x_to: f32,
    low_from: f32,
    low_to: f32,
}

impl AutoMix {
    fn progress(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            (self.elapsed as f64 / self.total as f64).min(1.0) as f32
        }
    }
}

/// The eight pads: their sounds (rendered once, at the output rate) and where each one is.
struct Sampler {
    pads: Vec<Vec<f32>>,
    /// Frame each pad is at while sounding.
    voices: [Option<usize>; PAD_COUNT],
}

impl Sampler {
    fn new(rate: u32, sounds: &[SfxKind; PAD_COUNT]) -> Self {
        Self { pads: sounds.iter().map(|k| sfx::render(*k, rate)).collect(), voices: [None; PAD_COUNT] }
    }

    fn trigger(&mut self, pad: usize) {
        if let Some(v) = self.voices.get_mut(pad) {
            *v = Some(0);
        }
    }

    /// Next frame of every sounding pad, summed.
    #[inline]
    fn next(&mut self) -> f32 {
        let mut sum = 0.0;
        for (voice, pad) in self.voices.iter_mut().zip(&self.pads) {
            if let Some(pos) = voice {
                match pad.get(*pos) {
                    Some(s) => {
                        sum += *s;
                        *pos += 1;
                    }
                    None => *voice = None,
                }
            }
        }
        sum
    }
}

pub struct DjMixer {
    cfg: MixerConfig,
    rate: u32,
    coef: f32,
    decks: [Deck; DECK_COUNT],
    xf_target: f32,
    xf: f32,
    gains: (f32, f32),
    master: Smooth,
    sampler: Sampler,
    auto: Option<AutoMix>,
    shared: Arc<DjShared>,
    /// Songs replaced or ejected go back to the engine thread to be freed there.
    trash: Option<HeapProd<Arc<DeckAudio>>>,
    peak: [f32; 2],
    limiting: bool,
}

impl DjMixer {
    /// A mixer for an output of `sample_rate` Hz. Builds the sampler's sounds (allocates):
    /// create it before the stream starts.
    pub fn new(sample_rate: u32, cfg: MixerConfig, shared: Arc<DjShared>) -> Self {
        Self::with_pads(sample_rate, cfg, shared, &SfxKind::ALL)
    }

    /// Like [`Self::new`] with the sound of each pad chosen (`DjLayout::pad_sounds`). The sounds are
    /// rendered here, once: changing them means a new mixer (the next time DJ mode starts).
    pub fn with_pads(
        sample_rate: u32,
        cfg: MixerConfig,
        shared: Arc<DjShared>,
        pad_sounds: &[SfxKind; PAD_COUNT],
    ) -> Self {
        let rate = sample_rate.max(1);
        let xf = 0.5;
        Self {
            cfg,
            rate,
            coef: smooth_coef(rate),
            decks: [Deck::new(rate), Deck::new(rate)],
            xf_target: xf,
            xf,
            gains: cfg.crossfader_curve.gains(xf),
            master: Smooth::new(1.0),
            sampler: Sampler::new(rate, pad_sounds),
            auto: None,
            shared,
            trash: None,
            peak: [0.0; 2],
            limiting: false,
        }
    }

    pub fn set_trash(&mut self, trash: HeapProd<Arc<DeckAudio>>) {
        self.trash = Some(trash);
    }

    pub fn shared(&self) -> &Arc<DjShared> {
        &self.shared
    }

    /// The state as it is now (what [`DjShared`] would show), without touching the lock.
    pub fn view(&self) -> MixerView {
        self.build_view()
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Control(c) => self.apply(c),
            Command::Load { deck, audio } => self.load(deck, audio),
            Command::Config(cfg) => self.cfg = cfg,
        }
    }

    pub fn load(&mut self, deck: usize, audio: Arc<DeckAudio>) {
        if deck >= DECK_COUNT {
            return;
        }
        if self.auto.is_some_and(|a| a.to == deck || a.from == deck) {
            self.cancel_auto();
        }
        let old = self.decks[deck].load(audio, &self.cfg);
        self.discard(old);
        // a song loaded next to a playing one takes its tempo
        let other = 1 - deck;
        if self.cfg.sync_on_load && self.decks[other].playing {
            self.sync_tempo(deck);
        }
    }

    fn discard(&mut self, old: Option<Arc<DeckAudio>>) {
        let Some(old) = old else { return };
        match &mut self.trash {
            // if the ring is full the engine thread is stuck; freeing here is the lesser evil
            Some(ring) => drop(ring.try_push(old)),
            None => drop(old),
        }
    }

    pub fn apply(&mut self, control: DjControl) {
        let cfg = self.cfg;
        match control {
            DjControl::Play { deck } => self.play(deck.index()),
            DjControl::Pause { deck } => self.decks[deck.index()].pause(),
            DjControl::TogglePlay { deck } => {
                if self.decks[deck.index()].playing {
                    self.decks[deck.index()].pause();
                } else {
                    self.play(deck.index());
                }
            }
            DjControl::Cue { deck, pressed } => self.decks[deck.index()].cue_button(pressed, &cfg),
            DjControl::Seek { deck, ms } => {
                let d = &mut self.decks[deck.index()];
                let frames = d.ms_to_frames(ms);
                d.seek_frames(frames);
            }
            DjControl::Nudge { deck, ms } => {
                let d = &mut self.decks[deck.index()];
                let frames = d.pos + d.ms_to_frames(ms);
                d.seek_frames(frames);
            }
            DjControl::Bend { deck, amount } => {
                self.decks[deck.index()].bend = if amount.is_nan() { 0.0 } else { amount.clamp(-1.0, 1.0) };
            }
            DjControl::HotCue { deck, index } => self.decks[deck.index()].hot_cue(index as usize, &cfg),
            DjControl::SetHotCue { deck, index } => self.decks[deck.index()].set_hot_cue(index as usize, &cfg),
            DjControl::ClearHotCue { deck, index } => self.decks[deck.index()].clear_hot_cue(index as usize),
            DjControl::LoopBeats { deck, beats } => self.decks[deck.index()].loop_beats(beats, &cfg),
            DjControl::LoopIn { deck } => self.decks[deck.index()].loop_in(&cfg),
            DjControl::LoopOut { deck } => self.decks[deck.index()].loop_out(&cfg),
            DjControl::LoopExit { deck } => self.decks[deck.index()].loop_exit(),
            DjControl::Tempo { deck, percent } => self.set_tempo(deck.index(), percent),
            DjControl::Sync { deck } => self.sync_deck(deck.index()),
            DjControl::SyncOff { deck } => self.decks[deck.index()].synced = false,
            DjControl::Trim { deck, db } => self.decks[deck.index()].set_trim_db(db),
            DjControl::Eq { deck, band, db } => self.decks[deck.index()].set_eq_db(band.index(), db, &cfg),
            DjControl::EqKill { deck, band, on } => self.decks[deck.index()].kill[band.index()] = on,
            DjControl::Filter { deck, value } => self.decks[deck.index()].set_filter(value),
            DjControl::Volume { deck, value } => self.decks[deck.index()].set_volume(value),
            DjControl::Eject { deck } => {
                let i = deck.index();
                if self.auto.is_some_and(|a| a.to == i || a.from == i) {
                    self.cancel_auto();
                }
                let old = self.decks[i].eject();
                self.discard(old);
            }
            DjControl::Crossfader { value } => {
                // touching the fader takes the mix back
                self.cancel_auto();
                self.xf_target = if value.is_nan() { 0.5 } else { value.clamp(0.0, 1.0) };
            }
            DjControl::Master { value } => {
                self.master.target = if value.is_nan() { 1.0 } else { value.clamp(0.0, MAX_MASTER) };
            }
            DjControl::SetCurve { curve } => self.cfg.crossfader_curve = curve,
            DjControl::AutoMix { to, beats } => self.start_auto_mix(to.index(), beats),
            DjControl::AutoMixCancel => self.cancel_auto(),
            DjControl::Sampler { pad } => self.sampler.trigger(pad as usize),
            DjControl::SamplerVolume { value } => {
                self.cfg.sampler_volume = if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) };
            }
        }
    }

    /// The tempo fader. Moving it on a synced deck keeps the pair together: this deck leads
    /// from now on and the other one follows it (a beginner never loses the match by touching it).
    fn set_tempo(&mut self, i: usize, percent: f32) {
        let cfg = self.cfg;
        let was_synced = self.decks[i].synced;
        self.decks[i].set_tempo_pct(percent, &cfg);
        self.decks[i].synced = false;
        if was_synced && self.grids(i).is_some() {
            self.decks[1 - i].synced = true;
        }
    }

    /// Play; a synced deck starts on the beat of the other one.
    fn play(&mut self, i: usize) {
        if self.decks[i].synced && self.decks[1 - i].playing {
            self.align_phase(i);
        }
        self.decks[i].play();
    }

    /// Both grids, as `(own, leader)`.
    fn grids(&self, follower: usize) -> Option<(Grid, Grid)> {
        Some((self.decks[follower].grid()?, self.decks[1 - follower].grid()?))
    }

    /// Sets the follower's tempo to the leader's. `false` when that is impossible (no grids, or
    /// beyond the tempo range).
    fn sync_tempo(&mut self, follower: usize) -> bool {
        let Some((own, _)) = self.grids(follower) else { return false };
        let lead_bpm = self.decks[1 - follower].current_bpm();
        match tempo_for_sync(own.bpm(), lead_bpm, self.cfg.tempo_range_pct as f64) {
            Some(ratio) => {
                self.decks[follower].tempo = ratio - 1.0;
                self.decks[follower].synced = true;
                self.decks[1 - follower].synced = false;
                true
            }
            None => false,
        }
    }

    /// Moves the follower's playhead the short way onto the leader's beat. Only when the tempos
    /// really match (not for half / double time).
    fn align_phase(&mut self, follower: usize) {
        let Some((own, lead)) = self.grids(follower) else { return };
        let (f, l) = (&self.decks[follower], &self.decks[1 - follower]);
        let ratio = f.current_bpm() / l.current_bpm();
        if !(0.995..=1.005).contains(&ratio) {
            return;
        }
        let shift = phase_offset_ms(&lead, l.pos_ms(), &own, f.pos_ms());
        let target = f.pos + f.ms_to_frames(shift);
        self.decks[follower].seek_frames(target);
    }

    fn sync_deck(&mut self, i: usize) {
        if self.sync_tempo(i) && self.decks[1 - i].playing {
            self.align_phase(i);
        }
    }

    fn start_auto_mix(&mut self, to: usize, requested_beats: u32) {
        if to >= DECK_COUNT || !self.decks[to].is_loaded() {
            return;
        }
        self.cancel_auto();
        let from = 1 - to;
        let beats =
            if requested_beats == 0 { self.cfg.auto_mix_beats } else { nearest_auto_mix_beats(requested_beats) };
        let x_to = if to == 0 { 0.0 } else { 1.0 };
        if !(self.decks[from].playing && self.decks[from].is_loaded()) {
            // nothing to mix out of: the new song just comes in
            self.decks[to].play();
            self.xf_target = x_to;
            return;
        }

        let cfg = self.cfg;
        let from_step = self.decks[from].step(&cfg).max(1e-6);
        let (wait, total) = match self.grids(to) {
            Some((to_grid, from_grid)) => {
                self.sync_tempo(to);
                if !self.decks[to].playing {
                    let at = to_grid.nearest_beat_ms(self.decks[to].pos_ms());
                    let at = if at < 0.0 { to_grid.next_beat_ms(at) } else { at };
                    let frames = self.decks[to].ms_to_frames(at);
                    self.decks[to].seek_frames(frames);
                }
                let f = &self.decks[from];
                let next = f.ms_to_frames(from_grid.next_beat_ms(f.pos_ms()));
                let wait = ((next - f.pos) / from_step).ceil().max(0.0) as u64;
                let beat = f.ms_to_frames(from_grid.beat_ms);
                (wait, (beats as f64 * beat / from_step).round() as u64)
            }
            None => (0, (beats as f64 * FALLBACK_BEAT_SECONDS * self.rate as f64).round() as u64),
        };

        let cap = cfg.eq_max_boost_db;
        let low_from = self.decks[from].eq_knob[0].clamp(EQ_KILL_DB, cap);
        let low_to = self.decks[to].eq_knob[0].clamp(EQ_KILL_DB, cap);
        // the incoming song enters without its bass (when the bass is swapped at all)
        if cfg.bass_swap {
            self.decks[to].low_override = Some(EQ_KILL_DB);
        }
        self.auto = Some(AutoMix {
            to,
            from,
            beats,
            phase: AutoPhase::Armed { wait },
            total: total.max(1),
            elapsed: 0,
            x_from: self.xf,
            x_to,
            low_from,
            low_to,
        });
    }

    fn cancel_auto(&mut self) {
        if let Some(a) = self.auto.take() {
            self.decks[a.from].low_override = None;
            self.decks[a.to].low_override = None;
        }
    }

    /// Before a block: starts the transition when its time has come and shortens the block so
    /// that the start and the end land exactly on a frame boundary.
    fn auto_begin(&mut self, n: usize) -> usize {
        let Some(a) = &mut self.auto else { return n };
        if let AutoPhase::Armed { wait: 0 } = a.phase {
            a.phase = AutoPhase::Running;
            let to = a.to;
            self.decks[to].play();
        }
        match a.phase {
            AutoPhase::Armed { wait } => n.min(wait as usize),
            AutoPhase::Running => n.min((a.total - a.elapsed) as usize),
        }
        .max(1)
    }

    /// After a block of `n` frames: advances the transition (crossfader and bass swap).
    fn auto_end(&mut self, n: usize) {
        let Some(mut a) = self.auto else { return };
        match &mut a.phase {
            AutoPhase::Armed { wait } => *wait = wait.saturating_sub(n as u64),
            AutoPhase::Running => {
                a.elapsed += n as u64;
                let p = a.progress();
                self.xf = a.x_from + (a.x_to - a.x_from) * p;
                self.xf_target = self.xf;
                if self.cfg.bass_swap {
                    let s = ((p - BASS_SWAP_START) / BASS_SWAP_SPAN).clamp(0.0, 1.0);
                    let s = s * s * (3.0 - 2.0 * s);
                    self.decks[a.from].low_override = Some(a.low_from + (EQ_KILL_DB - a.low_from) * s);
                    self.decks[a.to].low_override = Some(EQ_KILL_DB + (a.low_to - EQ_KILL_DB) * s);
                }
                if a.elapsed >= a.total {
                    // done: the outgoing song stops, the fader rests on the new one
                    self.decks[a.from].pause();
                    self.cancel_auto();
                    self.xf = a.x_to;
                    self.xf_target = a.x_to;
                    return;
                }
            }
        }
        self.auto = Some(a);
    }

    /// Follows the leader's tempo for every deck that is synced.
    fn follow_leaders(&mut self) {
        for i in 0..DECK_COUNT {
            if self.decks[i].synced {
                if let Some((own, _)) = self.grids(i) {
                    let lead_bpm = self.decks[1 - i].current_bpm();
                    if let Some(ratio) = tempo_for_sync(own.bpm(), lead_bpm, self.cfg.tempo_range_pct as f64) {
                        self.decks[i].tempo = ratio - 1.0;
                    }
                }
            }
        }
    }

    /// Fills `out` (interleaved, `channels` per frame) with the mix. Stereo goes to the first two
    /// channels; a mono output gets the mid; further channels stay silent.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let mut a_buf = [0f32; CONTROL_BLOCK * 2];
        let mut b_buf = [0f32; CONTROL_BLOCK * 2];
        let mut done = 0;
        while done < frames {
            let n = self.auto_begin((frames - done).min(CONTROL_BLOCK));
            let cfg = self.cfg;
            self.follow_leaders();
            self.decks[0].tick(&cfg);
            self.decks[1].tick(&cfg);
            self.decks[0].render(&cfg, n, &mut a_buf[..n * 2]);
            self.decks[1].render(&cfg, n, &mut b_buf[..n * 2]);

            // the crossfader glides unless a transition drives it
            if self.auto.is_none_or(|a| a.phase != AutoPhase::Running) {
                self.xf += (self.xf_target - self.xf) * self.coef;
                if (self.xf_target - self.xf).abs() < 1e-4 {
                    self.xf = self.xf_target;
                }
            }
            self.mix_block(n, &a_buf, &b_buf, &mut out[done * channels..(done + n) * channels], channels);
            self.auto_end(n);
            done += n;
        }
        out[frames * channels..].fill(0.0);
        self.shared.publish(self.build_view());
        self.peak = [0.0; 2];
        self.limiting = false;
        for d in &mut self.decks {
            d.peak = [0.0; 2];
        }
    }

    /// Mixes one block. The gains glide linearly across it, so a moving fader never steps.
    fn mix_block(&mut self, n: usize, a: &[f32], b: &[f32], out: &mut [f32], channels: usize) {
        let (g0, g1) = (self.gains, self.cfg.crossfader_curve.gains(self.xf));
        let (m0, m1) = self.master.step(self.coef);
        let inv = 1.0 / n as f32;
        let pad_gain = self.cfg.sampler_volume;
        for i in 0..n {
            let k = (i + 1) as f32 * inv;
            let (ga, gb) = (g0.0 + (g1.0 - g0.0) * k, g0.1 + (g1.1 - g0.1) * k);
            let m = m0 + (m1 - m0) * k;
            let pad = self.sampler.next() * pad_gain;
            let mut l = (a[i * 2] * ga + b[i * 2] * gb + pad) * m;
            let mut r = (a[i * 2 + 1] * ga + b[i * 2 + 1] * gb + pad) * m;
            if self.cfg.master_limiter {
                self.limiting |= l.abs() > LIMITING_LEVEL || r.abs() > LIMITING_LEVEL;
                l = soft_limit(l);
                r = soft_limit(r);
            } else {
                l = l.clamp(-1.0, 1.0);
                r = r.clamp(-1.0, 1.0);
            }
            self.peak = [self.peak[0].max(l.abs()), self.peak[1].max(r.abs())];
            let frame = &mut out[i * channels..(i + 1) * channels];
            if channels == 1 {
                frame[0] = 0.5 * (l + r);
            } else {
                frame[0] = l;
                frame[1] = r;
                frame[2..].fill(0.0);
            }
        }
        self.gains = g1;
    }

    fn build_view(&self) -> MixerView {
        let cfg = &self.cfg;
        let decks = std::array::from_fn(|i| {
            let d = &self.decks[i];
            DeckView {
                loaded: d.is_loaded(),
                playing: d.playing,
                pos_frames: d.pos,
                src_rate: d.src_rate(),
                cue_frames: d.cue,
                hot_frames: d.hot,
                loop_frames: d.loop_,
                tempo: d.tempo,
                bend: d.bend,
                synced: d.synced,
                current_bpm: d.current_bpm(),
                trim_db: d.user_trim_db,
                eq_db: d.eq_knob,
                eq_kill: d.kill,
                filter: d.filter_knob,
                volume: d.volume_knob,
                peak: d.peak,
            }
        });
        MixerView {
            decks,
            crossfader: self.xf,
            curve: cfg.crossfader_curve,
            master: self.master.target,
            peak: self.peak,
            limiting: self.limiting,
            auto_mix: self.auto.map(|a| AutoMixView {
                to: a.to,
                beats: a.beats,
                armed: matches!(a.phase, AutoPhase::Armed { .. }),
                progress: a.progress(),
            }),
            sampler_volume: cfg.sampler_volume,
            pads_active: std::array::from_fn(|p| self.sampler.voices[p].is_some()),
        }
    }

    #[cfg(test)]
    pub(crate) fn deck(&self, i: usize) -> &Deck {
        &self.decks[i]
    }
}
