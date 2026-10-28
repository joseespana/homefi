//! Tidal REST API.

mod cache;
mod client;

pub use cache::DiskCache;
pub use client::{CachePolicy, TidalClient, TokenRefresher, API_URL, API_V2_URL};

use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::models::*;
use crate::quality::{TrackQuality, VideoQuality};
use crate::resource::ResourceKind;

const HOUR: CachePolicy = CachePolicy::Ttl(Duration::from_secs(3600));
/// Genres rarely (if ever) change: cache them for a week.
const WEEK: CachePolicy = CachePolicy::Ttl(Duration::from_secs(604_800));

/// Page sizes: the biggest each endpoint accepts, so long lists take few calls.
pub mod limits {
    pub const ARTIST_ALBUMS: i64 = 100;
    pub const ARTIST_VIDEOS: i64 = 10;
    pub const ARTIST_TOP_TRACKS: i64 = 100;
    pub const ALBUM_ITEMS: i64 = 100;
    pub const PLAYLIST_ITEMS: i64 = 100;
    pub const MIX_ITEMS: i64 = 100;
    pub const USER_PLAYLISTS: i64 = 50;
    /// Tidal rejects bigger pages on the favorites endpoints (400, subStatus 1001).
    pub const FAVORITES: i64 = 50;
}

/// Pages fetched at once when a whole list is wanted.
const PARALLEL_PAGES: usize = 4;
/// Pause before fetching a failed page of a list again.
const PAGE_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Album filter for [`TidalApi::artist_albums`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtistAlbumsFilter {
    Albums,
    EpsAndSingles,
}

impl ArtistAlbumsFilter {
    fn as_str(self) -> &'static str {
        match self {
            Self::Albums => "ALBUMS",
            Self::EpsAndSingles => "EPSANDSINGLES",
        }
    }
}

/// Typed Tidal API bound to one user / country.
#[derive(Clone)]
pub struct TidalApi {
    client: Arc<TidalClient>,
    user_id: String,
    country_code: String,
}

