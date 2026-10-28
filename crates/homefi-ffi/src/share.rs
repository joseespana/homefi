//! Shared control ("car mode") for Android: run a hub ([`start_hub`]) or join
//! one as a guest ([`join_hub`]). See `docs/shared-control.md`.
//!
//! The Kotlin hub implements [`HubHostCallback`] (search, browse, covers,
//! player commands) and pushes what its player does through [`ShareHub`]. The
//! Kotlin guest gets a [`ShareGuest`] to send requests and a [`GuestListener`]
//! for everything the hub sends.

use std::net::SocketAddr;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use homefi_core::share::{self, permissions, GuestClient, GuestMessage, HostCommand, Hub, HubConfig, HubMessage};
use tokio::task::JoinHandle;

use crate::{HomefiError, Result};

fn share_error(message: impl Into<String>) -> HomefiError {
    HomefiError::Failed { message: message.into() }
}

// ------------------------------------------------------------------ records

/// A song as guests see it. `source` is `tidal:<id>` or `hub:<library id>`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareTrack {
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
    /// Absolute URL, or a hub path (`/cover/<id>`): see [`ShareGuest::resolve_url`].
    pub cover_url: Option<String>,
    pub explicit: bool,
}

impl From<share::ShareTrack> for ShareTrack {
    fn from(t: share::ShareTrack) -> Self {
        Self {
            source: t.source.to_string(),
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_ms: t.duration_ms,
            cover_url: t.cover_url,
            explicit: t.explicit,
        }
    }
}

impl From<ShareTrack> for share::ShareTrack {
    fn from(t: ShareTrack) -> Self {
        Self {
            source: share::Source::from(t.source),
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_ms: t.duration_ms,
            cover_url: t.cover_url,
            explicit: t.explicit,
        }
    }
}

/// A person in the session; the hub itself has `guest_id` = `hub`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareMember {
    pub guest_id: String,
    pub name: String,
    /// `#RRGGBB`
    pub color: String,
}

