//! Matching song names to real things: a file in the library and/or a Tidal
//! track. Anything below [`MIN_CONFIDENCE`] stays unmatched, which is the guard
//! against songs an AI made up.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures::StreamExt;

use super::model::{EntryTag, PlaylistEntry, SongCandidate};
use crate::api::TidalApi;
use crate::models::Track;
use crate::text::{fold, song_key};

/// Below this a match is dropped ("Not found").
pub const MIN_CONFIDENCE: f32 = 0.6;
/// Tidal searches running at once.
pub const RESOLVE_PARALLEL: usize = 4;
/// Longest wait for one song's search.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(8);

const TITLE_SAME: f32 = 0.6;
const TITLE_CONTAINED: f32 = 0.35;
/// Titles shorter than this are never matched by containment ("Go" is in everything).
const MIN_CONTAINED_LEN: usize = 4;
const ARTIST_SAME: f32 = 0.3;
/// Karaoke, tributes, demos and the like are almost never what was asked for.
const NOT_STUDIO_PENALTY: f32 = 0.4;
/// A live recording is a ranking tiebreak, not a rejection: a song that only exists live still resolves.
const LIVE_PENALTY: f32 = 0.15;
const POPULARITY_WEIGHT: f32 = 0.05;
const POPULARITY_MAX: f32 = 100.0;
const CERTAIN: f32 = 1.0;
const MS_PER_SECOND: u64 = 1000;
/// Words that make a release something other than the studio song.
const NOT_STUDIO: [&str; 5] = ["demo", "karaoke", "tribute", "instrumental", "cover"];
/// Live recordings: the real song, so they lose to the studio take but still count.
const LIVE: [&str; 3] = ["live", "en vivo", "unplugged"];
/// Where a credit of several artists is cut to get the first one.
const ARTIST_SEPARATORS: [&str; 5] = [";", "/", ",", " & ", " feat"];

/// A song file the user owns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryRef {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub isrc: Option<String>,
    pub duration_ms: u64,
}

/// The library, looked up by ISRC or by artist and song title.
#[derive(Debug, Default)]
pub struct LibraryIndex {
    tracks: Vec<LibraryRef>,
    by_isrc: HashMap<String, usize>,
    by_song: HashMap<(String, String), usize>,
}

/// The first artist of a credit ("A feat. B", "A & B", "A, B").
fn primary_artist(credit: &str) -> &str {
    let lower = credit.to_lowercase();
    let cut = ARTIST_SEPARATORS.iter().filter_map(|s| lower.find(s)).min().unwrap_or(credit.len());
    // `to_lowercase` can change byte lengths, so only cut where `credit` agrees.
    credit.get(..cut).unwrap_or(credit)
}

pub(crate) fn artist_key(credit: &str) -> String {
    fold(primary_artist(credit))
}

