//! This computer as the hub: the core's [`Hub`] server on the LAN, answering
//! guests' searches from the library and the Tidal account, and mirrored to the
//! desktop player (Android's `ShareManager` hub side, ported).
//!
//! The hub owns the shared queue; the player holds what can be played of it:
//! the songs after the current one are the hub's upcoming list, as far as their
//! files are on this computer (library files, Tidal tracks fetched into the
//! stream cache a few ahead). When the host changes the player's list by hand
//! (a new album, a jump, previous), the player's list is pushed to the hub
//! instead, like Android's reconcile.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use futures::FutureExt;
use homefi_core::api::pages_up_to;
use homefi_core::catalog;
use homefi_core::share::{
    BrowseKind, Collection, CoverImage, HostCommand, Hub, HubEvent, HubHost, JoinInfo, Page, QueueItem, SearchResults,
    ShareTrack, Source, HUB_ID,
};
use homefi_player::{Snapshot, Status};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use super::tracks::{self, LOCAL_COLLECTION};
use super::{emit_hub, notice, Share};
use crate::commands::AppState;

/// Songs a guest sees of a Tidal album or playlist at most.
const MAX_SONGS: usize = 500;

/// Positions while playing, so guests that play along correct their drift.
const POSITION_EVERY: Duration = Duration::from_secs(3);
/// ~15 frames a second: smooth enough for the meters, a few hundred bytes a second.
const LEVELS_EVERY: Duration = Duration::from_millis(66);
const LEVEL_BANDS: usize = 32;
/// A scheduled start is this far ahead, so the start time reaches every guest in time.
const START_MARGIN_MS: u64 = 350;
const READY_POLL: Duration = Duration::from_millis(100);
/// Tidal songs fetched ahead of the one playing.
const FETCH_AHEAD: usize = 3;
/// At most this many shared songs sit in the player's queue at once.
const MAX_PUSHED: usize = 100;
/// The player shows the list we gave it within this long, or the host changed it meanwhile.
const APPLY_TIMEOUT: Duration = Duration::from_millis(1500);
/// A position this far from where it should be is a seek.
const SEEK_JUMP_MS: i64 = 1500;

/// What reaches the mirror task: the hub's requests and finished fetches.
pub(super) enum Signal {
    Command(HostCommand),
    Event(HubEvent),
    Fetched(Source, Result<PathBuf, String>),
}

/// The hub's answers to guests (search, browse, covers) and its requests to the
/// player (posted to the mirror task).
struct DesktopHost {
    app: AppHandle,
    tx: mpsc::UnboundedSender<Signal>,
}

fn logged_in(state: &AppState) -> bool {
    state.session.auth_data().ok().is_some_and(|d| d.is_logged_in())
}

async fn search(app: &AppHandle, q: String) -> homefi_core::Result<SearchResults> {
    let state = app.state::<AppState>();
    let (local, albums, artists) = tracks::local_search(&state.library.lock().unwrap(), &q);
    let tidal = if logged_in(&state) && !q.trim().is_empty() {
        match state.api() {
            Ok(api) => api.search(q.trim()).await.ok().map(|s| catalog::SearchResults::from(&s)),
            Err(_) => None,
        }
    } else {
        None
    };
    if tidal.is_none() && local.is_empty() && logged_in(&state) && !q.trim().is_empty() {
        return Err(homefi_core::Error::Share("The hub could not search Tidal".into()));
    }
    let tidal = tidal.unwrap_or_default();
    let collections = |hits: &[catalog::Hit]| hits.iter().filter_map(tracks::collection_of_hit).collect::<Vec<_>>();
    Ok(SearchResults {
        q,
        tracks: local
            .into_iter()
            .chain(
                tidal
                    .tracks
                    .iter()
                    .filter(|h| h.kind == homefi_core::resource::ResourceKind::Track)
                    .map(tracks::track_of_hit),
            )
            .collect(),
        albums: albums.into_iter().chain(collections(&tidal.albums)).collect(),
        artists: artists.into_iter().chain(collections(&tidal.artists)).collect(),
        playlists: collections(&tidal.playlists),
    })
}

