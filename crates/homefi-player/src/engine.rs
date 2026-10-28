//! The audio thread: queue, decoding, output (re)configuration and events.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::DeviceTrait;
use homefi_core::dsp::crossfade::{CrossfadeSettings, TrackInfo, MAX_TRIM_SECONDS};
use homefi_core::dsp::{
    self, EqDesign, EqSettings, LoudnessSettings, ReplayGain, SoundPlan, TasteSettings, TrackSound,
};
use rand::seq::SliceRandom;
use ringbuf::traits::{Observer, Producer};
use serde::{Deserialize, Serialize};

use crate::deck::{music_end, Deck, DeckFormat, Fade, FadeSpec, ResamplerQuality, SurroundMode};
use crate::decoder::Source;
use crate::error::{PlayerError, Result};
use crate::output::{find_device, Meters, Output, Shared};

/// How often position updates are emitted while playing.
const TICK: Duration = Duration::from_millis(250);
/// "Previous" restarts the track instead when past this point.
const RESTART_THRESHOLD_MS: u64 = 3000;
/// Decoding time per pass spent reading ahead for a crossfade (the ring
/// buffer is always fed first).
const LOOKAHEAD_BUDGET: Duration = Duration::from_millis(3);
/// A fade shorter than this isn't worth it: a normal transition instead.
const MIN_FADE_SECONDS: f32 = 0.25;
/// Extra look-ahead past the fade and the trimmed silence.
const HOLD_MARGIN_SECONDS: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// What reaches the device for the current track.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamInfo {
    pub codec: String,
    pub source_rate: u32,
    pub bits_per_sample: Option<u32>,
    pub source_channels: u16,
    pub output_rate: u32,
    pub output_channels: u16,
    /// `true` when the device could not be opened at the file's rate.
    pub resampled: bool,
    pub device: String,
    /// How a stereo / mono track is spread over a surround device (more
    /// speakers than the track has channels); `None` otherwise.
    pub surround: Option<SurroundMode>,
    /// The resampler's quality, when the track is resampled.
    pub resampler: Option<ResamplerQuality>,
}

/// A crossfade the listener is hearing: the signal is not bit-perfect until it ends.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fading {
    /// The outgoing and incoming tracks.
    pub from: PathBuf,
    pub to: PathBuf,
    /// 0..=1
    pub progress: f32,
    /// Length of the fade.
    pub seconds: f32,
    /// The short fade of a skip or a pick.
    pub manual: bool,
}

/// How tracks reach a device with other channels or rates (desktop settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OutputOptions {
    pub surround: SurroundMode,
    pub resampler: ResamplerQuality,
}

/// Player state sent to listeners.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Snapshot {
    pub status: Status,
    pub queue: Vec<PathBuf>,
    /// Index into `queue` of the track being heard.
    pub current: Option<usize>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Device chosen by the user (`None` = system default).
    pub preferred_device: Option<String>,
    pub stream: Option<StreamInfo>,
    pub error: Option<String>,
    /// The equalizer is changing what reaches the device: the path is not
    /// bit-perfect. `false` when it is off, flat, or nothing is playing.
    pub eq_active: bool,
    /// Name of the equalizer profile in use (when `eq_active`).
    pub eq_profile: Option<String>,
    /// Id and label of the taste rule playing (when `eq_active`).
    pub taste_rule: Option<String>,
    pub taste_label: Option<String>,
    /// Loudness normalization gain applied to this track (dB; 0 = none).
    pub loudness_db: f32,
    /// A crossfade is playing (see [`Player::set_crossfade`]).
    pub fading: Option<Fading>,
}

enum Command {
    Load { queue: Vec<PathBuf>, start: usize, position_ms: u64, play: bool },
    Append(Vec<PathBuf>),
    SetUpcoming(Vec<PathBuf>),
    Move { from: usize, to: usize },
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Jump(usize),
    Seek(u64),
    Volume(f32),
    Device(Option<String>),
    Shuffle(bool),
    Repeat(RepeatMode),
    Eq(Box<EqSettings>),
    Taste(Box<TasteSettings>),
    Loudness(LoudnessSettings),
    SoundSource(SoundSource),
    Crossfade(CrossfadeSettings),
    Output(OutputOptions),
    Stop,
    Shutdown,
}

pub type Listener = Arc<dyn Fn(&Snapshot) + Send + Sync>;

/// Tells the engine what a file is (genre, album, artist, where the queue
/// came from, known gains), for taste layers and loudness. Called on the
/// audio thread once per track: keep it quick (no network).
pub type SoundSource = Arc<dyn Fn(&Path) -> TrackSound + Send + Sync>;

/// Handle to the audio thread. Cheap to clone; all methods are non-blocking.
#[derive(Clone)]
pub struct Player {
    tx: Sender<Command>,
    snapshot: Arc<Mutex<Snapshot>>,
    meters: Arc<Meters>,
}

