//! `config.toml` (same layout as tiddl's, so existing files keep working).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::paths;
use crate::quality::{TrackQuality, VideoQuality};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SinglesFilter {
    /// Only full albums.
    #[default]
    None,
    /// Only EPs and singles.
    Only,
    /// Albums, EPs and singles.
    Include,
}

/// Used for both the videos and the Dolby Atmos filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MediaFilter {
    #[default]
    None,
    Only,
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// Keep whatever Tidal serves (FLAC / M4A).
    #[default]
    Original,
    Flac,
    Mp3,
}

impl AudioFormat {
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::Original => None,
            Self::Flac => Some("flac"),
            Self::Mp3 => Some("mp3"),
        }
    }
}

macro_rules! impl_from_str {
    ($ty:ty { $($name:literal => $variant:expr),* $(,)? }) => {
        impl std::str::FromStr for $ty {
            type Err = String;
            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                match s.to_ascii_lowercase().as_str() {
                    $($name => Ok($variant),)*
                    _ => Err(format!("invalid value `{s}`, expected one of: {}", [$($name),*].join(", "))),
                }
            }
        }
    };
}

impl_from_str!(SinglesFilter { "none" => SinglesFilter::None, "only" => SinglesFilter::Only, "include" => SinglesFilter::Include });
impl_from_str!(MediaFilter { "none" => MediaFilter::None, "only" => MediaFilter::Only, "allow" => MediaFilter::Allow });
impl_from_str!(AudioFormat { "original" => AudioFormat::Original, "flac" => AudioFormat::Flac, "mp3" => AudioFormat::Mp3 });

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enable_cache: bool,
    pub debug: bool,
    /// The CLI says (at most once a day, on stderr) when a newer HomeFi is out.
    /// It never installs anything. `HOMEFI_NO_UPDATE_CHECK=1` turns it off too.
    pub update_check: bool,
    pub metadata: MetadataConfig,
    pub cover: CoverConfig,
    pub download: DownloadConfig,
    pub m3u: M3uConfig,
    pub templates: TemplatesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_cache: true,
            debug: false,
            update_check: true,
            metadata: MetadataConfig::default(),
            cover: CoverConfig::default(),
            download: DownloadConfig::default(),
            m3u: M3uConfig::default(),
            templates: TemplatesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetadataConfig {
    pub enable: bool,
    pub lyrics: bool,
    pub cover: bool,
    pub album_review: bool,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self { enable: true, lyrics: false, cover: true, album_review: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverResource {
    Track,
    Album,
    Playlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverConfig {
    pub save: bool,
    pub size: u32,
    pub allowed: Vec<CoverResource>,
    pub templates: CoverTemplates,
}

impl Default for CoverConfig {
    fn default() -> Self {
        Self { save: false, size: 1280, allowed: Vec::new(), templates: CoverTemplates::default() }
    }
}

impl CoverConfig {
    pub fn saves(&self, resource: CoverResource) -> bool {
        self.save && self.allowed.contains(&resource)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverTemplates {
    pub track: String,
    pub album: String,
    pub playlist: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DownloadConfig {
    pub track_quality: TrackQuality,
    pub video_quality: VideoQuality,
    pub skip_existing: bool,
    pub threads_count: usize,
    pub download_path: Option<PathBuf>,
    /// Where existing downloads are looked up; defaults to `download_path`.
    pub scan_path: Option<PathBuf>,
    pub singles_filter: SinglesFilter,
    pub videos_filter: MediaFilter,
    pub update_mtime: bool,
    pub rewrite_metadata: bool,
    pub write_lrc_file: bool,
    pub match_existing_path_case: bool,
    pub atmos_filter: MediaFilter,
    pub audio_format: AudioFormat,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            track_quality: TrackQuality::High,
            video_quality: VideoQuality::Fhd,
            skip_existing: true,
            threads_count: 4,
            download_path: None,
            scan_path: None,
            singles_filter: SinglesFilter::None,
            videos_filter: MediaFilter::None,
            update_mtime: false,
            rewrite_metadata: false,
            write_lrc_file: false,
            match_existing_path_case: false,
            atmos_filter: MediaFilter::None,
            audio_format: AudioFormat::Original,
        }
    }
}

impl DownloadConfig {
    pub fn download_dir(&self) -> PathBuf {
        self.download_path.as_deref().map(paths::expand_tilde).unwrap_or_else(paths::default_download_dir)
    }

    pub fn scan_dir(&self) -> PathBuf {
        self.scan_path.as_deref().map(paths::expand_tilde).unwrap_or_else(|| self.download_dir())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum M3uResource {
    Album,
    Playlist,
    Mix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct M3uConfig {
    pub save: bool,
    pub allowed: Vec<M3uResource>,
    pub templates: M3uTemplates,
}

impl Default for M3uConfig {
    fn default() -> Self {
        Self {
            save: false,
            allowed: vec![M3uResource::Album, M3uResource::Playlist, M3uResource::Mix],
            templates: M3uTemplates::default(),
        }
    }
}

impl M3uConfig {
    pub fn saves(&self, resource: M3uResource) -> bool {
        self.save && self.allowed.contains(&resource)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct M3uTemplates {
    pub album: String,
    pub playlist: String,
    pub mix: String,
}

impl Default for M3uTemplates {
    fn default() -> Self {
        Self {
            album: "m3u/{type}/{album.artist} - {album.title}".into(),
            playlist: "m3u/{type}/{playlist.title}".into(),
            mix: "m3u/{type}/{mix_id}".into(),
        }
    }
}

pub const DEFAULT_TEMPLATE: &str = "{album.artist}/{album.title}/{item.title}";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TemplatesConfig {
    pub default: String,
    pub track: String,
    pub video: String,
    pub album: String,
    pub playlist: String,
    pub mix: String,
}

impl Default for TemplatesConfig {
    fn default() -> Self {
        Self {
            default: DEFAULT_TEMPLATE.into(),
            track: String::new(),
            video: String::new(),
            album: String::new(),
            playlist: String::new(),
            mix: String::new(),
        }
    }
}

impl TemplatesConfig {
    fn or_default<'a>(&'a self, t: &'a str) -> &'a str {
        if t.is_empty() {
            &self.default
        } else {
            t
        }
    }
    pub fn track(&self) -> &str {
        self.or_default(&self.track)
    }
    pub fn video(&self) -> &str {
        self.or_default(&self.video)
    }
    pub fn album(&self) -> &str {
        self.or_default(&self.album)
    }
    pub fn playlist(&self) -> &str {
        self.or_default(&self.playlist)
    }
    pub fn mix(&self) -> &str {
        self.or_default(&self.mix)
    }
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self> {
        let config: Config = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        if config.templates.default.trim().is_empty() {
            return Err(Error::Config("templates.default cannot be empty".into()));
        }
        Ok(config)
    }

    /// Loads `path`, falling back to defaults when it does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Commented example written by `homefi config init`.
pub const EXAMPLE_CONFIG: &str = include_str!("../../../docs/config.example.toml");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_is_valid() {
        let config = Config::from_toml(EXAMPLE_CONFIG).unwrap();
        assert_eq!(config.download.track_quality, TrackQuality::High);
        assert_eq!(config.templates.track(), DEFAULT_TEMPLATE);
        assert!(config.update_check);
        assert!(!Config::from_toml("update_check = false").unwrap().update_check);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let config = Config::from_toml("[download]\ntrack_quality = \"max\"\ndownload_path = \"/tmp/x\"").unwrap();
        assert_eq!(config.download.track_quality, TrackQuality::Max);
        assert_eq!(config.download.scan_dir(), PathBuf::from("/tmp/x"));
        assert!(config.metadata.enable);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::from_toml("[download]\ntrack_qualty = \"max\"").is_err());
        assert!(Config::from_toml("[templates]\ndefault = \"\"").is_err());
    }
}
