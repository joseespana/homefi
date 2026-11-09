//! Builds the concert playlist: the AI's recent shows when it can search the
//! web, Tidal's most played when it can't, and then the resolver. Nothing is
//! saved here and nothing is written to Tidal (read-only calls only).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ai_search::{recent_shows, ShowsFound};
use super::words::{self, Fallback, Problem};
use super::{
    most_played, show_set, tour_set, ConcertMode, ConcertRequest, ConcertShow, ConcertThresholds, TourSet,
    MAX_CONCERT_SHOWS, MIN_CONCERT_SHOWS, MIN_TOUR_SHOWS, POPULAR_LIMIT,
};
use crate::ai::AiConfig;
use crate::api::TidalApi;
use crate::error::Result;
use crate::humane::Lang;
use crate::playlist::{
    entry_from_track, new_id, now_ms, ConcertOrigin, EntryTag, HomePlaylist, LibraryIndex, PlaylistEntry,
    PlaylistOrigin, PlaylistSettings, Resolver, SeenEntries, SetlistSource, SongCandidate, SourceLink,
    PLAYLIST_FORMAT_VERSION,
};

/// The whole build (AI, Tidal, resolving) must finish within this.
pub const BUILD_DEADLINE: Duration = Duration::from_secs(150);

/// What the build works with. Borrowed: the app owns all of it.
pub struct BuildContext<'a> {
    /// Clamped inside.
    pub settings: &'a PlaylistSettings,
    /// `None` = no AI set up.
    pub ai: Option<&'a AiConfig>,
    /// `None` = not signed in to Tidal.
    pub api: Option<&'a TidalApi>,
    pub library: &'a LibraryIndex,
    /// Language of notes, names and fallback text.
    pub lang: Lang,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcertBuilt {
    /// Not saved: the app saves it on "Save".
    pub playlist: HomePlaylist,
    /// Why the result isn't what was asked, already in `ctx.lang`.
    pub fallback_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConcertProgress {
    Searching { with_ai: bool },
    ShowsFound { count: usize },
    Resolving { done: usize, total: usize },
}

type Progress<'a> = &'a (dyn Fn(ConcertProgress) + Send + Sync);

/// Builds a playlist for `request.artist`. See the module docs.
pub async fn build(request: &ConcertRequest, ctx: &BuildContext<'_>, progress: Progress<'_>) -> Result<ConcertBuilt> {
    build_within(request, ctx, progress, BUILD_DEADLINE).await
}

pub(crate) async fn build_within(
    request: &ConcertRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    deadline: Duration,
) -> Result<ConcertBuilt> {
    tokio::time::timeout(deadline, build_inner(request, ctx, progress))
        .await
        .unwrap_or_else(|_| Err(words::problem(ctx.lang, Problem::TooSlow)))
}

/// Re-aggregates the shows stored in `playlist` (no AI call): another mode,
/// fewer shows, other thresholds. Keeps id, name-less identity and creation time.
pub async fn rearrange(
    playlist: &HomePlaylist,
    request: &ConcertRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
) -> Result<HomePlaylist> {
    rearrange_within(playlist, request, ctx, progress, BUILD_DEADLINE).await
}

pub(crate) async fn rearrange_within(
    playlist: &HomePlaylist,
    request: &ConcertRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    deadline: Duration,
) -> Result<HomePlaylist> {
    tokio::time::timeout(deadline, rearrange_inner(playlist, request, ctx, progress))
        .await
        .unwrap_or_else(|_| Err(words::problem(ctx.lang, Problem::TooSlow)))
}

/// What the request asks, with every number inside its range.
struct Plan<'a> {
    artist: &'a str,
    shows: u8,
    thresholds: ConcertThresholds,
    settings: PlaylistSettings,
}

fn plan<'a>(request: &'a ConcertRequest, ctx: &BuildContext<'_>) -> Result<Plan<'a>> {
    let artist = request.artist.trim();
    if artist.is_empty() {
        return Err(words::problem(ctx.lang, Problem::NoArtistName));
    }
    Ok(Plan {
        artist,
        shows: request.shows.clamp(MIN_CONCERT_SHOWS, MAX_CONCERT_SHOWS),
        thresholds: request.thresholds.clamped(),
        settings: ctx.settings.clamped(),
    })
}

/// The AI's answer, or the note that says why there is none.
enum Search {
    Shows(Box<ShowsFound>),
    Fallback(String),
}

