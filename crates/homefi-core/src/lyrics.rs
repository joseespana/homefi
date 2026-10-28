//! Song lyrics for the players: from Tidal (Musixmatch, usually synced line by
//! line) or from a local file (a `.lrc` next to it, or the lyrics tag inside).
//! Synced lyrics come as LRC: `[mm:ss.xx] text`.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::prelude::ItemKey;
use serde::Serialize;

use crate::api::TidalApi;
use crate::error::{Error, Result};
use crate::models::TrackLyrics;

/// One line of a song; `time_ms` is when it starts (None when not synced).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    /// Every line has a start time, so the player can follow along.
    pub synced: bool,
    /// Who provided them (Tidal says e.g. `MUSIXMATCH`); None for local files.
    pub provider: Option<String>,
}

impl Lyrics {
    /// Lyrics from text that may be LRC (synced) or plain.
    pub fn parse(text: &str, provider: Option<String>) -> Option<Lyrics> {
        let synced = parse_lrc(text);
        let lines = if synced.is_empty() {
            text.lines().map(|l| LyricLine { time_ms: None, text: l.trim().to_owned() }).collect::<Vec<_>>()
        } else {
            synced
        };
        let lines = trim_blank_ends(lines);
        if lines.is_empty() {
            return None;
        }
        let synced = lines.iter().all(|l| l.time_ms.is_some());
        Some(Lyrics { lines, synced, provider })
    }

    /// Tidal's answer: the synced version when there is one, else the plain text.
    pub fn from_tidal(l: &TrackLyrics) -> Option<Lyrics> {
        let provider = l.lyrics_provider.clone().filter(|p| !p.is_empty());
        l.subtitles
            .as_deref()
            .and_then(|s| Lyrics::parse(s, provider.clone()).filter(|l| l.synced))
            .or_else(|| l.lyrics.as_deref().and_then(|t| Lyrics::parse(t, provider)))
    }

    /// The line being sung at `position_ms` (None before the first one or when not synced).
    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        let after = self.lines.partition_point(|l| l.time_ms.unwrap_or(0) <= position_ms);
        after.checked_sub(1)
    }
}

