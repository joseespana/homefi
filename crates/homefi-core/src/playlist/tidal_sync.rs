//! Saving a HomeFi playlist into the user's TIDAL account, so it shows up on every device
//! signed in to it. Only the playlist HomeFi made is written: it is created once, and later
//! saves make its items match HomeFi's exactly (same songs, same order).

use std::collections::HashSet;

use futures::StreamExt;
use serde::Serialize;

use super::model::{HomePlaylist, PlaylistEntry, SongCandidate};
use super::resolve::{best_on_tidal, RESOLVE_PARALLEL, RESOLVE_TIMEOUT};
use super::store::PlaylistStore;
use crate::api::{limits, TidalApi};
use crate::error::Result;
use crate::humane::Lang;

/// Most items added or removed in one call (Tidal's page size for playlist items).
pub const TIDAL_ITEMS_PER_CALL: usize = limits::PLAYLIST_ITEMS as usize;

/// One save to TIDAL at a time: two saves of a new playlist at once would make two copies.
static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What a save to TIDAL did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TidalSaveReport {
    /// The playlist in the TIDAL account.
    pub uuid: String,
    /// It was made now (first save, or the old copy was deleted on TIDAL).
    pub created: bool,
    /// Songs the TIDAL copy has now.
    pub saved: usize,
    /// "Title · Artist" of the songs TIDAL doesn't have, in playlist order.
    pub skipped: Vec<String>,
}

impl TidalSaveReport {
    /// "Saved 24 songs to TIDAL · 2 aren't on TIDAL".
    pub fn summary(&self, lang: Lang) -> String {
        let n = self.saved;
        let k = self.skipped.len();
        let es = lang == Lang::Es;
        let done = match (es, self.created, n == 1) {
            (false, true, true) => "Saved 1 song to TIDAL".to_owned(),
            (false, true, false) => format!("Saved {n} songs to TIDAL"),
            (false, false, true) => "Updated on TIDAL: 1 song".to_owned(),
            (false, false, false) => format!("Updated on TIDAL: {n} songs"),
            (true, true, true) => "1 canción guardada en TIDAL".to_owned(),
            (true, true, false) => format!("{n} canciones guardadas en TIDAL"),
            (true, false, true) => "Actualizada en TIDAL: 1 canción".to_owned(),
            (true, false, false) => format!("Actualizada en TIDAL: {n} canciones"),
        };
        let missing = match (es, k) {
            (_, 0) => return done,
            (false, 1) => "1 isn't on TIDAL".to_owned(),
            (false, _) => format!("{k} aren't on TIDAL"),
            (true, 1) => "1 no está en TIDAL".to_owned(),
            (true, _) => format!("{k} no están en TIDAL"),
        };
        format!("{done} · {missing}")
    }
}

/// Saves `playlist` to the user's TIDAL account and sets its `tidal_uuid`. Without a copy
/// there (never saved, or deleted on TIDAL) one is created with its name and description;
/// otherwise that copy is renamed if needed and its items are replaced, so TIDAL matches
/// HomeFi exactly. Entries without a Tidal id are found by ISRC, then by title and artist
/// (the playlist resolver's confidence bar); the ones TIDAL doesn't have are skipped and
/// reported. `tidal_uuid` is set as soon as the copy exists, even if adding items fails.
pub async fn save_to_tidal(api: &TidalApi, playlist: &mut HomePlaylist) -> Result<TidalSaveReport> {
    let (track_ids, skipped) = tidal_tracks(api, &playlist.entries).await;
    let existing = match playlist.tidal_uuid.as_deref() {
        Some(uuid) => match api.playlist(uuid).await {
            Ok(found) => Some(found),
            Err(e) if e.is_not_found() => None,
            Err(e) => return Err(e),
        },
        None => None,
    };
    let created = existing.is_none();
    let uuid = match existing {
        Some(found) => {
            if found.title != playlist.name || found.description.as_deref().unwrap_or_default() != playlist.description
            {
                api.update_playlist(&found.uuid, &playlist.name, &playlist.description).await?;
            }
            let items = usize::try_from(found.number_of_tracks.saturating_add(found.number_of_videos)).unwrap_or(0);
            clear(api, &found.uuid, items).await?;
            found.uuid
        }
        None => api.create_playlist(&playlist.name, &playlist.description).await?.uuid,
    };
    playlist.tidal_uuid = Some(uuid.clone());
    for chunk in track_ids.chunks(TIDAL_ITEMS_PER_CALL) {
        api.add_to_playlist(&uuid, chunk).await?;
    }
    Ok(TidalSaveReport { uuid, created, saved: track_ids.len(), skipped })
}

/// Saves the stored playlist `id` to TIDAL (see [`save_to_tidal`]) and records its TIDAL copy
/// in `store`, even when adding the songs failed halfway. One save runs at a time, so quick
/// edits never make two copies. Returns the playlist as stored now.
pub async fn save_stored_to_tidal(
    api: &TidalApi,
    store: &PlaylistStore,
    id: &str,
) -> Result<(HomePlaylist, TidalSaveReport)> {
    let _one_at_a_time = SYNC_LOCK.lock().await;
    let mut playlist = store.load(id)?;
    let before = playlist.tidal_uuid.clone();
    let result = save_to_tidal(api, &mut playlist).await;
    if playlist.tidal_uuid != before {
        // the rest of the file may have changed meanwhile: only the uuid is written
        store.set_tidal_uuid(id, playlist.tidal_uuid.clone())?;
    }
    let report = result?;
    let stored = store.load(id).unwrap_or(playlist);
    Ok((stored, report))
}

