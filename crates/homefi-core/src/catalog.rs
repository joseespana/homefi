//! UI-friendly views of Tidal catalog objects, shared by the desktop app and
//! the Android bindings.

use serde::Serialize;

use crate::album_versions::{group_album_versions, AlbumGroup, AlbumVersion, Lang, VersionPrefs};
use crate::api::{all_pages, ArtistAlbumsFilter, TidalApi};
use crate::metadata::Cover;
use crate::models::{Album, Artist, ArtistRef, Playlist, Search, Track, Video};
use crate::resource::ResourceKind;

/// Thumbnail size used in lists.
pub const THUMB_SIZE: u32 = 320;

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub kind: ResourceKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    /// Main artist (tracks, albums, videos); empty otherwise.
    pub artist: String,
    /// Album title (tracks); empty otherwise.
    pub album: String,
    pub cover_url: Option<String>,
    pub explicit: bool,
    /// `Hi-Res`, `Lossless`, `Dolby Atmos` when known.
    pub quality: Option<String>,
    /// Tracks only: matches local files that were downloaded before.
    pub isrc: Option<String>,
    pub duration_ms: Option<u64>,
    /// Albums shown as one card: every release of it (this one included,
    /// `primary`), see [`album_cards`]. Empty for everything else.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<VersionHit>,
}

/// One release of an album, for a version picker.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VersionHit {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub explicit: bool,
    /// `Hi-Res`, `Lossless`, `Dolby Atmos` when known.
    pub quality: Option<String>,
    pub year: Option<u32>,
    pub tracks: u32,
    /// Deluxe / box set / expanded: tracks over the standard edition.
    pub extra_tracks: Option<u32>,
    /// Dolby Atmos: needs an Atmos device.
    pub atmos: bool,
    /// The one shown by default.
    pub primary: bool,
    /// Stable ids (`explicit`, `clean`, `dolby_atmos`, `hi_res`, `lossless`,
    /// `remastered`, `deluxe`, `box_set`, `expanded`, `anniversary`, `live`,
    /// `original`), same order as `labels`.
    pub label_codes: Vec<String>,
    /// Chip texts: `Explicit`, `Hi-Res`, `Remastered 2023`, `Box set +209`...
    pub labels: Vec<String>,
    /// `Explicit · Hi-Res 24-bit · 12 tracks · 2023`.
    pub description: String,
}

impl VersionHit {
    pub fn new(v: &AlbumVersion, primary: bool, lang: Lang) -> Self {
        let a = &v.album;
        Self {
            id: a.id.to_string(),
            title: a.title.clone(),
            cover_url: cover_url(a.cover.as_deref()),
            explicit: a.explicit,
            quality: quality_label(&a.media_metadata.tags, a.audio_quality.as_deref()),
            year: v.year(),
            tracks: u32::try_from(a.number_of_tracks).unwrap_or(0),
            extra_tracks: v.labels.iter().find_map(|l| l.extra_tracks()),
            atmos: v.is_atmos(),
            primary,
            label_codes: v.labels.iter().map(|l| l.code().to_owned()).collect(),
            labels: v.labels.iter().map(|l| l.text(lang)).collect(),
            description: v.description(lang),
        }
    }
}

fn version_hits(g: &AlbumGroup, lang: Lang) -> Vec<VersionHit> {
    g.versions.iter().enumerate().map(|(i, v)| VersionHit::new(v, i == g.primary_index, lang)).collect()
}

/// Album cards for a list: one per album with its releases in `versions`
/// (when there is more than one), the default pick in front; with
/// `prefs.group` off, every release as Tidal lists it.
pub fn album_cards(albums: &[Album], prefs: &VersionPrefs, lang: Lang) -> Vec<Hit> {
    if !prefs.group {
        return albums.iter().map(album_hit).collect();
    }
    group_album_versions(albums, prefs)
        .iter()
        .map(|g| {
            let mut hit = album_hit(&g.primary);
            if g.versions.len() > 1 {
                hit.versions = version_hits(g, lang);
            }
            hit
        })
        .collect()
}

/// Every release of `album` on its main artist's page (itself included, even
/// when the artist page doesn't list it), for the album header. Read-only.
pub async fn album_versions(
    api: &TidalApi,
    album: &Album,
    prefs: &VersionPrefs,
    lang: Lang,
) -> crate::Result<Vec<VersionHit>> {
    let me =
        || vec![VersionHit::new(&group_album_versions(std::slice::from_ref(album), prefs)[0].versions[0], true, lang)];
    let Some(artist) = album.artist.as_ref().or(album.artists.first()).map(|a| a.id.to_string()) else {
        return Ok(me());
    };
    let filter = match album.kind.to_ascii_uppercase().as_str() {
        "EP" | "SINGLE" => ArtistAlbumsFilter::EpsAndSingles,
        _ => ArtistAlbumsFilter::Albums,
    };
    let mut list = all_pages(|o| api.artist_albums(&artist, o, filter)).await?;
    if !list.iter().any(|a| a.id == album.id) {
        list.push(album.clone());
    }
    let groups = group_album_versions(&list, prefs);
    Ok(groups
        .iter()
        .find(|g| g.versions.iter().any(|v| v.album.id == album.id))
        .map_or_else(me, |g| version_hits(g, lang)))
}

