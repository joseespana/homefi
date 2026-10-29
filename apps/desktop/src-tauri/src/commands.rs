//! Tauri commands used by the web UI (`src/lib/api.ts`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use homefi_core::catalog::SearchResults;
use homefi_core::download::{self, DownloadOptions, Downloader, Observer};
use homefi_core::library::{self, TrackInfo};
use homefi_core::resource::Resource;
use homefi_core::session::{LoginPoll, Session};
use homefi_player::{list_devices, OutputDevice, Player, RepeatMode, Snapshot};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::mpsc;

use crate::settings::DesktopSettings;

type CmdResult<T> = Result<T, String>;

/// An error as the screen shows it: plain words, no URLs or codes (see
/// `homefi_core::humane`). The technical text goes to the log.
pub(crate) fn err<E: std::fmt::Display + 'static>(e: E) -> String {
    use homefi_core::Error;
    let any = &e as &dyn std::any::Any;
    let humane = if let Some(e) = any.downcast_ref::<Error>() {
        Some(e.humane())
    } else if let Some(e) = any.downcast_ref::<reqwest::Error>() {
        Some(homefi_core::humane::humane_http(e))
    } else {
        any.downcast_ref::<std::io::Error>().map(|e| Error::Io(std::io::Error::new(e.kind(), e.to_string())).humane())
    };
    match humane {
        Some(text) => {
            tracing::warn!("{e}");
            text
        }
        // already worded (our own messages)
        None => e.to_string(),
    }
}

/// A library track as the UI sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub year: Option<u32>,
    pub genre: String,
    pub duration_ms: u64,
    pub format: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub lossless: bool,
    /// `FLAC · 24-bit / 96 kHz`
    pub quality: String,
    pub isrc: Option<String>,
}

impl From<TrackInfo> for TrackView {
    fn from(t: TrackInfo) -> Self {
        Self {
            quality: t.quality_label(),
            path: t.path.to_string_lossy().into_owned(),
            title: t.title,
            artist: t.artist,
            album: t.album,
            album_artist: t.album_artist,
            track_number: t.track_number,
            disc_number: t.disc_number,
            year: t.year,
            genre: t.genre,
            duration_ms: t.duration_ms,
            format: t.format,
            sample_rate: t.sample_rate,
            bit_depth: t.bit_depth,
            lossless: t.lossless,
            isrc: t.isrc,
        }
    }
}

/// Player state + the track being heard.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    #[serde(flatten)]
    pub snapshot: Snapshot,
    pub track: Option<TrackView>,
}

pub struct AppState {
    pub session: Session,
    pub player: Player,
    pub library: Mutex<Vec<TrackView>>,
    pub settings: Mutex<DesktopSettings>,
    pub settings_path: PathBuf,
    pub library_cache: PathBuf,
    pub jobs: mpsc::UnboundedSender<Job>,
    pub cancel: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    /// Bumped by every `play`, so a running Tidal stream knows it was replaced.
    pub stream_generation: AtomicU64,
    /// Tidal tracks of the stream still to be fetched (they join the player's
    /// queue just before they play), so the queue views can show them.
    pub upcoming: Mutex<Vec<crate::stream::Upcoming>>,
    /// Tags of played files outside the library (Tidal cache, opened folders).
    pub outside: Mutex<HashMap<PathBuf, Option<TrackView>>>,
    /// The Tidal client of the signed-in account, reused by every command (one
    /// connection pool instead of a new client and TLS handshake per call).
    pub api_cache: Mutex<Option<(ApiKey, homefi_core::api::TidalApi)>>,
    /// Tidal lists streaming to the UI page by page, by load id (see [`tidal_library_stream`]).
    pub loads: Loads,
    /// Settings changed in memory and not written yet (see [`settings_writer`]).
    pub settings_dirty: Arc<tokio::sync::Notify>,
}

/// Writes the settings file shortly after they change, off the UI thread.
pub async fn settings_writer<R: Runtime>(app: AppHandle<R>) {
    let dirty = app.state::<AppState>().settings_dirty.clone();
    loop {
        dirty.notified().await;
        // changes in a burst (a slider drag) are one write
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let state = app.state::<AppState>();
        let (settings, path) = (state.settings(), state.settings_path.clone());
        match tokio::task::spawn_blocking(move || settings.save(&path)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("could not save the settings: {e}"),
            Err(e) => tracing::warn!("could not save the settings: {e}"),
        }
    }
}

/// What a cached Tidal client was built for: a new login (or a refreshed token) builds a new one.
pub type ApiKey = (Option<String>, Option<String>, Option<String>);

/// Cancel switches of the Tidal lists streaming to the UI: navigating away
/// stops a load, so stale pages don't compete with the view the user is on.
#[derive(Default)]
pub struct Loads(Mutex<HashMap<u64, tokio::sync::watch::Sender<bool>>>);

impl Loads {
    fn start(&self, id: u64) -> tokio::sync::watch::Receiver<bool> {
        let (tx, rx) = tokio::sync::watch::channel(false);
        self.0.lock().unwrap().insert(id, tx);
        rx
    }

    fn cancel(&self, id: u64) {
        if let Some(tx) = self.0.lock().unwrap().remove(&id) {
            let _ = tx.send(true);
        }
    }

    fn finish(&self, id: u64) {
        self.0.lock().unwrap().remove(&id);
    }
}

impl AppState {
    pub fn settings(&self) -> DesktopSettings {
        self.settings.lock().unwrap().clone()
    }

    /// Changes the settings in memory at once; the file is written a moment
    /// later by [`settings_writer`] (a volume drag is one write, not hundreds,
    /// and no caller waits on the disk).
    pub(crate) fn save_settings(&self, update: impl FnOnce(&mut DesktopSettings)) -> CmdResult<DesktopSettings> {
        let saved = {
            let mut s = self.settings.lock().unwrap();
            update(&mut s);
            s.clone()
        };
        self.settings_dirty.notify_one();
        Ok(saved)
    }

    /// Writes the settings file now (closing the app).
    pub fn flush_settings(&self) {
        if let Err(e) = self.settings().save(&self.settings_path) {
            tracing::warn!("could not save the settings: {e}");
        }
    }

    /// Library entry for `path`, or its tags read from disk. No lock is held
    /// while the file is read (the player, the UI and the engine all ask).
    pub fn track(&self, path: &Path) -> Option<TrackView> {
        let key = path.to_string_lossy();
        if let Some(t) = self.library.lock().unwrap().iter().find(|t| t.path == key) {
            return Some(t.clone());
        }
        if let Some(known) = self.outside.lock().unwrap().get(path) {
            return known.clone();
        }
        let read = library::read_track(path).ok().map(TrackView::from);
        self.outside.lock().unwrap().entry(path.to_path_buf()).or_insert(read).clone()
    }

