//! Loading a song for a deck: decode the whole file to RAM, analyze it (or read the analysis from
//! the cache), check the memory budget. Runs on its own thread, never on the audio thread.

use std::path::Path;
use std::sync::Arc;

use homefi_core::audio::decode::Source;
use homefi_core::dj::mixer_math::auto_gain_db;
use homefi_core::dj::{analyze, AnalysisCache, DjAnalysis};

use super::control::DjTrackInfo;
use super::deck::DeckAudio;
use super::error::DjError;

const BYTES_PER_MB: u64 = 1024 * 1024;
/// Bytes of one decoded frame (stereo `f32`).
const BYTES_PER_FRAME: u64 = 8;
/// Analyses of songs whose length differs by more than this (ms) from the file are stale.
const CACHE_DURATION_SLACK_MS: f64 = 100.0;

/// A song ready to be put on a deck.
pub struct LoadedTrack {
    pub audio: DeckAudio,
    pub analysis: Arc<DjAnalysis>,
    pub info: DjTrackInfo,
}

fn mb(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_MB)
}

/// Decodes `path`, using at most `available_bytes` of RAM (`limit_mb` is the user's whole limit,
/// only for the message). `cancelled` is polled while decoding so a newer load can end this one.
pub fn load_track(
    path: &Path,
    title: &str,
    available_bytes: u64,
    limit_mb: u64,
    cache: Option<&AnalysisCache>,
    cancelled: &dyn Fn() -> bool,
) -> Result<LoadedTrack, DjError> {
    let mut source = Source::open(path)?;
    let rate = source.sample_rate;
    let too_big = |frames: u64| DjError::TooBig { needed_mb: mb(frames * BYTES_PER_FRAME), limit_mb };

    // the container usually knows the length: refuse before decoding anything
    let expected_frames = source.duration_ms.map(|ms| ms * rate as u64 / 1000);
    if let Some(frames) = expected_frames {
        if frames * BYTES_PER_FRAME > available_bytes {
            return Err(too_big(frames));
        }
    }
    let reserve = expected_frames.unwrap_or(0).min(available_bytes / BYTES_PER_FRAME) as usize;
    let mut samples: Vec<f32> = Vec::with_capacity(reserve * 2);

    while let Some((chunk, channels)) = source.next_chunk_with_channels()? {
        if cancelled() {
            return Err(DjError::Cancelled);
        }
        // the decoder's real layout is known after the first packet
        let ch = channels.max(1) as usize;
        for frame in chunk.chunks_exact(ch) {
            // mono is doubled; more than two channels play their first two (a downmix of
            // surround files is not worth the guesswork for DJ use)
            let l = frame[0];
            let r = if ch > 1 { frame[1] } else { l };
            samples.push(l);
            samples.push(r);
        }
        let used = samples.len() as u64 / 2 * BYTES_PER_FRAME;
        if used > available_bytes {
            return Err(too_big(samples.len() as u64 / 2));
        }
    }
    if samples.is_empty() {
        return Err(DjError::Empty);
    }
    let rate = source.sample_rate;

    let duration_ms = samples.len() as f64 / 2.0 / rate as f64 * 1000.0;
    let key = AnalysisCache::key_for_file(path);
    let cached =
        cache.and_then(|c| c.get(&key)).filter(|a| (a.duration_ms - duration_ms).abs() <= CACHE_DURATION_SLACK_MS);
    let analysis = match cached {
        Some(a) => a,
        None => {
            let mono: Vec<f32> = samples.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
            let a = analyze(&mono, rate);
            if let Some(c) = cache {
                if let Err(e) = c.put(&key, &a) {
                    tracing::warn!("could not cache the DJ analysis: {e}");
                }
            }
            a
        }
    };

    let grid = analysis.grid();
    let gain = auto_gain_db(analysis.loudness_db);
    let audio = DeckAudio::new(samples, rate, grid, gain);
    let info = DjTrackInfo {
        title: title.to_owned(),
        duration_ms: audio.duration_ms(),
        sample_rate: rate,
        codec: source.codec.clone(),
        bpm: analysis.bpm,
        first_beat_ms: analysis.first_beat_ms,
        beat_ms: analysis.beat_ms,
        has_grid: grid.is_some(),
        confidence: analysis.confidence,
        loudness_db: analysis.loudness_db,
        auto_gain_db: gain,
        bytes: audio.bytes(),
    };
    Ok(LoadedTrack { audio, analysis: Arc::new(analysis), info })
}
