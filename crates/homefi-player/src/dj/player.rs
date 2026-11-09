//! [`DjPlayer`]: the handle the app holds while DJ mode is on. It owns the audio stream (on a
//! thread of its own: cpal streams must stay where they were made), sends commands to the mixer
//! through a lock-free ring, loads songs on helper threads and assembles snapshots.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, SupportedBufferSize};
use homefi_core::dj::{AnalysisCache, DjAnalysis, DjSettings, DECK_COUNT};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::control::{
    AutoMixSnapshot, DeckId, DeckSnapshot, DjControl, DjEvent, DjOutputInfo, DjSnapshot, DjTrackInfo,
};
use super::deck::DeckAudio;
use super::error::DjError;
use super::loader::load_track;
use super::mixer::{Command, DeckView, DjMixer, DjShared};
use crate::output::{device_queries, find_device};

/// Commands waiting for the callback at most.
const COMMAND_RING: usize = 1024;
/// Replaced songs waiting to be freed.
const TRASH_RING: usize = 16;
/// Largest callback the scratch buffer is sized for (frames); bigger ones are rendered in pieces.
const MAX_CALLBACK_FRAMES: usize = 4096;
const BYTES_PER_MB: u64 = 1024 * 1024;

#[derive(Default)]
struct DeckSlot {
    track: Option<(DjTrackInfo, Arc<DjAnalysis>)>,
    loading: bool,
    error: Option<String>,
    /// Bumped by every load and eject: an older load that finishes late is discarded.
    generation: u64,
}

struct Inner {
    commands: Mutex<HeapProd<Command>>,
    trash: Mutex<HeapCons<Arc<DeckAudio>>>,
    shared: Arc<DjShared>,
    settings: Mutex<DjSettings>,
    slots: Mutex<[DeckSlot; DECK_COUNT]>,
    output: DjOutputInfo,
    failed: Arc<AtomicBool>,
    lost_reported: AtomicBool,
    events: Box<dyn Fn(DjEvent) + Send + Sync>,
    cache: AnalysisCache,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    /// Frees the songs the callback has let go of.
    fn reap(&self) {
        let mut trash = lock(&self.trash);
        while trash.try_pop().is_some() {}
    }

    fn send(&self, command: Command) {
        self.reap();
        if lock(&self.commands).try_push(command).is_err() {
            tracing::warn!("DJ command ring full: a command was dropped");
        }
    }

    fn generation(&self, deck: usize) -> u64 {
        lock(&self.slots)[deck].generation
    }

    fn used_bytes(slots: &[DeckSlot; DECK_COUNT]) -> u64 {
        slots.iter().filter_map(|s| s.track.as_ref()).map(|(i, _)| i.bytes).sum()
    }
}

/// DJ mode's audio engine. Dropping it (or [`DjPlayer::stop`]) closes the stream.
pub struct DjPlayer {
    inner: Arc<Inner>,
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

struct Opened {
    commands: HeapProd<Command>,
    trash: HeapCons<Arc<DeckAudio>>,
    info: DjOutputInfo,
}

impl DjPlayer {
    /// Opens `device` (the system default when `None` or missing) with the buffer of
    /// `settings.latency` and starts the mixer. `events` is called from helper threads (loads) and
    /// must be quick.
    pub fn start(
        settings: &DjSettings,
        device: Option<&str>,
        events: impl Fn(DjEvent) + Send + Sync + 'static,
    ) -> Result<Self, DjError> {
        let settings = settings.clamped();
        let shared = Arc::new(DjShared::default());
        let failed = Arc::new(AtomicBool::new(false));
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<Opened, DjError>>();

        let thread = {
            let (shared, failed, settings, device) =
                (shared.clone(), failed.clone(), settings.clone(), device.map(str::to_owned));
            std::thread::Builder::new()
                .name("homefi-dj-audio".into())
                .spawn(move || {
                    // the stream lives here until we are told to stop
                    let stream = match open(device.as_deref(), &settings, shared, failed) {
                        Ok((stream, opened)) => {
                            let _ = ready_tx.send(Ok(opened));
                            stream
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                            return;
                        }
                    };
                    let _ = stop_rx.recv();
                    drop(stream);
                })
                .map_err(|e| DjError::Output(e.to_string()))?
        };
        let opened =
            ready_rx.recv().map_err(|_| DjError::Output("the audio thread ended unexpectedly".into())).and_then(|r| r);
        let opened = match opened {
            Ok(o) => o,
            Err(e) => {
                let _ = thread.join();
                return Err(e);
            }
        };

        let inner = Arc::new(Inner {
            commands: Mutex::new(opened.commands),
            trash: Mutex::new(opened.trash),
            shared,
            settings: Mutex::new(settings),
            slots: Mutex::new(Default::default()),
            output: opened.info,
            failed,
            lost_reported: AtomicBool::new(false),
            events: Box::new(events),
            cache: AnalysisCache::in_app_dir(),
        });
        Ok(Self { inner, stop: Some(stop_tx), thread: Some(thread) })
    }