/// Lyrics of a Tidal track; None when Tidal has none (instrumentals answer 404).
pub async fn tidal(api: &TidalApi, track_id: i64) -> Result<Option<Lyrics>> {
    match api.track_lyrics(track_id).await {
        Ok(l) => Ok(Lyrics::from_tidal(&l)),
        Err(Error::Api { status: 404, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Lyrics of a local file: `<same name>.lrc` next to it first, then the lyrics tag.
/// Blocking (disk and tag parsing).
pub fn local(path: &Path) -> Option<Lyrics> {
    local_and_isrc(path).0
}

/// The file's own lyrics and, when it has none, its ISRC: one read of the tags.
fn local_and_isrc(path: &Path) -> (Option<Lyrics>, Option<String>) {
    let lrc = path.with_extension("lrc");
    if let Some(l) = std::fs::read_to_string(&lrc).ok().and_then(|t| Lyrics::parse(&t, None)) {
        return (Some(l), None);
    }
    let Some(file) = read_tags(path) else { return (None, None) };
    let text = file.tags().iter().find_map(|t| t.get_string(&ItemKey::Lyrics).map(str::to_owned));
    match text.and_then(|t| Lyrics::parse(&t, None)) {
        Some(l) => (Some(l), None),
        None => (None, isrc_in(&file)),
    }
}

/// The tags only: no cover art (megabytes in some files) and no audio properties.
fn read_tags(path: &Path) -> Option<lofty::file::TaggedFile> {
    use lofty::config::ParseOptions;
    lofty::probe::Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false).read_properties(false))
        .guess_file_type()
        .ok()?
        .read()
        .ok()
}

/// Lyrics of a local file: its own (see [`local`]) or, when it has none, Tidal's
/// for the same recording, found by the ISRC in its tags (when signed in).
pub async fn for_file(api: Option<&TidalApi>, path: &Path) -> Result<Option<Lyrics>> {
    let file = path.to_path_buf();
    let (found, isrc) = tokio::task::spawn_blocking(move || local_and_isrc(&file)).await.unwrap_or_default();
    if let Some(found) = found {
        return Ok(Some(found));
    }
    let (Some(api), Some(isrc)) = (api, isrc) else { return Ok(None) };
    // the same recording is often on several releases; any of them has the lyrics
    for track in api.tracks_by_isrc(&isrc).await?.iter().take(3) {
        if let Some(found) = tidal(api, track.id).await? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The ISRC in a file's tags.
fn isrc_in(file: &lofty::file::TaggedFile) -> Option<String> {
    file.tags()
        .iter()
        .find_map(|t| t.get_string(&ItemKey::Isrc))
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
}

/// The timed lines of an LRC text, in order. Lines with several stamps
/// (`[00:10.00][01:20.00] chorus`) appear once per stamp; `[offset:±ms]` is applied.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut offset: i64 = 0;
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(body) = rest.strip_prefix('[') {
            let Some(end) = body.find(']') else { break };
            let tag = &body[..end];
            rest = body[end + 1..].trim_start();
            if let Some(ms) = stamp(tag) {
                stamps.push(ms);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            }
        }
        for ms in stamps {
            // a positive offset shows the lyrics sooner
            let time = (ms as i64 - offset).max(0) as u64;
            out.push(LyricLine { time_ms: Some(time), text: rest.trim().to_owned() });
        }
    }
    out.sort_by_key(|l| l.time_ms);
    out
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss.xxx` in milliseconds.
fn stamp(tag: &str) -> Option<u64> {
    let (min, sec) = tag.split_once(':')?;
    let min: u64 = min.trim().parse().ok()?;
    let (s, frac) = sec.split_once(['.', ':']).unwrap_or((sec, ""));
    let s: u64 = s.trim().parse().ok()?;
    if s >= 60 {
        return None;
    }
    let frac_ms = match frac.len() {
        0 => 0,
        1 => frac.parse::<u64>().ok()? * 100,
        2 => frac.parse::<u64>().ok()? * 10,
        _ => frac[..3].parse::<u64>().ok()?,
    };
    Some(min * 60_000 + s * 1000 + frac_ms)
}

fn trim_blank_ends(mut lines: Vec<LyricLine>) -> Vec<LyricLine> {
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    let first = lines.iter().position(|l| !l.text.is_empty() || l.time_ms.is_some()).unwrap_or(lines.len());
    lines.drain(..first);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_lines_are_timed_and_sorted() {
        let l = parse_lrc("[ar:Someone]\n[00:29.47] I don't wanna say\n[01:02.5][00:10.00] Chorus\n[00:40] \n");
        let times: Vec<_> = l.iter().map(|l| l.time_ms.unwrap()).collect();
        assert_eq!(times, [10_000, 29_470, 40_000, 62_500]);
        assert_eq!(l[0].text, "Chorus");
        assert_eq!(l[2].text, "");
    }

    #[test]
    fn offset_moves_every_line() {
        let l = parse_lrc("[offset:+500]\n[00:10.00]a\n[00:00.20]b");
        assert_eq!(l.iter().map(|l| l.time_ms.unwrap()).collect::<Vec<_>>(), [0, 9_500]);
    }

    #[test]
    fn plain_text_is_not_synced() {
        let l = Lyrics::parse("\nFirst line\nSecond line\n\n", None).unwrap();
        assert!(!l.synced);
        assert_eq!(l.lines.len(), 2);
        assert_eq!(l.line_at(5_000), None);
    }

    #[test]
    fn tidal_prefers_the_synced_version() {
        let t = TrackLyrics {
            lyrics: Some("Hello\nWorld".into()),
            subtitles: Some("[00:01.00] Hello\n[00:03.00] World".into()),
            lyrics_provider: Some("MUSIXMATCH".into()),
        };
        let l = Lyrics::from_tidal(&t).unwrap();
        assert!(l.synced);
        assert_eq!(l.provider.as_deref(), Some("MUSIXMATCH"));
        assert_eq!(l.line_at(500), None);
        assert_eq!(l.line_at(1_000), Some(0));
        assert_eq!(l.line_at(2_999), Some(0));
        assert_eq!(l.line_at(60_000), Some(1));

        let plain = TrackLyrics { lyrics: Some("Only text".into()), ..Default::default() };
        assert!(!Lyrics::from_tidal(&plain).unwrap().synced);
        assert!(Lyrics::from_tidal(&TrackLyrics::default()).is_none());
    }

    #[test]
    fn a_sidecar_lrc_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("song.flac");
        std::fs::write(&song, b"not audio").unwrap();
        assert!(local(&song).is_none());
        std::fs::write(dir.path().join("song.lrc"), "[00:01.00] La la").unwrap();
        let l = local(&song).unwrap();
        assert!(l.synced);
        assert_eq!(l.lines[0].text, "La la");
    }
}