fn songs(items: Vec<homefi_core::models::ListedItem>) -> Vec<ShareTrack> {
    items
        .into_iter()
        .filter_map(|l| match l.item {
            homefi_core::models::MediaItem::Track(t) => Some(tracks::track_of_hit(&catalog::track_hit(&t))),
            homefi_core::models::MediaItem::Video(_) => None,
        })
        .collect()
}

async fn browse(app: &AppHandle, kind: BrowseKind, id: String) -> homefi_core::Result<Page> {
    let state = app.state::<AppState>();
    let none = || homefi_core::Error::Share("The hub could not open it".into());
    if id.starts_with(LOCAL_COLLECTION) {
        return tracks::local_page(&state.library.lock().unwrap(), kind, &id).ok_or_else(none);
    }
    let api = state.api()?;
    let header = |hit: &catalog::Hit| Collection {
        kind,
        id: id.clone(),
        title: hit.title.clone(),
        subtitle: hit.subtitle.clone(),
        cover_url: hit.cover_url.clone(),
    };
    Ok(match kind {
        BrowseKind::Album => {
            let album = api.album(&id).await?;
            Page {
                header: header(&catalog::album_hit(&album)),
                tracks: songs(pages_up_to(MAX_SONGS, |o| api.album_items(&id, o)).await?),
            }
        }
        BrowseKind::Playlist => {
            let playlist = api.playlist(&id).await?;
            Page {
                header: header(&catalog::playlist_hit(&playlist)),
                tracks: songs(pages_up_to(MAX_SONGS, |o| api.playlist_items(&id, o)).await?),
            }
        }
        BrowseKind::Artist => {
            let artist = api.artist(&id).await?;
            let top = api.artist_top_tracks(&id, 0).await?;
            Page {
                header: header(&catalog::artist_hit(&artist)),
                tracks: top.items.iter().map(|t| tracks::track_of_hit(&catalog::track_hit(t))).collect(),
            }
        }
        BrowseKind::Unknown => return Err(none()),
    })
}

/// Embedded art of a song the hub may show: library files, the stream cache, the player's queue.
fn cover(app: &AppHandle, id: &str) -> Option<CoverImage> {
    let path = PathBuf::from(tracks::decode_id(id)?);
    let state = app.state::<AppState>();
    let key = path.to_string_lossy();
    let known = state.library.lock().unwrap().iter().any(|t| t.path == key)
        || path.parent() == Some(crate::stream::cache_dir(&state).as_path())
        || state.player.snapshot().queue.contains(&path);
    if !known {
        return None;
    }
    homefi_core::library::read_cover(&path).map(CoverImage::new)
}

impl HubHost for DesktopHost {
    fn search(&self, q: String) -> BoxFuture<'_, homefi_core::Result<SearchResults>> {
        async move { search(&self.app, q).await }.boxed()
    }

    fn browse(&self, kind: BrowseKind, id: String) -> BoxFuture<'_, homefi_core::Result<Page>> {
        async move { browse(&self.app, kind, id).await }.boxed()
    }

    fn cover(&self, id: String) -> BoxFuture<'_, Option<CoverImage>> {
        let app = self.app.clone();
        async move { tokio::task::spawn_blocking(move || cover(&app, &id)).await.ok().flatten() }.boxed()
    }

    fn command(&self, command: HostCommand) {
        let _ = self.tx.send(Signal::Command(command));
    }

    fn event(&self, event: HubEvent) {
        let _ = self.tx.send(Signal::Event(event));
    }
}

/// The player as the hub knows it.
#[derive(Default)]
struct Mirror {
    /// The player's last snapshot and when it came.
    snap: Option<(Instant, Snapshot)>,
    /// The file being heard.
    current: Option<PathBuf>,
    /// The hub's upcoming list (from its last `Queue` command or state).
    upcoming: Vec<QueueItem>,
    /// What we put after the current song in the player, with the queue ids.
    pushed: Vec<(u64, PathBuf)>,
    /// Since when we wait for the player to show `pushed`.
    awaiting: Option<Instant>,
    /// Files of shared songs, once on this computer.
    files: HashMap<Source, PathBuf>,
    fetching: Option<Source>,
    failed: HashSet<Source>,
    /// Party mode, a new song: held at 0 until the guests have it.
    hold: Option<JoinHandle<()>>,
    /// The player's volume while it only plays on the guests' speakers.
    muted: Option<f32>,
    /// "Play" in the app while sharing: start this one as soon as it is in the player.
    play_now: Option<u64>,
    /// The next song's file was given to the hub for this queue id.
    next_pushed: Option<u64>,
}

