//! Tidal API response models.
//!
//! Fields are deliberately lenient (`Option` / `default`): Tidal adds and
//! drops fields without notice, and a missing cosmetic field must never make a
//! download fail.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Accepts JSON strings and numbers, returning them as `String`.
fn de_string_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let values = Vec::<Value>::deserialize(d)?;
    Ok(values
        .into_iter()
        .map(|v| match v {
            Value::String(s) => s,
            other => other.to_string(),
        })
        .collect())
}

/// Treats `null` as the type's default.
fn de_null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaMetadata {
    #[serde(default, deserialize_with = "de_null_default")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: i64,
    pub name: String,
    /// `MAIN` or `FEATURED`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

impl ArtistRef {
    pub fn is_main(&self) -> bool {
        self.kind.as_deref() == Some("MAIN")
    }
    pub fn is_featured(&self) -> bool {
        self.kind.as_deref() == Some("FEATURED")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRef {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub vibrant_color: Option<String>,
    #[serde(default)]
    pub video_cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub replay_gain: Option<f64>,
    #[serde(default)]
    pub peak: Option<f64>,
    #[serde(default = "yes")]
    pub allow_streaming: bool,
    #[serde(default)]
    pub stream_ready: bool,
    #[serde(default)]
    pub stream_start_date: Option<String>,
    #[serde(default)]
    pub track_number: i64,
    #[serde(default)]
    pub volume_number: i64,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub popularity: Option<i64>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub bpm: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub audio_quality: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub audio_modes: Vec<String>,
    #[serde(default)]
    pub media_metadata: MediaMetadata,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub album: AlbumRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub volume_number: i64,
    #[serde(default)]
    pub track_number: i64,
    #[serde(default)]
    pub image_id: Option<String>,
    #[serde(default)]
    pub vibrant_color: Option<String>,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default = "yes")]
    pub allow_streaming: bool,
    #[serde(default)]
    pub stream_start_date: Option<String>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub popularity: Option<i64>,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub album: Option<AlbumRef>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub duration: i64,
    #[serde(default = "yes")]
    pub allow_streaming: bool,
    #[serde(default)]
    pub number_of_tracks: i64,
    #[serde(default)]
    pub number_of_videos: i64,
    #[serde(default)]
    pub number_of_volumes: i64,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    /// `ALBUM`, `EP` or `SINGLE`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub vibrant_color: Option<String>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub upc: Option<String>,
    #[serde(default)]
    pub popularity: Option<i64>,
    #[serde(default)]
    pub audio_quality: Option<String>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub audio_modes: Vec<String>,
    #[serde(default)]
    pub media_metadata: MediaMetadata,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default, deserialize_with = "de_null_default")]
    pub artists: Vec<ArtistRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub uuid: String,
    pub title: String,
    #[serde(default)]
    pub number_of_tracks: i64,
    #[serde(default)]
    pub number_of_videos: i64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub public_playlist: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub square_image: Option<String>,
    #[serde(default)]
    pub popularity: Option<i64>,
    /// Owner; `id` is `0` for Tidal's editorial playlists.
    #[serde(default)]
    pub creator: Option<PlaylistCreator>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaylistCreator {
    #[serde(default)]
    pub id: Option<i64>,
}

