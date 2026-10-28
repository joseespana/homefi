//! What guests see of the desktop's music: library files, Tidal tracks and the
//! albums / artists they are browsed by. Pure functions, so they are tested here.
//!
//! Sources in the shared queue (same scheme as Android): `tidal:<track id>`
//! (played with the hub's account) and `hub:<absolute path>` (a file on this
//! computer). Covers of local files are served by the hub at `/cover/<id>`, with
//! the path in URL-safe base64; library albums and artists are `local:<base64 key>`.

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use homefi_core::catalog::Hit;
use homefi_core::resource::ResourceKind;
use homefi_core::share::{BrowseKind, Collection, Page, ShareTrack, Source};

use crate::commands::TrackView;

/// Library albums / artists in search results.
pub const LOCAL_COLLECTION: &str = "local:";
const LOCAL_TRACKS: usize = 30;
const LOCAL_COLLECTIONS: usize = 10;

/// URL-safe id for a path or key (cover URLs, local collections).
pub fn encode_id(text: &str) -> String {
    URL_SAFE_NO_PAD.encode(text.as_bytes())
}

pub fn decode_id(id: &str) -> Option<String> {
    String::from_utf8(URL_SAFE_NO_PAD.decode(id).ok()?).ok()
}

/// Hub path guests turn into a URL; answered by the hub's `cover`.
pub fn cover_path(path: &Path) -> String {
    format!("/cover/{}", encode_id(&path.to_string_lossy()))
}

/// The Tidal id of a file in the stream cache (`<cache>/<id>.flac`).
pub fn cached_tidal_id(path: &Path, stream_dir: &Path) -> Option<String> {
    if path.parent()? != stream_dir {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    (!stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit())).then(|| stem.to_owned())
}

