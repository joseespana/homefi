//! The hub server: one port serving the guests' WebSocket (`GET /ws`), covers
//! (`GET /cover/<id>`), the song playing now for guests that play along
//! (`GET /audio/<queue_id>?k=<key>`, party mode) and, later, the web page (`GET /`).
//!
//! The hub owns the *shared* queue (who added what, in which order) and the
//! rules; the host app owns the player. They meet through [`HubHost`]: the hub
//! asks the host to search, browse, serve covers and apply commands (skip,
//! pause, the new upcoming list), and the host pushes back what the player
//! actually does (`set_now_playing`, `set_position`, `set_queue`).

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, Request, State as Extract};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures::future::BoxFuture;
use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::permissions::{AddDecision, Permissions, SkipDecision, SkipMode};
use super::protocol::{
    AudioStream, BrowseKind, GuestMessage, HubMessage, Member, Page, QueueItem, SearchResults, ShareTrack, State,
    HUB_ID,
};
use super::queue::{votes_needed, SharedQueue, SkipVotes, VoteOutcome};
use super::{JoinInfo, DEFAULT_PORT};
use crate::Result;

/// A guest must say `hello` this soon after connecting.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// The hub pings every guest this often, so phones that left the car (no TCP
/// close) are noticed...
const PING_EVERY: Duration = Duration::from_secs(15);
/// ...and drops a guest silent for this long (pongs count).
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// Guest messages are small; this stops a misbehaving client from making the
/// hub buffer megabytes.
const MAX_MESSAGE: usize = 64 * 1024;
/// Wrong join codes from one address before it is refused for the session
/// (the code is only four digits).
const MAX_CODE_FAILURES: u32 = 5;
/// WebSocket close code sent when the hub ends a guest's connection
/// (4000-4999 is the application range).
const CLOSE_BY_HUB: u16 = 4000;
const MAX_NAME_CHARS: usize = 32;
/// Messages waiting for one guest's socket. Levels (15 a second) are dropped
/// once half of it is in use; anything else finding it full means the guest
/// stopped reading, and it is disconnected.
const GUEST_QUEUE: usize = 64;

/// Image bytes for `GET /cover/<id>`.
#[derive(Debug, Clone)]
pub struct CoverImage {
    pub bytes: Vec<u8>,
    pub mime: String,
}

impl CoverImage {
    /// Guesses the MIME type from the bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        let mime = super::image_mime(&bytes).to_owned();
        Self { bytes, mime }
    }
}

/// Something the host's player must do.
#[derive(Debug, Clone, PartialEq)]
pub enum HostCommand {
    Skip,
    /// Pause / resume.
    Toggle,
    /// 0.0..=1.0
    Volume(f32),
    /// The upcoming list changed (a guest added, removed or moved a song, or
    /// the turn order changed): the player's queue after the current song
    /// should become exactly this. The host reports back via `Hub::set_now_playing`
    /// with the `queue_id` when one starts.
    Queue(Vec<QueueItem>),
}

/// Things the host may want to show (discreetly: the host may be driving).
#[derive(Debug, Clone, PartialEq)]
pub enum HubEvent {
    GuestJoined(Member),
    /// Left, timed out or was kicked.
    GuestLeft(Member),
    /// A guest's song went into the queue.
    Added(QueueItem),
    /// A song waits for `Hub::approve` / `Hub::reject` ([`super::AddMode::Approve`]).
    ApprovalRequested(QueueItem),
}

/// The host app's side of the hub. Calls come from the hub's tasks; `command`
/// and `event` are never called while the hub holds its lock, so they may call
/// back into the [`Hub`].
pub trait HubHost: Send + Sync + 'static {
    /// Search the hub's sources (its Tidal account, its library).
    fn search(&self, q: String) -> BoxFuture<'_, Result<SearchResults>>;
    fn browse(&self, kind: BrowseKind, id: String) -> BoxFuture<'_, Result<Page>>;
    /// Bytes for `GET /cover/<id>`; `None` answers 404.
    fn cover(&self, id: String) -> BoxFuture<'_, Option<CoverImage>>;
    fn command(&self, command: HostCommand);
    fn event(&self, _event: HubEvent) {}
}

#[derive(Debug, Clone)]
pub struct HubConfig {
    /// Shown to guests ("Música de José").
    pub session_name: String,
    /// The hub as a person: name and colour on the host's own songs.
    pub host_name: String,
    pub host_color: String,
    /// Join code; empty picks a random 4-digit one (see [`Hub::code`]).
    pub code: String,
    /// `0.0.0.0:47800` by default; port 0 picks a free one (tests).
    pub bind: SocketAddr,
    pub permissions: Permissions,
    /// Serve the song playing now so guests can play along (party mode); see
    /// [`Hub::set_audio`]. Can change while running ([`Hub::set_streaming`]).
    pub stream_audio: bool,
}

impl HubConfig {
    pub fn new(session_name: impl Into<String>, host_name: impl Into<String>) -> Self {
        Self {
            session_name: session_name.into(),
            host_name: host_name.into(),
            host_color: "#1db954".into(),
            code: String::new(),
            bind: SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)),
            permissions: Permissions::default(),
            stream_audio: false,
        }
    }
}

/// A running hub. Dropping it stops the server (prefer [`Hub::stop`], which
/// also waits for it).
pub struct Hub {
    shared: Arc<Shared>,
    addr: SocketAddr,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    server: Mutex<Option<JoinHandle<()>>>,
}

