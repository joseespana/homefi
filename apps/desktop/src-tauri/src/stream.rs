//! Playing from Tidal without adding tracks to the library.
//!
//! Each track is fetched just ahead of the player (same quality, tags and cover
//! as a download) into `<app dir>/stream-cache`, then appended to the player's
//! queue, so playback stays bit-perfect and gapless. Tracks that are already in
//! the library play from there. The cache is trimmed to [`CACHE_LIMIT`].

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use homefi_core::stream_cache;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::AppState;

/// Tracks fetched ahead of the one playing.
const AHEAD: usize = 2;
pub(crate) const CACHE_LIMIT: u64 = 3 * 1024 * 1024 * 1024;

/// A track to play: its Tidal id, or a local copy when the library has it.
/// The rest is what guests see of it while sharing (optional).
#[derive(Debug, Clone, Deserialize)]
pub struct StreamItem {
    pub id: String,
    pub title: String,
    pub local: Option<String>,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub explicit: bool,
}

/// A Tidal track of the stream that is not in the player's queue yet.
#[derive(Debug, Clone, Serialize)]
pub struct Upcoming {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<u64>,
}

/// Nothing left to fetch (a local play replaced the stream).
pub fn clear_upcoming(app: &AppHandle) {
    set_upcoming(app, &[]);
}

/// Publishes what is left to fetch (`stream-upcoming` event).
fn set_upcoming(app: &AppHandle, items: &[StreamItem]) {
    let list: Vec<Upcoming> = items
        .iter()
        .map(|i| Upcoming {
            id: i.id.clone(),
            title: i.title.clone(),
            artist: i.artist.clone(),
            duration_ms: i.duration_ms,
        })
        .collect();
    let state = app.state::<AppState>();
    *state.upcoming.lock().unwrap() = list.clone();
    let _ = app.emit("stream-upcoming", list);
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum StreamEvent {
    Loading { title: String },
    Ready,
    Failed { title: String, message: String },
}

pub fn cache_dir(state: &AppState) -> PathBuf {
    state.settings_path.with_file_name("stream-cache")
}

/// Plays `items` in order, fetching from Tidal as the queue advances. Any
/// other `play` (local or Tidal) stops this one. Once this computer shares its
/// music, what is left goes to the shared queue instead.
pub fn play(app: AppHandle, items: Vec<StreamItem>) {
    let state = app.state::<AppState>();
    let generation = state.stream_generation.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let alive = || state.stream_generation.load(Ordering::SeqCst) == generation;
        // everything after the first is still to come
        set_upcoming(&app, items.get(1..).unwrap_or_default());
        let mut next = 0;
        while next < items.len() && alive() {
            if next > 0 && crate::share::host_takes_stream(&app, &items[next..]) {
                set_upcoming(&app, &[]);
                return;
            }
            // the player's queue holds exactly the tracks fetched so far
            let queued = state.player.snapshot().queue.len();
            let current = state.player.snapshot().current.unwrap_or(0);
            if next > 0 && (queued < next || next > current + AHEAD) {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
            let item = &items[next];
            if next == 0 {
                let _ = app.emit("stream", StreamEvent::Loading { title: item.title.clone() });
            }
            match resolve(&app, item).await {
                Ok(path) if alive() => {
                    if next == 0 {
                        state.player.load(vec![path], 0, 0, true);
                        let _ = app.emit("stream", StreamEvent::Ready);
                    } else {
                        state.player.append(vec![path]);
                    }
                }
                Ok(_) => return,
                // replaced by another play: its errors are no longer interesting
                Err(_) if !alive() => return,
                Err(message) => {
                    tracing::warn!("stream {}: {message}", item.id);
                    let _ = app.emit("stream", StreamEvent::Failed { title: item.title.clone(), message });
                    if next == 0 && items.len() == 1 {
                        return;
                    }
                }
            }
            next += 1;
            if alive() {
                set_upcoming(&app, &items[next.min(items.len())..]);
            }
            // let the engine take the new track before reading the queue again
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}

/// Adds tracks to the queue without interrupting what plays: at the end, or
/// right after the current track (`next`). Tidal tracks are fetched first, one
/// by one in order; an empty queue starts playing.
pub async fn enqueue<R: Runtime>(app: &AppHandle<R>, items: Vec<StreamItem>, next: bool) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let mut added = 0;
    let mut last_error = None;
    for item in &items {
        match resolve(app, item).await {
            Ok(path) => {
                add_to_queue(&state.player, path, next, added);
                added += 1;
            }
            Err(message) => {
                tracing::warn!("queue {}: {message}", item.id);
                last_error = Some(message);
            }
        }
    }
    match (added, last_error) {
        (0, Some(message)) => Err(message),
        _ => Ok(added),
    }
}

/// `nth` is how many tracks this request added before, so "play next" keeps their order.
pub fn add_to_queue(player: &homefi_player::Player, path: PathBuf, next: bool, nth: usize) {
    let snapshot = player.snapshot();
    if snapshot.queue.is_empty() {
        player.load(vec![path], 0, 0, true);
        return;
    }
    let len = snapshot.queue.len();
    player.append(vec![path]);
    if next {
        let to = (snapshot.current.unwrap_or(0) + 1 + nth).min(len);
        if to < len {
            player.move_track(len, to);
        }
    }
}

async fn resolve<R: Runtime>(app: &AppHandle<R>, item: &StreamItem) -> Result<PathBuf, String> {
    if let Some(local) = item.local.as_ref().map(PathBuf::from).filter(|p| p.exists()) {
        return Ok(local);
    }
    let state = app.state::<AppState>();
    let dir = cache_dir(&state);
    let path = fetch(&state, &dir, &item.id).await.map_err(crate::commands::err)?;
    let keep: Vec<PathBuf> = state.player.snapshot().queue;
    tokio::task::spawn_blocking(move || stream_cache::trim(&dir, CACHE_LIMIT, &keep));
    Ok(path)
}

/// Same quality, tags and cover as a download, one track per id at a time
/// (see [`stream_cache::fetch`]).
async fn fetch(state: &AppState, dir: &Path, id: &str) -> homefi_core::Result<PathBuf> {
    state.session.refresh(600, false).await?;
    let api = state.session.api()?;
    stream_cache::fetch(api, state.session.config(), dir, id, state.settings().quality).await
}