impl From<share::Member> for ShareMember {
    fn from(m: share::Member) -> Self {
        Self { guest_id: m.guest_id, name: m.name, color: m.color }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareQueueItem {
    pub queue_id: u64,
    pub track: ShareTrack,
    pub added_by: ShareMember,
}

impl From<share::QueueItem> for ShareQueueItem {
    fn from(i: share::QueueItem) -> Self {
        Self { queue_id: i.queue_id, track: i.track.into(), added_by: i.added_by.into() }
    }
}

fn items(list: Vec<share::QueueItem>) -> Vec<ShareQueueItem> {
    list.into_iter().map(Into::into).collect()
}

/// An upcoming song as the hub's player has it, for [`ShareHub::set_queue`]:
/// `queue_id` from a [`HubCommand::Queue`] item, `None` for the host's own songs.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareQueueEntry {
    pub queue_id: Option<u64>,
    pub track: ShareTrack,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareBrowseKind {
    Album,
    Artist,
    Playlist,
    Unknown,
}

impl From<share::BrowseKind> for ShareBrowseKind {
    fn from(k: share::BrowseKind) -> Self {
        match k {
            share::BrowseKind::Album => Self::Album,
            share::BrowseKind::Artist => Self::Artist,
            share::BrowseKind::Playlist => Self::Playlist,
            share::BrowseKind::Unknown => Self::Unknown,
        }
    }
}

impl From<ShareBrowseKind> for share::BrowseKind {
    fn from(k: ShareBrowseKind) -> Self {
        match k {
            ShareBrowseKind::Album => Self::Album,
            ShareBrowseKind::Artist => Self::Artist,
            ShareBrowseKind::Playlist => Self::Playlist,
            ShareBrowseKind::Unknown => Self::Unknown,
        }
    }
}

/// An album, artist or playlist row.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareCollection {
    pub kind: ShareBrowseKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub cover_url: Option<String>,
}

impl From<share::Collection> for ShareCollection {
    fn from(c: share::Collection) -> Self {
        Self { kind: c.kind.into(), id: c.id, title: c.title, subtitle: c.subtitle, cover_url: c.cover_url }
    }
}

impl From<ShareCollection> for share::Collection {
    fn from(c: ShareCollection) -> Self {
        Self { kind: c.kind.into(), id: c.id, title: c.title, subtitle: c.subtitle, cover_url: c.cover_url }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareSearchResults {
    /// The query answered (the hub fills it in; hosts may leave it empty).
    pub q: String,
    pub tracks: Vec<ShareTrack>,
    pub albums: Vec<ShareCollection>,
    pub artists: Vec<ShareCollection>,
    pub playlists: Vec<ShareCollection>,
}

fn conv<A: Into<B>, B>(list: Vec<A>) -> Vec<B> {
    list.into_iter().map(Into::into).collect()
}

impl From<share::SearchResults> for ShareSearchResults {
    fn from(r: share::SearchResults) -> Self {
        Self {
            q: r.q,
            tracks: conv(r.tracks),
            albums: conv(r.albums),
            artists: conv(r.artists),
            playlists: conv(r.playlists),
        }
    }
}

impl From<ShareSearchResults> for share::SearchResults {
    fn from(r: ShareSearchResults) -> Self {
        Self {
            q: r.q,
            tracks: conv(r.tracks),
            albums: conv(r.albums),
            artists: conv(r.artists),
            playlists: conv(r.playlists),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SharePage {
    pub header: ShareCollection,
    pub tracks: Vec<ShareTrack>,
}

impl From<share::Page> for SharePage {
    fn from(p: share::Page) -> Self {
        Self { header: p.header.into(), tracks: conv(p.tracks) }
    }
}

impl From<SharePage> for share::Page {
    fn from(p: SharePage) -> Self {
        Self { header: p.header.into(), tracks: conv(p.tracks) }
    }
}

// ------------------------------------------------------------- permissions

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareAddMode {
    Free,
    /// Every song waits for [`ShareHub::approve`].
    Approve,
    /// At most `max_per_guest` waiting songs per guest.
    Limit,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareQueueOrder {
    FirstCome,
    /// Round robin per person (the hub counts as one).
    Turns,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareSkipMode {
    Nobody,
    Everyone,
    /// Half of the connected guests, rounded up.
    Vote,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareEditMode {
    Own,
    All,
    None,
}

/// The hub's rules; [`share_default_permissions`] gives the defaults.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SharePermissions {
    pub add: ShareAddMode,
    pub max_per_guest: u32,
    pub order: ShareQueueOrder,
    pub skip: ShareSkipMode,
    /// Guests may pause / resume and change the volume.
    pub playback: bool,
    /// Remove and move.
    pub edit: ShareEditMode,
    pub allow_explicit: bool,
    /// A guest who leaves (or is kicked) takes their waiting songs along.
    pub drop_songs_on_leave: bool,
}

impl From<permissions::Permissions> for SharePermissions {
    fn from(p: permissions::Permissions) -> Self {
        use permissions::{AddMode, EditMode, QueueOrder, SkipMode};
        Self {
            add: match p.add {
                AddMode::Free => ShareAddMode::Free,
                AddMode::Approve => ShareAddMode::Approve,
                AddMode::Limit => ShareAddMode::Limit,
            },
            max_per_guest: p.max_per_guest,
            order: match p.order {
                QueueOrder::FirstCome => ShareQueueOrder::FirstCome,
                QueueOrder::Turns => ShareQueueOrder::Turns,
            },
            skip: match p.skip {
                SkipMode::Nobody => ShareSkipMode::Nobody,
                SkipMode::Everyone => ShareSkipMode::Everyone,
                SkipMode::Vote => ShareSkipMode::Vote,
            },
            playback: p.playback,
            edit: match p.edit {
                EditMode::Own => ShareEditMode::Own,
                EditMode::All => ShareEditMode::All,
                EditMode::None => ShareEditMode::None,
            },
            allow_explicit: p.allow_explicit,
            drop_songs_on_leave: p.drop_songs_on_leave,
        }
    }
}

impl From<SharePermissions> for permissions::Permissions {
    fn from(p: SharePermissions) -> Self {
        use permissions::{AddMode, EditMode, QueueOrder, SkipMode};
        Self {
            add: match p.add {
                ShareAddMode::Free => AddMode::Free,
                ShareAddMode::Approve => AddMode::Approve,
                ShareAddMode::Limit => AddMode::Limit,
            },
            max_per_guest: p.max_per_guest,
            order: match p.order {
                ShareQueueOrder::FirstCome => QueueOrder::FirstCome,
                ShareQueueOrder::Turns => QueueOrder::Turns,
            },
            skip: match p.skip {
                ShareSkipMode::Nobody => SkipMode::Nobody,
                ShareSkipMode::Everyone => SkipMode::Everyone,
                ShareSkipMode::Vote => SkipMode::Vote,
            },
            playback: p.playback,
            edit: match p.edit {
                ShareEditMode::Own => EditMode::Own,
                ShareEditMode::All => EditMode::All,
                ShareEditMode::None => EditMode::None,
            },
            allow_explicit: p.allow_explicit,
            drop_songs_on_leave: p.drop_songs_on_leave,
        }
    }
}

/// Add freely, turns, skip by vote, no pause / volume, remove own, explicit allowed.
#[uniffi::export]
pub fn share_default_permissions() -> SharePermissions {
    permissions::Permissions::default().into()
}

// ------------------------------------------------------------------- state

/// The song playing now, served by the hub for guests that play along.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareAudio {
    pub queue_id: u64,
    /// Hub path; make it absolute with [`ShareGuest::resolve_url`]. Range requests work.
    pub url: String,
    pub mime: String,
    pub size: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareReady {
    pub ready: u32,
    pub connected: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareState {
    pub now_playing: Option<ShareQueueItem>,
    pub position_ms: u64,
    /// Hub clock when `position_ms` was true; see [`ShareGuest::expected_position_ms`].
    pub clock_ms: u64,
    pub playing: bool,
    /// Set while the hub streams its audio (party mode).
    pub audio: Option<ShareAudio>,
    /// Party mode: next and previous songs' files, to download ahead.
    pub prefetch: Vec<ShareAudio>,
    /// Upcoming songs in play order.
    pub queue: Vec<ShareQueueItem>,
    /// Connected guests (the hub is not listed).
    pub guests: Vec<ShareMember>,
    pub permissions: SharePermissions,
    pub skip_votes: u32,
    pub skip_votes_needed: u32,
    pub volume: Option<f32>,
}

fn audio(a: share::AudioStream) -> ShareAudio {
    ShareAudio { queue_id: a.queue_id, url: a.url, mime: a.mime, size: a.size }
}

impl From<share::State> for ShareState {
    fn from(s: share::State) -> Self {
        Self {
            now_playing: s.now_playing.map(Into::into),
            position_ms: s.position_ms,
            clock_ms: s.clock_ms,
            playing: s.playing,
            audio: s.audio.map(audio),
            prefetch: s.prefetch.into_iter().map(audio).collect(),
            queue: items(s.queue),
            guests: conv(s.guests),
            permissions: s.permissions.into(),
            skip_votes: s.skip_votes,
            skip_votes_needed: s.skip_votes_needed,
            volume: s.volume,
        }
    }
}

/// What to put in the QR code. `join_uri` is the QR payload
/// (`homefi://join?...`).
#[derive(Debug, Clone, uniffi::Record)]
pub struct HubInfo {
    pub host: String,
    pub port: u16,
    pub session_name: String,
    pub code: String,
    pub join_uri: String,
}

impl From<share::JoinInfo> for HubInfo {
    fn from(j: share::JoinInfo) -> Self {
        Self { join_uri: j.to_uri(), host: j.host, port: j.port, session_name: j.session_name, code: j.code }
    }
}

/// Reads a scanned QR code; `None` when it is not a HomeFi join code.
#[uniffi::export]
pub fn parse_join_uri(uri: String) -> Option<HubInfo> {
    share::JoinInfo::parse(&uri).map(Into::into)
}

// --------------------------------------------------------------------- hub

#[derive(Debug, Clone, uniffi::Record)]
pub struct HubSettings {
    /// "Música de José"
    pub session_name: String,
    /// The host as a person (shown on their own songs).
    pub host_name: String,
    pub host_color: String,
    /// Empty picks a random 4-digit code (returned in [`HubInfo`]).
    pub code: String,
    /// 0 picks a free port; the default is 47800.
    pub port: u16,
    /// Address guests reach the hub at (the Wi-Fi Direct group owner /
    /// hotspot address); `None` guesses the LAN address.
    pub advertise_host: Option<String>,
    pub permissions: SharePermissions,
    /// Serve the playing song to guests that play along (party mode).
    pub stream_audio: bool,
}

/// Something the hub's player must do. `Queue`: the upcoming list after the
/// current song must become exactly `items`; report the `queue_id` back in
/// [`ShareHub::set_now_playing`] when one starts.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum HubCommand {
    Skip,
    Toggle,
    Volume { value: f32 },
    Queue { items: Vec<ShareQueueItem> },
}

/// For discreet notices on the hub.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum HubEvent {
    GuestJoined {
        guest: ShareMember,
    },
    GuestLeft {
        guest: ShareMember,
    },
    Added {
        item: ShareQueueItem,
    },
    /// Answer with [`ShareHub::approve`] / [`ShareHub::reject`].
    ApprovalRequested {
        item: ShareQueueItem,
    },
}

/// The hub app's side. `search`, `browse` and `cover` run on a background
/// thread and may block; `on_command` and `on_event` are called from the hub's
/// network threads and should just post to the app's own thread.
#[uniffi::export(callback_interface)]
pub trait HubHostCallback: Send + Sync {
    /// `None` tells the guest the search failed.
    fn search(&self, q: String) -> Option<ShareSearchResults>;
    fn browse(&self, kind: ShareBrowseKind, id: String) -> Option<SharePage>;
    /// JPEG / PNG / WebP bytes for `GET /cover/<id>`.
    fn cover(&self, id: String) -> Option<Vec<u8>>;
    fn on_command(&self, command: HubCommand);
    fn on_event(&self, event: HubEvent);
}

struct CallbackHost(Arc<dyn HubHostCallback>);

/// Runs a (possibly blocking) Kotlin callback off the async threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok().flatten()
}

impl share::HubHost for CallbackHost {
    fn search(&self, q: String) -> BoxFuture<'_, homefi_core::Result<share::SearchResults>> {
        let cb = self.0.clone();
        async move {
            blocking(move || cb.search(q))
                .await
                .map(Into::into)
                .ok_or_else(|| homefi_core::Error::Share("the hub could not search".into()))
        }
        .boxed()
    }

    fn browse(&self, kind: share::BrowseKind, id: String) -> BoxFuture<'_, homefi_core::Result<share::Page>> {
        let cb = self.0.clone();
        async move {
            blocking(move || cb.browse(kind.into(), id))
                .await
                .map(Into::into)
                .ok_or_else(|| homefi_core::Error::Share("the hub could not open it".into()))
        }
        .boxed()
    }

    fn cover(&self, id: String) -> BoxFuture<'_, Option<share::CoverImage>> {
        let cb = self.0.clone();
        async move { blocking(move || cb.cover(id)).await.map(share::CoverImage::new) }.boxed()
    }

    fn command(&self, command: HostCommand) {
        self.0.on_command(match command {
            HostCommand::Skip => HubCommand::Skip,
            HostCommand::Toggle => HubCommand::Toggle,
            HostCommand::Volume(value) => HubCommand::Volume { value },
            HostCommand::Queue(list) => HubCommand::Queue { items: items(list) },
        });
    }

    fn event(&self, event: share::HubEvent) {
        self.0.on_event(match event {
            share::HubEvent::GuestJoined(m) => HubEvent::GuestJoined { guest: m.into() },
            share::HubEvent::GuestLeft(m) => HubEvent::GuestLeft { guest: m.into() },
            share::HubEvent::Added(i) => HubEvent::Added { item: i.into() },
            share::HubEvent::ApprovalRequested(i) => HubEvent::ApprovalRequested { item: i.into() },
        });
    }
}

/// Starts sharing: guests can join at [`ShareHub::info`] until
/// [`ShareHub::stop`] (or until the object is released).
#[uniffi::export(async_runtime = "tokio")]
pub async fn start_hub(settings: HubSettings, host: Box<dyn HubHostCallback>) -> Result<Arc<ShareHub>> {
    let mut config = HubConfig::new(settings.session_name, settings.host_name);
    config.host_color = settings.host_color;
    config.code = settings.code;
    config.bind = SocketAddr::from(([0, 0, 0, 0], settings.port));
    config.permissions = settings.permissions.into();
    config.stream_audio = settings.stream_audio;
    let hub = Hub::start(config, Arc::new(CallbackHost(Arc::from(host)))).await?;
    let host = settings
        .advertise_host
        .filter(|h| !h.trim().is_empty())
        .or_else(|| share::local_ip_guess().map(|ip| ip.to_string()))
        .unwrap_or_else(|| "127.0.0.1".into());
    let info = hub.join_info(&host).into();
    Ok(Arc::new(ShareHub { hub, info }))
}

#[derive(uniffi::Object)]
pub struct ShareHub {
    hub: Hub,
    info: HubInfo,
}

#[uniffi::export(async_runtime = "tokio")]
impl ShareHub {
    pub fn info(&self) -> HubInfo {
        self.info.clone()
    }

    pub fn state(&self) -> ShareState {
        self.hub.state().into()
    }

    pub fn guests(&self) -> Vec<ShareMember> {
        conv(self.hub.guests())
    }

    pub fn set_permissions(&self, permissions: SharePermissions) {
        self.hub.set_permissions(permissions.into());
    }

    /// `ban` keeps the guest's device out for the rest of the session.
    pub fn kick(&self, guest_id: String, ban: bool) -> bool {
        self.hub.kick(&guest_id, ban)
    }

    /// The host adds a song (it takes turns like a guest).
    pub fn add(&self, track: ShareTrack, next: bool) -> ShareQueueItem {
        self.hub.add(track.into(), next).into()
    }

    pub fn remove(&self, queue_id: u64) -> bool {
        self.hub.remove(queue_id)
    }

    pub fn move_item(&self, queue_id: u64, to: u32) -> bool {
        self.hub.move_item(queue_id, to as usize)
    }

    pub fn pending(&self) -> Vec<ShareQueueItem> {
        items(self.hub.pending())
    }

    pub fn approve(&self, queue_id: u64) -> bool {
        self.hub.approve(queue_id)
    }

    pub fn reject(&self, queue_id: u64) -> bool {
        self.hub.reject(queue_id)
    }

    /// The player's current song: `queue_id` when it came from a
    /// [`HubCommand::Queue`] item, `track: None` when nothing plays.
    pub fn set_now_playing(&self, queue_id: Option<u64>, track: Option<ShareTrack>, position_ms: u64, playing: bool) {
        self.hub.set_now_playing(queue_id, track.map(Into::into), position_ms, playing);
    }

    /// Call on play / pause / seek (guests interpolate in between).
    pub fn set_position(&self, position_ms: u64, playing: bool) {
        self.hub.set_position(position_ms, playing);
    }

    /// After the host reordered or edited the player's queue by hand.
    pub fn set_queue(&self, entries: Vec<ShareQueueEntry>) {
        self.hub.set_queue(entries.into_iter().map(|e| (e.queue_id, e.track.into())).collect());
    }

    pub fn set_volume(&self, volume: Option<f32>) {
        self.hub.set_volume(volume);
    }

    /// The file of the song playing now, for guests that play along. Call it
    /// right after [`Self::set_now_playing`]; `None` when there is no local file.
    pub fn set_audio(&self, path: Option<String>) {
        self.hub.set_audio(path.map(std::path::PathBuf::from));
    }

    /// The file of the song that comes next (`queue_id` from the state's queue), so
    /// guests download it ahead; `None` clears it.
    pub fn set_next_audio(&self, queue_id: u64, path: Option<String>) {
        self.hub.set_next_audio(queue_id, path.map(std::path::PathBuf::from));
    }

    /// Party mode: start the song from 0 in `delay_ms`, everyone at once; returns the
    /// hub time to start the host's own player at (compare with [`Self::now_ms`]).
    pub fn schedule_start(&self, delay_ms: u64) -> u64 {
        self.hub.schedule_start(delay_ms)
    }

    /// The music's levels (0..=255) for guests' visualizers, ~15 times a second while playing.
    pub fn send_levels(&self, left: u8, right: u8, bands: Vec<u8>) {
        self.hub.send_levels(left, right, bands);
    }

    /// How many guests have `queue_id` ready to start, out of how many connected.
    pub fn ready_guests(&self, queue_id: u64) -> ShareReady {
        let (ready, connected) = self.hub.ready_guests(queue_id);
        ShareReady { ready: ready as u32, connected: connected as u32 }
    }

    /// Party mode on / off while sharing.
    pub fn set_streaming(&self, on: bool) {
        self.hub.set_streaming(on);
    }

    /// The hub's clock (milliseconds since it started).
    pub fn now_ms(&self) -> u64 {
        self.hub.now_ms()
    }

    /// A notice for one guest, or everyone with `None`.
    pub fn notify(&self, guest_id: Option<String>, text: String) {
        self.hub.notify(guest_id.as_deref(), &text);
    }

    /// Disconnects everyone and frees the port.
    pub async fn stop(&self) {
        self.hub.stop().await;
    }
}

/// What a guest's player should do to stay with the hub ([`share_sync_action`]).
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ShareSyncAction {
    /// Normal speed.
    Keep,
    /// Play at this speed for a while (1.0 = normal, within ±4 %).
    Rate { rate: f32 },
    /// Jump to this position.
    Seek { position_ms: u64 },
}

/// `target_ms` = where the player should be (the hub's position plus this
/// speaker's delay), `actual_ms` = where it is.
#[uniffi::export]
pub fn share_sync_action(target_ms: u64, actual_ms: u64) -> ShareSyncAction {
    match share::sync::sync_action(target_ms, actual_ms) {
        share::sync::SyncAction::Keep => ShareSyncAction::Keep,
        share::sync::SyncAction::Rate(rate) => ShareSyncAction::Rate { rate },
        share::sync::SyncAction::Seek(position_ms) => ShareSyncAction::Seek { position_ms },
    }
}

// ------------------------------------------------------------------- guest

// UniFFI enums cannot box their fields, and each event crosses the FFI once.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, uniffi::Enum)]
pub enum GuestEvent {
    State {
        state: ShareState,
    },
    Results {
        results: ShareSearchResults,
    },
    Page {
        page: SharePage,
    },
    Notice {
        text: String,
    },
    /// The last request was refused.
    Denied {
        reason: String,
    },
    /// The hub's music levels right now (0..=255), for the visualizer and VU meters.
    Levels {
        left: u8,
        right: u8,
        bands: Vec<u8>,
    },
    /// Always last. `reason` is the hub's text (kicked, hub stopped) when it
    /// closed the connection.
    Disconnected {
        reason: Option<String>,
    },
}

/// Called on a background thread; post to the UI thread.
#[uniffi::export(callback_interface)]
pub trait GuestListener: Send + Sync {
    fn on_event(&self, event: GuestEvent);
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GuestWelcome {
    pub guest_id: String,
    pub session_name: String,
    pub permissions: SharePermissions,
    pub state: ShareState,
}

/// Joins the hub at `host:port`. Fails with the hub's reason ("Wrong join
/// code", ...). Events go to `listener` until `Disconnected`.
#[uniffi::export(async_runtime = "tokio")]
pub async fn join_hub(
    host: String,
    port: u16,
    name: String,
    color: String,
    code: String,
    listener: Box<dyn GuestListener>,
) -> Result<Arc<ShareGuest>> {
    let (client, welcome, mut rx) = GuestClient::connect(&host, port, &name, &color, &code).await?;
    let pump = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let event = match event {
                share::GuestEvent::Message(msg) => match msg {
                    HubMessage::State(s) => GuestEvent::State { state: s.into() },
                    HubMessage::Results(r) => GuestEvent::Results { results: r.into() },
                    HubMessage::Page(p) => GuestEvent::Page { page: p.into() },
                    HubMessage::Notice { text } => GuestEvent::Notice { text },
                    HubMessage::Denied { reason } => GuestEvent::Denied { reason },
                    HubMessage::Levels { l, r, b } => GuestEvent::Levels { left: l, right: r, bands: b },
                    HubMessage::Welcome { .. } | HubMessage::Clock { .. } | HubMessage::Unknown => continue,
                },
                share::GuestEvent::Disconnected { reason } => GuestEvent::Disconnected { reason },
            };
            listener.on_event(event);
        }
    });
    let welcome = GuestWelcome {
        guest_id: welcome.guest_id,
        session_name: welcome.session_name,
        permissions: welcome.permissions.into(),
        state: welcome.state.into(),
    };
    Ok(Arc::new(ShareGuest { client, welcome, pump }))
}

#[derive(uniffi::Object)]
pub struct ShareGuest {
    client: GuestClient,
    welcome: GuestWelcome,
    pump: JoinHandle<()>,
}

impl ShareGuest {
    fn send(&self, msg: GuestMessage) -> Result<()> {
        self.client.send(msg).map_err(|e| share_error(e.to_string()))
    }
}

/// Requests are fire-and-forget: answers and refusals arrive at the listener.
/// They fail only when the connection is gone.
#[uniffi::export]
impl ShareGuest {
    pub fn welcome(&self) -> GuestWelcome {
        self.welcome.clone()
    }

