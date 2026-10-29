//! UniFFI bindings of `homefi-core` for the Android app.
//!
//! The Kotlin side (`com.perseusoft.homefi.core`) is generated from this
//! crate; keep the exported surface small and Kotlin-friendly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use homefi_core::api::{all_pages, pages_up_to, ArtistAlbumsFilter};
use homefi_core::catalog;
use homefi_core::config::{Config, MediaFilter};
use homefi_core::download::{self, DownloadOptions, Downloader, Observer};
use homefi_core::library;
use homefi_core::m3u;
use homefi_core::metadata::Cover;
use homefi_core::models::{Album, ListedItem, MediaItem, Page, Playlist, PlaylistEntry};
use homefi_core::quality::TrackQuality;
use homefi_core::resource::{Resource, ResourceKind};
use homefi_core::session::{LoginPoll, Session};
use homefi_core::stream_cache;

uniffi::setup_scaffolding!();

mod ai;
mod eq;
mod runtime;
mod share;
mod update;
mod versions;

pub use update::AppRelease;
pub use versions::{AlbumEditionPref, AlbumExplicitPref, AlbumSpatialPref, AlbumVersionHit, AlbumVersionPrefs};

const LIBRARY_CACHE: &str = "library_cache.json";

/// Exposed as a "flat" error: Kotlin gets `HomefiException.<Variant>` whose
/// `message` is the text below.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum HomefiError {
    #[error("{message}")]
    NotLoggedIn { message: String },
    #[error("{message}")]
    InvalidLink { message: String },
    #[error("{message}")]
    Failed { message: String },
}

impl From<homefi_core::Error> for HomefiError {
    /// The message is for the screen (plain words in the app's language, see
    /// [`set_language`]); the technical text goes to the log.
    fn from(e: homefi_core::Error) -> Self {
        tracing::warn!("{e}");
        let message = e.humane();
        match e {
            homefi_core::Error::NotLoggedIn(_) => Self::NotLoggedIn { message },
            homefi_core::Error::Resource(_) => Self::InvalidLink { message },
            _ => Self::Failed { message },
        }
    }
}

/// Language of the error messages, from the device locale (`es`, `es-MX`, `en-US`...).
#[uniffi::export]
pub fn set_language(code: String) {
    homefi_core::humane::set_language(&code);
}

type Result<T> = std::result::Result<T, HomefiError>;

#[derive(uniffi::Record)]
pub struct DeviceLogin {
    pub device_code: String,
    pub user_code: String,
    pub url: String,
    pub interval_secs: u64,
    pub expires_in_secs: u64,
}

#[derive(uniffi::Enum)]
pub enum LoginState {
    Pending,
    Success,
    Expired,
}

#[derive(uniffi::Record)]
pub struct Account {
    pub user_id: String,
    pub country_code: String,
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ItemKind {
    Track,
    Video,
    Album,
    Playlist,
    Artist,
    Mix,
}

impl From<ResourceKind> for ItemKind {
    fn from(k: ResourceKind) -> Self {
        match k {
            ResourceKind::Track => Self::Track,
            ResourceKind::Video => Self::Video,
            ResourceKind::Album => Self::Album,
            ResourceKind::Playlist => Self::Playlist,
            ResourceKind::Artist => Self::Artist,
            ResourceKind::Mix => Self::Mix,
        }
    }
}

impl From<ItemKind> for ResourceKind {
    fn from(k: ItemKind) -> Self {
        match k {
            ItemKind::Track => Self::Track,
            ItemKind::Video => Self::Video,
            ItemKind::Album => Self::Album,
            ItemKind::Playlist => Self::Playlist,
            ItemKind::Artist => Self::Artist,
            ItemKind::Mix => Self::Mix,
        }
    }
}

/// A parsed Tidal link.
#[derive(uniffi::Record)]
pub struct Link {
    pub kind: ItemKind,
    pub id: String,
}

/// Parses a Tidal URL or `type/id`.
#[uniffi::export]
pub fn parse_link(link: String) -> Result<Link> {
    let r = Resource::parse(&link)?;
    Ok(Link { kind: r.kind.into(), id: r.id })
}

/// A local audio file in the music library.
#[derive(uniffi::Record)]
pub struct LibraryTrack {
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
    /// `FLAC`, `MP3`, `WAV`, `AAC`, `ALAC`, `Vorbis`, `Opus`, ...
    pub format: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
    /// `FLAC · 24-bit / 96 kHz`, `MP3 · 320 kbps`, ...
    pub quality: String,
}

impl From<library::TrackInfo> for LibraryTrack {
    fn from(t: library::TrackInfo) -> Self {
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
            channels: t.channels,
            bitrate_kbps: t.bitrate_kbps,
            lossless: t.lossless,
        }
    }
}

