//! Builds a ride: the workout (a style at a length, or the rider's / the AI's own), the songs whose tempo
//! fits each segment, and the playlist that plays them. Nothing is saved here and nothing is written to
//! Tidal (read-only calls only).
//!
//! Candidates come from the library (the `BPM` tag), the AI's proposals resolved on Tidal and the library,
//! or Tidal's search by genre; a tempo is the tag, then Tidal's, then HomeFi's own analysis within the
//! budget. Each segment takes the best fit; when none fits the ladder of [`super::bpm::ladder`] widens.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::ai::{propose_songs, Proposal};
use super::bpm::{fit, ladder, BpmSource, Fit, Rung, Tolerance};
use super::model::{
    round_minutes, MusicWish, Popularity, RideRequest, RideTaste, Segment, Workout, MAX_ROUNDS, MAX_SEGMENTS,
};
use super::presets::{beginner_caps, preset};
use super::profile::{profile, ProfilePoint};
use super::settings::{ExerciseSettings, SourcesMode};
use super::words::{self, Problem};
use crate::ai::AiConfig;
use crate::api::TidalApi;
use crate::dj::{DjAnalysis, Grid};
use crate::error::Result;
use crate::humane::Lang;
use crate::models::Track;
use crate::playlist::{
    artist_key, entry_from_library, entry_from_track, new_id, now_ms, popularity_0_100, EntryTag, HomePlaylist,
    LibraryIndex, PlaylistEntry, PlaylistOrigin, Resolver, RideOrigin, SeenEntries, SongCandidate,
    PLAYLIST_FORMAT_VERSION, RESOLVE_PARALLEL, RESOLVE_TIMEOUT,
};
use crate::text::fold;

/// The whole build (AI, Tidal, analysis) must finish within this.
pub const BUILD_DEADLINE: Duration = Duration::from_secs(150);
/// A song may be this much shorter than its segment (the segment then follows the song); a longer song is
/// cut at the segment's end (`PlannedSong::play_ms`).
const DURATION_SLACK: f32 = 0.2;
/// Library songs without a tempo that stay candidates for analysis.
const MAX_UNKNOWN_KEPT: usize = 80;
/// A tempo read from the audio agrees with the known one within this share (any octave).
const GRID_AGREE: f32 = 0.03;
const BEATS_PER_BAR: f64 = 8.0;
const MS_PER_S: u64 = 1000;

/// A build's memory is reused for this long ("new songs" right after a build); older, everything is found again.
const MEMORY_TTL: Duration = Duration::from_secs(30 * 60);
/// An artist name shorter than this never matches another by containment ("U2" is in many names).
const MIN_ARTIST_CONTAINED_CHARS: usize = 4;
const POPULARITY_MAX: f32 = 100.0;

// score weights, in RPM-equivalents (fame by popularity: see `fame_weights`)
/// A song by an artist the rider likes.
const LIKED_ARTIST_BONUS: f32 = 1.0;
/// A song an earlier build of the same ride played: it comes back only when nothing else fits.
const PLAYED_PENALTY: f32 = 50.0;
const MIX_REPEAT_PENALTY: f32 = 1.5;
const SAME_ARTIST_PENALTY: f32 = 2.0;
const CENTER_WEIGHT: f32 = 0.05;
const DURATION_WEIGHT: f32 = 2.0;
const ANALYSIS_PENALTY: f32 = 0.3;
const TIDAL_PENALTY: f32 = 0.1;

/// Reads the tempo and beat grid of a song's audio. The app implements it (decoding needs its own
/// audio stack and the DJ analysis cache); `None` when the song cannot be read.
pub trait Analyzer: Send + Sync {
    fn analyze<'a>(&'a self, entry: &'a PlaylistEntry)
        -> Pin<Box<dyn Future<Output = Option<DjAnalysis>> + Send + 'a>>;

    /// Whether [`Self::analyze`] could read this song at all (a file it can open), checked without decoding so
    /// songs it cannot read do not use up the analysis budget.
    fn can_read(&self, _entry: &PlaylistEntry) -> bool {
        true
    }
}

/// What the build works with. Borrowed: the app owns all of it.
pub struct BuildContext<'a> {
    /// Clamped inside.
    pub settings: &'a ExerciseSettings,
    /// `None` = no AI set up: genres drive the search.
    pub ai: Option<&'a AiConfig>,
    /// `None` = not signed in to Tidal (or library only).
    pub api: Option<&'a TidalApi>,
    pub library: &'a LibraryIndex,
    /// `None` = no analysis: only tags and Tidal's tempo are known.
    pub analyzer: Option<&'a dyn Analyzer>,
    /// Language of notes, names and fallback text.
    pub lang: Lang,
}

/// Where the pulse comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PulseSource {
    /// The song's own beat grid.
    SongBeat,
    #[default]
    #[serde(other)]
    Metronome,
}

/// Cue timing frozen into the plan, so the clock needs no settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CueSettings {
    pub countdown_seconds: u8,
    pub notice_seconds: u8,
}

impl Default for CueSettings {
    fn default() -> Self {
        Self { countdown_seconds: 3, notice_seconds: 10 }
    }
}

/// The song of one segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlannedSong {
    /// The segment of `RidePlan::workout` this song plays for.
    pub segment_index: usize,
    pub entry: PlaylistEntry,
    pub bpm: Option<u16>,
    pub source: Option<BpmSource>,
    /// How the rider pedals to it; `None` for a song picked with no tempo (metronome).
    pub fit: Option<Fit>,
    pub grid: Option<Grid>,
    pub pulse: PulseSource,
    /// The song is cut here (ms of the song): the segment ends and the app must go to the next song at this
    /// position. `None` = the song plays to its end (it is not longer than its segment).
    pub play_ms: Option<u64>,
}