    /// The Tidal client of the signed-in account (built once, rebuilt after a
    /// new login or token).
    pub fn api(&self) -> homefi_core::Result<homefi_core::api::TidalApi> {
        let data = self.session.auth_data()?;
        let key: ApiKey = (data.token, data.user_id, data.country_code);
        if let Some((k, api)) = self.api_cache.lock().unwrap().as_ref() {
            if *k == key {
                return Ok(api.clone());
            }
        }
        let api = self.session.api()?;
        *self.api_cache.lock().unwrap() = Some((key, api.clone()));
        Ok(api)
    }
}

// ----------------------------------------------------------------- app / auth

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    app_dir: String,
    ffmpeg: bool,
    platform: &'static str,
}

#[tauri::command(async)]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: homefi_core::VERSION,
        app_dir: state.session.app_dir().display().to_string(),
        ffmpeg: homefi_core::ffmpeg::is_available(),
        platform: std::env::consts::OS,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    user_id: String,
    country_code: String,
}

#[tauri::command(async)]
pub fn account(state: State<'_, AppState>) -> Option<Account> {
    let data = state.session.auth_data().ok().filter(|d| d.is_logged_in())?;
    Some(Account { user_id: data.user_id.unwrap_or_default(), country_code: data.country_code.unwrap_or_default() })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLogin {
    device_code: String,
    user_code: String,
    url: String,
    interval: i64,
    expires_in: i64,
}

#[tauri::command]
pub async fn start_login(state: State<'_, AppState>) -> CmdResult<DeviceLogin> {
    let d = state.session.start_login().await.map_err(err)?;
    Ok(DeviceLogin {
        url: d.url(),
        device_code: d.device_code,
        user_code: d.user_code,
        interval: d.interval.max(1),
        expires_in: d.expires_in,
    })
}

/// `pending`, `success` or `expired`.
#[tauri::command]
pub async fn poll_login(state: State<'_, AppState>, device_code: String) -> CmdResult<&'static str> {
    Ok(match state.session.poll_login(&device_code).await.map_err(err)? {
        LoginPoll::Pending => "pending",
        LoginPoll::Success(_) => "success",
        LoginPoll::Expired => "expired",
    })
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> CmdResult<()> {
    if let Err(e) = state.session.logout(true).await {
        tracing::warn!("remote logout failed: {e}");
    }
    Ok(())
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> CmdResult<SearchResults> {
    let api = state.api().map_err(err)?;
    let results = api.search(&query).await.map_err(err)?;
    Ok(SearchResults::grouped(&results, &state.settings().album_versions, VERSION_LANG))
}

// ----------------------------------------------------------------- my Tidal

use homefi_core::api::all_pages;
use homefi_core::catalog::{self, Hit};
use homefi_core::models::MediaItem;

/// The desktop is in English.
const VERSION_LANG: homefi_core::album_versions::Lang = homefi_core::album_versions::Lang::En;

/// Album cards (one per album, its releases in `versions`) as the settings say.
fn album_cards(state: &AppState, albums: &[homefi_core::models::Album]) -> Vec<Hit> {
    catalog::album_cards(albums, &state.settings().album_versions, VERSION_LANG)
}

/// Pages fetched at once when the rest of a list is wanted.
const PARALLEL_PAGES: usize = 4;

/// One page of a Tidal list, or everything from `offset` to the end.
#[derive(Clone, Serialize)]
pub struct HitPage {
    items: Vec<Hit>,
    /// Items in the whole list, as Tidal counts them.
    total: i64,
    /// Where the next page starts; `None` once the end was reached.
    next_offset: Option<i64>,
}

/// The page at `offset` and, with `rest`, every page after it (a few at a
/// time, in parallel), so the list ends there.
async fn pages_from<T, F, Fut>(offset: i64, rest: bool, mut fetch: F) -> homefi_core::Result<(Vec<T>, i64, Option<i64>)>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = homefi_core::Result<homefi_core::models::Page<T>>>,
{
    use futures::{stream, StreamExt, TryStreamExt};

    let first = fetch(offset).await?;
    let total = first.total_number_of_items;
    let next = first.next_offset();
    let step = if first.limit > 0 { first.limit } else { first.items.len() as i64 };
    let mut items = first.items;
    if !rest {
        return Ok((items, total, next));
    }
    if let Some(start) = next {
        let offsets = (0..).map(|i| start + i * step).take_while(|&o| o < total);
        let pages: Vec<_> = stream::iter(offsets.map(&mut fetch)).buffered(PARALLEL_PAGES).try_collect().await?;
        for page in pages {
            items.extend(page.items);
        }
    }
    Ok((items, total, None))
}

/// Resolves once `rx` says "cancelled" (never, if nobody can cancel any more).
async fn cancelled(mut rx: tokio::sync::watch::Receiver<bool>) {
    if rx.wait_for(|c| *c).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// Sends every page from `offset` to the end through `on_page`, in order, as
/// each arrives (a few fetched at once), so the list fills while the user goes
/// on. Stops when `cancel` fires. Returns how many pages were sent.
async fn stream_pages<T, F, Fut>(
    offset: i64,
    mut fetch: F,
    to_hits: impl Fn(Vec<T>) -> Vec<Hit>,
    on_page: &tauri::ipc::Channel<HitPage>,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> homefi_core::Result<usize>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = homefi_core::Result<homefi_core::models::Page<T>>>,
{
    use futures::{stream, StreamExt};

    let send = |page: homefi_core::models::Page<T>| {
        let next_offset = page.next_offset();
        let total = page.total_number_of_items;
        // a closed window (or page) is no reason to fail the command
        let _ = on_page.send(HitPage { items: to_hits(page.items), total, next_offset });
    };
    let first = tokio::select! {
        biased;
        _ = cancelled(cancel.clone()) => return Ok(0),
        page = fetch(offset) => page?,
    };
    let total = first.total_number_of_items;
    let step = if first.limit > 0 { first.limit } else { first.items.len() as i64 };
    let start = first.next_offset();
    send(first);
    let mut sent = 1;
    let Some(start) = start else { return Ok(sent) };
    let offsets = (0..).map(|i| start + i * step).take_while(|&o| o < total);
    let mut pages = stream::iter(offsets.map(&mut fetch)).buffered(PARALLEL_PAGES);
    let stop = cancelled(cancel);
    tokio::pin!(stop);
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            page = pages.next() => match page {
                Some(page) => {
                    send(page?);
                    sent += 1;
                }
                None => break,
            },
        }
    }
    Ok(sent)
}

fn hit_page<T>(page: (Vec<T>, i64, Option<i64>), to_hits: impl FnOnce(Vec<T>) -> Vec<Hit>) -> HitPage {
    let (items, total, next_offset) = page;
    HitPage { items: to_hits(items), total, next_offset }
}

fn playlist_entries(v: Vec<homefi_core::models::PlaylistEntry>) -> Vec<Hit> {
    v.into_iter()
        .map(|e| {
            let own = e.is_own();
            let mut hit = catalog::playlist_hit(&e.playlist);
            hit.subtitle = format!("{} · {}", if own { "Yours" } else { "Following" }, hit.subtitle);
            hit
        })
        .collect()
}

/// Every page of the user's Tidal collection from `offset`, each sent through
/// `on_page` as it arrives (see [`stream_pages`]); `tidal_cancel(load)` stops it.
/// Returns how many pages were sent.
#[tauri::command]
pub async fn tidal_library_stream(
    state: State<'_, AppState>,
    kind: String,
    offset: i64,
    load: u64,
    on_page: tauri::ipc::Channel<HitPage>,
) -> CmdResult<usize> {
    let api = state.api().map_err(err)?;
    let cancel = state.loads.start(load);
    let sent = match kind.as_str() {
        "playlists" => {
            stream_pages(offset, |o| api.playlists_and_favorites(o), playlist_entries, &on_page, cancel).await
        }
        "albums" => {
            // grouped page by page: versions saved far apart stay two cards
            let hits = |v: Vec<homefi_core::models::Favorite<homefi_core::models::Album>>| {
                album_cards(&state, &v.into_iter().map(|f| f.item).collect::<Vec<_>>())
            };
            stream_pages(offset, |o| api.favorite_albums(o), hits, &on_page, cancel).await
        }
        "artists" => {
            let hits = |v: Vec<homefi_core::models::Favorite<homefi_core::models::Artist>>| {
                v.iter().map(|f| catalog::artist_hit(&f.item)).collect()
            };
            stream_pages(offset, |o| api.favorite_artists(o), hits, &on_page, cancel).await
        }
        "tracks" => {
            let hits = |v: Vec<homefi_core::models::Favorite<homefi_core::models::Track>>| {
                v.iter().map(|f| catalog::track_hit(&f.item)).collect()
            };
            stream_pages(offset, |o| api.favorite_tracks(o), hits, &on_page, cancel).await
        }
        other => {
            state.loads.finish(load);
            return Err(format!("unknown collection `{other}`"));
        }
    };
    state.loads.finish(load);
    sent.map_err(err)
}

/// Every track of an album, playlist or mix from `offset`, page by page (see
/// [`tidal_library_stream`]).
#[tauri::command]
pub async fn tidal_detail_stream(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    offset: i64,
    load: u64,
    on_page: tauri::ipc::Channel<HitPage>,
) -> CmdResult<usize> {
    let api = state.api().map_err(err)?;
    let cancel = state.loads.start(load);
    let sent = match kind.as_str() {
        "album" => stream_pages(offset, |o| api.album_items(&id, o), tracks_of, &on_page, cancel).await,
        "playlist" => stream_pages(offset, |o| api.playlist_items(&id, o), tracks_of, &on_page, cancel).await,
        "mix" => stream_pages(offset, |o| api.mix_items(&id, o), tracks_of, &on_page, cancel).await,
        other => {
            state.loads.finish(load);
            return Err(format!("`{other}` has no track list"));
        }
    };
    state.loads.finish(load);
    sent.map_err(err)
}

/// Stops a list streaming to the UI (the user went elsewhere).
#[tauri::command]
pub fn tidal_cancel(state: State<'_, AppState>, load: u64) {
    state.loads.cancel(load);
}

/// A page of the user's Tidal collection (`playlists`, `albums`, `artists` or
/// `tracks`) from `offset`; `rest` returns everything from there to the end.
#[tauri::command]
pub async fn tidal_library_page(
    state: State<'_, AppState>,
    kind: String,
    offset: i64,
    rest: bool,
) -> CmdResult<HitPage> {
    let api = state.api().map_err(err)?;
    Ok(match kind.as_str() {
        "playlists" => {
            hit_page(pages_from(offset, rest, |o| api.playlists_and_favorites(o)).await.map_err(err)?, playlist_entries)
        }
        "albums" => hit_page(pages_from(offset, rest, |o| api.favorite_albums(o)).await.map_err(err)?, |v| {
            album_cards(&state, &v.into_iter().map(|f| f.item).collect::<Vec<_>>())
        }),
        "artists" => hit_page(pages_from(offset, rest, |o| api.favorite_artists(o)).await.map_err(err)?, |v| {
            v.iter().map(|f| catalog::artist_hit(&f.item)).collect()
        }),
        "tracks" => hit_page(pages_from(offset, rest, |o| api.favorite_tracks(o)).await.map_err(err)?, |v| {
            v.iter().map(|f| catalog::track_hit(&f.item)).collect()
        }),
        other => return Err(format!("unknown collection `{other}`")),
    })
}

#[derive(Serialize)]
pub struct TidalDetail {
    header: Hit,
    /// A playlist the user created: tracks can be added / removed.
    owned: bool,
    description: Option<String>,
    /// The first page (`lazy`) or every track.
    tracks: Vec<Hit>,
    albums: Vec<Hit>,
    /// Tracks in the whole list (Tidal's count; may include videos, which are left out).
    total: i64,
    /// Length of the whole list, when Tidal says it.
    duration_ms: Option<u64>,
    /// Offset for [`tidal_detail_page`]; `None` when `tracks` is the whole list.
    next_offset: Option<i64>,
}

fn tracks_of(items: Vec<homefi_core::models::ListedItem>) -> Vec<Hit> {
    items
        .into_iter()
        .filter_map(|l| match l.item {
            MediaItem::Track(t) => Some(catalog::track_hit(&t)),
            MediaItem::Video(_) => None,
        })
        .collect()
}

fn seconds_ms(s: i64) -> Option<u64> {
    u64::try_from(s).ok().filter(|&s| s > 0).map(|s| s * 1000)
}

/// Tracks of an album, playlist or mix from `offset`; `rest` returns everything from there to the end.
async fn detail_tracks(
    api: &homefi_core::api::TidalApi,
    kind: &str,
    id: &str,
    offset: i64,
    rest: bool,
) -> CmdResult<HitPage> {
    let page = match kind {
        "album" => pages_from(offset, rest, |o| api.album_items(id, o)).await,
        "playlist" => pages_from(offset, rest, |o| api.playlist_items(id, o)).await,
        "mix" => pages_from(offset, rest, |o| api.mix_items(id, o)).await,
        other => return Err(format!("`{other}` has no track list")),
    };
    Ok(hit_page(page.map_err(err)?, tracks_of))
}

/// Contents of an album, playlist, mix or artist on Tidal. With `lazy` only the
/// first page of tracks comes back; [`tidal_detail_page`] brings the rest.
#[tauri::command]
pub async fn tidal_detail(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    lazy: Option<bool>,
) -> CmdResult<TidalDetail> {
    let api = state.api().map_err(err)?;
    let rest = !lazy.unwrap_or(false);
    Ok(match kind.as_str() {
        "album" => {
            let (album, page) = tokio::try_join!(
                async { api.album(&id).await.map_err(err) },
                detail_tracks(&api, "album", &id, 0, rest)
            )?;
            TidalDetail {
                header: catalog::album_hit(&album),
                owned: false,
                description: album.copyright.clone(),
                total: page.total,
                duration_ms: seconds_ms(album.duration),
                next_offset: page.next_offset,
                tracks: page.items,
                albums: vec![],
            }
        }
        "playlist" => {
            let (playlist, page) = tokio::try_join!(
                async { api.playlist(&id).await.map_err(err) },
                detail_tracks(&api, "playlist", &id, 0, rest)
            )?;
            TidalDetail {
                header: catalog::playlist_hit(&playlist),
                owned: playlist.is_owned_by(api.user_id()),
                description: playlist.description.clone(),
                total: page.total,
                duration_ms: seconds_ms(playlist.duration),
                next_offset: page.next_offset,
                tracks: page.items,
                albums: vec![],
            }
        }
        "mix" => {
            let page = detail_tracks(&api, "mix", &id, 0, rest).await?;
            let mut header = page.items.first().cloned().ok_or("empty mix")?;
            header.title = "Mix".into();
            TidalDetail {
                header,
                owned: false,
                description: None,
                total: page.total,
                duration_ms: None,
                next_offset: page.next_offset,
                tracks: page.items,
                albums: vec![],
            }
        }
        "artist" => {
            // the discography is a grid of covers: always whole
            use homefi_core::api::ArtistAlbumsFilter as Filter;
            // the artist, the albums and the singles at the same time
            let (artist, full, singles) = tokio::try_join!(
                async { api.artist(&id).await.map_err(err) },
                async { all_pages(|o| api.artist_albums(&id, o, Filter::Albums)).await.map_err(err) },
                async { all_pages(|o| api.artist_albums(&id, o, Filter::EpsAndSingles)).await.map_err(err) },
            )?;
            // albums and singles are grouped apart: a single never hides in an album's card
            let mut albums = album_cards(&state, &full);
            albums.extend(album_cards(&state, &singles));
            TidalDetail {
                header: catalog::artist_hit(&artist),
                owned: false,
                description: None,
                tracks: vec![],
                albums,
                total: 0,
                duration_ms: None,
                next_offset: None,
            }
        }
        other => return Err(format!("unknown kind `{other}`")),
    })
}

/// Every release of an album (explicit / clean, stereo / Atmos, remasters,
/// deluxe...) as its artist's page lists them, itself included, for the album
/// header's "Other versions".
#[tauri::command]
pub async fn tidal_album_versions(state: State<'_, AppState>, id: String) -> CmdResult<Vec<catalog::VersionHit>> {
    let api = state.api().map_err(err)?;
    let album = api.album(&id).await.map_err(err)?;
    catalog::album_versions(&api, &album, &state.settings().album_versions, VERSION_LANG).await.map_err(err)
}

/// More tracks of an album, playlist or mix, from `offset` (see [`tidal_detail`]).
#[tauri::command]
pub async fn tidal_detail_page(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    offset: i64,
    rest: bool,
) -> CmdResult<HitPage> {
    let api = state.api().map_err(err)?;
    detail_tracks(&api, &kind, &id, offset, rest).await
}

// ----------------------------------------------------------------- collection edits

/// Ids of everything the user has in favorites, by kind.
#[derive(Serialize)]
pub struct FavoriteIds {
    track: Vec<String>,
    album: Vec<String>,
    artist: Vec<String>,
    playlist: Vec<String>,
}

/// Tidal ids of a recording (every release it is on), for local files: to like
/// it or add it to a playlist.
#[tauri::command]
pub async fn tidal_tracks_by_isrc(state: State<'_, AppState>, isrc: String) -> CmdResult<Vec<String>> {
    let api = state.api().map_err(err)?;
    Ok(api.tracks_by_isrc(&isrc).await.map_err(err)?.iter().map(|t| t.id.to_string()).collect())
}

#[tauri::command]
pub async fn tidal_favorite_ids(state: State<'_, AppState>) -> CmdResult<FavoriteIds> {
    let f = state.api().map_err(err)?.favorites().await.map_err(err)?;
    Ok(FavoriteIds { track: f.track, album: f.album, artist: f.artist, playlist: f.playlist })
}

/// Lyrics of what is playing: a local file's own (`.lrc` next to it or its
/// tag) first, then Tidal's. None when neither has any.
#[tauri::command]
pub async fn lyrics(
    state: State<'_, AppState>,
    track_id: Option<String>,
    path: Option<String>,
) -> CmdResult<Option<homefi_core::lyrics::Lyrics>> {
    // signed out still finds a file's own lyrics
    let api = state.api().ok();
    // the file's own lyrics (an .lrc, its tags) are read on a blocking thread
    let own = match path.clone() {
        Some(p) => tokio::task::spawn_blocking(move || homefi_core::lyrics::local(Path::new(&p))).await.map_err(err)?,
        None => None,
    };
    if own.is_some() {
        return Ok(own);
    }
    if let Some(id) = track_id.and_then(|id| id.parse::<i64>().ok()) {
        let Some(api) = api else { return Ok(None) };
        return homefi_core::lyrics::tidal(&api, id).await.map_err(err);
    }
    let (Some(path), Some(_)) = (path, api.as_ref()) else { return Ok(None) };
    homefi_core::lyrics::for_file(api.as_ref(), Path::new(&path)).await.map_err(err)
}

fn kind_of(kind: &str) -> CmdResult<homefi_core::resource::ResourceKind> {
    kind.parse().map_err(|_| format!("unknown kind `{kind}`"))
}

/// Adds (`on`) or removes a track / album / artist / playlist from favorites.
#[tauri::command]
pub async fn tidal_favorite(state: State<'_, AppState>, kind: String, id: String, on: bool) -> CmdResult<()> {
    let api = state.api().map_err(err)?;
    let kind = kind_of(&kind)?;
    if on {
        api.add_favorite(kind, &id).await.map_err(err)
    } else {
        api.remove_favorite(kind, &id).await.map_err(err)
    }
}

/// Playlists the user created (targets for "Add to playlist").
#[tauri::command]
pub async fn tidal_my_playlists(state: State<'_, AppState>) -> CmdResult<Vec<Hit>> {
    let api = state.api().map_err(err)?;
    Ok(all_pages(|o| api.playlists_and_favorites(o))
        .await
        .map_err(err)?
        .into_iter()
        .filter(|e| e.is_own())
        .map(|e| catalog::playlist_hit(&e.playlist))
        .collect())
}

/// Creates a playlist, optionally with tracks already in it.
#[tauri::command]
pub async fn tidal_create_playlist(
    state: State<'_, AppState>,
    title: String,
    description: String,
    track_ids: Vec<String>,
) -> CmdResult<Hit> {
    let api = state.api().map_err(err)?;
    let playlist = api.create_playlist(title.trim(), description.trim()).await.map_err(err)?;
    api.add_to_playlist(&playlist.uuid, &track_ids).await.map_err(err)?;
    let mut hit = catalog::playlist_hit(&playlist);
    hit.subtitle = format!("{} tracks", track_ids.len());
    Ok(hit)
}

#[tauri::command]
pub async fn tidal_playlist_add(state: State<'_, AppState>, uuid: String, track_ids: Vec<String>) -> CmdResult<()> {
    state.api().map_err(err)?.add_to_playlist(&uuid, &track_ids).await.map_err(err)
}

#[tauri::command]
pub async fn tidal_playlist_remove(state: State<'_, AppState>, uuid: String, index: usize) -> CmdResult<()> {
    state.api().map_err(err)?.remove_from_playlist(&uuid, index).await.map_err(err)
}

#[tauri::command]
pub async fn tidal_playlist_delete(state: State<'_, AppState>, uuid: String) -> CmdResult<()> {
    state.api().map_err(err)?.delete_playlist(&uuid).await.map_err(err)
}

// ----------------------------------------------------------------- artwork

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkProgress {
    done: usize,
    total: usize,
    fixed: usize,
    current: String,
}

/// Looks up covers on Tidal for library files without one and embeds them.
#[tauri::command]
pub async fn fix_artwork<R: Runtime>(app: AppHandle<R>) -> CmdResult<ArtworkProgress> {
    let state = app.state::<AppState>();
    let api = state.api().map_err(err)?;
    let http = api.client().http().clone();

    let tracks: Vec<TrackView> = state.library.lock().unwrap().clone();
    let missing: Vec<TrackView> = tokio::task::spawn_blocking(move || {
        tracks.into_iter().filter(|t| library::read_cover(Path::new(&t.path)).is_none()).collect()
    })
    .await
    .map_err(err)?;

    // one lookup per album
    let mut groups: Vec<(String, Vec<TrackView>)> = Vec::new();
    for t in missing {
        let key = format!("{}\u{0}{}", t.album_artist.to_lowercase(), t.album.to_lowercase());
        match groups.iter_mut().find(|(k, _)| *k == key && !t.album.is_empty()) {
            Some((_, list)) => list.push(t),
            None => groups.push((key, vec![t])),
        }
    }

    let total: usize = groups.iter().map(|(_, l)| l.len()).sum();
    let mut progress = ArtworkProgress { done: 0, total, fixed: 0, current: String::new() };
    for (_, list) in groups {
        let first = &list[0];
        progress.current =
            format!("{} – {}", first.artist, if first.album.is_empty() { &first.title } else { &first.album });
        let _ = app.emit("artwork", progress.clone());

        let info = library::read_track(Path::new(&first.path)).map_err(err)?;
        let cover = match homefi_core::artwork::find_cover_id(&api, &info).await {
            Some(id) => homefi_core::artwork::fetch_cover(&http, &id).await,
            None => None,
        };
        for t in &list {
            if let Some(bytes) = &cover {
                let (path, bytes) = (PathBuf::from(&t.path), bytes.clone());
                let ok = tokio::task::spawn_blocking(move || homefi_core::metadata::embed_cover(&path, &bytes)).await;
                match ok {
                    Ok(Ok(())) => progress.fixed += 1,
                    Ok(Err(e)) => tracing::warn!("{}: {e}", t.path),
                    Err(e) => tracing::warn!("{}: {e}", t.path),
                }
            }
            progress.done += 1;
        }
    }
    progress.current.clear();
    let _ = app.emit("artwork", progress.clone());
    Ok(progress)
}

// ----------------------------------------------------------------- folders

/// Tracks in any folder (not added to the library).
#[tauri::command]
pub async fn scan_folder(path: String) -> CmdResult<Vec<TrackView>> {
    let root = PathBuf::from(path);
    let tracks = tokio::task::spawn_blocking(move || library::scan(&[root], None)).await.map_err(err)?;
    Ok(tracks.into_iter().map(TrackView::from).collect())
}

// ----------------------------------------------------------------- downloads

pub struct Job {
    pub label: String,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DownloadView {
    JobStarted { label: String, pending: usize },
    JobFinished { label: String, downloaded: u64, failed: u64 },
    Queued { id: String, title: String },
    Started { task: u64, id: String, title: String, quality: String },
    Progress { task: u64, downloaded: u64, total: Option<u64> },
    Finished { task: u64, id: String, title: String, quality: String, path: String },
    Exists { id: String, title: String, path: String },
    Skipped { id: String, title: String, reason: String },
    Failed { id: Option<String>, context: String, message: String },
}

/// Sends download events to the UI; progress at most ~10 times a second per
/// file (the engine reports every few hundred KiB).
struct EmitObserver<R: Runtime>(AppHandle<R>, Mutex<HashMap<u64, std::time::Instant>>);

const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

impl<R: Runtime> Observer for EmitObserver<R> {
    fn on_event(&self, event: download::Event) {
        use download::Event as E;
        if let E::Progress { task, downloaded, total } = &event {
            let last = total.is_some_and(|t| *downloaded >= t);
            let mut sent = self.1.lock().unwrap();
            let now = std::time::Instant::now();
            if !last && sent.get(task).is_some_and(|at| now.duration_since(*at) < PROGRESS_EVERY) {
                return;
            }
            sent.insert(*task, now);
        }
        if let E::Finished { task, .. } = &event {
            self.1.lock().unwrap().remove(task);
        }
        let view = match event {
            E::Queued { id, title } => DownloadView::Queued { id, title },
            E::Started { task, id, title, quality } => DownloadView::Started { task, id, title, quality },
            E::Progress { task, downloaded, total } => DownloadView::Progress { task, downloaded, total },
            E::Finished { task, id, title, quality, path, .. } => {
                DownloadView::Finished { task, id, title, quality, path: path.display().to_string() }
            }
            E::Exists { id, title, path } => DownloadView::Exists { id, title, path: path.display().to_string() },
            E::Skipped { id, title, reason } => DownloadView::Skipped { id, title, reason },
            E::Failed { id, context, message } => DownloadView::Failed { id, context, message },
            E::ExtraWritten { .. } => return,
        };
        let _ = self.0.emit("download", view);
    }
}

#[tauri::command(async)]
pub fn download(state: State<'_, AppState>, label: String, links: Vec<String>) -> CmdResult<()> {
    for link in &links {
        Resource::parse(link).map_err(err)?;
    }
    state.jobs.send(Job { label, links }).map_err(err)
}

#[tauri::command(async)]
pub fn cancel_downloads(state: State<'_, AppState>) {
    if let Some(tx) = state.cancel.lock().unwrap().as_ref() {
        let _ = tx.send(true);
    }
}

/// Runs download jobs one after another; rescans the library after each.
pub async fn download_worker<R: Runtime>(app: AppHandle<R>, mut jobs: mpsc::UnboundedReceiver<Job>) {
    while let Some(job) = jobs.recv().await {
        let state = app.state::<AppState>();
        let _ = app.emit("download", DownloadView::JobStarted { label: job.label.clone(), pending: jobs.len() });

        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        *state.cancel.lock().unwrap() = Some(cancel_tx);

        let result = async {
            state.session.refresh(600, false).await?;
            let api = state.api()?;
            let settings = state.settings();
            let mut opts = DownloadOptions::from_config(state.session.config());
            opts.download_dir = settings.download_dir.clone();
            opts.scan_dir = settings.download_dir.clone();
            opts.track_quality = settings.quality;
            opts.skip_existing = settings.skip_existing;
            opts.metadata.cover = settings.embed_cover;
            opts.metadata.lyrics = settings.embed_lyrics;
            let resources = job.links.iter().map(|l| Resource::parse(l)).collect::<homefi_core::Result<Vec<_>>>()?;
            let downloader = Downloader::new(api, opts, Arc::new(EmitObserver(app.clone(), Mutex::default())));
            tokio::select! {
                summary = downloader.run(&resources) => summary,
                _ = cancel_rx.wait_for(|c| *c) => Err(homefi_core::Error::Stream("cancelled".into())),
            }
        }
        .await;

        let (downloaded, failed) = match result {
            Ok(s) => (s.downloaded, s.failed),
            Err(e) => {
                let _ = app
                    .emit("download", DownloadView::Failed { id: None, context: job.label.clone(), message: err(e) });
                (0, 1)
            }
        };
        *state.cancel.lock().unwrap() = None;
        let _ = app.emit("download", DownloadView::JobFinished { label: job.label, downloaded, failed });
        if downloaded > 0 {
            let _ = rescan(&app).await;
        }
    }
}

// ----------------------------------------------------------------- library

pub async fn rescan<R: Runtime>(app: &AppHandle<R>) -> CmdResult<Vec<TrackView>> {
    forget_local_playlists();
    // one scan at a time: a second request waits for it, then scans what changed since
    static SCANNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = SCANNING.lock().await;
    let state = app.state::<AppState>();
    let roots = state.settings().library_roots();
    let cache = state.library_cache.clone();
    let tracks: Vec<TrackView> = tokio::task::spawn_blocking(move || library::scan(&roots, Some(&cache)))
        .await
        .map_err(err)?
        .into_iter()
        .map(TrackView::from)
        .collect();
    *state.library.lock().unwrap() = tracks.clone();
    let _ = app.emit("library", tracks.len());
    Ok(tracks)
}

/// An `.m3u` / `.m3u8` playlist found in the library folders (tiddl writes these).
#[derive(Serialize)]
pub struct LocalPlaylistView {
    pub name: String,
    pub path: String,
    pub tracks: Vec<String>,
}

/// The .m3u files found under the music folders, kept until the folders change or the
/// library is rescanned: finding them walks every folder, and the view asks often.
static LOCAL_PLAYLISTS: Mutex<Option<FoundPlaylists>> = Mutex::new(None);

/// The music folders looked through, and what was found there.
type FoundPlaylists = (Vec<PathBuf>, Arc<Vec<LocalPlaylistView>>);

/// Forgets the .m3u files found, so the next `local_playlists` looks again.
pub fn forget_local_playlists() {
    *LOCAL_PLAYLISTS.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[tauri::command]
pub async fn local_playlists(state: State<'_, AppState>) -> CmdResult<Arc<Vec<LocalPlaylistView>>> {
    let roots = state.settings().library_roots();
    if let Some((seen, lists)) = LOCAL_PLAYLISTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *seen == roots {
            return Ok(lists.clone());
        }
    }
    let walk = roots.clone();
    let found = tokio::task::spawn_blocking(move || homefi_core::m3u::find(&walk)).await.map_err(err)?;
    let lists: Arc<Vec<LocalPlaylistView>> = Arc::new(
        found
            .into_iter()
            .map(|p| LocalPlaylistView {
                name: p.name,
                path: p.path.to_string_lossy().into_owned(),
                tracks: p.tracks.iter().map(|t| t.to_string_lossy().into_owned()).collect(),
            })
            .collect(),
    );
    *LOCAL_PLAYLISTS.lock().unwrap_or_else(|e| e.into_inner()) = Some((roots, lists.clone()));
    Ok(lists)
}

#[tauri::command(async)]
pub fn library(state: State<'_, AppState>) -> Vec<TrackView> {
    state.library.lock().unwrap().clone()
}

/// Moves downloaded files to the Trash. Only files that are part of the
/// library can be deleted. Returns the paths that were removed.
#[tauri::command]
pub async fn delete_tracks<R: Runtime>(app: AppHandle<R>, paths: Vec<String>) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let known: Vec<String> = {
        let library = state.library.lock().unwrap();
        paths.into_iter().filter(|p| library.iter().any(|t| &t.path == p)).collect()
    };
    if known.is_empty() {
        return Ok(Vec::new());
    }
    let to_trash = known.clone();
    let removed: Vec<String> = tokio::task::spawn_blocking(move || {
        to_trash
            .into_iter()
            .filter(|p| match trash::delete(p) {
                Ok(()) => true,
                // already gone counts as deleted
                Err(_) if !Path::new(p).exists() => true,
                Err(e) => {
                    tracing::warn!("could not move {p} to the trash: {e}");
                    false
                }
            })
            .collect()
    })
    .await
    .map_err(err)?;
    if removed.len() < known.len() && removed.is_empty() {
        return Err("Could not move the files to the Trash".into());
    }
    state.library.lock().unwrap().retain(|t| !removed.contains(&t.path));
    let _ = app.emit("library", removed.len());
    Ok(removed)
}

/// Drops library entries whose file no longer exists (deleted or moved outside
/// HomeFi) and returns their paths. Cheap: one `stat` per track.
#[tauri::command]
pub async fn prune_library<R: Runtime>(app: AppHandle<R>) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let paths: Vec<String> = state.library.lock().unwrap().iter().map(|t| t.path.clone()).collect();
    let missing: Vec<String> =
        tokio::task::spawn_blocking(move || paths.into_iter().filter(|p| !Path::new(p).exists()).collect::<Vec<_>>())
            .await
            .map_err(err)?;
    if !missing.is_empty() {
        state.library.lock().unwrap().retain(|t| !missing.contains(&t.path));
        state.outside.lock().unwrap().retain(|p, _| p.exists());
        let _ = app.emit("library", missing.len());
    }
    Ok(missing)
}

#[tauri::command]
pub async fn rescan_library<R: Runtime>(app: AppHandle<R>) -> CmdResult<Vec<TrackView>> {
    rescan(&app).await
}

// ----------------------------------------------------------------- player

#[tauri::command(async)]
pub fn player_state(state: State<'_, AppState>) -> PlayerView {
    let snapshot = state.player.snapshot();
    let track = snapshot.current.and_then(|i| snapshot.queue.get(i)).and_then(|p| state.track(p));
    PlayerView { snapshot, track }
}

/// Plays `paths` from `index`; `source` says where they came from (an album,
/// a playlist...), for taste layers and album loudness.
#[tauri::command]
pub fn play_tracks(
    app: AppHandle,
    paths: Vec<String>,
    index: usize,
    shuffle: bool,
    source: Option<homefi_core::dsp::SourceContext>,
) {
    let state = app.state::<AppState>();
    // sharing: the chosen song plays now, the rest joins the shared queue
    if crate::share::host_play_files(&app, &paths, index) {
        return;
    }
    crate::sound::forget_sources(&app);
    for p in &paths {
        crate::sound::remember(&app, Path::new(p), source.clone(), None);
    }
    state.stream_generation.fetch_add(1, Ordering::SeqCst);
    crate::stream::clear_upcoming(&app);
    state.player.set_shuffle(shuffle);
    let start = if shuffle && !paths.is_empty() { rand_index(paths.len()) } else { index };
    state.player.load(paths.into_iter().map(PathBuf::from).collect(), start, 0, true);
}

/// Tidal tracks still to come after the player's queue (see `stream::Upcoming`).
#[tauri::command(async)]
pub fn stream_upcoming(state: State<'_, AppState>) -> Vec<crate::stream::Upcoming> {
    state.upcoming.lock().unwrap().clone()
}

/// Plays Tidal tracks (from `index`), fetching each one just before it's needed.
#[tauri::command(async)]
pub fn play_tidal(
    app: AppHandle,
    items: Vec<crate::stream::StreamItem>,
    index: usize,
    shuffle: bool,
    source: Option<homefi_core::dsp::SourceContext>,
) {
    if crate::share::host_play_tidal(&app, &items, index) {
        return;
    }
    crate::sound::forget_sources(&app);
    let items: Vec<_> = items
        .into_iter()
        .map(|mut i| {
            i.source = i.source.or_else(|| source.clone());
            i
        })
        .collect();
    let state = app.state::<AppState>();
    state.player.set_shuffle(false);
    let mut items: Vec<_> = items.into_iter().skip(index).collect();
    if shuffle && items.len() > 1 {
        use rand_shuffle::shuffle_tail;
        shuffle_tail(&mut items);
    }
    crate::stream::play(app.clone(), items);
}

/// Library or local files to the queue: at the end, or after the current track.
#[tauri::command(async)]
pub fn queue_add(app: AppHandle, paths: Vec<String>, next: bool) {
    if crate::share::host_add_files(&app, &paths, next) {
        return;
    }
    let state = app.state::<AppState>();
    crate::stream::add_all_to_queue(&state.player, paths.into_iter().map(PathBuf::from).collect(), next);
}

/// Tidal tracks to the queue (fetched first; local copies when the library has them).
/// Returns how many were added.
#[tauri::command]
pub async fn queue_tidal(app: AppHandle, items: Vec<crate::stream::StreamItem>, next: bool) -> CmdResult<usize> {
    if crate::share::host_add_tidal(&app, &items, next) {
        return Ok(items.len());
    }
    crate::stream::enqueue(&app, items, next).await
}

mod rand_shuffle {
    /// Fisher-Yates over everything but the first item (the one clicked).
    pub fn shuffle_tail<T>(items: &mut [T]) {
        for i in (2..items.len()).rev() {
            let j = 1 + super::rand_index(i);
            items.swap(i, j);
        }
    }
}

#[tauri::command]
pub fn player_move(app: AppHandle, from: usize, to: usize) {
    if crate::share::host_move(&app, from, to) {
        return;
    }
    app.state::<AppState>().player.move_track(from, to);
}

// ----------------------------------------------------------------- mini player

const MINI: &str = "mini";

/// Opens the always-on-top mini player (bottom-right corner) and minimizes the
/// main window, or closes it and brings the main window back.
#[tauri::command]
pub async fn mini_player<R: Runtime>(app: AppHandle<R>, open: bool) -> CmdResult<()> {
    let main = app.get_webview_window("main");
    if !open {
        if let Some(mini) = app.get_webview_window(MINI) {
            mini.close().map_err(err)?;
        }
        if let Some(main) = main {
            let _ = main.unminimize();
            let _ = main.show();
            let _ = main.set_focus();
        }
        return Ok(());
    }
    if let Some(mini) = app.get_webview_window(MINI) {
        return mini.set_focus().map_err(err);
    }
    let (w, h) = (380.0, 120.0);
    let mini = tauri::WebviewWindowBuilder::new(&app, MINI, tauri::WebviewUrl::App("index.html?mini".into()))
        .title("HomeFi")
        .inner_size(w, h)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .shadow(true)
        // shown by `mini_ready` once its page has drawn: never a blank white window
        .visible(false)
        .build()
        .map_err(err)?;
    if let Ok(Some(monitor)) = mini.current_monitor() {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let x = area.position.x as f64 / scale + area.size.width as f64 / scale - w - 24.0;
        let y = area.position.y as f64 / scale + area.size.height as f64 / scale - h - 24.0;
        let _ = mini.set_position(tauri::LogicalPosition::new(x, y));
    }
    // a page that never comes up (a web view that failed to start) must not leave the
    // user with nothing: close it and keep the main window
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        if let Some(mini) = app2.get_webview_window(MINI) {
            if !mini.is_visible().unwrap_or(false) {
                tracing::warn!("mini player didn't start, back to the main window");
                let _ = mini.close();
                if let Some(main) = app2.get_webview_window("main") {
                    let _ = main.unminimize();
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
        }
    });
    Ok(())
}

/// The mini player's page has drawn: show it, and only then tuck the main window away.
#[tauri::command]
pub async fn mini_ready<R: Runtime>(app: AppHandle<R>) -> CmdResult<()> {
    if let Some(mini) = app.get_webview_window(MINI) {
        mini.show().map_err(err)?;
        let _ = mini.set_focus();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.minimize();
    }
    Ok(())
}

/// Track info for queue entries (`None` when the file can't be read).
/// Reads tags from disk for files outside the library, so it runs on a blocking thread.
#[tauri::command]
pub async fn tracks_for<R: Runtime>(app: AppHandle<R>, paths: Vec<String>) -> CmdResult<Vec<Option<TrackView>>> {
    tokio::task::spawn_blocking(move || {
        let state = app.state::<AppState>();
        paths.iter().map(|p| state.track(Path::new(p))).collect()
    })
    .await
    .map_err(err)
}

fn rand_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos() as usize % len).unwrap_or(0)
}

#[tauri::command]
pub fn player_toggle(state: State<'_, AppState>) {
    state.player.toggle()
}

#[tauri::command]
pub fn player_next(state: State<'_, AppState>) {
    state.player.next()
}

#[tauri::command]
pub fn player_previous(state: State<'_, AppState>) {
    state.player.previous()
}

#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, ms: u64) {
    state.player.seek(ms)
}

