//! FLAC in pure Rust (`flacenc`), fed from the pipeline's chunks, and the check that makes
//! "lossless" a fact: after writing, the file is decoded (symphonia, an independent decoder)
//! and its audio compared with what the drive delivered.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::config::{Encoder as EncoderConfig, OrderSel};
use flacenc::error::{SourceError, Verified, Verify};
use flacenc::source::{Fill, Source};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::checksum::PcmDigest;
use crate::chunks::ChunkConsumer;
use crate::error::{DiscError, Result};
use crate::source::SAMPLES_PER_SECTOR;

const CHANNELS: usize = 2;
const BITS_PER_SAMPLE: usize = 16;
const SAMPLE_RATE_HZ: usize = 44_100;
/// Level 0 to 8, in the spirit of the reference encoder: (block size, LPC order; `None` = only
/// fixed predictors). Level 5 is the default.
const LEVELS: [(usize, Option<usize>); 9] = [
    (1152, None),
    (1152, None),
    (1152, Some(4)),
    (4096, Some(6)),
    (4096, Some(8)),
    (4096, Some(8)),
    (4096, Some(10)),
    (4096, Some(12)),
    (4096, Some(12)),
];
/// Level from which the predictor order is picked by counting bits instead of estimating.
const EXACT_ORDER_FROM_LEVEL: u8 = 8;
/// Fixed predictor order for the levels without LPC.
const FIXED_ONLY_ORDERS: [usize; 2] = [2, 4];

fn encoder_config(level: u8) -> Result<Verified<EncoderConfig>> {
    let level = usize::from(level).min(LEVELS.len() - 1);
    let (block_size, lpc_order) = LEVELS[level];
    let mut config = EncoderConfig::default();
    // one thread: ripping must not fight the music for the CPU
    config.multithread = false;
    config.block_size = block_size;
    match lpc_order {
        Some(order) => config.subframe_coding.qlpc.lpc_order = order,
        None => {
            config.subframe_coding.use_lpc = false;
            config.subframe_coding.fixed.max_order = FIXED_ONLY_ORDERS[level.min(FIXED_ONLY_ORDERS.len() - 1)];
        }
    }
    if level >= usize::from(EXACT_ORDER_FROM_LEVEL) {
        config.subframe_coding.fixed.order_sel = OrderSel::BitCount;
    }
    config.into_verified().map_err(|(_, error)| DiscError::Encode(format!("flac settings: {error}")))
}

/// Re-blocks the pipeline's chunks into the encoder's blocks and keeps the digest of what it
/// saw. Holds at most one chunk and one block of audio.
struct ChunkSource<'a> {
    chunks: &'a ChunkConsumer,
    pending: Vec<i32>,
    start: usize,
    ended: bool,
    digest: PcmDigest,
}

impl<'a> ChunkSource<'a> {
    fn new(chunks: &'a ChunkConsumer) -> Self {
        let capacity = 2 * SAMPLES_PER_SECTOR * usize::from(crate::source::CHUNK_SECTORS);
        ChunkSource {
            chunks,
            pending: Vec::with_capacity(capacity),
            start: 0,
            ended: false,
            digest: PcmDigest::default(),
        }
    }

    fn available(&self) -> usize {
        self.pending.len() - self.start
    }

    fn pull(&mut self) {
        let Some(chunk) = self.chunks.recv() else {
            self.ended = true;
            return;
        };
        self.digest.push(&chunk.samples);
        if self.start > 0 {
            self.pending.drain(..self.start);
            self.start = 0;
        }
        self.pending.extend(chunk.samples.iter().map(|s| i32::from(*s)));
        self.chunks.recycle(chunk);
    }
}

impl Source for ChunkSource<'_> {
    fn channels(&self) -> usize {
        CHANNELS
    }
    fn bits_per_sample(&self) -> usize {
        BITS_PER_SAMPLE
    }
    fn sample_rate(&self) -> usize {
        SAMPLE_RATE_HZ
    }
    fn read_samples<F: Fill>(&mut self, block_size: usize, dest: &mut F) -> std::result::Result<usize, SourceError> {
        let wanted = block_size * CHANNELS;
        while self.available() < wanted && !self.ended {
            self.pull();
        }
        let take = self.available().min(wanted);
        if take == 0 {
            return Ok(0);
        }
        dest.fill_interleaved(&self.pending[self.start..self.start + take])?;
        self.start += take;
        Ok(take / CHANNELS)
    }
}

/// symphonia (the player's decoder, and the check below) refuses a last block shorter than
/// this, although the FLAC format allows it.
const MIN_LAST_BLOCK_FRAMES: u64 = 16;

/// The block size closest below `preferred` whose last block is not a sliver, for a track of
/// `frames` frames (0 = unknown: `preferred`).
fn block_size_for(frames: u64, preferred: usize) -> usize {
    let fits = |size: usize| {
        let rest = frames % size as u64;
        rest == 0 || rest >= MIN_LAST_BLOCK_FRAMES
    };
    (MIN_LAST_BLOCK_FRAMES as usize..=preferred).rev().find(|size| frames == 0 || fits(*size)).unwrap_or(preferred)
}

