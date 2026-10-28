//! Audio output through cpal: device enumeration and the realtime stream.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use homefi_core::dsp::{EqDesign, Equalizer};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use serde::Serialize;

use crate::error::{PlayerError, Result};

/// Sample rates shown to the user (the device may support ranges).
const COMMON_RATES: [u32; 8] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000];
/// Ring buffer length in seconds.
const BUFFER_SECONDS: f32 = 0.5;
/// Longer on surround devices and on Windows (WASAPI shared mode at a fixed
/// rate means resampling more often), where a busy moment would underrun.
const SAFE_BUFFER_SECONDS: f32 = 1.0;

/// An output device as shown in the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutputDevice {
    pub name: String,
    pub is_default: bool,
    /// Common rates the device can be opened at.
    pub sample_rates: Vec<u32>,
    /// Current / default rate of the device.
    pub default_rate: Option<u32>,
    pub max_channels: u16,
}

/// Lists the output devices of the default host.
pub fn list_devices() -> Vec<OutputDevice> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let Ok(devices) = host.output_devices() else { return Vec::new() };

    let mut out: Vec<OutputDevice> = devices
        .filter_map(|d| {
            let name = d.name().ok()?;
            let ranges: Vec<_> = d.supported_output_configs().ok()?.collect();
            if ranges.is_empty() {
                return None;
            }
            let mut rates: Vec<u32> = COMMON_RATES
                .into_iter()
                .filter(|r| ranges.iter().any(|c| c.min_sample_rate().0 <= *r && *r <= c.max_sample_rate().0))
                .collect();
            rates.dedup();
            Some(OutputDevice {
                is_default: Some(&name) == default_name.as_ref(),
                default_rate: d.default_output_config().ok().map(|c| c.sample_rate().0),
                max_channels: ranges.iter().map(|c| c.channels()).max().unwrap_or(2),
                sample_rates: rates,
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.cmp(&b.name)));
    // devices are picked by name, so two with the same one (twin monitors) are a single choice
    out.dedup_by(|a, b| a.name == b.name);
    out
}

/// Finds a device by name, falling back to the system default.
pub(crate) fn find_device(name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        if let Ok(mut devices) = host.output_devices() {
            if let Some(d) = devices.find(|d| d.name().ok().as_deref() == Some(name)) {
                return Ok(d);
            }
        }
        tracing::warn!("output device `{name}` not found, using the default");
    }
    host.default_output_device().ok_or_else(|| PlayerError::Output("no audio output device".into()))
}

/// Peak levels of what reaches the device, for VU meters. Each read takes
/// (and resets) the highest peak since the previous read.
pub struct Meters {
    // non-negative f32 bits order like the floats, so `fetch_max` works
    left: AtomicU32,
    right: AtomicU32,
    pub(crate) tap: crate::visual::Tap,
}

impl Default for Meters {
    fn default() -> Self {
        Self { left: AtomicU32::new(0), right: AtomicU32::new(0), tap: crate::visual::Tap::new() }
    }
}

impl Meters {
    fn record(&self, left: f32, right: f32) {
        self.left.fetch_max(left.to_bits(), Ordering::Relaxed);
        self.right.fetch_max(right.to_bits(), Ordering::Relaxed);
    }

    /// `(left, right)` peaks in 0.0 ..= 1.0 (linear) since the last call.
    pub fn take(&self) -> (f32, f32) {
        (f32::from_bits(self.left.swap(0, Ordering::Relaxed)), f32::from_bits(self.right.swap(0, Ordering::Relaxed)))
    }
}

/// Shared between the engine and the realtime callback.
pub(crate) struct Shared {
    pub volume: AtomicU32,
    /// Output frames actually sent to the device since the stream opened.
    pub frames_played: AtomicU64,
    pub failed: AtomicBool,
    pub meters: Arc<Meters>,
    /// Equalizer filters waiting for the callback (latest wins); `eq_pending`
    /// saves the callback a lock when nothing changed.
    eq: Mutex<Option<EqDesign>>,
    eq_pending: AtomicBool,
    /// Filters for tracks queued behind the current one (gapless): each
    /// starts at its first frame (counted like `frames_played`).
    scheduled: Mutex<[Option<(u64, EqDesign)>; SCHEDULED]>,
    scheduled_pending: AtomicBool,
}

