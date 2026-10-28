//! Audio output through cpal: device enumeration and the realtime stream.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use serde::Serialize;

use crate::error::{PlayerError, Result};

/// Sample rates shown to the user (the device may support ranges).
const COMMON_RATES: [u32; 8] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000];
/// Ring buffer length in seconds.
const BUFFER_SECONDS: f32 = 0.5;

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
}

impl Shared {
    pub fn new(volume: f32, meters: Arc<Meters>) -> Arc<Self> {
        Arc::new(Self {
            volume: AtomicU32::new(volume.to_bits()),
            frames_played: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            meters,
        })
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
        candidates.sort_by_key(|c| {
            let ch = c.channels();
            (
                if ch == channels {
                    0
                } else if ch == 2 {
                    1
                } else {
                    2
                },
                rank(c.sample_format()),
            )
        });
        let range = candidates.first().ok_or(PlayerError::RateUnsupported(rate))?;
        let config = (*range).with_sample_rate(cpal::SampleRate(rate));
        let out_channels = config.channels();
        let format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let capacity = (rate as f32 * BUFFER_SECONDS) as usize * out_channels as usize;
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
    let err_shared = shared.clone();

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if scratch.len() < data.len() {
                    scratch.resize(data.len(), 0.0);
                }
                let got = consumer.pop_slice(&mut scratch[..data.len()]);
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