impl Playlist {
    /// `true` when `user_id` created it (and can edit it).
    pub fn is_owned_by(&self, user_id: &str) -> bool {
        self.creator.as_ref().and_then(|c| c.id).is_some_and(|id| id.to_string() == user_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub popularity: Option<i64>,
}

/// A track or a video, as returned in album / playlist / mix listings.
#[derive(Debug, Clone, Serialize)]
pub enum MediaItem {
    Track(Track),
    Video(Video),
}

impl MediaItem {
    pub fn id(&self) -> i64 {
        match self {
            MediaItem::Track(t) => t.id,
            MediaItem::Video(v) => v.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            MediaItem::Track(t) => &t.title,
            MediaItem::Video(v) => &v.title,
        }
    }

    pub fn album_id(&self) -> Option<i64> {
        match self {
            MediaItem::Track(t) => t.album.id,
            MediaItem::Video(v) => v.album.as_ref().and_then(|a| a.id),
        }
    }

    pub fn allow_streaming(&self) -> bool {
        match self {
            MediaItem::Track(t) => t.allow_streaming,
            MediaItem::Video(v) => v.allow_streaming,
        }
    }

    pub fn artists(&self) -> &[ArtistRef] {
        match self {
            MediaItem::Track(t) => &t.artists,
            MediaItem::Video(v) => &v.artists,
        }
    }

    pub fn artist(&self) -> Option<&ArtistRef> {
        match self {
            MediaItem::Track(t) => t.artist.as_ref(),
            MediaItem::Video(v) => v.artist.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    pub name: String,
    #[serde(default)]
    pub id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditsEntry {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, deserialize_with = "de_null_default")]
    pub contributors: Vec<Contributor>,
}

/// One entry of an album / playlist / mix listing.
#[derive(Debug, Clone, Serialize)]
pub struct ListedItem {
    pub item: MediaItem,
    pub credits: Vec<CreditsEntry>,
}

impl<'de> Deserialize<'de> for ListedItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            item: Value,
            #[serde(rename = "type", default)]
            kind: Option<String>,
            #[serde(default, deserialize_with = "de_null_default")]
            credits: Vec<CreditsEntry>,
        }

        let raw = Raw::deserialize(d)?;
        let item = match raw.kind.as_deref().unwrap_or("track") {
            "video" => MediaItem::Video(serde_json::from_value(raw.item).map_err(de::Error::custom)?),
            _ => MediaItem::Track(serde_json::from_value(raw.item).map_err(de::Error::custom)?),
        };
        Ok(ListedItem { item, credits: raw.credits })
    }
}

/// Paginated response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    #[serde(default)]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub total_number_of_items: i64,
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

