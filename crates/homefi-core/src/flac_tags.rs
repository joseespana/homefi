//! FLAC metadata writer that never touches the audio frames.
//!
//! Rewrites only the metadata blocks (STREAMINFO and SEEKTABLE etc. are kept,
//! VORBIS_COMMENT / PICTURE / PADDING are replaced) and copies the frames
//! byte for byte, so the decoded audio is bit-identical to the source.

use std::path::Path;

use crate::error::{Error, Result};

const PADDING: usize = 8 * 1024;

const BLOCK_PADDING: u8 = 1;
const BLOCK_VORBIS_COMMENT: u8 = 4;
const BLOCK_PICTURE: u8 = 6;

fn err(path: &Path, msg: impl Into<String>) -> Error {
    Error::Metadata { path: path.to_path_buf(), message: msg.into() }
}

/// `(width, height)` of a JPEG or PNG, `(0, 0)` when unknown.
pub fn image_size(data: &[u8]) -> (u32, u32) {
    if data.len() >= 24 && data.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes(data[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(data[20..24].try_into().unwrap());
        return (w, h);
    }
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < data.len() {
            if data[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = data[i + 1];
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            // SOF0..SOF15 except DHT (C4), JPG (C8), DAC (CC)
            if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return (w, h);
            }
            i += 2 + len;
        }
    }
    (0, 0)
}

fn mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG") {
        "image/png"
    } else {
        "image/jpeg"
    }
}

fn vorbis_block(comments: &[(String, String)]) -> Vec<u8> {
    let vendor = concat!("homefi ", env!("CARGO_PKG_VERSION"));
    let mut b = Vec::new();
    b.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    b.extend_from_slice(vendor.as_bytes());
    let entries: Vec<String> =
        comments.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{}={v}", k.to_uppercase())).collect();
    b.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for e in entries {
        b.extend_from_slice(&(e.len() as u32).to_le_bytes());
        b.extend_from_slice(e.as_bytes());
    }
    b
}

fn picture_block(data: &[u8]) -> Vec<u8> {
    let (w, h) = image_size(data);
    let mime = mime(data);
    let desc = b"Cover";
    let mut b = Vec::with_capacity(data.len() + 64);
    b.extend_from_slice(&3u32.to_be_bytes()); // front cover
    b.extend_from_slice(&(mime.len() as u32).to_be_bytes());
    b.extend_from_slice(mime.as_bytes());
    b.extend_from_slice(&(desc.len() as u32).to_be_bytes());
    b.extend_from_slice(desc);
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&24u32.to_be_bytes()); // colour depth
    b.extend_from_slice(&0u32.to_be_bytes()); // indexed colours
    b.extend_from_slice(&(data.len() as u32).to_be_bytes());
    b.extend_from_slice(data);
    b
}

/// Replaces the Vorbis comments (and, when `cover` is given, the pictures)
/// of the FLAC file at `path`. Existing pictures are kept when `cover` is `None`.
pub fn write(path: &Path, comments: &[(String, String)], cover: Option<&[u8]>) -> Result<()> {
    rewrite(path, Some(comments), cover)
}

/// Replaces only the front cover; tags and audio stay as they are.
pub fn set_cover(path: &Path, cover: &[u8]) -> Result<()> {
    rewrite(path, None, Some(cover))
}

fn rewrite(path: &Path, comments: Option<&[(String, String)]>, cover: Option<&[u8]>) -> Result<()> {
    let data = std::fs::read(path)?;
    if !data.starts_with(b"fLaC") {
        return Err(err(path, "not a FLAC file"));
    }

    let mut pos = 4;
    let mut kept: Vec<(u8, &[u8])> = Vec::new();
    loop {
        let header = *data.get(pos).ok_or_else(|| err(path, "truncated metadata"))?;
        let kind = header & 0x7F;
        let len = data
            .get(pos + 1..pos + 4)
            .map(|b| u32::from_be_bytes([0, b[0], b[1], b[2]]) as usize)
            .ok_or_else(|| err(path, "truncated metadata"))?;
        let body = data.get(pos + 4..pos + 4 + len).ok_or_else(|| err(path, "metadata block outside the file"))?;
        let drop = kind == BLOCK_PADDING
            || (kind == BLOCK_VORBIS_COMMENT && comments.is_some())
            || (kind == BLOCK_PICTURE && cover.is_some());
        if !drop {
            kept.push((kind, body));
        }
        pos += 4 + len;
        if header & 0x80 != 0 {
            break;
        }
    }
    if kept.first().map(|(k, _)| *k) != Some(0) {
        return Err(err(path, "STREAMINFO is not the first block"));
    }
    let audio = &data[pos..];

    let vorbis = comments.map(vorbis_block);
    let picture = cover.map(picture_block);
    let padding = vec![0u8; PADDING];

    let mut blocks: Vec<(u8, &[u8])> = kept;
    if let Some(v) = &vorbis {
        blocks.push((BLOCK_VORBIS_COMMENT, v));
    }
    if let Some(p) = &picture {
        blocks.push((BLOCK_PICTURE, p));
    }
    blocks.push((BLOCK_PADDING, &padding));

    let mut out = Vec::with_capacity(data.len() + PADDING + cover.map_or(0, |c| c.len()));
    out.extend_from_slice(b"fLaC");
    let last = blocks.len() - 1;
    for (i, (kind, body)) in blocks.iter().enumerate() {
        if body.len() >= 1 << 24 {
            return Err(err(path, "metadata block too large"));
        }
        out.push(if i == last { kind | 0x80 } else { *kind });
        out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(body);
    }
    out.extend_from_slice(audio);

    let tmp = path.with_extension("tags.tmp");
    std::fs::write(&tmp, out)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_flac() -> Vec<u8> {
        let mut f = b"fLaC".to_vec();
        f.extend([0x00, 0, 0, 34]);
        f.extend([0xAB; 34]); // fake STREAMINFO
        f.extend([BLOCK_VORBIS_COMMENT | 0x80, 0, 0, 8]);
        f.extend([4, 0, 0, 0, b'o', b'l', b'd', b'!']); // garbage old comment
        f.extend([0xFF, 0xF8, 1, 2, 3, 4, 5]); // "audio"
        f
    }

    #[test]
    fn audio_is_untouched_and_blocks_are_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.flac");
        std::fs::write(&path, minimal_flac()).unwrap();
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x02\0\0\0\x01\0";
        write(&path, &[("title".into(), "Héllo".into()), ("artist".into(), "A".into())], Some(png)).unwrap();

        let out = std::fs::read(&path).unwrap();
        assert!(out.ends_with(&[0xFF, 0xF8, 1, 2, 3, 4, 5]));
        assert_eq!(&out[4..8], &[0x00, 0, 0, 34]);
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("TITLE=Héllo"));
        assert!(!text.contains("old!"));
        assert!(text.contains("image/png"));
        assert_eq!(image_size(png), (512, 256));

        // replacing only the cover keeps the comments
        set_cover(&path, b"\xFF\xD8\xFF\xE0jpeg").unwrap();
        let out = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("TITLE=H"));
        assert!(text.contains("image/jpeg") && !text.contains("image/png"));
        assert!(out.ends_with(&[0xFF, 0xF8, 1, 2, 3, 4, 5]));
    }
}