/// Deletes the TIDAL copy of `playlist`. `false` when it had none or it was already gone.
pub async fn delete_from_tidal(api: &TidalApi, playlist: &HomePlaylist) -> Result<bool> {
    let Some(uuid) = playlist.tidal_uuid.as_deref() else {
        return Ok(false);
    };
    match api.delete_playlist(uuid).await {
        Ok(()) => Ok(true),
        Err(e) if e.is_not_found() => Ok(false),
        Err(e) => Err(e),
    }
}

/// Removes the first `items` items, from the end so the indices of the rest stay valid.
async fn clear(api: &TidalApi, uuid: &str, items: usize) -> Result<()> {
    let mut remaining = items;
    while remaining > 0 {
        let start = remaining.saturating_sub(TIDAL_ITEMS_PER_CALL);
        let indices: Vec<usize> = (start..remaining).collect();
        api.remove_many_from_playlist(uuid, &indices).await?;
        remaining = start;
    }
    Ok(())
}

/// The Tidal track ids of `entries`, in order and each once, and the names of the songs
/// that have none.
async fn tidal_tracks(api: &TidalApi, entries: &[PlaylistEntry]) -> (Vec<String>, Vec<String>) {
    // the futures are made up front (not in a stream closure) so the whole future stays `Send`
    let lookups: Vec<_> = entries.iter().map(|e| tidal_track_of(api, e)).collect();
    let found: Vec<Option<String>> = futures::stream::iter(lookups).buffered(RESOLVE_PARALLEL).collect().await;
    let mut seen = HashSet::with_capacity(found.len());
    let mut ids = Vec::with_capacity(found.len());
    let mut skipped = Vec::new();
    for (entry, id) in entries.iter().zip(found) {
        match id {
            Some(id) if seen.insert(id.clone()) => ids.push(id),
            Some(_) => {}
            None => skipped.push(song_name(entry)),
        }
    }
    (ids, skipped)
}

fn song_name(entry: &PlaylistEntry) -> String {
    if entry.artist.trim().is_empty() {
        entry.title.clone()
    } else {
        format!("{} · {}", entry.title, entry.artist)
    }
}

/// The entry's Tidal id; else the streamable track with its ISRC; else the best match of its
/// title and artist. A failed or slow lookup counts as not on TIDAL.
async fn tidal_track_of(api: &TidalApi, entry: &PlaylistEntry) -> Option<String> {
    if let Some(id) = entry.tidal_id.as_deref().map(str::trim).filter(|id| !id.is_empty()) {
        return Some(id.to_owned());
    }
    if let Some(isrc) = entry.isrc.as_deref().map(str::trim).filter(|i| !i.is_empty()) {
        match tokio::time::timeout(RESOLVE_TIMEOUT, api.tracks_by_isrc(isrc)).await {
            Ok(Ok(tracks)) => {
                if let Some(track) = tracks.into_iter().find(|t| t.allow_streaming) {
                    return Some(track.id.to_string());
                }
            }
            Ok(Err(e)) => tracing::debug!("looking up ISRC {isrc} failed: {e}"),
            Err(_) => tracing::debug!("looking up ISRC {isrc} timed out"),
        }
    }
    if entry.title.trim().is_empty() {
        return None;
    }
    let candidate =
        SongCandidate { title: entry.title.clone(), artist: entry.artist.clone(), ..SongCandidate::default() };
    best_on_tidal(api, &candidate).await.ok().flatten().map(|(track, _)| track.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(created: bool, saved: usize, skipped: usize) -> TidalSaveReport {
        TidalSaveReport { uuid: "u".into(), created, saved, skipped: vec!["x".into(); skipped] }
    }

    #[test]
    fn summaries_in_both_languages() {
        assert_eq!(report(true, 24, 2).summary(Lang::En), "Saved 24 songs to TIDAL · 2 aren't on TIDAL");
        assert_eq!(report(true, 1, 0).summary(Lang::En), "Saved 1 song to TIDAL");
        assert_eq!(report(false, 10, 1).summary(Lang::En), "Updated on TIDAL: 10 songs · 1 isn't on TIDAL");
        assert_eq!(report(true, 24, 2).summary(Lang::Es), "24 canciones guardadas en TIDAL · 2 no están en TIDAL");
        assert_eq!(report(false, 1, 1).summary(Lang::Es), "Actualizada en TIDAL: 1 canción · 1 no está en TIDAL");
    }

    #[test]
    fn songs_are_named_with_their_artist() {
        let e = PlaylistEntry { title: "Blind".into(), artist: "Korn".into(), ..PlaylistEntry::default() };
        assert_eq!(song_name(&e), "Blind · Korn");
        let bare = PlaylistEntry { title: "Intro".into(), ..PlaylistEntry::default() };
        assert_eq!(song_name(&bare), "Intro");
    }
}