impl Hub {
    pub async fn start(config: HubConfig, host: Arc<dyn HubHost>) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(config.bind).await?;
        let addr = listener.local_addr()?;
        let code = match config.code.trim() {
            "" => random_code(),
            c => c.to_owned(),
        };
        let me = Member { guest_id: HUB_ID.into(), name: config.host_name, color: config.host_color };
        let inner = Inner {
            session_name: config.session_name,
            code,
            queue: SharedQueue::new(config.permissions.order),
            permissions: config.permissions,
            me,
            guests: Vec::new(),
            next_guest: 1,
            votes: SkipVotes::default(),
            position_ms: 0,
            playing: false,
            volume: None,
            pending: Vec::new(),
            banned: HashSet::new(),
            failures: HashMap::new(),
            stopped: false,
            clock_ms: 0,
            streaming: config.stream_audio,
            audio: None,
            next_audio: None,
            previous_audio: None,
            audio_key: format!("{:016x}", RandomState::new().build_hasher().finish()),
        };
        let shared = Arc::new(Shared { host, inner: Mutex::new(inner), started: Instant::now() });

        let app = Router::new()
            .route("/", get(index))
            .route("/hello", get(hello))
            .route("/ws", get(ws_route))
            .route("/cover/{id}", get(cover))
            .route("/audio/{id}", get(audio))
            .with_state(shared.clone());
        let (tx, rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let service = app.into_make_service_with_connect_info::<SocketAddr>();
            let result = axum::serve(listener, service)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
            if let Err(e) = result {
                tracing::warn!("share hub server stopped: {e}");
            }
        });
        Ok(Self { shared, addr, shutdown: Mutex::new(Some(tx)), server: Mutex::new(Some(server)) })
    }

    /// The bound address (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn code(&self) -> String {
        self.shared.lock().code.clone()
    }

    pub fn session_name(&self) -> String {
        self.shared.lock().session_name.clone()
    }

    /// QR / discovery payload. `host` is the address guests can reach (the
    /// app knows its Wi-Fi Direct / hotspot address; the hub binds all).
    pub fn join_info(&self, host: &str) -> JoinInfo {
        let inner = self.shared.lock();
        JoinInfo {
            host: host.to_owned(),
            port: self.addr.port(),
            code: inner.code.clone(),
            session_name: inner.session_name.clone(),
        }
    }

    pub fn state(&self) -> State {
        self.shared.lock().state()
    }

    /// The hub's clock: milliseconds since it started (what `clock_ms` and
    /// the `clock` answers use).
    pub fn now_ms(&self) -> u64 {
        self.shared.now_ms()
    }

    /// The file of the song playing now, served to guests while streaming. Call
    /// it right after [`Self::set_now_playing`] (it belongs to that song);
    /// `None` when there is no local file (yet).
    pub fn set_audio(&self, file: Option<PathBuf>) {
        let audio = file.and_then(|path| AudioFile::open(0, path));
        self.shared.update(|inner, _| {
            let audio = match (audio, inner.queue.now_playing()) {
                (Some(a), Some(now)) => Some(AudioFile { queue_id: now.queue_id, ..a }),
                _ => None,
            };
            // the song that just ended stays available as "previous" (guests keep it ready)
            let new_id = audio.as_ref().map(|a| a.queue_id);
            if let Some(old) = inner.audio.take() {
                if Some(old.queue_id) != new_id {
                    inner.previous_audio = Some(old);
                }
            }
            if inner.next_audio.as_ref().map(|a| a.queue_id) == new_id {
                inner.next_audio = None;
            }
            inner.audio = audio;
            inner.broadcast();
        })
    }

    /// The file of the song that comes next (`queue_id` from [`State::queue`]), so
    /// guests download it ahead. `None` clears it. Ignored once that song is not next.
    pub fn set_next_audio(&self, queue_id: u64, file: Option<PathBuf>) {
        let audio = file.and_then(|path| AudioFile::open(queue_id, path));
        self.shared.update(|inner, _| {
            inner.next_audio = audio;
            inner.broadcast();
        })
    }

    /// Turns streaming to the guests on or off.
    pub fn set_streaming(&self, on: bool) {
        self.shared.update(|inner, _| {
            inner.streaming = on;
            inner.broadcast();
        })
    }

    pub fn guests(&self) -> Vec<Member> {
        self.shared.lock().members()
    }

    pub fn permissions(&self) -> Permissions {
        self.shared.lock().permissions.clone()
    }

    /// New rules apply to the next request; switching the queue order
    /// reorders right away.
    pub fn set_permissions(&self, permissions: Permissions) {
        self.shared.update(|inner, fx| {
            let before = inner.queue.ids();
            inner.queue.set_order(permissions.order);
            inner.permissions = permissions;
            if inner.queue.ids() != before {
                fx.queue_changed(inner);
            }
            inner.broadcast();
        })
    }

    /// Disconnects a guest. With `ban` their address cannot rejoin this
    /// session. `false` when no such guest is connected.
    pub fn kick(&self, guest_id: &str, ban: bool) -> bool {
        self.shared.update(|inner, fx| {
            let Some(guest) = inner.remove_guest(guest_id, fx) else {
                return false;
            };
            if ban {
                inner.banned.insert(guest.ip);
            }
            guest.tx.close("You were removed from the session");
            inner.broadcast();
            true
        })
    }

    /// The host adds a song: as one more person in the turns, or with `next`
    /// right after the current song, skipping the turns.
    pub fn add(&self, track: ShareTrack, next: bool) -> QueueItem {
        self.shared.update(|inner, fx| {
            let me = inner.me.clone();
            let item = if next { inner.queue.add_pinned(track, me) } else { inner.queue.add(track, me, false) };
            fx.queue_changed(inner);
            inner.broadcast();
            item
        })
    }

    pub fn remove(&self, queue_id: u64) -> bool {
        self.shared.update(|inner, fx| {
            let removed = inner.queue.remove(queue_id).is_some();
            if removed {
                fx.queue_changed(inner);
                inner.broadcast();
            }
            removed
        })
    }

    pub fn move_item(&self, queue_id: u64, to: usize) -> bool {
        self.shared.update(|inner, fx| {
            let moved = inner.queue.move_to(queue_id, to);
            if moved {
                fx.queue_changed(inner);
                inner.broadcast();
            }
            moved
        })
    }

    /// Songs waiting for approval, oldest first.
    pub fn pending(&self) -> Vec<QueueItem> {
        self.shared.lock().pending.clone()
    }

    pub fn approve(&self, queue_id: u64) -> bool {
        self.shared.update(|inner, fx| {
            let Some(pos) = inner.pending.iter().position(|i| i.queue_id == queue_id) else {
                return false;
            };
            let item = inner.pending.remove(pos);
            inner.queue.insert(item.clone(), false);
            fx.events.push(HubEvent::Added(item));
            fx.queue_changed(inner);
            inner.broadcast();
            true
        })
    }

    pub fn reject(&self, queue_id: u64) -> bool {
        self.shared.update(|inner, _| {
            let Some(pos) = inner.pending.iter().position(|i| i.queue_id == queue_id) else {
                return false;
            };
            let item = inner.pending.remove(pos);
            let text = format!("{} did not add \u{201c}{}\u{201d}", inner.me.name, item.track.title);
            inner.send(&item.added_by.guest_id, &HubMessage::Notice { text });
            true
        })
    }

    /// What the player plays now. `queue_id` is the id from a
    /// [`HostCommand::Queue`] item when the song came from the shared queue;
    /// anything else counts as the host's song. `track: None` = nothing.
    pub fn set_now_playing(&self, queue_id: Option<u64>, track: Option<ShareTrack>, position_ms: u64, playing: bool) {
        let now = self.shared.now_ms();
        self.shared.update(|inner, fx| {
            inner.position_ms = position_ms;
            inner.clock_ms = now;
            inner.playing = playing;
            let current = inner.queue.now_playing().map(|i| (i.queue_id, i.track.source.clone()));
            match (queue_id, track) {
                (_, None) => inner.queue.stop(),
                (Some(id), _) if current.as_ref().is_some_and(|c| c.0 == id) => {}
                // Re-pushed without an id: the same source is the same song.
                (None, Some(t)) if current.as_ref().is_some_and(|c| c.1 == t.source) => {}
                (id, Some(t)) => {
                    let before: Vec<u64> = inner.queue.ids().into_iter().filter(|&i| Some(i) != id).collect();
                    if id.and_then(|id| inner.queue.start(id)).is_none() {
                        let me = inner.me.clone();
                        inner.queue.play_other(t, me);
                    }
                    // A song starting changes whose turn it is.
                    if inner.queue.ids() != before {
                        fx.queue_changed(inner);
                    }
                }
            }
            let now = inner.queue.now_playing().map(|i| i.queue_id);
            inner.votes.track_changed(now);
            inner.broadcast();
        })
    }

    /// Party mode: the song starts from 0 in `delay_ms` on the hub clock, for the host and
    /// every guest at once (guests wait at 0 until then). Returns that start time; the
    /// host starts its own player when [`Self::now_ms`] reaches it. Calling it again moves it.
    pub fn schedule_start(&self, delay_ms: u64) -> u64 {
        let start = self.shared.now_ms() + delay_ms;
        self.shared.update(|inner, _| {
            inner.position_ms = 0;
            inner.clock_ms = start;
            inner.playing = true;
            inner.broadcast();
        });
        start
    }

    /// The music's levels right now (see [`HubMessage::Levels`]); sent to every guest,
    /// not part of the state.
    pub fn send_levels(&self, left: u8, right: u8, bands: Vec<u8>) {
        let inner = self.shared.lock();
        if !inner.guests.is_empty() {
            inner.send_all(&HubMessage::Levels { l: left, r: right, b: bands });
        }
    }

    /// `(ready, connected)`: how many guests said they can start `queue_id` at once.
    pub fn ready_guests(&self, queue_id: u64) -> (usize, usize) {
        let inner = self.shared.lock();
        (inner.guests.iter().filter(|g| g.ready == Some(queue_id)).count(), inner.guests.len())
    }

    /// Call on play / pause / seek, and every few seconds while playing so
    /// guests that play along can correct their drift.
    pub fn set_position(&self, position_ms: u64, playing: bool) {
        let now = self.shared.now_ms();
        self.shared.update(|inner, _| {
            inner.position_ms = position_ms;
            inner.clock_ms = now;
            inner.playing = playing;
            inner.broadcast();
        })
    }

    /// Mirrors the player's upcoming list after the host changed it by hand:
    /// `(queue_id, track)` pairs, `None` (or an unknown id) for the host's own
    /// songs. Kept in this order until the next change.
    pub fn set_queue(&self, items: Vec<(Option<u64>, ShareTrack)>) {
        self.shared.update(|inner, _| {
            let me = inner.me.clone();
            inner.queue.replace(items, &me);
            inner.broadcast();
        })
    }

    pub fn set_volume(&self, volume: Option<f32>) {
        self.shared.update(|inner, _| {
            inner.volume = volume.map(|v| v.clamp(0.0, 1.0));
            inner.broadcast();
        })
    }

    /// Shows `text` to one guest, or everyone with `None`.
    pub fn notify(&self, guest_id: Option<&str>, text: &str) {
        let inner = self.shared.lock();
        let msg = HubMessage::Notice { text: text.to_owned() };
        match guest_id {
            Some(id) => inner.send(id, &msg),
            None => inner.send_all(&msg),
        }
    }

    /// Says goodbye to every guest and stops the server.
    pub async fn stop(&self) {
        self.shared.close_all();
        if let Some(tx) = lock(&self.shutdown).take() {
            let _ = tx.send(());
        }
        let server = lock(&self.server).take();
        if let Some(mut server) = server {
            if tokio::time::timeout(Duration::from_secs(3), &mut server).await.is_err() {
                server.abort();
            }
        }
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.shared.close_all();
        if let Some(tx) = lock(&self.shutdown).take() {
            let _ = tx.send(());
        }
    }
}