/// What guests see of a file the player has: a Tidal track when it came from the
/// stream cache, a file of this computer otherwise.
pub fn track_of_file(path: &Path, info: Option<&TrackView>, stream_dir: &Path) -> ShareTrack {
    let source = match cached_tidal_id(path, stream_dir) {
        Some(id) => Source::Tidal(id),
        None => Source::Hub(path.to_string_lossy().into_owned()),
    };
    let stem = || path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    ShareTrack {
        source,
        title: info.map(|t| t.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(stem),
        artist: info.map(|t| t.artist.clone()).unwrap_or_default(),
        album: info.map(|t| t.album.clone()).unwrap_or_default(),
        duration_ms: info.map(|t| t.duration_ms).filter(|d| *d > 0),
        cover_url: Some(cover_path(path)),
        explicit: false,
    }
}

/// A Tidal search hit as a song guests can add.
pub fn track_of_hit(hit: &Hit) -> ShareTrack {
    ShareTrack {
        source: Source::Tidal(hit.id.clone()),
        title: hit.title.clone(),
        artist: hit.artist.clone(),
        album: hit.album.clone(),
        duration_ms: hit.duration_ms,
        cover_url: hit.cover_url.clone(),
        explicit: hit.explicit,
    }
}

pub fn collection_of_hit(hit: &Hit) -> Option<Collection> {
    let kind = match hit.kind {
        ResourceKind::Album => BrowseKind::Album,
        ResourceKind::Artist => BrowseKind::Artist,
        ResourceKind::Playlist => BrowseKind::Playlist,
        _ => return None,
    };
    Some(Collection {
        kind,
        id: hit.id.clone(),
        title: hit.title.clone(),
        subtitle: hit.subtitle.clone(),
        cover_url: hit.cover_url.clone(),
    })
}

/// Where a queued song's file is, when it is already on this computer:
/// `hub:` files that exist, `tidal:` tracks already in the stream cache.
pub fn local_file(source: &Source, stream_dir: &Path) -> Option<PathBuf> {
    match source {
        Source::Hub(path) => Some(PathBuf::from(path)).filter(|p| p.is_file()),
        Source::Tidal(id) => std::fs::read_dir(stream_dir).ok()?.flatten().map(|e| e.path()).find(|p| {
            p.is_file()
                && p.file_stem().and_then(|s| s.to_str()) == Some(id.as_str())
                // half-written files of a running fetch
                && !p.extension().is_some_and(|e| e == "part" || e == "tmp")
        }),
        _ => None,
    }
}

fn folder(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    parts.len().checked_sub(2).and_then(|i| parts.get(i)).map(|s| s.to_string()).unwrap_or_default()
}

/// Same grouping as the UI's albums (album artist + album, or the folder).
pub fn album_key(t: &TrackView) -> String {
    let album = if t.album.is_empty() { folder(&t.path) } else { t.album.clone() };
    format!("{}\u{0}{}", t.album_artist.to_lowercase(), album.to_lowercase())
}

fn artist_name(t: &TrackView) -> String {
    if t.album_artist.is_empty() { &t.artist } else { &t.album_artist }.trim().to_owned()
}

fn matches(t: &TrackView, q: &str) -> bool {
    [&t.title, &t.artist, &t.album, &t.album_artist].iter().any(|f| f.to_lowercase().contains(q))
}

fn library_track(t: &TrackView) -> ShareTrack {
    let path = Path::new(&t.path);
    ShareTrack {
        source: Source::Hub(t.path.clone()),
        title: if t.title.is_empty() {
            path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            t.title.clone()
        },
        artist: t.artist.clone(),
        album: t.album.clone(),
        duration_ms: Some(t.duration_ms).filter(|d| *d > 0),
        cover_url: Some(cover_path(path)),
        explicit: false,
    }
}

/// Library songs, albums and artists matching `q` (case-insensitive).
pub fn local_search(tracks: &[TrackView], q: &str) -> (Vec<ShareTrack>, Vec<Collection>, Vec<Collection>) {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Default::default();
    }
    let found: Vec<&TrackView> = tracks.iter().filter(|t| matches(t, &q)).collect();
    let songs = found.iter().take(LOCAL_TRACKS).map(|t| library_track(t)).collect();

    let mut albums: Vec<Collection> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for t in &found {
        let key = album_key(t);
        if keys.contains(&key) || albums.len() >= LOCAL_COLLECTIONS {
            continue;
        }
        keys.push(key.clone());
        albums.push(Collection {
            kind: BrowseKind::Album,
            id: format!("{LOCAL_COLLECTION}{}", encode_id(&key)),
            title: if t.album.is_empty() { folder(&t.path) } else { t.album.clone() },
            subtitle: artist_name(t),
            cover_url: Some(cover_path(Path::new(&t.path))),
        });
    }

    let mut artists: Vec<Collection> = Vec::new();
    for t in &found {
        let name = artist_name(t);
        if name.is_empty()
            || !name.to_lowercase().contains(&q)
            || artists.iter().any(|a| a.title == name)
            || artists.len() >= LOCAL_COLLECTIONS
        {
            continue;
        }
        artists.push(Collection {
            kind: BrowseKind::Artist,
            id: format!("{LOCAL_COLLECTION}{}", encode_id(&name)),
            title: name,
            subtitle: String::new(),
            cover_url: Some(cover_path(Path::new(&t.path))),
        });
    }
    (songs, albums, artists)
}

fn album_order(a: &&TrackView, b: &&TrackView) -> std::cmp::Ordering {
    a.disc_number.cmp(&b.disc_number).then(a.track_number.cmp(&b.track_number)).then(a.title.cmp(&b.title))
}

