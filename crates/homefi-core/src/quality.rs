//! Quality presets and their Tidal API names.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Audio quality preset, named as in the config file / CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrackQuality {
    /// 96 kbps AAC.
    Low,
    /// 320 kbps AAC.
    Normal,
    /// 16-bit / 44.1 kHz FLAC.
    #[default]
    High,
    /// Up to 24-bit / 192 kHz FLAC.
    Max,
}

impl TrackQuality {
    pub const ALL: [TrackQuality; 4] = [Self::Low, Self::Normal, Self::High, Self::Max];

    /// Value of the `audioquality` API parameter.
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Normal => "HIGH",
            Self::High => "LOSSLESS",
            Self::Max => "HI_RES_LOSSLESS",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "96 kbps AAC",
            Self::Normal => "320 kbps AAC",
            Self::High => "16-bit 44.1 kHz FLAC",
            Self::Max => "up to 24-bit 192 kHz FLAC",
        }
    }

    pub fn is_lossless(self) -> bool {
        matches!(self, Self::High | Self::Max)
    }
}

/// `true` for the API names of lossless qualities.
pub fn is_lossless_api_name(name: &str) -> bool {
    matches!(name, "LOSSLESS" | "HI_RES_LOSSLESS")
}

impl fmt::Display for TrackQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TrackQuality {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|q| q.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown track quality `{s}` (low, normal, high, max)"))
    }
}

/// Video quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VideoQuality {
    /// 360p.
    Sd,
    /// 720p.
    Hd,
    /// 1080p.
    #[default]
    Fhd,
}

impl VideoQuality {
    pub const ALL: [VideoQuality; 3] = [Self::Sd, Self::Hd, Self::Fhd];

    pub fn api_name(self) -> &'static str {
        match self {
            Self::Sd => "LOW",
            Self::Hd => "MEDIUM",
            Self::Fhd => "HIGH",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sd => "sd",
            Self::Hd => "hd",
            Self::Fhd => "fhd",
        }
    }
}

impl fmt::Display for VideoQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for VideoQuality {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|q| q.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown video quality `{s}` (sd, hd, fhd)"))
    }
}

/// Human readable label for a track stream quality (API name).
pub fn track_quality_label(api_name: &str) -> &'static str {
    match api_name {
        "LOW" => "96 kbps",
        "HIGH" => "320 kbps",
        "LOSSLESS" => "Lossless",
        "HI_RES_LOSSLESS" => "Hi-Res",
        _ => "",
    }
}

/// Human readable label for a video stream quality (API name).
pub fn video_quality_label(api_name: &str) -> &'static str {
    match api_name {
        "LOW" => "360p",
        "MEDIUM" => "720p",
        "HIGH" => "1080p",
        _ => "",
    }
}