/// A poisoned lock only means another task panicked mid-update; the data is
/// still usable and the hub must keep serving.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn random_code() -> String {
    // `RandomState` is seeded from the OS per instance: good enough for a
    // join code without another dependency.
    let n = RandomState::new().build_hasher().finish();
    format!("{:04}", n % 10_000)
}

/// What kind of [`HubMessage`] an encoded text is, for [`GuestTx::push`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Latest wins: a slow guest skips states it would only overwrite.
    State,
    /// Dropped when the guest's queue is getting full.
    Levels,
    Other,
}

impl Kind {
    fn of(msg: &HubMessage) -> Self {
        match msg {
            HubMessage::State(_) => Kind::State,
            HubMessage::Levels { .. } => Kind::Levels,
            _ => Kind::Other,
        }
    }
}

/// Latest-wins part of what goes to a guest.
#[derive(Debug, Clone, Default)]
struct Latest {
    state: Option<Arc<str>>,
    /// Close the connection with this reason (after what is queued).
    close: Option<String>,
}

/// The hub's side of one guest's connection: a bounded queue of messages plus
/// the latest state, so a guest on a bad connection never makes the hub
/// buffer without limit.
#[derive(Clone)]
struct GuestTx {
    queue: mpsc::Sender<String>,
    latest: Arc<watch::Sender<Latest>>,
}

