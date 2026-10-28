//! Finds album art on Tidal for local files that have none.

use crate::api::TidalApi;
use crate::library::TrackInfo;
use crate::metadata::Cover;
use crate::models::Track;

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

fn same_artist(track: &Track, artist: &str) -> bool {
    let a = norm(artist);
    !a.is_empty()
        && track.artists.iter().chain(track.artist.iter()).any(|x| {
            let n = norm(&x.name);
            !n.is_empty() && (a.contains(&n) || n.contains(&a))
        })
}

/// Tidal cover id for `t`: exact ISRC match first, then title + artist
/// (preferring the same album), then the album itself.
pub async fn find_cover_id(api: &TidalApi, t: &TrackInfo) -> Option<String> {
    let artist = if t.artist.is_empty() { &t.album_artist } else { &t.artist };
    if t.title.is_empty() || artist.is_empty() {
        return None;
    }
    let results = api.search(&format!("{artist} {}", t.title)).await.ok()?;
    let tracks = &results.tracks.items;

    if let Some(isrc) = &t.isrc {
        if let Some(hit) = tracks.iter().find(|x| x.isrc.as_deref().is_some_and(|i| i.eq_ignore_ascii_case(isrc))) {
            if hit.album.cover.is_some() {
                return hit.album.cover.clone();
            }
        }
    }

    let title = norm(&t.title);
    let album = norm(&t.album);
    let candidates: Vec<&Track> = tracks.iter().filter(|x| norm(&x.title) == title && same_artist(x, artist)).collect();
    if let Some(hit) = candidates
        .iter()
        .find(|x| !album.is_empty() && x.album.title.as_deref().map(norm) == Some(album.clone()))
        .or(candidates.first())
    {
        if hit.album.cover.is_some() {
            return hit.album.cover.clone();
        }
    }

    if !t.album.is_empty() {
        let albums = api.search(&format!("{artist} {}", t.album)).await.ok()?;
        return albums.albums.items.iter().find(|a| norm(&a.title) == album).and_then(|a| a.cover.clone());
    }
    None
}

/// Downloads the 1280px cover with that id.
pub async fn fetch_cover(http: &reqwest::Client, cover_id: &str) -> Option<Vec<u8>> {
    Cover::new(cover_id, Cover::MAX_SIZE).fetch(http).await
}
