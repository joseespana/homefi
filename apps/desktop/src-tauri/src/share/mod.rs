//! Shared control and party mode on the desktop (see `docs/shared-control.md`,
//! "Desktop"): this computer runs a hub on the local network, or joins someone
//! else's as a guest. The protocol, server and client are `homefi_core::share`;
//! this module wires them to the desktop player, library and UI.
//!
//! - [`hub`]: the hub, mirrored to the player (Android's `ShareManager` hub side).
//! - [`guest`] and [`party`]: remote control and playing along here, in sync.
//! - [`discovery`]: mDNS announce / browse (`_homefi._tcp`) and the `/hello` probe.
//! - [`tracks`]: what guests see of the library and Tidal.
//!
//! The UI talks to it through the `share_*` commands and these events:
//! `share-hub` ([`HubView`]), `share-guest` ([`GuestView`]), `share-results`,
//! `share-page`, `share-denied`, `share-notice` ([`NoticeView`]) and
//! `share-nearby` (hubs found on the network).

mod discovery;
mod guest;
mod hub;
mod party;
mod tracks;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use homefi_core::share::{GuestMessage, JoinInfo, Member, QueueItem, ShareTrack, Source, State};
use homefi_player::{Snapshot, Visual};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;

pub use discovery::Nearby;
use guest::GuestSession;
pub use guest::GuestView;
use hub::HubSession;

use crate::commands::AppState;
use crate::settings::{DesktopSettings, ShareSettings};

type CmdResult<T> = Result<T, String>;

/// Shared control state, managed by Tauri next to [`AppState`].
pub struct Share {
    hub: Mutex<Option<Arc<HubSession>>>,
    hub_starting: AtomicBool,
    hub_error: Mutex<Option<String>>,
    guest: Mutex<Option<Arc<GuestSession>>>,
    /// The guest side while connecting or after a disconnect (no session).
    guest_view: Mutex<GuestView>,
    guest_seq: AtomicU64,
    pub discovery: discovery::Discovery,
    /// The player's latest snapshot (and when it came), for the hub's mirror.
    snapshots: watch::Sender<Option<(Instant, Snapshot)>>,
}

impl Default for Share {
    fn default() -> Self {
        Self {
            hub: Mutex::default(),
            hub_starting: AtomicBool::default(),
            hub_error: Mutex::default(),
            guest: Mutex::default(),
            guest_view: Mutex::new(GuestView { status: "idle", ..Default::default() }),
            guest_seq: AtomicU64::default(),
            discovery: discovery::Discovery::default(),
            snapshots: watch::channel(None).0,
        }
    }
}

impl Share {
    pub fn hub(&self) -> Option<Arc<HubSession>> {
        self.hub.lock().unwrap().clone()
    }

    fn guest(&self) -> Option<Arc<GuestSession>> {
        self.guest.lock().unwrap().clone()
    }

    fn set_guest_view(&self, view: GuestView) {
        *self.guest_view.lock().unwrap() = view;
    }

    /// Forgets guest session `seq` if it is still the current one.
    fn end_guest(&self, seq: u64) -> bool {
        let mut slot = self.guest.lock().unwrap();
        if slot.as_ref().is_some_and(|g| g.seq == seq) {
            *slot = None;
            true
        } else {
            false
        }
    }
}

// ----------------------------------------------------------------- names

/// This computer's name (Finder / Explorer / hostname), for "<name>'s music".
pub fn computer_name() -> String {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let from_command = |cmd: &str, args: &[&str]| {
            std::process::Command::new(cmd)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let name = if cfg!(target_os = "macos") {
            from_command("scutil", &["--get", "ComputerName"])
        } else if cfg!(windows) {
            std::env::var("COMPUTERNAME").ok()
        } else {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .or_else(|| from_command("hostname", &[]))
        };
        name.unwrap_or_else(|| "HomeFi".into())
    })
    .clone()
}

/// How others see this computer; blank uses its name.
pub fn display_name(s: &ShareSettings) -> String {
    let name = s.name.trim();
    if name.is_empty() {
        computer_name()
    } else {
        name.to_owned()
    }
}

pub fn session_name(s: &ShareSettings) -> String {
    let name = s.session_name.trim();
    if name.is_empty() {
        format!("{}\u{2019}s music", display_name(s))
    } else {
        name.to_owned()
    }
}

// ----------------------------------------------------------------- views and events

