//! WAV written by us: a 44-byte RIFF header whose sizes are patched when the audio ends, then
//! the samples as they arrive. Audio CDs are 16-bit stereo at 44.1 kHz.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::checksum::PcmDigest;
use crate::chunks::ChunkConsumer;
use crate::error::Result;

const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const SAMPLE_RATE_HZ: u32 = 44_100;
const BYTES_PER_FRAME: u16 = CHANNELS * BITS_PER_SAMPLE / 8;
const PCM_FORMAT: u16 = 1;
const HEADER_BYTES: u32 = 44;
/// RIFF size = file size minus the 8 bytes of "RIFF" + size.
const RIFF_PREFIX_BYTES: u32 = 8;
const RIFF_SIZE_OFFSET: u64 = 4;
const DATA_SIZE_OFFSET: u64 = 40;

fn header(data_bytes: u32) -> [u8; HEADER_BYTES as usize] {
    let mut h = [0u8; HEADER_BYTES as usize];
    let put16 = |h: &mut [u8], at: usize, v: u16| h[at..at + 2].copy_from_slice(&v.to_le_bytes());
    let put32 = |h: &mut [u8], at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    h[0..4].copy_from_slice(b"RIFF");
    put32(&mut h, 4, HEADER_BYTES - RIFF_PREFIX_BYTES + data_bytes);
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    put32(&mut h, 16, 16);
    put16(&mut h, 20, PCM_FORMAT);
    put16(&mut h, 22, CHANNELS);
    put32(&mut h, 24, SAMPLE_RATE_HZ);
    put32(&mut h, 28, SAMPLE_RATE_HZ * u32::from(BYTES_PER_FRAME));
    put16(&mut h, 32, BYTES_PER_FRAME);
    put16(&mut h, 34, BITS_PER_SAMPLE);
    h[36..40].copy_from_slice(b"data");
    put32(&mut h, 40, data_bytes);
    h
}

/// Writes everything the pipeline delivers into `path`. Returns what was written.
pub fn write_wav(chunks: &ChunkConsumer, path: &Path) -> Result<PcmDigest> {
    let mut file = BufWriter::new(File::create(path)?);
    file.write_all(&header(0))?;
    let mut digest = PcmDigest::default();
    let mut bytes: Vec<u8> = Vec::new();
    let mut data_bytes: u64 = 0;
    while let Some(chunk) = chunks.recv() {
        digest.push(&chunk.samples);
        bytes.clear();
        bytes.extend(chunk.samples.iter().flat_map(|s| s.to_le_bytes()));
        file.write_all(&bytes)?;
        data_bytes += bytes.len() as u64;
        chunks.recycle(chunk);
    }
    let data_bytes = u32::try_from(data_bytes).unwrap_or(u32::MAX);
    file.seek(SeekFrom::Start(RIFF_SIZE_OFFSET))?;
    file.write_all(&(HEADER_BYTES - RIFF_PREFIX_BYTES + data_bytes).to_le_bytes())?;
    file.seek(SeekFrom::Start(DATA_SIZE_OFFSET))?;
    file.write_all(&data_bytes.to_le_bytes())?;
    file.flush()?;
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunks::chunk_channel;
    use std::sync::atomic::AtomicBool;

    fn write(pcm: &[i16]) -> Vec<u8> {
        let (mut tx, rx) = chunk_channel(2);
        let owned = pcm.to_vec();
        std::thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            for piece in owned.chunks(1001) {
                tx.send(piece, &cancel);
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let digest = write_wav(&rx, &path).unwrap();
        let mut expected = PcmDigest::default();
        expected.push(pcm);
        assert_eq!(digest, expected);
        std::fs::read(path).unwrap()
    }

    #[test]
    fn header_sizes_match_the_audio_for_odd_lengths() {
        for frames in [0usize, 1, 3, 2999] {
            let pcm: Vec<i16> = (0..frames * 2).map(|n| n as i16 - 5).collect();
            let file = write(&pcm);
            let data = frames * 4;
            assert_eq!(file.len(), 44 + data);
            assert_eq!(&file[0..4], b"RIFF");
            assert_eq!(u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize, 36 + data);
            assert_eq!(&file[8..16], b"WAVEfmt ");
            assert_eq!(u32::from_le_bytes(file[24..28].try_into().unwrap()), 44_100);
            assert_eq!(&file[36..40], b"data");
            assert_eq!(u32::from_le_bytes(file[40..44].try_into().unwrap()) as usize, data);
            let back: Vec<i16> = file[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            assert_eq!(back, pcm);
        }
    }
}