    pub fn guest_id(&self) -> String {
        self.client.guest_id().to_owned()
    }

    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    /// The hub's clock now, once the first clock sample came back.
    pub fn hub_now_ms(&self) -> Option<u64> {
        self.client.hub_now_ms()
    }

    /// Round trip of the clock sample in use (the sync is about half this precise).
    pub fn clock_round_trip_ms(&self) -> Option<u64> {
        self.client.clock_round_trip_ms()
    }

    /// Where the hub's song is right now, from a [`ShareState`]'s
    /// `position_ms`, `clock_ms` and `playing`.
    pub fn expected_position_ms(&self, position_ms: u64, clock_ms: u64, playing: bool) -> u64 {
        match self.client.hub_now_ms() {
            Some(now) => share::sync::expected_position(position_ms, clock_ms, playing, now),
            None => position_ms,
        }
    }

    /// Makes hub paths (`/cover/<id>`) absolute; other URLs are returned as-is.
    pub fn resolve_url(&self, url: String) -> String {
        self.client.resolve_url(&url)
    }

    pub fn search(&self, q: String) -> Result<()> {
        self.send(GuestMessage::Search { q })
    }

    pub fn browse(&self, kind: ShareBrowseKind, id: String) -> Result<()> {
        self.send(GuestMessage::Browse { kind: kind.into(), id })
    }