#[derive(Debug, Clone, Serialize)]
pub struct InfoView {
    pub host: String,
    pub port: u16,
    pub code: String,
    pub session_name: String,
    /// QR payload: `homefi://join?host=..&port=..&code=..&name=..`
    pub join_uri: String,
}

impl From<&JoinInfo> for InfoView {
    fn from(j: &JoinInfo) -> Self {
        Self {
            host: j.host.clone(),
            port: j.port,
            code: j.code.clone(),
            session_name: j.session_name.clone(),
            join_uri: j.to_uri(),
        }
    }
}

/// This computer as the hub, for the share panel.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HubView {
    pub running: bool,
    pub starting: bool,
    /// Streaming the music to guests that play along.
    pub party: bool,
    /// Sound here too; off, only the guests' speakers play.
    pub play_here: bool,
    pub info: Option<InfoView>,
    pub guests: Vec<Member>,
    /// Songs waiting for the host's OK.
    pub pending: Vec<QueueItem>,
    pub state: Option<State>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoticeView {
    pub text: String,
    /// The person's colour, when it is about someone.
    pub color: Option<String>,
    /// `hub` (someone joined, added...), `guest` (from the hub) or `error`.
    pub kind: &'static str,
    /// How long it stays (Settings).
    pub seconds: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareStatus {
    pub hub: HubView,
    pub guest: GuestView,
    pub nearby: Vec<Nearby>,
    /// Defaults for the join form.
    pub name: String,
    pub session_name: String,
}

fn hub_view(app: &AppHandle) -> HubView {
    let share = app.state::<Share>();
    match share.hub() {
        Some(h) => HubView {
            running: true,
            starting: false,
            party: h.party(),
            play_here: h.play_here(),
            info: Some((&h.info).into()),
            guests: h.hub.guests(),
            pending: h.hub.pending(),
            state: Some(h.hub.state()),
            error: None,
        },
        None => HubView {
            starting: share.hub_starting.load(Ordering::SeqCst),
            play_here: true,
            error: share.hub_error.lock().unwrap().clone(),
            ..Default::default()
        },
    }
}

fn guest_view(app: &AppHandle) -> GuestView {
    let share = app.state::<Share>();
    match share.guest() {
        Some(g) => g.view(),
        None => share.guest_view.lock().unwrap().clone(),
    }
}

pub(crate) fn emit_hub(app: &AppHandle) {
    let _ = app.emit("share-hub", hub_view(app));
}

pub(crate) fn emit_guest(app: &AppHandle) {
    let _ = app.emit("share-guest", guest_view(app));
}

/// A discreet note in the UI (it goes away by itself).
pub(crate) fn notice(app: &AppHandle, text: &str, color: Option<&str>, kind: &'static str) {
    let seconds = app.state::<AppState>().settings().share.notice_seconds.clamp(1, 30);
    let _ =
        app.emit("share-notice", NoticeView { text: text.to_owned(), color: color.map(str::to_owned), kind, seconds });
}

/// Looks for sessions while it makes sense: "Look for music nearby" on, not
/// sharing and not in someone's session.
pub(crate) fn refresh_discovery(app: &AppHandle) {
    let share = app.state::<Share>();
    let want = app.state::<AppState>().settings().share.discover && share.hub().is_none() && share.guest().is_none();
    if want {
        let handle = app.clone();
        share.discovery.browse(Arc::new(move |list| {
            let _ = handle.emit("share-nearby", list);
        }));
    } else {
        share.discovery.stop_browse();
        let _ = app.emit("share-nearby", Vec::<Nearby>::new());
    }
}

// ----------------------------------------------------------------- hooks

/// Starts looking for sessions (app start).
pub fn init(app: &AppHandle) {
    refresh_discovery(app);
    // dev aid: `HOMEFI_SHARE_AUTOSTART=<code>` shares at launch with that join code
    // (smoke tests against a running app, a guest phone without touching the UI)
    #[cfg(debug_assertions)]
    if let Ok(code) = std::env::var("HOMEFI_SHARE_AUTOSTART") {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let party = std::env::var("HOMEFI_SHARE_PARTY").is_ok();
            if let Err(e) = start_hub(app.clone(), party, Some(code)).await {
                tracing::warn!("share autostart: {e}");
            }
        });
    }
}