impl LibraryIndex {
    /// The first entry wins when two files share a key.
    pub fn new(tracks: Vec<LibraryRef>) -> Self {
        let mut by_isrc = HashMap::with_capacity(tracks.len());
        let mut by_song = HashMap::with_capacity(tracks.len());
        for (i, t) in tracks.iter().enumerate() {
            if let Some(isrc) = t.isrc.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                by_isrc.entry(isrc.to_uppercase()).or_insert(i);
            }
            by_song.entry((artist_key(&t.artist), song_key(&t.title))).or_insert(i);
        }
        Self { tracks, by_isrc, by_song }
    }

    pub fn by_isrc(&self, isrc: &str) -> Option<&LibraryRef> {
        self.by_isrc.get(&isrc.trim().to_uppercase()).map(|&i| &self.tracks[i])
    }

    pub fn find(&self, artist: &str, title: &str) -> Option<&LibraryRef> {
        self.by_song.get(&(artist_key(artist), song_key(title))).map(|&i| &self.tracks[i])
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

/// Lower case words separated by single spaces, padded, to find whole words.
fn padded_words(s: &str) -> String {
    let words: Vec<String> =
        s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect();
    format!(" {} ", words.join(" "))
}

fn track_artist_keys(track: &Track) -> Vec<String> {
    track.artist.iter().chain(track.artists.iter()).map(|a| artist_key(&a.name)).collect()
}

/// How sure we are that `track` is the song asked for, 0..=~1.3. Pure.
pub fn score(candidate: &SongCandidate, track: &Track) -> f32 {
    let wanted = song_key(&candidate.title);
    let found = song_key(&track.title);
    let mut points = 0.0;
    if wanted == found {
        points += TITLE_SAME;
    } else if wanted.len() >= MIN_CONTAINED_LEN
        && found.len() >= MIN_CONTAINED_LEN
        && (wanted.contains(&found) || found.contains(&wanted))
    {
        points += TITLE_CONTAINED;
    }
    let artist = artist_key(&candidate.artist);
    if !artist.is_empty() && track_artist_keys(track).contains(&artist) {
        points += ARTIST_SAME;
    }
    let asked = padded_words(&candidate.title);
    let release = padded_words(&format!(
        "{} {} {}",
        track.version.as_deref().unwrap_or_default(),
        track.album.title.as_deref().unwrap_or_default(),
        track.title
    ));
    let has_unasked = |words: &[&str]| {
        words.iter().any(|w| {
            let needle = format!(" {w} ");
            release.contains(&needle) && !asked.contains(&needle)
        })
    };
    if has_unasked(&NOT_STUDIO) {
        points -= NOT_STUDIO_PENALTY;
    } else if has_unasked(&LIVE) {
        points -= LIVE_PENALTY;
    }
    points + POPULARITY_WEIGHT * (track.popularity.unwrap_or(0).clamp(0, POPULARITY_MAX as i64) as f32 / POPULARITY_MAX)
}

/// A playlist entry from a Tidal track. With `prefer_library`, a file of the
/// library (same ISRC, or same artist and title) becomes its `local_path`.
pub fn entry_from_track(
    track: &Track,
    library: &LibraryIndex,
    prefer_library: bool,
    tag: EntryTag,
    note: String,
) -> PlaylistEntry {
    let artist = track.artist.as_ref().or(track.artists.first()).map(|a| a.name.clone()).unwrap_or_default();
    let isrc = track.isrc.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_uppercase);
    let local_path = prefer_library
        .then(|| {
            isrc.as_deref()
                .and_then(|i| library.by_isrc(i))
                .or_else(|| library.find(&artist, &track.title))
                .map(|l| l.path.clone())
        })
        .flatten();
    PlaylistEntry {
        title: track.title.clone(),
        album: track.album.title.clone().unwrap_or_default(),
        tidal_id: Some(track.id.to_string()),
        isrc,
        local_path,
        duration_ms: u64::try_from(track.duration).unwrap_or(0) * MS_PER_SECOND,
        explicit: track.explicit,
        cover_url: crate::catalog::cover_url(track.album.cover.as_deref()),
        tag,
        note,
        artist,
    }
}

fn entry_from_library(lib: &LibraryRef, tag: EntryTag, note: String) -> PlaylistEntry {
    PlaylistEntry {
        title: lib.title.clone(),
        artist: lib.artist.clone(),
        album: lib.album.clone(),
        isrc: lib.isrc.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_uppercase),
        local_path: Some(lib.path.clone()),
        duration_ms: lib.duration_ms,
        tag,
        note,
        ..PlaylistEntry::default()
    }
}

/// Remembers entries by Tidal id and library path: a file and its Tidal twin count once.
#[derive(Debug, Default)]
pub struct SeenEntries {
    keys: HashSet<String>,
}

impl SeenEntries {
    /// `true` when neither the Tidal id nor the path of `entry` was seen; both are remembered either way.
    pub fn first_time(&mut self, entry: &PlaylistEntry) -> bool {
        let keys = [entry.tidal_id.as_deref(), entry.local_path.as_deref()];
        let fresh = keys.iter().flatten().all(|k| !self.keys.contains(*k));
        for key in keys.iter().flatten() {
            self.keys.insert((*key).to_owned());
        }
        fresh
    }
}

/// A song asked for and what it was matched to.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub candidate: SongCandidate,
    /// `None` = not found (below [`MIN_CONFIDENCE`]).
    pub entry: Option<PlaylistEntry>,
    pub confidence: f32,
    /// Tidal's search failed or timed out (not the same as "no match"), and the library had no copy.
    pub failed: bool,
}

pub struct Resolver<'a> {
    /// `None` = not signed in to Tidal: only library songs resolve.
    pub api: Option<&'a TidalApi>,
    pub library: &'a LibraryIndex,
    /// Local copies play instead of streaming.
    pub prefer_library: bool,
}