/// The writer task's side of [`GuestTx`].
struct GuestRx {
    queue: mpsc::Receiver<String>,
    latest: watch::Receiver<Latest>,
}

impl GuestTx {
    fn channel() -> (Self, GuestRx) {
        let (queue, queue_rx) = mpsc::channel(GUEST_QUEUE);
        let (latest, latest_rx) = watch::channel(Latest::default());
        (Self { queue, latest: Arc::new(latest) }, GuestRx { queue: queue_rx, latest: latest_rx })
    }

    fn send(&self, msg: &HubMessage) {
        match serde_json::to_string(msg) {
            Ok(text) => self.push(Kind::of(msg), text),
            Err(e) => tracing::warn!("share: could not encode a hub message: {e}"),
        }
    }

    /// Never blocks (it runs under the hub's lock).
    fn push(&self, kind: Kind, text: String) {
        match kind {
            Kind::State => self.latest.send_modify(|l| l.state = Some(text.into())),
            Kind::Levels => {
                if self.queue.capacity() > GUEST_QUEUE / 2 {
                    let _ = self.queue.try_send(text);
                }
            }
            Kind::Other => {
                if let Err(mpsc::error::TrySendError::Full(_)) = self.queue.try_send(text) {
                    tracing::debug!("share: a guest stopped reading, disconnecting it");
                    self.close("The connection to the hub is too slow");
                }
            }
        }
    }

    fn close(&self, reason: &str) {
        self.latest.send_modify(|l| {
            l.close.get_or_insert_with(|| reason.to_owned());
        });
    }
}

/// The file behind [`State::audio`] (and [`State::prefetch`]).
struct AudioFile {
    queue_id: u64,
    path: PathBuf,
    mime: String,
    size: u64,
}

impl AudioFile {
    fn open(queue_id: u64, path: PathBuf) -> Option<Self> {
        let size = std::fs::metadata(&path).ok()?.len();
        Some(Self { queue_id, mime: audio_mime(&path).into(), path, size })
    }

    fn stream(&self, key: &str) -> AudioStream {
        AudioStream {
            queue_id: self.queue_id,
            url: format!("/audio/{}?k={}", self.queue_id, key),
            mime: self.mime.clone(),
            size: self.size,
        }
    }
}

