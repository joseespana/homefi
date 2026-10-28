//! Lossless FLAC-in-MP4 → native FLAC remux, in pure Rust.
//!
//! Tidal serves Hi-Res tracks as (fragmented) MP4 with FLAC frames inside.
//! Every MP4 sample is one complete FLAC frame and the `dfLa` box holds the
//! FLAC metadata blocks, so a native `.flac` is just `fLaC` + those blocks +
//! the frames in order. No decoding, no ffmpeg; bit-identical audio.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::paths::with_suffix;

fn err(msg: impl Into<String>) -> Error {
    Error::Stream(format!("mp4 remux: {}", msg.into()))
}

#[derive(Clone, Copy)]
struct BoxRef {
    kind: [u8; 4],
    /// Offset of the box header.
    start: usize,
    /// Offset of the payload.
    body: usize,
    end: usize,
}

/// Iterates the boxes in `data[from..to]`.
fn boxes(data: &[u8], from: usize, to: usize) -> Result<Vec<BoxRef>> {
    let mut out = Vec::new();
    let mut pos = from;
    while pos + 8 <= to {
        let size32 = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = data[pos + 4..pos + 8].try_into().unwrap();
        let (size, header) = match size32 {
            0 => ((to - pos) as u64, 8),
            1 => {
                let large = data.get(pos + 8..pos + 16).ok_or_else(|| err("truncated box"))?;
                (u64::from_be_bytes(large.try_into().unwrap()), 16)
            }
            n => (n, 8),
        };
        let end = pos
            .checked_add(size as usize)
            .filter(|e| *e <= to && size >= header as u64)
            .ok_or_else(|| err("bad box size"))?;
        out.push(BoxRef { kind, start: pos, body: pos + header, end });
        pos = end;
    }
    Ok(out)
}

fn find(data: &[u8], parent: &BoxRef, kind: &[u8; 4]) -> Result<Option<BoxRef>> {
    Ok(boxes(data, parent.body, parent.end)?.into_iter().find(|b| &b.kind == kind))
}

fn path_to(data: &[u8], root: &BoxRef, kinds: &[&[u8; 4]]) -> Result<Option<BoxRef>> {
    let mut cur = *root;
    for kind in kinds {
        match find(data, &cur, kind)? {
            Some(b) => cur = b,
            None => return Ok(None),
        }
    }
    Ok(Some(cur))
}