impl Resolver<'_> {
    /// Same order as `candidates`, [`RESOLVE_PARALLEL`] searches at a time.
    /// `progress(done, total)` is called as each song finishes. A failed or
    /// slow search leaves that song unmatched; it never fails the batch.
    pub async fn resolve_all(
        &self,
        candidates: Vec<SongCandidate>,
        progress: &(dyn Fn(usize, usize) + Send + Sync),
    ) -> Vec<Resolved> {
        let total = candidates.len();
        let done = AtomicUsize::new(0);
        let done = &done;
        futures::stream::iter(candidates)
            .map(|candidate| async move {
                let resolved = self.resolve_one(candidate).await;
                progress(done.fetch_add(1, Ordering::Relaxed) + 1, total);
                resolved
            })
            .buffered(RESOLVE_PARALLEL)
            .collect()
            .await
    }

    async fn resolve_one(&self, candidate: SongCandidate) -> Resolved {
        let library_hit = self.library.find(&candidate.artist, &candidate.title);
        let (tidal, search_failed) = match self.api {
            Some(api) => match self.best_on_tidal(api, &candidate).await {
                Ok(best) => (best, false),
                Err(()) => (None, true),
            },
            None => (None, false),
        };
        let entry = match (tidal.as_ref(), library_hit) {
            (Some((track, _)), lib) => {
                let mut entry =
                    entry_from_track(track, self.library, self.prefer_library, candidate.tag, candidate.note.clone());
                if entry.local_path.is_none() && self.prefer_library {
                    entry.local_path = lib.map(|l| l.path.clone());
                }
                Some(entry)
            }
            (None, Some(lib)) => Some(entry_from_library(lib, candidate.tag, candidate.note.clone())),
            (None, None) => None,
        };
        let tidal_score = tidal.as_ref().map_or(0.0, |(_, s)| *s);
        let confidence = if library_hit.is_some() { CERTAIN } else { tidal_score.min(CERTAIN) };
        let failed = search_failed && entry.is_none();
        Resolved { candidate, entry, confidence, failed }
    }

    /// The best studio match on Tidal and its score, if it reaches [`MIN_CONFIDENCE`];
    /// `Err` when the search itself failed or timed out.
    async fn best_on_tidal(&self, api: &TidalApi, candidate: &SongCandidate) -> Result<Option<(Track, f32)>, ()> {
        let query = format!("{} {}", candidate.artist, candidate.title);
        let found = match tokio::time::timeout(RESOLVE_TIMEOUT, api.search(&query)).await {
            Ok(Ok(found)) => found,
            Ok(Err(e)) => {
                tracing::warn!("searching Tidal for “{}” by {} failed: {e}", candidate.title, candidate.artist);
                return Err(());
            }
            Err(_) => {
                tracing::warn!("searching Tidal for “{}” by {} timed out", candidate.title, candidate.artist);
                return Err(());
            }
        };
        let top = found
            .top_hit
            .filter(|h| h.kind.eq_ignore_ascii_case("TRACKS"))
            .and_then(|h| serde_json::from_value::<Track>(h.value).ok());
        let mut seen = std::collections::HashSet::new();
        Ok(found
            .tracks
            .items
            .into_iter()
            .chain(top)
            .filter(|t| t.allow_streaming && seen.insert(t.id))
            .map(|t| {
                let s = score(candidate, &t);
                (t, s)
            })
            .filter(|(_, s)| *s >= MIN_CONFIDENCE)
            .reduce(|best, next| if is_better(&next, &best) { next } else { best }))
    }
}

