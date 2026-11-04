//! CD-Text: titles and performers stored on the disc itself, read as raw 18-byte packs
//! (SCSI READ TOC format 5 / `DKIOCCDREADTOC` format 5) and parsed here.

use serde::Serialize;

/// Size of one CD-Text pack: 4 header bytes, 12 text bytes, 2 CRC bytes.
const PACK_BYTES: usize = 18;
/// Header of the READ TOC response: data length (2 bytes) and 2 reserved.
const HEADER_BYTES: usize = 4;
const TEXT_BYTES: usize = 12;

const PACK_TITLE: u8 = 0x80;
const PACK_PERFORMER: u8 = 0x81;
const PACK_GENRE: u8 = 0x87;
const PACK_UPC_ISRC: u8 = 0x8E;
const PACK_SIZE_INFO: u8 = 0x8F;
/// Highest CD track number.
const MAX_TRACK: usize = 99;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdTextTrack {
    pub number: u8,
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdText {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    /// UPC/EAN of the disc when present.
    pub upc: Option<String>,
    pub tracks: Vec<CdTextTrack>,
}

impl CdText {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.artist.is_none() && self.tracks.is_empty()
    }
}

/// Strings of one pack type: index 0 is the album, 1.. are tracks.
fn strings_of(packs: &[&[u8]], kind: u8) -> Vec<Option<String>> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut first_track: Option<usize> = None;
    for pack in packs.iter().filter(|pack| pack[0] == kind) {
        // Only the first language block; byte 3 holds block number (bits 4-6).
        if (pack[3] >> 4) & 0x07 != 0 {
            continue;
        }
        first_track.get_or_insert(usize::from(pack[1] & 0x7F));
        bytes.extend_from_slice(&pack[4..4 + TEXT_BYTES]);
    }
    let Some(first) = first_track else {
        return Vec::new();
    };
    let mut out: Vec<Option<String>> = vec![None; first.min(MAX_TRACK)];
    // Strings end with NUL; the bytes after the last NUL are padding.
    let mut parts: Vec<&[u8]> = bytes.split(|b| *b == 0).collect();
    parts.pop();
    for part in parts.into_iter().take(MAX_TRACK + 1 - first.min(MAX_TRACK)) {
        out.push(Some(latin1(part)).filter(|text| !text.is_empty()));
    }
    // The last pack is padded with NULs that look like empty strings.
    while out.last().is_some_and(Option::is_none) {
        out.pop();
    }
    out
}

/// ISO 8859-1 maps byte for byte onto the first 256 Unicode code points.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect::<String>().trim().to_owned()
}

/// Parses the response of READ TOC format 5 (with or without the 4-byte header).
/// `None` when the data holds no usable text. Text in double-byte charsets (MS-JIS) is not
/// decoded: such discs yield `None`.
pub fn parse_cd_text(raw: &[u8]) -> Option<CdText> {
    let body = if raw.len() >= HEADER_BYTES && (raw.len() - HEADER_BYTES) % PACK_BYTES == 0 {
        &raw[HEADER_BYTES..]
    } else if raw.len() % PACK_BYTES == 0 {
        raw
    } else {
        // A truncated last pack is dropped.
        let body = if raw.len() >= HEADER_BYTES { &raw[HEADER_BYTES..] } else { return None };
        &body[..body.len() / PACK_BYTES * PACK_BYTES]
    };
    let packs: Vec<&[u8]> = body.chunks_exact(PACK_BYTES).collect();
    if packs.is_empty() {
        return None;
    }
    // Size-info pack, first block: byte 4 is the character code (0 = ISO 8859-1, 1 = ASCII).
    if let Some(size) = packs.iter().find(|p| p[0] == PACK_SIZE_INFO && (p[3] >> 4) & 0x07 == 0) {
        if size[4] == 0x80 {
            return None;
        }
    }
    let titles = strings_of(&packs, PACK_TITLE);
    let artists = strings_of(&packs, PACK_PERFORMER);
    let genres = strings_of(&packs, PACK_GENRE);
    let codes = strings_of(&packs, PACK_UPC_ISRC);
    let at = |list: &[Option<String>], i: usize| list.get(i).cloned().flatten();
    // A tab means "same as the album" (performer repeated for every track).
    let clean = |text: Option<String>| text.filter(|t| t != "\t");
    let track_count = titles.len().max(artists.len()).saturating_sub(1);
    let tracks: Vec<CdTextTrack> = (1..=track_count)
        .map(|n| CdTextTrack { number: n as u8, title: clean(at(&titles, n)), artist: clean(at(&artists, n)) })
        .collect();
    let text = CdText {
        title: clean(at(&titles, 0)),
        artist: clean(at(&artists, 0)),
        genre: at(&genres, 0),
        upc: at(&codes, 0).filter(|code| code.chars().all(|c| c.is_ascii_digit())),
        tracks,
    };
    (!text.is_empty()).then_some(text)
}