pub struct HubSession {
    pub hub: Hub,
    pub info: JoinInfo,
    party: AtomicBool,
    play_here: AtomicBool,
    holding: Arc<AtomicBool>,
    signals: mpsc::UnboundedSender<Signal>,
    mirror: Mutex<Mirror>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

fn current_path(s: &Snapshot) -> Option<PathBuf> {
    s.current.and_then(|i| s.queue.get(i)).cloned()
}

/// The player's list after the current song.
fn engine_upcoming(s: &Snapshot) -> &[PathBuf] {
    match s.current {
        Some(i) if i < s.queue.len() => &s.queue[i + 1..],
        Some(_) => &[],
        None => &s.queue,
    }
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0) as u8
}

impl HubSession {
    /// Starts the hub on the configured port and wires it to the player.
    pub async fn start(app: &AppHandle, party: bool, code: Option<String>) -> Result<Arc<Self>, String> {
        let state = app.state::<AppState>();
        let s = state.settings().share;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut config = homefi_core::share::HubConfig::new(super::session_name(&s), super::display_name(&s));
        config.host_color = s.color.clone();
        config.code = match code {
            Some(code) => code,
            None if s.fixed_code => s.code.chars().filter(char::is_ascii_digit).collect(),
            None => String::new(),
        };
        config.bind = std::net::SocketAddr::from(([0, 0, 0, 0], s.port));
        config.permissions = s.permissions();
        config.stream_audio = party;
        let host = Arc::new(DesktopHost { app: app.clone(), tx: tx.clone() });
        let hub = Hub::start(config, host).await.map_err(|e| match e {
            homefi_core::Error::Io(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                format!("Port {} is in use (another HomeFi?). Pick another one in Settings.", s.port)
            }
            e => crate::commands::err(e),
        })?;
        let address =
            homefi_core::share::local_ip_guess().map(|ip| ip.to_string()).unwrap_or_else(|| "127.0.0.1".into());
        let info = hub.join_info(&address);
        let session = Arc::new(Self {
            hub,
            info,
            party: AtomicBool::new(party),
            play_here: AtomicBool::new(true),
            holding: Default::default(),
            signals: tx,
            mirror: Mutex::new(Mirror::default()),
            tasks: Mutex::new(Vec::new()),
        });
        let share = app.state::<Share>();
        if s.announce_lan {
            share.discovery.announce(&session.info.session_name, &super::computer_name(), session.info.port);
        }
        // the hub decides the order of what comes next
        state.player.set_shuffle(false);
        let snaps = share.snapshots.subscribe();
        session.attach(app);
        let tasks = vec![
            tokio::spawn(run(app.clone(), session.clone(), rx, snaps)),
            tokio::spawn(levels(app.clone(), session.clone())),
        ];
        *session.tasks.lock().unwrap() = tasks;
        Ok(session)
    }

    pub fn party(&self) -> bool {
        self.party.load(Ordering::SeqCst)
    }

    pub fn play_here(&self) -> bool {
        self.play_here.load(Ordering::SeqCst)
    }

    /// Says goodbye to the guests and gives the player back as it is.
    pub async fn stop(&self, app: &AppHandle) {
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
        {
            let mut m = self.mirror.lock().unwrap();
            self.cancel_hold(&mut m);
            if let Some(v) = m.muted.take() {
                app.state::<AppState>().player.set_volume(v);
            }
        }
        app.state::<Share>().discovery.unannounce();
        self.hub.stop().await;
    }

