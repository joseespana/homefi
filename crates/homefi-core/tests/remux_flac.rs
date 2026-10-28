//! FLAC-in-MP4 (plain and fragmented, like Tidal's DASH) → native FLAC must
//! be bit-identical to the original FLAC. Needs ffmpeg (skipped otherwise).

use std::path::Path;
use std::process::Command;

use homefi_core::remux::flac_from_mp4;

fn ffmpeg(args: &[&str]) -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// MD5 of the decoded PCM, independent of the container.
fn audio_md5(path: &Path) -> String {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-map", "0:a:0", "-f", "md5", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "ffmpeg could not decode {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn remux_is_bit_identical() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.flac");
    let ok = ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:sample_rate=192000:duration=3",
        "-ac",
        "2",
        "-c:a",
        "flac",
        "-sample_fmt",
        "s32",
        "-bits_per_raw_sample",
        "24",
        original.to_str().unwrap(),
    ]);
    if !ok {
        eprintln!("ffmpeg not available, skipping");
        return;
    }
    let reference = audio_md5(&original);

    let variants: &[(&str, &[&str])] = &[
        ("plain.m4a", &[]),
        ("fragmented.m4a", &["-movflags", "frag_keyframe+empty_moov+default_base_moof", "-frag_duration", "500000"]),
    ];
    for (name, extra) in variants {
        let mp4 = dir.path().join(name);
        let mut args = vec!["-i", original.to_str().unwrap(), "-c:a", "copy", "-strict", "-2", "-f", "mp4"];
        args.extend_from_slice(extra);
        args.push(mp4.to_str().unwrap());
        assert!(ffmpeg(&args), "could not build {name}");

        let flac = flac_from_mp4(&mp4).unwrap_or_else(|e| panic!("{name}: {e}")).expect("FLAC track found");
        assert_eq!(flac.extension().unwrap(), "flac");
        assert!(!mp4.exists(), "{name}: source should be removed");
        assert_eq!(&std::fs::read(&flac).unwrap()[..4], b"fLaC");
        assert_eq!(audio_md5(&flac), reference, "{name}: audio differs");

        let props = lofty::read_from_path(&flac).unwrap();
        use lofty::file::AudioFile;
        assert_eq!(props.properties().sample_rate(), Some(192_000), "{name}");
        assert_eq!(props.properties().bit_depth(), Some(24), "{name}");
        std::fs::remove_file(&flac).unwrap();
    }

    // AAC in MP4 is not touched
    let aac = dir.path().join("aac.m4a");
    if ffmpeg(&["-f", "lavfi", "-i", "sine=duration=1", "-c:a", "aac", aac.to_str().unwrap()]) {
        assert!(flac_from_mp4(&aac).unwrap().is_none());
        assert!(aac.exists());
    }
}