impl Player {
    pub fn new(volume: f32, device: Option<String>, listener: Listener) -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot =
            Arc::new(Mutex::new(Snapshot { volume, preferred_device: device.clone(), ..Default::default() }));
        let shared_snapshot = snapshot.clone();
        let meters = Arc::new(Meters::default());
        let engine_meters = meters.clone();
        std::thread::Builder::new()
            .name("homefi-audio".into())
            .spawn(move || Engine::new(rx, volume, device, listener, shared_snapshot, engine_meters).run())
            .expect("spawn audio thread");
        Self { tx, snapshot, meters }
    }

    /// Output peak levels `(left, right)`, linear 0..=1, since the last call.
    pub fn levels(&self) -> (f32, f32) {
        self.meters.take()
    }

    /// Spectrum (`bands` log-spaced bands), waveform and peaks of what the
    /// device is playing right now. Cheap enough to call 30-60 times a second.
    pub fn visual(&self, bands: usize) -> crate::visual::Visual {
        let (left, right) = self.meters.take();
        let (spectrum, wave) = crate::visual::analyze(&self.meters.tap, bands.clamp(1, 256));
        crate::visual::Visual { left, right, spectrum, wave }
    }

    fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Replaces the queue and starts at `start` (index into `queue`).
    pub fn load(&self, queue: Vec<PathBuf>, start: usize, position_ms: u64, play: bool) {
        self.send(Command::Load { queue, start, position_ms, play });
    }
    /// Adds tracks to the end of the queue without interrupting playback. If the
    /// queue already finished, playback continues with the first new track.
    pub fn append(&self, tracks: Vec<PathBuf>) {
        self.send(Command::Append(tracks))
    }
    /// Replaces everything after the track being heard with `tracks` (it keeps
    /// playing; shuffle turns off). Used by shared control, where the hub decides
    /// what comes next. If the queue had finished, playback goes on with them.
    pub fn set_upcoming(&self, tracks: Vec<PathBuf>) {
        self.send(Command::SetUpcoming(tracks))
    }
    /// Moves `queue[from]` to `to`; the track being heard keeps playing.
    pub fn move_track(&self, from: usize, to: usize) {
        self.send(Command::Move { from, to })
    }
    pub fn play(&self) {
        self.send(Command::Play)
    }
    pub fn pause(&self) {
        self.send(Command::Pause)
    }
    pub fn toggle(&self) {
        self.send(Command::Toggle)
    }
    pub fn next(&self) {
        self.send(Command::Next)
    }
    pub fn previous(&self) {
        self.send(Command::Previous)
    }
    /// Plays `queue[index]`.
    pub fn jump(&self, index: usize) {
        self.send(Command::Jump(index))
    }
    pub fn seek(&self, ms: u64) {
        self.send(Command::Seek(ms))
    }
    pub fn set_volume(&self, volume: f32) {
        self.send(Command::Volume(volume.clamp(0.0, 1.0)))
    }
    /// `None` follows the system default output.
    pub fn set_device(&self, name: Option<String>) {
        self.send(Command::Device(name))
    }
    pub fn set_shuffle(&self, on: bool) {
        self.send(Command::Shuffle(on))
    }
    pub fn set_repeat(&self, mode: RepeatMode) {
        self.send(Command::Repeat(mode))
    }
    pub fn stop(&self) {
        self.send(Command::Stop)
    }
    /// Equalizer settings: the profile for the device in use (or the default
    /// one) applies right away, gliding from the previous one.
    pub fn set_eq(&self, settings: EqSettings) {
        self.send(Command::Eq(Box::new(settings)))
    }
    /// Taste layers: the rule for each track is found when it starts and
    /// glides in at its first sample (also when the rules change).
    pub fn set_taste(&self, settings: TasteSettings) {
        self.send(Command::Taste(Box::new(settings)))
    }
    /// Loudness normalization (ReplayGain from [`TrackSound`] or the file's tags).
    pub fn set_loudness(&self, settings: LoudnessSettings) {
        self.send(Command::Loudness(settings))
    }
    /// Where the engine learns what each file is (see [`SoundSource`]).
    pub fn set_sound_source(&self, source: SoundSource) {
        self.send(Command::SoundSource(source))
    }
    /// Crossfade between tracks (off: gapless and bit-perfect). Applies from
    /// the next transition.
    pub fn set_crossfade(&self, settings: CrossfadeSettings) {
        self.send(Command::Crossfade(settings))
    }
    /// Surround spreading and resampler quality; the track playing restarts
    /// where it is to pick them up.
    pub fn set_output_options(&self, options: OutputOptions) {
        self.send(Command::Output(options))
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // last handle gone: stop the thread
        if Arc::strong_count(&self.snapshot) <= 2 {
            let _ = self.tx.send(Command::Shutdown);
        }
    }
}

/// Where queue index `i` ends up after moving the entry at `from` to `to`.
fn moved_index(i: usize, from: usize, to: usize) -> usize {
    if i == from {
        to
    } else if from < to && i > from && i <= to {
        i - 1
    } else if to < from && i >= to && i < from {
        i + 1
    } else {
        i
    }
}

/// A track whose samples were pushed starting at `start_frame`.
struct Segment {
    order_pos: usize,
    start_frame: u64,
    /// Frame where its filters take over (the middle of a crossfade into it).
    eq_frame: u64,
    start_ms: u64,
    /// Leading silence dropped (smart crossfade), counted into the position.
    skipped_frames: u64,
    duration_ms: Option<u64>,
    info: StreamInfo,
    path: PathBuf,
    sound: TrackSound,
    /// ReplayGain from the file's tags, read the first time loudness needs it.
    tags_gain: Option<Option<ReplayGain>>,
    /// What runs for it on the open output.
    plan: SoundPlan,
}

struct Engine {
    rx: Receiver<Command>,
    listener: Listener,
    published: Arc<Mutex<Snapshot>>,

    queue: Vec<PathBuf>,
    order: Vec<usize>,
    /// Position in `order` of the track being decoded.
    decoding: Option<usize>,

    /// The track being decoded.
    deck: Option<Deck>,
    source_eof: bool,
    output: Option<Output>,
    shared: Arc<Shared>,
    meters: Arc<Meters>,
    pushed_frames: u64,
    /// Samples ready for the ring buffer, in order.
    pending: Vec<f32>,
    /// The last decoded chunk.
    decoded: Vec<f32>,
    /// Crossfade look-ahead: the newest decoded seconds, held back behind
    /// `pending` so the end of a track is known before it is heard.
    held: VecDeque<f32>,
    /// The crossfade being mixed / heard.
    fade: Option<Fade>,
    segments: VecDeque<Segment>,
    /// Next track that needs a different output config: started once the buffer drains.
    after_drain: Option<usize>,

    status: Status,
    volume: f32,
    device: Option<String>,
    shuffle: bool,
    repeat: RepeatMode,
    error: Option<String>,
    eq: EqSettings,
    taste: TasteSettings,
    loudness: LoudnessSettings,
    sound_source: Option<SoundSource>,
    crossfade: CrossfadeSettings,
    output_options: OutputOptions,
}

impl Engine {
    fn new(
        rx: Receiver<Command>,
        volume: f32,
        device: Option<String>,
        listener: Listener,
        published: Arc<Mutex<Snapshot>>,
        meters: Arc<Meters>,
    ) -> Self {
        Self {
            rx,
            listener,
            published,
            queue: Vec::new(),
            order: Vec::new(),
            decoding: None,
            deck: None,
            source_eof: false,
            output: None,
            shared: Shared::new(volume, meters.clone()),
            meters,
            pushed_frames: 0,
            pending: Vec::new(),
            decoded: Vec::new(),
            held: VecDeque::new(),
            fade: None,
            segments: VecDeque::new(),
            after_drain: None,
            status: Status::Stopped,
            volume,
            device,
            shuffle: false,
            repeat: RepeatMode::Off,
            error: None,
            eq: EqSettings::default(),
            taste: TasteSettings::default(),
            loudness: LoudnessSettings::default(),
            sound_source: None,
            crossfade: CrossfadeSettings::default(),
            output_options: OutputOptions::default(),
        }
    }