    /// The current song and the player's list go to the hub (the host's songs).
    fn attach(&self, app: &AppHandle) {
        let state = app.state::<AppState>();
        let snap = state.player.snapshot();
        self.hub.set_volume(Some(snap.volume));
        let mut m = self.mirror.lock().unwrap();
        m.snap = Some((Instant::now(), snap.clone()));
        m.current = current_path(&snap);
        if let Some(path) = &m.current {
            self.hub.set_now_playing(
                None,
                Some(track_of(app, path)),
                self.reported(app, snap.position_ms),
                snap.status == Status::Playing,
            );
            if self.party() {
                self.hub.set_audio(Some(path.clone()));
            }
        }
        let up: Vec<PathBuf> = engine_upcoming(&snap).iter().take(MAX_PUSHED).cloned().collect();
        self.reconcile(app, &mut m, &up);
    }

    /// What guests hear: the player's position minus this computer's speaker delay.
    fn reported(&self, app: &AppHandle, position_ms: u64) -> u64 {
        let delay = app.state::<AppState>().settings().share.speaker_delay_ms;
        (position_ms as i64 - i64::from(delay)).max(0) as u64
    }

    fn precise(m: &Mirror) -> Option<(u64, bool)> {
        let (at, s) = m.snap.as_ref()?;
        let playing = s.status == Status::Playing;
        Some((s.position_ms + if playing { at.elapsed().as_millis() as u64 } else { 0 }, playing))
    }

    fn on_snapshot(&self, app: &AppHandle, at: Instant, snap: Snapshot) {
        let mut m = self.mirror.lock().unwrap();
        let prev = m.snap.replace((at, snap.clone()));
        let path = current_path(&snap);
        let playing = snap.status == Status::Playing;
        let engine_changed = prev.as_ref().is_none_or(|(_, p)| p.queue != snap.queue || p.current != snap.current);

        let transition = path != m.current;
        if transition {
            self.cancel_hold(&mut m);
            m.current = path.clone();
            // the song came from the shared queue: its id, so its owner and turn are kept
            let started = path.as_ref().and_then(|p| m.pushed.iter().position(|(_, q)| q == p));
            let id = match (started, &path) {
                (Some(k), _) => Some(m.pushed[k].0),
                (None, Some(p)) => {
                    let source = track_of(app, p).source;
                    m.upcoming.iter().find(|i| i.track.source == source).map(|i| i.queue_id)
                }
                (None, None) => None,
            };
            if let Some(k) = started {
                m.pushed.drain(..=k);
            }
            match &path {
                None => {
                    self.hub.set_now_playing(None, None, 0, false);
                    self.hub.set_audio(None);
                }
                Some(p) => {
                    self.hub.set_now_playing(id, Some(track_of(app, p)), self.reported(app, snap.position_ms), playing);
                    if self.party() {
                        self.hub.set_audio(Some(p.clone()));
                    }
                }
            }
            m.upcoming = self.hub.state().queue;
            if self.party() && playing {
                self.hold_for_guests(app, &mut m);
            }
        } else if let Some((prev_at, p)) = &prev {
            let was_playing = p.status == Status::Playing;
            if playing != was_playing && self.holding.load(Ordering::SeqCst) && playing {
                // pressing play during the wait starts right away
                self.cancel_hold(&mut m);
            }
            let expected = p.position_ms as i64 + if was_playing { prev_at.elapsed().as_millis() as i64 } else { 0 };
            let seeked = (snap.position_ms as i64 - expected).abs() > SEEK_JUMP_MS;
            if (playing != was_playing || seeked) && !self.holding.load(Ordering::SeqCst) {
                self.hub.set_position(self.reported(app, snap.position_ms), playing);
            }
            if (p.volume - snap.volume).abs() > f32::EPSILON && m.muted.is_none() {
                self.hub.set_volume(Some(snap.volume));
            }
        }

        // the player's list after the current song, against what the hub expects
        let up: Vec<PathBuf> = engine_upcoming(&snap).iter().take(MAX_PUSHED).cloned().collect();
        let pushed: Vec<PathBuf> = m.pushed.iter().map(|(_, p)| p.clone()).collect();
        let mut timed_out = false;
        if let Some(since) = m.awaiting {
            if up == pushed {
                m.awaiting = None;
            } else if since.elapsed() > APPLY_TIMEOUT {
                // the player kept something else (a song already buffered): take what it has
                m.awaiting = None;
                timed_out = true;
            } else {
                return; // our own change is on its way to the player
            }
        }
        if up == pushed {
            // a song started: the turns may have changed what comes next
            if transition {
                self.apply(app, &mut m);
            }
        } else if engine_changed || timed_out {
            // the host changed the player's list (a new album, a jump, previous): it wins
            self.reconcile(app, &mut m, &up);
        }
    }

