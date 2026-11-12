//! Another song for one part of a built ride, in the plan preview or while riding.
//!
//! The song comes first from what the build already found (`BuildMemory`'s pool: instant, no network). Only
//! when nothing there fits, one targeted search runs: the AI proposes songs for that single part (when the rider
//! allows it: `swap_uses_ai`), or Tidal is searched by genre. The song sent away never comes back to the ride.
//!
//! The new song must cover the part as it is (80 % at least, like in a build): a longer one is cut where the part
//! ends, a shorter one shortens the part (20 % at most). Swapped in mid-part, the song starts from its beginning
//! while the part's clock goes on (`PlannedSong::at_segment_ms`).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::ai::propose_songs;
use super::bpm::{ladder, Tolerance};
use super::builder::{
    add_library, add_proposals, add_searches, analyze_unknowns, cut_ms, entry_keys, fame_weights, known_artists,
    memory_key, plan_song, search_queries, select_rung, widen_words, BuildContext, BuildMemory, Judge, Pick,
    PlannedSong, Pool, Progress, RideBuilt, RideProgress,
};
use super::model::{MusicWish, Popularity, RideRequest, Segment, Workout};
use super::presets::beginner_caps;
use super::profile::profile;
use super::settings::{ExerciseSettings, SourcesMode};
use super::words::{self, Problem};
use crate::error::Result;
use crate::playlist::{artist_key, now_ms, Resolver};

/// The whole swap (AI, Tidal, analysis) must finish within this: the rider is waiting on a screen.
pub const SWAP_DEADLINE: Duration = Duration::from_secs(60);
/// Songs whose audio a swap reads at most, whatever the settings allow (the build has its own, larger budget).
const SWAP_ANALYSIS_BUDGET: usize = 4;
const MS_PER_S: u64 = 1000;

/// Which song of which ride, and where the rider is.
pub struct SwapRequest<'a> {
    pub built: &'a RideBuilt,
    /// The song to replace: its index in `built.plan.songs` (the same as in the playlist).
    pub segment_index: usize,
    /// While riding: how far into the part the rider is (ms). 0 in the plan preview.
    pub at_segment_ms: u64,
}

/// The ride with the new song, the song itself and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Swapped {
    pub built: RideBuilt,
    pub song: PlannedSong,
    /// Found in what the build already had (no search).
    pub from_pool: bool,
}

/// Swaps the song at `req.segment_index`. `request` and `ctx` are the ones of the build, so the memory of that
/// build is recognised (`memory`); the answer carries the memory for the next swap or build, with the song
/// sent away remembered. The ride in `req.built` is not touched.
pub async fn swap_song(
    req: &SwapRequest<'_>,
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    memory: Option<BuildMemory>,
) -> Result<(Swapped, BuildMemory)> {
    tokio::time::timeout(SWAP_DEADLINE, swap_inner(req, request, ctx, progress, memory))
        .await
        .unwrap_or_else(|_| Err(words::problem(ctx.lang, Problem::TooSlow)))
}

/// The scoring of one part's candidates (the build's judge, for a single part).
struct SwapJudge<'a> {
    wish: &'a MusicWish,
    settings: &'a ExerciseSettings,
    liked: HashSet<String>,
    played: &'a HashSet<String>,
    blocked: &'a HashSet<String>,
}

impl SwapJudge<'_> {
    /// `known` is the artists the AI named so far; it changes when the AI proposes more.
    fn judge<'k>(&'k self, known: &'k HashSet<String>, asked_any: bool) -> Judge<'k> {
        Judge {
            weights: fame_weights(self.wish.taste.popularity),
            mix: self.wish.mix,
            allow_explicit: self.settings.allow_explicit,
            known,
            liked: &self.liked,
            played: self.played,
            rejected: self.blocked,
            origin: &[0],
            analyze_strangers: self.wish.taste.popularity != Popularity::Hits || !asked_any,
            strict_length: true,
        }
    }
}

/// `Some(pick)` of the first rung (that needs no new search) where a candidate fits the part.
fn pick_from(
    pool: &Pool,
    probe: &Workout,
    tolerance: &Tolerance,
    judge: &Judge<'_>,
    last_resorts: bool,
) -> Option<Pick> {
    let mut picks: Vec<Option<Pick>> = vec![None];
    for (at, rung) in ladder(tolerance).iter().enumerate() {
        if !last_resorts && (rung.search_tidal || rung.metronome) {
            continue;
        }
        select_rung(&pool.songs, probe, &mut picks, rung, at, judge);
        if picks[0].is_some() {
            break;
        }
    }
    picks[0]
}