/// Tracks whose filters can wait in line (one is the usual: the next track).
const SCHEDULED: usize = 4;

impl Shared {
    pub fn new(volume: f32, meters: Arc<Meters>) -> Arc<Self> {
        Arc::new(Self {
            volume: AtomicU32::new(volume.to_bits()),
            frames_played: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            meters,
            eq: Mutex::new(None),
            eq_pending: AtomicBool::new(false),
            scheduled: Mutex::new([None; SCHEDULED]),
            scheduled_pending: AtomicBool::new(false),
        })
    }

    /// Filters to glide to once frame `at` plays (a track change). When the
    /// line is full, the latest replaces the furthest.
    pub fn schedule_eq(&self, at: u64, design: EqDesign) {
        if let Ok(mut line) = self.scheduled.lock() {
            let slot = match line.iter().position(|s| s.is_none_or(|(a, _)| a == at)) {
                Some(i) => i,
                None => (0..SCHEDULED).max_by_key(|&i| line[i].map_or(0, |(a, _)| a)).unwrap_or(0),
            };
            line[slot] = Some((at, design));
            self.scheduled_pending.store(true, Ordering::Release);
        }
    }

    /// Forgets the filters waiting in line (the settings changed: they are scheduled again).
    pub fn clear_scheduled(&self) {
        if let Ok(mut line) = self.scheduled.lock() {
            *line = [None; SCHEDULED];
            self.scheduled_pending.store(false, Ordering::Release);
        }
    }

    /// The earliest waiting filters that start before frame `before`. Never blocks.
    fn take_due(&self, before: u64) -> Option<(u64, EqDesign)> {
        if !self.scheduled_pending.load(Ordering::Acquire) {
            return None;
        }
        let mut line = self.scheduled.try_lock().ok()?;
        let i = (0..SCHEDULED)
            .filter(|&i| line[i].is_some_and(|(a, _)| a < before))
            .min_by_key(|&i| line[i].map(|(a, _)| a))?;
        let due = line[i].take();
        if line.iter().all(Option::is_none) {
            self.scheduled_pending.store(false, Ordering::Release);
        }
        due
    }

    /// Hands new equalizer filters to the callback, which glides to them.
    pub fn set_eq(&self, design: EqDesign) {
        if let Ok(mut eq) = self.eq.lock() {
            *eq = Some(design);
            self.eq_pending.store(true, Ordering::Release);
        }
    }