/// The ride: segments snapped to the songs, one song per segment, in play order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RidePlan {
    pub version: u32,
    pub workout: Workout,
    pub songs: Vec<PlannedSong>,
    /// Why the ride isn't exactly what was asked, already in the build language.
    pub notes: Vec<String>,
    pub cues: CueSettings,
    pub wish: MusicWish,
    /// The course profile of `workout`, drawn on the ride screen and the TV.
    pub profile: Vec<ProfilePoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RideBuilt {
    pub plan: RidePlan,
    /// The playlist the player plays (same order as `plan.songs`); not saved: the app saves it on start.
    pub playlist: HomePlaylist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RideProgress {
    Searching { with_ai: bool },
    Resolving { done: usize, total: usize },
    Analyzing { done: usize, total: usize },
    Matching,
}

type Progress<'a> = &'a (dyn Fn(RideProgress) + Send + Sync);

/// Builds the ride for `request`. Dropping the future cancels it between songs.
pub async fn build(request: &RideRequest, ctx: &BuildContext<'_>, progress: Progress<'_>) -> Result<RideBuilt> {
    build_with_memory(request, ctx, progress, None).await.map(|(built, _)| built)
}

/// [`build`] that remembers what it found. Given the memory of the previous build of the same inputs (not
/// older than [`MEMORY_TTL`]), it searches nothing again: it plays songs the earlier builds did not, asking
/// the AI only for the segments with nothing new left. Any other memory is ignored. The answer carries the
/// memory for the next build; the caller keeps it (a cancelled build leaves the caller's copy as it was).
pub async fn build_with_memory(
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    memory: Option<BuildMemory>,
) -> Result<(RideBuilt, BuildMemory)> {
    build_within(request, ctx, progress, memory, BUILD_DEADLINE).await
}

pub(crate) async fn build_within(
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    memory: Option<BuildMemory>,
    deadline: Duration,
) -> Result<(RideBuilt, BuildMemory)> {
    tokio::time::timeout(deadline, build_inner(request, ctx, progress, memory))
        .await
        .unwrap_or_else(|_| Err(words::problem(ctx.lang, Problem::TooSlow)))
}

/// A song that could be in the ride.
#[derive(Debug, Clone)]
struct Candidate {
    entry: PlaylistEntry,
    /// [`artist_key`] of the entry's artist, worked out once.
    artist: String,
    bpm: Option<(f32, BpmSource)>,
    grid: Option<Grid>,
    /// The wish genre that found it (empty when unknown).
    genre: String,
    /// The segment (of the asked workout, before any split) the AI proposed it for.
    proposed_for: Option<usize>,
    /// Its place in the AI's list for that segment (0 = the best-known song).
    rank: usize,
    /// Tidal's popularity, 0..=100, when known.
    popularity: Option<u8>,
    /// The audio was read already (whatever came of it): never twice.
    analyzed: bool,
}

impl Candidate {
    fn new(entry: PlaylistEntry, bpm: Option<(f32, BpmSource)>, genre: String) -> Self {
        let artist = artist_key(&entry.artist);
        Self { entry, artist, bpm, grid: None, genre, proposed_for: None, rank: 0, popularity: None, analyzed: false }
    }

    fn duration_fits(&self, seg: &Segment) -> bool {
        let wanted = seg.seconds as f32 * MS_PER_S as f32;
        // a longer song is cut to the segment; a shorter one must nearly cover it
        self.entry.duration_ms == 0 || self.entry.duration_ms as f32 >= wanted * (1.0 - DURATION_SLACK)
    }

    /// Its Tidal id and library path: how an earlier build's song is recognized.
    fn keys(&self) -> impl Iterator<Item = &str> {
        [self.entry.tidal_id.as_deref(), self.entry.local_path.as_deref()].into_iter().flatten()
    }
}

/// Everything found so far. Songs by an avoided artist never get in.
#[derive(Debug, Clone)]
struct Pool {
    songs: Vec<Candidate>,
    seen: SeenEntries,
    avoid: HashSet<String>,
}

impl Pool {
    fn new(taste: &RideTaste) -> Self {
        let avoid = taste.avoid.iter().map(|a| artist_key(a)).filter(|a| !a.is_empty()).collect();
        Self { songs: Vec::new(), seen: SeenEntries::default(), avoid }
    }

    fn add(&mut self, candidate: Candidate) {
        if !self.avoid.contains(&candidate.artist) && self.seen.first_time(&candidate.entry) {
            self.songs.push(candidate);
        }
    }
}

/// What a build found, kept by the app so "new songs" for the same ride reuses the songs already found
/// (no new search) and plays different ones. The app holds the newest one and passes it to the next build
/// ([`build_with_memory`]); a build for other inputs, or after [`MEMORY_TTL`], starts afresh.
#[derive(Debug, Clone)]
pub struct BuildMemory {
    /// The inputs it was found for ([`memory_key`]).
    key: String,
    made: Instant,
    pool: Pool,
    /// Notes of the gathering (no AI, the AI failed, no Tidal), said again by every build from it.
    notes: Vec<String>,
    /// Every song the AI proposed for this ride, so a rebuild does not ask for them again.
    asked: Vec<SongCandidate>,
    /// Tidal ids and library paths of the songs earlier builds of this ride played.
    played: HashSet<String>,
}

impl BuildMemory {
    fn usable_for(&self, key: &str) -> bool {
        self.key == key && self.made.elapsed() < MEMORY_TTL
    }
}

fn positive_bpm(b: i64) -> Option<f32> {
    (b > 0 && b < i64::from(u16::MAX)).then_some(b as f32)
}

/// The inputs that decide which songs are found: the request (taste included), the settings the search
/// reads, the AI, whether Tidal and the library are there, and the language of the notes.
fn memory_key(request: &RideRequest, wish: &MusicWish, settings: &ExerciseSettings, ctx: &BuildContext<'_>) -> String {
    serde_json::json!({
        "minutes": request.minutes,
        "style": request.style,
        "workout": request.workout,
        "wish": wish,
        "settings": [
            serde_json::json!(settings.beginner_mode), serde_json::json!(settings.cadence_tolerance_rpm),
            serde_json::json!(settings.allow_half_time), serde_json::json!(settings.allow_double_time),
            serde_json::json!(settings.sources), serde_json::json!(settings.analysis_budget),
            serde_json::json!(settings.allow_explicit),
        ],
        "ai": ctx.ai.map(|a| (a.provider.id(), a.model.as_str())),
        "tidal": ctx.api.is_some(),
        "library": ctx.library.len(),
        "lang": matches!(ctx.lang, Lang::Es),
    })
    .to_string()
}

async fn build_inner(
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    memory: Option<BuildMemory>,
) -> Result<(RideBuilt, BuildMemory)> {
    let lang = ctx.lang;
    let settings = ctx.settings.clamped();
    let api = ctx.api.filter(|_| settings.sources != SourcesMode::LibraryOnly);
    if api.is_none() && ctx.library.is_empty() {
        return Err(words::problem(lang, Problem::NothingToUse));
    }
    let minutes = round_minutes(request.minutes);
    let asked_workout = match &request.workout {
        Some(w) if w.segments.is_empty() => return Err(words::problem(lang, Problem::NoRide)),
        Some(w) => beginner_caps(&w.clone().sanitized(), &settings),
        None => preset(request.style, minutes, &settings),
    };
    let wish = request.wish.clone().clamped();
    let taste = &wish.taste;
    let tolerance = Tolerance::from_settings(&settings);
    let prefer_library = settings.sources != SourcesMode::TidalFirst;
    let mut budget = usize::from(settings.analysis_budget).min(if ctx.analyzer.is_some() { usize::MAX } else { 0 });
    let key = memory_key(request, &wish, &settings, ctx);
    let gatherer = Gatherer { ctx, api, wish: &wish, tolerance: &tolerance, settings: &settings, prefer_library };

    let reused = memory.filter(|m| m.usable_for(&key));
    let again = reused.is_some();
    let BuildMemory { mut pool, notes: gathered, mut asked, played, .. } = match reused {
        Some(memory) => memory,
        None => gatherer.gather(key.clone(), &asked_workout, progress).await,
    };
    let mut notes = gathered.clone();
    let liked: HashSet<String> = taste.artists.iter().map(|a| artist_key(a)).filter(|a| !a.is_empty()).collect();
    if again {
        // "new songs": the parts with nothing fresh left ask the AI for more, the rest come from what was found
        let known = known_artists(&asked, &liked);
        let fresh = Freshness { played: &played, known: &known, taste, allow_explicit: settings.allow_explicit };
        gatherer.top_up(&mut pool, &mut asked, &asked_workout, &fresh, progress).await;
    }
    let known = known_artists(&asked, &liked);

    // long segments become several parts when the songs found are short, so no song has to stretch
    let (workout, origin) = match typical_seconds(&pool) {
        Some(typical_s) => split_long_segments(asked_workout, typical_s),
        None => {
            let origin = (0..asked_workout.segments.len()).collect();
            (asked_workout, origin)
        }
    };
    let judge = Judge {
        weights: fame_weights(taste.popularity),
        mix: wish.mix,
        allow_explicit: settings.allow_explicit,
        known: &known,
        liked: &liked,
        played: &played,
        origin: &origin,
        // with the AI's picks for big hits, songs of artists nobody named are not worth decoding
        analyze_strangers: taste.popularity != Popularity::Hits || asked.is_empty(),
    };
    analyze_unknowns(&mut pool, &workout, ctx.analyzer, &mut budget, &judge, progress).await;

    progress(RideProgress::Matching);
    let rungs = ladder(&tolerance);
    let mut picks: Vec<Option<Pick>> = vec![None; workout.segments.len()];
    let mut searched_more = false;
    for (rung_at, rung) in rungs.iter().enumerate() {
        if rung.search_tidal && !searched_more && picks.iter().any(Option::is_none) {
            searched_more = true;
            if let Some(api) = api {
                let queries = search_queries(&wish, widen_words(taste.popularity));
                add_searches(&mut pool, api, ctx.library, prefer_library, queries).await;
                analyze_unknowns(&mut pool, &workout, ctx.analyzer, &mut budget, &judge, progress).await;
            }
        }
        select_rung(&pool.songs, &workout, &mut picks, rung, rung_at, &judge);
    }

    let built = assemble(request, &wish, workout, &mut pool, &picks, &settings, ctx, &mut budget, &mut notes).await?;
    let mut played = played;
    for pick in picks.iter().flatten() {
        played.extend(pool.songs[pick.candidate].keys().map(str::to_owned));
    }
    let memory = BuildMemory { key, made: Instant::now(), pool, notes: gathered, asked, played };
    Ok((built, memory))
}

/// Artist keys the AI proposed or the rider likes: songs of theirs are "known" ones.
fn known_artists(asked: &[SongCandidate], liked: &HashSet<String>) -> HashSet<String> {
    asked.iter().map(|c| artist_key(&c.artist)).filter(|a| !a.is_empty()).chain(liked.iter().cloned()).collect()
}

// ------------------------------------------------------------------ gathering

/// Finds the candidates of a build: the library, the AI's proposals resolved, Tidal's search.
struct Gatherer<'a> {
    ctx: &'a BuildContext<'a>,
    api: Option<&'a TidalApi>,
    wish: &'a MusicWish,
    tolerance: &'a Tolerance,
    settings: &'a ExerciseSettings,
    prefer_library: bool,
}

