//! Single Tidal tracks fetched for playback ("streaming") into a cache folder.
//!
//! Each track is downloaded with the regular engine (same quality handling,
//! tags and cover as a download) to `<dir>/<id>.<ext>`, so players get a plain
//! local file and playback stays bit-perfect and gapless. Used by the desktop
//! app and the Android bindings; the front end decides how big the cache may
//! grow and calls [`trim`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use crate::api::TidalApi;
use crate::config::Config;
use crate::download::{DownloadOptions, Downloader, Event};
use crate::error::{Error, Result};
use crate::quality::TrackQuality;
use crate::resource::{Resource, ResourceKind};

type Locks = Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>;

/// One fetch per track and folder at a time: two plays of the same track
/// (double tap, a new play while the previous one is still fetching) would
/// write the same temporary file, and the second would lose it.
static FETCHING: LazyLock<Locks> = LazyLock::new(Default::default);

/// Download options used for cached tracks: `<dir>/<id>.<ext>` with the cover
/// embedded, no lyrics, no side files, existing files reused.
pub fn options(config: &Config, dir: &Path, quality: TrackQuality) -> DownloadOptions {
    let mut opts = DownloadOptions::from_config(config);
    opts.download_dir = dir.to_path_buf();
    opts.scan_dir = dir.to_path_buf();
    opts.template_override = Some("{item.id}".into());
    opts.track_quality = quality;
    opts.skip_existing = true;
    opts.write_lrc_file = false;
    opts.fail_fast = false;
    opts.metadata.cover = true;
    opts.metadata.lyrics = false;
    opts.cover.save = false;
    opts.m3u.save = false;
    opts
}

/// Fetches track `id` into `dir` (or finds it there) and returns its path.
/// Concurrent calls for the same track and folder wait for each other.
pub async fn fetch(api: TidalApi, config: &Config, dir: &Path, id: &str, quality: TrackQuality) -> Result<PathBuf> {
    let resource = Resource::parse(&format!("track/{id}"))?;
    if resource.kind != ResourceKind::Track {
        return Err(Error::Stream(format!("`{id}` is not a track id")));
    }

    let entry = FetchEntry::new(dir.join(&resource.id));
    let lock = entry.lock();
    let _guard = lock.lock().await;
    download(api, options(config, dir, quality), &resource).await
}

/// This call's share of a track's lock in [`FETCHING`]. Dropping it (the fetch
/// finished, failed or was cancelled mid-way) forgets locks nobody waits on.
struct FetchEntry(Option<Arc<tokio::sync::Mutex<()>>>);

impl FetchEntry {
    fn new(key: PathBuf) -> Self {
        Self(Some(FETCHING.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default().clone()))
    }

    fn lock(&self) -> Arc<tokio::sync::Mutex<()>> {
        self.0.clone().unwrap_or_default()
    }
}

impl Drop for FetchEntry {
    fn drop(&mut self) {
        drop(self.0.take());
        FETCHING.lock().unwrap_or_else(|e| e.into_inner()).retain(|_, l| Arc::strong_count(l) > 1);
    }
}

async fn download(api: TidalApi, opts: DownloadOptions, resource: &Resource) -> Result<PathBuf> {
    let found: Arc<Mutex<std::result::Result<PathBuf, String>>> =
        Arc::new(Mutex::new(Err("track not available".into())));
    let sink = found.clone();
    let observer = Arc::new(move |event: Event| {
        let result = match event {
            Event::Finished { path, .. } | Event::Exists { path, .. } => Ok(path),
            Event::Skipped { reason, .. } => Err(reason),
            Event::Failed { message, .. } => Err(message),
            _ => return,
        };
        *sink.lock().unwrap_or_else(|e| e.into_inner()) = result;
    });
    Downloader::new(api, opts, observer).run(std::slice::from_ref(resource)).await?;
    let result = found.lock().unwrap_or_else(|e| e.into_inner()).clone();
    result.map_err(Error::Stream)
}

/// Deletes the oldest files in `dir` until it fits in `limit` bytes, never
/// touching the files in `keep` (e.g. what the player has queued). Returns the
/// number of bytes freed.
pub fn trim(dir: &Path, limit: u64, keep: &[PathBuf]) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| (e.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    let mut freed = 0;
    files.sort_by_key(|f| f.2);
    for (path, size, _) in files {
        if total <= limit {
            break;
        }
        if keep.contains(&path) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= size;
            freed += size;
        }
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn trim_keeps_queued_and_newest() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..4).map(|i| dir.path().join(format!("{i}.flac"))).collect();
        for p in &paths {
            std::fs::write(p, vec![0u8; 100]).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(trim(dir.path(), 300, &paths[..1]), 100);
        assert!(paths[0].exists(), "queued file is kept");
        assert!(!paths[1].exists() && paths[2].exists() && paths[3].exists());
        assert_eq!(trim(&dir.path().join("missing"), 0, &[]), 0);
    }

    #[tokio::test]
    async fn cancelled_fetches_leave_no_lock_behind() {
        let key = PathBuf::from("/cache-test/42");
        let held = FetchEntry::new(key.clone());
        let lock = held.lock();
        let guard = lock.lock().await;
        // a second fetch waits on the lock, then is cancelled (dropped)
        let waiting = async {
            let entry = FetchEntry::new(key.clone());
            let lock = entry.lock();
            let _g = lock.lock().await;
        };
        assert!(tokio::time::timeout(std::time::Duration::from_millis(20), waiting).await.is_err());
        assert!(FETCHING.lock().unwrap().contains_key(&key));
        drop(guard);
        drop(lock);
        drop(held);
        assert!(!FETCHING.lock().unwrap().contains_key(&key));
    }

    #[test]
    fn options_write_one_file_per_id() {
        let opts = options(&Config::default(), Path::new("/cache"), TrackQuality::Max);
        assert_eq!(opts.template_override.as_deref(), Some("{item.id}"));
        assert_eq!(opts.track_quality, TrackQuality::Max);
        assert!(opts.skip_existing && !opts.m3u.save && !opts.cover.save && !opts.metadata.lyrics);
        assert_eq!(opts.download_dir, Path::new("/cache"));
    }
}