    /// The player's upcoming list becomes the hub's: known songs keep their ids
    /// (owner, turn), the rest are the host's. Shared songs not in the player yet
    /// (still being fetched) stay after them.
    fn reconcile(&self, app: &AppHandle, m: &mut Mirror, up: &[PathBuf]) {
        let mut used = HashSet::new();
        let mut entries: Vec<(Option<u64>, ShareTrack)> = Vec::with_capacity(up.len());
        for path in up {
            let id = m.pushed.iter().find(|(id, p)| p == path && !used.contains(id)).map(|(id, _)| *id);
            if let Some(id) = id {
                used.insert(id);
            }
            let track = track_of(app, path);
            m.files.insert(track.source.clone(), path.clone());
            entries.push((id, track));
        }
        let pushed_ids: HashSet<u64> = m.pushed.iter().map(|(id, _)| *id).collect();
        entries.extend(
            m.upcoming
                .iter()
                .filter(|i| !pushed_ids.contains(&i.queue_id))
                .map(|i| (Some(i.queue_id), i.track.clone())),
        );
        self.hub.set_queue(entries);
        m.upcoming = self.hub.state().queue;
        m.pushed = m.upcoming.iter().zip(up).map(|(i, p)| (i.queue_id, p.clone())).collect();
        self.push_next_audio(m);
    }

    /// Files of the hub's upcoming songs, in order, as far as they are on this
    /// computer; starts fetching the first Tidal song that isn't.
    fn resolve(&self, app: &AppHandle, m: &mut Mirror) -> Vec<(u64, PathBuf)> {
        let state = app.state::<AppState>();
        let stream_dir = crate::stream::cache_dir(&state);
        let mut out = Vec::new();
        let mut fetch = None;
        for item in m.upcoming.iter().take(MAX_PUSHED) {
            let source = &item.track.source;
            let known = m.files.get(source).filter(|p| p.is_file()).cloned();
            if let Some(path) = known.or_else(|| tracks::local_file(source, &stream_dir)) {
                m.files.insert(source.clone(), path.clone());
                out.push((item.queue_id, path));
                continue;
            }
            match source {
                // order matters: wait for it (a few ahead only)
                Source::Tidal(_) if !m.failed.contains(source) && logged_in(&state) => {
                    if out.len() < FETCH_AHEAD {
                        fetch = Some((source.clone(), item.track.title.clone()));
                    }
                    break;
                }
                // can't be played here (another device's file, a failed fetch): skipped
                _ => continue,
            }
        }
        if let (Some((source, _title)), None) = (fetch, &m.fetching) {
            m.fetching = Some(source.clone());
            let app = app.clone();
            let tx = self.signals.clone();
            tokio::spawn(async move {
                let state = app.state::<AppState>();
                let Source::Tidal(id) = &source else { return };
                let dir = crate::stream::cache_dir(&state);
                let result = async {
                    state.session.refresh(600, false).await?;
                    let api = state.api()?;
                    homefi_core::stream_cache::fetch(api, state.session.config(), &dir, id, state.settings().quality)
                        .await
                }
                .await
                .map_err(crate::commands::err);
                let _ = tx.send(Signal::Fetched(source, result));
            });
        }
        out
    }

    /// The hub's list into the player (what of it is here), if it differs.
    fn apply(&self, app: &AppHandle, m: &mut Mirror) {
        let expected = self.resolve(app, m);
        let up: Vec<PathBuf> = m
            .snap
            .as_ref()
            .map(|(_, s)| engine_upcoming(s).iter().take(MAX_PUSHED).cloned().collect())
            .unwrap_or_default();
        let paths: Vec<PathBuf> = expected.iter().map(|(_, p)| p.clone()).collect();
        let player = &app.state::<AppState>().player;
        if up != paths {
            player.set_upcoming(paths);
            m.awaiting = Some(Instant::now());
        }
        m.pushed = expected;
        if let Some(id) = m.play_now {
            if m.pushed.first().map(|(i, _)| *i) == Some(id) {
                m.play_now = None;
                match m.snap.as_ref().map(|(_, s)| (s.status, s.current)) {
                    Some((Status::Playing | Status::Paused, _)) => player.next(),
                    Some((Status::Stopped, None)) | None => player.jump(0),
                    // the queue had finished: the new list starts by itself
                    _ => {}
                }
            }
        }
        self.push_next_audio(m);
    }

