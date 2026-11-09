//! DJ mode on the desktop: the Tauri side of `homefi_player::dj`.
//!
//! While DJ mode is on, a [`DjPlayer`] owns the output device and the normal player is parked
//! (silent, remembering where it was; see `Player::set_parked`). Turning it off closes the DJ
//! stream first and only then gives the device back, paused where it was.
//!
//! Events: `dj` (the [`DjState`], about 20 times a second while on), `dj-load` (a [`DjEvent`]:
//! loading, loaded, failed, device lost), and for songs picked elsewhere in the app while DJ mode
//! is on (see [`take_pick`]): `dj-picked` (it went to a deck), `dj-ask` (the UI asks which deck)
//! and `dj-crate` (it goes to the crate).
//!
//! Party / share: the hub follows the normal player, which is parked, so guests see nothing
//! playing and get no audio until DJ mode ends. A master-mix stream for guests comes later.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use homefi_core::dj::analysis::OVERVIEW_PER_SECOND;
use homefi_core::dj::keymap::{self, Binding};
use homefi_core::dj::{free_deck, AnalysisCache, DeckUse, DjSettings, LoadTarget};
use homefi_player::dj::{DeckId, DjControl, DjEvent, DjPlayer, DjSnapshot};
use homefi_player::Status;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::AppState;
use crate::stream::StreamItem;

type CmdResult<T> = Result<T, String>;

/// How often the screen gets the mixer's state while DJ mode is on.
const TICK: Duration = Duration::from_millis(50);
/// How long the normal player may take to let go of the device.
const PARK_TIMEOUT: Duration = Duration::from_millis(800);
const PARK_POLL: Duration = Duration::from_millis(10);
/// Songs whose tempo one `dj_cached_bpm` call looks up at most (a screen of rows).
const MAX_BPM_LOOKUPS: usize = 200;

/// The DJ engine, when DJ mode is on, and what each deck holds (as the screen names it).
#[derive(Default)]
pub struct Dj {
    player: Mutex<Option<DjPlayer>>,
    picks: Mutex<[Option<DjPick>; 2]>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Dj {
    fn with<T>(&self, f: impl FnOnce(&DjPlayer) -> T) -> Option<T> {
        lock(&self.player).as_ref().map(f)
    }

    fn take(&self) -> Option<DjPlayer> {
        *lock(&self.picks) = Default::default();
        lock(&self.player).take()
    }

    fn set_pick(&self, deck: DeckId, pick: Option<DjPick>) {
        lock(&self.picks)[deck.index()] = pick;
    }

    pub fn is_on(&self) -> bool {
        self.with(|_| ()).is_some()
    }
}

/// What the screen gets: on or off, the mixer's state while on, and the songs' names and covers
/// (the engine only knows a title), so any window can draw the decks.
#[derive(Debug, Clone, Serialize)]
pub struct DjState {
    pub on: bool,
    pub snapshot: Option<DjSnapshot>,
    pub decks: [Option<DjPick>; 2],
}

/// A song for a deck: a file, or a TIDAL track (its local copy when the library has one).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DjSource {
    Path {
        path: String,
        #[serde(default)]
        title: String,
    },
    Tidal {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        local: Option<String>,
    },
}

/// A song for a deck as the screen names it: what to load, title, artist, cover. Also what a
/// song picked elsewhere while DJ mode is on carries (`dj-picked`, `dj-ask`, `dj-crate`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DjPick {
    pub source: DjSource,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// The deck it is on / went to.
    #[serde(default)]
    pub deck: Option<DeckId>,
}

/// The analysis the waveforms draw. `slices` is base64 of `[peak, low, mid, high]` bytes per
/// slice ([`OVERVIEW_PER_SECOND`] slices a second): a fraction of the JSON of the same numbers.
#[derive(Debug, Clone, Serialize)]
pub struct DjOverview {
    pub duration_ms: f64,
    pub bpm: f64,
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    pub confidence: f32,
    pub slices_per_second: u32,
    pub slices: String,
}

/// The shortcuts in effect (defaults with the user's changes) and keys used twice.
#[derive(Debug, Clone, Serialize)]
pub struct DjKeymap {
    pub bindings: Vec<Binding>,
    pub conflicts: Vec<(String, Vec<String>)>,
}

fn state_of(dj: &Dj) -> DjState {
    let snapshot = dj.with(DjPlayer::snapshot);
    let decks = lock(&dj.picks).clone();
    DjState { on: snapshot.is_some(), snapshot, decks }
}

/// First letter up: the engine words its errors as the middle of a sentence.
fn sentence(message: &str) -> String {
    let mut chars = message.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Waits (briefly) until the normal player has closed its output.
async fn wait_parked(app: &AppHandle) {
    let player = app.state::<AppState>().player.clone();
    let deadline = tokio::time::Instant::now() + PARK_TIMEOUT;
    while player.snapshot().status != Status::Stopped && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(PARK_POLL).await;
    }
}