    pub fn add(&self, track: ShareTrack, next: bool) -> Result<()> {
        self.send(GuestMessage::Add { item: track.into(), next })
    }

    pub fn remove(&self, queue_id: u64) -> Result<()> {
        self.send(GuestMessage::Remove { queue_id })
    }

    pub fn move_item(&self, queue_id: u64, to: u32) -> Result<()> {
        self.send(GuestMessage::Move { queue_id, to: to as usize })
    }

    pub fn skip(&self) -> Result<()> {
        self.send(GuestMessage::Skip)
    }

    /// Play this queued song now (needs pause/volume control or "everyone can skip").
    pub fn play_now(&self, queue_id: u64) -> Result<()> {
        self.send(GuestMessage::Play { queue_id })
    }

    pub fn vote_skip(&self) -> Result<()> {
        self.send(GuestMessage::VoteSkip)
    }

    pub fn toggle(&self) -> Result<()> {
        self.send(GuestMessage::Toggle)
    }

    /// Party mode: this phone has `queue_id` loaded and can start it at once.
    pub fn ready(&self, queue_id: u64) -> Result<()> {
        self.send(GuestMessage::Ready { queue_id })
    }

    /// 0.0..=1.0
    pub fn volume(&self, value: f32) -> Result<()> {
        self.send(GuestMessage::Volume { value })
    }

    /// Leaves the session; the listener gets `Disconnected`.
    pub fn disconnect(&self) {
        self.client.close();
    }
}

impl Drop for ShareGuest {
    fn drop(&mut self) {
        // The client's own drop closes the socket; the pump ends with it, but
        // stop calling into a listener whose owner released us.
        self.pump.abort();
    }
}
