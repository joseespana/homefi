//! ALAC, MP3, AAC and Opus through the user's ffmpeg, the way downloads convert: the audio is
//! written as WAV first, then `homefi_core::ffmpeg::transcode` makes the final file.

use std::path::{Path, PathBuf};

use tokio::runtime::Handle;

use super::{Mp3Kind, RipFormat};
use crate::error::{DiscError, Result};

const MP3_CBR_KBPS: u16 = 320;
/// LAME's highest VBR quality (V0).
const MP3_V0_QUALITY: &str = "0";

/// ffmpeg codec arguments for a format that needs ffmpeg; empty for FLAC and WAV.
pub fn codec_args(format: &RipFormat) -> Vec<String> {
    let owned = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
    match format {
        RipFormat::Flac { .. } | RipFormat::Wav => Vec::new(),
        RipFormat::Alac => owned(&["-c:a", "alac"]),
        RipFormat::Mp3 { kind: Mp3Kind::Cbr320 } => owned(&["-c:a", "libmp3lame", "-b:a", &format!("{MP3_CBR_KBPS}k")]),
        RipFormat::Mp3 { kind: Mp3Kind::V0 } => owned(&["-c:a", "libmp3lame", "-q:a", MP3_V0_QUALITY]),
        RipFormat::Aac { kbps } => owned(&["-c:a", "aac", "-b:a", &format!("{kbps}k")]),
        RipFormat::Opus { kbps } => owned(&["-c:a", "libopus", "-b:a", &format!("{kbps}k")]),
    }
}

/// Turns the WAV at `wav` into `format`; the WAV is removed. Returns the new file.
pub fn convert(runtime: &Handle, wav: &Path, format: &RipFormat) -> Result<PathBuf> {
    let args = codec_args(format);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    runtime
        .block_on(homefi_core::ffmpeg::transcode(wav, &args, format.extension()))
        .map_err(|error| DiscError::Encode(format!("ffmpeg: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_lossy_format_has_its_codec() {
        assert!(codec_args(&RipFormat::Flac { level: 5 }).is_empty());
        assert_eq!(codec_args(&RipFormat::Alac), ["-c:a", "alac"]);
        assert_eq!(codec_args(&RipFormat::Mp3 { kind: Mp3Kind::Cbr320 }), ["-c:a", "libmp3lame", "-b:a", "320k"]);
        assert_eq!(codec_args(&RipFormat::Mp3 { kind: Mp3Kind::V0 }), ["-c:a", "libmp3lame", "-q:a", "0"]);
        assert_eq!(codec_args(&RipFormat::Aac { kbps: 256 }), ["-c:a", "aac", "-b:a", "256k"]);
        assert_eq!(codec_args(&RipFormat::Opus { kbps: 160 }), ["-c:a", "libopus", "-b:a", "160k"]);
    }
}
