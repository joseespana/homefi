//! Data for visualizers: spectrum (FFT), waveform and peak levels of the
//! audio that actually reaches the device.

use std::f32::consts::PI;
use std::sync::Mutex;

use serde::Serialize;

/// Samples kept for analysis (power of two, ~46 ms at 44.1 kHz).
const WINDOW: usize = 2048;
/// Points of the waveform sent to the UI.
const WAVE_POINTS: usize = 256;
/// Lowest and highest band edges (Hz).
const MIN_HZ: f32 = 30.0;
const MAX_HZ: f32 = 20_000.0;
/// dB range mapped to 0..=1.
const FLOOR_DB: f32 = -72.0;

/// Mono mix of the most recent output samples. The realtime callback only
/// ever `try_lock`s it, so it never blocks the audio thread.
pub(crate) struct Tap {
    inner: Mutex<TapBuf>,
}

struct TapBuf {
    data: Vec<f32>,
    pos: usize,
    rate: u32,
}

impl Tap {
    pub fn new() -> Self {
        Self { inner: Mutex::new(TapBuf { data: vec![0.0; WINDOW], pos: 0, rate: 44_100 }) }
    }

    pub fn set_rate(&self, rate: u32) {
        if let Ok(mut b) = self.inner.lock() {
            b.rate = rate;
        }
    }

    /// Called from the audio callback with interleaved samples.
    pub fn push(&self, interleaved: &[f32], channels: usize) {
        let Ok(mut b) = self.inner.try_lock() else { return };
        let ch = channels.max(1);
        for frame in interleaved.chunks_exact(ch) {
            let mono = frame.iter().take(2).sum::<f32>() / ch.min(2) as f32;
            let pos = b.pos;
            b.data[pos] = mono;
            b.pos = (pos + 1) % WINDOW;
        }
    }

    pub fn clear(&self) {
        if let Ok(mut b) = self.inner.lock() {
            b.data.iter_mut().for_each(|s| *s = 0.0);
        }
    }

    /// Snapshot in chronological order plus the sample rate.
    fn snapshot(&self) -> (Vec<f32>, u32) {
        let b = self.inner.lock().expect("tap lock");
        let mut out = Vec::with_capacity(WINDOW);
        out.extend_from_slice(&b.data[b.pos..]);
        out.extend_from_slice(&b.data[..b.pos]);
        (out, b.rate)
    }
}

/// One visualizer frame.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Visual {
    /// Peak levels since the previous frame, linear 0..=1.
    pub left: f32,
    pub right: f32,
    /// Log-spaced bands from 30 Hz to 20 kHz, 0..=1 (−72 dBFS .. 0 dBFS).
    pub spectrum: Vec<f32>,
    /// Recent waveform, −1..=1.
    pub wave: Vec<f32>,
}

pub(crate) fn analyze(tap: &Tap, bands: usize) -> (Vec<f32>, Vec<f32>) {
    let (samples, rate) = tap.snapshot();
    let n = samples.len();

    // waveform: the latest WAVE_POINTS * 2 samples, decimated
    let tail = &samples[n - WAVE_POINTS * 2..];
    let wave: Vec<f32> = tail.chunks(2).map(|c| c[0].clamp(-1.0, 1.0)).collect();

    // Hann-windowed FFT
    let mut re: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| s * (0.5 - 0.5 * (2.0 * PI * i as f32 / (n - 1) as f32).cos()))
        .collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);

    let bin_hz = rate as f32 / n as f32;
    let max_hz = MAX_HZ.min(rate as f32 / 2.0);
    let ratio = (max_hz / MIN_HZ).powf(1.0 / bands.max(1) as f32);
    let mut spectrum = Vec::with_capacity(bands);
    let mut lo = MIN_HZ;
    for _ in 0..bands {
        let hi = lo * ratio;
        let (a, b) = ((lo / bin_hz) as usize, ((hi / bin_hz).ceil() as usize).max((lo / bin_hz) as usize + 1));
        let peak = (a..b.min(n / 2)).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).fold(0.0f32, f32::max);
        // Hann window halves the amplitude; a full-scale sine ≈ n/4
        let db = 20.0 * (peak / (n as f32 / 4.0)).max(1e-9).log10();
        spectrum.push(((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0));
        lo = hi;
    }
    (spectrum, wave)
}

/// In-place iterative radix-2 FFT (`re.len()` must be a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        let (w_re, w_im) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cur_re, mut cur_im) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * cur_re - im[b] * cur_im;
                let t_im = re[b] * cur_im + im[b] * cur_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_peaks_in_its_band() {
        let tap = Tap::new();
        tap.set_rate(48_000);
        let sine: Vec<f32> = (0..WINDOW * 2).map(|i| (2.0 * PI * 1000.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let stereo: Vec<f32> = sine.iter().flat_map(|s| [*s, *s]).collect();
        tap.push(&stereo, 2);

        let (spectrum, wave) = analyze(&tap, 32);
        assert_eq!(wave.len(), WAVE_POINTS);
        let loudest = spectrum.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        // band containing 1 kHz
        let ratio = (MAX_HZ / MIN_HZ).powf(1.0 / 32.0);
        let expected = ((1000.0 / MIN_HZ).ln() / ratio.ln()) as usize;
        assert!(loudest.abs_diff(expected) <= 1, "loudest band {loudest}, expected {expected}");
        // -6 dBFS sine ≈ 0.9 on the 72 dB scale
        assert!(spectrum[loudest] > 0.8, "{}", spectrum[loudest]);
        assert!(spectrum[2] < 0.5, "low bands should be quiet: {}", spectrum[2]);
    }

    #[test]
    fn silence_is_zero() {
        let tap = Tap::new();
        let (spectrum, wave) = analyze(&tap, 16);
        assert!(spectrum.iter().all(|v| *v == 0.0));
        assert!(wave.iter().all(|v| *v == 0.0));
    }
}