/// Reads tags and audio properties of one file.
#[uniffi::export]
pub fn read_track_info(path: String) -> Result<LibraryTrack> {
    Ok(library::read_track(Path::new(&path))?.into())
}

/// Embedded cover art, or a `cover.jpg`-style image next to the file.
#[uniffi::export]
pub fn read_cover(path: String) -> Option<Vec<u8>> {
    library::read_cover(Path::new(&path))
}

/// File extensions the player supports.
#[uniffi::export]
pub fn playable_extensions() -> Vec<String> {
    library::PLAYABLE_EXTENSIONS.iter().map(|e| e.to_string()).collect()
}

#[uniffi::export]
pub fn core_version() -> String {
    homefi_core::VERSION.to_owned()
}

/// A row in Tidal lists (search, artist / album / playlist pages, favorites).
/// For tracks, `id` is the Tidal track id: `track/<id>` plays or downloads it.
#[derive(uniffi::Record)]
pub struct SearchHit {
    pub kind: ItemKind,
    pub id: String,
    pub title: String,
    /// `Artist · Album` for tracks, `Artist · Year` for albums, ...
    pub subtitle: String,
    pub cover_url: Option<String>,
    pub explicit: bool,
    /// `Hi-Res`, `Lossless`, ... when known.
    pub quality: Option<String>,
    /// Main artist (tracks, albums, videos; the name itself for artists).
    #[uniffi(default = "")]
    pub artist: String,
    /// Album title (tracks and albums).
    #[uniffi(default = "")]
    pub album: String,
    /// Tracks only.
    #[uniffi(default = None)]
    pub duration_ms: Option<u64>,
    /// Tracks only, upper case: matches local files downloaded before.
    #[uniffi(default = None)]
    pub isrc: Option<String>,
    /// Albums shown as one card: every release of it (this one is `primary`).
    /// Empty for everything else, and for albums Tidal lists once.
    #[uniffi(default = [])]
    pub versions: Vec<AlbumVersionHit>,
}

#[derive(uniffi::Record)]
pub struct SearchResults {
    pub top_hit: Option<SearchHit>,
    pub tracks: Vec<SearchHit>,
    pub albums: Vec<SearchHit>,
    pub artists: Vec<SearchHit>,
    pub playlists: Vec<SearchHit>,
    pub videos: Vec<SearchHit>,
}

impl From<catalog::Hit> for SearchHit {
    fn from(h: catalog::Hit) -> Self {
        Self {
            kind: h.kind.into(),
            id: h.id,
            title: h.title,
            subtitle: h.subtitle,
            cover_url: h.cover_url,
            explicit: h.explicit,
            quality: h.quality,
            artist: h.artist,
            album: h.album,
            duration_ms: h.duration_ms,
            isrc: h.isrc,
            versions: h.versions.into_iter().map(AlbumVersionHit::from).collect(),
        }
    }
}

fn hits(list: Vec<catalog::Hit>) -> Vec<SearchHit> {
    list.into_iter().map(SearchHit::from).collect()
}

/// Tracks of an album / playlist listing (videos are left out).
fn track_hits(items: Vec<ListedItem>) -> Vec<SearchHit> {
    items
        .into_iter()
        .filter_map(|l| match l.item {
            MediaItem::Track(t) => Some(catalog::track_hit(&t).into()),
            MediaItem::Video(_) => None,
        })
        .collect()
}

/// Header image of a detail page (thumbnails in [`SearchHit`] are 320 px).
fn image_url(uid: Option<&str>, size: u32) -> Option<String> {
    uid.filter(|u| !u.is_empty()).map(|u| Cover::new(u, size).url)
}

/// Top tracks shown on an artist page.
const ARTIST_TOP_TRACKS: usize = 20;
/// Albums and EPs / singles shown on an artist page (each).
const ARTIST_ALBUMS: usize = 50;

#[derive(uniffi::Record)]
pub struct ArtistPage {
    pub id: String,
    pub name: String,
    /// 750 px picture.
    pub picture_url: Option<String>,
    /// Most popular first (up to 20).
    pub top_tracks: Vec<SearchHit>,
    /// Up to 50, as Tidal orders them.
    pub albums: Vec<SearchHit>,
    /// Up to 50.
    pub eps_and_singles: Vec<SearchHit>,
}

