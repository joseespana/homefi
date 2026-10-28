//! Decodes real files of every format (generated with ffmpeg) and checks
//! length, sample rate and seeking. Skipped when ffmpeg is missing.

use std::path::Path;
use std::process::Command;

use homefi_player::Source;

/// (file, ffmpeg codec args, sample rate, expected codec)
const CASES: &[(&str, &[&str], u32, &str)] = &[
    ("flac16.flac", &["-c:a", "flac", "-sample_fmt", "s16"], 44_100, "flac"),
    ("flac24.flac", &["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"], 96_000, "flac"),
    ("mp3.mp3", &["-c:a", "libmp3lame", "-b:a", "320k"], 44_100, "mp3"),
    ("wav24.wav", &["-c:a", "pcm_s24le"], 48_000, "pcm_s24le"),
    ("aac.m4a", &["-c:a", "aac", "-b:a", "256k"], 44_100, "aac"),
    ("alac.m4a", &["-c:a", "alac"], 44_100, "alac"),
    ("vorbis.ogg", &["-c:a", "vorbis", "-strict", "-2"], 44_100, "vorbis"),
    ("aiff.aiff", &["-c:a", "pcm_s16be"], 44_100, "pcm_s16be"),
    (
        "hires.m4a",
        &["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24", "-strict", "-2", "-f", "mp4"],
        192_000,
        "flac",
    ),
];

fn ffmpeg(out: &Path, codec: &[&str], rate: u32) -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
        .arg("-i")
        .arg(format!("sine=frequency=440:sample_rate={rate}:duration=2"))
        .args(["-ac", "2"])
        .args(codec)
        .arg(out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn decodes_every_format() {
    let dir = tempfile::tempdir().unwrap();
    let mut tested = 0;
    for (name, codec, rate, expected) in CASES {
        let path = dir.path().join(name);
        if !ffmpeg(&path, codec, *rate) {
            eprintln!("skipping {name}: ffmpeg encoder missing");
            continue;
        }
        let mut src = Source::open(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(src.codec, *expected, "{name}");

        let mut samples = 0usize;
        let mut peak = 0f32;
        while let Some(chunk) = src.next_chunk().unwrap_or_else(|e| panic!("{name}: {e}")) {
            samples += chunk.len();
            peak = chunk.iter().fold(peak, |p, s| p.max(s.abs()));
        }
        assert_eq!(src.sample_rate, *rate, "{name}");
        assert_eq!(src.channels, 2, "{name}");
        let seconds = samples as f64 / 2.0 / *rate as f64;
        assert!((1.9..=2.2).contains(&seconds), "{name}: {seconds}s");
        assert!(peak > 0.05, "{name}: silent ({peak})");

        // seeking lands near the requested time
        let mut src = Source::open(&path).unwrap();
        let reached = src.seek(1000).unwrap_or_else(|e| panic!("{name} seek: {e}"));
        assert!((900..=1100).contains(&reached), "{name}: seek reached {reached} ms");
        assert!(src.next_chunk().unwrap().is_some(), "{name}: nothing after seek");
        tested += 1;
    }
    eprintln!("decoded {tested}/{} formats", CASES.len());
}
