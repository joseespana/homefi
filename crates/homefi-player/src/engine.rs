//! The audio thread: queue, decoding, output (re)configuration and events.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::DeviceTrait;
use rand::seq::SliceRandom;
use ringbuf::traits::{Observer, Producer};
use rubato::{Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use serde::Serialize;

use crate::decoder::Source;
use crate::error::{PlayerError, Result};
use crate::output::{find_device, Meters, Output, Shared};

/// How often position updates are emitted while playing.
const TICK: Duration = Duration::from_millis(250);
/// Resampler input chunk (frames).
const RESAMPLE_CHUNK: usize = 1024;
/// "Previous" restarts the track instead when past this point.
const RESTART_THRESHOLD_MS: u64 = 3000;

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
    Stop,
    Shutdown,
}

pub type Listener = Arc<dyn Fn(&Snapshot) + Send + Sync>;

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
}

impl Drop for Player {
    fn drop(&mut self) {
        // last handle gone: stop the thread
        if Arc::strong_count(&self.snapshot) <= 2 {
            let _ = self.tx.send(Command::Shutdown);
        }
    }
}

/// Converts between sample rates with a high quality sinc filter.
struct Resampler {
    inner: SincFixedIn<f32>,
    channels: usize,
    input: Vec<Vec<f32>>,
}

impl Resampler {
    fn new(from: u32, to: u32, channels: usize) -> Result<Self> {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let inner = SincFixedIn::new(to as f64 / from as f64, 1.0, params, RESAMPLE_CHUNK, channels)
            .map_err(|e| PlayerError::Resample(e.to_string()))?;
        Ok(Self { inner, channels, input: vec![Vec::new(); channels] })
    }