impl<T> Page<T> {
    /// Offset of the next page, or `None` when this was the last one.
    pub fn next_offset(&self) -> Option<i64> {
        let step = if self.limit > 0 { self.limit } else { self.items.len() as i64 };
        let next = self.offset + step;
        (step > 0 && next < self.total_number_of_items).then_some(next)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Favorites {
    #[serde(default, deserialize_with = "de_string_list")]
    pub playlist: Vec<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub album: Vec<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub video: Vec<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub track: Vec<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub artist: Vec<String>,
}

/// An entry of `users/{id}/favorites/*`.
#[derive(Debug, Clone, Deserialize)]
pub struct Favorite<T> {
    #[serde(default)]
    pub created: Option<String>,
    pub item: T,
}

/// An entry of `users/{id}/playlistsAndFavoritePlaylists`.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistEntry {
    /// `USER_CREATED` or `USER_FAVORITE`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    pub playlist: Playlist,
}

impl PlaylistEntry {
    pub fn is_own(&self) -> bool {
        self.kind.as_deref() == Some("USER_CREATED")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStream {
    pub track_id: i64,
    /// `STEREO` or `DOLBY_ATMOS`.
    #[serde(default)]
    pub audio_mode: String,
    pub audio_quality: String,
    pub manifest_mime_type: String,
    pub manifest: String,
    #[serde(default)]
    pub bit_depth: Option<i64>,
    #[serde(default)]
    pub sample_rate: Option<i64>,
    /// ReplayGain of the track and its album (dB) and their peaks (linear).
    #[serde(default)]
    pub track_replay_gain: Option<f64>,
    #[serde(default)]
    pub track_peak_amplitude: Option<f64>,
    #[serde(default)]
    pub album_replay_gain: Option<f64>,
    #[serde(default)]
    pub album_peak_amplitude: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStream {
    pub video_id: i64,
    pub video_quality: String,
    pub manifest_mime_type: String,
    pub manifest: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackLyrics {
    #[serde(default)]
    pub lyrics: Option<String>,
    #[serde(default)]
    pub subtitles: Option<String>,
    #[serde(default)]
    pub lyrics_provider: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumReview {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub summary: Option<String>,
}

impl AlbumReview {
    /// Review text with Tidal's `[wimpLink ...]` markup removed.
    pub fn normalized_text(&self) -> String {
        normalize_review_text(&self.text)
    }
}

pub fn normalize_review_text(text: &str) -> String {
    use std::sync::LazyLock;
    static LINK: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?is)\[wimpLink\b[^\]]*\](.*?)\[/wimpLink\]").unwrap());
    static TAG: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?i)\[/?wimpLink\b[^\]]*\]").unwrap());

    let text = LINK.replace_all(text, "$1");
    TAG.replace_all(&text, "").trim().to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub user_id: i64,
    pub country_code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    #[serde(default = "empty_page")]
    pub artists: Page<Artist>,
    #[serde(default = "empty_page")]
    pub albums: Page<Album>,
    #[serde(default = "empty_page")]
    pub playlists: Page<Playlist>,
    #[serde(default = "empty_page")]
    pub tracks: Page<Track>,
    #[serde(default = "empty_page")]
    pub videos: Page<Video>,
    #[serde(default)]
    pub top_hit: Option<TopHit>,
}

fn empty_page<T>() -> Page<T> {
    Page { limit: 0, offset: 0, total_number_of_items: 0, items: Vec::new() }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopHit {
    /// `ARTISTS`, `TRACKS`, `PLAYLISTS`, `ALBUMS`, `VIDEOS`.
    #[serde(rename = "type")]
    pub kind: String,
    pub value: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_text_is_normalized() {
        let text = "  Great [wimpLink artistId=\"1\"]Daft Punk[/wimpLink] album [wimpLink x] ";
        assert_eq!(normalize_review_text(text), "Great Daft Punk album");
    }

    #[test]
    fn listed_items_parse_tracks_and_videos() {
        let json = r#"{
            "limit": 2, "offset": 0, "totalNumberOfItems": 3,
            "items": [
                {"type": "track", "item": {"id": 1, "title": "A", "artists": [], "album": {"id": 9, "title": "X"}},
                 "credits": [{"type": "Producer", "contributors": [{"name": "Thomas Bangalter"}]}]},
                {"type": "video", "item": {"id": 2, "title": "B", "artists": null}}
            ]
        }"#;
        let page: Page<ListedItem> = serde_json::from_str(json).unwrap();
        assert!(matches!(page.items[0].item, MediaItem::Track(_)));
        assert!(matches!(page.items[1].item, MediaItem::Video(_)));
        assert_eq!(page.items[0].credits[0].contributors[0].name, "Thomas Bangalter");
        assert_eq!(page.next_offset(), Some(2));
    }

    #[test]
    fn favorite_and_playlist_entries() {
        let page: Page<Favorite<Artist>> = serde_json::from_str(
            r#"{"limit":2,"offset":0,"totalNumberOfItems":35,"items":[{"created":"2024-01-01T00:00:00.000+0000","item":{"id":7,"name":"Daft Punk"}}]}"#,
        )
        .unwrap();
        assert_eq!(page.items[0].item.name, "Daft Punk");

        let entry: PlaylistEntry =
            serde_json::from_str(r#"{"type":"USER_CREATED","playlist":{"uuid":"u","title":"Mine"}}"#).unwrap();
        assert!(entry.is_own());
    }

    #[test]
    fn favorites_accept_numbers_and_strings() {
        let fav: Favorites = serde_json::from_str(r#"{"TRACK": [1, "2"], "ALBUM": [], "PLAYLIST": ["abc"]}"#).unwrap();
        assert_eq!(fav.track, vec!["1", "2"]);
        assert_eq!(fav.playlist, vec!["abc"]);
    }
}