impl Gatherer<'_> {
    fn resolver(&self) -> Resolver<'_> {
        Resolver { api: self.api, library: self.ctx.library, prefer_library: self.prefer_library }
    }

    /// A first build: everything is searched.
    async fn gather(&self, key: String, workout: &Workout, progress: Progress<'_>) -> BuildMemory {
        let lang = self.ctx.lang;
        let mut pool = Pool::new(&self.wish.taste);
        let mut notes = Vec::new();
        let mut asked = Vec::new();
        add_library(&mut pool, self.ctx.library, self.wish);
        let mut with_ai = false;
        if let Some(ai) = self.ctx.ai {
            progress(RideProgress::Searching { with_ai: true });
            match propose_songs(ai, workout, self.wish, self.tolerance, self.settings.allow_explicit, &[]).await {
                Ok(proposals) if !proposals.is_empty() => {
                    with_ai = true;
                    asked.extend(proposals.iter().map(|p| p.candidate.clone()));
                    add_proposals(&mut pool, &self.resolver(), proposals, progress).await;
                }
                Ok(_) => notes.push(words::ai_failed_note(lang, words::ai_no_songs(lang))),
                Err(e) => {
                    tracing::warn!("{e}");
                    notes.push(words::ai_failed_note(lang, &e.humane_in(lang)));
                }
            }
        } else if !self.wish.genres.is_empty() {
            notes.push(words::no_ai_note(lang));
        }
        if !with_ai {
            if let Some(api) = self.api {
                progress(RideProgress::Searching { with_ai: false });
                let queries = search_queries(self.wish, "");
                add_searches(&mut pool, api, self.ctx.library, self.prefer_library, queries).await;
            }
        }
        if self.api.is_none() {
            notes.push(crate::concert::words::no_tidal_note(lang));
        }
        BuildMemory { key, made: Instant::now(), pool, notes, asked, played: HashSet::new() }
    }

    /// A rebuild: only the segments of `workout` with no fresh song that fits are asked of the AI again,
    /// with the songs it proposed before excluded. Without an AI, or when every segment has one, no call.
    async fn top_up(
        &self,
        pool: &mut Pool,
        asked: &mut Vec<SongCandidate>,
        workout: &Workout,
        fresh: &Freshness<'_>,
        progress: Progress<'_>,
    ) {
        let Some(ai) = self.ctx.ai else { return };
        let thin: Vec<usize> = (0..workout.segments.len())
            .filter(|&i| !pool.songs.iter().any(|c| fresh.fits(c, &workout.segments[i], self.tolerance)))
            .collect();
        if thin.is_empty() {
            return;
        }
        let part = Workout { segments: thin.iter().map(|&i| workout.segments[i].clone()).collect(), ..workout.clone() };
        progress(RideProgress::Searching { with_ai: true });
        match propose_songs(ai, &part, self.wish, self.tolerance, self.settings.allow_explicit, asked).await {
            Ok(mut proposals) => {
                for p in &mut proposals {
                    p.segment = thin[p.segment];
                }
                asked.extend(proposals.iter().map(|p| p.candidate.clone()));
                add_proposals(pool, &self.resolver(), proposals, progress).await;
            }
            // the songs found before still make a ride
            Err(e) => tracing::warn!("{e}"),
        }
    }
}

