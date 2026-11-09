//! Analysis of a whole audio file: decode to a small mono copy, analyze, keep the result in the
//! [`AnalysisCache`]. Shared by Android's DJ, desktop's Exercise builder and anything that only needs a
//! song's tempo without playing it.

use std::path::Path;

use super::analysis::{analyze, AnalysisCache, DjAnalysis};
use crate::audio::decode::Source;

/// The decoded song is kept mono at about this rate (Hz): plenty for beats and the overview, and a
/// quarter of the memory of a 44.1 kHz stereo copy.
pub const ANALYSIS_RATE_HZ: u32 = 22_050;
const BYTES_PER_SAMPLE: u64 = std::mem::size_of::<f32>() as u64;
const BYTES_PER_MB: u64 = 1024 * 1024;

/// Why a file could not be analyzed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileAnalysisError {
    /// The file could not be opened or decoded.
    #[error("{0}")]
    Open(String),
    /// The song does not fit in the memory limit.
    #[error("the song does not fit in the memory limit")]
    TooBig,
}

/// Decodes `path` to mono at about [`ANALYSIS_RATE_HZ`], refusing a song that needs more than
/// `memory_mb`. Returns the samples and their rate.
pub fn decode_mono(path: &Path, memory_mb: u32) -> Result<(Vec<f32>, u32), FileAnalysisError> {
    let open = |e: crate::audio::decode::DecodeError| FileAnalysisError::Open(e.to_string());
    let mut source = Source::open(path).map_err(open)?;
    let rate = source.sample_rate.max(1);
    // average this many frames into one (keeps the rate near the target)
    let step = ((rate as f32 / ANALYSIS_RATE_HZ as f32).round() as usize).max(1);
    let out_rate = rate / step as u32;
    let limit = (u64::from(memory_mb) * BYTES_PER_MB / BYTES_PER_SAMPLE) as usize;
    if let Some(ms) = source.duration_ms {
        if (ms as f64 / 1000.0 * f64::from(out_rate)) as usize > limit {
            return Err(FileAnalysisError::TooBig);
        }
    }

    let mut mono: Vec<f32> = Vec::new();
    let mut acc = 0f32;
    let mut in_acc = 0usize;
    while let Some((chunk, channels)) = source.next_chunk_with_channels().map_err(open)? {
        let channels = channels.max(1) as usize;
        for frame in chunk.chunks_exact(channels) {
            acc += frame.iter().sum::<f32>() / channels as f32;
            in_acc += 1;
            if in_acc == step {
                if mono.len() >= limit {
                    return Err(FileAnalysisError::TooBig);
                }
                mono.push(acc / step as f32);
                acc = 0.0;
                in_acc = 0;
            }
        }
    }
    Ok((mono, out_rate))
}

/// The analysis of `path`: from `cache` when it has it, otherwise decoded, analyzed and stored. A cache that
/// cannot be written only costs the next analysis.
pub fn analyze_file(path: &Path, cache: &AnalysisCache, memory_mb: u32) -> Result<DjAnalysis, FileAnalysisError> {
    let key = AnalysisCache::key_for_file(path);
    if let Some(hit) = cache.get(&key) {
        return Ok(hit);
    }
    let (mono, rate) = decode_mono(path, memory_mb)?;
    let analysis = analyze(&mono, rate);
    drop(mono);
    if let Err(e) = cache.put(&key, &analysis) {
        tracing::warn!("DJ analysis cache: {e}");
    }
    Ok(analysis)
}