/// MIME type from the file name (players use it to pick a decoder).
fn audio_mime(path: &FsPath) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("flac") => "audio/flac",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "mp4" | "aac" | "alac") => "audio/mp4",
        Some("ogg" | "oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

struct Guest {
    member: Member,
    ip: IpAddr,
    tx: GuestTx,
    /// Last song this guest said it is ready to start (`GuestMessage::Ready`).
    ready: Option<u64>,
}

struct Inner {
    session_name: String,
    code: String,
    permissions: Permissions,
    me: Member,
    guests: Vec<Guest>,
    next_guest: u64,
    queue: SharedQueue,
    votes: SkipVotes,
    position_ms: u64,
    playing: bool,
    volume: Option<f32>,
    pending: Vec<QueueItem>,
    banned: HashSet<IpAddr>,
    failures: HashMap<IpAddr, u32>,
    stopped: bool,
    /// Hub clock when `position_ms` was last set.
    clock_ms: u64,
    streaming: bool,
    audio: Option<AudioFile>,
    /// Served too while streaming, for guests to download ahead.
    next_audio: Option<AudioFile>,
    previous_audio: Option<AudioFile>,
    /// Part of the audio URLs, so only guests who joined can fetch the songs.
    audio_key: String,
}

/// Host calls collected under the lock and made after releasing it.
#[derive(Default)]
struct Effects {
    commands: Vec<HostCommand>,
    events: Vec<HubEvent>,
}

impl Effects {
    fn queue_changed(&mut self, inner: &Inner) {
        self.commands.push(HostCommand::Queue(inner.queue.upcoming()));
    }
}

impl Inner {
    /// Next (only while it still is next) then previous (unless it is playing again).
    fn prefetched(&self) -> impl Iterator<Item = &AudioFile> {
        let next_id = self.queue.upcoming().first().map(|q| q.queue_id);
        let now_id = self.queue.now_playing().map(|q| q.queue_id);
        let next = self.next_audio.as_ref().filter(move |a| Some(a.queue_id) == next_id);
        let before = move |a: &&AudioFile| Some(a.queue_id) != now_id && Some(a.queue_id) != next_id;
        // the current file of a song that just ended counts as previous until set_audio
        let just_ended = self.audio.as_ref().filter(before);
        let previous = self.previous_audio.as_ref().filter(before);
        next.into_iter().chain(just_ended).chain(previous)
    }

    fn members(&self) -> Vec<Member> {
        self.guests.iter().map(|g| g.member.clone()).collect()
    }

    fn state(&self) -> State {
        let now = self.queue.now_playing();
        let audio = match (&self.audio, now) {
            (Some(a), Some(now)) if self.streaming && a.queue_id == now.queue_id => Some(a.stream(&self.audio_key)),
            _ => None,
        };
        let prefetch =
            if self.streaming { self.prefetched().map(|a| a.stream(&self.audio_key)).collect() } else { Vec::new() };
        State {
            now_playing: now.cloned(),
            position_ms: self.position_ms,
            clock_ms: self.clock_ms,
            playing: self.playing,
            audio,
            prefetch,
            queue: self.queue.upcoming(),
            guests: self.members(),
            permissions: self.permissions.clone(),
            skip_votes: self.votes.count(),
            skip_votes_needed: votes_needed(self.guests.len()),
            volume: self.volume,
        }
    }

    fn send(&self, guest_id: &str, msg: &HubMessage) {
        if let Some(g) = self.guests.iter().find(|g| g.member.guest_id == guest_id) {
            g.tx.send(msg);
        }
    }

    fn send_all(&self, msg: &HubMessage) {
        // Serialize once for everyone.
        let Ok(text) = serde_json::to_string(msg) else { return };
        let kind = Kind::of(msg);
        for g in &self.guests {
            g.tx.push(kind, text.clone());
        }
    }

    fn broadcast(&self) {
        self.send_all(&HubMessage::State(self.state()));
    }

    fn remove_guest(&mut self, guest_id: &str, fx: &mut Effects) -> Option<Guest> {
        let pos = self.guests.iter().position(|g| g.member.guest_id == guest_id)?;
        let guest = self.guests.remove(pos);
        if self.votes.forget(guest_id, self.guests.len()) == Some(VoteOutcome::Skip) {
            fx.commands.push(HostCommand::Skip);
        }
        if self.permissions.drop_songs_on_leave {
            self.pending.retain(|i| i.added_by.guest_id != guest_id);
            if self.queue.remove_owner(guest_id) > 0 {
                fx.queue_changed(self);
            }
        }
        fx.events.push(HubEvent::GuestLeft(guest.member.clone()));
        Some(guest)
    }

    fn join(&mut self, hello: Hello, ip: IpAddr, tx: GuestTx) -> Result<Member, String> {
        if self.stopped {
            return Err("The host stopped sharing".into());
        }
        if self.banned.contains(&ip) {
            return Err("You were removed from this session".into());
        }
        let failures = self.failures.get(&ip).copied().unwrap_or(0);
        if failures >= MAX_CODE_FAILURES {
            return Err("Too many wrong join codes".into());
        }
        if hello.code.trim() != self.code {
            self.failures.insert(ip, failures + 1);
            return Err("Wrong join code".into());
        }
        self.failures.remove(&ip);

        let n = self.next_guest;
        self.next_guest += 1;
        let name: String = hello.name.trim().chars().take(MAX_NAME_CHARS).collect();
        let member = Member {
            guest_id: format!("g{n}"),
            name: if name.is_empty() { format!("Guest {n}") } else { name },
            color: hello.color.trim().chars().take(16).collect(),
        };
        self.guests.push(Guest { member: member.clone(), ip, tx: tx.clone(), ready: None });
        // Welcome first, so the guest never sees a state before it (the
        // writer sends the first queued message before anything else).
        tx.send(&HubMessage::Welcome {
            guest_id: member.guest_id.clone(),
            session_name: self.session_name.clone(),
            permissions: self.permissions.clone(),
            state: self.state(),
        });
        Ok(member)
    }

    fn handle(&mut self, who: &Member, msg: GuestMessage, fx: &mut Effects) {
        let deny = |inner: &Inner, reason: String| inner.send(&who.guest_id, &HubMessage::Denied { reason });
        match msg {
            GuestMessage::Add { item, next } => {
                let waiting = self.queue.waiting_count(&who.guest_id)
                    + self.pending.iter().filter(|i| i.added_by.guest_id == who.guest_id).count();
                match self.permissions.check_add(&item, waiting) {
                    Err(reason) => deny(self, reason),
                    Ok(AddDecision::Queue) => {
                        let item = self.queue.add(item, who.clone(), next);
                        fx.events.push(HubEvent::Added(item));
                        fx.queue_changed(self);
                        self.broadcast();
                    }
                    Ok(AddDecision::AskHost) => {
                        let item = self.queue.new_item(item, who.clone());
                        let text =
                            format!("Waiting for {} to approve \u{201c}{}\u{201d}", self.me.name, item.track.title);
                        self.pending.push(item.clone());
                        fx.events.push(HubEvent::ApprovalRequested(item));
                        self.send(&who.guest_id, &HubMessage::Notice { text });
                    }
                }
            }
            GuestMessage::Remove { queue_id } => {
                let Some(item) = self.queue.get(queue_id) else {
                    return deny(self, "That song is no longer in the queue".into());
                };
                if let Err(reason) = self.permissions.check_edit(who, item) {
                    return deny(self, reason);
                }
                self.queue.remove(queue_id);
                fx.queue_changed(self);
                self.broadcast();
            }
            GuestMessage::Move { queue_id, to } => {
                let Some(item) = self.queue.get(queue_id) else {
                    return deny(self, "That song is no longer in the queue".into());
                };
                if let Err(reason) = self.permissions.check_edit(who, item) {
                    return deny(self, reason);
                }
                self.queue.move_to(queue_id, to);
                fx.queue_changed(self);
                self.broadcast();
            }
            GuestMessage::Play { queue_id } => {
                if self.queue.get(queue_id).is_none() {
                    return deny(self, "That song is no longer in the queue".into());
                }
                if !self.permissions.playback && self.permissions.skip != SkipMode::Everyone {
                    return deny(self, "Only the host can choose what plays now".into());
                }
                self.queue.move_to(queue_id, 0);
                fx.queue_changed(self);
                fx.commands.push(HostCommand::Skip);
                self.broadcast();
            }
            GuestMessage::Skip | GuestMessage::VoteSkip => match self.permissions.check_skip() {
                Err(reason) => deny(self, reason),
                Ok(SkipDecision::Skip) => fx.commands.push(HostCommand::Skip),
                Ok(SkipDecision::Vote) => {
                    if self.queue.now_playing().is_none() {
                        return deny(self, "Nothing is playing".into());
                    }
                    match self.votes.vote(&who.guest_id, self.guests.len()) {
                        VoteOutcome::AlreadyVoted => {
                            self.send(&who.guest_id, &HubMessage::Notice { text: "You already voted".into() })
                        }
                        VoteOutcome::Skip => {
                            fx.commands.push(HostCommand::Skip);
                            self.broadcast();
                        }
                        VoteOutcome::Counted { .. } => self.broadcast(),
                    }
                }
            },
            GuestMessage::Toggle => match self.permissions.check_playback() {
                Ok(()) => fx.commands.push(HostCommand::Toggle),
                Err(reason) => deny(self, reason),
            },
            GuestMessage::Volume { value } => match self.permissions.check_playback() {
                Ok(()) if value.is_finite() => fx.commands.push(HostCommand::Volume(value.clamp(0.0, 1.0))),
                Ok(()) => deny(self, "Invalid volume".into()),
                Err(reason) => deny(self, reason),
            },
            // A second hello is harmless; clock is answered by `Shared::handle`.
            GuestMessage::Ready { queue_id } => {
                if let Some(g) = self.guests.iter_mut().find(|g| g.member.guest_id == who.guest_id) {
                    g.ready = Some(queue_id);
                }
            }
            GuestMessage::Hello { .. } | GuestMessage::Clock { .. } => {}
            // Search / browse are answered asynchronously by `Shared::handle`.
            GuestMessage::Search { .. } | GuestMessage::Browse { .. } => {}
            GuestMessage::Unknown => deny(self, "Not supported by this hub".into()),
        }
    }
}

struct Hello {
    name: String,
    color: String,
    code: String,
}

struct Shared {
    host: Arc<dyn HubHost>,
    inner: Mutex<Inner>,
    /// Zero of the hub's clock.
    started: Instant,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Runs `f` under the lock, then calls the host with what it collected.
    fn update<R>(&self, f: impl FnOnce(&mut Inner, &mut Effects) -> R) -> R {
        let mut fx = Effects::default();
        let result = {
            let mut inner = self.lock();
            f(&mut inner, &mut fx)
        };
        for event in fx.events {
            self.host.event(event);
        }
        for command in fx.commands {
            self.host.command(command);
        }
        result
    }

    fn close_all(&self) {
        let mut inner = self.lock();
        inner.stopped = true;
        for g in inner.guests.drain(..) {
            g.tx.close("The host stopped sharing");
        }
    }

    fn join(&self, hello: Hello, ip: IpAddr, tx: GuestTx) -> Result<Member, String> {
        self.update(|inner, fx| {
            let member = inner.join(hello, ip, tx)?;
            fx.events.push(HubEvent::GuestJoined(member.clone()));
            inner.broadcast();
            Ok(member)
        })
    }

    fn leave(&self, guest_id: &str) {
        self.update(|inner, fx| {
            if inner.remove_guest(guest_id, fx).is_some() {
                inner.broadcast();
            }
        })
    }

    /// The guest's sender, `None` once it was kicked / left.
    fn sender(&self, guest_id: &str) -> Option<GuestTx> {
        self.lock().guests.iter().find(|g| g.member.guest_id == guest_id).map(|g| g.tx.clone())
    }

    fn handle(self: &Arc<Self>, who: &Member, msg: GuestMessage) {
        match msg {
            // answered at once, without the lock, so the round trip stays short
            GuestMessage::Clock { t0 } => {
                if let Some(tx) = self.sender(&who.guest_id) {
                    tx.send(&HubMessage::Clock { t0, hub_ms: self.now_ms() });
                }
            }
            GuestMessage::Search { q } => {
                let Some(tx) = self.sender(&who.guest_id) else { return };
                let host = self.host.clone();
                tokio::spawn(async move {
                    let reply = match host.search(q.clone()).await {
                        Ok(mut results) => {
                            results.q = q;
                            HubMessage::Results(results)
                        }
                        Err(e) => HubMessage::Notice { text: format!("Search failed: {e}") },
                    };
                    tx.send(&reply);
                });
            }
            GuestMessage::Browse { kind, id } => {
                let Some(tx) = self.sender(&who.guest_id) else { return };
                let host = self.host.clone();
                tokio::spawn(async move {
                    let reply = match host.browse(kind, id).await {
                        Ok(page) => HubMessage::Page(page),
                        Err(e) => HubMessage::Notice { text: format!("Could not open it: {e}") },
                    };
                    tx.send(&reply);
                });
            }
            msg => self.update(|inner, fx| {
                // Ignore what a kicked guest still had in flight.
                if inner.guests.iter().any(|g| g.member.guest_id == who.guest_id) {
                    inner.handle(who, msg, fx);
                }
            }),
        }
    }
}

// ------------------------------------------------------------------ routes

async fn index(Extract(shared): Extract<Arc<Shared>>) -> Html<String> {
    let name = html_escape(&shared.lock().session_name);
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{name} · HomeFi</title></head>\
         <body style=\"font-family:sans-serif;text-align:center;padding:3em 1em\">\
         <h1>{name}</h1><p>Open HomeFi on your phone to join. The web remote is coming soon.</p></body></html>"
    ))
}