#[derive(uniffi::Record)]
pub struct AlbumPage {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    /// 640 px cover.
    pub cover_url: Option<String>,
    pub year: Option<u32>,
    /// `YYYY-MM-DD` when known.
    pub release_date: Option<String>,
    /// `Hi-Res`, `Lossless`, `Dolby Atmos` when known.
    pub quality: Option<String>,
    pub explicit: bool,
    pub number_of_tracks: u32,
    pub duration_secs: u64,
    pub copyright: Option<String>,
    /// Every track, in album order (only the first page from `album_page_first`).
    pub tracks: Vec<SearchHit>,
    /// Where the next page of `tracks` starts; `None` when `tracks` is complete.
    #[uniffi(default = None)]
    pub tracks_next_offset: Option<u32>,
}

#[derive(uniffi::Record)]
pub struct PlaylistPage {
    pub uuid: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub number_of_tracks: u32,
    pub duration_secs: u64,
    /// Created by the logged-in user.
    pub owned: bool,
    /// Every track, in playlist order (videos are left out; only the first
    /// page from `playlist_page_first`).
    pub tracks: Vec<SearchHit>,
    /// Where the next page of `tracks` starts; `None` when `tracks` is complete.
    #[uniffi(default = None)]
    pub tracks_next_offset: Option<u32>,
}

/// One page of a Tidal list, for lists that load while scrolling.
#[derive(uniffi::Record)]
pub struct HitPage {
    /// Album / playlist / mix pages leave videos out, so this may hold fewer
    /// rows than the page size.
    pub items: Vec<SearchHit>,
    /// Items in the whole list, as Tidal counts them (videos included).
    pub total: u32,
    /// Offset of the next page; `None` on the last one.
    pub next_offset: Option<u32>,
}

fn page_next<T>(page: &Page<T>) -> Option<u32> {
    page.next_offset().and_then(|o| u32::try_from(o).ok())
}

fn hit_page<T>(page: &Page<T>, items: Vec<SearchHit>) -> HitPage {
    let total = u32::try_from(page.total_number_of_items.max(0)).unwrap_or(u32::MAX);
    HitPage { items, total, next_offset: page_next(page) }
}

/// A page of album / playlist / mix items as track rows.
fn listed_page(mut page: Page<ListedItem>) -> HitPage {
    if page.limit <= 0 {
        // `next_offset` steps by the item count when Tidal sends no limit:
        // count the videos too, before they are left out
        page.limit = i64::try_from(page.items.len()).unwrap_or(0);
    }
    let items = track_hits(std::mem::take(&mut page.items));
    hit_page(&page, items)
}

/// A playlist of the user's own list; the subtitle starts with `Yours` or `Following`.
fn playlist_entry_hit(e: &PlaylistEntry) -> SearchHit {
    let mut hit = catalog::playlist_hit(&e.playlist);
    hit.subtitle = format!("{} · {}", if e.is_own() { "Yours" } else { "Following" }, hit.subtitle);
    hit.into()
}

fn album_page(album: Album, tracks: Vec<SearchHit>, tracks_next_offset: Option<u32>) -> AlbumPage {
    let hit = catalog::album_hit(&album);
    let main = album.artist.as_ref().or(album.artists.first());
    AlbumPage {
        id: album.id.to_string(),
        title: album.title.clone(),
        artist: hit.artist,
        artist_id: main.map(|a| a.id.to_string()),
        cover_url: image_url(album.cover.as_deref(), 640),
        year: album.release_date.as_deref().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()),
        release_date: album.release_date.clone(),
        quality: hit.quality,
        explicit: album.explicit,
        number_of_tracks: album.number_of_tracks.max(0) as u32,
        duration_secs: album.duration.max(0) as u64,
        copyright: album.copyright.clone(),
        tracks,
        tracks_next_offset,
    }
}

fn playlist_page(
    playlist: Playlist,
    user_id: &str,
    tracks: Vec<SearchHit>,
    tracks_next_offset: Option<u32>,
) -> PlaylistPage {
    let hit = catalog::playlist_hit(&playlist);
    PlaylistPage {
        owned: playlist.is_owned_by(user_id),
        cover_url: image_url(playlist.square_image.as_deref(), 750).or(hit.cover_url),
        uuid: playlist.uuid,
        title: playlist.title,
        description: playlist.description.filter(|d| !d.trim().is_empty()),
        number_of_tracks: playlist.number_of_tracks.max(0) as u32,
        duration_secs: playlist.duration.max(0) as u64,
        tracks,
        tracks_next_offset,
    }
}

