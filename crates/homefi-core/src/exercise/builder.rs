//! Builds a ride: the workout (a style at a length, or the rider's / the AI's own), the songs whose tempo
//! fits each segment, and the playlist that plays them. Nothing is saved here and nothing is written to
//! Tidal (read-only calls only).
//!
//! Candidates come from the library (the `BPM` tag), the AI's proposals resolved on Tidal and the library,
//! or Tidal's search by genre; a tempo is the tag, then Tidal's, then HomeFi's own analysis within the
//! budget. Each segment takes the best fit; when none fits the ladder of [`super::bpm::ladder`] widens.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use super::ai::{propose_songs, Proposal};
use super::bpm::{fit, ladder, BpmSource, Fit, Rung, Tolerance};
use super::model::{round_minutes, MusicWish, RideRequest, Segment, Workout, MAX_ROUNDS, MAX_SEGMENTS};
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
    entry_from_library, entry_from_track, new_id, now_ms, EntryTag, HomePlaylist, LibraryIndex, PlaylistEntry,
    PlaylistOrigin, Resolver, RideOrigin, SeenEntries, SongCandidate, PLAYLIST_FORMAT_VERSION, RESOLVE_PARALLEL,
    RESOLVE_TIMEOUT,
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

// score weights, in RPM-equivalents
const PROPOSED_BONUS: f32 = 1.5;
/// Per place down the AI's list for a segment: among songs that fit the tempo, the best-known one wins
/// (the rider wants to sing along), but never over a clearly better fit.
const FAME_WEIGHT: f32 = 0.6;
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
    build_within(request, ctx, progress, BUILD_DEADLINE).await
}

pub(crate) async fn build_within(
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    deadline: Duration,
) -> Result<RideBuilt> {
    tokio::time::timeout(deadline, build_inner(request, ctx, progress))
        .await
        .unwrap_or_else(|_| Err(words::problem(ctx.lang, Problem::TooSlow)))
}

/// A song that could be in the ride.
#[derive(Debug, Clone)]
struct Candidate {
    entry: PlaylistEntry,
    bpm: Option<(f32, BpmSource)>,
    grid: Option<Grid>,
    /// The wish genre that found it (empty when unknown).
    genre: String,
    /// The segment the AI proposed it for.
    proposed_for: Option<usize>,
    /// Its place in the AI's list for that segment (0 = the best-known song).
    rank: usize,
}

impl Candidate {
    fn duration_fits(&self, seg: &Segment) -> bool {
        let wanted = seg.seconds as f32 * MS_PER_S as f32;
        // a longer song is cut to the segment; a shorter one must nearly cover it
        self.entry.duration_ms == 0 || self.entry.duration_ms as f32 >= wanted * (1.0 - DURATION_SLACK)
    }
}

/// Everything found so far.
struct Pool {
    songs: Vec<Candidate>,
    seen: SeenEntries,
}

impl Pool {
    fn add(&mut self, candidate: Candidate) {
        if self.seen.first_time(&candidate.entry) {
            self.songs.push(candidate);
        }
    }
}

fn positive_bpm(b: i64) -> Option<f32> {
    (b > 0 && b < i64::from(u16::MAX)).then_some(b as f32)
}