/// Asks the AI, unless the chain says not to.
async fn search_shows(plan: &Plan<'_>, ctx: &BuildContext<'_>, progress: Progress<'_>) -> Search {
    let note = |reason| Search::Fallback(words::fallback_note(ctx.lang, reason, plan.artist));
    let Some(ai) = ctx.ai else { return note(Fallback::NoAi) };
    if !ai.provider.web_search() {
        return note(Fallback::NoWebSearch { provider: ai.provider.short_name() });
    }
    progress(ConcertProgress::Searching { with_ai: true });
    match recent_shows(ai, plan.artist, plan.shows, plan.settings.search_budget).await {
        Ok(found) if found.shows.len() >= MIN_TOUR_SHOWS => Search::Shows(Box::new(found)),
        Ok(_) => note(Fallback::FewShows),
        Err(e) => {
            // already scrubbed of the key
            tracing::warn!("{e}");
            note(Fallback::AiFailed { message: &e.humane_in(ctx.lang) })
        }
    }
}

async fn build_inner(request: &ConcertRequest, ctx: &BuildContext<'_>, progress: Progress<'_>) -> Result<ConcertBuilt> {
    let plan = plan(request, ctx)?;
    let search = if request.popular_only { None } else { Some(search_shows(&plan, ctx, progress).await) };
    let now = now_ms();
    match search {
        Some(Search::Shows(found)) => {
            let found = *found;
            progress(ConcertProgress::ShowsFound { count: found.shows.len() });
            let arranged = assemble(&plan, &request.mode, &found.shows, ctx, progress).await?;
            let origin = ConcertOrigin {
                artist: plan.artist.to_owned(),
                source: found.source,
                mode: request.mode.clone(),
                shows_wanted: plan.shows,
                thresholds: plan.thresholds,
                shows: found.shows,
                fetched_ms: now,
            };
            let playlist = fresh_playlist(origin, arranged, found.sources, found.search_widget_html, now);
            let fallback_note = ctx.api.is_none().then(|| words::no_tidal_note(ctx.lang));
            Ok(ConcertBuilt { playlist, fallback_note })
        }
        other => {
            let fallback_note = match other {
                Some(Search::Fallback(note)) => Some(note),
                _ => None,
            };
            let playlist = popular_playlist(&plan, ctx, progress, now).await?;
            Ok(ConcertBuilt { playlist, fallback_note })
        }
    }
}

/// Tidal's most played for the artist: not a real setlist.
async fn popular_playlist(
    plan: &Plan<'_>,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
    now: u64,
) -> Result<HomePlaylist> {
    let Some(api) = ctx.api else { return Err(words::problem(ctx.lang, Problem::NothingToUse)) };
    progress(ConcertProgress::Searching { with_ai: false });
    let Some((name, tracks)) = most_played(api, plan.artist, POPULAR_LIMIT).await? else {
        return Err(words::problem(ctx.lang, Problem::NoSuchArtist(plan.artist)));
    };
    let entries = tracks
        .iter()
        .map(|t| entry_from_track(t, ctx.library, plan.settings.prefer_library, EntryTag::Plain, String::new()))
        .collect();
    let origin = ConcertOrigin {
        artist: name.clone(),
        source: SetlistSource::TidalPopular,
        mode: ConcertMode::Tour,
        shows_wanted: plan.shows,
        thresholds: plan.thresholds,
        shows: Vec::new(),
        fetched_ms: now,
    };
    let arranged = Arranged {
        entries,
        unmatched: Vec::new(),
        name: words::popular_name(ctx.lang, &name),
        description: words::popular_description(ctx.lang, &name),
    };
    Ok(fresh_playlist(origin, arranged, Vec::new(), None, now))
}

/// The songs of a setlist resolved to entries, with the playlist's name.
struct Arranged {
    entries: Vec<PlaylistEntry>,
    unmatched: Vec<SongCandidate>,
    name: String,
    description: String,
}

fn fresh_playlist(
    origin: ConcertOrigin,
    arranged: Arranged,
    sources: Vec<SourceLink>,
    search_widget_html: Option<String>,
    now: u64,
) -> HomePlaylist {
    HomePlaylist {
        version: PLAYLIST_FORMAT_VERSION,
        id: new_id(),
        name: arranged.name,
        description: arranged.description,
        created_ms: now,
        updated_ms: now,
        origin: PlaylistOrigin::Concert(origin),
        entries: arranged.entries,
        unmatched: arranged.unmatched,
        sources,
        search_widget_html,
        tidal_uuid: None,
    }
}

/// The set of songs for a mode. A show that doesn't exist is `None`.
fn song_set(
    mode: &ConcertMode,
    shows: &[ConcertShow],
    thresholds: &ConcertThresholds,
) -> Option<(TourSet, Option<usize>)> {
    match mode {
        ConcertMode::Tour => Some((tour_set(shows, thresholds), None)),
        ConcertMode::Show { show_id } => {
            let at = shows.iter().position(|s| &s.id == show_id)?;
            Some((show_set(&shows[at]), Some(at)))
        }
    }
}