/// Encodes everything the pipeline delivers into `path`. `frames` is how many stereo frames
/// are expected (0 if unknown). Returns what the encoder was given.
pub fn encode_flac(chunks: &ChunkConsumer, path: &Path, level: u8, frames: u64) -> Result<PcmDigest> {
    let config = encoder_config(level)?;
    let mut source = ChunkSource::new(chunks);
    let block_size = block_size_for(frames, config.block_size);
    let mut stream = flacenc::encode_with_fixed_block_size(&config, &mut source, block_size)
        .map_err(|error| DiscError::Encode(format!("flac: {error}")))?;
    // The encoder counts a short last block as the minimum size; the format (and symphonia) say
    // the minimum excludes it, and a stream of one short block has that block's size for both.
    let frames_seen = (source.digest.samples / CHANNELS as u64) as usize;
    let size = if stream.frame_count() > 1 { block_size } else { frames_seen };
    if size >= MIN_LAST_BLOCK_FRAMES as usize {
        stream
            .stream_info_mut()
            .set_block_sizes(size, size)
            .map_err(|error| DiscError::Encode(format!("flac: {error}")))?;
    }
    let mut sink = ByteSink::with_capacity(stream.count_bits());
    stream.write(&mut sink).map_err(|error| DiscError::Encode(format!("flac: {error}")))?;
    let mut file = BufWriter::new(File::create(path)?);
    file.write_all(sink.as_slice())?;
    file.flush()?;
    Ok(source.digest)
}

fn decode_error(error: impl std::fmt::Debug) -> DiscError {
    DiscError::Encode(format!("flac check: {error:?}"))
}

/// Decodes `path` and compares its audio with `expected` (what the encoder was given): same
/// number of samples and the same CRC. A mismatch is an [`DiscError::Encode`].
pub fn verify_flac(path: &Path, expected: &PcmDigest) -> Result<()> {
    let stream = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("flac");
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(decode_error)?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| decode_error("no audio track"))?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(decode_error)?;
    let mut digest = PcmDigest::default();
    let mut samples: Option<SampleBuffer<i16>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(decode_error(error)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(decode_error)?;
        let buffer = samples.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, *decoded.spec()));
        buffer.copy_interleaved_ref(decoded);
        digest.push(buffer.samples());
    }
    if digest == *expected {
        Ok(())
    } else {
        Err(DiscError::Encode(format!(
            "flac check: the file decodes to {} samples (crc {:08x}), expected {} (crc {:08x})",
            digest.samples,
            digest.crc32(),
            expected.samples,
            expected.crc32()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::chunk_channel;
    use std::sync::atomic::AtomicBool;

    /// Feeds `pcm` through the pipe in odd-sized pieces, as the reader would.
    fn feed(pcm: Vec<i16>) -> ChunkConsumer {
        let (mut tx, rx) = chunk_channel(4);
        std::thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            for piece in pcm.chunks(SAMPLES_PER_SECTOR * 27 - 10) {
                if !tx.send(piece, &cancel) {
                    break;
                }
            }
        });
        rx
    }

    fn music(frames: usize) -> Vec<i16> {
        (0..frames)
            .flat_map(|n| {
                let t = n as f32 / SAMPLE_RATE_HZ as f32;
                let left = (t * 440.0 * std::f32::consts::TAU).sin() * 12_000.0;
                let right = (t * 660.0 * std::f32::consts::TAU).sin() * 9_000.0 + ((n * 7919) % 97) as f32;
                [left as i16, right as i16]
            })
            .collect()
    }

    #[test]
    fn every_level_is_lossless_and_decodes_identically() {
        let pcm = music(44_100 + 123); // not a multiple of any block size
        let dir = tempfile::tempdir().unwrap();
        for level in 0..=8u8 {
            let path = dir.path().join(format!("level{level}.flac"));
            let digest = encode_flac(&feed(pcm.clone()), &path, level, 0).unwrap();
            let mut expected = PcmDigest::default();
            expected.push(&pcm);
            assert_eq!(digest, expected, "the encoder saw what the reader gave (level {level})");
            verify_flac(&path, &expected).unwrap_or_else(|e| panic!("level {level}: {e}"));
            assert!(std::fs::metadata(&path).unwrap().len() < (pcm.len() * 2) as u64, "level {level} compresses");
        }
    }

    #[test]
    fn a_sliver_for_a_last_block_is_avoided() {
        // 588 frames per sector: some track lengths leave 4, 8 or 12 frames over a 4096 block
        assert_eq!(block_size_for(4096 * 10 + 4, 4096), 4094);
        assert_eq!(block_size_for(4096 * 10, 4096), 4096);
        assert_eq!(block_size_for(4096 * 10 + 300, 4096), 4096);
        assert_eq!(block_size_for(0, 4096), 4096);
        for rest in [4u64, 8, 12] {
            let frames = 588 * 1000 - (588 * 1000 % 4096) + rest;
            let size = block_size_for(frames, 4096);
            let last = frames % size as u64;
            assert!(last == 0 || last >= MIN_LAST_BLOCK_FRAMES);
        }
    }

    #[test]
    fn a_track_whose_length_leaves_a_sliver_still_decodes() {
        let frames = 4096 * 6 + 4;
        let pcm = music(frames);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sliver.flac");
        let digest = encode_flac(&feed(pcm), &path, 5, frames as u64).unwrap();
        verify_flac(&path, &digest).unwrap();
    }

    #[test]
    fn silence_and_tiny_tracks_work() {
        let dir = tempfile::tempdir().unwrap();
        for frames in [0usize, 1, 100, 5000] {
            let pcm = vec![0i16; frames * 2];
            let path = dir.path().join(format!("silence{frames}.flac"));
            let digest = encode_flac(&feed(pcm.clone()), &path, 5, 0).unwrap();
            if frames as u64 >= MIN_LAST_BLOCK_FRAMES {
                verify_flac(&path, &digest).unwrap();
            }
        }
    }

    #[test]
    fn the_check_catches_a_changed_sample() {
        let pcm = music(20_000);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.flac");
        encode_flac(&feed(pcm.clone()), &path, 5, 0).unwrap();
        let mut other = pcm;
        other[777] = other[777].wrapping_add(1);
        let mut expected = PcmDigest::default();
        expected.push(&other);
        assert!(matches!(verify_flac(&path, &expected), Err(DiscError::Encode(_))));
    }
}
