//! A `.cue` sheet for the ripped files (one file per track), for people who keep one.

use std::fmt::Write;

use crate::encode::RipFormat;

pub struct CueTrack<'a> {
    pub number: u8,
    pub title: &'a str,
    pub artist: &'a str,
    pub isrc: Option<&'a str>,
    /// File name relative to the cue sheet.
    pub file: &'a str,
}

pub struct Cue<'a> {
    pub album: &'a str,
    pub artist: &'a str,
    pub genre: Option<&'a str>,
    pub year: Option<u16>,
    /// Media catalog number / UPC.
    pub catalog: Option<&'a str>,
    pub format: &'a RipFormat,
    pub tracks: &'a [CueTrack<'a>],
}

/// Quotes are not allowed inside cue strings.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "'"))
}

fn file_type(format: &RipFormat) -> &'static str {
    match format {
        RipFormat::Mp3 { .. } => "MP3",
        _ => "WAVE",
    }
}

pub fn render(cue: &Cue<'_>) -> String {
    let mut out = String::new();
    if let Some(genre) = cue.genre {
        let _ = writeln!(out, "REM GENRE {}", quoted(genre));
    }
    if let Some(year) = cue.year {
        let _ = writeln!(out, "REM DATE {year}");
    }
    let _ = writeln!(out, "PERFORMER {}", quoted(cue.artist));
    let _ = writeln!(out, "TITLE {}", quoted(cue.album));
    if let Some(catalog) = cue.catalog.filter(|c| !c.is_empty()) {
        let _ = writeln!(out, "CATALOG {catalog}");
    }
    for track in cue.tracks {
        let _ = writeln!(out, "FILE {} {}", quoted(track.file), file_type(cue.format));
        let _ = writeln!(out, "  TRACK {:02} AUDIO", track.number);
        let _ = writeln!(out, "    TITLE {}", quoted(track.title));
        let _ = writeln!(out, "    PERFORMER {}", quoted(track.artist));
        if let Some(isrc) = track.isrc.filter(|i| !i.is_empty()) {
            let _ = writeln!(out, "    ISRC {isrc}");
        }
        let _ = writeln!(out, "    INDEX 01 00:00:00");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_file_per_track_each_starting_at_zero() {
        let tracks = [
            CueTrack { number: 1, title: "Blind", artist: "Korn", isrc: Some("USSM19400001"), file: "Blind.flac" },
            CueTrack { number: 2, title: "Say \"hi\"", artist: "Korn", isrc: None, file: "Say hi.flac" },
        ];
        let text = render(&Cue {
            album: "Korn",
            artist: "Korn",
            genre: Some("Nu Metal"),
            year: Some(1994),
            catalog: Some("074646672229"),
            format: &RipFormat::default(),
            tracks: &tracks,
        });
        assert!(text.starts_with("REM GENRE \"Nu Metal\"\nREM DATE 1994\n"));
        assert!(text.contains("CATALOG 074646672229"));
        assert!(text.contains("FILE \"Blind.flac\" WAVE\n  TRACK 01 AUDIO"));
        assert!(text.contains("ISRC USSM19400001"));
        assert!(text.contains("TITLE \"Say 'hi'\""));
        assert_eq!(text.matches("INDEX 01 00:00:00").count(), 2);
    }
}