async fn build_inner(request: &RideRequest, ctx: &BuildContext<'_>, progress: Progress<'_>) -> Result<RideBuilt> {
    let lang = ctx.lang;
    let settings = ctx.settings.clamped();
    let api = ctx.api.filter(|_| settings.sources != SourcesMode::LibraryOnly);
    if api.is_none() && ctx.library.is_empty() {
        return Err(words::problem(lang, Problem::NothingToUse));
    }
    let minutes = round_minutes(request.minutes);
    let workout = match &request.workout {
        Some(w) if w.segments.is_empty() => return Err(words::problem(lang, Problem::NoRide)),
        Some(w) => beginner_caps(&w.clone().sanitized(), &settings),
        None => preset(request.style, minutes, &settings),
    };
    let wish = request.wish.clone().clamped();
    let mut workout = workout;
    let tolerance = Tolerance::from_settings(&settings);
    let prefer_library = settings.sources != SourcesMode::TidalFirst;
    let mut notes = Vec::new();
    let mut pool = Pool { songs: Vec::new(), seen: SeenEntries::default() };
    let mut budget = usize::from(settings.analysis_budget).min(if ctx.analyzer.is_some() { usize::MAX } else { 0 });

    add_library(&mut pool, ctx.library, &wish);
    let mut with_ai = false;
    if let Some(ai) = ctx.ai {
        progress(RideProgress::Searching { with_ai: true });
        match propose_songs(ai, &workout, &wish, &tolerance, settings.allow_explicit, lang).await {
            Ok(proposals) if !proposals.is_empty() => {
                with_ai = true;
                let resolver = Resolver { api, library: ctx.library, prefer_library };
                add_proposals(&mut pool, &resolver, proposals, progress).await;
            }
            Ok(_) => notes.push(words::ai_failed_note(lang, words::ai_no_songs(lang))),
            Err(e) => {
                tracing::warn!("{e}");
                notes.push(words::ai_failed_note(lang, &e.humane_in(lang)));
            }
        }
    } else if !wish.genres.is_empty() {
        notes.push(words::no_ai_note(lang));
    }
    if !with_ai {
        if let Some(api) = api {
            progress(RideProgress::Searching { with_ai: false });
            let queries = genre_queries(&wish, "");
            add_searches(&mut pool, api, ctx.library, prefer_library, queries).await;
        }
    }
    if api.is_none() {
        notes.push(crate::concert::words::no_tidal_note(lang));
    }
    // long segments become several parts when the songs found are short, so no song has to stretch
    if let Some(typical_s) = typical_seconds(&pool) {
        workout = split_long_segments(workout, typical_s);
    }
    analyze_unknowns(&mut pool, &workout, ctx.analyzer, &mut budget, progress).await;

    progress(RideProgress::Matching);
    let rungs = ladder(&tolerance);
    let mut picks: Vec<Option<Pick>> = vec![None; workout.segments.len()];
    let mut searched_more = false;
    for (rung_at, rung) in rungs.iter().enumerate() {
        if rung.search_tidal && !searched_more && picks.iter().any(Option::is_none) {
            searched_more = true;
            if let Some(api) = api {
                add_searches(&mut pool, api, ctx.library, prefer_library, genre_queries(&wish, "workout")).await;
                analyze_unknowns(&mut pool, &workout, ctx.analyzer, &mut budget, progress).await;
            }
        }
        select_rung(&mut pool.songs, &workout, &mut picks, rung, rung_at, wish.mix, settings.allow_explicit);
    }

    assemble(request, &wish, workout, &pool, picks, &settings, ctx, &mut budget, &mut notes).await
}

// ------------------------------------------------------------------ gathering

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
        pool.add(Candidate {
            entry: entry_from_library(lib, EntryTag::Plain, String::new()),
            bpm,
            grid: None,
            genre,
            proposed_for: None,
            rank: 0,
        });
    }
}

async fn add_proposals(pool: &mut Pool, resolver: &Resolver<'_>, proposals: Vec<Proposal>, progress: Progress<'_>) {
    let (places, candidates): (Vec<(usize, usize)>, Vec<SongCandidate>) =
        proposals.into_iter().map(|p| ((p.segment, p.rank), p.candidate)).unzip();
    let resolved =
        resolver.resolve_all(candidates, &|done, total| progress(RideProgress::Resolving { done, total })).await;
    for ((segment, rank), r) in places.into_iter().zip(resolved) {
        let Some(entry) = r.entry else { continue };
        let bpm = match (r.library_bpm, r.tidal_bpm) {
            (Some(b), _) => Some((f32::from(b), BpmSource::Tag)),
            (None, Some(b)) => Some((f32::from(b), BpmSource::Tidal)),
            _ => None,
        };
        pool.add(Candidate { entry, bpm, grid: None, genre: String::new(), proposed_for: Some(segment), rank });
    }
}

/// `(genre, query)` pairs: the genre alone and with the rider's sentence; `extra` is appended to each.
fn genre_queries(wish: &MusicWish, extra: &str) -> Vec<(String, String)> {
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
            pool.add(Candidate { entry, bpm, grid: None, genre: genre.clone(), proposed_for: None, rank: 0 });
        }
    }
}