    /// Party mode: the next song's file, so guests download it ahead.
    fn push_next_audio(&self, m: &mut Mirror) {
        if !self.party() {
            m.next_pushed = None;
            return;
        }
        let Some(next) = m.upcoming.first() else { return };
        if m.next_pushed == Some(next.queue_id) {
            return;
        }
        if let Some((_, path)) = m.pushed.first().filter(|(id, _)| *id == next.queue_id) {
            self.hub.set_next_audio(next.queue_id, Some(path.clone()));
            m.next_pushed = Some(next.queue_id);
        }
    }

    /// Party mode, a new song: hold it at 0 until every guest has it loaded (or
    /// the wait in Settings runs out), then everyone starts at the same hub-clock
    /// instant. With the songs downloaded ahead this is usually well under a second.
    fn hold_for_guests(&self, app: &AppHandle, m: &mut Mirror) {
        let wait_ms = u64::from(app.state::<AppState>().settings().share.party_start_wait_sec.min(10)) * 1000;
        let Some(queue_id) = self.hub.state().now_playing.map(|n| n.queue_id) else { return };
        if wait_ms == 0 || self.hub.ready_guests(queue_id).1 == 0 {
            return;
        }
        let player = app.state::<AppState>().player.clone();
        self.holding.store(true, Ordering::SeqCst);
        player.pause();
        player.seek(0);
        let deadline = self.hub.schedule_start(wait_ms);
        let holding = self.holding.clone();
        let share = app.state::<Share>();
        let Some(session) = share.hub() else { return };
        m.hold = Some(tokio::spawn(async move {
            let hub = &session.hub;
            let mut start = deadline;
            while hub.now_ms() + START_MARGIN_MS < deadline {
                let (ready, connected) = hub.ready_guests(queue_id);
                if ready >= connected {
                    // everyone has it: start shortly, so the new start time reaches them first
                    start = hub.schedule_start(START_MARGIN_MS);
                    break;
                }
                tokio::time::sleep(READY_POLL).await;
            }
            tokio::time::sleep(Duration::from_millis(start.saturating_sub(hub.now_ms()))).await;
            holding.store(false, Ordering::SeqCst);
            player.play();
        }));
    }

    fn cancel_hold(&self, m: &mut Mirror) {
        if let Some(h) = m.hold.take() {
            h.abort();
        }
        self.holding.store(false, Ordering::SeqCst);
    }

    fn on_command(&self, app: &AppHandle, command: HostCommand) {
        let player = &app.state::<AppState>().player;
        let mut m = self.mirror.lock().unwrap();
        match command {
            HostCommand::Skip => match m.upcoming.first() {
                // the next shared song is still on its way from Tidal: it starts once it is here
                Some(next)
                    if m.pushed.first().map(|(id, _)| *id) != Some(next.queue_id)
                        && matches!(next.track.source, Source::Tidal(_))
                        && !m.failed.contains(&next.track.source) =>
                {
                    m.play_now = Some(next.queue_id);
                }
                _ => player.next(),
            },
            HostCommand::Toggle => player.toggle(),
            HostCommand::Volume(v) => match m.muted {
                // only the guests' speakers play: the volume they see changes, this one stays muted
                Some(_) => {
                    m.muted = Some(v);
                    self.hub.set_volume(Some(v));
                }
                None => player.set_volume(v),
            },
            HostCommand::Queue(items) => {
                m.upcoming = items;
                self.apply(app, &mut m);
            }
        }
    }