#[tauri::command]
pub fn player_jump(state: State<'_, AppState>, index: usize) {
    state.player.jump(index)
}

#[tauri::command]
pub fn player_shuffle(app: AppHandle, on: bool) {
    // sharing: the hub decides what comes next
    if on && crate::share::blocks_shuffle(&app) {
        return;
    }
    app.state::<AppState>().player.set_shuffle(on)
}

#[tauri::command]
pub fn player_repeat(state: State<'_, AppState>, mode: RepeatMode) {
    state.player.set_repeat(mode)
}

#[tauri::command(async)]
pub fn player_volume(app: AppHandle, volume: f32) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if !crate::share::on_volume(&app, volume) {
        state.player.set_volume(volume);
    }
    state.save_settings(|s| s.volume = volume.clamp(0.0, 1.0)).map(drop)
}

/// One visualizer frame: spectrum, waveform and peaks of what is playing (in
/// someone's room: the party played here, or the hub's levels).
#[tauri::command(async)]
pub fn visual(app: AppHandle, bands: usize) -> homefi_player::Visual {
    if let Some(v) = crate::share::guest_visual(&app, bands.clamp(1, 256)) {
        return v;
    }
    app.state::<AppState>().player.visual(bands)
}

/// Asking the audio system for its devices can take a while (CoreAudio, ALSA).
#[tauri::command]
pub async fn output_devices() -> CmdResult<Vec<OutputDevice>> {
    tokio::task::spawn_blocking(list_devices).await.map_err(err)
}