    fn run(mut self) {
        let mut last_tick = Instant::now();
        loop {
            let timeout = if self.status == Status::Playing { Duration::from_millis(5) } else { TICK };
            match self.rx.recv_timeout(timeout) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(cmd) => {
                    self.handle(cmd);
                    self.publish();
                }
                Err(RecvTimeoutError::Timeout) => {}
            }

            if self.status == Status::Playing {
                if self.shared.failed.swap(false, Ordering::Relaxed) {
                    self.error = Some("the audio device was disconnected, switched to the default output".into());
                    self.restart_current(true);
                }
                self.fill();
                if self.advance_segments() || last_tick.elapsed() >= TICK {
                    last_tick = Instant::now();
                    self.publish();
                }
            }
        }
    }

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Load { queue, start, position_ms, play } => {
                self.queue = queue;
                self.rebuild_order(start);
                let pos = self.order.iter().position(|&i| i == start).unwrap_or(0);
                self.error = None;
                if play {
                    self.start(pos, position_ms);
                } else {
                    self.close();
                    self.decoding = Some(pos);
                    self.status = Status::Stopped;
                }
            }
            Command::Append(tracks) => self.append(tracks),
            Command::SetUpcoming(tracks) => self.set_upcoming(tracks),
            Command::Move { from, to } => self.move_track(from, to),
            Command::Play => self.play(),
            Command::Pause => self.pause(),
            Command::Toggle => {
                if self.status == Status::Playing {
                    self.pause()
                } else {
                    self.play()
                }
            }
            Command::Next => {
                if let Some(pos) = self.next_pos(self.current_pos(), false) {
                    self.skip_to(pos);
                }
            }
            Command::Previous => {
                let current = self.current_pos();
                if self.position_ms() > RESTART_THRESHOLD_MS || current == Some(0) && self.repeat != RepeatMode::All {
                    if let Some(pos) = current {
                        self.skip_to(pos);
                    }
                } else if let Some(pos) = current.and_then(|p| self.prev_pos(p)) {
                    self.skip_to(pos);
                }
            }
            Command::Jump(index) => {
                if let Some(pos) = self.order.iter().position(|&i| i == index) {
                    self.skip_to(pos);
                }
            }
            Command::Seek(ms) => {
                if let Some(pos) = self.current_pos() {
                    let was_paused = self.status == Status::Paused;
                    self.start(pos, ms);
                    if was_paused {
                        self.pause();
                    }
                }
            }
            Command::Volume(v) => {
                self.volume = v;
                self.shared.volume.store(v.to_bits(), Ordering::Relaxed);
            }
            Command::Device(name) => {
                self.device = name;
                if self.output.is_some() {
                    self.restart_current(false);
                }
            }
            Command::Shuffle(on) => {
                self.shuffle = on;
                let current = self.current_pos().map(|p| self.order[p]);
                self.rebuild_order(current.unwrap_or(0));
                // keep playing the same track; its place in the new order changed
                let new_pos = current.and_then(|q| self.order.iter().position(|&i| i == q));
                for seg in &mut self.segments {
                    seg.order_pos = new_pos.unwrap_or(0);
                }
                if let Some(p) = new_pos {
                    self.decoding = Some(p);
                }
                // album loudness only while an album plays in order
                self.replan();
            }
            Command::Repeat(mode) => self.repeat = mode,
            Command::Eq(settings) => {
                self.eq = settings.clamped();
                self.replan();
            }
            Command::Taste(settings) => {
                self.taste = settings.clamped();
                self.replan();
            }
            Command::Loudness(settings) => {
                self.loudness = settings.clamped();
                self.replan();
            }
            Command::SoundSource(source) => {
                self.sound_source = Some(source);
                for seg in &mut self.segments {
                    seg.sound = self.sound_source.as_ref().map(|f| f(&seg.path)).unwrap_or_default();
                }
                self.replan();
            }
            Command::Crossfade(settings) => self.crossfade = settings.clamped(),
            Command::Output(options) => {
                let changed = options != self.output_options;
                self.output_options = options;
                if changed && self.output.is_some() {
                    self.restart_current(false);
                }
            }
            Command::Stop => {
                self.close();
                self.status = Status::Stopped;
            }
            Command::Shutdown => {}
        }
    }

    /// `order` = queue order, or shuffled with `first` in front.
    fn rebuild_order(&mut self, first: usize) {
        self.order = (0..self.queue.len()).collect();
        if self.shuffle && !self.order.is_empty() {
            self.order.retain(|&i| i != first);
            self.order.shuffle(&mut rand::thread_rng());
            self.order.insert(0, first.min(self.queue.len() - 1));
        }
    }

    fn append(&mut self, tracks: Vec<PathBuf>) {
        if tracks.is_empty() {
            return;
        }
        // ran out of tracks while these were on their way
        let finished = self.source_eof && self.after_drain.is_none() && !self.queue.is_empty();
        let first_new = self.order.len();
        for path in tracks {
            let index = self.queue.len();
            self.queue.push(path);
            if self.shuffle {
                let after = self.current_pos().map_or(0, |p| p + 1);
                let at = after
                    + rand::Rng::gen_range(&mut rand::thread_rng(), 0..=self.order.len() - after.min(self.order.len()));
                self.order.insert(at.min(self.order.len()), index);
            } else {
                self.order.push(index);
            }
        }
        if !finished || self.repeat == RepeatMode::One {
            return;
        }
        let pos = if self.shuffle { self.current_pos().map_or(first_new, |p| p + 1) } else { first_new };
        match self.status {
            // still playing the tail of the last track: continue after it
            Status::Playing if self.output.is_some() => self.after_drain = Some(pos),
            Status::Stopped if self.output.is_none() => self.start(pos, 0),
            _ => {}
        }
    }

    fn set_upcoming(&mut self, tracks: Vec<PathBuf>) {
        let finished = self.source_eof && self.after_drain.is_none() && !self.queue.is_empty();
        let cur = self.current_pos();
        let played: Vec<PathBuf> = match cur {
            Some(p) => self.order.iter().take(p + 1).map(|&i| self.queue[i].clone()).collect(),
            None => Vec::new(),
        };
        // the next track may already be decoded behind the current one (gapless): it will be heard
        let decoded = match (cur, self.decoding) {
            (Some(c), Some(d)) if d > c => self.order.get(d).map(|&i| self.queue[i].clone()),
            _ => None,
        };
        let keep = played.len();
        self.queue = upcoming_queue(played, decoded, tracks);
        // queue order from here on: positions up to the current one keep their meaning
        self.shuffle = false;
        self.order = (0..self.queue.len()).collect();
        let len = self.queue.len();
        self.decoding = self.decoding.filter(|&d| d < len);
        self.after_drain = self.after_drain.filter(|&a| a < len);
        if !finished || keep >= len || self.repeat == RepeatMode::One {
            return;
        }
        match self.status {
            Status::Playing if self.output.is_some() => self.after_drain = Some(keep),
            Status::Stopped if self.output.is_none() => self.start(keep, 0),
            _ => {}
        }
    }

    fn move_track(&mut self, from: usize, to: usize) {
        let len = self.queue.len();
        if from >= len || to >= len || from == to {
            return;
        }
        let path = self.queue.remove(from);
        self.queue.insert(to, path);
        let remap = |i: usize| moved_index(i, from, to);
        let old_order = std::mem::take(&mut self.order);
        if self.shuffle {
            self.order = old_order.iter().map(|&i| remap(i)).collect();
        } else {
            // queue order: positions in `order` are queue indices
            self.order = (0..len).collect();
            let fix = |pos: usize| remap(old_order[pos]);
            for seg in &mut self.segments {
                seg.order_pos = fix(seg.order_pos);
            }
            self.decoding = self.decoding.map(fix);
            self.after_drain = self.after_drain.map(fix);
        }
    }

    fn play(&mut self) {
        match (&self.output, self.status) {
            (Some(out), Status::Paused) => {
                out.resume();
                self.status = Status::Playing;
            }
            _ if !self.queue.is_empty() => {
                let pos = self.current_pos().or(self.decoding).unwrap_or(0);
                // finished queue: start over
                let pos =
                    if self.status == Status::Stopped && self.output.is_none() && self.source_eof { 0 } else { pos };
                self.start(pos, 0);
            }
            _ => {}
        }
    }

    fn pause(&mut self) {
        if let Some(out) = &self.output {
            out.pause();
            self.meters.tap.clear();
            self.status = Status::Paused;
        }
    }

    fn close(&mut self) {
        self.meters.tap.clear();
        self.output = None;
        self.deck = None;
        self.pending.clear();
        self.decoded.clear();
        self.held.clear();
        self.fade = None;
        self.segments.clear();
        self.after_drain = None;
        self.pushed_frames = 0;
        self.source_eof = false;
    }

    /// Restarts the current track where it is (new device / device lost).
    fn restart_current(&mut self, default_device: bool) {
        if default_device {
            self.device = None;
        }
        if let Some(pos) = self.current_pos() {
            let ms = self.position_ms();
            let paused = self.status == Status::Paused;
            self.start(pos, ms);
            if paused {
                self.pause();
            }
        }
    }

    /// Starts `order[pos]` at `ms`, flushing whatever was buffered.
    fn start(&mut self, pos: usize, ms: u64) {
        self.close();
        let mut pos = pos;
        // skip unplayable files (at most one full lap)
        for _ in 0..self.queue.len().max(1) {
            match self.begin_track(pos, ms, true) {
                Ok(()) => {
                    self.status = Status::Playing;
                    return;
                }
                Err(e) => {
                    tracing::warn!("{e}");
                    self.error = Some(e.to_string());
                    match self.next_pos(Some(pos), true) {
                        Some(next) if next != pos => pos = next,
                        _ => break,
                    }
                }
            }
        }
        self.close();
        self.status = Status::Stopped;
    }

    /// Opens the file at `order[pos]`; reuses the output when possible
    /// (gapless) unless `reopen`.
    fn begin_track(&mut self, pos: usize, ms: u64, reopen: bool) -> Result<()> {
        let path = self.queue.get(self.order[pos]).cloned().ok_or_else(|| PlayerError::Open("empty queue".into()))?;
        let mut source = Source::open(&path)?;
        let start_ms = if ms > 0 { source.seek(ms).unwrap_or(0) } else { 0 };

        let device = find_device(self.device.as_deref())?;
        let target_rate = best_rate(&device, source.sample_rate);

        let reuse = !reopen
            && self
                .output
                .as_ref()
                .is_some_and(|o| o.rate == target_rate && o.device_name == device.name().unwrap_or_default());
        let sound = self.sound_of(&path);
        let mut tags_gain = None;
        let device_name = device.name().unwrap_or_else(|_| "output".into());
        let plan = self.plan_for(&path, &sound, &mut tags_gain, &device_name, target_rate);
        if !reuse {
            self.output = None;
            self.shared = Shared::new(self.volume, self.meters.clone());
            // from the first sample, without a glide
            self.shared.set_eq(plan.design);
            let output = Output::open(&device, target_rate, source.channels.max(1), self.shared.clone())?;
            self.meters.tap.set_rate(output.rate);
            self.output = Some(output);
            self.pushed_frames = 0;
            self.segments.clear();
        } else {
            // gapless: the track's filters glide in at its first frame
            self.shared.schedule_eq(self.written_frames(), plan.design);
        }
        let output = self.output.as_ref().expect("output opened");
        let deck = Deck::new(source, self.deck_format(output), 0)?;
        let info = self.stream_info(&deck, output);
        let start_frame = self.written_frames();
        self.segments.push_back(Segment {
            order_pos: pos,
            start_frame,
            eq_frame: start_frame,
            start_ms,
            skipped_frames: 0,
            duration_ms: deck.source().duration_ms,
            info,
            path,
            sound,
            tags_gain,
            plan,
        });
        self.decoding = Some(pos);
        self.deck = Some(deck);
        self.source_eof = false;
        Ok(())
    }

    fn sound_of(&self, path: &Path) -> TrackSound {
        self.sound_source.as_ref().map(|f| f(path)).unwrap_or_default()
    }

    fn deck_format(&self, output: &Output) -> DeckFormat {
        DeckFormat {
            rate: output.rate,
            channels: output.channels,
            surround: self.output_options.surround,
            quality: self.output_options.resampler,
        }
    }

    fn stream_info(&self, deck: &Deck, output: &Output) -> StreamInfo {
        let s = deck.source();
        let spread = output.channels > 2 && s.channels <= 2 && deck.remapped();
        StreamInfo {
            codec: s.codec.clone(),
            source_rate: s.sample_rate,
            bits_per_sample: s.bits_per_sample,
            source_channels: s.channels,
            output_rate: output.rate,
            output_channels: output.channels,
            resampled: deck.resampled(),
            device: output.device_name.clone(),
            surround: spread.then_some(self.output_options.surround),
            resampler: deck.resampled().then_some(self.output_options.resampler),
        }
    }

    /// Frames written so far: in the ring buffer or waiting for it.
    fn written_frames(&self) -> u64 {
        let channels = self.output.as_ref().map_or(1, |o| o.channels.max(1) as usize);
        self.pushed_frames + (self.pending.len() / channels) as u64
    }

    /// Crossfade on: the end of each track is decoded ahead.
    fn holding(&self) -> bool {
        self.crossfade.is_on() && self.output.is_some()
    }

    /// Samples to hold back (0 when not holding).
    fn hold_target(&self) -> usize {
        match &self.output {
            Some(o) if self.crossfade.is_on() => hold_frames(&self.crossfade, o.rate) * o.channels.max(1) as usize,
            _ => 0,
        }
    }

    /// What runs for a track on `device` at `rate`: its output profile, the
    /// taste rule for it and the loudness gain. Track change only, never per buffer.
    fn plan_for(
        &self,
        path: &Path,
        sound: &TrackSound,
        tags_gain: &mut Option<Option<ReplayGain>>,
        device: &str,
        rate: u32,
    ) -> SoundPlan {
        let loudness_db = if self.loudness.is_on() {
            let rg = match sound.replay_gain {
                Some(rg) if !rg.is_empty() => Some(rg),
                _ => *tags_gain.get_or_insert_with(|| dsp::read_replay_gain(path)),
            };
            let album_in_order = !self.shuffle && sound.context.source.as_ref().is_some_and(|s| s.is("album"));
            rg.map_or(0.0, |rg| self.loudness.gain_db(&rg, album_in_order))
        } else {
            0.0
        };
        dsp::sound_plan(&self.eq, &self.taste, device, &sound.context, loudness_db, rate)
    }

    /// The settings changed: every buffered track gets new filters (the one
    /// playing glides to them now, the next ones where they take over).
    fn replan(&mut self) {
        let Some(out) = &self.output else { return };
        let (device, rate) = (out.device_name.clone(), out.rate);
        let mut segments = std::mem::take(&mut self.segments);
        for seg in &mut segments {
            seg.plan = self.plan_for(&seg.path, &seg.sound, &mut seg.tags_gain, &device, rate);
        }
        self.shared.clear_scheduled();
        // the callback's filters over time: (first frame, design)
        let mut line: Vec<(u64, EqDesign)> = Vec::new();
        if let Some(f) = self.fade.as_mut() {
            if let Some(i) = segments.iter().position(|s| s.start_frame == f.start_frame && s.path == f.to) {
                f.gain_to = segments[i].plan.design.gain() as f32;
                if i > 0 {
                    f.from_design = segments[i - 1].plan.design;
                    f.gain_from = f.from_design.gain() as f32;
                } else {
                    // the outgoing track is gone from the list: its filters until the switch
                    line.push((f.start_frame, f.from_design));
                }
            }
        }
        line.extend(segments.iter().map(|s| (s.eq_frame, s.plan.design)));
        let played = self.shared.frames_played.load(Ordering::Relaxed);
        let now = line.iter().rposition(|(at, _)| *at <= played).unwrap_or(0);
        if let Some((_, design)) = line.get(now) {
            self.shared.set_eq(*design);
        }
        for (at, design) in line.iter().skip(now + 1) {
            self.shared.schedule_eq(*at, *design);
        }
        self.segments = segments;
    }

    /// Decodes into the ring buffer until it is full. With crossfade on, the
    /// newest seconds wait in `held` (decoded faster than real time, within
    /// [`LOOKAHEAD_BUDGET`] per pass, the ring buffer first) so a track's end
    /// is known before it is heard.
    fn fill(&mut self) {
        let started = Instant::now();
        loop {
            let target = self.hold_target();
            let Some(output) = self.output.as_mut() else { return };
            let channels = output.channels.max(1) as usize;

            if !self.pending.is_empty() {
                let pushed = output.producer.push_slice(&self.pending);
                self.pushed_frames += (pushed / channels) as u64;
                self.pending.drain(..pushed);
                if !self.pending.is_empty() {
                    // ring full: spare time reads further ahead for the crossfade
                    let grow = self.holding()
                        && self.deck.is_some()
                        && self.held.len() < target
                        && started.elapsed() < LOOKAHEAD_BUDGET;
                    if grow && self.decode_step() {
                        continue;
                    }
                    return; // buffer full
                }
            }

            if self.source_eof {
                if output.producer.occupied_len() > 0 {
                    return; // still playing the tail
                }
                match self.after_drain.take() {
                    // the next track needs another output config
                    Some(pos) => {
                        self.start(pos, 0);
                        continue;
                    }
                    // end of the queue and everything was heard
                    None => {
                        self.output = None;
                        self.status = Status::Stopped;
                        self.publish();
                        return;
                    }
                }
            }

            if !self.held.is_empty() {
                let excess = self.held.len().saturating_sub(target);
                let take = if excess > 0 {
                    excess
                } else {
                    let low = output.producer.occupied_len() < output.producer.capacity().get() / 2;
                    let over = started.elapsed() >= LOOKAHEAD_BUDGET;
                    if target == 0 || low || over || self.deck.is_none() {
                        let free = output.producer.vacant_len() / channels * channels;
                        if free == 0 {
                            return;
                        }
                        self.held.len().min(free)
                    } else {
                        0 // read further ahead first
                    }
                };
                if take > 0 {
                    self.pending.extend(self.held.drain(..take));
                    continue;
                }
            }

            if !self.decode_step() {
                return;
            }
        }
    }

    /// Decodes one chunk of the deck (mixing the crossfade in). `false` when
    /// there is nothing to decode.
    fn decode_step(&mut self) -> bool {
        let Some(deck) = self.deck.as_mut() else { return false };
        self.decoded.clear();
        let more = match deck.read(&mut self.decoded) {
            Ok(more) => more,
            Err(e) => {
                tracing::warn!("{e}");
                self.error = Some(e.to_string());
                false
            }
        };
        let skipped = deck.take_skipped();
        if skipped > 0 {
            if let Some(seg) = self.segments.back_mut() {
                seg.skipped_frames += skipped;
            }
        }
        if let Some(fade) = self.fade.as_mut() {
            fade.mix(&mut self.decoded);
        }
        self.commit_decoded();
        if !more {
            self.end_of_deck();
        }
        true
    }

    /// The last chunk goes behind what is held back, or straight to the ring.
    fn commit_decoded(&mut self) {
        if self.holding() {
            self.held.extend(self.decoded.drain(..));
        } else {
            self.pending.append(&mut self.decoded);
        }
    }

    /// The track being decoded ended: crossfade into the next one, or queue
    /// it gapless / after the buffer drains.
    fn end_of_deck(&mut self) {
        self.deck = None;
        if let Some(fade) = self.fade.as_mut() {
            if fade.mixing() {
                // shorter than its fade-in: the outgoing track finishes alone
                self.decoded.clear();
                fade.finish_alone(&mut self.decoded);
                self.commit_decoded();
            }
        }
        if self.holding() && self.try_crossfade() {
            return;
        }
        self.pending.extend(self.held.drain(..));
        self.on_source_end();
    }

    /// The natural end of a track: fade into the next one when the settings
    /// say so and it can be opened now (a stream still on its way, an
    /// unreadable file: a normal transition instead).
    fn try_crossfade(&mut self) -> bool {
        let Some(cur) = self.decoding else { return false };
        let Some(next) = self.next_pos(Some(cur), true) else { return false };
        let Some(path) = self.order.get(next).and_then(|&i| self.queue.get(i)).cloned() else { return false };
        let source = Source::open(&path).ok();
        let sound = self.sound_of(&path);
        let Some(from) = self.segments.back() else { return false };
        let to = source.as_ref().map(|s| TrackInfo::of(&sound.context, s.duration_ms));
        let from_info = TrackInfo::of(&from.sound.context, from.duration_ms);
        if !wants_fade(&self.crossfade, &from_info, to.as_ref(), self.shuffle, self.repeat, false) {
            return false;
        }
        let Some(source) = source else { return false };
        let tail: Vec<f32> = self.held.drain(..).collect();
        self.begin_fade(next, path, source, sound, tail, false)
    }

    /// A skip or a pick while playing: a short fade when the settings allow
    /// it, else a hard start.
    fn skip_to(&mut self, pos: usize) {
        if !self.manual_fade(pos) {
            self.start(pos, 0);
        }
    }

    fn manual_fade(&mut self, pos: usize) -> bool {
        let ready = self.crossfade.is_on()
            && self.crossfade.on_manual_skip
            && self.status == Status::Playing
            && !self.fade.as_ref().is_some_and(Fade::mixing)
            && self.after_drain.is_none()
            && !self.source_eof
            && self.segments.len() == 1
            && self.deck.is_some();
        let Some(out) = self.output.as_ref().filter(|_| ready) else { return false };
        let (rate, channels) = (out.rate as f32, out.channels.max(1) as usize);
        let Some(path) = self.order.get(pos).and_then(|&i| self.queue.get(i)).cloned() else { return false };
        let Ok(source) = Source::open(&path) else { return false };
        let sound = self.sound_of(&path);
        let Some(from) = self.segments.back() else { return false };
        let to = TrackInfo::of(&sound.context, source.duration_ms);
        let from_info = TrackInfo::of(&from.sound.context, from.duration_ms);
        if !wants_fade(&self.crossfade, &from_info, Some(&to), self.shuffle, self.repeat, true) {
            return false;
        }
        // the outgoing track fades from what comes right after the samples already queued
        let want = (self.crossfade.fade_seconds(true) * rate) as usize * channels;
        let mut tail: Vec<f32> = Vec::with_capacity(want);
        tail.extend(self.held.drain(..want.min(self.held.len())));
        self.held.clear();
        if let Some(deck) = self.deck.as_mut() {
            while tail.len() < want {
                self.decoded.clear();
                let more = deck.read(&mut self.decoded).unwrap_or(false);
                tail.extend_from_slice(&self.decoded);
                if !more {
                    break;
                }
            }
        }
        self.decoded.clear();
        tail.truncate(want);
        self.deck = None;
        self.begin_fade(pos, path, source, sound, tail, true)
    }

    /// Starts mixing `order[pos]` over `tail` (the outgoing track's last
    /// samples). On failure the tail goes to the ring as is and it returns
    /// `false` (the caller makes a normal transition).
    fn begin_fade(
        &mut self,
        pos: usize,
        path: PathBuf,
        source: Source,
        sound: TrackSound,
        mut tail: Vec<f32>,
        manual: bool,
    ) -> bool {
        let Some(out) = self.output.as_ref() else {
            self.pending.extend_from_slice(&tail);
            return false;
        };
        let (rate, channels) = (out.rate, out.channels.max(1) as usize);
        let format = self.deck_format(out);
        let device = out.device_name.clone();
        // smart: the fade ends where the outgoing music does
        let end = if self.crossfade.smart && !manual { music_end(&tail, channels) } else { tail.len() };
        let Some(frames) = fade_frames(&self.crossfade, manual, end / channels, rate) else {
            self.pending.extend_from_slice(&tail);
            return false;
        };
        let lead = if self.crossfade.smart { (MAX_TRIM_SECONDS * rate as f32) as u64 } else { 0 };
        let deck = match Deck::new(source, format, lead) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("crossfade: {e}");
                self.pending.extend_from_slice(&tail);
                return false;
            }
        };
        tail.truncate(end);
        let split = end - frames * channels;
        self.pending.extend_from_slice(&tail[..split]);
        tail.drain(..split);

        let start_frame = self.written_frames();
        let mut tags_gain = None;
        let plan = self.plan_for(&path, &sound, &mut tags_gain, &device, rate);
        let (from_design, from_path) =
            self.segments.back().map_or((EqDesign::BYPASS, PathBuf::new()), |s| (s.plan.design, s.path.clone()));
        let fade = Fade::new(
            tail,
            channels,
            FadeSpec {
                curve: self.crossfade.curve,
                start_frame,
                glide: dsp::glide_frames(rate) as usize,
                from_design,
                gain_to: plan.design.gain() as f32,
                from: from_path,
                to: path.clone(),
                manual,
            },
        );
        // the incoming track's filters take over halfway, where both are at half power
        self.shared.schedule_eq(fade.switch_frame(), plan.design);
        let info = match self.output.as_ref() {
            Some(out) => self.stream_info(&deck, out),
            None => return false,
        };
        self.segments.push_back(Segment {
            order_pos: pos,
            start_frame,
            eq_frame: fade.switch_frame(),
            start_ms: 0,
            skipped_frames: 0,
            duration_ms: deck.source().duration_ms,
            info,
            path,
            sound,
            tags_gain,
            plan,
        });
        self.fade = Some(fade);
        self.deck = Some(deck);
        self.decoding = Some(pos);
        self.source_eof = false;
        self.after_drain = None;
        true
    }

    /// Queues the next track right behind the current one (gapless) or
    /// after the buffer drains when it needs another output config.
    fn on_source_end(&mut self) {
        self.deck = None;
        let next = self.decoding.and_then(|p| self.next_pos(Some(p), true));
        let Some(pos) = next else {
            self.source_eof = true;
            return;
        };

        let path = self.queue[self.order[pos]].clone();
        let rate = Source::open(&path).map(|s| s.sample_rate).ok();
        let same_rate = rate.is_some()
            && self.output.as_ref().is_some_and(|o| {
                let device = find_device(self.device.as_deref()).ok();
                let target = match (&device, rate) {
                    (Some(d), Some(r)) => best_rate(d, r),
                    _ => 0,
                };
                o.rate == target
            });

        if same_rate {
            if let Err(e) = self.begin_track(pos, 0, false) {
                self.error = Some(e.to_string());
                self.source_eof = true;
            }
        } else {
            self.source_eof = true;
            self.after_drain = Some(pos);
        }
    }

    /// Drops segments that finished playing. Returns `true` if the audible track changed.
    fn advance_segments(&mut self) -> bool {
        let played = self.shared.frames_played.load(Ordering::Relaxed);
        let mut changed = false;
        while self.segments.len() > 1 && self.segments[1].start_frame <= played {
            self.segments.pop_front();
            changed = true;
        }
        if self.fade.as_ref().is_some_and(|f| f.heard(played)) {
            self.fade = None;
            changed = true;
        }
        changed
    }

    fn current_pos(&self) -> Option<usize> {
        self.segments.front().map(|s| s.order_pos).or(self.decoding)
    }

    fn position_ms(&self) -> u64 {
        let Some(seg) = self.segments.front() else { return 0 };
        let Some(rate) = self.output.as_ref().map(|o| o.rate as u64) else { return seg.start_ms };
        let played = self.shared.frames_played.load(Ordering::Relaxed);
        seg.start_ms + (played.saturating_sub(seg.start_frame) + seg.skipped_frames) * 1000 / rate.max(1)
    }

    /// Next position in `order`. `auto` = natural end of a track (honours repeat-one).
    fn next_pos(&self, current: Option<usize>, auto: bool) -> Option<usize> {
        let len = self.order.len();
        if len == 0 {
            return None;
        }
        let Some(cur) = current else { return Some(0) };
        if auto && self.repeat == RepeatMode::One {
            return Some(cur);
        }
        if cur + 1 < len {
            Some(cur + 1)
        } else if self.repeat == RepeatMode::All || !auto && self.repeat == RepeatMode::One {
            Some(0)
        } else {
            None
        }
    }

    fn prev_pos(&self, current: usize) -> Option<usize> {
        if current > 0 {
            Some(current - 1)
        } else if self.repeat == RepeatMode::All {
            self.order.len().checked_sub(1)
        } else {
            Some(0)
        }
    }

    fn publish(&mut self) {
        let seg = self.segments.front();
        let plan = seg.filter(|_| self.output.is_some()).map(|s| &s.plan);
        let played = self.shared.frames_played.load(Ordering::Relaxed);
        let rate = self.output.as_ref().map_or(48_000.0, |o| o.rate as f32);
        let fading = self.fade.as_ref().filter(|f| self.output.is_some() && played >= f.start_frame).map(|f| Fading {
            from: f.from.clone(),
            to: f.to.clone(),
            progress: f.progress(played),
            seconds: f.len as f32 / rate,
            manual: f.manual,
        });
        let snapshot = Snapshot {
            status: self.status,
            queue: self.queue.clone(),
            current: self.current_pos().and_then(|p| self.order.get(p).copied()),
            position_ms: self.position_ms(),
            duration_ms: seg.and_then(|s| s.duration_ms),
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat,
            preferred_device: self.device.clone(),
            stream: seg.map(|s| s.info.clone()),
            error: self.error.clone(),
            eq_active: self.output.is_some() && plan.is_some_and(|p| p.eq_active),
            eq_profile: plan.and_then(|p| p.profile.clone()),
            taste_rule: plan.and_then(|p| p.taste_rule.as_ref().map(|r| r.id.clone())),
            taste_label: plan.and_then(|p| p.taste_rule.as_ref().map(|r| r.label.clone())),
            loudness_db: plan.map_or(0.0, |p| p.loudness_db),
            fading,
        };
        let changed = self.published.lock().map(|mut p| {
            let changed = *p != snapshot;
            *p = snapshot.clone();
            changed
        });
        if changed.unwrap_or(true) {
            (self.listener)(&snapshot);
        }
    }
}