/// An `.m3u` / `.m3u8` file on the device.
#[derive(uniffi::Record)]
pub struct LocalPlaylist {
    /// File name without the extension.
    pub name: String,
    pub path: String,
    /// Absolute paths, in order; files may be missing.
    pub tracks: Vec<String>,
}

impl From<m3u::LocalPlaylist> for LocalPlaylist {
    fn from(p: m3u::LocalPlaylist) -> Self {
        Self {
            name: p.name,
            path: p.path.to_string_lossy().into_owned(),
            tracks: p.tracks.iter().map(|t| t.to_string_lossy().into_owned()).collect(),
        }
    }
}

/// Finds `.m3u` / `.m3u8` playlists below `roots` (recursively, hidden folders
/// skipped), sorted by name. Entries are resolved to absolute paths.
#[uniffi::export]
pub fn find_playlists(roots: Vec<String>) -> Vec<LocalPlaylist> {
    let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
    m3u::find(&roots).into_iter().map(LocalPlaylist::from).collect()
}

/// Reads one playlist file.
#[uniffi::export]
pub fn read_playlist(path: String) -> Result<LocalPlaylist> {
    Ok(m3u::read(Path::new(&path))?.into())
}

/// Deletes the oldest files in the stream cache `cache_dir` until it fits in
/// `max_bytes`, never touching the paths in `keep` (what the player has
/// queued). Returns the number of bytes freed.
#[uniffi::export(default(keep = []))]
pub fn trim_stream_cache(cache_dir: String, max_bytes: u64, keep: Vec<String>) -> u64 {
    let keep: Vec<PathBuf> = keep.into_iter().map(PathBuf::from).collect();
    stream_cache::trim(Path::new(&cache_dir), max_bytes, &keep)
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum AudioQuality {
    Low,
    Normal,
    High,
    Max,
}

impl From<AudioQuality> for TrackQuality {
    fn from(q: AudioQuality) -> Self {
        match q {
            AudioQuality::Low => Self::Low,
            AudioQuality::Normal => Self::Normal,
            AudioQuality::High => Self::High,
            AudioQuality::Max => Self::Max,
        }
    }
}

#[derive(uniffi::Record)]
pub struct DownloadSettings {
    pub quality: AudioQuality,
    pub skip_existing: bool,
    pub threads: u32,
    pub embed_cover: bool,
    pub embed_lyrics: bool,
    pub include_videos: bool,
    /// Path template; `None` uses `{album.artist}/{album.title}/{item.title}`.
    pub template: Option<String>,
    /// Files are written (and looked up for `skip_existing`) below this directory.
    pub download_dir: String,
}

#[derive(uniffi::Enum)]
pub enum DownloadEvent {
    Queued {
        title: String,
    },
    Started {
        task: u64,
        title: String,
        quality: String,
    },
    Progress {
        task: u64,
        downloaded: u64,
        total: Option<u64>,
    },
    /// `relative_path` is relative to the download directory.
    Finished {
        task: u64,
        title: String,
        quality: String,
        path: String,
        relative_path: String,
    },
    Exists {
        title: String,
        path: String,
        relative_path: String,
    },
    Skipped {
        title: String,
        reason: String,
    },
    Failed {
        context: String,
        message: String,
    },
}

#[uniffi::export(callback_interface)]
pub trait DownloadListener: Send + Sync {
    fn on_event(&self, event: DownloadEvent);
}

#[derive(uniffi::Record)]
pub struct DownloadSummary {
    pub queued: u64,
    pub downloaded: u64,
    pub existing: u64,
    pub skipped: u64,
    pub failed: u64,
}

struct ListenerObserver {
    listener: Box<dyn DownloadListener>,
    base: PathBuf,
}

impl ListenerObserver {
    fn paths(&self, path: &Path) -> (String, String) {
        let relative = path.strip_prefix(&self.base).unwrap_or(path);
        (path.to_string_lossy().into_owned(), relative.to_string_lossy().into_owned())
    }
}

impl Observer for ListenerObserver {
    fn on_event(&self, event: download::Event) {
        use download::Event as E;
        let event = match event {
            E::Queued { title, .. } => DownloadEvent::Queued { title },
            E::Started { task, title, quality, .. } => DownloadEvent::Started { task, title, quality },
            E::Progress { task, downloaded, total } => DownloadEvent::Progress { task, downloaded, total },
            E::Finished { task, title, quality, path, .. } => {
                let (path, relative_path) = self.paths(&path);
                DownloadEvent::Finished { task, title, quality, path, relative_path }
            }
            E::Exists { title, path, .. } => {
                let (path, relative_path) = self.paths(&path);
                DownloadEvent::Exists { title, path, relative_path }
            }
            E::Skipped { title, reason, .. } => DownloadEvent::Skipped { title, reason },
            E::Failed { context, message, .. } => DownloadEvent::Failed { context, message },
            E::ExtraWritten { .. } => return,
        };
        self.listener.on_event(event);
    }
}

/// Entry point used by the Android app.
#[derive(uniffi::Object)]
pub struct HomefiEngine {
    session: Session,
    /// How album lists are grouped ([`Self::set_album_versions`]).
    grouping: std::sync::Mutex<versions::Grouping>,
}

impl HomefiEngine {
    fn grouping(&self) -> versions::Grouping {
        *self.grouping.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// One card per album (its releases in `versions`), as the settings say.
    fn album_cards(&self, albums: &[Album]) -> Vec<SearchHit> {
        let g = self.grouping();
        hits(catalog::album_cards(albums, &g.prefs, g.lang))
    }
}

#[uniffi::export]
impl HomefiEngine {
    /// `app_dir` holds credentials and the API cache (use the app's private
    /// files dir).
    #[uniffi::constructor]
    pub fn new(app_dir: String) -> Result<Arc<Self>> {
        let app_dir = PathBuf::from(app_dir);
        std::fs::create_dir_all(&app_dir).map_err(homefi_core::Error::from)?;
        let config = Config::load(&app_dir.join(homefi_core::paths::CONFIG_FILENAME))?;
        // answered before any download asks (no ffmpeg on phones: quick)
        homefi_core::ffmpeg::warm_up();
        Ok(Arc::new(Self { session: Session::new(app_dir, config), grouping: Default::default() }))
    }

    /// How album lists (artist pages, search, favorites) are grouped from now
    /// on, and the language (`en`, `es`, `es-MX`...) of the version labels.
    pub fn set_album_versions(&self, prefs: AlbumVersionPrefs, language: String) {
        *self.grouping.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            versions::Grouping::new(&prefs, &language);
    }

    /// Every release of an album (itself included) as its artist's page lists
    /// them, for the album page's "Other versions".
    pub async fn album_versions(self: Arc<Self>, id: String) -> Result<Vec<AlbumVersionHit>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let g = self.grouping();
            let album = api.album(&id).await?;
            let list = catalog::album_versions(&api, &album, &g.prefs, g.lang).await?;
            Ok(list.into_iter().map(AlbumVersionHit::from).collect())
        })
        .await
    }

    pub fn account(&self) -> Option<Account> {
        let data = self.session.auth_data().ok().filter(|d| d.is_logged_in())?;
        Some(Account { user_id: data.user_id.unwrap_or_default(), country_code: data.country_code.unwrap_or_default() })
    }

    pub async fn start_login(self: Arc<Self>) -> Result<DeviceLogin> {
        crate::runtime::run(async move {
            let d = self.session.start_login().await?;
            Ok(DeviceLogin {
                url: d.url(),
                device_code: d.device_code,
                user_code: d.user_code,
                interval_secs: d.interval.max(1) as u64,
                expires_in_secs: d.expires_in.max(1) as u64,
            })
        })
        .await
    }

    pub async fn poll_login(self: Arc<Self>, device_code: String) -> Result<LoginState> {
        crate::runtime::run(async move {
            Ok(match self.session.poll_login(&device_code).await? {
                LoginPoll::Pending => LoginState::Pending,
                LoginPoll::Success(_) => LoginState::Success,
                LoginPoll::Expired => LoginState::Expired,
            })
        })
        .await
    }

    /// Logs out remotely and always clears the local credentials.
    pub async fn logout(self: Arc<Self>) -> Result<()> {
        crate::runtime::run(async move {
            if let Err(e) = self.session.logout(true).await {
                tracing::warn!("remote logout failed: {e}");
            }
            Ok(())
        })
        .await
    }

    pub async fn search(self: Arc<Self>, query: String) -> Result<SearchResults> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let g = self.grouping();
            let r = catalog::SearchResults::grouped(&api.search(&query).await?, &g.prefs, g.lang);
            Ok(SearchResults {
                top_hit: r.top_hit.map(SearchHit::from),
                tracks: hits(r.tracks),
                albums: hits(r.albums),
                artists: hits(r.artists),
                playlists: hits(r.playlists),
                videos: hits(r.videos),
            })
        })
        .await
    }

    // ------------------------------------------------------------- browsing

    /// Artist header, top tracks, albums and EPs / singles.
    pub async fn artist_page(self: Arc<Self>, id: String) -> Result<ArtistPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let (artist, top, albums, singles) = tokio::try_join!(
                api.artist(&id),
                pages_up_to(ARTIST_TOP_TRACKS, |o| api.artist_top_tracks(&id, o)),
                pages_up_to(ARTIST_ALBUMS, |o| api.artist_albums(&id, o, ArtistAlbumsFilter::Albums)),
                pages_up_to(ARTIST_ALBUMS, |o| api.artist_albums(&id, o, ArtistAlbumsFilter::EpsAndSingles)),
            )?;
            Ok(ArtistPage {
                id: artist.id.to_string(),
                picture_url: image_url(artist.picture.as_deref(), 750),
                name: artist.name,
                top_tracks: top.iter().map(|t| catalog::track_hit(t).into()).collect(),
                albums: self.album_cards(&albums),
                eps_and_singles: self.album_cards(&singles),
            })
        })
        .await
    }

    /// Album info and all of its tracks.
    pub async fn album_page(self: Arc<Self>, id: String) -> Result<AlbumPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let (album, items) = tokio::try_join!(api.album(&id), all_pages(|o| api.album_items(&id, o)))?;
            Ok(album_page(album, track_hits(items), None))
        })
        .await
    }

    /// Album info and only the first page of its tracks; `tracks_next_offset`
    /// tells where [`Self::album_items_page`] continues.
    pub async fn album_page_first(self: Arc<Self>, id: String) -> Result<AlbumPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let (album, page) = tokio::try_join!(api.album(&id), api.album_items(&id, 0))?;
            let next = page_next(&page);
            Ok(album_page(album, track_hits(page.items), next))
        })
        .await
    }

    /// Lyrics of a Tidal track (synced when Tidal has them); None when there are none.
    pub async fn track_lyrics(self: Arc<Self>, track_id: String) -> Result<Option<SongLyrics>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let id: i64 =
                track_id.parse().map_err(|_| homefi_core::Error::Resource(format!("not a track id: {track_id}")))?;
            Ok(homefi_core::lyrics::tidal(&api, id).await?.map(SongLyrics::from))
        })
        .await
    }

    /// Lyrics of a local file: its own (`.lrc` or tag) or, when it has none and the
    /// user is signed in, Tidal's for the same recording (found by its ISRC).
    pub async fn file_lyrics(self: Arc<Self>, path: String) -> Result<Option<SongLyrics>> {
        crate::runtime::run(async move {
            let api = self.session.api().ok();
            Ok(homefi_core::lyrics::for_file(api.as_ref(), std::path::Path::new(&path)).await?.map(SongLyrics::from))
        })
        .await
    }

    /// One page of an album's tracks, starting at `offset`.
    pub async fn album_items_page(self: Arc<Self>, id: String, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            Ok(listed_page(api.album_items(&id, i64::from(offset)).await?))
        })
        .await
    }

    /// Playlist info and all of its tracks.
    pub async fn playlist_page(self: Arc<Self>, uuid: String) -> Result<PlaylistPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let (playlist, items) = tokio::try_join!(api.playlist(&uuid), all_pages(|o| api.playlist_items(&uuid, o)))?;
            Ok(playlist_page(playlist, api.user_id(), track_hits(items), None))
        })
        .await
    }

    /// Playlist info and only the first page of its tracks; `tracks_next_offset`
    /// tells where [`Self::playlist_items_page`] continues.
    pub async fn playlist_page_first(self: Arc<Self>, uuid: String) -> Result<PlaylistPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let (playlist, page) = tokio::try_join!(api.playlist(&uuid), api.playlist_items(&uuid, 0))?;
            let next = page_next(&page);
            Ok(playlist_page(playlist, api.user_id(), track_hits(page.items), next))
        })
        .await
    }

    /// One page of a playlist's tracks, starting at `offset`.
    pub async fn playlist_items_page(self: Arc<Self>, uuid: String, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            Ok(listed_page(api.playlist_items(&uuid, i64::from(offset)).await?))
        })
        .await
    }

    /// One page of a mix's tracks, starting at `offset`.
    pub async fn mix_items_page(self: Arc<Self>, id: String, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            Ok(listed_page(api.mix_items(&id, i64::from(offset)).await?))
        })
        .await
    }

    /// Playlists the user created and the ones they follow, newest first. The
    /// subtitle starts with `Yours` or `Following`.
    pub async fn my_playlists(self: Arc<Self>) -> Result<Vec<SearchHit>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let entries = all_pages(|o| api.playlists_and_favorites(o)).await?;
            Ok(entries.iter().map(playlist_entry_hit).collect())
        })
        .await
    }

    /// Every favorite track, newest first.
    pub async fn favorite_tracks(self: Arc<Self>) -> Result<Vec<SearchHit>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let list = all_pages(|o| api.favorite_tracks(o)).await?;
            Ok(list.iter().map(|f| catalog::track_hit(&f.item).into()).collect())
        })
        .await
    }

    /// Every favorite album, newest first.
    pub async fn favorite_albums(self: Arc<Self>) -> Result<Vec<SearchHit>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let list = all_pages(|o| api.favorite_albums(o)).await?;
            Ok(self.album_cards(&list.into_iter().map(|f| f.item).collect::<Vec<_>>()))
        })
        .await
    }

    /// Every favorite artist, newest first.
    pub async fn favorite_artists(self: Arc<Self>) -> Result<Vec<SearchHit>> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let list = all_pages(|o| api.favorite_artists(o)).await?;
            Ok(list.iter().map(|f| catalog::artist_hit(&f.item).into()).collect())
        })
        .await
    }

    /// One page of [`Self::my_playlists`], starting at `offset`.
    pub async fn my_playlists_page(self: Arc<Self>, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let page = api.playlists_and_favorites(i64::from(offset)).await?;
            Ok(hit_page(&page, page.items.iter().map(playlist_entry_hit).collect()))
        })
        .await
    }

    /// One page of favorite tracks (newest first), starting at `offset`.
    pub async fn favorite_tracks_page(self: Arc<Self>, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let page = api.favorite_tracks(i64::from(offset)).await?;
            Ok(hit_page(&page, page.items.iter().map(|f| catalog::track_hit(&f.item).into()).collect()))
        })
        .await
    }

    /// One page of favorite albums (newest first), starting at `offset`.
    pub async fn favorite_albums_page(self: Arc<Self>, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let page = api.favorite_albums(i64::from(offset)).await?;
            // grouped page by page: versions saved far apart stay two cards
            let cards = self.album_cards(&page.items.iter().map(|f| f.item.clone()).collect::<Vec<_>>());
            Ok(hit_page(&page, cards))
        })
        .await
    }

    /// One page of favorite artists (newest first), starting at `offset`.
    pub async fn favorite_artists_page(self: Arc<Self>, offset: u32) -> Result<HitPage> {
        crate::runtime::run(async move {
            let api = self.session.api()?;
            let page = api.favorite_artists(i64::from(offset)).await?;
            Ok(hit_page(&page, page.items.iter().map(|f| catalog::artist_hit(&f.item).into()).collect()))
        })
        .await
    }

    // ------------------------------------------------------------- streaming

    /// Fetches Tidal track `track_id` into `cache_dir` (as `<id>.flac`, or
    /// `.m4a` for lossy qualities) and returns the file path, so the player can
    /// play a local file. A cached copy is reused; concurrent calls for the
    /// same track wait for each other. `quality` defaults to the configured
    /// download quality (`High`, 16-bit / 44.1 kHz FLAC). Call
    /// [`trim_stream_cache`] afterwards to bound the folder.
    #[uniffi::method(default(quality = None))]
    pub async fn fetch_stream(
        self: Arc<Self>,
        track_id: String,
        cache_dir: String,
        quality: Option<AudioQuality>,
    ) -> Result<String> {
        crate::runtime::run(async move {
            self.session.refresh(600, false).await?;
            let api = self.session.api()?;
            let quality = quality.map(TrackQuality::from).unwrap_or(self.session.config().download.track_quality);
            let dir = PathBuf::from(cache_dir);
            let path = stream_cache::fetch(api, self.session.config(), &dir, &track_id, quality).await?;
            Ok(path.to_string_lossy().into_owned())
        })
        .await
    }

    /// Scans `roots` recursively for playable files. Unchanged files are
    /// served from a cache in the app directory, so rescans are cheap.
    pub async fn scan_library(self: Arc<Self>, roots: Vec<String>) -> Vec<LibraryTrack> {
        crate::runtime::run(async move {
            let cache = self.session.app_dir().join(LIBRARY_CACHE);
            let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
            let tracks =
                tokio::task::spawn_blocking(move || library::scan(&roots, Some(&cache))).await.unwrap_or_default();
            tracks.into_iter().map(LibraryTrack::from).collect()
        })
        .await
    }

    /// Downloads `links` (Tidal URLs or `type/id`), reporting progress to
    /// `listener`. Cancelling the coroutine stops the download.
    pub async fn download(
        self: Arc<Self>,
        links: Vec<String>,
        settings: DownloadSettings,
        listener: Box<dyn DownloadListener>,
    ) -> Result<DownloadSummary> {
        crate::runtime::run(async move {
            let resources = links.iter().map(|l| Resource::parse(l)).collect::<homefi_core::Result<Vec<_>>>()?;

            self.session.refresh(600, false).await?;
            let api = self.session.api()?;

            let download_dir = PathBuf::from(&settings.download_dir);
            let mut opts = DownloadOptions::from_config(self.session.config());
            opts.download_dir = download_dir.clone();
            opts.scan_dir = download_dir.clone();
            opts.track_quality = settings.quality.into();
            opts.skip_existing = settings.skip_existing;
            opts.threads = settings.threads.clamp(1, 8) as usize;
            opts.metadata.cover = settings.embed_cover;
            opts.metadata.lyrics = settings.embed_lyrics;
            opts.videos_filter = if settings.include_videos { MediaFilter::Allow } else { MediaFilter::None };
            opts.template_override = settings.template.filter(|t| !t.trim().is_empty());

            let observer = Arc::new(ListenerObserver { listener, base: download_dir });
            let s = Downloader::new(api, opts, observer).run(&resources).await?;

            Ok(DownloadSummary {
                queued: s.queued,
                downloaded: s.downloaded,
                existing: s.existing,
                skipped: s.skipped,
                failed: s.failed,
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(json: &str) -> Page<ListedItem> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn listed_page_leaves_videos_out_but_keeps_the_offset() {
        let page = listed(
            r#"{"limit": 2, "offset": 0, "totalNumberOfItems": 5, "items": [
                {"type": "track", "item": {"id": 1, "title": "A", "artists": [], "album": {"id": 9, "title": "X"}}},
                {"type": "video", "item": {"id": 2, "title": "B", "artists": null}}
            ]}"#,
        );
        let hits = listed_page(page);
        assert_eq!(hits.items.len(), 1);
        assert_eq!(hits.total, 5);
        assert_eq!(hits.next_offset, Some(2));
    }

    /// Kotlin awaits exports from its own threads (often Main), never from a
    /// tokio one: the calls must bring their runtime.
    #[test]
    fn async_exports_complete_when_called_from_a_plain_thread() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(&music).unwrap();
        std::fs::write(music.join("song.flac"), b"not really flac").unwrap();
        let engine = HomefiEngine::new(dir.path().join("app").display().to_string()).unwrap();

        let scanned = std::thread::spawn({
            let engine = engine.clone();
            let root = music.display().to_string();
            move || futures::executor::block_on(engine.scan_library(vec![root]))
        })
        .join()
        .unwrap();
        assert_eq!(scanned.len(), 1);

        let searched = futures::executor::block_on(engine.clone().search("x".into()));
        assert!(matches!(searched, Err(HomefiError::NotLoggedIn { .. })));
        let lyrics = futures::executor::block_on(engine.file_lyrics(music.join("song.flac").display().to_string()));
        assert!(lyrics.unwrap().is_none());
    }

    #[test]
    fn listed_page_without_limit_steps_by_every_item() {
        let page = listed(
            r#"{"offset": 4, "totalNumberOfItems": 5, "items": [
                {"type": "video", "item": {"id": 2, "title": "B", "artists": null}}
            ]}"#,
        );
        let hits = listed_page(page);
        assert!(hits.items.is_empty());
        assert_eq!(hits.next_offset, None);
    }
}

/// One line of a song; `time_ms` is when it starts (None when not synced).
#[derive(uniffi::Record)]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(uniffi::Record)]
pub struct SongLyrics {
    pub lines: Vec<LyricLine>,
    /// Every line has a start time, so the player can follow along.
    pub synced: bool,
    /// Who provided them (e.g. `MUSIXMATCH`); None for local files.
    pub provider: Option<String>,
}

impl From<homefi_core::lyrics::Lyrics> for SongLyrics {
    fn from(l: homefi_core::lyrics::Lyrics) -> Self {
        SongLyrics {
            lines: l.lines.into_iter().map(|l| LyricLine { time_ms: l.time_ms, text: l.text }).collect(),
            synced: l.synced,
            provider: l.provider,
        }
    }
}

/// Lyrics of a local file: `<same name>.lrc` next to it, or its lyrics tag.
#[uniffi::export]
pub fn local_lyrics(path: String) -> Option<SongLyrics> {
    homefi_core::lyrics::local(std::path::Path::new(&path)).map(SongLyrics::from)
}