/// Is a candidate a new song worth playing on a rebuild?
struct Freshness<'a> {
    played: &'a HashSet<String>,
    known: &'a HashSet<String>,
    taste: &'a RideTaste,
    allow_explicit: bool,
}

impl Freshness<'_> {
    fn fits(&self, c: &Candidate, seg: &Segment, tolerance: &Tolerance) -> bool {
        let wanted =
            self.taste.popularity != Popularity::Hits || c.proposed_for.is_some() || self.known.contains(&c.artist);
        wanted
            && (self.allow_explicit || !c.entry.explicit)
            && !c.keys().any(|k| self.played.contains(k))
            && c.bpm.is_some_and(|(bpm, _)| fit(bpm, &seg.target, tolerance).is_some())
    }
}

/// Library songs whose genre tag names a wish genre (every song when the wish has no genre).
fn add_library(pool: &mut Pool, library: &LibraryIndex, wish: &MusicWish) {
    let wanted: Vec<(String, &String)> =
        wish.genres.iter().map(|g| (fold(g), g)).filter(|(f, _)| !f.is_empty()).collect();
    let mut unknown = 0usize;
    for lib in library.refs() {
        let genre = if wanted.is_empty() {
            Some(String::new())
        } else {
            let tag = fold(&lib.genre);
            wanted.iter().find(|(f, _)| tag.contains(f.as_str())).map(|(_, original)| (*original).clone())
        };
        let Some(genre) = genre else { continue };
        let bpm = lib.bpm.map(|b| (f32::from(b), BpmSource::Tag));
        if bpm.is_none() {
            unknown += 1;
            if unknown > MAX_UNKNOWN_KEPT {
                continue;
            }
        }
        pool.add(Candidate::new(entry_from_library(lib, EntryTag::Plain, String::new()), bpm, genre));
    }
}

/// The AI named an artist: a match by someone else (a cover band, a karaoke label) is not the song asked for.
fn same_artist(found: &str, asked: &str) -> bool {
    let (found, asked) = (artist_key(found), artist_key(asked));
    if found.is_empty() || asked.is_empty() {
        return true;
    }
    let contains = |a: &str, b: &str| b.chars().count() >= MIN_ARTIST_CONTAINED_CHARS && a.contains(b);
    found == asked || contains(&found, &asked) || contains(&asked, &found)
}

async fn add_proposals(pool: &mut Pool, resolver: &Resolver<'_>, proposals: Vec<Proposal>, progress: Progress<'_>) {
    let (places, candidates): (Vec<(usize, usize)>, Vec<SongCandidate>) =
        proposals.into_iter().map(|p| ((p.segment, p.rank), p.candidate)).unzip();
    let resolved =
        resolver.resolve_all(candidates, &|done, total| progress(RideProgress::Resolving { done, total })).await;
    for ((segment, rank), r) in places.into_iter().zip(resolved) {
        let Some(entry) = r.entry else { continue };
        if !same_artist(&entry.artist, &r.candidate.artist) {
            tracing::debug!(
                "“{}” resolved to {}, not {}: left out",
                r.candidate.title,
                entry.artist,
                r.candidate.artist
            );
            continue;
        }
        let bpm = match (r.library_bpm, r.tidal_bpm) {
            (Some(b), _) => Some((f32::from(b), BpmSource::Tag)),
            (None, Some(b)) => Some((f32::from(b), BpmSource::Tidal)),
            _ => None,
        };
        pool.add(Candidate {
            proposed_for: Some(segment),
            rank,
            popularity: r.tidal_popularity,
            ..Candidate::new(entry, bpm, String::new())
        });
    }
}

/// What the widening search adds to each genre: hits for big hits, workout music otherwise.
fn widen_words(popularity: Popularity) -> &'static str {
    match popularity {
        Popularity::Hits => "hits",
        Popularity::Mix | Popularity::Discover => "workout",
    }
}

/// `(genre, query)` pairs: the genre alone and with the rider's sentence (`extra` appended to each), then
/// each artist the rider likes.
fn search_queries(wish: &MusicWish, extra: &str) -> Vec<(String, String)> {
    let join = |parts: &[&str]| parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" ");
    let mut queries = Vec::new();
    if wish.genres.is_empty() {
        let q = join(&[wish.sentence.as_str(), extra]);
        if !q.is_empty() {
            queries.push((String::new(), q));
        }
    }
    for genre in &wish.genres {
        queries.push((genre.clone(), join(&[genre, extra])));
        if extra.is_empty() && !wish.sentence.is_empty() {
            queries.push((genre.clone(), join(&[genre, &wish.sentence])));
        }
    }
    let genre = wish.genres.first().cloned().unwrap_or_default();
    queries.extend(wish.taste.artists.iter().map(|artist| (genre.clone(), artist.clone())));
    queries
}

async fn search_tracks(api: &TidalApi, query: &str) -> Vec<Track> {
    match tokio::time::timeout(RESOLVE_TIMEOUT, api.search(query)).await {
        Ok(Ok(found)) => found.tracks.items,
        Ok(Err(e)) => {
            tracing::warn!("searching Tidal for “{query}” failed: {e}");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("searching Tidal for “{query}” timed out");
            Vec::new()
        }
    }
}

