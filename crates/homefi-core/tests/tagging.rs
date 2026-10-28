//! Writes tags into real files generated with ffmpeg and reads them back.
//! Skipped when ffmpeg is not installed.

use std::path::Path;
use std::process::Command;

use homefi_core::metadata::{write_track_tags, TrackTags};
use homefi_core::models::{Contributor, CreditsEntry};
use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::tag::ItemKey;

/// 1x1 white JPEG.
const TINY_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
    0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08,
    0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A,
    0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31, 0x34,
    0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00,
    0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
    0x00, 0x00, 0x3F, 0x00, 0x2A, 0x9F, 0xFF, 0xD9,
];

fn make_silence(path: &Path) -> bool {
    Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-t",
            "0.2",
        ])
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn tags() -> TrackTags {
    TrackTags {
        title: "One More Time (Radio Edit)".into(),
        track_number: 1,
        disc_number: 1,
        album_artist: "Daft Punk".into(),
        artists: vec!["Daft Punk".into(), "Romanthony".into()],
        album_title: "Discovery".into(),
        date: Some("2001-03-13".into()),
        isrc: Some("GBDUW0000053".into()),
        bpm: Some(123),
        genre: "House, Electronic".into(),
        comment: "review".into(),
        lyrics: Some("[00:01.00] one more time".into()),
        credits: vec![CreditsEntry {
            kind: "PRODUCER".into(),
            contributors: vec![Contributor { name: "Thomas Bangalter".into(), id: None }],
        }],
        cover: Some(TINY_JPEG.to_vec()),
        copyright: Some("(P) 2001 Virgin".into()),
        replay_gain: Some(homefi_core::dsp::ReplayGain {
            track_gain_db: Some(-9.43),
            track_peak: Some(0.999969),
            album_gain_db: Some(-8.9),
            album_peak: Some(1.0),
        }),
    }
}

/// MD5 of the decoded audio (container and tags excluded).
fn audio_md5(path: &Path) -> String {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-map", "0:a", "-f", "md5", "-"])
        .output()
        .unwrap();
    assert!(out.stderr.is_empty(), "decode errors in {}: {}", path.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn tags_roundtrip_in_every_format() {
    let dir = tempfile::tempdir().unwrap();

    for ext in ["flac", "m4a", "mp3"] {
        let path = dir.path().join(format!("song.{ext}"));
        if !make_silence(&path) {
            eprintln!("ffmpeg not available, skipping {ext}");
            return;
        }

        let before = audio_md5(&path);
        write_track_tags(&path, &tags()).unwrap_or_else(|e| panic!("{ext}: {e}"));
        assert_eq!(audio_md5(&path), before, "{ext}: tagging changed the audio");
        // rewriting must not grow or break anything either
        write_track_tags(&path, &tags()).unwrap_or_else(|e| panic!("{ext} (2nd): {e}"));
        assert_eq!(audio_md5(&path), before, "{ext}: second tagging changed the audio");

        let file = lofty::read_from_path(&path).unwrap();
        let tag = file.primary_tag().unwrap_or_else(|| panic!("{ext}: no tag"));
        assert_eq!(tag.title().as_deref(), Some("One More Time (Radio Edit)"), "{ext}");
        assert_eq!(tag.album().as_deref(), Some("Discovery"), "{ext}");
        assert_eq!(tag.track(), Some(1), "{ext}");
        assert_eq!(tag.get_string(&ItemKey::AlbumArtist), Some("Daft Punk"), "{ext}");
        assert_eq!(tag.genre().as_deref(), Some("House, Electronic"), "{ext}");
        assert!(tag.get_string(&ItemKey::RecordingDate).is_some_and(|d| d.starts_with("2001")), "{ext}");
        assert!(tag.artist().is_some_and(|a| a.contains("Daft Punk")), "{ext}");

        assert_eq!(tag.pictures().len(), 1, "{ext}: cover");
        // loudness normalization reads the ReplayGain back
        let rg = homefi_core::dsp::read_replay_gain(&path).unwrap_or_else(|| panic!("{ext}: no ReplayGain"));
        assert_eq!(rg.track_gain_db, Some(-9.43), "{ext}");
        assert_eq!(rg.album_gain_db, Some(-8.9), "{ext}");
        assert_eq!(rg.track_peak, Some(0.999969), "{ext}");

        if ext == "flac" {
            let artists: Vec<_> = tag.get_strings(&ItemKey::TrackArtist).collect();
            assert_eq!(artists, ["Daft Punk", "Romanthony"]);
            assert_eq!(tag.get_string(&ItemKey::Producer), Some("Thomas Bangalter"));
        }
    }
}
