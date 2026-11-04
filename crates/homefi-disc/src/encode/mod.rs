//! Rip formats and the encoders behind them. [`encode_track`] takes the audio the reader
//! delivers (a [`ChunkConsumer`]) and makes one file: FLAC and WAV are written by us, the rest
//! goes through the user's ffmpeg.

mod ffmpeg;
mod flac;
mod wav;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::checksum::PcmDigest;
use crate::chunks::ChunkConsumer;
use crate::error::{DiscError, Result};

pub const FLAC_MAX_LEVEL: u8 = 8;
pub const DEFAULT_FLAC_LEVEL: u8 = 5;
pub const DEFAULT_AAC_KBPS: u16 = 256;
pub const DEFAULT_OPUS_KBPS: u16 = 160;
const AAC_KBPS: std::ops::RangeInclusive<u16> = 64..=512;
const OPUS_KBPS: std::ops::RangeInclusive<u16> = 16..=512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mp3Kind {
    Cbr320,
    V0,
}

/// How the ripped audio is stored. Audio CDs are 16-bit / 44.1 kHz whatever the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RipFormat {
    Flac {
        level: u8,
    },
    Alac,
    Wav,
    Mp3 {
        #[serde(rename = "mp3")]
        kind: Mp3Kind,
    },
    Aac {
        kbps: u16,
    },
    Opus {
        kbps: u16,
    },
}

impl Default for RipFormat {
    fn default() -> Self {
        RipFormat::Flac { level: DEFAULT_FLAC_LEVEL }
    }
}

impl RipFormat {
    /// Everything except FLAC and WAV goes through the user's ffmpeg.
    pub fn needs_ffmpeg(&self) -> bool {
        !matches!(self, RipFormat::Flac { .. } | RipFormat::Wav)
    }

    pub fn extension(&self) -> &'static str {
        match self {
            RipFormat::Flac { .. } => "flac",
            RipFormat::Alac | RipFormat::Aac { .. } => "m4a",
            RipFormat::Wav => "wav",
            RipFormat::Mp3 { .. } => "mp3",
            RipFormat::Opus { .. } => "opus",
        }
    }

    pub fn label(&self) -> String {
        match self {
            RipFormat::Flac { level } => format!("FLAC level {level}"),
            RipFormat::Alac => "ALAC".into(),
            RipFormat::Wav => "WAV".into(),
            RipFormat::Mp3 { kind: Mp3Kind::Cbr320 } => "MP3 320 kbps".into(),
            RipFormat::Mp3 { kind: Mp3Kind::V0 } => "MP3 V0".into(),
            RipFormat::Aac { kbps } => format!("AAC {kbps} kbps"),
            RipFormat::Opus { kbps } => format!("Opus {kbps} kbps"),
        }
    }

    /// Puts levels and bit rates inside what the encoders accept.
    pub fn clamped(self) -> Self {
        match self {
            RipFormat::Flac { level } => RipFormat::Flac { level: level.min(FLAC_MAX_LEVEL) },
            RipFormat::Aac { kbps } => RipFormat::Aac { kbps: kbps.clamp(*AAC_KBPS.start(), *AAC_KBPS.end()) },
            RipFormat::Opus { kbps } => RipFormat::Opus { kbps: kbps.clamp(*OPUS_KBPS.start(), *OPUS_KBPS.end()) },
            other => other,
        }
    }
}

/// What the check of an encoded file proved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verified {
    /// The FLAC was decoded again and its audio equals what the drive delivered.
    FlacDecoded,
}

/// Where an encode is, after the audio has all arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Closing the file (or converting it with ffmpeg).
    Finishing,
    /// Decoding the FLAC to compare it.
    Verifying,
}

/// One track to encode.
pub struct EncodeJob<'a> {
    pub format: RipFormat,
    /// Decode-and-compare after writing a FLAC.
    pub verify_flac: bool,
    /// Where the file goes, without extension. Nothing else may use this name.
    pub stem: &'a Path,
    /// Stereo frames the track should have (0 if unknown); lets the FLAC encoder pick block sizes.
    pub frames: u64,
    pub runtime: &'a Handle,
    pub on_stage: &'a (dyn Fn(Stage) + Sync),
}

