//! Song analysis for DJ mode: tempo, beat grid, a colored overview and loudness.
//!
//! The song is split into low / mid / high with the mixer's own filters. Per 2.5 ms hop, the
//! growth of each band's (log-compressed) energy is summed into an onset curve (a band-wise
//! spectral flux without an FFT). Its autocorrelation proposes a tempo between 60 and 200 BPM;
//! a comb of beats then refines tempo and phase against the whole song, and a check of the
//! off-beats settles half / double-time doubts. The result is a constant grid, as DJ software
//! uses; it is only as good as the song is steady.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::mixer_math::BiquadState;
use super::sync::Grid;
use crate::dsp::{Biquad, EqBand, EqBandKind};

pub const MIN_BPM: f64 = 60.0;
pub const MAX_BPM: f64 = 200.0;
/// Overview slices per second of audio.
pub const OVERVIEW_PER_SECOND: u32 = 50;
/// Bump when the algorithm changes: older cache entries are ignored.
pub const ANALYSIS_VERSION: u32 = 1;
/// Below this confidence the grid should not be trusted (the UI hides it).
pub const MIN_GRID_CONFIDENCE: f32 = 0.25;
/// Songs shorter than this get no grid.
const MIN_GRID_SECONDS: f64 = 4.0;
/// Onset curve resolution.
const HOPS_PER_SECOND: u32 = 400;
/// Crossovers of the colored overview (Hz).
const LOW_CORNER_HZ: f32 = 250.0;
const HIGH_CORNER_HZ: f32 = 2_500.0;
/// Tempo the proposal leans to when two readings tie, and how wide that lean is (octaves).
const TEMPO_PRIOR_BPM: f64 = 120.0;
const TEMPO_PRIOR_OCTAVES: f64 = 1.0;
/// Longest stretch used to propose a tempo (seconds), taken from the middle of the song.
const TEMPO_WINDOW_SECONDS: f64 = 90.0;
/// The refinement searches this far around the proposal (fraction) in steps of this many BPM.
const REFINE_SPAN: f64 = 0.03;
const REFINE_STEP_BPM: f64 = 0.05;
/// Off-beat onsets at least this strong (versus the on-beats) mean the real tempo is double.
const DOUBLE_TIME_RATIO: f64 = 0.8;
/// Moving average of the onset curve that is subtracted (seconds).
const ONSET_BASELINE_SECONDS: f64 = 0.4;
/// Loudness blocks (seconds) and how far below the loudest block still counts (dB).
const LOUDNESS_BLOCK_SECONDS: f64 = 0.4;
const LOUDNESS_GATE_DB: f32 = 12.0;
/// Loudness reported for silence.
pub const SILENCE_DB: f32 = -120.0;

/// One 20 ms slice of the overview, each value 0..=255 (the bands share one scale, so their balance is the song's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(from = "[u8; 4]", into = "[u8; 4]")]
pub struct OverviewSlice {
    pub peak: u8,
    pub low: u8,
    pub mid: u8,
    pub high: u8,
}

impl From<[u8; 4]> for OverviewSlice {
    fn from(v: [u8; 4]) -> Self {
        Self { peak: v[0], low: v[1], mid: v[2], high: v[3] }
    }
}