    fn on_fetched(&self, app: &AppHandle, source: Source, result: Result<PathBuf, String>) {
        let mut m = self.mirror.lock().unwrap();
        m.fetching = None;
        match result {
            Ok(path) => {
                m.files.insert(source, path);
                let state = app.state::<AppState>();
                let dir = crate::stream::cache_dir(&state);
                let mut keep = state.player.snapshot().queue;
                keep.extend(m.files.values().cloned());
                tokio::task::spawn_blocking(move || {
                    homefi_core::stream_cache::trim(&dir, crate::stream::CACHE_LIMIT, &keep)
                });
            }
            Err(e) => {
                let title = m.upcoming.iter().find(|i| i.track.source == source).map(|i| i.track.title.clone());
                tracing::warn!("share: fetch {source}: {e}");
                m.failed.insert(source);
                if let Some(title) = title {
                    notice(app, &format!("Couldn't get \u{201c}{title}\u{201d} from Tidal: {e}"), None, "error");
                }
            }
        }
        self.apply(app, &mut m);
    }

    fn on_event(&self, app: &AppHandle, event: HubEvent) {
        let s = app.state::<AppState>().settings().share;
        let text = match &event {
            HubEvent::GuestJoined(g) if s.notice_joined => Some((format!("{} joined", g.name), g.color.clone())),
            HubEvent::GuestLeft(g) if s.notice_left => Some((format!("{} left", g.name), g.color.clone())),
            HubEvent::Added(i) if s.notice_added && i.added_by.guest_id != HUB_ID => {
                Some((format!("{} added \u{201c}{}\u{201d}", i.added_by.name, i.track.title), i.added_by.color.clone()))
            }
            HubEvent::ApprovalRequested(i) if s.notice_approval => Some((
                format!("{} wants to add \u{201c}{}\u{201d}", i.added_by.name, i.track.title),
                i.added_by.color.clone(),
            )),
            _ => None,
        };
        if let Some((text, color)) = text {
            notice(app, &text, Some(&color), "hub");
        }
    }

    fn on_tick(&self, app: &AppHandle) {
        let m = self.mirror.lock().unwrap();
        if let Some((position, true)) = Self::precise(&m) {
            if !self.holding.load(Ordering::SeqCst) {
                self.hub.set_position(self.reported(app, position), true);
            }
        }
    }

    // ------------------------------------------------------------- host actions

    /// Party mode on / off while sharing.
    pub fn set_party(&self, app: &AppHandle, on: bool) {
        self.party.store(on, Ordering::SeqCst);
        self.hub.set_streaming(on);
        if !on {
            // nobody else plays: back on this computer
            self.set_play_here(app, true);
        }
        let mut m = self.mirror.lock().unwrap();
        if on {
            self.hub.set_audio(m.current.clone());
        }
        m.next_pushed = None;
        self.push_next_audio(&mut m);
    }

    /// Party mode: play here too, or only on the guests' speakers. The player keeps
    /// running muted (the guests follow its clock); guests still see the real volume.
    pub fn set_play_here(&self, app: &AppHandle, on: bool) {
        let player = &app.state::<AppState>().player;
        let mut m = self.mirror.lock().unwrap();
        if on {
            if let Some(v) = m.muted.take() {
                player.set_volume(v);
            }
        } else if m.muted.is_none() {
            let v = player.snapshot().volume;
            m.muted = Some(v);
            player.set_volume(0.0);
            self.hub.set_volume(Some(v));
        }
        self.play_here.store(on, Ordering::SeqCst);
    }

    /// The host's volume knob while muted: what guests see changes, the output stays silent.
    /// `false` when the player should take it as usual.
    pub fn host_volume(&self, volume: f32) -> bool {
        let mut m = self.mirror.lock().unwrap();
        if m.muted.is_none() {
            return false;
        }
        m.muted = Some(volume);
        self.hub.set_volume(Some(volume));
        true
    }

    /// The player is empty: the app's own "play" fills it (the list then joins the hub).
    pub fn player_is_empty(&self, app: &AppHandle) -> bool {
        app.state::<AppState>().player.snapshot().queue.is_empty()
    }