/// Sends the mixer's state to the screen while DJ mode is on; ends with it.
fn spawn_ticker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            let state = state_of(&app.state::<Dj>());
            let _ = app.emit("dj", &state);
            if !state.on {
                break;
            }
        }
    });
}

/// Turns DJ mode on (the normal player is parked first: one engine owns the device) or off (the
/// normal player gets the device back, paused where it was).
#[tauri::command]
pub async fn dj_enable(app: AppHandle, on: bool) -> CmdResult<DjState> {
    let dj = app.state::<Dj>();
    let player = app.state::<AppState>().player.clone();
    if !on {
        if let Some(engine) = dj.take() {
            // the stream closes (its thread joins) before the device is handed back
            let _ = tauri::async_runtime::spawn_blocking(move || engine.stop()).await;
            player.set_parked(false);
        }
        let state = state_of(&dj);
        let _ = app.emit("dj", &state);
        return Ok(state);
    }
    if dj.is_on() {
        return Ok(state_of(&dj));
    }

    player.set_parked(true);
    wait_parked(&app).await;
    let settings = app.state::<AppState>().settings();
    let events = {
        let app = app.clone();
        move |event: DjEvent| {
            let _ = app.emit("dj-load", &event);
        }
    };
    let started = tauri::async_runtime::spawn_blocking(move || {
        DjPlayer::start(&settings.dj, settings.output_device.as_deref(), events)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r.map_err(|e| sentence(&e.to_string())));
    match started {
        Ok(engine) => {
            *dj.player.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(engine);
            spawn_ticker(app.clone());
            Ok(state_of(&dj))
        }
        Err(message) => {
            tracing::warn!("DJ mode did not start: {message}");
            player.set_parked(false);
            Err(message)
        }
    }
}

#[tauri::command]
pub fn dj_state(dj: State<'_, Dj>) -> DjState {
    state_of(&dj)
}

/// Puts a song on a deck. A TIDAL track is fetched whole first (the stream cache, same quality as
/// the normal player), then decoded and analyzed; progress arrives as `dj-load` events.
#[tauri::command]
pub async fn dj_load(app: AppHandle, deck: DeckId, pick: DjPick) -> CmdResult<()> {
    let dj = app.state::<Dj>();
    if !dj.is_on() {
        return Err("DJ mode is off.".into());
    }
    dj.set_pick(deck, Some(DjPick { deck: Some(deck), ..pick.clone() }));
    let (path, title) = resolve(&app, &pick.source).await?;
    let title = if pick.title.is_empty() { title } else { pick.title };
    dj.with(|p| p.load(deck, path, title)).ok_or_else(|| "DJ mode is off.".to_owned())
}

async fn resolve(app: &AppHandle, source: &DjSource) -> CmdResult<(PathBuf, String)> {
    match source {
        DjSource::Path { path, title } => {
            let path = PathBuf::from(path);
            let title = if title.is_empty() { file_title(&path) } else { title.clone() };
            Ok((path, title))
        }
        DjSource::Tidal { id, title, local } => {
            let item = StreamItem {
                id: id.clone(),
                title: title.clone(),
                local: local.clone(),
                artist: String::new(),
                album: String::new(),
                cover_url: None,
                duration_ms: None,
                explicit: false,
                source: None,
            };
            let path = crate::stream::fetch_or_local(app, &item).await?;
            Ok((path, title.clone()))
        }
    }
}

fn file_title(path: &std::path::Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[tauri::command]
pub fn dj_control(dj: State<'_, Dj>, control: DjControl) -> CmdResult<()> {
    if let DjControl::Eject { deck } = control {
        dj.set_pick(deck, None);
    }
    dj.with(|p| p.control(control)).ok_or_else(|| "DJ mode is off.".to_owned())
}

/// The overview and beat grid of the song on `deck` (`None` while it is empty or loading).
#[tauri::command(async)]
pub fn dj_overview(dj: State<'_, Dj>, deck: DeckId) -> Option<DjOverview> {
    let analysis = dj.with(|p| p.analysis(deck)).flatten()?;
    let bytes: Vec<u8> = analysis.overview.iter().flat_map(|s| <[u8; 4]>::from(*s)).collect();
    Some(DjOverview {
        duration_ms: analysis.duration_ms,
        bpm: analysis.bpm,
        first_beat_ms: analysis.first_beat_ms,
        beat_ms: analysis.beat_ms,
        confidence: analysis.confidence,
        slices_per_second: OVERVIEW_PER_SECOND,
        slices: STANDARD.encode(bytes),
    })
}

#[tauri::command]
pub fn dj_keymap(state: State<'_, AppState>) -> DjKeymap {
    let bindings = keymap::resolve(&state.settings().dj.key_overrides);
    let conflicts = keymap::conflicts(&bindings);
    DjKeymap { bindings, conflicts }
}

/// The tempo of songs already analyzed (`None`: not yet, or no trusted beat), for the crate.
#[tauri::command]
pub async fn dj_cached_bpm(paths: Vec<String>) -> Vec<Option<f64>> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = AnalysisCache::in_app_dir();
        paths
            .iter()
            .take(MAX_BPM_LOOKUPS)
            .map(|p| cache.bpm(&AnalysisCache::key_for_file(std::path::Path::new(p))))
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// New DJ settings take effect at once (the buffer size at the next start).
pub fn on_settings(app: &AppHandle, before: &DjSettings, after: &DjSettings) {
    if before != after {
        app.state::<Dj>().with(|p| p.set_settings(after));
    }
}

/// A song picked elsewhere in the app (a library row, a TIDAL list) while DJ mode is on goes
/// where `load_target` says instead of the parked player. `false`: DJ mode is off, play normally.
pub fn take_pick(app: &AppHandle, mut pick: DjPick) -> bool {
    let Some(snapshot) = app.state::<Dj>().with(DjPlayer::snapshot) else { return false };
    match app.state::<AppState>().settings().dj.load_target {
        LoadTarget::Ask => {
            let _ = app.emit("dj-ask", &pick);
        }
        LoadTarget::Crate => {
            let _ = app.emit("dj-crate", &pick);
        }
        LoadTarget::FreeDeck => {
            let uses = snapshot
                .decks
                .each_ref()
                .map(|d| DeckUse { loaded: d.track.is_some() || d.loading, playing: d.playing });
            let deck = DeckId::from_index(free_deck(uses, snapshot.crossfader));
            pick.deck = Some(deck);
            let _ = app.emit("dj-picked", &pick);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(message) = dj_load(app.clone(), deck, pick).await {
                    let _ = app.emit("dj-load", &DjEvent::LoadFailed { deck, message });
                }
            });
        }
    }
    true
}