async fn add_searches(
    pool: &mut Pool,
    api: &TidalApi,
    library: &LibraryIndex,
    prefer_library: bool,
    queries: Vec<(String, String)>,
) {
    let found: Vec<(String, Vec<Track>)> = futures::stream::iter(queries)
        .map(|(genre, query)| async move { (genre, search_tracks(api, &query).await) })
        .buffered(RESOLVE_PARALLEL)
        .collect()
        .await;
    for (genre, tracks) in found {
        for track in tracks.into_iter().filter(|t| t.allow_streaming) {
            let bpm = track.bpm.and_then(positive_bpm).map(|b| (b, BpmSource::Tidal));
            let entry = entry_from_track(&track, library, prefer_library, EntryTag::Plain, String::new());
            let popularity = track.popularity.map(popularity_0_100);
            pool.add(Candidate { popularity, ..Candidate::new(entry, bpm, genre.clone()) });
        }
    }
}

/// Reads the tempo of candidates that have none, within `budget` songs: the AI's own picks first, then
/// songs of known artists, then the rest (unless the judge says strangers are not worth it).
async fn analyze_unknowns(
    pool: &mut Pool,
    workout: &Workout,
    analyzer: Option<&dyn Analyzer>,
    budget: &mut usize,
    judge: &Judge<'_>,
    progress: Progress<'_>,
) {
    let Some(analyzer) = analyzer else { return };
    let mut todo: Vec<usize> = (0..pool.songs.len())
        .filter(|&i| {
            let c = &pool.songs[i];
            c.bpm.is_none()
                && !c.analyzed
                && (judge.analyze_strangers || judge.is_known(c))
                && analyzer.can_read(&c.entry)
                && workout.segments.iter().any(|s| c.duration_fits(s))
        })
        .collect();
    todo.sort_by_key(|&i| {
        let c = &pool.songs[i];
        (c.proposed_for.is_none(), !judge.is_known(c), i)
    });
    todo.truncate(*budget);
    let total = todo.len();
    for (done, i) in todo.into_iter().enumerate() {
        *budget -= 1;
        let song = &mut pool.songs[i];
        song.analyzed = true;
        if let Some(a) = analyzer.analyze(&song.entry).await.filter(|a| a.bpm > 0.0) {
            song.bpm = Some((a.bpm as f32, BpmSource::Analysis));
            song.grid = a.grid();
        }
        progress(RideProgress::Analyzing { done: done + 1, total });
    }
}

// ------------------------------------------------------------------ choosing

#[derive(Debug, Clone, Copy)]
struct Pick {
    candidate: usize,
    fit: Option<Fit>,
    rung: usize,
    metronome: bool,
}

/// How much fame counts against the tempo fit, for one [`Popularity`] (RPM-equivalents; lower wins).
#[derive(Debug, Clone, Copy, PartialEq)]
struct FameWeights {
    /// Bonus of a song the AI proposed for this very segment.
    proposed_here: f32,
    /// Bonus of a song the AI proposed for another segment: still the kind of song asked for.
    proposed_elsewhere: f32,
    /// Per place down the AI's list (it lists the best-known first).
    per_rank: f32,
    /// A song nobody proposed by an artist neither the AI nor the rider named.
    unknown_artist: f32,
    /// Times (1 - Tidal's popularity share): less played songs lose.
    unpopular: f32,
}

/// Big hits outweigh any fit inside the tolerance ([`super::settings::MAX_TOLERANCE_RPM`] at most): a famous
/// song that fits always beats an unknown one; discovering lets the tempo decide.
fn fame_weights(popularity: Popularity) -> FameWeights {
    match popularity {
        Popularity::Hits => FameWeights {
            proposed_here: 3.0,
            proposed_elsewhere: 1.5,
            per_rank: 1.0,
            unknown_artist: 14.0,
            unpopular: 3.0,
        },
        Popularity::Mix => FameWeights {
            proposed_here: 2.0,
            proposed_elsewhere: 1.0,
            per_rank: 0.5,
            unknown_artist: 3.0,
            unpopular: 1.0,
        },
        Popularity::Discover => FameWeights {
            proposed_here: 1.5,
            proposed_elsewhere: 0.75,
            per_rank: 0.0,
            unknown_artist: 0.0,
            unpopular: 0.0,
        },
    }
}

/// Everything the scoring reads besides the candidate.
struct Judge<'a> {
    weights: FameWeights,
    mix: bool,
    allow_explicit: bool,
    /// Artist keys the AI proposed or the rider likes.
    known: &'a HashSet<String>,
    liked: &'a HashSet<String>,
    /// Songs earlier builds of this ride played (Tidal ids, library paths).
    played: &'a HashSet<String>,
    /// For each segment of the (split) workout, the segment of the asked workout it came from.
    origin: &'a [usize],
    /// Decode songs of artists nobody named.
    analyze_strangers: bool,
}

impl Judge<'_> {
    fn is_known(&self, c: &Candidate) -> bool {
        c.proposed_for.is_some() || self.known.contains(&c.artist)
    }

    fn was_played(&self, c: &Candidate) -> bool {
        c.keys().any(|k| self.played.contains(k))
    }

    fn origin_of(&self, index: usize) -> usize {
        self.origin.get(index).copied().unwrap_or(index)
    }

    /// Lower is better: the tempo fit, then fame and taste, variety, length and the tempo's source.
    fn score(&self, c: &Candidate, fit: Option<Fit>, seg: &Segment, index: usize, before: Option<&Candidate>) -> f32 {
        let range = seg.cadence();
        let w = &self.weights;
        let mut s = match fit {
            Some(f) => f.error_rpm + CENTER_WEIGHT * (f.rpm - f32::from(range.center())).abs(),
            // no tempo to judge by: all the same, closest length decides below
            None => 0.0,
        };
        match c.proposed_for {
            Some(segment) => {
                s -= if segment == self.origin_of(index) { w.proposed_here } else { w.proposed_elsewhere };
                s += w.per_rank * c.rank as f32;
            }
            None if !self.known.contains(&c.artist) => s += w.unknown_artist,
            None => {}
        }
        if self.liked.contains(&c.artist) {
            s -= LIKED_ARTIST_BONUS;
        }
        if let Some(p) = c.popularity {
            s += w.unpopular * (1.0 - f32::from(p) / POPULARITY_MAX);
        }
        if self.was_played(c) {
            s += PLAYED_PENALTY;
        }
        if let Some(b) = before {
            if self.mix && !c.genre.is_empty() && c.genre == b.genre {
                s += MIX_REPEAT_PENALTY;
            }
            if !c.artist.is_empty() && c.artist == b.artist {
                s += SAME_ARTIST_PENALTY;
            }
        }
        if c.entry.duration_ms > 0 {
            let wanted = seg.seconds as f32 * MS_PER_S as f32;
            s += DURATION_WEIGHT * ((c.entry.duration_ms as f32 / wanted) - 1.0).abs();
        }
        s + match c.bpm {
            Some((_, BpmSource::Analysis)) => ANALYSIS_PENALTY,
            Some((_, BpmSource::Tidal)) => TIDAL_PENALTY,
            _ => 0.0,
        }
    }
}