async fn swap_inner(
    req: &SwapRequest<'_>,
    request: &RideRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    memory: Option<BuildMemory>,
) -> Result<(Swapped, BuildMemory)> {
    let lang = ctx.lang;
    let settings = ctx.settings.clamped();
    if !settings.ride_swap {
        return Err(words::problem(lang, Problem::SwapOff));
    }
    let plan = &req.built.plan;
    let old = plan.songs.get(req.segment_index).ok_or_else(|| words::problem(lang, Problem::NoRide))?;
    let segment_at = old.segment_index;
    let seg = plan.workout.segments.get(segment_at).ok_or_else(|| words::problem(lang, Problem::NoRide))?;
    let api = ctx.api.filter(|_| settings.sources != SourcesMode::LibraryOnly);
    if api.is_none() && ctx.library.is_empty() {
        return Err(words::problem(lang, Problem::NothingToUse));
    }

    // what is left of the part: the new song must cover that, and only that is judged
    let segment_ms = u64::from(seg.seconds) * MS_PER_S;
    let at_segment_ms = req.at_segment_ms.min(segment_ms);
    let left_s = (segment_ms - at_segment_ms).div_ceil(MS_PER_S).max(1);
    let probe = Workout {
        segments: vec![Segment { seconds: u32::try_from(left_s).unwrap_or(u32::MAX), ..seg.clone() }],
        ..Workout::default()
    };

    let wish = request.wish.clone().clamped();
    let tolerance = Tolerance::from_settings(&settings);
    let prefer_library = settings.sources != SourcesMode::TidalFirst;
    let key = memory_key(request, &wish, &settings, ctx);
    let (made, mut asked, mut played, mut rejected, songs) = match memory.filter(|m| m.usable_for_swap(&key)) {
        Some(m) => m.into_parts(),
        None => (Instant::now(), Vec::new(), HashSet::new(), HashSet::new(), Vec::new()),
    };
    let had_pool = !songs.is_empty();
    // the song sent away is gone for good; the others already in the ride cannot come twice
    rejected.extend(entry_keys(&old.entry).map(str::to_owned));
    played.extend(entry_keys(&old.entry).map(str::to_owned));
    let mut blocked = rejected.clone();
    for (at, song) in plan.songs.iter().enumerate() {
        if at != req.segment_index {
            blocked.extend(entry_keys(&song.entry).map(str::to_owned));
        }
    }

    let mut pool = Pool::new(&wish.taste);
    for song in songs {
        pool.add(song);
    }
    let scene = SwapJudge {
        wish: &wish,
        settings: &settings,
        liked: wish.taste.artists.iter().map(|a| artist_key(a)).filter(|a| !a.is_empty()).collect(),
        played: &played,
        blocked: &blocked,
    };
    let mut budget = SWAP_ANALYSIS_BUDGET.min(usize::from(settings.analysis_budget)).min(if ctx.analyzer.is_some() {
        usize::MAX
    } else {
        0
    });

    // 1. what the build already found
    let mut known = known_artists(&asked, &scene.liked);
    let mut pick = if had_pool {
        pick_from(&pool, &probe, &tolerance, &scene.judge(&known, !asked.is_empty()), false)
    } else {
        None
    };
    let from_pool = pick.is_some();

    // 2. one targeted search: the AI for this part alone, or Tidal by genre
    if pick.is_none() {
        if !had_pool {
            // no memory (a saved ride ridden again, or the app restarted): the library is the first place to look
            add_library(&mut pool, ctx.library, &wish);
        }
        let mut ai_gave = false;
        if let Some(ai) = ctx.ai.filter(|_| settings.swap_uses_ai) {
            progress(RideProgress::Searching { with_ai: true });
            match propose_songs(ai, &probe, &wish, &tolerance, settings.allow_explicit, &asked).await {
                Ok(proposals) if !proposals.is_empty() => {
                    ai_gave = true;
                    asked.extend(proposals.iter().map(|p| p.candidate.clone()));
                    let resolver = Resolver { api, library: ctx.library, prefer_library };
                    add_proposals(&mut pool, &resolver, proposals, progress).await;
                }
                Ok(_) => tracing::warn!("the AI proposed no song for a swap"),
                Err(e) => tracing::warn!("the swap's AI failed: {e}"),
            }
        }
        let mut searched = false;
        if !ai_gave {
            if let Some(api) = api {
                progress(RideProgress::Searching { with_ai: false });
                let queries = search_queries(&wish, widen_words(wish.taste.popularity));
                add_searches(&mut pool, api, ctx.library, prefer_library, queries).await;
                searched = true;
            }
        }
        known = known_artists(&asked, &scene.liked);
        analyze_unknowns(
            &mut pool,
            &probe,
            ctx.analyzer,
            &mut budget,
            &scene.judge(&known, !asked.is_empty()),
            progress,
        )
        .await;

        // 3. the whole ladder: wider tempos, one more search, a metronome last
        let judge = scene.judge(&known, !asked.is_empty());
        pick = pick_from(&pool, &probe, &tolerance, &judge, true);
        if pick.is_none() && !searched {
            if let Some(api) = api {
                let queries = search_queries(&wish, widen_words(wish.taste.popularity));
                add_searches(&mut pool, api, ctx.library, prefer_library, queries).await;
                analyze_unknowns(&mut pool, &probe, ctx.analyzer, &mut budget, &judge, progress).await;
                pick = pick_from(&pool, &probe, &tolerance, &judge, true);
            }
        }
    }
    let pick = pick.ok_or_else(|| words::problem(lang, Problem::NoReplacement))?;

    // the song as the part's: the segment follows it (at most 20 % shorter), the notes and pulse are its own
    let (mut song, new_seg) =
        plan_song(&mut pool.songs[pick.candidate], &pick, seg, segment_at, at_segment_ms, ctx, &mut budget).await;
    let mut workout = plan.workout.clone();
    workout.segments[segment_at] = new_seg;
    let workout = beginner_caps(&workout.sanitized(), &settings);
    let new_segment_ms = workout.segments.get(segment_at).map_or(segment_ms, |s| u64::from(s.seconds) * MS_PER_S);
    song.play_ms = cut_ms(song.entry.duration_ms, new_segment_ms, at_segment_ms);

    let mut built = req.built.clone();
    built.plan.profile = profile(&workout);
    built.plan.workout = workout;
    built.plan.songs[req.segment_index] = song.clone();
    if let Some(entry) = built.playlist.entries.get_mut(req.segment_index) {
        entry.clone_from(&song.entry);
    }
    built.playlist.updated_ms = now_ms();

    played.extend(entry_keys(&song.entry).map(str::to_owned));
    let memory = BuildMemory::after_swap(key, made, asked, played, rejected, pool.songs);
    Ok((Swapped { built, song, from_pool }, memory))
}
