//! Optional ffmpeg post-processing.
//!
//! ffmpeg is used to unwrap FLAC from MP4 containers, remux videos and
//! transcode to FLAC / MP3. When it is missing (e.g. on Android) downloads
//! still work and files keep the container Tidal served.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tokio::process::Command;

use crate::config::AudioFormat;
use crate::error::{Error, Result};
use crate::paths::with_suffix;

/// Overrides the ffmpeg executable.
pub const FFMPEG_ENV: &str = "HOMEFI_FFMPEG";

fn ffmpeg_bin() -> String {
    std::env::var(FFMPEG_ENV).unwrap_or_else(|_| "ffmpeg".into())
}

fn ffprobe_bin() -> String {
    let ffmpeg = ffmpeg_bin();
    match ffmpeg.rfind("ffmpeg") {
        Some(i) => format!("{}ffprobe{}", &ffmpeg[..i], &ffmpeg[i + 6..]),
        None => "ffprobe".into(),
    }
}

/// `true` when ffmpeg can be executed (checked once).
pub fn is_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new(ffmpeg_bin()).arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    })
}

async fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| Error::Ffmpeg(format!("could not run {program}: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.trim().lines().rev().take(3).collect();
        return Err(Error::Ffmpeg(format!(
            "{program} failed ({}): {}",
            out.status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

async fn ffmpeg(args: &[&std::ffi::OsStr]) -> Result<()> {
    let mut full: Vec<&std::ffi::OsStr> =
        vec!["-hide_banner".as_ref(), "-loglevel".as_ref(), "error".as_ref(), "-y".as_ref()];
    full.extend_from_slice(args);
    run(&ffmpeg_bin(), &full).await.map(drop)
}

/// Codec name of the first audio stream, or `None` if ffprobe is unavailable.
pub async fn probe_audio_codec(source: &Path) -> Option<String> {
    let args: [&std::ffi::OsStr; 8] = [
        "-v".as_ref(),
        "error".as_ref(),
        "-select_streams".as_ref(),
        "a:0".as_ref(),
        "-show_entries".as_ref(),
        "stream=codec_name".as_ref(),
        "-of".as_ref(),
        "default=noprint_wrappers=1:nokey=1".as_ref(),
    ];
    let mut full = args.to_vec();
    full.push(source.as_os_str());
    run(&ffprobe_bin(), &full).await.ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// Container detected from the first bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Flac,
    Mp4,
    Unknown,
}

pub fn sniff_container(path: &Path) -> Container {
    use std::io::Read;
    let mut head = [0u8; 12];
    let Ok(mut f) = std::fs::File::open(path) else { return Container::Unknown };
    if f.read_exact(&mut head).is_err() {
        return Container::Unknown;
    }
    if &head[..4] == b"fLaC" {
        Container::Flac
    } else if &head[4..8] == b"ftyp" || &head[4..8] == b"styp" || &head[4..8] == b"moov" {
        Container::Mp4
    } else {
        Container::Unknown
    }
}

/// Remuxes a `.ts` video into `.mp4` and removes the source.
pub async fn convert_to_mp4(source: &Path) -> Result<PathBuf> {
    let output = with_suffix(source, "mp4");
    ffmpeg(&["-i".as_ref(), source.as_os_str(), "-c".as_ref(), "copy".as_ref(), output.as_os_str()]).await?;
    tokio::fs::remove_file(source).await?;
    Ok(output)
}

/// Unwraps FLAC audio from an MP4 container. If the stream is not FLAC
/// (Tidal serves AAC for some "lossless" tracks) the file becomes `.m4a`.
pub async fn extract_flac(source: &Path) -> Result<PathBuf> {
    if let Some(codec) = probe_audio_codec(source).await {
        if codec != "flac" {
            let target = with_suffix(source, "m4a");
            if target != source {
                tokio::fs::rename(source, &target).await?;
            }
            return Ok(target);
        }
    }

    let target = with_suffix(source, "flac");
    let tmp = with_suffix(source, "tmp.flac");
    ffmpeg(&["-i".as_ref(), source.as_os_str(), "-c".as_ref(), "copy".as_ref(), tmp.as_os_str()]).await?;
    tokio::fs::rename(&tmp, &target).await?;
    if source != target && source.exists() {
        tokio::fs::remove_file(source).await?;
    }
    Ok(target)
}

/// Transcodes `source` into `format` and removes the source.
pub async fn convert_audio(source: &Path, format: AudioFormat) -> Result<PathBuf> {
    let Some(ext) = format.extension() else {
        return Ok(source.to_path_buf());
    };
    let target = with_suffix(source, ext);
    if source.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        return Ok(source.to_path_buf());
    }

    let tmp = with_suffix(source, &format!("tmp.{ext}"));
    let codec: &[&str] = match format {
        AudioFormat::Flac => &["-c:a", "flac"],
        // -ac 2 downmixes multichannel (Dolby Atmos) sources
        AudioFormat::Mp3 => &["-c:a", "libmp3lame", "-b:a", "320k", "-ac", "2"],
        AudioFormat::Original => unreachable!(),
    };

    let mut args: Vec<&std::ffi::OsStr> =
        vec!["-i".as_ref(), source.as_os_str(), "-map".as_ref(), "0:a:0".as_ref(), "-vn".as_ref()];
    args.extend(codec.iter().map(|s| std::ffi::OsStr::new(*s)));
    args.push(tmp.as_os_str());

    ffmpeg(&args).await?;
    tokio::fs::rename(&tmp, &target).await?;
    tokio::fs::remove_file(source).await?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_containers() {
        let dir = tempfile::tempdir().unwrap();
        let flac = dir.path().join("a");
        std::fs::write(&flac, b"fLaC\0\0\0\"0000000").unwrap();
        assert_eq!(sniff_container(&flac), Container::Flac);

        let mp4 = dir.path().join("b");
        std::fs::write(&mp4, b"\0\0\0\x18ftypiso6").unwrap();
        assert_eq!(sniff_container(&mp4), Container::Mp4);

        assert_eq!(sniff_container(&dir.path().join("missing")), Container::Unknown);
    }
}