impl TidalApi {
    pub fn new(client: Arc<TidalClient>, user_id: impl Into<String>, country_code: impl Into<String>) -> Self {
        Self { client, user_id: user_id.into(), country_code: country_code.into() }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn client(&self) -> &TidalClient {
        &self.client
    }

    pub fn country_code(&self) -> &str {
        &self.country_code
    }

    fn cc(&self) -> (&'static str, String) {
        ("countryCode", self.country_code.clone())
    }

    fn paged(&self, limit: i64, offset: i64) -> [(&'static str, String); 3] {
        [self.cc(), ("limit", limit.to_string()), ("offset", offset.to_string())]
    }

    pub async fn album(&self, id: &str) -> Result<Album> {
        self.client.fetch(&format!("albums/{id}"), &[self.cc()], HOUR).await
    }

    pub async fn album_items(&self, id: &str, offset: i64) -> Result<Page<ListedItem>> {
        let params = self.paged(limits::ALBUM_ITEMS, offset);
        self.client.fetch(&format!("albums/{id}/items"), &params, HOUR).await
    }

    /// Album items including credits (producers, composers, ...).
    pub async fn album_items_credits(&self, id: &str, offset: i64) -> Result<Page<ListedItem>> {
        let params = self.paged(limits::ALBUM_ITEMS, offset);
        self.client.fetch(&format!("albums/{id}/items/credits"), &params, HOUR).await
    }

    pub async fn album_review(&self, id: &str) -> Result<AlbumReview> {
        self.client.fetch(&format!("albums/{id}/review"), &[self.cc()], HOUR).await
    }

    pub async fn artist(&self, id: &str) -> Result<Artist> {
        self.client.fetch(&format!("artists/{id}"), &[self.cc()], HOUR).await
    }

    pub async fn artist_albums(&self, id: &str, offset: i64, filter: ArtistAlbumsFilter) -> Result<Page<Album>> {
        let [cc, limit, offset] = self.paged(limits::ARTIST_ALBUMS, offset);
        let params = [cc, limit, offset, ("filter", filter.as_str().to_owned())];
        self.client.fetch(&format!("artists/{id}/albums"), &params, HOUR).await
    }

    /// The artist's most popular tracks, most played first.
    pub async fn artist_top_tracks(&self, id: &str, offset: i64) -> Result<Page<Track>> {
        let params = self.paged(limits::ARTIST_TOP_TRACKS, offset);
        self.client.fetch(&format!("artists/{id}/toptracks"), &params, HOUR).await
    }

    pub async fn artist_videos(&self, id: &str, offset: i64) -> Result<Page<Video>> {
        let params = self.paged(limits::ARTIST_VIDEOS, offset);
        self.client.fetch(&format!("artists/{id}/videos"), &params, HOUR).await
    }

    pub async fn mix_items(&self, id: &str, offset: i64) -> Result<Page<ListedItem>> {
        let params = self.paged(limits::MIX_ITEMS, offset);
        self.client.fetch(&format!("mixes/{id}/items"), &params, HOUR).await
    }

    pub async fn favorites(&self) -> Result<Favorites> {
        let endpoint = format!("users/{}/favorites/ids", self.user_id);
        self.client.fetch(&endpoint, &[self.cc()], CachePolicy::NoStore).await
    }

    // ------------------------------------------------------------ collection edits

    /// `(endpoint segment, form field)` for favorites of `kind`.
    fn favorite_target(kind: ResourceKind) -> Result<(&'static str, &'static str)> {
        Ok(match kind {
            ResourceKind::Track => ("tracks", "trackIds"),
            ResourceKind::Album => ("albums", "albumIds"),
            ResourceKind::Artist => ("artists", "artistIds"),
            ResourceKind::Playlist => ("playlists", "uuids"),
            ResourceKind::Video => ("videos", "videoIds"),
            ResourceKind::Mix => return Err(Error::Stream("mixes can't be added to favorites".into())),
        })
    }

    /// Adds a track / album / artist / playlist / video to the user's favorites.
    pub async fn add_favorite(&self, kind: ResourceKind, id: &str) -> Result<()> {
        let (segment, field) = Self::favorite_target(kind)?;
        let endpoint = format!("users/{}/favorites/{segment}", self.user_id);
        let form = [(field, id.to_owned()), ("onArtifactNotFound", "FAIL".to_owned())];
        self.client.send(Method::POST, false, &endpoint, &[self.cc()], &form, None).await.map(drop)
    }

    pub async fn remove_favorite(&self, kind: ResourceKind, id: &str) -> Result<()> {
        let (segment, _) = Self::favorite_target(kind)?;
        let endpoint = format!("users/{}/favorites/{segment}/{id}", self.user_id);
        self.client.send(Method::DELETE, false, &endpoint, &[self.cc()], &[], None).await.map(drop)
    }

    /// Creates a playlist in the user's collection and returns it.
    pub async fn create_playlist(&self, title: &str, description: &str) -> Result<Playlist> {
        let params = [
            ("name", title.to_owned()),
            ("description", description.to_owned()),
            ("folderId", "root".to_owned()),
            self.cc(),
        ];
        let created = self
            .client
            .send(Method::PUT, true, "my-collection/playlists/folders/create-playlist", &params, &[], None)
            .await;
        let uuid = match created {
            Ok(v) => v.pointer("/data/uuid").and_then(|u| u.as_str()).map(str::to_owned),
            Err(e) => {
                tracing::debug!("v2 create-playlist failed ({e}), trying v1");
                None
            }
        };
        let uuid = match uuid {
            Some(uuid) => uuid,
            None => {
                let form = [("title", title.to_owned()), ("description", description.to_owned())];
                let endpoint = format!("users/{}/playlists", self.user_id);
                let v = self.client.send(Method::POST, false, &endpoint, &[self.cc()], &form, None).await?;
                v.get("uuid")
                    .and_then(|u| u.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| Error::Decode { endpoint, message: "no playlist uuid in the answer".into() })?
            }
        };
        self.playlist(&uuid).await
    }

    /// Appends tracks to a playlist the user owns; duplicates are skipped.
    pub async fn add_to_playlist(&self, uuid: &str, track_ids: &[String]) -> Result<()> {
        if track_ids.is_empty() {
            return Ok(());
        }
        let endpoint = format!("playlists/{uuid}");
        let etag = self.client.etag(&endpoint, &[self.cc()]).await?;
        let form = [
            ("trackIds", track_ids.join(",")),
            ("onArtifactNotFound", "SKIP".to_owned()),
            ("onDupes", "SKIP".to_owned()),
        ];
        let items = format!("{endpoint}/items");
        self.client.send(Method::POST, false, &items, &[self.cc()], &form, etag.as_deref()).await.map(drop)
    }

    /// Removes the item at `index` (0-based, playlist order).
    pub async fn remove_from_playlist(&self, uuid: &str, index: usize) -> Result<()> {
        let endpoint = format!("playlists/{uuid}");
        let etag = self.client.etag(&endpoint, &[self.cc()]).await?;
        let items = format!("{endpoint}/items/{index}");
        let params = [self.cc(), ("order", "INDEX".to_owned()), ("orderDirection", "ASC".to_owned())];
        self.client.send(Method::DELETE, false, &items, &params, &[], etag.as_deref()).await.map(drop)
    }

    pub async fn delete_playlist(&self, uuid: &str) -> Result<()> {
        let endpoint = format!("playlists/{uuid}");
        self.client.send(Method::DELETE, false, &endpoint, &[self.cc()], &[], None).await.map(drop)
    }

    /// Playlists created by the user.
    pub async fn user_playlists(&self, offset: i64) -> Result<Page<Playlist>> {
        let params = self.paged(limits::USER_PLAYLISTS, offset);
        let endpoint = format!("users/{}/playlists", self.user_id);
        self.client.fetch(&endpoint, &params, CachePolicy::NoStore).await
    }

    fn favorites_params(&self, offset: i64) -> [(&'static str, String); 5] {
        [
            self.cc(),
            ("limit", limits::FAVORITES.to_string()),
            ("offset", offset.to_string()),
            ("order", "DATE".to_owned()),
            ("orderDirection", "DESC".to_owned()),
        ]
    }

    async fn favorites_page<T: serde::de::DeserializeOwned>(
        &self,
        kind: &str,
        offset: i64,
    ) -> Result<Page<Favorite<T>>> {
        let endpoint = format!("users/{}/favorites/{kind}", self.user_id);
        self.client.fetch(&endpoint, &self.favorites_params(offset), CachePolicy::NoStore).await
    }

    /// Favorite albums, newest first.
    pub async fn favorite_albums(&self, offset: i64) -> Result<Page<Favorite<Album>>> {
        self.favorites_page("albums", offset).await
    }

    /// Favorite tracks, newest first.
    pub async fn favorite_tracks(&self, offset: i64) -> Result<Page<Favorite<Track>>> {
        self.favorites_page("tracks", offset).await
    }

    /// Favorite artists, newest first.
    pub async fn favorite_artists(&self, offset: i64) -> Result<Page<Favorite<Artist>>> {
        self.favorites_page("artists", offset).await
    }

    /// Playlists the user created plus the ones they follow.
    pub async fn playlists_and_favorites(&self, offset: i64) -> Result<Page<PlaylistEntry>> {
        let endpoint = format!("users/{}/playlistsAndFavoritePlaylists", self.user_id);
        self.client.fetch(&endpoint, &self.favorites_params(offset), CachePolicy::NoStore).await
    }

    pub async fn playlist(&self, uuid: &str) -> Result<Playlist> {
        self.client.fetch(&format!("playlists/{uuid}"), &[self.cc()], CachePolicy::NoStore).await
    }

    pub async fn playlist_items(&self, uuid: &str, offset: i64) -> Result<Page<ListedItem>> {
        let params = self.paged(limits::PLAYLIST_ITEMS, offset);
        self.client.fetch(&format!("playlists/{uuid}/items"), &params, CachePolicy::NoStore).await
    }

    pub async fn search(&self, query: &str) -> Result<Search> {
        let params = [self.cc(), ("query", query.to_owned())];
        self.client.fetch("search", &params, CachePolicy::NoStore).await
    }

    pub async fn session(&self) -> Result<SessionInfo> {
        self.client.fetch("sessions", &[], CachePolicy::NoStore).await
    }

    pub async fn track(&self, id: &str) -> Result<Track> {
        self.client.fetch(&format!("tracks/{id}"), &[self.cc()], HOUR).await
    }

    /// Tidal tracks with this ISRC: the same recording is often on many releases
    /// (album, singles, compilations), each with its own id.
    pub async fn tracks_by_isrc(&self, isrc: &str) -> Result<Vec<Track>> {
        let params = [("isrc", isrc.to_uppercase()), self.cc(), ("limit", "100".to_owned())];
        let page: Page<Track> = self.client.fetch("tracks", &params, WEEK).await?;
        Ok(page.items)
    }

    pub async fn track_lyrics(&self, id: i64) -> Result<TrackLyrics> {
        self.client.fetch(&format!("tracks/{id}/lyrics"), &[self.cc()], HOUR).await
    }

    pub async fn track_stream(&self, id: i64, quality: TrackQuality) -> Result<TrackStream> {
        let params = [
            ("audioquality", quality.api_name().to_owned()),
            ("playbackmode", "STREAM".to_owned()),
            ("assetpresentation", "FULL".to_owned()),
        ];
        self.client.fetch(&format!("tracks/{id}/playbackinfopostpaywall"), &params, CachePolicy::NoStore).await
    }

    pub async fn video(&self, id: &str) -> Result<Video> {
        self.client.fetch(&format!("videos/{id}"), &[self.cc()], HOUR).await
    }

    pub async fn video_stream(&self, id: i64, quality: VideoQuality) -> Result<VideoStream> {
        let params = [
            ("videoquality", quality.api_name().to_owned()),
            ("playbackmode", "STREAM".to_owned()),
            ("assetpresentation", "FULL".to_owned()),
        ];
        self.client.fetch(&format!("videos/{id}/playbackinfopostpaywall"), &params, CachePolicy::NoStore).await
    }

    /// Genre names of an album (Tidal v2 JSON:API). Empty on any failure.
    pub async fn album_genres(&self, id: i64) -> Vec<String> {
        self.genres(&format!("albums/{id}/relationships/genres")).await
    }

    /// Genre names of a track (Tidal v2 JSON:API). Empty on any failure.
    pub async fn track_genres(&self, id: i64) -> Vec<String> {
        self.genres(&format!("tracks/{id}/relationships/genres")).await
    }

    async fn genres(&self, endpoint: &str) -> Vec<String> {
        let params = [self.cc(), ("locale", "en_US".to_owned()), ("include", "genres".to_owned())];
        match self.client.fetch_v2(endpoint, &params, WEEK).await {
            Ok(data) => parse_genre_names(&data),
            Err(e) => {
                tracing::debug!("{endpoint} failed: {e}");
                Vec::new()
            }
        }
    }
}

/// Fetches every page of a paginated endpoint.
pub async fn all_pages<T, F, Fut>(fetch: F) -> Result<Vec<T>>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = Result<Page<T>>>,
{
    pages_up_to(usize::MAX, fetch).await
}

/// Fetches pages until at least `max` items were received (or the last page),
/// then truncates to `max`. The first page tells how many there are; the rest
/// are fetched a few at a time, in parallel.
pub async fn pages_up_to<T, F, Fut>(max: usize, mut fetch: F) -> Result<Vec<T>>
where
    F: FnMut(i64) -> Fut,
    Fut: std::future::Future<Output = Result<Page<T>>>,
{
    use futures::{stream, StreamExt};

    let first = fetch(0).await?;
    let step = if first.limit > 0 { first.limit } else { first.items.len() as i64 };
    let wanted = first.total_number_of_items.min(i64::try_from(max).unwrap_or(i64::MAX));
    let mut out = first.items;
    if step > 0 && wanted > step {
        let offsets = (1..).map(|i| i * step).take_while(|&o| o < wanted);
        let pages: Vec<(i64, Result<Page<T>>)> = stream::iter(offsets.map(|o| {
            let page = fetch(o);
            async move { (o, page.await) }
        }))
        .buffered(PARALLEL_PAGES)
        .collect()
        .await;
        for (offset, page) in pages {
            // one busy page (429 after the client's own retries) does not lose the list
            let page = match page {
                Err(e) if e.is_retryable() => {
                    tracing::debug!("page at {offset} failed ({e}), trying it again");
                    tokio::time::sleep(PAGE_RETRY_DELAY).await;
                    fetch(offset).await?
                }
                page => page?,
            };
            out.extend(page.items);
        }
    }
    out.truncate(max);
    Ok(out)
}

/// Extracts unique genre names, in order, from a v2 JSON:API response.
pub fn parse_genre_names(payload: &Value) -> Vec<String> {
    let Some(included) = payload.get("included").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut names: Vec<String> = Vec::new();
    for entry in included {
        if entry.get("type").and_then(Value::as_str) != Some("genres") {
            continue;
        }
        if let Some(name) = entry.pointer("/attributes/name").and_then(Value::as_str) {
            if !name.is_empty() && !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres_are_deduplicated() {
        let payload = serde_json::json!({
            "data": [],
            "included": [
                {"type": "genres", "attributes": {"name": "Pop"}},
                {"type": "artists", "attributes": {"name": "Nope"}},
                {"type": "genres", "attributes": {"name": "Pop"}},
                {"type": "genres", "attributes": {"name": "House"}},
                {"type": "genres", "attributes": {}}
            ]
        });
        assert_eq!(parse_genre_names(&payload), vec!["Pop", "House"]);
        assert!(parse_genre_names(&serde_json::json!([])).is_empty());
    }
}