/// One pass of the ladder: every segment still without a song takes the best candidate that fits `rung`.
fn select_rung(
    pool: &[Candidate],
    workout: &Workout,
    picks: &mut [Option<Pick>],
    rung: &Rung,
    rung_at: usize,
    judge: &Judge<'_>,
) {
    for index in 0..workout.segments.len() {
        if picks[index].is_some() {
            continue;
        }
        let seg = &workout.segments[index];
        let before = index.checked_sub(1).and_then(|i| picks[i]).map(|p| &pool[p.candidate]);
        let used: HashSet<usize> = picks.iter().flatten().map(|p| p.candidate).collect();
        let best = pool
            .iter()
            .enumerate()
            .filter(|(i, c)| !used.contains(i) && (judge.allow_explicit || !c.entry.explicit))
            .filter(|(_, c)| rung.metronome || c.duration_fits(seg))
            .filter_map(|(i, c)| {
                let fit = match c.bpm {
                    Some((bpm, _)) => fit(bpm, &seg.target, &rung.tolerance),
                    None => None,
                };
                if fit.is_none() && !rung.metronome {
                    return None;
                }
                Some((i, fit, judge.score(c, fit, seg, index, before)))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((candidate, fit, _)) = best {
            picks[index] = Some(Pick { candidate, fit, rung: rung_at, metronome: rung.metronome && fit.is_none() });
        }
    }
}

// ------------------------------------------------------------------ assembling

/// Rounds `seconds` to a whole number of 8-beat bars of a song with `beat_ms`, at least one bar.
pub(crate) fn snap_to_bars(seconds: u32, beat_ms: f64) -> u32 {
    let bar_s = BEATS_PER_BAR * beat_ms / 1000.0;
    if bar_s <= 0.0 || !bar_s.is_finite() {
        return seconds;
    }
    let bars = (f64::from(seconds) / bar_s).round().max(1.0);
    (bars * bar_s).round() as u32
}

/// A song longer than its segment is cut (see `play_ms`); one shorter shortens the segment. A sprint block's
/// work and rest are whole bars and its rounds fill the segment.
fn snap_segment(seg: &mut Segment, song: &PlannedSong) {
    if song.entry.duration_ms > 0 {
        seg.seconds = seg.seconds.min((song.entry.duration_ms as f64 / MS_PER_S as f64).round() as u32);
    }
    if let Some(iv) = &mut seg.interval {
        if let Some(grid) = song.grid {
            iv.work_s = snap_to_bars(iv.work_s, grid.beat_ms);
            iv.rest_s = snap_to_bars(iv.rest_s, grid.beat_ms);
        }
        iv.rounds = (seg.seconds / iv.cycle_s().max(1)).clamp(1, u32::from(iv.rounds.min(MAX_ROUNDS))) as u8;
    }
}

/// The median length (s) of the songs found, when any has one.
fn typical_seconds(pool: &Pool) -> Option<u32> {
    let mut lengths: Vec<u64> = pool.songs.iter().map(|c| c.entry.duration_ms).filter(|&d| d > 0).collect();
    lengths.sort_unstable();
    lengths.get(lengths.len() / 2).map(|ms| (ms / MS_PER_S) as u32).filter(|&s| s > 0)
}

/// A segment longer than the songs can cover (`typical_s` plus the slack) becomes equal consecutive parts of
/// the same kind, a sprint block by rounds. Shorter segments stay: their song is cut. Also answers, for each
/// segment of the result, the segment of `workout` it came from (the AI proposed songs for those).
fn split_long_segments(workout: Workout, typical_s: u32) -> (Workout, Vec<usize>) {
    let longest_s = ((typical_s as f32 / (1.0 - DURATION_SLACK)) as u32).max(1);
    let mut segments = Vec::with_capacity(workout.segments.len());
    let mut origin = Vec::with_capacity(workout.segments.len());
    for (from, seg) in workout.segments.iter().enumerate() {
        let wanted_parts = seg.seconds.div_ceil(longest_s).max(1);
        let parts = seg.interval.map_or(wanted_parts, |iv| wanted_parts.min(u32::from(iv.rounds)));
        if parts <= 1 {
            segments.push(seg.clone());
            origin.push(from);
            continue;
        }
        let mut left_s = seg.seconds;
        let mut left_rounds = seg.interval.map_or(0, |iv| u32::from(iv.rounds));
        for part in (1..=parts).rev() {
            let mut piece = seg.clone();
            match &mut piece.interval {
                Some(iv) => {
                    let rounds = left_rounds / part;
                    iv.rounds = rounds as u8;
                    // the last part keeps whatever the whole segment had beyond its rounds
                    piece.seconds = if part == 1 { left_s } else { rounds * iv.cycle_s() };
                    left_rounds -= rounds;
                }
                None => piece.seconds = left_s / part,
            }
            left_s -= piece.seconds;
            segments.push(piece);
            origin.push(from);
        }
    }
    // too many parts for a ride: keep it as it was (the songs are cut)
    if segments.len() > MAX_SEGMENTS {
        let origin = (0..workout.segments.len()).collect();
        return (workout, origin);
    }
    (Workout { segments, ..workout }, origin)
}

/// A tempo from the audio that agrees with `bpm` (any octave) to within [`GRID_AGREE`].
fn agrees(measured: f32, known: f32) -> bool {
    [1.0, 0.5, 2.0].iter().any(|k| ((measured * k) / known - 1.0).abs() <= GRID_AGREE)
}

#[allow(clippy::too_many_arguments)]
async fn assemble(
    request: &RideRequest,
    wish: &MusicWish,
    workout: Workout,
    pool: &mut Pool,
    picks: &[Option<Pick>],
    settings: &ExerciseSettings,
    ctx: &BuildContext<'_>,
    budget: &mut usize,
    notes: &mut Vec<String>,
) -> Result<RideBuilt> {
    let lang = ctx.lang;
    let mut segments: Vec<Segment> = Vec::new();
    let mut songs: Vec<PlannedSong> = Vec::new();
    for (seg, pick) in workout.segments.iter().zip(picks) {
        let Some(pick) = pick else {
            notes.push(words::no_song_note(lang, seg.kind));
            continue;
        };
        let c = &mut pool.songs[pick.candidate];
        // a song with a tempo but no grid: read the grid when there is budget, trust it only if it agrees; what
        // is read stays in the pool, so a rebuild that plays the song again reads nothing
        if c.grid.is_none() && !c.analyzed && *budget > 0 && !pick.metronome {
            if let Some(analyzer) = ctx.analyzer.filter(|a| a.can_read(&c.entry)) {
                *budget -= 1;
                c.analyzed = true;
                if let Some(a) = analyzer.analyze(&c.entry).await.filter(|a| a.bpm > 0.0) {
                    if c.bpm.is_none_or(|(known, _)| agrees(a.bpm as f32, known)) {
                        c.grid = a.grid();
                        c.bpm = c.bpm.or(Some((a.bpm as f32, BpmSource::Analysis)));
                    }
                }
            }
        }
        let (grid, bpm) = (c.grid, c.bpm);
        if pick.rung > 0 && !pick.metronome {
            notes.push(words::no_fit_note(lang, seg.kind));
        }
        if pick.metronome {
            notes.push(words::metronome_note(lang, seg.kind));
        }
        let mut song = PlannedSong {
            segment_index: segments.len(),
            entry: c.entry.clone(),
            bpm: bpm.map(|(b, _)| b.round() as u16),
            source: bpm.map(|(_, s)| s),
            fit: pick.fit,
            grid,
            pulse: if grid.is_some() { PulseSource::SongBeat } else { PulseSource::Metronome },
            play_ms: None,
        };
        let mut seg = seg.clone();
        snap_segment(&mut seg, &song);
        let range = seg.cadence();
        song.entry.note = words::song_note(lang, seg.kind, range.min_rpm, range.max_rpm, song.bpm);
        seg.note.clone_from(&song.entry.note);
        segments.push(seg);
        songs.push(song);
    }
    if songs.is_empty() {
        return Err(words::problem(lang, Problem::NoSongs));
    }
    // bar snapping may have stretched a rest: the caps and the interval's fit hold again
    let mut final_workout = beginner_caps(&Workout { segments, ..workout }.sanitized(), settings);
    if final_workout.id.trim().is_empty() {
        final_workout.id = new_id();
    }
    // a song longer than its (final) segment is cut where the segment ends
    for (song, seg) in songs.iter_mut().zip(&final_workout.segments) {
        let segment_ms = u64::from(seg.seconds) * MS_PER_S;
        song.play_ms = (song.entry.duration_ms == 0 || song.entry.duration_ms > segment_ms).then_some(segment_ms);
    }
    // the caps can change a segment's length (a sprint that needs a longer rest): the songs are untouched
    let total_minutes = (final_workout.total_seconds() as f32 / 60.0).round() as u32;
    let now = now_ms();
    let playlist = HomePlaylist {
        version: PLAYLIST_FORMAT_VERSION,
        id: new_id(),
        name: words::ride_name(lang, &final_workout.name, request.style, total_minutes),
        description: words::ride_description(lang, &wish.genres),
        created_ms: now,
        updated_ms: now,
        origin: PlaylistOrigin::Ride(RideOrigin { workout_id: final_workout.id.clone(), wish: wish.clone() }),
        entries: songs.iter().map(|s| s.entry.clone()).collect(),
        unmatched: Vec::new(),
        sources: Vec::new(),
        search_widget_html: None,
        tidal_uuid: None,
    };
    let course = profile(&final_workout);
    let plan = RidePlan {
        version: super::model::WORKOUT_FORMAT_VERSION,
        workout: final_workout,
        songs,
        notes: std::mem::take(notes),
        cues: CueSettings {
            countdown_seconds: settings.countdown_seconds,
            notice_seconds: settings.segment_notice_seconds,
        },
        wish: wish.clone(),
        profile: course,
    };
    Ok(RideBuilt { plan, playlist })
}

/// A plan for tests of the clock: one song per segment, in order, no grid.
#[cfg(test)]
pub(crate) fn test_plan(workout: Workout, cues: CueSettings) -> RidePlan {
    let songs = (0..workout.segments.len())
        .map(|i| PlannedSong {
            segment_index: i,
            entry: PlaylistEntry { title: format!("Song {i}"), ..PlaylistEntry::default() },
            ..PlannedSong::default()
        })
        .collect();
    let profile = profile(&workout);
    RidePlan { version: 1, workout, songs, notes: Vec::new(), cues, wish: MusicWish::default(), profile }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_are_eight_beats() {
        // 120 BPM: a beat is 500 ms, a bar 4 s
        assert_eq!(snap_to_bars(20, 500.0), 20);
        assert_eq!(snap_to_bars(21, 500.0), 20);
        assert_eq!(snap_to_bars(1, 500.0), 4, "at least one bar");
        assert_eq!(snap_to_bars(30, 0.0), 30, "no grid, no change");
    }

    #[test]
    fn tempos_agree_in_any_octave() {
        assert!(agrees(170.0, 85.0) && agrees(85.0, 170.0) && agrees(128.0, 127.0));
        assert!(!agrees(100.0, 128.0));
    }

    fn song(id: &str, artist: &str, bpm: f32) -> Candidate {
        let entry = PlaylistEntry {
            title: id.into(),
            artist: artist.into(),
            tidal_id: Some(id.into()),
            duration_ms: 240_000,
            ..PlaylistEntry::default()
        };
        Candidate::new(entry, Some((bpm, BpmSource::Tidal)), String::new())
    }

    fn proposed(id: &str, artist: &str, bpm: f32, segment: usize, rank: usize) -> Candidate {
        Candidate { proposed_for: Some(segment), rank, ..song(id, artist, bpm) }
    }

    struct Scene {
        known: HashSet<String>,
        liked: HashSet<String>,
        played: HashSet<String>,
        origin: Vec<usize>,
    }

    impl Scene {
        fn new(known: &[&str]) -> Self {
            Self {
                known: known.iter().map(|a| artist_key(a)).collect(),
                liked: HashSet::new(),
                played: HashSet::new(),
                origin: vec![0, 1],
            }
        }

        fn judge(&self, popularity: Popularity) -> Judge<'_> {
            Judge {
                weights: fame_weights(popularity),
                mix: false,
                allow_explicit: true,
                known: &self.known,
                liked: &self.liked,
                played: &self.played,
                origin: &self.origin,
                analyze_strangers: true,
            }
        }
    }

    fn flat() -> Workout {
        let seg = Segment {
            seconds: 240,
            target: crate::exercise::Target::Cadence { min_rpm: 85, max_rpm: 95 },
            ..Segment::default()
        };
        Workout { segments: vec![seg.clone(), seg], ..Workout::default() }
    }

    /// The title each segment gets from `pool` at the first rung.
    fn choose(pool: &[Candidate], workout: &Workout, judge: &Judge<'_>) -> Vec<String> {
        let rung = ladder(&Tolerance::from_settings(&ExerciseSettings::default()))[0];
        let mut picks = vec![None; workout.segments.len()];
        select_rung(pool, workout, &mut picks, &rung, 0, judge);
        picks.iter().map(|p| p.map_or_else(String::new, |p: Pick| pool[p.candidate].entry.title.clone())).collect()
    }

    #[test]
    fn big_hits_beat_a_better_fit_by_an_unknown_artist_and_discover_lets_the_tempo_win() {
        // the obscure song sits on the center of the cadence; the hit is 3 RPM off
        let pool = [song("obscure", "Basement Band", 90.0), proposed("hit", "Metallica", 98.0, 0, 0)];
        let scene = Scene::new(&["Metallica"]);
        let workout = flat();
        assert_eq!(choose(&pool, &workout, &scene.judge(Popularity::Hits))[0], "hit");
        assert_eq!(choose(&pool, &workout, &scene.judge(Popularity::Discover))[0], "obscure");
        // a song by an artist the AI named, found by a search, counts as known (no stranger's penalty)
        let pool = [song("obscure", "Basement Band", 90.0), song("deep cut", "Metallica", 97.0)];
        assert_eq!(choose(&pool, &workout, &scene.judge(Popularity::Hits))[0], "deep cut");
    }

    #[test]
    fn the_ais_order_and_tidals_popularity_rank_the_hits() {
        let scene = Scene::new(&["Iron Maiden"]);
        let workout = flat();
        let pool = [proposed("b-side", "Iron Maiden", 90.0, 0, 3), proposed("anthem", "Iron Maiden", 93.0, 0, 0)];
        assert_eq!(choose(&pool, &workout, &scene.judge(Popularity::Hits))[0], "anthem");
        let quiet = Candidate { popularity: Some(5), ..song("quiet", "Nobody", 90.0) };
        let loud = Candidate { popularity: Some(95), ..song("loud", "Somebody", 91.0) };
        assert_eq!(choose(&[quiet, loud], &workout, &scene.judge(Popularity::Hits))[0], "loud");
    }

    #[test]
    fn played_songs_come_back_last_and_liked_artists_first() {
        let mut scene = Scene::new(&["A", "B"]);
        scene.played.insert("first".into());
        let workout = flat();
        let pool = [proposed("first", "A", 90.0, 0, 0), proposed("second", "B", 92.0, 0, 1)];
        let picks = choose(&pool, &workout, &scene.judge(Popularity::Hits));
        assert_eq!(picks[0], "second", "a rebuild plays what the last build did not");
        assert_eq!(picks[1], "first", "and an old song only where nothing new is left");
        let mut scene = Scene::new(&[]);
        scene.liked.insert(artist_key("Slayer"));
        let pool = [song("other", "Other", 90.0), song("liked", "Slayer", 92.0)];
        assert_eq!(choose(&pool, &workout, &scene.judge(Popularity::Discover))[0], "liked");
    }

    #[test]
    fn split_parts_keep_the_ais_segment() {
        // a 10-minute warm-up for songs of 200 s becomes 3 parts; the flat after it is still segment 1
        let warm = Segment { seconds: 600, ..Segment::default() };
        let flat = Segment { seconds: 240, ..Segment::default() };
        let (split, origin) = split_long_segments(Workout { segments: vec![warm, flat], ..Workout::default() }, 200);
        assert_eq!(split.segments.len(), 4);
        assert_eq!(origin, [0, 0, 0, 1]);
        let scene = Scene { origin, ..Scene::new(&["A", "B"]) };
        let judge = scene.judge(Popularity::Hits);
        let for_flat = proposed("flat song", "A", 90.0, 1, 0);
        let seg = &split.segments[3];
        let here = judge.score(
            &for_flat,
            fit(90.0, &seg.target, &Tolerance::from_settings(&ExerciseSettings::default())),
            seg,
            3,
            None,
        );
        let elsewhere = judge.score(
            &for_flat,
            fit(90.0, &seg.target, &Tolerance::from_settings(&ExerciseSettings::default())),
            seg,
            1,
            None,
        );
        assert!(
            here < elsewhere,
            "the song proposed for the flat gets its bonus on the flat's part, not the warm-up's"
        );
    }

    #[test]
    fn a_song_by_someone_else_is_not_the_one_asked_for() {
        assert!(same_artist("Metallica", "metallica"));
        assert!(same_artist("Guns N' Roses", "Guns N Roses"));
        assert!(same_artist("The Offspring", "Offspring"));
        assert!(same_artist("Metallica & San Francisco Symphony", "Metallica"));
        assert!(!same_artist("Metal Tribute Band", "Metallica"));
        assert!(!same_artist("Karaoke Hits", "Iron Maiden"));
        assert!(!same_artist("U2 Tribute", "U2"), "short names never match by containment");
    }

    #[test]
    fn avoided_artists_never_enter_the_pool() {
        let taste = RideTaste { avoid: vec!["Nickelback".into()], ..RideTaste::default() };
        let mut pool = Pool::new(&taste);
        pool.add(song("photograph", "Nickelback", 90.0));
        pool.add(song("one", "Metallica", 90.0));
        assert_eq!(pool.songs.len(), 1);
        assert_eq!(pool.songs[0].entry.title, "one");
    }

    #[test]
    fn the_widening_search_follows_the_popularity_and_the_liked_artists() {
        let wish = MusicWish {
            genres: vec!["metal".into()],
            taste: RideTaste { artists: vec!["Slayer".into()], ..RideTaste::default() },
            ..MusicWish::default()
        };
        let queries: Vec<String> =
            search_queries(&wish, widen_words(Popularity::Hits)).into_iter().map(|q| q.1).collect();
        assert_eq!(queries, ["metal hits", "Slayer"]);
        assert_eq!(widen_words(Popularity::Discover), "workout");
    }
}