/// A finished file and what was written into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub path: PathBuf,
    pub digest: PcmDigest,
    pub verified: Option<Verified>,
}

fn with_extension(stem: &Path, ext: &str) -> PathBuf {
    let mut name = stem.as_os_str().to_owned();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}

/// Consumes `chunks` until the reader is done and writes the file. On any error nothing is
/// left behind under the job's name.
pub fn encode_track(job: &EncodeJob<'_>, chunks: &ChunkConsumer) -> Result<Encoded> {
    let ext = job.format.extension();
    let mut made: Vec<PathBuf> = Vec::new();
    let outcome = encode_into(job, chunks, ext, &mut made);
    if outcome.is_err() {
        for path in made {
            let _ = std::fs::remove_file(path);
        }
    }
    outcome
}

fn encode_into(job: &EncodeJob<'_>, chunks: &ChunkConsumer, ext: &str, made: &mut Vec<PathBuf>) -> Result<Encoded> {
    match job.format {
        RipFormat::Flac { level } => {
            let path = with_extension(job.stem, ext);
            made.push(path.clone());
            let digest = flac::encode_flac(chunks, &path, level, job.frames)?;
            (job.on_stage)(Stage::Finishing);
            let verified = if job.verify_flac {
                (job.on_stage)(Stage::Verifying);
                flac::verify_flac(&path, &digest)?;
                Some(Verified::FlacDecoded)
            } else {
                None
            };
            Ok(Encoded { path, digest, verified })
        }
        RipFormat::Wav => {
            let path = with_extension(job.stem, ext);
            made.push(path.clone());
            let digest = wav::write_wav(chunks, &path)?;
            (job.on_stage)(Stage::Finishing);
            Ok(Encoded { path, digest, verified: None })
        }
        _ => {
            let wav_path = with_extension(job.stem, "wav");
            made.push(wav_path.clone());
            made.push(with_extension(job.stem, ext));
            made.push(with_extension(job.stem, &format!("tmp.{ext}")));
            let digest = wav::write_wav(chunks, &wav_path)?;
            (job.on_stage)(Stage::Finishing);
            let path = ffmpeg::convert(job.runtime, &wav_path, &job.format)?;
            Ok(Encoded { path, digest, verified: None })
        }
    }
}

/// `true` when `format` can be made on this computer (ffmpeg present when it needs it).
pub fn available(format: &RipFormat) -> bool {
    !format.needs_ffmpeg() || homefi_core::ffmpeg::is_available()
}

/// Checks a format can be made before any audio is read.
pub fn ensure_available(format: &RipFormat) -> Result<()> {
    if available(format) {
        Ok(())
    } else {
        Err(DiscError::NeedsFfmpeg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_matches_the_ui_shapes() {
        let cases = [
            (RipFormat::Flac { level: 5 }, r#"{"kind":"flac","level":5}"#),
            (RipFormat::Alac, r#"{"kind":"alac"}"#),
            (RipFormat::Mp3 { kind: Mp3Kind::V0 }, r#"{"kind":"mp3","mp3":"v0"}"#),
            (RipFormat::Aac { kbps: 256 }, r#"{"kind":"aac","kbps":256}"#),
        ];
        for (format, json) in cases {
            assert_eq!(serde_json::to_string(&format).unwrap(), json);
            assert_eq!(serde_json::from_str::<RipFormat>(json).unwrap(), format);
        }
    }

    #[test]
    fn only_lossless_pcm_skips_ffmpeg() {
        assert!(!RipFormat::default().needs_ffmpeg());
        assert!(!RipFormat::Wav.needs_ffmpeg());
        assert!(RipFormat::Alac.needs_ffmpeg());
        assert_eq!(RipFormat::Alac.extension(), "m4a");
    }

    #[test]
    fn clamps_levels_and_rates() {
        assert_eq!(RipFormat::Flac { level: 12 }.clamped(), RipFormat::Flac { level: 8 });
        assert_eq!(RipFormat::Opus { kbps: 1 }.clamped(), RipFormat::Opus { kbps: 16 });
    }
}