    /// The filters set since the last call, if any. Never blocks (realtime safe).
    fn take_eq(&self) -> Option<EqDesign> {
        if !self.eq_pending.load(Ordering::Acquire) {
            return None;
        }
        let mut eq = self.eq.try_lock().ok()?;
        self.eq_pending.store(false, Ordering::Release);
        eq.take()
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
}

/// An open stream fed through a ring buffer.
pub(crate) struct Output {
    stream: cpal::Stream,
    pub producer: HeapProd<f32>,
    pub rate: u32,
    pub channels: u16,
    pub device_name: String,
}

impl Output {
    /// Opens `device` at exactly `rate` Hz, or fails with [`PlayerError::RateUnsupported`].
    pub fn open(device: &cpal::Device, rate: u32, channels: u16, shared: Arc<Shared>) -> Result<Self> {
        let device_name = device.name().unwrap_or_else(|_| "output".into());
        let ranges: Vec<_> =
            device.supported_output_configs().map_err(|e| PlayerError::Output(e.to_string()))?.collect();

        // prefer the track's channel count, then stereo, then anything; float samples first
        let rank = |f: SampleFormat| match f {
            SampleFormat::F32 => 0,
            SampleFormat::I32 => 1,
            SampleFormat::I16 => 2,
            SampleFormat::U16 => 3,
            _ => 9,
        };
        let mut candidates: Vec<_> = ranges
            .iter()
            .filter(|c| c.min_sample_rate().0 <= rate && rate <= c.max_sample_rate().0 && rank(c.sample_format()) < 9)
            .collect();
        // the track's own channel count, else stereo, else the smallest layout above it,
        // else anything (WASAPI shared mode often lists only the mix format, e.g. 8 ch)
        candidates.sort_by_key(|c| (channel_rank(c.channels(), channels), rank(c.sample_format())));
        let range = candidates.first().ok_or(PlayerError::RateUnsupported(rate))?;
        let config = (*range).with_sample_rate(cpal::SampleRate(rate));
        let out_channels = config.channels();
        let format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let seconds = if cfg!(windows) || out_channels > 2 { SAFE_BUFFER_SECONDS } else { BUFFER_SECONDS };
        let capacity = (rate as f32 * seconds) as usize * out_channels as usize;
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();

        let stream = match format {
            SampleFormat::F32 => build::<f32>(device, &stream_config, consumer, shared)?,
            SampleFormat::I32 => build::<i32>(device, &stream_config, consumer, shared)?,
            SampleFormat::I16 => build::<i16>(device, &stream_config, consumer, shared)?,
            SampleFormat::U16 => build::<u16>(device, &stream_config, consumer, shared)?,
            other => return Err(PlayerError::Output(format!("unsupported sample format {other}"))),
        };
        stream.play().map_err(|e| PlayerError::Output(e.to_string()))?;

        Ok(Self { stream, producer, rate, channels: out_channels, device_name })
    }

    pub fn pause(&self) {
        let _ = self.stream.pause();
    }

    pub fn resume(&self) {
        let _ = self.stream.play();
    }
}

/// Preference of a device layout of `offered` channels for a `wanted` channel track (lower first).
fn channel_rank(offered: u16, wanted: u16) -> (u8, u16) {
    if offered == wanted {
        (0, 0)
    } else if offered == 2 {
        (1, 0)
    } else if offered > wanted {
        (2, offered)
    } else {
        (3, u16::MAX - offered)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: HeapCons<f32>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as u64;
    let mut scratch: Vec<f32> = Vec::new();
    // the filters set before opening apply from the first sample, without a glide
    let mut eq = Equalizer::with_design(shared.take_eq().unwrap_or_default(), config.sample_rate.0, channels as usize);
    let err_shared = shared.clone();

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if scratch.len() < data.len() {
                    scratch.resize(data.len(), 0.0);
                }
                let got = consumer.pop_slice(&mut scratch[..data.len()]);
                // after decoding / resampling, before the volume: allocation free
                let start = shared.frames_played.load(Ordering::Relaxed);
                run_eq(&mut eq, &shared, start, &mut scratch[..got], channels as usize);
                let volume = shared.volume();
                let (mut peak_l, mut peak_r) = (0f32, 0f32);
                for (i, (out, s)) in data.iter_mut().zip(&scratch[..got]).enumerate() {
                    let v = *s * volume;
                    if i as u64 % channels == 0 {
                        peak_l = peak_l.max(v.abs());
                    } else {
                        peak_r = peak_r.max(v.abs());
                    }
                    *out = T::from_sample(v);
                }
                if channels == 1 {
                    peak_r = peak_l;
                }
                shared.meters.record(peak_l.min(1.0), peak_r.min(1.0));
                shared.meters.tap.push(&scratch[..got], channels as usize);
                // underrun: silence
                for out in &mut data[got..] {
                    *out = T::from_sample(0.0f32);
                }
                shared.frames_played.fetch_add(got as u64 / channels, Ordering::Relaxed);
            },
            move |err| {
                tracing::error!("audio stream error: {err}");
                err_shared.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| PlayerError::Output(e.to_string()))
}

/// Runs the equalizer over `samples` (interleaved, the first frame is
/// `start_frame`): new filters for the current track glide in at once, a
/// queued track's filters at its first frame, so a track change and its
/// taste / loudness change line up to the sample.
pub(crate) fn run_eq(eq: &mut Equalizer, shared: &Shared, start_frame: u64, samples: &mut [f32], channels: usize) {
    if let Some(design) = shared.take_eq() {
        eq.set_design(design);
    }
    let channels = channels.max(1);
    let frames = (samples.len() / channels) as u64;
    let mut done = 0usize;
    while let Some((at, design)) = shared.take_due(start_frame + frames) {
        let split = (at.saturating_sub(start_frame) as usize * channels).clamp(done, samples.len());
        eq.process_interleaved(&mut samples[done..split]);
        eq.set_design(design);
        done = split;
    }
    eq.process_interleaved(&mut samples[done..]);
}

#[cfg(test)]
mod tests {
    use homefi_core::dsp::{EqBand, EqBandKind, EqProfile};