impl Hit {
    /// `type/id`, accepted by [`crate::resource::Resource::parse`].
    pub fn link(&self) -> String {
        format!("{}/{}", self.kind, self.id)
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchResults {
    pub top_hit: Option<Hit>,
    pub tracks: Vec<Hit>,
    pub albums: Vec<Hit>,
    pub artists: Vec<Hit>,
    pub playlists: Vec<Hit>,
    pub videos: Vec<Hit>,
}

pub fn cover_url(uid: Option<&str>) -> Option<String> {
    uid.map(|u| Cover::new(u, THUMB_SIZE).url)
}

fn artist_names(main: Option<&ArtistRef>, all: &[ArtistRef]) -> String {
    main.or(all.first()).map(|a| a.name.clone()).unwrap_or_default()
}

fn quality_label(tags: &[String], audio_quality: Option<&str>) -> Option<String> {
    if tags.iter().any(|t| t == "HIRES_LOSSLESS") {
        Some("Hi-Res".into())
    } else if tags.iter().any(|t| t == "DOLBY_ATMOS") {
        Some("Dolby Atmos".into())
    } else if tags.iter().any(|t| t == "LOSSLESS") || audio_quality == Some("LOSSLESS") {
        Some("Lossless".into())
    } else {
        None
    }
}

pub fn track_hit(t: &Track) -> Hit {
    let artist = artist_names(t.artist.as_ref(), &t.artists);
    let album = t.album.title.clone().unwrap_or_default();
    Hit {
        kind: ResourceKind::Track,
        id: t.id.to_string(),
        title: t.title.clone(),
        subtitle: format!("{artist} · {album}"),
        artist,
        album,
        cover_url: cover_url(t.album.cover.as_deref()),
        explicit: t.explicit,
        quality: quality_label(&t.media_metadata.tags, Some(&t.audio_quality)),
        isrc: t.isrc.as_ref().map(|i| i.to_uppercase()),
        duration_ms: Some(t.duration as u64 * 1000),
        versions: Vec::new(),
    }
}

pub fn album_hit(a: &Album) -> Hit {
    let year = a.release_date.as_deref().and_then(|d| d.get(..4)).unwrap_or_default();
    let artist = artist_names(a.artist.as_ref(), &a.artists);
    Hit {
        kind: ResourceKind::Album,
        id: a.id.to_string(),
        title: a.title.clone(),
        subtitle: [artist.clone(), year.to_owned()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        artist,
        album: a.title.clone(),
        cover_url: cover_url(a.cover.as_deref()),
        explicit: a.explicit,
        quality: quality_label(&a.media_metadata.tags, a.audio_quality.as_deref()),
        isrc: None,
        duration_ms: None,
        versions: Vec::new(),
    }
}

pub fn artist_hit(a: &Artist) -> Hit {
    Hit {
        kind: ResourceKind::Artist,
        id: a.id.to_string(),
        title: a.name.clone(),
        subtitle: "Artist".into(),
        artist: a.name.clone(),
        album: String::new(),
        cover_url: cover_url(a.picture.as_deref()),
        explicit: false,
        quality: None,
        isrc: None,
        duration_ms: None,
        versions: Vec::new(),
    }
}

pub fn playlist_hit(p: &Playlist) -> Hit {
    Hit {
        kind: ResourceKind::Playlist,
        id: p.uuid.clone(),
        title: p.title.clone(),
        subtitle: format!("{} tracks", p.number_of_tracks),
        artist: String::new(),
        album: String::new(),
        cover_url: cover_url(p.square_image.as_deref().or(p.image.as_deref())),
        explicit: false,
        quality: None,
        isrc: None,
        duration_ms: None,
        versions: Vec::new(),
    }
}

pub fn video_hit(v: &Video) -> Hit {
    Hit {
        kind: ResourceKind::Video,
        id: v.id.to_string(),
        title: v.title.clone(),
        subtitle: artist_names(v.artist.as_ref(), &v.artists),
        artist: artist_names(v.artist.as_ref(), &v.artists),
        album: String::new(),
        cover_url: cover_url(v.image_id.as_deref()),
        explicit: v.explicit,
        quality: None,
        isrc: None,
        duration_ms: None,
        versions: Vec::new(),
    }
}

impl From<&Search> for SearchResults {
    fn from(r: &Search) -> Self {
        let top_hit = r.top_hit.as_ref().and_then(|top| {
            let v = top.value.clone();
            match top.kind.parse::<ResourceKind>().ok()? {
                ResourceKind::Track => serde_json::from_value(v).ok().map(|t| track_hit(&t)),
                ResourceKind::Album => serde_json::from_value(v).ok().map(|a| album_hit(&a)),
                ResourceKind::Artist => serde_json::from_value(v).ok().map(|a| artist_hit(&a)),
                ResourceKind::Playlist => serde_json::from_value(v).ok().map(|p| playlist_hit(&p)),
                ResourceKind::Video => serde_json::from_value(v).ok().map(|v| video_hit(&v)),
                ResourceKind::Mix => None,
            }
        });
        Self {
            top_hit,
            tracks: r.tracks.items.iter().map(track_hit).collect(),
            albums: r.albums.items.iter().map(album_hit).collect(),
            artists: r.artists.items.iter().map(artist_hit).collect(),
            playlists: r.playlists.items.iter().map(playlist_hit).collect(),
            videos: r.videos.items.iter().map(video_hit).collect(),
        }
    }
}

impl SearchResults {
    /// Like `From<&Search>`, with the albums as [`album_cards`].
    pub fn grouped(r: &Search, prefs: &VersionPrefs, lang: Lang) -> Self {
        let mut out = Self::from(r);
        out.albums = album_cards(&r.albums.items, prefs, lang);
        out
    }
}