impl From<OverviewSlice> for [u8; 4] {
    fn from(s: OverviewSlice) -> Self {
        [s.peak, s.low, s.mid, s.high]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DjAnalysis {
    pub version: u32,
    pub duration_ms: f64,
    /// 0 when the song has no readable tempo.
    pub bpm: f64,
    /// Position of a beat in `0..beat_ms`; the grid continues from it in both directions.
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    /// 0..=1: how clearly the beats stand out.
    pub confidence: f32,
    pub overview: Vec<OverviewSlice>,
    /// Loudness of the song's louder parts (RMS dBFS).
    pub loudness_db: f32,
}

impl Default for DjAnalysis {
    fn default() -> Self {
        Self {
            version: ANALYSIS_VERSION,
            duration_ms: 0.0,
            bpm: 0.0,
            first_beat_ms: 0.0,
            beat_ms: 0.0,
            confidence: 0.0,
            overview: Vec::new(),
            loudness_db: SILENCE_DB,
        }
    }
}

impl DjAnalysis {
    /// The beat grid, when there is one worth trusting.
    pub fn grid(&self) -> Option<Grid> {
        if self.bpm > 0.0 && self.confidence >= MIN_GRID_CONFIDENCE {
            Grid::new(self.first_beat_ms, self.beat_ms)
        } else {
            None
        }
    }
}

/// Analyzes `mono` samples at `sample_rate` Hz.
pub fn analyze(mono: &[f32], sample_rate: u32) -> DjAnalysis {
    let mut result = DjAnalysis::default();
    if mono.is_empty() || sample_rate == 0 {
        return result;
    }
    let rate = sample_rate as f64;
    result.duration_ms = mono.len() as f64 / rate * 1000.0;

    let bands = split_bands(mono, sample_rate);
    result.overview = bands.overview;
    result.loudness_db = loudness_db(&bands.slice_power);

    if result.duration_ms / 1000.0 >= MIN_GRID_SECONDS {
        let hop_seconds = bands.hop as f64 / rate;
        let onsets = onset_curve(&bands.hop_energy, hop_seconds);
        if let Some(beat) = find_beats(&onsets, hop_seconds) {
            result.beat_ms = beat.period_hops * hop_seconds * 1000.0;
            result.bpm = 60_000.0 / result.beat_ms;
            // the attack happens somewhere inside its hop: take the middle
            result.first_beat_ms = ((beat.phase_hops + 0.5) * hop_seconds * 1000.0).rem_euclid(result.beat_ms);
            result.confidence = beat.confidence;
        }
    }
    result
}

/// Per-hop band energies (for the onsets) and the overview, from one pass over the song.
struct Bands {
    hop: usize,
    /// `[low, mid, high]` mean energy of each hop.
    hop_energy: [Vec<f32>; 3],
    overview: Vec<OverviewSlice>,
    /// Mean square of every overview slice (for the loudness).
    slice_power: Vec<f32>,
}

fn split_bands(mono: &[f32], sample_rate: u32) -> Bands {
    let low_q = Biquad::design(&EqBand::new(EqBandKind::LowPass, LOW_CORNER_HZ, 0.0, 0.707), sample_rate);
    let high_q = Biquad::design(&EqBand::new(EqBandKind::HighPass, HIGH_CORNER_HZ, 0.0, 0.707), sample_rate);
    let (mut low_s, mut high_s) = (BiquadState::default(), BiquadState::default());

    let hop = (sample_rate / HOPS_PER_SECOND).max(16) as usize;
    let slice = (sample_rate / OVERVIEW_PER_SECOND).max(1) as usize;

    let mut hop_energy: [Vec<f32>; 3] = std::array::from_fn(|_| Vec::with_capacity(mono.len() / hop + 1));
    let mut hop_sum = [0f64; 3];
    let mut hop_n = 0usize;

    let mut raw: Vec<[f32; 4]> = Vec::with_capacity(mono.len() / slice + 1); // peak, low, mid, high
    let mut slice_power = Vec::with_capacity(mono.len() / slice + 1);
    let mut slice_sum = [0f64; 4]; // low², mid², high², x²
    let mut slice_peak = 0f32;
    let mut slice_n = 0usize;

    for &s in mono {
        let x = if s.is_finite() { s as f64 } else { 0.0 };
        let low = low_s.process(&low_q, x);
        let high = high_s.process(&high_q, x);
        let mid = x - low - high;
        let bands = [low * low, mid * mid, high * high];

        for (sum, b) in hop_sum.iter_mut().zip(bands) {
            *sum += b;
        }
        hop_n += 1;
        if hop_n == hop {
            for (v, sum) in hop_energy.iter_mut().zip(&mut hop_sum) {
                v.push((*sum / hop as f64) as f32);
                *sum = 0.0;
            }
            hop_n = 0;
        }

        for (sum, b) in slice_sum.iter_mut().zip(bands) {
            *sum += b;
        }
        slice_sum[3] += x * x;
        slice_peak = slice_peak.max(x.abs() as f32);
        slice_n += 1;
        if slice_n == slice {
            raw.push(slice_values(slice_peak, &slice_sum, slice));
            slice_power.push((slice_sum[3] / slice as f64) as f32);
            slice_sum = [0.0; 4];
            slice_peak = 0.0;
            slice_n = 0;
        }
    }
    if slice_n > 0 {
        raw.push(slice_values(slice_peak, &slice_sum, slice_n));
        slice_power.push((slice_sum[3] / slice_n as f64) as f32);
    }

    Bands { hop, hop_energy, overview: scale_overview(&raw), slice_power }
}

fn slice_values(peak: f32, sums: &[f64; 4], n: usize) -> [f32; 4] {
    let rms = |s: f64| (s / n as f64).sqrt() as f32;
    [peak, rms(sums[0]), rms(sums[1]), rms(sums[2])]
}

/// Peaks scaled to the song's highest peak, the three bands to the song's strongest band
/// (so their balance stays true), as 0..=255.
fn scale_overview(raw: &[[f32; 4]]) -> Vec<OverviewSlice> {
    let mut max = [0f32; 4];
    for r in raw {
        max[0] = max[0].max(r[0]);
        let band = r[1].max(r[2]).max(r[3]);
        for m in &mut max[1..] {
            *m = m.max(band);
        }
    }
    let q = |v: f32, m: f32| if m > 0.0 { (v / m * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
    raw.iter()
        .map(|r| OverviewSlice {
            peak: q(r[0], max[0]),
            low: q(r[1], max[1]),
            mid: q(r[2], max[2]),
            high: q(r[3], max[3]),
        })
        .collect()
}

/// RMS dBFS of the song's louder blocks (quiet intros and breaks don't pull it down).
fn loudness_db(slice_power: &[f32]) -> f32 {
    let per_block = ((LOUDNESS_BLOCK_SECONDS * OVERVIEW_PER_SECOND as f64) as usize).max(1);
    let blocks: Vec<f32> = slice_power.chunks(per_block).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    let loudest = blocks.iter().cloned().fold(0.0f32, f32::max);
    if loudest <= 0.0 {
        return SILENCE_DB;
    }
    let gate = loudest * 10f32.powf(-LOUDNESS_GATE_DB / 10.0);
    let kept: Vec<f32> = blocks.into_iter().filter(|p| *p >= gate).collect();
    let mean = kept.iter().sum::<f32>() / kept.len() as f32;
    (10.0 * mean.max(1e-12).log10()).max(SILENCE_DB)
}

/// The onset curve: growth of each band's compressed energy, smoothed, with its slow average
/// removed.
fn onset_curve(hop_energy: &[Vec<f32>; 3], hop_seconds: f64) -> Vec<f32> {
    let n = hop_energy[0].len();
    let mut flux = vec![0f32; n];
    for energy in hop_energy {
        let mean = energy.iter().sum::<f32>() / n.max(1) as f32;
        if mean <= 1e-12 {
            continue;
        }
        let mut prev = 0f32;
        for (f, e) in flux.iter_mut().zip(energy) {
            let c = (1.0 + 50.0 * e / mean).ln();
            *f += (c - prev).max(0.0);
            prev = c;
        }
    }
    let smooth = smooth3(&flux);
    let window = ((ONSET_BASELINE_SECONDS / hop_seconds) as usize).max(3) | 1;
    let baseline = moving_average(&smooth, window);
    smooth.iter().zip(&baseline).map(|(s, b)| (s - b).max(0.0)).collect()
}

fn smooth3(v: &[f32]) -> Vec<f32> {
    (0..v.len())
        .map(|i| {
            let prev = v[i.saturating_sub(1)];
            let next = v[(i + 1).min(v.len() - 1)];
            0.25 * prev + 0.5 * v[i] + 0.25 * next
        })
        .collect()
}

fn moving_average(v: &[f32], window: usize) -> Vec<f32> {
    let half = window / 2;
    let mut prefix = Vec::with_capacity(v.len() + 1);
    prefix.push(0f64);
    for x in v {
        prefix.push(prefix.last().unwrap() + *x as f64);
    }
    (0..v.len())
        .map(|i| {
            let (lo, hi) = (i.saturating_sub(half), (i + half + 1).min(v.len()));
            ((prefix[hi] - prefix[lo]) / (hi - lo) as f64) as f32
        })
        .collect()
}

struct Beats {
    period_hops: f64,
    phase_hops: f64,
    confidence: f32,
}

fn find_beats(onsets: &[f32], hop_seconds: f64) -> Option<Beats> {
    let energy: f32 = onsets.iter().sum();
    if onsets.len() < 16 || energy <= 1e-6 {
        return None;
    }
    let proposal = propose_tempo(onsets, hop_seconds)?;
    let mut fit = refine(onsets, hop_seconds, proposal)?;

    // half-time doubt: strong onsets between the beats mean the tempo is double
    let double = fit.bpm * 2.0;
    if double <= MAX_BPM && off_beat_ratio(onsets, &fit) >= DOUBLE_TIME_RATIO {
        if let Some(better) = refine(onsets, hop_seconds, double) {
            fit = better;
        }
    }
    Some(Beats { period_hops: fit.period, phase_hops: fit.phase, confidence: fit.confidence })
}

/// Tempo (BPM) that the autocorrelation of the onset curve likes best.
fn propose_tempo(onsets: &[f32], hop_seconds: f64) -> Option<f64> {
    let window = (TEMPO_WINDOW_SECONDS / hop_seconds) as usize;
    let part = if onsets.len() > window {
        let start = (onsets.len() - window) / 2;
        &onsets[start..start + window]
    } else {
        onsets
    };
    let lag_of = |bpm: f64| (60.0 / bpm / hop_seconds).round() as usize;
    let (min_lag, max_lag) = (lag_of(MAX_BPM).max(1), lag_of(MIN_BPM));
    if part.len() <= max_lag + 1 {
        return None;
    }
    let ac = |lag: usize| -> f64 {
        if lag >= part.len() {
            return 0.0;
        }
        let sum: f64 = part[..part.len() - lag].iter().zip(&part[lag..]).map(|(a, b)| (*a as f64) * (*b as f64)).sum();
        sum / (part.len() - lag) as f64
    };
    (min_lag..=max_lag)
        .map(|lag| {
            let bpm = 60.0 / (lag as f64 * hop_seconds);
            let octaves = (bpm / TEMPO_PRIOR_BPM).log2() / TEMPO_PRIOR_OCTAVES;
            let prior = (-0.5 * octaves * octaves).exp();
            (bpm, (ac(lag) + 0.5 * ac(2 * lag)) * prior)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, score)| *score > 0.0)
        .map(|(bpm, _)| bpm)
}

struct Fit {
    bpm: f64,
    period: f64,
    phase: f64,
    confidence: f32,
}

/// Linear interpolation of the onset curve at a fractional hop.
#[inline]
fn at(onsets: &[f32], pos: f64) -> f32 {
    if pos < 0.0 {
        return 0.0;
    }
    let i = pos as usize;
    if i + 1 >= onsets.len() {
        return onsets.get(i).copied().unwrap_or(0.0);
    }
    let f = (pos - i as f64) as f32;
    onsets[i] * (1.0 - f) + onsets[i + 1] * f
}

/// Mean onset strength on a comb of beats `period` hops apart starting at `phase`.
fn comb(onsets: &[f32], period: f64, phase: f64) -> f64 {
    let mut sum = 0f64;
    let mut count = 0usize;
    let mut pos = phase;
    while pos < onsets.len() as f64 {
        sum += at(onsets, pos) as f64;
        count += 1;
        pos += period;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Best tempo within `REFINE_SPAN` of `center_bpm` and its phase, against the whole song.
fn refine(onsets: &[f32], hop_seconds: f64, center_bpm: f64) -> Option<Fit> {
    let lo = (center_bpm * (1.0 - REFINE_SPAN)).max(MIN_BPM * 0.9);
    let hi = (center_bpm * (1.0 + REFINE_SPAN)).min(MAX_BPM * 1.1);
    let steps = ((hi - lo) / REFINE_STEP_BPM).ceil() as usize;
    let mut best: Option<(f64, f64, f64, f64)> = None; // score, bpm, period, phase
    let mut phase_scores: Vec<f64> = Vec::new();
    let mut best_scores: Vec<f64> = Vec::new();
    for step in 0..=steps {
        let bpm = lo + step as f64 * REFINE_STEP_BPM;
        let period = 60.0 / bpm / hop_seconds;
        let phases = period.ceil() as usize;
        phase_scores.clear();
        phase_scores.extend((0..phases).map(|p| comb(onsets, period, p as f64)));
        let (at_best, &score) = phase_scores.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
        if best.is_none_or(|b| score > b.0) {
            // parabolic peak between neighbours (the curve wraps around the period)
            let prev = phase_scores[(at_best + phases - 1) % phases];
            let next = phase_scores[(at_best + 1) % phases];
            let denom = prev - 2.0 * score + next;
            let shift = if denom.abs() > 1e-12 { (0.5 * (prev - next) / denom).clamp(-0.5, 0.5) } else { 0.0 };
            best = Some((score, bpm, period, (at_best as f64 + shift).rem_euclid(period)));
            best_scores.clone_from(&phase_scores);
        }
    }
    let (score, bpm, period, phase) = best?;
    if score <= 0.0 {
        return None;
    }
    let mean = best_scores.iter().sum::<f64>() / best_scores.len() as f64;
    let confidence = ((score - mean) / score).clamp(0.0, 1.0) as f32;
    Some(Fit { bpm, period, phase, confidence })
}

/// Onset strength halfway between the beats, relative to on the beats.
fn off_beat_ratio(onsets: &[f32], fit: &Fit) -> f64 {
    let on = comb(onsets, fit.period, fit.phase);
    let off = comb(onsets, fit.period, (fit.phase + fit.period / 2.0).rem_euclid(fit.period));
    if on <= 0.0 {
        0.0
    } else {
        off / on
    }
}

/// Analyses kept on disk (`~/.homefi/dj/analysis/<key>.json`).
#[derive(Debug, Clone)]
pub struct AnalysisCache {
    dir: PathBuf,
}

impl AnalysisCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `<app dir>/dj/analysis`.
    pub fn default_dir() -> PathBuf {
        crate::paths::app_dir().join("dj").join("analysis")
    }

    pub fn in_app_dir() -> Self {
        Self::new(Self::default_dir())
    }

    /// Key of a local file: changes when the file does (path, size and modification time).
    pub fn key_for_file(path: &Path) -> String {
        let meta = std::fs::metadata(path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let digest = Sha256::digest(format!("{}|{size}|{modified}", path.display()));
        let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        format!("file-{hex}")
    }

    /// Key of a TIDAL track.
    pub fn key_for_tidal(track_id: u64, quality: &str) -> String {
        let q: String = quality.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        format!("tidal-{track_id}-{q}")
    }

    fn path_for(&self, key: &str) -> Option<PathBuf> {
        // keys come from the functions above; refuse anything that could leave the folder
        let ok = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        ok.then(|| self.dir.join(format!("{key}.json")))
    }

    /// The stored analysis, `None` when absent, unreadable or from another algorithm version.
    pub fn get(&self, key: &str) -> Option<DjAnalysis> {
        let bytes = std::fs::read(self.path_for(key)?).ok()?;
        let a: DjAnalysis = serde_json::from_slice(&bytes).ok()?;
        (a.version == ANALYSIS_VERSION).then_some(a)
    }

    /// Stores `analysis` atomically (a reader never sees half a file).
    pub fn put(&self, key: &str, analysis: &DjAnalysis) -> crate::Result<()> {
        let Some(path) = self.path_for(key) else {
            return Err(crate::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad cache key")));
        };
        crate::paths::write_atomic(&path, &serde_json::to_vec(analysis)?)?;
        Ok(())
    }

    /// The tempo of a stored analysis, when it has a grid worth trusting (for song lists: the
    /// overview is skipped, not built).
    pub fn bpm(&self, key: &str) -> Option<f64> {
        #[derive(Deserialize)]
        struct Head {
            version: u32,
            bpm: f64,
            confidence: f32,
        }
        let bytes = std::fs::read(self.path_for(key)?).ok()?;
        let head: Head = serde_json::from_slice(&bytes).ok()?;
        let trusted = head.version == ANALYSIS_VERSION && head.bpm > 0.0 && head.confidence >= MIN_GRID_CONFIDENCE;
        trusted.then_some(head.bpm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    /// A click track: 5 ms bursts of 1 kHz on every beat, the first at `offset_s`.
    fn clicks(bpm: f64, offset_s: f64, seconds: f64) -> Vec<f32> {
        let mut out = vec![0f32; (seconds * RATE as f64) as usize];
        let period = 60.0 / bpm;
        let burst = (0.005 * RATE as f64) as usize;
        let mut t = offset_s;
        while t < seconds {
            let start = (t * RATE as f64) as usize;
            for k in 0..burst {
                if let Some(s) = out.get_mut(start + k) {
                    let env = (std::f64::consts::PI * k as f64 / burst as f64).sin();
                    *s = (env * (std::f64::consts::TAU * 1000.0 * k as f64 / RATE as f64).sin()) as f32 * 0.8;
                }
            }
            t += period;
        }
        out
    }

    fn phase_error_ms(a: &DjAnalysis, offset_s: f64) -> f64 {
        let diff = (a.first_beat_ms - offset_s * 1000.0).rem_euclid(a.beat_ms);
        diff.min(a.beat_ms - diff)
    }

    #[test]
    fn finds_tempo_and_phase_of_click_tracks() {
        for (bpm, offset) in [(128.0, 0.1), (90.0, 0.23), (174.0, 0.05)] {
            let a = analyze(&clicks(bpm, offset, 24.0), RATE);
            assert!((a.bpm - bpm).abs() <= 0.5, "{bpm} BPM read as {}", a.bpm);
            assert!(phase_error_ms(&a, offset) <= 10.0, "{bpm} BPM: phase off by {} ms", phase_error_ms(&a, offset));
            assert!(a.confidence > 0.6, "{bpm}: confidence {}", a.confidence);
            let grid = a.grid().expect("a grid");
            assert!((grid.bpm() - a.bpm).abs() < 1e-9);
        }
    }

    #[test]
    fn overview_and_loudness_describe_the_song() {
        let a = analyze(&clicks(120.0, 0.0, 10.0), RATE);
        assert!((a.overview.len() as i64 - 500).abs() <= 1, "50 slices per second, got {}", a.overview.len());
        assert!((a.duration_ms - 10_000.0).abs() < 1.0);
        assert!(a.overview.iter().any(|s| s.peak == 255));
        // a 1 kHz click is mid-range: the low band stays down
        let mid: u32 = a.overview.iter().map(|s| s.mid as u32).sum();
        let low: u32 = a.overview.iter().map(|s| s.low as u32).sum();
        assert!(mid > low * 2, "mid {mid} low {low}");
        assert!(a.loudness_db < -10.0 && a.loudness_db > -60.0, "{}", a.loudness_db);

        // a louder song reads louder
        let loud: Vec<f32> = clicks(120.0, 0.0, 10.0).iter().map(|x| x * 1.0).collect();
        let quiet: Vec<f32> = loud.iter().map(|x| x * 0.25).collect();
        let (l, q) = (analyze(&loud, RATE).loudness_db, analyze(&quiet, RATE).loudness_db);
        assert!((l - q - 12.0).abs() < 0.5, "{l} vs {q}");
    }

    #[test]
    fn silence_short_and_empty_songs_get_no_grid() {
        let silent = analyze(&vec![0.0; RATE as usize * 10], RATE);
        assert_eq!((silent.bpm, silent.confidence), (0.0, 0.0));
        assert!(silent.grid().is_none());
        assert_eq!(silent.loudness_db, SILENCE_DB);
        let short = analyze(&clicks(120.0, 0.0, 2.0), RATE);
        assert!(short.grid().is_none());
        let empty = analyze(&[], RATE);
        assert!(empty.overview.is_empty() && empty.grid().is_none());
        assert!(analyze(&[0.5; 100], 0).grid().is_none());
        // NaN input must not poison anything
        let mut noisy = clicks(120.0, 0.0, 8.0);
        noisy[1000] = f32::NAN;
        assert!(analyze(&noisy, RATE).loudness_db.is_finite());
    }

    #[test]
    fn cache_round_trips_atomically_and_ignores_old_versions() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AnalysisCache::new(dir.path().join("dj").join("analysis"));
        let a = analyze(&clicks(128.0, 0.1, 12.0), RATE);
        assert!(cache.get("k1").is_none());
        cache.put("k1", &a).unwrap();
        assert_eq!(cache.get("k1").unwrap(), a);
        // no temporary files are left behind
        let names: Vec<_> = std::fs::read_dir(dir.path().join("dj").join("analysis"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec!["k1.json".to_owned()]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("dj/analysis/k1.json")).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "private file");
        }
        // an older algorithm's entry is a miss; garbage too
        let old = DjAnalysis { version: ANALYSIS_VERSION + 1, ..a.clone() };
        cache.put("k2", &old).unwrap();
        assert!(cache.get("k2").is_none());
        std::fs::write(dir.path().join("dj/analysis/k3.json"), b"{nope").unwrap();
        assert!(cache.get("k3").is_none());
        // keys can't escape the folder
        assert!(cache.get("../x").is_none() && cache.put("a/b", &a).is_err() && cache.put("", &a).is_err());
        // the tempo alone, only when the grid is trusted
        assert!((cache.bpm("k1").unwrap() - 128.0).abs() < 0.5, "{:?}", cache.bpm("k1"));
        assert_eq!(cache.bpm("k2"), None, "older version");
        assert_eq!(cache.bpm("k3"), None, "garbage");
        cache.put("k4", &DjAnalysis { confidence: MIN_GRID_CONFIDENCE / 2.0, ..a.clone() }).unwrap();
        assert_eq!(cache.bpm("k4"), None, "a grid not worth trusting");
        assert_eq!(cache.bpm("missing"), None);
    }

    #[test]
    fn cache_keys_follow_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.flac");
        std::fs::write(&f, b"one").unwrap();
        let k1 = AnalysisCache::key_for_file(&f);
        assert_eq!(k1, AnalysisCache::key_for_file(&f));
        std::fs::write(&f, b"longer content").unwrap();
        assert_ne!(k1, AnalysisCache::key_for_file(&f));
        assert_eq!(AnalysisCache::key_for_tidal(42, "HI_RES_LOSSLESS"), "tidal-42-HIRESLOSSLESS");
    }

    #[test]
    fn overview_slices_serialize_compactly() {
        let s = OverviewSlice { peak: 1, low: 2, mid: 3, high: 4 };
        assert_eq!(serde_json::to_string(&s).unwrap(), "[1,2,3,4]");
        assert_eq!(serde_json::from_str::<OverviewSlice>("[9,8,7,6]").unwrap().high, 6);
    }
}