    use super::*;

    #[test]
    fn a_queued_tracks_filters_start_at_its_first_frame_and_glide() {
        let shared = Shared::new(1.0, Arc::new(Meters::default()));
        let mut eq = Equalizer::with_design(EqDesign::BYPASS, 48_000, 2);
        // the next track starts at frame 1000, 6 dB quieter (loudness)
        shared.schedule_eq(1000, EqDesign::new(&EqProfile::default(), -6.0, 48_000));
        let mut buf = vec![0.5f32; 2 * 800];
        run_eq(&mut eq, &shared, 0, &mut buf, 2);
        assert!(buf.iter().all(|&s| s == 0.5), "the current track is untouched (bit-perfect)");
        let mut buf = vec![0.5f32; 2 * 4000];
        run_eq(&mut eq, &shared, 800, &mut buf, 2);
        // frames 800..1000 untouched, then a 20 ms glide (960 frames) down to -6 dB
        assert!(buf[..400].iter().all(|&s| s == 0.5));
        assert!(buf[400] <= 0.5 && buf[400] > 0.49);
        let worst = buf.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
        assert!(worst < 0.001, "no click: {worst}");
        let end = *buf.last().unwrap();
        assert!((end - 0.5 * 10f32.powf(-6.0 / 20.0)).abs() < 1e-3, "{end}");
        // a design for a frame already past applies at once
        shared.schedule_eq(10, EqDesign::BYPASS);
        let mut buf = vec![0.5f32; 2 * 2000];
        run_eq(&mut eq, &shared, 5000, &mut buf, 2);
        assert!((buf[buf.len() - 1] - 0.5).abs() < 1e-6);
        shared.schedule_eq(99_999, EqDesign::BYPASS);
        shared.clear_scheduled();
        assert!(shared.take_due(u64::MAX).is_none());
    }

    #[test]
    fn device_layouts_prefer_the_tracks_then_stereo() {
        let pick = |offered: &[u16], wanted: u16| offered.iter().copied().min_by_key(|&o| channel_rank(o, wanted));
        assert_eq!(pick(&[8, 2, 6], 2), Some(2));
        assert_eq!(pick(&[8, 2, 6], 6), Some(6));
        assert_eq!(pick(&[8, 2], 6), Some(2), "stereo before an unlike layout");
        // only the mix format (WASAPI shared): take it, the engine maps the channels
        assert_eq!(pick(&[8], 2), Some(8));
        assert_eq!(pick(&[8, 6], 1), Some(6));
        assert_eq!(pick(&[1], 2), Some(1));
    }

    #[test]
    fn equalizer_handoff_keeps_the_latest() {
        let shared = Shared::new(1.0, Arc::new(Meters::default()));
        assert!(shared.take_eq().is_none());
        let p = EqProfile { bands: vec![EqBand::new(EqBandKind::Peak, 1000.0, 3.0, 1.0)], ..Default::default() };
        shared.set_eq(EqDesign::BYPASS);
        shared.set_eq(EqDesign::new(&p, -3.0, 48_000));
        let got = shared.take_eq().expect("pending filters");
        assert_eq!(got.stages().len(), 1);
        assert!(shared.take_eq().is_none());
    }
}