/// Shared by `build` and `rearrange`: set -> candidates -> resolver -> entries.
/// `shows` are the shows to consider (most recent first).
async fn assemble(
    plan: &Plan<'_>,
    mode: &ConcertMode,
    shows: &[ConcertShow],
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
) -> Result<Arranged> {
    let lang = ctx.lang;
    let (set, show_at) =
        song_set(mode, shows, &plan.thresholds).ok_or_else(|| words::problem(lang, Problem::NoShows))?;
    let single = matches!(mode, ConcertMode::Show { .. });
    let candidates: Vec<SongCandidate> = set
        .songs
        .iter()
        .filter(|s| !(plan.settings.skip_tape && s.tape))
        .map(|song| {
            let covered = song.original_artist.as_deref().filter(|_| plan.settings.covers_original_artist);
            SongCandidate {
                title: song.name.clone(),
                artist: covered.unwrap_or(plan.artist).to_owned(),
                tag: song.tag,
                note: if single {
                    words::show_song_note(lang, song)
                } else {
                    words::song_note(lang, song, set.shows_considered)
                },
            }
        })
        .collect();

    let resolver = Resolver { api: ctx.api, library: ctx.library, prefer_library: plan.settings.prefer_library };
    let resolved =
        resolver.resolve_all(candidates, &|done, total| progress(ConcertProgress::Resolving { done, total })).await;

    let mut seen = SeenEntries::default();
    let (mut entries, mut unmatched) = (Vec::new(), Vec::new());
    for r in resolved {
        match r.entry {
            // first wins; a file and its Tidal twin don't double up
            Some(entry) if seen.first_time(&entry) => entries.push(entry),
            Some(_) => {}
            None => unmatched.push(r.candidate),
        }
    }

    let (name, description) = match show_at.map(|i| &shows[i]) {
        Some(show) => (
            words::show_name(lang, plan.artist, &show.city, &show.date),
            words::show_description(lang, &show.venue, &show.city),
        ),
        None => (words::tour_name(lang, plan.artist), words::tour_description(lang, set.shows_considered)),
    };
    Ok(Arranged { entries, unmatched, name, description })
}

async fn rearrange_inner(
    playlist: &HomePlaylist,
    request: &ConcertRequest,
    ctx: &BuildContext<'_>,
    progress: Progress<'_>,
) -> Result<HomePlaylist> {
    // a party playlist has no shows to rearrange
    let PlaylistOrigin::Concert(old) = &playlist.origin else {
        return Err(words::problem(ctx.lang, Problem::NoShows));
    };
    if old.shows.is_empty() {
        return Err(words::problem(ctx.lang, Problem::NoShows));
    }
    let mut plan = plan(request, ctx)?;
    let artist = if old.artist.trim().is_empty() { plan.artist } else { old.artist.as_str() };
    plan.artist = artist;
    let considered = &old.shows[..old.shows.len().min(usize::from(plan.shows))];
    // a show picked from the stored list may lie beyond the first `shows`
    let arranged =
        assemble(&plan, &request.mode, considered_or_all(&request.mode, considered, &old.shows), ctx, progress).await?;
    let origin = ConcertOrigin {
        mode: request.mode.clone(),
        shows_wanted: plan.shows,
        thresholds: plan.thresholds,
        ..old.clone()
    };
    let mut rebuilt = HomePlaylist {
        name: arranged.name,
        description: arranged.description,
        origin: PlaylistOrigin::Concert(origin),
        entries: arranged.entries,
        unmatched: arranged.unmatched,
        updated_ms: now_ms(),
        ..playlist.clone()
    };
    rebuilt.keep_identity_of(playlist);
    Ok(rebuilt)
}

/// Tour mode looks at the first `wanted` shows; a single show can be any stored one.
fn considered_or_all<'a>(
    mode: &ConcertMode,
    considered: &'a [ConcertShow],
    all: &'a [ConcertShow],
) -> &'a [ConcertShow] {
    match mode {
        ConcertMode::Tour => considered,
        ConcertMode::Show { .. } => all,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn a_slow_build_ends_with_a_worded_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;
        let config = AiConfig::new(crate::ai::AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let (settings, library) = (PlaylistSettings::default(), LibraryIndex::default());
        let ctx = BuildContext { settings: &settings, ai: Some(&config), api: None, library: &library, lang: Lang::En };
        let request = ConcertRequest { artist: "Korn".into(), ..ConcertRequest::default() };
        let started = std::time::Instant::now();
        let err = build_within(&request, &ctx, &|_| {}, Duration::from_millis(100)).await.unwrap_err();
        assert_eq!(err.to_string(), "Building the playlist took too long. Try again.");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
