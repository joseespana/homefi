//! OS media integration: macOS Now Playing / Control Center + media keys,
//! Windows media overlay (SMTC), Linux MPRIS.

use std::cell::RefCell;
use std::time::Duration;

use homefi_player::{Player, Snapshot, Status};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
};
#[cfg(target_os = "windows")]
use tauri::Manager;
use tauri::{AppHandle, Runtime};

use crate::commands::TrackView;

thread_local! {
    // the OS APIs must be used from the main thread
    static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
}

/// Creates the controls on the main thread and routes their events to `player`.
pub fn init<R: Runtime>(app: &AppHandle<R>, player: Player) {
    #[cfg(target_os = "windows")]
    let hwnd = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0);
    #[cfg(not(target_os = "windows"))]
    let hwnd = None;
    let _ = app;

    let config = PlatformConfig { dbus_name: "homefi", display_name: "HomeFi", hwnd };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("media controls unavailable: {e:?}");
            return;
        }
    };

    let attached = controls.attach(move |event| match event {
        MediaControlEvent::Play => player.play(),
        MediaControlEvent::Pause => player.pause(),
        MediaControlEvent::Toggle => player.toggle(),
        MediaControlEvent::Next => player.next(),
        MediaControlEvent::Previous => player.previous(),
        MediaControlEvent::Stop => player.pause(),
        MediaControlEvent::SetPosition(MediaPosition(pos)) => player.seek(pos.as_millis() as u64),
        MediaControlEvent::Seek(dir) => {
            let now = player.snapshot().position_ms;
            player.seek(match dir {
                SeekDirection::Forward => now + 10_000,
                SeekDirection::Backward => now.saturating_sub(10_000),
            })
        }
        MediaControlEvent::SeekBy(dir, by) => {
            let now = player.snapshot().position_ms;
            let by = by.as_millis() as u64;
            player.seek(match dir {
                SeekDirection::Forward => now + by,
                SeekDirection::Backward => now.saturating_sub(by),
            })
        }
        _ => {}
    });
    if let Err(e) = attached {
        tracing::warn!("could not attach media controls: {e:?}");
        return;
    }
    CONTROLS.with(|c| *c.borrow_mut() = Some(controls));
}

/// Track whose metadata was last sent (metadata only changes with the track).
static SHOWN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Pushes the current track and state to the OS. Call it off the main thread:
/// a new track's cover is read (and written for the OS) here, and only the
/// ready data goes to the main thread.
pub fn update<R: Runtime>(app: &AppHandle<R>, snapshot: &Snapshot, track: Option<TrackView>) {
    let status = snapshot.status;
    let position = Duration::from_millis(snapshot.position_ms);
    let duration = snapshot.duration_ms.map(Duration::from_millis);
    let changed = {
        let path = track.as_ref().map(|t| t.path.clone());
        let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        let changed = *shown != path;
        *shown = path;
        changed
    };
    let metadata = match (changed, track) {
        (true, Some(t)) => {
            let cover = cover_file(&t.path);
            Some((t, cover))
        }
        _ => None,
    };
    let _ = app.run_on_main_thread(move || {
        CONTROLS.with(|c| {
            let mut c = c.borrow_mut();
            let Some(controls) = c.as_mut() else { return };
            if let Some((t, cover)) = &metadata {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&t.title),
                    artist: Some(&t.artist),
                    album: Some(&t.album),
                    cover_url: cover.as_deref(),
                    duration,
                });
            }
            let progress = Some(MediaPosition(position));
            let _ = controls.set_playback(match status {
                Status::Playing => MediaPlayback::Playing { progress },
                Status::Paused => MediaPlayback::Paused { progress },
                Status::Stopped => MediaPlayback::Stopped,
            });
        });
    });
}

/// Writes the embedded cover to a temp file the OS can show (`file://` URL).
fn cover_file(track: &str) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    track.hash(&mut h);
    let file = std::env::temp_dir().join(format!("homefi-cover-{:x}.jpg", h.finish()));
    if !file.exists() {
        let bytes = homefi_core::library::read_cover(std::path::Path::new(track))?;
        std::fs::write(&file, bytes).ok()?;
    }
    Some(format!("file://{}", file.display()))
}