/// Rate to open `device` at for a `source` Hz file: the file's own rate when
/// supported (no resampling), else the highest supported rate of the same
/// family (176.4 kHz → 88.2 kHz, 192 kHz → 96 kHz), else the highest below it.
fn best_rate(device: &cpal::Device, source: u32) -> u32 {
    let ranges: Vec<_> = device.supported_output_configs().map(|it| it.collect()).unwrap_or_default();
    let supported = |r: u32| ranges.iter().any(|c| c.min_sample_rate().0 <= r && r <= c.max_sample_rate().0);
    if supported(source) {
        return source;
    }
    let candidates =
        [8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000];
    let rates: Vec<u32> = candidates.into_iter().filter(|r| supported(*r)).collect();
    pick_rate(&rates, source)
        .or_else(|| device.default_output_config().ok().map(|c| c.sample_rate().0))
        .unwrap_or(48_000)
}

/// The queue after [`Player::set_upcoming`]: what was played (up to the current
/// track), then the next track if its audio is already buffered and the new list
/// doesn't start with it, then `tracks`.
fn upcoming_queue(played: Vec<PathBuf>, decoded: Option<PathBuf>, tracks: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut queue = played;
    if let Some(next) = decoded.filter(|d| tracks.first() != Some(d)) {
        queue.push(next);
    }
    queue.extend(tracks);
    queue
}