/// Every player snapshot (from the audio thread): the hub mirrors it.
pub fn on_player(app: &AppHandle, snapshot: &Snapshot) {
    let share = app.state::<Share>();
    if share.hub.lock().unwrap().is_some() {
        share.snapshots.send_replace(Some((Instant::now(), snapshot.clone())));
    }
}

/// The visualizer while in someone's room (unless the user's own music plays).
pub fn guest_visual(app: &AppHandle, bands: usize) -> Option<Visual> {
    let guest = app.state::<Share>().guest()?;
    if app.state::<AppState>().player.snapshot().status == homefi_player::Status::Playing {
        return None;
    }
    Some(guest.visual(bands))
}

/// The volume knob: the party player follows it; while the hub only plays on the
/// guests' speakers it is the volume they see. `true` when the player must not take it.
pub fn on_volume(app: &AppHandle, volume: f32) -> bool {
    let share = app.state::<Share>();
    if let Some(g) = share.guest() {
        g.set_volume(volume, app.state::<AppState>().settings().output_device);
    }
    share.hub().is_some_and(|h| h.host_volume(volume))
}

/// Settings were saved: the rules apply right away, discovery follows its switch.
pub fn on_settings(app: &AppHandle, before: &DesktopSettings, after: &DesktopSettings) {
    let share = app.state::<Share>();
    let (b, a) = (&before.share, &after.share);
    if let Some(h) = share.hub() {
        if b.permissions() != a.permissions() {
            h.hub.set_permissions(a.permissions());
        }
        if b.announce_lan != a.announce_lan {
            if a.announce_lan {
                share.discovery.announce(&h.info.session_name, &computer_name(), h.info.port);
            } else {
                share.discovery.unannounce();
            }
        }
        emit_hub(app);
    }
    if b.discover != a.discover {
        refresh_discovery(app);
    }
}

/// A file the player can play, as the hub's queue sees it.
fn file_song(app: &AppHandle, path: PathBuf) -> (ShareTrack, Option<PathBuf>) {
    let state = app.state::<AppState>();
    let track = tracks::track_of_file(&path, state.track(&path).as_ref(), &crate::stream::cache_dir(&state));
    (track, Some(path))
}

/// A Tidal track (or its local copy) for the hub's queue.
fn stream_song(app: &AppHandle, item: &crate::stream::StreamItem) -> (ShareTrack, Option<PathBuf>) {
    if let Some(local) = item.local.as_ref().map(PathBuf::from).filter(|p| p.is_file()) {
        return file_song(app, local);
    }
    let track = ShareTrack {
        source: Source::Tidal(item.id.clone()),
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms,
        cover_url: item.cover_url.clone(),
        explicit: item.explicit,
    };
    (track, None)
}

/// "Play" with files while sharing: through the hub. `false` = play as usual.
pub fn host_play_files(app: &AppHandle, paths: &[String], index: usize) -> bool {
    let Some(h) = app.state::<Share>().hub() else { return false };
    if h.player_is_empty(app) || paths.is_empty() {
        return false;
    }
    let songs = paths.iter().map(|p| file_song(app, PathBuf::from(p))).collect();
    h.host_play(app, songs, index.min(paths.len() - 1));
    emit_hub(app);
    true
}

/// "Play" with Tidal tracks while sharing.
pub fn host_play_tidal(app: &AppHandle, items: &[crate::stream::StreamItem], index: usize) -> bool {
    let Some(h) = app.state::<Share>().hub() else { return false };
    if h.player_is_empty(app) || items.is_empty() {
        return false;
    }
    let songs = items.iter().map(|i| stream_song(app, i)).collect();
    h.host_play(app, songs, index.min(items.len() - 1));
    emit_hub(app);
    true
}

/// "Add to queue" / "Play next" with files while sharing.
pub fn host_add_files(app: &AppHandle, paths: &[String], next: bool) -> bool {
    let Some(h) = app.state::<Share>().hub() else { return false };
    if h.player_is_empty(app) {
        return false;
    }
    h.host_add(app, paths.iter().map(|p| file_song(app, PathBuf::from(p))).collect(), next);
    emit_hub(app);
    true
}

pub fn host_add_tidal(app: &AppHandle, items: &[crate::stream::StreamItem], next: bool) -> bool {
    let Some(h) = app.state::<Share>().hub() else { return false };
    if h.player_is_empty(app) {
        return false;
    }
    h.host_add(app, items.iter().map(|i| stream_song(app, i)).collect(), next);
    emit_hub(app);
    true
}