/// Public "is this a HomeFi hub?" for guests that cannot scan (no camera, no
/// location): they ask the gateway of the Wi-Fi they are on. Never the code.
async fn hello(Extract(shared): Extract<Arc<Shared>>) -> impl IntoResponse {
    let inner = shared.lock();
    axum::Json(serde_json::json!({
        "app": "homefi",
        "v": 1,
        "name": inner.session_name,
        "party": inner.streaming,
    }))
}

fn html_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&#39;".into(),
            c => c.to_string(),
        })
        .collect()
}

async fn cover(Extract(shared): Extract<Arc<Shared>>, Path(id): Path<String>) -> Response {
    match shared.host.cover(id).await {
        Some(img) => {
            ([(header::CONTENT_TYPE, img.mime), (header::CACHE_CONTROL, "max-age=3600".to_owned())], img.bytes)
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct AudioQuery {
    #[serde(default)]
    k: String,
}

/// The song playing now (with range requests), for guests that joined.
async fn audio(
    Extract(shared): Extract<Arc<Shared>>,
    Path(id): Path<u64>,
    Query(q): Query<AudioQuery>,
    req: Request,
) -> Response {
    let file = {
        let inner = shared.lock();
        let now = inner.audio.as_ref().filter(|a| a.queue_id == id);
        match now.or_else(|| inner.prefetched().find(|a| a.queue_id == id)) {
            Some(a) if inner.streaming && q.k == inner.audio_key => Some((a.path.clone(), a.mime.clone())),
            _ => None,
        }
    };
    let Some((path, mime)) = file else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match ServeFile::new(path).oneshot(req).await {
        Ok(res) => {
            let mut res = res.map(Body::new);
            if let Ok(v) = header::HeaderValue::from_str(&mime) {
                res.headers_mut().insert(header::CONTENT_TYPE, v);
            }
            res.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn ws_route(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extract(shared): Extract<Arc<Shared>>,
) -> Response {
    ws.max_message_size(MAX_MESSAGE).on_upgrade(move |socket| serve_guest(shared, socket, addr.ip()))
}

/// Next text frame, skipping control frames; `None` when the socket ends.
async fn next_text(stream: &mut SplitStream<WebSocket>) -> Option<String> {
    loop {
        match stream.next().await? {
            Ok(Message::Text(t)) => return Some(t.as_str().to_owned()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

async fn serve_guest(shared: Arc<Shared>, socket: WebSocket, ip: IpAddr) {
    let (mut sink, mut stream) = socket.split();

    let hello = match tokio::time::timeout(HELLO_TIMEOUT, next_text(&mut stream)).await {
        Ok(Some(text)) => match serde_json::from_str::<GuestMessage>(&text) {
            Ok(GuestMessage::Hello { name, color, code, .. }) => Some(Hello { name, color, code }),
            _ => None,
        },
        _ => None,
    };
    let Some(hello) = hello else {
        let _ = sink.send(close_frame("Expected hello")).await;
        return;
    };

    let (tx, rx) = GuestTx::channel();
    let member = match shared.join(hello, ip, tx) {
        Ok(m) => m,
        Err(reason) => {
            if let Ok(text) = serde_json::to_string(&HubMessage::Denied { reason: reason.clone() }) {
                let _ = sink.send(Message::Text(text.into())).await;
            }
            let _ = sink.send(close_frame(&reason)).await;
            return;
        }
    };

    let mut writer = tokio::spawn(write_guest(sink, rx));

    loop {
        tokio::select! {
            // Kicked, hub stopped or the socket broke on write.
            _ = &mut writer => break,
            next = tokio::time::timeout(IDLE_TIMEOUT, stream.next()) => match next {
                Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<GuestMessage>(t.as_str()) {
                    Ok(msg) => shared.handle(&member, msg),
                    Err(e) => tracing::debug!("share: bad message from {}: {e}", member.guest_id),
                },
                Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
                Ok(Some(Ok(_))) => {}
            },
        }
    }
    shared.leave(&member.guest_id);
    writer.abort();
}

/// Sends one guest what [`GuestTx`] collected: the welcome first, then queued
/// messages in order, with the newest state as soon as the queue is empty;
/// pings keep the connection alive.
async fn write_guest(mut sink: futures::stream::SplitSink<WebSocket, Message>, mut rx: GuestRx) {
    // the welcome is always the first queued message
    let Some(welcome) = rx.queue.recv().await else { return };
    if sink.send(Message::Text(welcome.into())).await.is_err() {
        return;
    }
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        tokio::select! {
            biased;
            out = rx.queue.recv() => match out {
                Some(t) => {
                    if sink.send(Message::Text(t.into())).await.is_err() {
                        return;
                    }
                }
                // the hub let go of this guest (kicked, stopped, left)
                None => break,
            },
            changed = rx.latest.changed() => {
                if changed.is_err() {
                    break;
                }
                let latest = rx.latest.borrow_and_update().clone();
                if latest.close.is_some() {
                    break;
                }
                if let Some(state) = latest.state {
                    if sink.send(Message::Text(state.as_ref().into())).await.is_err() {
                        return;
                    }
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
        }
    }
    // the reason survives the hub dropping its side
    let reason = rx.latest.borrow().close.clone();
    match reason {
        Some(reason) => {
            let _ = sink.send(close_frame(&reason)).await;
        }
        None => {
            let _ = sink.close().await;
        }
    }
}

fn close_frame(reason: &str) -> Message {
    Message::Close(Some(CloseFrame { code: CLOSE_BY_HUB, reason: reason.into() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> HubMessage {
        HubMessage::Levels { l: 1, r: 2, b: vec![3; 16] }
    }

    #[test]
    fn a_guest_that_stops_reading_never_grows_the_queue() {
        let (tx, mut rx) = GuestTx::channel();
        // ten minutes of levels with nobody reading
        for _ in 0..15 * 600 {
            tx.send(&levels());
        }
        assert!(rx.queue.len() <= GUEST_QUEUE / 2, "levels stop at half the queue: {}", rx.queue.len());
        // other messages still fit after the levels
        tx.send(&HubMessage::Notice { text: "hi".into() });
        assert!(rx.latest.borrow().close.is_none());
        let queued: Vec<String> = std::iter::from_fn(|| rx.queue.try_recv().ok()).collect();
        assert!(queued.last().unwrap().contains("hi"));
    }

    #[test]
    fn states_are_latest_wins() {
        let (tx, mut rx) = GuestTx::channel();
        for playing in [true, false, true] {
            tx.send(&HubMessage::State(State { playing, position_ms: 7, ..Default::default() }));
        }
        assert!(rx.queue.try_recv().is_err(), "states don't queue");
        assert!(rx.latest.has_changed().unwrap());
        let state = rx.latest.borrow_and_update().state.clone().unwrap();
        let back: HubMessage = serde_json::from_str(&state).unwrap();
        assert!(matches!(back, HubMessage::State(State { playing: true, position_ms: 7, .. })));
        assert!(!rx.latest.has_changed().unwrap());
    }

    #[test]
    fn a_full_queue_disconnects_instead_of_dropping_answers() {
        let (tx, rx) = GuestTx::channel();
        for i in 0..GUEST_QUEUE {
            tx.send(&HubMessage::Notice { text: i.to_string() });
        }
        assert!(rx.latest.borrow().close.is_none());
        tx.send(&HubMessage::Notice { text: "one too many".into() });
        assert!(rx.latest.borrow().close.is_some());
        // the first reason stays
        tx.close("later");
        assert_ne!(rx.latest.borrow().close.as_deref(), Some("later"));
    }
}