    /// Feeds interleaved samples, appends resampled interleaved samples to `out`.
    fn process(&mut self, interleaved: &[f32], out: &mut Vec<f32>) -> Result<()> {
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
    fn flush(&mut self, out: &mut Vec<f32>) -> Result<()> {
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

/// Maps `src` channels onto `dst` (mono → both sides, extra channels dropped).
fn map_channels(samples: &[f32], src: u16, dst: u16, out: &mut Vec<f32>) {
    let (src, dst) = (src as usize, dst as usize);
    if src == dst {
        out.extend_from_slice(samples);
        return;
    }
    for frame in samples.chunks_exact(src) {
        for ch in 0..dst {
            out.push(if src == 1 { frame[0] } else { frame.get(ch).copied().unwrap_or(0.0) });
        }
    }
}

/// A track whose samples were pushed starting at `start_frame`.
struct Segment {
    order_pos: usize,
    start_frame: u64,
    start_ms: u64,
    duration_ms: Option<u64>,
    info: StreamInfo,
}

struct Engine {
    rx: Receiver<Command>,
    listener: Listener,
    published: Arc<Mutex<Snapshot>>,

    queue: Vec<PathBuf>,
    order: Vec<usize>,
    /// Position in `order` of the track being decoded.
    decoding: Option<usize>,

    source: Option<Source>,
    source_eof: bool,
    resampler: Option<Resampler>,
    output: Option<Output>,
    shared: Arc<Shared>,
    meters: Arc<Meters>,
    pushed_frames: u64,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    segments: VecDeque<Segment>,
    /// Next track that needs a different output config: started once the buffer drains.
    after_drain: Option<usize>,

    status: Status,
    volume: f32,
    device: Option<String>,
    shuffle: bool,
    repeat: RepeatMode,
    error: Option<String>,
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
            source: None,
            source_eof: false,
            resampler: None,
            output: None,
            shared: Shared::new(volume, meters.clone()),
            meters,
            pushed_frames: 0,
            pending: Vec::new(),
            scratch: Vec::new(),
            segments: VecDeque::new(),
            after_drain: None,
            status: Status::Stopped,
            volume,
            device,
            shuffle: false,
            repeat: RepeatMode::Off,
            error: None,
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
                    self.start(pos, 0);
                }
            }
            Command::Previous => {
                let current = self.current_pos();
                if self.position_ms() > RESTART_THRESHOLD_MS || current == Some(0) && self.repeat != RepeatMode::All {
                    if let Some(pos) = current {
                        self.start(pos, 0);
                    }
                } else if let Some(pos) = current.and_then(|p| self.prev_pos(p)) {
                    self.start(pos, 0);
                }
            }
            Command::Jump(index) => {
                if let Some(pos) = self.order.iter().position(|&i| i == index) {
                    self.start(pos, 0);
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
            }
            Command::Repeat(mode) => self.repeat = mode,
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
        self.source = None;
        self.resampler = None;
        self.pending.clear();
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
        if !reuse {
            self.output = None;
            self.shared = Shared::new(self.volume, self.meters.clone());
            let output = Output::open(&device, target_rate, source.channels.max(1), self.shared.clone())?;
            self.meters.tap.set_rate(output.rate);
            self.output = Some(output);
            self.pushed_frames = 0;
            self.segments.clear();
        }
        let output = self.output.as_ref().expect("output opened");

        self.resampler = if source.sample_rate != output.rate {
            Some(Resampler::new(source.sample_rate, output.rate, output.channels as usize)?)
        } else {
            None
        };

        let info = StreamInfo {
            codec: source.codec.clone(),
            source_rate: source.sample_rate,
            bits_per_sample: source.bits_per_sample,
            source_channels: source.channels,
            output_rate: output.rate,
            output_channels: output.channels,
            resampled: self.resampler.is_some(),
            device: output.device_name.clone(),
        };
        self.segments.push_back(Segment {
            order_pos: pos,
            start_frame: self.pushed_frames,
            start_ms,
            duration_ms: source.duration_ms,
            info,
        });
        self.decoding = Some(pos);
        self.source = Some(source);
        self.source_eof = false;
        Ok(())
    }

    /// Decodes into the ring buffer until it is full.
    fn fill(&mut self) {
        loop {
            let Some(output) = self.output.as_mut() else { return };
            let channels = output.channels as usize;

            if !self.pending.is_empty() {
                let pushed = output.producer.push_slice(&self.pending);
                self.pushed_frames += (pushed / channels) as u64;
                self.pending.drain(..pushed);
                if !self.pending.is_empty() {
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

            let Some(source) = self.source.as_mut() else { return };
            let src_channels = source.channels;
            match source.next_chunk() {
                Ok(Some(samples)) => {
                    self.scratch.clear();
                    map_channels(samples, src_channels, channels as u16, &mut self.scratch);
                    match &mut self.resampler {
                        Some(r) => {
                            if let Err(e) = r.process(&self.scratch, &mut self.pending) {
                                self.error = Some(e.to_string());
                                self.source_eof = true;
                            }
                        }
                        None => self.pending.extend_from_slice(&self.scratch),
                    }
                }
                Ok(None) => {
                    if let Some(r) = &mut self.resampler {
                        let _ = r.flush(&mut self.pending);
                    }
                    self.on_source_end();
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    self.on_source_end();
                }
            }
        }
    }

    /// Queues the next track right behind the current one (gapless) or
    /// after the buffer drains when it needs another output config.
    fn on_source_end(&mut self) {
        self.source = None;
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
        changed
    }

    fn current_pos(&self) -> Option<usize> {
        self.segments.front().map(|s| s.order_pos).or(self.decoding)
    }

    fn position_ms(&self) -> u64 {
        let Some(seg) = self.segments.front() else { return 0 };
        let Some(rate) = self.output.as_ref().map(|o| o.rate as u64) else { return seg.start_ms };
        let played = self.shared.frames_played.load(Ordering::Relaxed);
        seg.start_ms + played.saturating_sub(seg.start_frame) * 1000 / rate.max(1)
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
    fn channel_mapping() {
        let mut out = Vec::new();
        map_channels(&[0.1, 0.2], 1, 2, &mut out);
        assert_eq!(out, [0.1, 0.1, 0.2, 0.2]);

        out.clear();
        map_channels(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, 2, &mut out);
        assert_eq!(out, [1.0, 2.0, 4.0, 5.0]);
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

    #[test]
    fn resampler_changes_length_by_ratio() {
        let mut r = Resampler::new(44_100, 48_000, 2).unwrap();
        let input = vec![0.25f32; 44_100 * 2];
        let mut out = Vec::new();
        r.process(&input, &mut out).unwrap();
        r.flush(&mut out).unwrap();
        let frames = out.len() / 2;
        assert!((47_000..=49_200).contains(&frames), "{frames}");
    }
}