/// Reads the tempo of candidates that have none, within `budget` songs, the AI's own picks first.
async fn analyze_unknowns(
    pool: &mut Pool,
    workout: &Workout,
    analyzer: Option<&dyn Analyzer>,
    budget: &mut usize,
    progress: Progress<'_>,
) {
    let Some(analyzer) = analyzer else { return };
    let mut todo: Vec<usize> = (0..pool.songs.len())
        .filter(|&i| {
            let c = &pool.songs[i];
            c.bpm.is_none() && workout.segments.iter().any(|s| c.duration_fits(s))
        })
        .collect();
    todo.sort_by_key(|&i| (pool.songs[i].proposed_for.is_none(), i));
    todo.truncate(*budget);
    let total = todo.len();
    for (done, i) in todo.into_iter().enumerate() {
        *budget -= 1;
        if let Some(a) = analyzer.analyze(&pool.songs[i].entry).await.filter(|a| a.bpm > 0.0) {
            pool.songs[i].bpm = Some((a.bpm as f32, BpmSource::Analysis));
            pool.songs[i].grid = a.grid();
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

/// One pass of the ladder: every segment still without a song takes the best candidate that fits `rung`.
fn select_rung(
    pool: &mut [Candidate],
    workout: &Workout,
    picks: &mut [Option<Pick>],
    rung: &Rung,
    rung_at: usize,
    mix: bool,
    allow_explicit: bool,
) {
    for index in 0..workout.segments.len() {
        if picks[index].is_some() {
            continue;
        }
        let seg = &workout.segments[index];
        let before = index.checked_sub(1).and_then(|i| picks[i]).map(|p| &pool[p.candidate]);
        let used: Vec<usize> = picks.iter().flatten().map(|p| p.candidate).collect();
        let best = pool
            .iter()
            .enumerate()
            .filter(|(i, c)| !used.contains(i) && (allow_explicit || !c.entry.explicit))
            .filter(|(_, c)| rung.metronome || c.duration_fits(seg))
            .filter_map(|(i, c)| {
                let fit = match c.bpm {
                    Some((bpm, _)) => fit(bpm, &seg.target, &rung.tolerance),
                    None => None,
                };
                if fit.is_none() && !rung.metronome {
                    return None;
                }
                Some((i, fit, score(c, fit, seg, index, before, mix)))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((candidate, fit, _)) = best {
            picks[index] = Some(Pick { candidate, fit, rung: rung_at, metronome: rung.metronome && fit.is_none() });
        }
    }
}

fn score(c: &Candidate, fit: Option<Fit>, seg: &Segment, index: usize, before: Option<&Candidate>, mix: bool) -> f32 {
    let range = seg.cadence();
    let mut s = match fit {
        Some(f) => f.error_rpm + CENTER_WEIGHT * (f.rpm - f32::from(range.center())).abs(),
        // no tempo to judge by: all the same, closest length decides below
        None => 0.0,
    };
    if c.proposed_for == Some(index) {
        s -= PROPOSED_BONUS;
        s += FAME_WEIGHT * c.rank as f32;
    }
    if let Some(b) = before {
        if mix && !c.genre.is_empty() && c.genre == b.genre {
            s += MIX_REPEAT_PENALTY;
        }
        if !c.entry.artist.is_empty() && c.entry.artist == b.entry.artist {
            s += SAME_ARTIST_PENALTY;
        }
    }
    if c.entry.duration_ms > 0 {
        let wanted = seg.seconds as f32 * MS_PER_S as f32;
        s += DURATION_WEIGHT * ((c.entry.duration_ms as f32 / wanted) - 1.0).abs();
    }
    s += match c.bpm {
        Some((_, BpmSource::Analysis)) => ANALYSIS_PENALTY,
        Some((_, BpmSource::Tidal)) => TIDAL_PENALTY,
        _ => 0.0,
    };
    s
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
/// the same kind, a sprint block by rounds. Shorter segments stay: their song is cut.
fn split_long_segments(workout: Workout, typical_s: u32) -> Workout {
    let longest_s = ((typical_s as f32 / (1.0 - DURATION_SLACK)) as u32).max(1);
    let mut segments = Vec::with_capacity(workout.segments.len());
    for seg in workout.segments.iter() {
        let wanted_parts = seg.seconds.div_ceil(longest_s).max(1);
        let parts = seg.interval.map_or(wanted_parts, |iv| wanted_parts.min(u32::from(iv.rounds)));
        if parts <= 1 {
            segments.push(seg.clone());
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
        }
    }
    // too many parts for a ride: keep it as it was (the songs are cut)
    if segments.len() > MAX_SEGMENTS {
        return workout;
    }
    Workout { segments, ..workout }
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
    pool: &Pool,
    picks: Vec<Option<Pick>>,
    settings: &ExerciseSettings,
    ctx: &BuildContext<'_>,
    budget: &mut usize,
    notes: &mut Vec<String>,
) -> Result<RideBuilt> {
    let lang = ctx.lang;
    let mut segments: Vec<Segment> = Vec::new();
    let mut songs: Vec<PlannedSong> = Vec::new();
    for (seg, pick) in workout.segments.iter().zip(&picks) {
        let Some(pick) = pick else {
            notes.push(words::no_song_note(lang, seg.kind));
            continue;
        };
        let c = &pool.songs[pick.candidate];
        let mut grid = c.grid;
        let mut bpm = c.bpm;
        // a song with a tempo but no grid: read the grid when there is budget, trust it only if it agrees
        if grid.is_none() && *budget > 0 && !pick.metronome {
            if let Some(analyzer) = ctx.analyzer {
                *budget -= 1;
                if let Some(a) = analyzer.analyze(&c.entry).await.filter(|a| a.bpm > 0.0) {
                    if bpm.is_none_or(|(known, _)| agrees(a.bpm as f32, known)) {
                        grid = a.grid();
                        bpm = bpm.or(Some((a.bpm as f32, BpmSource::Analysis)));
                    }
                }
            }
        }
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
}