/// Frames of look-ahead a crossfade needs at `rate`: the fade, the trailing
/// silence the smart trim may skip, and a margin.
fn hold_frames(settings: &CrossfadeSettings, rate: u32) -> usize {
    let trim = if settings.smart { MAX_TRIM_SECONDS } else { 0.0 };
    ((settings.fade_seconds(false) + trim + HOLD_MARGIN_SECONDS) * rate as f32) as usize
}

/// Whether going from `from` to `to` crossfades. `to` is `None` when the next
/// track isn't ready (a stream still downloading, a file that can't be
/// opened): a normal transition then. Repeat-one loops stay gapless.
fn wants_fade(
    settings: &CrossfadeSettings,
    from: &TrackInfo,
    to: Option<&TrackInfo>,
    shuffle: bool,
    repeat: RepeatMode,
    manual: bool,
) -> bool {
    let Some(to) = to else { return false };
    if !manual && repeat == RepeatMode::One {
        return false;
    }
    settings.fades(from, to, shuffle, manual)
}

/// Frames the fade lasts when `music_frames` of the outgoing track are left
/// to fade over: the setting, shortened to what there is, `None` when too
/// little is left.
fn fade_frames(settings: &CrossfadeSettings, manual: bool, music_frames: usize, rate: u32) -> Option<usize> {
    let wanted = (settings.fade_seconds(manual) * rate as f32) as usize;
    let frames = wanted.min(music_frames);
    (frames as f32 >= MIN_FADE_SECONDS * rate as f32).then_some(frames)
}