    /// "Add to queue" / "Play next" while sharing: the host's songs take turns like
    /// anyone's; `next` puts them first, skipping the turns.
    pub fn host_add(&self, app: &AppHandle, songs: Vec<(ShareTrack, Option<PathBuf>)>, next: bool) {
        let mut m = self.mirror.lock().unwrap();
        for (track, path) in songs {
            if let Some(p) = path {
                m.files.insert(track.source.clone(), p);
            }
            self.hub.add(track, next);
        }
        m.upcoming = self.hub.state().queue;
        self.apply(app, &mut m);
    }

    /// "Play" while sharing: the chosen song plays now, the rest of the list joins
    /// the shared queue.
    pub fn host_play(&self, app: &AppHandle, songs: Vec<(ShareTrack, Option<PathBuf>)>, index: usize) {
        let mut m = self.mirror.lock().unwrap();
        for (i, (track, path)) in songs.into_iter().enumerate().skip(index) {
            if let Some(p) = path {
                m.files.insert(track.source.clone(), p);
            }
            if i == index {
                let item = self.hub.add(track, true);
                self.hub.move_item(item.queue_id, 0);
                m.play_now = Some(item.queue_id);
            } else {
                self.hub.add(track, false);
            }
        }
        m.upcoming = self.hub.state().queue;
        self.apply(app, &mut m);
    }

    /// The host drags a song in the queue panel: the hub's order changes.
    /// `false` when it isn't one of the shared songs (moved in the player only).
    pub fn host_move(&self, app: &AppHandle, from: usize, to: usize) -> bool {
        let mut m = self.mirror.lock().unwrap();
        let Some(cur) = m.snap.as_ref().and_then(|(_, s)| s.current) else { return false };
        if from <= cur || to <= cur || from - cur > m.pushed.len() {
            return false;
        }
        let id = m.pushed[from - cur - 1].0;
        self.hub.move_item(id, to - cur - 1);
        m.upcoming = self.hub.state().queue;
        self.apply(app, &mut m);
        true
    }
}

/// What guests see of a file the player has.
fn track_of(app: &AppHandle, path: &Path) -> ShareTrack {
    let state = app.state::<AppState>();
    tracks::track_of_file(path, state.track(path).as_ref(), &crate::stream::cache_dir(&state))
}

/// The mirror: player snapshots, the hub's requests, finished fetches and the
/// position every few seconds, all on one task.
async fn run(
    app: AppHandle,
    session: Arc<HubSession>,
    mut rx: mpsc::UnboundedReceiver<Signal>,
    mut snaps: watch::Receiver<Option<(Instant, Snapshot)>>,
) {
    let mut tick = tokio::time::interval(POSITION_EVERY);
    tick.tick().await;
    loop {
        tokio::select! {
            changed = snaps.changed() => {
                if changed.is_err() {
                    break;
                }
                let snap = snaps.borrow_and_update().clone();
                if let Some((at, s)) = snap {
                    session.on_snapshot(&app, at, s);
                }
            }
            signal = rx.recv() => {
                match signal {
                    None => break,
                    Some(Signal::Command(c)) => session.on_command(&app, c),
                    Some(Signal::Event(e)) => session.on_event(&app, e),
                    Some(Signal::Fetched(source, result)) => session.on_fetched(&app, source, result),
                }
                emit_hub(&app);
            }
            _ = tick.tick() => {
                session.on_tick(&app);
                emit_hub(&app);
            }
        }
    }
}

/// The music's levels, so guests without the audio still see the meters move.
async fn levels(app: AppHandle, session: Arc<HubSession>) {
    loop {
        let state = app.state::<AppState>();
        let want = state.settings().share.send_levels
            && !session.hub.guests().is_empty()
            && state.player.snapshot().status == Status::Playing;
        if !want {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }
        let v = state.player.visual(LEVEL_BANDS);
        let bands: Vec<u8> = v.spectrum.iter().map(|b| byte(*b)).collect();
        let (left, right) = if session.play_here() {
            (byte(v.left), byte(v.right))
        } else {
            // muted here (the meters read after the volume): the spectrum still tells how loud it is
            let loud = v.spectrum.iter().copied().fold(0.0f32, f32::max) * 0.8;
            (byte(loud), byte(loud))
        };
        session.hub.send_levels(left, right, bands);
        tokio::time::sleep(LEVELS_EVERY).await;
    }
}