/// A library album or artist (`id` = `local:<base64 key>`).
pub fn local_page(tracks: &[TrackView], kind: BrowseKind, id: &str) -> Option<Page> {
    let key = decode_id(id.strip_prefix(LOCAL_COLLECTION)?)?;
    let mut list: Vec<&TrackView> = match kind {
        BrowseKind::Album => tracks.iter().filter(|t| album_key(t) == key).collect(),
        BrowseKind::Artist => tracks.iter().filter(|t| artist_name(t) == key).collect(),
        _ => return None,
    };
    let first = *list.first()?;
    if kind == BrowseKind::Album {
        list.sort_by(album_order);
    } else {
        // album by album, each in track order
        list.sort_by(|a, b| album_key(a).cmp(&album_key(b)).then_with(|| album_order(a, b)));
    }
    let header = Collection {
        kind,
        id: id.to_owned(),
        title: match kind {
            BrowseKind::Album if first.album.is_empty() => folder(&first.path),
            BrowseKind::Album => first.album.clone(),
            _ => key.clone(),
        },
        subtitle: if kind == BrowseKind::Album { artist_name(first) } else { String::new() },
        cover_url: Some(cover_path(Path::new(&first.path))),
    };
    Some(Page { header, tracks: list.into_iter().map(library_track).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str, title: &str, artist: &str, album: &str, n: u32) -> TrackView {
        TrackView {
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            album_artist: artist.into(),
            track_number: n,
            disc_number: 1,
            year: None,
            genre: String::new(),
            duration_ms: 200_000,
            format: "FLAC".into(),
            sample_rate: None,
            bit_depth: None,
            lossless: true,
            quality: String::new(),
            isrc: None,
        }
    }

    #[test]
    fn ids_round_trip_any_path() {
        for text in ["/Users/José/Música/a b.flac", "C:\\Music\\x.flac", "daft punk\u{0}discovery"] {
            let id = encode_id(text);
            assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'), "{id}");
            assert_eq!(decode_id(&id).as_deref(), Some(text));
        }
        assert_eq!(decode_id("***"), None);
        assert!(cover_path(Path::new("/m/a.flac")).starts_with("/cover/"));
    }

    #[test]
    fn stream_cache_files_are_tidal_tracks() {
        let cache = Path::new("/app/stream-cache");
        assert_eq!(cached_tidal_id(Path::new("/app/stream-cache/251380837.flac"), cache).as_deref(), Some("251380837"));
        assert_eq!(cached_tidal_id(Path::new("/music/251380837.flac"), cache), None);
        assert_eq!(cached_tidal_id(Path::new("/app/stream-cache/notes.flac"), cache), None);

        let t = track_of_file(Path::new("/app/stream-cache/42.flac"), None, cache);
        assert_eq!(t.source, Source::Tidal("42".into()));
        assert_eq!(t.title, "42");
        let info = track("/music/a.flac", "Song", "Artist", "Album", 1);
        let t = track_of_file(Path::new("/music/a.flac"), Some(&info), cache);
        assert_eq!(t.source, Source::Hub("/music/a.flac".into()));
        assert_eq!((t.title.as_str(), t.duration_ms), ("Song", Some(200_000)));
    }

    #[test]
    fn library_search_and_pages() {
        let lib = vec![
            track("/m/dp/disc/2.flac", "Aerodynamic", "Daft Punk", "Discovery", 2),
            track("/m/dp/disc/1.flac", "One More Time", "Daft Punk", "Discovery", 1),
            track("/m/rh/ir/1.flac", "15 Step", "Radiohead", "In Rainbows", 1),
        ];
        let (songs, albums, artists) = local_search(&lib, "  daft ");
        assert_eq!(songs.len(), 2);
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "Discovery");
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].title, "Daft Punk");
        // an album match does not list its artist
        let (_, albums, artists) = local_search(&lib, "rainbows");
        assert_eq!((albums.len(), artists.len()), (1, 0));
        assert_eq!(local_search(&lib, " ").0.len(), 0);

        let (_, albums, _) = local_search(&lib, "discovery");
        let page = local_page(&lib, BrowseKind::Album, &albums[0].id).unwrap();
        assert_eq!(page.header.title, "Discovery");
        let titles: Vec<&str> = page.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["One More Time", "Aerodynamic"]);

        let (_, _, artists) = local_search(&lib, "radiohead");
        let page = local_page(&lib, BrowseKind::Artist, &artists[0].id).unwrap();
        assert_eq!(page.tracks.len(), 1);
        assert!(local_page(&lib, BrowseKind::Playlist, &artists[0].id).is_none());
        assert!(local_page(&lib, BrowseKind::Album, "12345").is_none());
    }
}
