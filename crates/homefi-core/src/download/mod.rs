//! Concurrent download engine shared by the CLI and the Android app.
//!
//! The engine reports everything through [`Observer`] events so each front
//! end can render progress its own way.

mod engine;

pub use engine::Downloader;

use std::path::PathBuf;

use crate::config::{
    AudioFormat, Config, CoverConfig, M3uConfig, MediaFilter, MetadataConfig, SinglesFilter, TemplatesConfig,
};
use crate::quality::{TrackQuality, VideoQuality};

/// Everything that controls a download run.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub track_quality: TrackQuality,
    pub video_quality: VideoQuality,
    pub skip_existing: bool,
    pub threads: usize,
    pub download_dir: PathBuf,
    pub scan_dir: PathBuf,
    pub singles_filter: SinglesFilter,
    pub videos_filter: MediaFilter,
    pub atmos_filter: MediaFilter,
    pub audio_format: AudioFormat,
    pub match_existing_path_case: bool,
    pub rewrite_metadata: bool,
    pub update_mtime: bool,
    pub write_lrc_file: bool,
    /// Stop at the first failure instead of reporting it and continuing.
    pub fail_fast: bool,
    /// Template used for every resource type instead of the configured ones.
    pub template_override: Option<String>,
    pub templates: TemplatesConfig,
    pub metadata: MetadataConfig,
    pub cover: CoverConfig,
    pub m3u: M3uConfig,
}

impl DownloadOptions {
    pub fn from_config(config: &Config) -> Self {
        let d = &config.download;
        Self {
            track_quality: d.track_quality,
            video_quality: d.video_quality,
            skip_existing: d.skip_existing,
            threads: d.threads_count.max(1),
            download_dir: d.download_dir(),
            scan_dir: d.scan_dir(),
            singles_filter: d.singles_filter,
            videos_filter: d.videos_filter,
            atmos_filter: d.atmos_filter,
            audio_format: d.audio_format,
            match_existing_path_case: d.match_existing_path_case,
            rewrite_metadata: d.rewrite_metadata,
            update_mtime: d.update_mtime,
            write_lrc_file: d.write_lrc_file,
            fail_fast: false,
            template_override: None,
            templates: config.templates.clone(),
            metadata: config.metadata.clone(),
            cover: config.cover.clone(),
            m3u: config.m3u.clone(),
        }
    }
}

/// How an item download ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Downloaded,
    /// The file existed and was downloaded again (`skip_existing = false`).
    Overwritten,
}

/// Files written next to the media files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraFile {
    Cover,
    Lyrics,
    Playlist,
}

/// Progress reported by the [`Downloader`].
#[derive(Debug, Clone)]
pub enum Event {
    // `id` is the Tidal id of the track / video, so UIs can show the state of each row.
    /// A track or video was added to the queue.
    Queued { id: String, title: String },
    /// The transfer of an item started.
    Started { task: u64, id: String, title: String, quality: String },
    /// Bytes received so far for `task`.
    Progress { task: u64, downloaded: u64, total: Option<u64> },
    /// The item was downloaded and post-processed.
    Finished { task: u64, id: String, title: String, quality: String, path: PathBuf, outcome: Outcome },
    /// The item already exists on disk and was skipped.
    Exists { id: String, title: String, path: PathBuf },
    /// The item was skipped (filters, not streamable, ...).
    Skipped { id: String, title: String, reason: String },
    /// Something went wrong; the run continues unless `fail_fast` is set.
    /// `id` is set when the failure belongs to one track / video.
    Failed { id: Option<String>, context: String, message: String },
    /// A cover, `.lrc` or `.m3u` file was written.
    ExtraWritten { kind: ExtraFile, path: PathBuf },
}

/// Receives [`Event`]s. Called from download tasks, so it must be cheap.
pub trait Observer: Send + Sync {
    fn on_event(&self, event: Event);
}

impl<F: Fn(Event) + Send + Sync> Observer for F {
    fn on_event(&self, event: Event) {
        self(event)
    }
}

/// Counts of a finished run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub queued: u64,
    pub downloaded: u64,
    pub existing: u64,
    pub skipped: u64,
    pub failed: u64,
}
