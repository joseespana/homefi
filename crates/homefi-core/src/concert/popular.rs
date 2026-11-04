//! Tidal's most played songs of an artist: the fallback when there is no
//! real setlist. Read-only calls.

use crate::api::TidalApi;
use crate::error::Result;
use crate::models::Track;
use crate::text::fold;

/// How many of an artist's most played songs the playlist takes.
pub const POPULAR_LIMIT: usize = 25;

/// Tidal's best match for `artist` and its most played tracks (at most
/// `limit`, streamable only). `Ok(None)` = Tidal has no such artist.
pub async fn most_played(api: &TidalApi, artist: &str, limit: usize) -> Result<Option<(String, Vec<Track>)>> {
    let found = api.search(artist).await?;
    let wanted = fold(artist);
    let items = &found.artists.items;
    let Some(best) = items.iter().find(|a| fold(&a.name) == wanted).or_else(|| items.first()) else {
        return Ok(None);
    };
    let top = api.artist_top_tracks(&best.id.to_string(), 0).await?;
    let tracks = top.items.into_iter().filter(|t| t.allow_streaming).take(limit).collect();
    Ok(Some((best.name.clone(), tracks)))
}