#[tauri::command(async)]
pub fn set_output(state: State<'_, AppState>, name: Option<String>) -> CmdResult<()> {
    state.player.set_device(name.clone());
    state.save_settings(|s| s.output_device = name).map(drop)
}

// ----------------------------------------------------------------- settings

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> DesktopSettings {
    state.settings()
}

#[tauri::command]
pub async fn set_settings(app: AppHandle, settings: DesktopSettings) -> CmdResult<DesktopSettings> {
    let state = app.state::<AppState>();
    let before = state.settings();
    let saved = state.save_settings(|s| *s = settings)?;
    crate::share::on_settings(&app, &before, &saved);
    if before.eq != saved.eq {
        crate::eq::apply(&app, &saved.eq);
    }
    if before.taste != saved.taste {
        state.player.set_taste(saved.taste.clone());
    }
    if before.loudness != saved.loudness {
        state.player.set_loudness(saved.loudness);
    }
    if before.crossfade != saved.crossfade {
        state.player.set_crossfade(saved.crossfade);
    }
    if before.output != saved.output {
        state.player.set_output_options(saved.output);
    }
    // every window follows (the mini player takes the skin and theme)
    let _ = app.emit("settings", &saved);
    // new folders: the library is scanned in the background ("library" event when done)
    if before.library_roots() != saved.library_roots() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = rescan(&app).await {
                tracing::warn!("library scan failed: {e}");
            }
        });
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use homefi_core::models::Page;
    use std::sync::atomic::AtomicUsize;

    fn page(offset: i64) -> homefi_core::Result<Page<i64>> {
        Ok(Page { limit: 10, offset, total_number_of_items: 45, items: (offset..(offset + 10).min(45)).collect() })
    }

    fn hits(v: Vec<i64>) -> Vec<Hit> {
        v.into_iter()
            .map(|i| Hit {
                kind: homefi_core::resource::ResourceKind::Track,
                id: i.to_string(),
                title: String::new(),
                subtitle: String::new(),
                artist: String::new(),
                album: String::new(),
                cover_url: None,
                explicit: false,
                quality: None,
                isrc: None,
                duration_ms: None,
                versions: Vec::new(),
            })
            .collect()
    }

    fn channel() -> (tauri::ipc::Channel<HitPage>, Arc<Mutex<Vec<serde_json::Value>>>) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let sink = got.clone();
        let channel = tauri::ipc::Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                sink.lock().unwrap().push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        });
        (channel, got)
    }

    #[tokio::test]
    async fn streams_every_page_in_order() {
        let (channel, got) = channel();
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let sent = stream_pages(0, |o| async move { page(o) }, hits, &channel, rx).await.unwrap();
        assert_eq!(sent, 5);
        let got = got.lock().unwrap();
        let firsts: Vec<_> = got.iter().map(|p| p["items"][0]["id"].as_str().unwrap().to_owned()).collect();
        assert_eq!(firsts, ["0", "10", "20", "30", "40"]);
        assert_eq!(got[3]["next_offset"], 40);
        assert!(got[4]["next_offset"].is_null());
    }

    #[tokio::test]
    async fn a_cancelled_stream_stops() {
        let (channel, got) = channel();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let fetched = Arc::new(AtomicUsize::new(0));
        let count = fetched.clone();
        let fetch = move |o| {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if o > 0 {
                    // later pages are slow: the user leaves meanwhile
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                page(o)
            }
        };
        let started = std::time::Instant::now();
        let stream = stream_pages(0, fetch, hits, &channel, rx);
        let cancel = async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            tx.send(true).unwrap();
        };
        let (sent, ()) = tokio::join!(stream, cancel);
        assert_eq!(sent.unwrap(), 1);
        assert_eq!(got.lock().unwrap().len(), 1);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    /// A sync command freezes the window while it runs; the ones that read the
    /// disk or wait on locks must not be plain `fn`s on the main thread.
    #[test]
    fn blocking_commands_are_not_on_the_main_thread() {
        let sources = [
            include_str!("commands.rs"),
            include_str!("eq.rs"),
            include_str!("sound.rs"),
            include_str!("ai.rs"),
            include_str!("share/mod.rs"),
        ];
        // cheap: channel sends to the player, pure math, a parse
        let main_thread_ok = [
            "play_tracks",
            "player_move",
            "player_toggle",
            "player_next",
            "player_previous",
            "player_seek",
            "player_jump",
            "player_shuffle",
            "player_repeat",
            "tidal_cancel",
            "eq_response",
            "eq_log_freqs",
            "eq_auto_preamp",
            "eq_with_taste",
            "ai_providers",
            "share_parse_link",
        ];
        for source in sources {
            let mut lines = source.lines();
            while let Some(line) = lines.next() {
                if line.trim() != "#[tauri::command]" {
                    continue;
                }
                let next = lines.next().unwrap_or_default();
                if let Some(rest) = next.strip_prefix("pub fn ") {
                    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    assert!(main_thread_ok.contains(&name.as_str()), "`{name}` runs on the main thread");
                }
            }
        }
    }
}
