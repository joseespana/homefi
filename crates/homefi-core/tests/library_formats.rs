//! Scans real files of every playable format (generated with ffmpeg) and
//! checks the detected format, properties, tags and the scan cache.
//! Skipped when ffmpeg is not installed.

use std::path::Path;
use std::process::Command;

use homefi_core::library::{read_cover, scan};

/// (file name, ffmpeg codec args, sample rate, expected format, expected bit depth)
type Case = (&'static str, &'static [&'static str], u32, &'static str, Option<u8>);

const CASES: &[Case] = &[
    ("01 flac16.flac", &["-c:a", "flac", "-sample_fmt", "s16"], 44_100, "FLAC", Some(16)),
    ("02 flac24.flac", &["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"], 96_000, "FLAC", Some(24)),
    ("03 mp3.mp3", &["-c:a", "libmp3lame", "-b:a", "320k"], 44_100, "MP3", None),
    ("04 wav16.wav", &["-c:a", "pcm_s16le"], 44_100, "WAV", Some(16)),
    ("05 wav24.wav", &["-c:a", "pcm_s24le"], 48_000, "WAV", Some(24)),
    ("06 aac.m4a", &["-c:a", "aac", "-b:a", "256k"], 44_100, "AAC", None),
    ("07 alac.m4a", &["-c:a", "alac"], 44_100, "ALAC", Some(16)),
    ("08 vorbis.ogg", &["-c:a", "vorbis", "-strict", "-2"], 44_100, "Vorbis", None),
    ("09 opus.opus", &["-c:a", "libopus", "-b:a", "128k"], 48_000, "Opus", None),
    // what Tidal serves for Hi-Res when ffmpeg is not available (Android)
    (
        "10 hires.m4a",
        &["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24", "-strict", "-2", "-f", "mp4"],
        192_000,
        "FLAC",
        Some(24),
    ),
];

fn ffmpeg(out: &Path, codec: &[&str], rate: u32, title: &str) -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
        .arg("-i")
        .arg(format!("sine=frequency=440:sample_rate={rate}:duration=1"))
        .args(["-ac", "2"])
        .args(codec)
        .args(["-metadata", &format!("title={title}"), "-metadata", "artist=HomeFi Test"])
        .args(["-metadata", "album=Formats", "-metadata", "track=3", "-metadata", "date=2024"])
        .arg(out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn scans_every_playable_format() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("Music/Album");
    std::fs::create_dir_all(&music).unwrap();

    let mut expected = Vec::new();
    for (name, codec, rate, format, bits) in CASES {
        let path = music.join(name);
        if !ffmpeg(&path, codec, *rate, name) {
            eprintln!("ffmpeg (or its {format} encoder) not available, skipping {name}");
            continue;
        }
        expected.push((path, *rate, *format, *bits));
    }
    if expected.is_empty() {
        return;
    }
    // playable but not parseable by lofty: listed with file-name metadata
    let mka = music.join("11 matroska.mka");
    if ffmpeg(&mka, &["-c:a", "flac"], 44_100, "mka") {
        let tracks = scan(std::slice::from_ref(&music), None);
        let t = tracks.iter().find(|t| t.path.file_name() == mka.file_name()).expect("mka listed");
        assert_eq!((t.title.as_str(), t.format.as_str()), ("11 matroska", "MKA"));
        std::fs::remove_file(&mka).unwrap();
    }

    // noise the scanner must ignore
    std::fs::write(music.join("cover.jpg"), b"jpeg").unwrap();
    std::fs::write(music.join(".homefi-123.part"), b"partial").unwrap();

    let cache = dir.path().join("library_cache.json");
    let tracks = scan(&[dir.path().join("Music")], Some(&cache));
    assert_eq!(tracks.len(), expected.len(), "{tracks:#?}");

    for (path, rate, format, bits) in &expected {
        let name = path.file_name().unwrap().to_string_lossy();
        let t = tracks
            .iter()
            .find(|t| t.path.file_name() == path.file_name())
            .unwrap_or_else(|| panic!("{name} not scanned"));
        assert_eq!(t.format, *format, "{name}");
        assert_eq!(t.sample_rate, Some(*rate), "{name}");
        assert_eq!(t.bit_depth, *bits, "{name}");
        assert_eq!(t.channels, Some(2), "{name}");
        assert!((900..=1200).contains(&t.duration_ms), "{name}: {} ms", t.duration_ms);
        assert_eq!(t.title, name, "{name}");
        assert_eq!(t.artist, "HomeFi Test", "{name}");
        assert_eq!(t.album, "Formats", "{name}");
        assert_eq!(t.year, Some(2024), "{name}");
        assert_eq!(t.track_number, 3, "{name}");
    }

    // folder image is used as cover for files without embedded art
    assert_eq!(read_cover(&expected[0].0).as_deref(), Some(&b"jpeg"[..]));

    // second scan is served from the cache and gives the same result
    assert!(cache.exists());
    assert_eq!(scan(&[dir.path().join("Music")], Some(&cache)), tracks);

    // removed files disappear
    std::fs::remove_file(&expected[0].0).unwrap();
    assert_eq!(scan(&[dir.path().join("Music")], Some(&cache)).len(), expected.len() - 1);
}