/// Higher score wins; on a tie the earliest release (the original).
fn is_better(next: &(Track, f32), best: &(Track, f32)) -> bool {
    match next.1.total_cmp(&best.1) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match (&next.0.stream_start_date, &best.0.stream_start_date) {
            (Some(n), Some(b)) => n < b,
            (Some(_), None) => true,
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AlbumRef, ArtistRef};

    fn track(id: i64, title: &str, artist: &str, album: &str, popularity: i64) -> Track {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "duration": 200, "popularity": popularity,
            "artist": {"id": 1, "name": artist, "type": "MAIN"},
            "artists": [{"id": 1, "name": artist, "type": "MAIN"}],
            "album": {"id": 5, "title": album}
        }))
        .unwrap()
    }

    fn want(title: &str, artist: &str) -> SongCandidate {
        SongCandidate { title: title.into(), artist: artist.into(), ..SongCandidate::default() }
    }

    #[test]
    fn studio_beats_live_beats_karaoke() {
        let c = want("Freak On A Leash", "Korn");
        let studio = score(&c, &track(1, "Freak On A Leash", "Korn", "Follow the Leader", 70));
        let live = score(&c, &track(2, "Freak On a Leash (Live)", "Korn", "MTV Unplugged", 70));
        let karaoke = score(&c, &track(3, "Freak On A Leash", "Karaoke Band", "Karaoke Hits", 70));
        let tribute = score(&c, &track(4, "Freak On A Leash", "Some Tribute", "A Tribute to Korn", 70));
        assert!(studio >= MIN_CONFIDENCE && studio > live);
        assert!(live > karaoke && live > tribute);
        assert!(karaoke < MIN_CONFIDENCE);
    }

    #[test]
    fn a_live_only_song_still_resolves_and_studio_wins_when_both_exist() {
        let c = want("Freak On A Leash", "Korn");
        let live = track(2, "Freak On a Leash (Live)", "Korn", "MTV Unplugged", 0);
        assert!(score(&c, &live) >= MIN_CONFIDENCE, "live-only must resolve");
        let studio = track(1, "Freak On A Leash", "Korn", "Follow the Leader", 0);
        assert!(score(&c, &studio) > score(&c, &live));
        // a live title by another artist is still rejected
        assert!(score(&c, &track(3, "Freak On a Leash (Live)", "Cover Band", "Live", 0)) < MIN_CONFIDENCE);
    }

    #[test]
    fn an_invented_song_is_below_the_bar() {
        let c = want("Dreaming of Orange Skies", "Korn");
        assert!(score(&c, &track(1, "Freak On A Leash", "Korn", "Follow the Leader", 90)) < MIN_CONFIDENCE);
        assert!(score(&c, &track(2, "Dreaming", "Someone Else", "Other", 90)) < MIN_CONFIDENCE);
    }

    #[test]
    fn a_cover_is_found_under_the_original_artist() {
        let c = want("Wicked Game", "Chris Isaak");
        assert!(score(&c, &track(1, "Wicked Game", "Chris Isaak", "Heart Shaped World", 50)) >= MIN_CONFIDENCE);
    }

    #[test]
    fn live_words_in_the_request_are_not_penalised() {
        let c = want("One (Live)", "Metallica");
        let t = track(1, "One (Live)", "Metallica", "Live Shit", 10);
        assert!(score(&c, &t) >= MIN_CONFIDENCE);
    }

    #[test]
    fn first_artist_of_a_credit() {
        assert_eq!(primary_artist("Korn feat. Skrillex"), "Korn");
        assert_eq!(primary_artist("A & B"), "A");
        assert_eq!(primary_artist("A;B"), "A");
        assert_eq!(primary_artist("Tom Petty"), "Tom Petty");
    }

    fn lib(path: &str, artist: &str, title: &str, isrc: Option<&str>) -> LibraryRef {
        LibraryRef {
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            album: "Album".into(),
            isrc: isrc.map(str::to_owned),
            duration_ms: 1000,
        }
    }

    #[test]
    fn library_lookup_by_isrc_and_by_song() {
        let index = LibraryIndex::new(vec![
            lib("/m/a.flac", "Korn feat. X", "Blind (Remastered)", Some("us123")),
            lib("/m/dup.flac", "Korn", "Blind", None),
        ]);
        assert_eq!(index.len(), 2);
        assert_eq!(index.by_isrc("US123").map(|l| l.path.as_str()), Some("/m/a.flac"));
        assert_eq!(index.find("KORN", "Blind - Live").map(|l| l.path.as_str()), Some("/m/a.flac"));
        assert!(index.find("Korn", "Other").is_none());
        assert!(LibraryIndex::default().is_empty());
    }

    #[test]
    fn entries_from_tracks_carry_the_details() {
        let mut t = track(9, "Blind", "Korn", "Korn", 10);
        t.isrc = Some("ussm19400123".into());
        t.album = AlbumRef { cover: Some("aaaa-bbbb".into()), title: Some("Korn".into()), ..AlbumRef::default() };
        t.artists = vec![ArtistRef { id: 1, name: "Korn".into(), kind: None, picture: None }];
        let index = LibraryIndex::new(vec![lib("/m/blind.flac", "Korn", "Blind", Some("USSM19400123"))]);
        let e = entry_from_track(&t, &index, true, EntryTag::Always, "n".into());
        assert_eq!((e.tidal_id.as_deref(), e.isrc.as_deref()), (Some("9"), Some("USSM19400123")));
        assert_eq!((e.local_path.as_deref(), e.duration_ms), (Some("/m/blind.flac"), 200_000));
        assert!(e.cover_url.is_some());
        assert!(entry_from_track(&t, &index, false, EntryTag::Always, String::new()).local_path.is_none());
    }
}