fn pick_rate(supported: &[u32], source: u32) -> Option<u32> {
    let family = |r: u32| r % 11_025 == 0;
    supported
        .iter()
        .copied()
        .filter(|r| *r <= source && family(*r) == family(source))
        .max()
        .or_else(|| supported.iter().copied().filter(|r| *r <= source).max())
        .or_else(|| supported.iter().copied().min())
}

#[cfg(test)]
mod tests {
    #[test]
    fn moved_index_matches_vec_move() {
        for (from, to) in [(0, 3), (3, 0), (1, 2), (2, 1), (4, 4)] {
            let mut v: Vec<usize> = (0..5).collect();
            let x = v.remove(from);
            v.insert(to, x);
            for old in 0..5 {
                assert_eq!(v[moved_index(old, from, to)], old, "from {from} to {to}");
            }
        }
    }

    use super::*;

    #[test]
    fn upcoming_replaces_what_comes_next() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(upcoming_queue(vec![p("a"), p("b")], None, vec![p("x"), p("y")]), [p("a"), p("b"), p("x"), p("y")]);
        // the buffered next track is heard anyway: it stays, unless it is the new next
        assert_eq!(upcoming_queue(vec![p("a")], Some(p("b")), vec![p("x")]), [p("a"), p("b"), p("x")]);
        assert_eq!(upcoming_queue(vec![p("a")], Some(p("x")), vec![p("x")]), [p("a"), p("x")]);
        assert_eq!(upcoming_queue(vec![], None, vec![p("x")]), [p("x")]);
    }

    #[test]
    fn rate_choice_keeps_the_family() {
        let jack = [44_100, 48_000, 88_200, 96_000];
        assert_eq!(pick_rate(&jack, 192_000), Some(96_000));
        assert_eq!(pick_rate(&jack, 176_400), Some(88_200));
        assert_eq!(pick_rate(&[44_100], 96_000), Some(44_100));
        assert_eq!(pick_rate(&[48_000], 44_100), Some(48_000));
        assert_eq!(pick_rate(&[], 44_100), None);
    }

    use homefi_core::dsp::crossfade::CrossfadeMode;
    use homefi_core::dsp::ItemRef;

    fn on() -> CrossfadeSettings {
        CrossfadeSettings { mode: CrossfadeMode::On, ..Default::default() }
    }

    #[test]
    fn look_ahead_covers_the_fade_and_the_trim() {
        // 6 s fade + 10 s of silence that may be trimmed + 0.5 s margin
        assert_eq!(hold_frames(&on(), 48_000), (16.5 * 48_000.0) as usize);
        let plain = CrossfadeSettings { smart: false, seconds: 3.0, ..on() };
        assert_eq!(hold_frames(&plain, 44_100), (3.5 * 44_100.0) as usize);
    }

    #[test]
    fn fade_length_follows_what_is_left() {
        let s = on();
        assert_eq!(fade_frames(&s, false, 1_000_000, 48_000), Some(6 * 48_000));
        // the outgoing music ends sooner (short track, slow decoding): a shorter fade
        assert_eq!(fade_frames(&s, false, 48_000, 48_000), Some(48_000));
        // almost nothing left: a normal transition
        assert_eq!(fade_frames(&s, false, 1000, 48_000), None);
        // a skip: the short fade
        assert_eq!(fade_frames(&s, true, 1_000_000, 48_000), Some(72_000));
    }

    #[test]
    fn falls_back_to_a_normal_transition_when_the_next_track_is_not_ready() {
        let a = ItemRef::named("A");
        let b = ItemRef::named("B");
        let from = TrackInfo { album: Some(&a), artist: None, duration_ms: Some(200_000) };
        let to = TrackInfo { album: Some(&b), artist: None, duration_ms: Some(200_000) };
        assert!(wants_fade(&on(), &from, Some(&to), false, RepeatMode::Off, false));
        assert!(!wants_fade(&on(), &from, None, false, RepeatMode::Off, false), "not downloaded yet");
        assert!(!wants_fade(&on(), &from, Some(&to), false, RepeatMode::One, false), "repeat one loops gapless");
        assert!(wants_fade(&on(), &from, Some(&to), false, RepeatMode::One, true), "a skip still fades");
        // the same album in order stays gapless
        let next = TrackInfo { album: Some(&a), ..to };
        assert!(!wants_fade(&on(), &from, Some(&next), false, RepeatMode::All, false));
        assert!(!wants_fade(&CrossfadeSettings::default(), &from, Some(&to), false, RepeatMode::Off, false));
    }
}