    /// Closes the stream and frees the decks.
    pub fn stop(self) {
        drop(self);
    }

    pub fn output(&self) -> &DjOutputInfo {
        &self.inner.output
    }

    pub fn settings(&self) -> DjSettings {
        lock(&self.inner.settings).clone()
    }

    /// Applies new settings. Everything takes effect at once except the buffer size
    /// (`latency`), which needs DJ mode to be restarted, and `memory_mb`, read by the next load.
    pub fn set_settings(&self, settings: &DjSettings) {
        let settings = settings.clamped();
        self.inner.send(Command::Config(settings.mixer()));
        *lock(&self.inner.settings) = settings;
    }

    pub fn control(&self, control: DjControl) {
        if let DjControl::Eject { deck } = control {
            let mut slots = lock(&self.inner.slots);
            let slot = &mut slots[deck.index()];
            // a load in flight is dropped
            let generation = slot.generation + 1;
            *slot = DeckSlot { generation, ..Default::default() };
        }
        self.inner.send(Command::Control(control));
    }

    /// Loads `path` onto `deck` in the background; the result arrives as a [`DjEvent`] and in the
    /// next snapshot. The deck keeps playing its current song until the new one is ready.
    pub fn load(&self, deck: DeckId, path: PathBuf, title: String) {
        let inner = self.inner.clone();
        let i = deck.index();
        let limit_mb = u64::from(lock(&inner.settings).memory_mb);
        let (generation, available) = {
            let mut slots = lock(&inner.slots);
            slots[i].generation += 1;
            slots[i].loading = true;
            slots[i].error = None;
            (slots[i].generation, (limit_mb * BYTES_PER_MB).saturating_sub(Inner::used_bytes(&slots)))
        };
        (inner.events)(DjEvent::Loading { deck });

        let spawned = std::thread::Builder::new().name("homefi-dj-load".into()).spawn({
            let inner = inner.clone();
            move || {
                let stale = || inner.generation(i) != generation;
                let result = load_track(&path, &title, available, limit_mb, Some(&inner.cache), &stale);
                if stale() {
                    return;
                }
                match result {
                    Ok(loaded) => {
                        {
                            let mut slots = lock(&inner.slots);
                            slots[i].track = Some((loaded.info.clone(), loaded.analysis));
                            slots[i].loading = false;
                        }
                        inner.send(Command::Load { deck: i, audio: Arc::new(loaded.audio) });
                        (inner.events)(DjEvent::Loaded { deck, track: loaded.info });
                    }
                    Err(e) => {
                        let message = e.to_string();
                        {
                            let mut slots = lock(&inner.slots);
                            slots[i].loading = false;
                            slots[i].error = Some(message.clone());
                        }
                        (inner.events)(DjEvent::LoadFailed { deck, message });
                    }
                }
            }
        });
        if spawned.is_err() {
            let mut slots = lock(&inner.slots);
            slots[i].loading = false;
            slots[i].error = Some("could not start loading the song".into());
        }
    }

    /// The analysis (overview, beat grid) of the song on `deck`.
    pub fn analysis(&self, deck: DeckId) -> Option<Arc<DjAnalysis>> {
        lock(&self.inner.slots)[deck.index()].track.as_ref().map(|(_, a)| a.clone())
    }