/// Builds raw packs for tests and mock discs (also used by [`crate::ImageSource`]).
pub fn build_packs(text: &CdText) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seq = 0u8;
    let mut emit = |out: &mut Vec<u8>, kind: u8, strings: &[Option<&str>]| {
        let mut stream: Vec<u8> = Vec::new();
        for s in strings {
            stream.extend(s.unwrap_or("").chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }));
            stream.push(0);
        }
        let mut track = 0u8;
        for chunk in stream.chunks(TEXT_BYTES) {
            let mut pack = [0u8; PACK_BYTES];
            pack[0] = kind;
            pack[1] = track;
            pack[2] = seq;
            seq = seq.wrapping_add(1);
            pack[4..4 + chunk.len()].copy_from_slice(chunk);
            // Strings that finish inside this pack advance the track counter for the next.
            track = track.saturating_add(chunk.iter().filter(|b| **b == 0).count() as u8);
            out.extend_from_slice(&pack);
        }
    };
    let titles: Vec<Option<&str>> =
        std::iter::once(text.title.as_deref()).chain(text.tracks.iter().map(|t| t.title.as_deref())).collect();
    let artists: Vec<Option<&str>> =
        std::iter::once(text.artist.as_deref()).chain(text.tracks.iter().map(|t| t.artist.as_deref())).collect();
    emit(&mut out, PACK_TITLE, &titles);
    emit(&mut out, PACK_PERFORMER, &artists);
    let mut size = [0u8; PACK_BYTES];
    size[0] = PACK_SIZE_INFO;
    size[2] = seq;
    out.extend_from_slice(&size);
    let mut framed = vec![0u8; HEADER_BYTES];
    framed[..2].copy_from_slice(&((out.len() + 2) as u16).to_be_bytes());
    framed.extend_from_slice(&out);
    framed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CdText {
        CdText {
            title: Some("Café del Mar".into()),
            artist: Some("Various Artists".into()),
            tracks: vec![
                CdTextTrack {
                    number: 1,
                    title: Some("Sunset on the Beach, a long title".into()),
                    artist: Some("Ana".into()),
                },
                CdTextTrack { number: 2, title: Some("Niño".into()), artist: None },
            ],
            ..CdText::default()
        }
    }

    #[test]
    fn parses_multi_pack_strings_and_latin1() {
        let parsed = parse_cd_text(&build_packs(&sample())).expect("text");
        assert_eq!(parsed.title.as_deref(), Some("Café del Mar"));
        assert_eq!(parsed.artist.as_deref(), Some("Various Artists"));
        assert_eq!(parsed.tracks[0].title.as_deref(), Some("Sunset on the Beach, a long title"));
        assert_eq!(parsed.tracks[0].artist.as_deref(), Some("Ana"));
        assert_eq!(parsed.tracks[1].title.as_deref(), Some("Niño"));
        assert_eq!(parsed.tracks[1].artist, None);
    }

    #[test]
    fn accepts_data_without_the_header() {
        let framed = build_packs(&sample());
        assert!(parse_cd_text(&framed[4..]).is_some());
    }

    #[test]
    fn tab_means_same_as_album() {
        let mut text = sample();
        text.tracks[0].artist = Some("\t".into());
        assert_eq!(parse_cd_text(&build_packs(&text)).unwrap().tracks[0].artist, None);
    }

    #[test]
    fn nothing_usable_is_none() {
        assert!(parse_cd_text(&[]).is_none());
        assert!(parse_cd_text(&[0, 2, 0, 0]).is_none());
        assert!(parse_cd_text(&build_packs(&CdText::default())).is_none());
    }

    #[test]
    fn double_byte_charsets_are_not_guessed() {
        let mut bytes = build_packs(&sample());
        let size = bytes.len() - PACK_BYTES;
        bytes[size + 4] = 0x80;
        assert!(parse_cd_text(&bytes).is_none());
    }
}