fn u32_at(data: &[u8], pos: usize) -> Result<u32> {
    data.get(pos..pos + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or_else(|| err("truncated"))
}

fn u64_at(data: &[u8], pos: usize) -> Result<u64> {
    data.get(pos..pos + 8).map(|b| u64::from_be_bytes(b.try_into().unwrap())).ok_or_else(|| err("truncated"))
}

/// The FLAC track: its `dfLa` metadata blocks plus the sample table.
struct FlacTrack {
    metadata: Vec<u8>,
    track_id: u32,
    /// Non-fragmented files: (offset, size) of every sample.
    samples: Vec<(u64, u32)>,
    /// `trex` default sample size for fragments.
    default_size: u32,
}

/// Finds the first FLAC track in `moov`. `None` when the MP4 holds no FLAC.
fn flac_track(data: &[u8], moov: &BoxRef) -> Result<Option<FlacTrack>> {
    for trak in boxes(data, moov.body, moov.end)?.iter().filter(|b| &b.kind == b"trak") {
        let Some(stbl) = path_to(data, trak, &[b"mdia", b"minf", b"stbl"])? else { continue };
        let Some(stsd) = find(data, &stbl, b"stsd")? else { continue };
        // stsd: version/flags (4) + entry count (4), then sample entries
        let entries = boxes(data, stsd.body + 8, stsd.end)?;
        let Some(entry) = entries.iter().find(|e| &e.kind == b"fLaC") else { continue };
        // audio sample entry: 28 bytes of fields before child boxes
        let Some(dfla) = boxes(data, entry.body + 28, entry.end)?.into_iter().find(|b| &b.kind == b"dfLa") else {
            return Err(err("fLaC entry without dfLa"));
        };
        let mut metadata = data[dfla.body + 4..dfla.end].to_vec(); // skip version/flags
        mark_last_block(&mut metadata)?;

        let tkhd = find(data, trak, b"tkhd")?.ok_or_else(|| err("no tkhd"))?;
        let version = data[tkhd.body];
        let track_id = u32_at(data, tkhd.body + if version == 1 { 20 } else { 12 })?;

        let samples = sample_table(data, &stbl)?;
        let default_size = path_to(data, moov, &[b"mvex", b"trex"])?
            .map(|trex| u32_at(data, trex.body + 16))
            .transpose()?
            .unwrap_or(0);
        return Ok(Some(FlacTrack { metadata, track_id, samples, default_size }));
    }
    Ok(None)
}

/// Sets the "last metadata block" flag on the final block.
fn mark_last_block(metadata: &mut [u8]) -> Result<()> {
    let mut pos = 0;
    let mut last = None;
    while pos + 4 <= metadata.len() {
        last = Some(pos);
        let len = u32::from_be_bytes([0, metadata[pos + 1], metadata[pos + 2], metadata[pos + 3]]) as usize;
        metadata[pos] &= 0x7F;
        pos += 4 + len;
    }
    let last = last.filter(|_| pos == metadata.len()).ok_or_else(|| err("malformed dfLa metadata"))?;
    metadata[last] |= 0x80;
    Ok(())
}

/// (offset, size) of every sample from `stsz` / `stsc` / `stco` / `co64`.
fn sample_table(data: &[u8], stbl: &BoxRef) -> Result<Vec<(u64, u32)>> {
    let (Some(stsz), Some(stsc)) = (find(data, stbl, b"stsz")?, find(data, stbl, b"stsc")?) else {
        return Ok(Vec::new());
    };
    let fixed = u32_at(data, stsz.body + 4)?;
    let count = u32_at(data, stsz.body + 8)? as usize;
    let sizes: Vec<u32> = if fixed != 0 {
        vec![fixed; count]
    } else {
        (0..count).map(|i| u32_at(data, stsz.body + 12 + i * 4)).collect::<Result<_>>()?
    };
    if sizes.is_empty() {
        return Ok(Vec::new());
    }

    let chunks: Vec<u64> = if let Some(stco) = find(data, stbl, b"stco")? {
        let n = u32_at(data, stco.body + 4)? as usize;
        (0..n).map(|i| u32_at(data, stco.body + 8 + i * 4).map(u64::from)).collect::<Result<_>>()?
    } else if let Some(co64) = find(data, stbl, b"co64")? {
        let n = u32_at(data, co64.body + 4)? as usize;
        (0..n).map(|i| u64_at(data, co64.body + 8 + i * 8)).collect::<Result<_>>()?
    } else {
        return Err(err("no chunk offsets"));
    };

    // stsc: (first_chunk, samples_per_chunk, description) runs
    let runs = u32_at(data, stsc.body + 4)? as usize;
    let table: Vec<(u32, u32)> = (0..runs)
        .map(|i| Ok((u32_at(data, stsc.body + 8 + i * 12)?, u32_at(data, stsc.body + 12 + i * 12)?)))
        .collect::<Result<_>>()?;

    let mut out = Vec::with_capacity(sizes.len());
    let mut sample = 0;
    for (chunk_index, chunk_offset) in chunks.iter().enumerate() {
        let chunk_no = chunk_index as u32 + 1;
        let per_chunk = table.iter().rev().find(|(first, _)| *first <= chunk_no).map(|(_, n)| *n).unwrap_or(0);
        let mut offset = *chunk_offset;
        for _ in 0..per_chunk {
            let Some(size) = sizes.get(sample) else { break };
            out.push((offset, *size));
            offset += *size as u64;
            sample += 1;
        }
    }
    Ok(out)
}

/// Appends the samples of one `moof` fragment of `track_id`.
fn fragment_samples(data: &[u8], moof: &BoxRef, track: &FlacTrack, out: &mut Vec<u8>) -> Result<()> {
    for traf in boxes(data, moof.body, moof.end)?.iter().filter(|b| &b.kind == b"traf") {
        let tfhd = find(data, traf, b"tfhd")?.ok_or_else(|| err("traf without tfhd"))?;
        let flags = u32_at(data, tfhd.body)? & 0x00FF_FFFF;
        if u32_at(data, tfhd.body + 4)? != track.track_id {
            continue;
        }
        let mut pos = tfhd.body + 8;
        let mut base = moof.start as u64; // default-base-is-moof / no base offset
        if flags & 0x01 != 0 {
            base = u64_at(data, pos)?;
            pos += 8;
        }
        if flags & 0x02 != 0 {
            pos += 4; // sample description index
        }
        if flags & 0x08 != 0 {
            pos += 4; // default duration
        }
        let default_size = if flags & 0x10 != 0 { u32_at(data, pos)? } else { track.default_size };

        let mut next_offset = None;
        for trun in boxes(data, traf.body, traf.end)?.iter().filter(|b| &b.kind == b"trun") {
            let tflags = u32_at(data, trun.body)? & 0x00FF_FFFF;
            let count = u32_at(data, trun.body + 4)? as usize;
            let mut p = trun.body + 8;
            let mut offset = next_offset.unwrap_or(base);
            if tflags & 0x01 != 0 {
                offset = (base as i64 + u32_at(data, p)? as i32 as i64) as u64;
                p += 4;
            }
            if tflags & 0x04 != 0 {
                p += 4; // first sample flags
            }
            let per_sample = [0x100, 0x200, 0x400, 0x800].iter().filter(|f| tflags & **f != 0).count() * 4;
            for i in 0..count {
                let entry = p + i * per_sample;
                let size = if tflags & 0x200 != 0 {
                    let skip = if tflags & 0x100 != 0 { 4 } else { 0 };
                    u32_at(data, entry + skip)?
                } else {
                    default_size
                };
                let (start, end) = (offset as usize, offset as usize + size as usize);
                out.extend_from_slice(data.get(start..end).ok_or_else(|| err("sample outside the file"))?);
                offset += size as u64;
            }
            next_offset = Some(offset);
        }
    }
    Ok(())
}

/// Converts FLAC-in-MP4 bytes into a native FLAC stream.
/// `Ok(None)` when `data` is not an MP4 with a FLAC track (e.g. AAC).
pub fn flac_from_mp4_bytes(data: &[u8]) -> Result<Option<Vec<u8>>> {
    if data.len() < 12 || !matches!(&data[4..8], b"ftyp" | b"styp" | b"moov") {
        return Ok(None);
    }
    let top = boxes(data, 0, data.len())?;
    let Some(moov) = top.iter().find(|b| &b.kind == b"moov") else { return Ok(None) };
    let Some(track) = flac_track(data, moov)? else { return Ok(None) };

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(b"fLaC");
    out.extend_from_slice(&track.metadata);

    let audio_start = out.len();
    for (offset, size) in &track.samples {
        let (start, end) = (*offset as usize, *offset as usize + *size as usize);
        out.extend_from_slice(data.get(start..end).ok_or_else(|| err("sample outside the file"))?);
    }
    for moof in top.iter().filter(|b| &b.kind == b"moof") {
        fragment_samples(data, moof, &track, &mut out)?;
    }

    if out.len() == audio_start {
        return Err(err("no FLAC frames found"));
    }
    // every FLAC frame starts with the 0xFFF8/0xFFF9 sync code
    if !(out[audio_start] == 0xFF && out[audio_start + 1] & 0xFE == 0xF8) {
        return Err(err("samples are not FLAC frames"));
    }
    Ok(Some(out))
}

/// Remuxes `path` (FLAC inside MP4) into `<path>.flac` and removes the source.
/// Returns `None`, leaving the file untouched, when it holds no FLAC track.
pub fn flac_from_mp4(path: &Path) -> Result<Option<PathBuf>> {
    let data = fs::read(path)?;
    let Some(flac) = flac_from_mp4_bytes(&data)? else { return Ok(None) };
    let target = with_suffix(path, "flac");
    let tmp = with_suffix(path, "remux.flac");
    fs::write(&tmp, flac)?;
    fs::rename(&tmp, &target)?;
    if target != path {
        let _ = fs::remove_file(path);
    }
    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_mp4_is_ignored() {
        assert!(flac_from_mp4_bytes(b"fLaC\0\0\0\0\0\0\0\0").unwrap().is_none());
        assert!(flac_from_mp4_bytes(b"short").unwrap().is_none());
    }

    #[test]
    fn last_block_flag() {
        // STREAMINFO (34 bytes) + PADDING (2 bytes), neither marked last
        let mut meta = vec![0x00, 0, 0, 34];
        meta.extend([0u8; 34]);
        meta.extend([0x81, 0, 0, 2, 0, 0]);
        mark_last_block(&mut meta).unwrap();
        assert_eq!(meta[0], 0x00);
        assert_eq!(meta[38], 0x81);
        assert!(mark_last_block(&mut [0x00, 0, 0, 10, 1]).is_err());
    }
}