/// The Tidal play loop (`stream::play`) while sharing: what is left joins the hub.
pub fn host_takes_stream(app: &AppHandle, rest: &[crate::stream::StreamItem]) -> bool {
    host_add_tidal(app, rest, false)
}

pub fn host_move(app: &AppHandle, from: usize, to: usize) -> bool {
    app.state::<Share>().hub().is_some_and(|h| h.host_move(app, from, to))
}

/// While sharing, the hub decides the order: no shuffle.
pub fn blocks_shuffle(app: &AppHandle) -> bool {
    app.state::<Share>().hub().is_some()
}

/// The app closes: the hub says goodbye, the party cache goes.
pub fn shutdown(app: &AppHandle) {
    let share = app.state::<Share>();
    if let Some(g) = share.guest.lock().unwrap().take() {
        g.close();
    }
    if let Some(h) = share.hub.lock().unwrap().take() {
        tauri::async_runtime::block_on(h.stop(app));
    }
    share.discovery.shutdown();
}

// ----------------------------------------------------------------- commands

#[tauri::command(async)]
pub fn share_status(app: AppHandle) -> ShareStatus {
    let s = app.state::<AppState>().settings().share;
    ShareStatus {
        hub: hub_view(&app),
        guest: guest_view(&app),
        nearby: app.state::<Share>().discovery.found(),
        name: display_name(&s),
        session_name: session_name(&s),
    }
}

/// Starts sharing (party: guests can play the music too).
#[tauri::command]
pub async fn share_start(app: AppHandle, party: bool) -> CmdResult<HubView> {
    start_hub(app, party, None).await
}