/// `play_tracks`' pick (the song clicked among `paths`).
pub fn take_file(app: &AppHandle, paths: &[String], index: usize) -> bool {
    let Some(path) = paths.get(index).or_else(|| paths.first()) else { return false };
    let source = DjSource::Path { path: path.clone(), title: String::new() };
    let track = app.state::<AppState>().track(std::path::Path::new(path));
    let pick = DjPick {
        title: track.as_ref().map(|t| t.title.clone()).unwrap_or_else(|| file_title(std::path::Path::new(path))),
        artist: track.as_ref().map(|t| t.artist.clone()).unwrap_or_default(),
        cover_url: None,
        duration_ms: track.as_ref().map(|t| t.duration_ms),
        source,
        deck: None,
    };
    take_pick(app, pick)
}

/// `play_tidal`'s pick.
pub fn take_tidal(app: &AppHandle, items: &[StreamItem], index: usize) -> bool {
    let Some(item) = items.get(index).or_else(|| items.first()) else { return false };
    let pick = DjPick {
        source: DjSource::Tidal { id: item.id.clone(), title: item.title.clone(), local: item.local.clone() },
        title: item.title.clone(),
        artist: item.artist.clone(),
        cover_url: item.cover_url.clone(),
        duration_ms: item.duration_ms,
        deck: None,
    };
    take_pick(app, pick)
}

/// Closing the app: the DJ stream goes first.
pub fn shutdown(app: &AppHandle) {
    if let Some(engine) = app.state::<Dj>().take() {
        engine.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_read_the_screens_json() {
        let s: DjSource = serde_json::from_str(r#"{"kind":"tidal","id":"123","title":"Dreams"}"#).unwrap();
        assert!(matches!(s, DjSource::Tidal { ref id, local: None, .. } if id == "123"));
        let s: DjSource = serde_json::from_str(r#"{"kind":"path","path":"/m/a.flac"}"#).unwrap();
        assert!(matches!(s, DjSource::Path { ref title, .. } if title.is_empty()));
        assert!(serde_json::from_str::<DjSource>(r#"{"kind":"url","url":"x"}"#).is_err());
        assert_eq!(file_title(std::path::Path::new("/m/One More Time.flac")), "One More Time");
    }

    #[test]
    fn picks_read_the_screens_json_and_are_forgotten_with_the_engine() {
        let p: DjPick = serde_json::from_str(r#"{"source":{"kind":"path","path":"/m/a.flac"},"title":"A"}"#).unwrap();
        assert!(p.artist.is_empty() && p.deck.is_none() && p.cover_url.is_none());
        let dj = Dj::default();
        dj.set_pick(DeckId::B, Some(p));
        assert!(state_of(&dj).decks[1].is_some());
        assert!(dj.take().is_none(), "no engine was running");
        assert!(state_of(&dj).decks.iter().all(Option::is_none));
    }

    #[test]
    fn engine_errors_read_as_sentences() {
        assert_eq!(sentence("could not start the DJ audio: busy"), "Could not start the DJ audio: busy");
        assert_eq!(sentence(""), "");
    }

    #[test]
    fn the_state_off_has_no_snapshot() {
        let dj = Dj::default();
        assert!(!dj.is_on());
        let s = state_of(&dj);
        assert!(!s.on && s.snapshot.is_none() && s.decks.iter().all(Option::is_none));
        assert_eq!(serde_json::to_value(&s).unwrap()["on"], false);
    }
}