    /// The state for the screen. Peaks are those since the previous snapshot.
    pub fn snapshot(&self) -> DjSnapshot {
        self.inner.reap();
        let view = self.inner.shared.take();
        let settings = lock(&self.inner.settings).clone();
        let slots = lock(&self.inner.slots);
        let lost = self.inner.failed.load(Ordering::Relaxed);
        let device_lost = lost.then(|| "the audio device stopped: restart DJ mode".to_owned());
        if lost && !self.inner.lost_reported.swap(true, Ordering::Relaxed) {
            (self.inner.events)(DjEvent::DeviceLost { message: device_lost.clone().unwrap_or_default() });
        }
        let decks = std::array::from_fn(|i| deck_snapshot(&view.decks[i], &slots[i]));
        DjSnapshot {
            running: !lost,
            output: self.inner.output.clone(),
            device_lost,
            decks,
            crossfader: view.crossfader,
            crossfader_curve: view.curve,
            master: view.master,
            master_peak: view.peak,
            limiting: view.limiting,
            auto_mix: view.auto_mix.map(|a| AutoMixSnapshot {
                to: DeckId::from_index(a.to),
                beats: a.beats,
                armed: a.armed,
                progress: a.progress,
            }),
            sampler_volume: view.sampler_volume,
            pads_active: view.pads_active,
            memory_used_mb: Inner::used_bytes(&slots).div_ceil(BYTES_PER_MB) as u32,
            memory_limit_mb: settings.memory_mb,
        }
    }
}

fn deck_snapshot(v: &DeckView, slot: &DeckSlot) -> DeckSnapshot {
    let ms = |frames: f64| frames / v.src_rate * 1000.0;
    DeckSnapshot {
        track: slot.track.as_ref().map(|(i, _)| i.clone()),
        loading: slot.loading,
        error: slot.error.clone(),
        playing: v.playing,
        position_ms: ms(v.pos_frames),
        cue_ms: ms(v.cue_frames),
        hot_cues: v.hot_frames.map(|h| h.map(ms)),
        loop_ms: v.loop_frames.map(|(a, b)| (ms(a), ms(b))),
        tempo_percent: (v.tempo * 100.0) as f32,
        current_bpm: v.current_bpm,
        synced: v.synced,
        bend: v.bend,
        trim_db: v.trim_db,
        eq_db: v.eq_db,
        eq_kill: v.eq_kill,
        filter: v.filter,
        volume: v.volume,
        peak: v.peak,
    }
}

impl Drop for DjPlayer {
    fn drop(&mut self) {
        // invalidate loads in flight so they don't touch a dead engine
        for slot in lock(&self.inner.slots).iter_mut() {
            slot.generation += 1;
        }
        drop(self.stop.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Opens the output: a fixed buffer of `settings.latency` first, the device's own when it refuses.
fn open(
    name: Option<&str>,
    settings: &DjSettings,
    shared: Arc<DjShared>,
    failed: Arc<AtomicBool>,
) -> Result<(cpal::Stream, Opened), DjError> {
    let device = find_device(name)?;
    // every device call under the lock: CoreAudio does not like concurrent queries
    let _queries = device_queries();
    let device_name = device.name().unwrap_or_else(|_| "output".into());
    let supported = device.default_output_config().map_err(|e| DjError::Output(e.to_string()))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels();
    let format = supported.sample_format();
    let wanted = settings.latency.frames();
    let fixed = match supported.buffer_size() {
        SupportedBufferSize::Range { min, max } => wanted.clamp(*min, *max),
        SupportedBufferSize::Unknown => wanted,
    };

    let mut last_error = DjError::Output("no usable output".into());
    for buffer in [BufferSize::Fixed(fixed), BufferSize::Default] {
        let mut config: cpal::StreamConfig = supported.config();
        config.buffer_size = buffer;

        let (commands, command_rx) = HeapRb::<Command>::new(COMMAND_RING).split();
        let (trash_tx, trash) = HeapRb::<Arc<DeckAudio>>::new(TRASH_RING).split();
        let mut mixer = DjMixer::with_pads(rate, settings.mixer(), shared.clone(), &settings.layout.pad_sounds());
        mixer.set_trash(trash_tx);

        let built = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, mixer, command_rx, failed.clone()),
            SampleFormat::I32 => build::<i32>(&device, &config, mixer, command_rx, failed.clone()),
            SampleFormat::I16 => build::<i16>(&device, &config, mixer, command_rx, failed.clone()),
            SampleFormat::U16 => build::<u16>(&device, &config, mixer, command_rx, failed.clone()),
            other => return Err(DjError::Output(format!("unsupported sample format {other}"))),
        };
        match built.and_then(|s| s.play().map(|()| s).map_err(|e| e.to_string())) {
            Ok(stream) => {
                let buffer_frames = match buffer {
                    BufferSize::Fixed(f) => Some(f),
                    BufferSize::Default => None,
                };
                let info = DjOutputInfo { device: device_name, sample_rate: rate, channels, buffer_frames };
                return Ok((stream, Opened { commands, trash, info }));
            }
            Err(e) => {
                tracing::warn!("DJ output with {buffer:?} failed: {e}");
                last_error = DjError::Output(e);
            }
        }
    }
    Err(last_error)
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: DjMixer,
    mut commands: HeapCons<Command>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    // allocated once, here: the callback only ever slices it
    let mut scratch = vec![0f32; MAX_CALLBACK_FRAMES * channels];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Some(command) = commands.try_pop() {
                    mixer.handle(command);
                }
                let mut offset = 0;
                while offset < data.len() {
                    let len = (data.len() - offset).min(scratch.len()) / channels * channels;
                    if len == 0 {
                        break;
                    }
                    mixer.render(&mut scratch[..len], channels);
                    for (out, s) in data[offset..offset + len].iter_mut().zip(&scratch[..len]) {
                        *out = T::from_sample(*s);
                    }
                    offset += len;
                }
                for out in &mut data[offset..] {
                    *out = T::from_sample(0.0f32);
                }
            },
            move |err| {
                tracing::error!("DJ audio stream error: {err}");
                failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