/// `code`: this join code instead of the one in Settings (dev autostart).
async fn start_hub(app: AppHandle, party: bool, code: Option<String>) -> CmdResult<HubView> {
    let share = app.state::<Share>();
    if share.guest().is_some() {
        return Err("Leave the session you are in to share your own music".into());
    }
    if share.hub().is_some() || share.hub_starting.swap(true, Ordering::SeqCst) {
        return Ok(hub_view(&app));
    }
    *share.hub_error.lock().unwrap() = None;
    share.discovery.stop_browse();
    emit_hub(&app);
    let result = HubSession::start(&app, party, code).await;
    share.hub_starting.store(false, Ordering::SeqCst);
    match result {
        Ok(session) => {
            *share.hub.lock().unwrap() = Some(session);
            // the mirror starts from what plays now
            let snap = app.state::<AppState>().player.snapshot();
            share.snapshots.send_replace(Some((Instant::now(), snap)));
            let view = hub_view(&app);
            let _ = app.emit("share-hub", &view);
            Ok(view)
        }
        Err(e) => {
            *share.hub_error.lock().unwrap() = Some(e.clone());
            emit_hub(&app);
            refresh_discovery(&app);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn share_stop(app: AppHandle) -> CmdResult<()> {
    let share = app.state::<Share>();
    let hub = share.hub.lock().unwrap().take();
    if let Some(h) = hub {
        h.stop(&app).await;
    }
    *share.hub_error.lock().unwrap() = None;
    emit_hub(&app);
    refresh_discovery(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn share_set_party(app: AppHandle, on: bool) {
    if let Some(h) = app.state::<Share>().hub() {
        h.set_party(&app, on);
        emit_hub(&app);
    }
}

#[tauri::command(async)]
pub fn share_set_play_here(app: AppHandle, on: bool) {
    if let Some(h) = app.state::<Share>().hub() {
        h.set_play_here(&app, on);
        emit_hub(&app);
    }
}

/// Removes a guest; `ban` keeps their device out until the session ends.
#[tauri::command(async)]
pub fn share_kick(app: AppHandle, guest_id: String, ban: bool) -> bool {
    let done = app.state::<Share>().hub().is_some_and(|h| h.hub.kick(&guest_id, ban));
    emit_hub(&app);
    done
}

#[tauri::command(async)]
pub fn share_approve(app: AppHandle, queue_id: u64) -> bool {
    let done = app.state::<Share>().hub().is_some_and(|h| h.hub.approve(queue_id));
    emit_hub(&app);
    done
}

#[tauri::command(async)]
pub fn share_reject(app: AppHandle, queue_id: u64) -> bool {
    let done = app.state::<Share>().hub().is_some_and(|h| h.hub.reject(queue_id));
    emit_hub(&app);
    done
}

/// Joins the hub at `host:port` with its code. `name` / `color` default to Settings.
#[tauri::command]
pub async fn share_join(
    app: AppHandle,
    host: String,
    port: u16,
    code: String,
    name: Option<String>,
    color: Option<String>,
    session_name: Option<String>,
) -> CmdResult<GuestView> {
    let share = app.state::<Share>();
    if share.hub().is_some() {
        return Err("Stop sharing first to join someone else\u{2019}s music".into());
    }
    if let Some(old) = share.guest.lock().unwrap().take() {
        old.close();
    }
    let s = app.state::<AppState>().settings().share;
    let seq = share.guest_seq.fetch_add(1, Ordering::SeqCst) + 1;
    share.set_guest_view(GuestView {
        status: "connecting",
        session_name: session_name.unwrap_or_default(),
        host: host.clone(),
        port,
        play_along: s.play_along,
        ..Default::default()
    });
    emit_guest(&app);
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| display_name(&s));
    let color = color.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| s.color.clone());
    let joined = GuestSession::join(&app, seq, host.trim(), port, name.trim(), &color, code.trim()).await;
    if share.guest_seq.load(Ordering::SeqCst) != seq {
        // left (or joined elsewhere) while connecting
        if let Ok(g) = joined {
            g.close();
        }
        return Err("Cancelled".into());
    }
    match joined {
        Ok(session) => {
            *share.guest.lock().unwrap() = Some(session.clone());
            share.discovery.stop_browse();
            let _ = app.emit("share-nearby", Vec::<Nearby>::new());
            let view = session.view();
            let _ = app.emit("share-guest", &view);
            Ok(view)
        }
        Err(e) => {
            let mut view = share.guest_view.lock().unwrap().clone();
            view.status = "disconnected";
            view.error = Some(e.clone());
            share.set_guest_view(view);
            emit_guest(&app);
            Err(e)
        }
    }
}

/// Leaves the session (or stops joining). The app's own queue is as it was.
#[tauri::command(async)]
pub fn share_leave(app: AppHandle) {
    let share = app.state::<Share>();
    share.guest_seq.fetch_add(1, Ordering::SeqCst);
    if let Some(g) = share.guest.lock().unwrap().take() {
        g.close();
    }
    let play_along = app.state::<AppState>().settings().share.play_along;
    share.set_guest_view(GuestView { status: "idle", play_along, ..Default::default() });
    emit_guest(&app);
    refresh_discovery(&app);
}

/// A guest request: search, browse, add, remove, move, skip, vote_skip, toggle,
/// volume, play. Answers and refusals come back as events.
#[tauri::command(async)]
pub fn share_guest(app: AppHandle, message: GuestMessage) -> CmdResult<()> {
    if matches!(
        message,
        GuestMessage::Hello { .. } | GuestMessage::Clock { .. } | GuestMessage::Ready { .. } | GuestMessage::Unknown
    ) {
        return Err("Not a guest request".into());
    }
    let guest = app.state::<Share>().guest().ok_or("Not in a session")?;
    guest.send(message)
}

/// "Play on this computer" while in a party.
#[tauri::command(async)]
pub fn share_play_along(app: AppHandle, on: bool) {
    if let Some(g) = app.state::<Share>().guest() {
        g.set_play_along(on);
    } else {
        let share = app.state::<Share>();
        share.guest_view.lock().unwrap().play_along = on;
    }
    emit_guest(&app);
}

/// Is there a HomeFi hub at this address?
#[tauri::command]
pub async fn share_probe(host: String, port: u16) -> Option<Nearby> {
    discovery::hello(host.trim(), port).await
}

#[derive(Serialize)]
pub struct LinkView {
    host: String,
    port: u16,
    code: String,
    session_name: String,
}

/// Reads a `homefi://join?...` link (QR code, pasted); `None` when it isn't one.
#[tauri::command]
pub fn share_parse_link(link: String) -> Option<LinkView> {
    JoinInfo::parse(&link).map(|j| LinkView { host: j.host, port: j.port, code: j.code, session_name: j.session_name })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against a running debug app started with `HOMEFI_SHARE_AUTOSTART=4821`:
    /// `cargo test -p homefi-desktop smoke -- --ignored --nocapture`. Finds it over
    /// mDNS, asks `/hello`, joins, searches, adds a song, sees it reach the player's
    /// queue (fetched ahead), then removes it again.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a running app"]
    async fn smoke_against_a_running_app() {
        use homefi_core::share::{GuestClient, GuestEvent, HubMessage};
        use std::time::Duration;

        let port: u16 = std::env::var("HOMEFI_SMOKE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(47800);
        let code = std::env::var("HOMEFI_SMOKE_CODE").unwrap_or_else(|_| "4821".into());
        let query = std::env::var("HOMEFI_SMOKE_QUERY").unwrap_or_else(|_| "daft punk".into());

        // mDNS: the app announces itself like Android does
        let daemon = mdns_sd::ServiceDaemon::new().unwrap();
        let rx = daemon.browse(discovery::SERVICE_TYPE).unwrap();
        let mut seen = None;
        let until = std::time::Instant::now() + Duration::from_secs(8);
        while seen.is_none() && std::time::Instant::now() < until {
            if let Ok(mdns_sd::ServiceEvent::ServiceResolved(s)) = rx.recv_timeout(Duration::from_millis(500)) {
                if s.get_port() == port {
                    println!(
                        "mDNS: {} v={:?} name={:?} port={:?}",
                        s.get_fullname(),
                        s.get_property_val_str("v"),
                        s.get_property_val_str("name"),
                        s.get_property_val_str("port")
                    );
                    assert!(s.get_property("code").is_none(), "the code is never announced");
                    seen = Some(s);
                }
            }
        }
        assert!(seen.is_some(), "not announced over mDNS");
        let _ = daemon.shutdown();

        let hello = discovery::hello("127.0.0.1", port).await.expect("/hello answers");
        println!("hello: {} party={:?}", hello.name, hello.party);

        let (guest, welcome, mut rx) =
            GuestClient::connect("127.0.0.1", port, "Smoke test", "#123456", &code).await.unwrap();
        println!("joined {} as {}; {} queued", welcome.session_name, welcome.guest_id, welcome.state.queue.len());
        guest.send(GuestMessage::Search { q: query.clone() }).unwrap();
        let mut results = None;
        while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(20), rx.recv()).await {
            if let GuestEvent::Message(HubMessage::Results(r)) = event {
                results = Some(r);
                break;
            }
        }
        let results = results.expect("search results");
        println!(
            "search {query:?}: {} songs, {} albums, {} artists, {} playlists",
            results.tracks.len(),
            results.albums.len(),
            results.artists.len(),
            results.playlists.len()
        );
        let song = results.tracks.first().cloned().expect("a song to add");
        guest.send(GuestMessage::Add { item: song.clone(), next: false }).unwrap();
        let mut added = None;
        while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            if let GuestEvent::Message(HubMessage::State(s)) = event {
                if let Some(item) =
                    s.queue.iter().find(|i| i.track.source == song.source && i.added_by.name == "Smoke test")
                {
                    added = Some(item.clone());
                    break;
                }
            }
        }
        let added = added.expect("the song is in the shared queue");
        println!("added {} ({}) as queue id {}", added.track.title, added.track.source, added.queue_id);
        // the hub fetches it ahead into its stream cache (same machine here), then takes it back out
        tokio::time::sleep(Duration::from_secs(12)).await;
        if let homefi_core::share::Source::Tidal(id) = &added.track.source {
            let cache = homefi_core::paths::app_dir().join("stream-cache");
            println!(
                "fetched ahead: {:?}",
                tracks::local_file(&added.track.source, &cache).map(|p| p.display().to_string())
            );
            let _ = id;
        }
        guest.send(GuestMessage::Remove { queue_id: added.queue_id }).unwrap();
        let mut gone = false;
        while let Ok(Some(event)) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            if let GuestEvent::Message(HubMessage::State(s)) = event {
                if !s.queue.iter().any(|i| i.queue_id == added.queue_id) {
                    gone = true;
                    break;
                }
            }
        }
        assert!(gone, "removed again");
        guest.close();
    }

    #[test]
    fn names_fall_back_to_the_computer() {
        let mut s = ShareSettings::default();
        assert_eq!(display_name(&s), computer_name());
        assert!(session_name(&s).ends_with("\u{2019}s music"));
        s.name = "  José ".into();
        assert_eq!(session_name(&s), "José\u{2019}s music");
        s.session_name = "Car".into();
        assert_eq!(session_name(&s), "Car");
    }
}
