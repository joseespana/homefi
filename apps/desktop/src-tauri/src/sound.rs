//! Taste layers, loudness and "How does it sound?" (`src/lib/api.ts` →
//! "sound"): the settings, what the engine needs to know about each file
//! (genre, album, artist, where the queue came from) and the reactions.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use homefi_core::ai::{
    self, AssistantTurn, ListeningSummary, OutputKind, ReactionContext, ReactionResult, SoundReaction, SoundSnapshot,
    UndoStack,
};
use homefi_core::dsp::{
    self, split_genres, CrossfadeSettings, ItemRef, LoudnessSettings, NowPlayingContext, SavedSound, SourceContext,
    TargetHit, TargetMove, TargetSource, TasteAudition, TasteCreator, TasteError, TasteRule, TasteScope, TasteSettings,
    TrackSound,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::{AppState, TrackView};

type CmdResult<T> = Result<T, String>;

/// How long the listening summary is reused.
const SUMMARY_TTL: Duration = Duration::from_secs(30 * 60);
/// Paths remembered with their source (the queue rarely holds more).
const MAX_SOURCES: usize = 5000;
/// Library candidates gathered for one target search (the ranking keeps far fewer).
const MAX_LIBRARY_CANDIDATES: usize = 200;
/// Target hits returned to the UI.
const MAX_TARGET_HITS: usize = 30;
/// How long Tidal gets to answer a target search before only the library is shown.
const TIDAL_SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

/// The guarded value even if another thread panicked holding the lock.
fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Where a queued file came from, and its Tidal id.
type Origin = (Option<SourceContext>, Option<String>);

/// What the app remembers for taste layers: where each queued file came from,
/// the undo of the last reactions and the listening summary.
#[derive(Default)]
pub struct Sound {
    /// Path → (where the queue came from, Tidal id).
    sources: Mutex<HashMap<PathBuf, Origin>>,
    undo: UndoStack,
    summary: Mutex<Option<(Instant, ListeningSummary)>>,
    /// The Try on the track playing (the player holds the same one).
    audition: Mutex<Option<TasteAudition>>,
}

/// Remembers that `path` was queued from `source` (and is Tidal track `tidal_id`).
pub fn remember<R: Runtime>(app: &AppHandle<R>, path: &Path, source: Option<SourceContext>, tidal_id: Option<String>) {
    let Some(sound) = app.try_state::<Sound>() else { return };
    let mut map = locked(&sound.sources);
    if map.len() > MAX_SOURCES {
        map.clear();
    }
    let entry = map.entry(path.to_path_buf()).or_default();
    if source.is_some() {
        entry.0 = source;
    }
    if tidal_id.is_some() {
        entry.1 = tidal_id;
    }
}

/// A new play replaces the sources of what was queued before.
pub fn forget_sources<R: Runtime>(app: &AppHandle<R>) {
    if let Some(sound) = app.try_state::<Sound>() {
        locked(&sound.sources).clear();
    }
}

/// What the engine needs to know about a file.
pub fn track_sound<R: Runtime>(app: &AppHandle<R>, path: &Path) -> TrackSound {
    let Some(state) = app.try_state::<AppState>() else { return TrackSound::default() };
    let (source, tidal_id) =
        app.try_state::<Sound>().and_then(|s| locked(&s.sources).get(path).cloned()).unwrap_or_default();
    // streamed tracks are `<stream-cache>/<id>.<ext>`
    let tidal_id = tidal_id.or_else(|| {
        let in_cache = path.parent() == Some(crate::stream::cache_dir(&state).as_path());
        let stem = path.file_stem()?.to_str()?;
        (in_cache && stem.chars().all(|c| c.is_ascii_digit())).then(|| stem.to_owned())
    });
    let track = state.track(path);
    TrackSound { context: context_of(path, track.as_ref(), source, tidal_id), replay_gain: None }
}

fn context_of(
    path: &Path,
    t: Option<&TrackView>,
    source: Option<SourceContext>,
    tidal_id: Option<String>,
) -> NowPlayingContext {
    let mut artists: Vec<ItemRef> = Vec::new();
    if let Some(t) = t {
        for name in t.artist.split([';', '/']).chain(std::iter::once(t.album_artist.as_str())) {
            let name = name.trim();
            if !name.is_empty() && !artists.iter().any(|a| a.name.eq_ignore_ascii_case(name)) {
                artists.push(ItemRef::named(name));
            }
        }
    }
    NowPlayingContext {
        track_id: tidal_id,
        path: Some(path.to_string_lossy().into_owned()),
        title: t.map(|t| t.title.clone()).filter(|s| !s.is_empty()),
        album: t.filter(|t| !t.album.is_empty()).map(|t| ItemRef::named(t.album.clone())),
        artists,
        genres: t.map(|t| split_genres(&t.genre)).unwrap_or_default(),
        source,
    }
}

/// What is playing now, as taste rules see it.
fn now_context(app: &AppHandle) -> Option<NowPlayingContext> {
    let state = app.state::<AppState>();
    let snapshot = state.player.snapshot();
    let path = snapshot.current.and_then(|i| snapshot.queue.get(i))?.clone();
    Some(track_sound(app, &path).context)
}

/// The output sounds are resolved for: the device playing, else the chosen one.
fn current_output(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    state.player.snapshot().stream.map(|s| s.device).or_else(|| state.settings().output_device)
}

/// The stored Try if it is still about the track playing; else it is dropped.
fn live_audition(app: &AppHandle, now: Option<&NowPlayingContext>) -> Option<TasteAudition> {
    let sound = app.state::<Sound>();
    let mut stored = locked(&sound.audition);
    if !stored.as_ref().is_some_and(|a| now.is_some_and(|c| a.applies_to(c))) {
        *stored = None;
    }
    stored.clone()
}

/// Gives the player its settings and the way to learn about files.
pub fn init(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings();
    state.player.set_taste(s.taste.clone());
    state.player.set_loudness(s.loudness);
    state.player.set_crossfade(s.crossfade);
    state.player.set_output_options(s.output);
    let handle = app.clone();
    state.player.set_sound_source(std::sync::Arc::new(move |path: &Path| track_sound(&handle, path)));
}

/// Sends the saved layers to the player and the UI.
fn apply(app: &AppHandle, saved: &crate::settings::DesktopSettings) {
    let state = app.state::<AppState>();
    state.player.set_taste(saved.taste.clone());
    state.player.set_loudness(saved.loudness);
    let _ = app.emit("settings", saved);
}

#[tauri::command(async)]
pub fn taste_get(app: AppHandle) -> TasteSettings {
    app.state::<AppState>().settings().taste
}

/// Saves and applies the taste rules (tilts brought in range).
#[tauri::command(async)]
pub fn taste_set(app: AppHandle, taste: TasteSettings) -> CmdResult<TasteSettings> {
    let taste = taste.clamped();
    let saved = app.state::<AppState>().save_settings(|s| s.taste = taste)?;
    apply(&app, &saved);
    Ok(saved.taste)
}

#[tauri::command(async)]
pub fn loudness_get(app: AppHandle) -> LoudnessSettings {
    app.state::<AppState>().settings().loudness
}

#[tauri::command(async)]
pub fn loudness_set(app: AppHandle, loudness: LoudnessSettings) -> CmdResult<LoudnessSettings> {
    let loudness = loudness.clamped();
    let saved = app.state::<AppState>().save_settings(|s| s.loudness = loudness)?;
    apply(&app, &saved);
    Ok(saved.loudness)
}

#[tauri::command(async)]
pub fn crossfade_get(app: AppHandle) -> CrossfadeSettings {
    app.state::<AppState>().settings().crossfade
}

/// Saves and applies the crossfade (from the next transition); returns it clamped.
#[tauri::command(async)]
pub fn crossfade_set(app: AppHandle, crossfade: CrossfadeSettings) -> CmdResult<CrossfadeSettings> {
    let crossfade = crossfade.clamped();
    let state = app.state::<AppState>();
    let saved = state.save_settings(|s| s.crossfade = crossfade)?;
    state.player.set_crossfade(saved.crossfade);
    let _ = app.emit("settings", &saved);
    Ok(saved.crossfade)
}

#[tauri::command(async)]
pub fn output_options_get(app: AppHandle) -> homefi_player::OutputOptions {
    app.state::<AppState>().settings().output
}

/// Saves the surround and resampler options; the track playing restarts where it is.
#[tauri::command(async)]
pub fn output_options_set(
    app: AppHandle,
    options: homefi_player::OutputOptions,
) -> CmdResult<homefi_player::OutputOptions> {
    let state = app.state::<AppState>();
    let saved = state.save_settings(|s| s.output = options)?;
    state.player.set_output_options(saved.output);
    let _ = app.emit("settings", &saved);
    Ok(saved.output)
}

/// Saves a change to the sounds and applies it; the error text is for the user.
fn edit_taste<T>(app: &AppHandle, change: impl FnOnce(&mut TasteSettings) -> Result<T, TasteError>) -> CmdResult<T> {
    let mut outcome = None;
    let saved = app.state::<AppState>().save_settings(|s| outcome = Some(change(&mut s.taste)))?;
    let value = outcome.ok_or_else(|| "The sound could not be changed.".to_owned())?.map_err(|e| e.to_string())?;
    apply(app, &saved);
    Ok(value)
}

/// Puts `scope` in the sound `sound_id`, taking it out of the ones that overlap.
#[tauri::command(async)]
pub fn taste_assign(app: AppHandle, sound_id: String, scope: TasteScope) -> CmdResult<Vec<TargetMove>> {
    edit_taste(&app, |t| t.assign(&sound_id, scope, TasteCreator::Manual))
}

#[tauri::command(async)]
pub fn taste_unassign(app: AppHandle, sound_id: String, scope: TasteScope) -> CmdResult<()> {
    edit_taste(&app, |t| {
        t.unassign(&sound_id, &scope);
        Ok(())
    })
}

/// Undo of an assignment: each target goes back to the sound it came from.
#[tauri::command(async)]
pub fn taste_undo_moves(app: AppHandle, moves: Vec<TargetMove>) -> CmdResult<()> {
    edit_taste(&app, |t| {
        t.undo_moves(&moves);
        Ok(())
    })
}

#[tauri::command(async)]
pub fn taste_set_outputs(app: AppHandle, sound_id: String, outputs: Vec<String>) -> CmdResult<Vec<TargetMove>> {
    edit_taste(&app, |t| t.set_outputs(&sound_id, outputs))
}

/// Adds or replaces a sound (by id): "+ New", rename, curve edits.
#[tauri::command(async)]
pub fn taste_save(app: AppHandle, rule: TasteRule) -> CmdResult<SavedSound> {
    edit_taste(&app, |t| Ok(t.upsert(rule)))
}

#[tauri::command(async)]
pub fn taste_remove(app: AppHandle, sound_id: String) -> CmdResult<bool> {
    edit_taste(&app, |t| Ok(t.remove(&sound_id)))
}

/// Tries `rule` on the track playing (never saved); `None` stops trying.
/// `false` when nothing plays, the Sound master is off or the track can't be tried.
#[tauri::command(async)]
pub fn taste_audition(app: AppHandle, rule: Option<TasteRule>) -> CmdResult<bool> {
    let state = app.state::<AppState>();
    let sound = app.state::<Sound>();
    let Some(rule) = rule else {
        *locked(&sound.audition) = None;
        state.player.set_audition(None);
        return Ok(true);
    };
    if !state.settings().eq.enabled {
        return Ok(false);
    }
    let Some(audition) = now_context(&app).and_then(|ctx| TasteAudition::new(rule, &ctx)) else { return Ok(false) };
    *locked(&sound.audition) = Some(audition.clone());
    state.player.set_audition(Some(audition));
    Ok(true)
}

/// "Keep": the Try (or a proposal) becomes a saved sound for `scope`, or its
/// curve replaces the one it came from. Ends the Try if it was this rule.
#[tauri::command(async)]
pub fn taste_keep(
    app: AppHandle,
    rule: TasteRule,
    scope: TasteScope,
    only_on: Option<String>,
) -> CmdResult<SavedSound> {
    let by = rule.created_by;
    let tried = locked(&app.state::<Sound>().audition).as_ref().is_some_and(|a| a.rule.id == rule.id);
    let saved = edit_taste(&app, |t| t.keep(&rule, scope, only_on, by))?;
    if tried {
        *locked(&app.state::<Sound>().audition) = None;
        app.state::<AppState>().player.set_audition(None);
    }
    Ok(saved)
}

/// Candidates from the library tags for `query` (no disk access, capped).
fn library_hits(state: &AppState, query: &str) -> Vec<TargetHit> {
    let query = query.trim().to_lowercase();
    let mut seen: HashSet<(&'static str, String)> = HashSet::new();
    let mut hits: Vec<TargetHit> = Vec::new();
    let count = std::cell::Cell::new(0usize);
    let mut add = |scope: TasteScope, subtitle: String, key: String| {
        if count.get() < MAX_LIBRARY_CANDIDATES && seen.insert((scope.kind(), key)) {
            count.set(count.get() + 1);
            hits.push(TargetHit { scope, subtitle, source: TargetSource::Library, in_sound: None });
        }
    };
    let has = |text: &str| text.to_lowercase().contains(&query);
    for t in locked(&state.library).iter() {
        if count.get() >= MAX_LIBRARY_CANDIDATES {
            break;
        }
        let by = if t.album_artist.trim().is_empty() { t.artist.as_str() } else { t.album_artist.as_str() };
        if has(&t.title) {
            let scope = TasteScope::Track { id: t.path.clone(), name: t.title.clone() };
            add(scope, format!("Song · {}", t.artist), t.path.clone());
        }
        if !t.album.is_empty() && has(&t.album) {
            let scope = TasteScope::Album { id: String::new(), name: t.album.clone() };
            add(scope, format!("Album · {by}"), t.album.to_lowercase());
        }
        for name in t.artist.split([';', '/']).chain(std::iter::once(t.album_artist.as_str())) {
            let name = name.trim();
            if !name.is_empty() && has(name) {
                add(
                    TasteScope::Artist { id: String::new(), name: name.to_owned() },
                    "Artist".into(),
                    name.to_lowercase(),
                );
            }
        }
        for genre in split_genres(&t.genre).into_iter().filter(|g| has(g)) {
            let key = genre.to_lowercase();
            add(TasteScope::Genre { name: genre }, "Genre".into(), key);
        }
    }
    // playlists already found on disk (nothing is scanned here)
    if let Some(lists) = crate::commands::cached_local_playlists(&state.settings().library_roots()) {
        for p in lists.iter().filter(|p| has(&p.name)) {
            add(TasteScope::Playlist { id: p.path.clone(), name: p.name.clone() }, "Playlist".into(), p.path.clone());
        }
    }
    hits
}

/// What Tidal finds for `query`; empty when it is slow, fails or isn't connected.
async fn tidal_hits(state: &AppState, query: &str) -> Vec<TargetHit> {
    let Ok(api) = state.api() else { return Vec::new() };
    let found = match tokio::time::timeout(TIDAL_SEARCH_TIMEOUT, api.search(query)).await {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => {
            tracing::debug!("Tidal target search failed: {e}");
            return Vec::new();
        }
        Err(_) => {
            tracing::debug!("Tidal target search timed out");
            return Vec::new();
        }
    };
    let by = |main: Option<&homefi_core::models::ArtistRef>, all: &[homefi_core::models::ArtistRef]| {
        main.or(all.first()).map(|a| a.name.clone()).unwrap_or_default()
    };
    let hit = |scope: TasteScope, subtitle: String| TargetHit {
        scope,
        subtitle,
        source: TargetSource::Tidal,
        in_sound: None,
    };
    let mut hits = Vec::new();
    hits.extend(
        found
            .artists
            .items
            .iter()
            .map(|a| hit(TasteScope::Artist { id: a.id.to_string(), name: a.name.clone() }, "Artist".into())),
    );
    hits.extend(found.albums.items.iter().map(|a| {
        let scope = TasteScope::Album { id: a.id.to_string(), name: a.title.clone() };
        hit(scope, format!("Album · {}", by(a.artist.as_ref(), &a.artists)))
    }));
    hits.extend(found.tracks.items.iter().map(|t| {
        let scope = TasteScope::Track { id: t.id.to_string(), name: t.title.clone() };
        hit(scope, format!("Song · {}", by(t.artist.as_ref(), &t.artists)))
    }));
    hits.extend(found.playlists.items.iter().map(|p| {
        let subtitle = if p.number_of_tracks > 0 {
            format!("Playlist · {} tracks", p.number_of_tracks)
        } else {
            "Playlist".to_owned()
        };
        hit(TasteScope::Playlist { id: p.uuid.clone(), name: p.title.clone() }, subtitle)
    }));
    hits
}

/// "Add" in a sound: songs, albums, artists, playlists and genres matching
/// `query`, from the library and (when connected) Tidal, best first.
#[tauri::command]
pub async fn sound_search_targets(app: AppHandle, query: String) -> CmdResult<Vec<TargetHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let state = app.state::<AppState>();
    let mut hits = library_hits(&state, &query);
    hits.extend(tidal_hits(&state, query.trim()).await);
    let taste = state.settings().taste;
    Ok(dsp::rank_targets(&query, hits, &taste, current_output(&app).as_deref(), MAX_TARGET_HITS))
}

/// The track playing and what sounds can say about it.
#[derive(Debug, Clone, Serialize)]
pub struct NowSound {
    pub context: Option<NowPlayingContext>,
    /// Scopes to offer, most specific first (track, album, artist, playlist, genre).
    pub scopes: Vec<TasteScope>,
    /// The sound playing now (the Try while auditioning).
    pub rule: Option<TasteRule>,
    /// Which of its targets matched this track.
    pub target: Option<TasteScope>,
    /// The output it resolves for (playing device, else the chosen one).
    pub output: Option<String>,
    pub auditioning: bool,
}

#[tauri::command(async)]
pub fn sound_now(app: AppHandle) -> NowSound {
    let settings = app.state::<AppState>().settings();
    let output = current_output(&app);
    let context = now_context(&app);
    // a Try only counts while the master is on and the track is the one tried
    let audition = live_audition(&app, context.as_ref()).filter(|_| settings.eq.enabled);
    let scopes = context.as_ref().map(NowPlayingContext::scopes).unwrap_or_default();
    let resolved = context
        .as_ref()
        .and_then(|c| dsp::resolve_taste_with(&settings.taste, output.as_deref().unwrap_or(""), audition.as_ref(), c));
    let (rule, target, auditioning) = match resolved {
        Some(r) => (Some(r.rule.clone()), Some(r.target.clone()), r.auditioning),
        None => (None, None, false),
    };
    NowSound { context, scopes, rule, target, output, auditioning }
}

/// What a taste rule's editor draws: the output's own profile (faint, when
/// the equalizer is on), the rule's curve alone and both together as the
/// player runs them (with the preamp), plus what the taste caps changed.
#[derive(Debug, Clone, Serialize)]
pub struct TastePreview {
    /// The output it would play on (the one playing, else the chosen one).
    pub output: Option<String>,
    pub device: Option<dsp::EqProfile>,
    pub taste: dsp::EqProfile,
    pub combined: dsp::EqProfile,
    /// The rule's curve as it will play (in range, safe for the output).
    pub bands: Vec<dsp::TasteBand>,
    pub notes: Vec<String>,
    pub small_speaker: bool,
}

#[tauri::command(async)]
pub fn taste_preview(app: AppHandle, rule: TasteRule, output: Option<String>) -> TastePreview {
    let settings = app.state::<AppState>().settings();
    let output = output.or_else(|| current_output(&app));
    let small = output.as_deref().is_some_and(dsp::is_built_in_speaker);
    let device = output
        .as_deref()
        .and_then(|o| settings.eq.profile_for(o))
        .or(settings.eq.default_profile.as_ref())
        .filter(|_| settings.eq.enabled)
        .map(dsp::EqProfile::clamped);
    let (bands, tilt, notes) = dsp::clamp_taste_curve(&rule.bands, rule.tilt, small);
    let base = device.clone().unwrap_or_else(|| dsp::EqProfile::flat(""));
    let mut combined = dsp::with_taste_rule(&base, &bands, tilt, small);
    combined.preamp_db = dsp::auto_preamp_db(&combined);
    TastePreview {
        output,
        device,
        taste: dsp::taste_profile(&bands, tilt),
        combined,
        bands,
        notes,
        small_speaker: small,
    }
}

fn output_kind(app: &AppHandle) -> OutputKind {
    let device = app.state::<AppState>().player.snapshot().stream.map(|s| s.device);
    match device {
        Some(d) if dsp::is_built_in_speaker(&d) => OutputKind::BuiltIn,
        Some(d) => crate::ai::guess_kind(&d),
        None => OutputKind::Other,
    }
}

/// "How does it sound?": a taste rule for what plays (with AI when a provider
/// is set up, else fixed safe steps), saved and applied at once. `scope`
/// overrides where it goes ("Sound for this album").
#[tauri::command]
pub async fn sound_react(
    app: AppHandle,
    reaction: SoundReaction,
    scope: Option<TasteScope>,
    language: Option<String>,
) -> CmdResult<ReactionResult> {
    let settings = app.state::<AppState>().settings();
    let ctx = ReactionContext {
        language: language.unwrap_or_else(|| "en".into()),
        now_playing: now_context(&app).unwrap_or_default(),
        taste: settings.taste.clone(),
        loudness: settings.loudness,
        output_kind: output_kind(&app),
        scope_hint: scope,
        active_output: current_output(&app),
    };
    let config = crate::ai::settings_config(&app).await;
    let result = ai::react(reaction, &ctx, config.as_ref()).await;
    if result.undo_token.is_empty() {
        return Ok(result);
    }
    let sound = app.state::<Sound>();
    sound.undo.push(&result.undo_token, SoundSnapshot::before(&ctx, &result));
    // merge the rule into the settings as they are now (the AI call took seconds), not a whole copy
    let (change, loudness) = (result.taste_change(), result.loudness);
    let loudness_changed = loudness != ctx.loudness;
    let saved = app.state::<AppState>().save_settings(|s| {
        s.taste.apply(&change);
        if loudness_changed {
            s.loudness = loudness;
        }
    })?;
    apply(&app, &saved);
    Ok(result)
}

/// Puts back what a reaction changed; `false` when it was already undone.
#[tauri::command(async)]
pub fn sound_undo(app: AppHandle, token: String) -> CmdResult<bool> {
    let Some(before) = app.state::<Sound>().undo.undo(&token) else { return Ok(false) };
    // the inverse change on today's rules: what the user did since stays
    let saved = app.state::<AppState>().save_settings(|s| {
        let change = before.undo_change(&s.taste);
        s.taste.apply(&change);
        s.loudness = before.loudness;
    })?;
    apply(&app, &saved);
    Ok(true)
}

/// A proactive taste suggestion for a card. It opens the assistant's
/// conversation: answers go to `eq_assistant_reply`.
#[tauri::command]
pub async fn taste_suggest(app: AppHandle, language: String) -> CmdResult<AssistantTurn> {
    crate::ai::suggest(&app, &language).await
}

/// What the user listens to most (library tags, Tidal favorites and
/// playlists), cached for half an hour.
#[tauri::command]
pub async fn listening_summary(app: AppHandle) -> ListeningSummary {
    summary(&app).await
}

pub async fn summary(app: &AppHandle) -> ListeningSummary {
    let sound = app.state::<Sound>();
    if let Some((at, s)) = locked(&sound.summary).as_ref() {
        if at.elapsed() < SUMMARY_TTL {
            return s.clone();
        }
    }
    let state = app.state::<AppState>();
    let library: Vec<TrackView> = state.library.lock().unwrap().clone();
    let mut artists_tidal = Vec::new();
    let mut playlists = Vec::new();
    if let Ok(api) = state.api() {
        let (a, p) = tokio::join!(api.favorite_artists(0), api.playlists_and_favorites(0));
        if let Ok(page) = a {
            artists_tidal = page.items.into_iter().map(|f| ItemRef::new(f.item.id.to_string(), f.item.name)).collect();
        }
        if let Ok(page) = p {
            playlists =
                page.items.into_iter().map(|e| ItemRef::new(e.playlist.uuid, e.playlist.title)).take(20).collect();
        }
    }
    let s = summarize(&library, artists_tidal, playlists);
    *locked(&sound.summary) = Some((Instant::now(), s.clone()));
    s
}

/// Top genres and artists by how many tracks the library has of them;
/// Tidal favorite artists count as five tracks each.
fn summarize(library: &[TrackView], favorite_artists: Vec<ItemRef>, playlists: Vec<ItemRef>) -> ListeningSummary {
    fn top(counts: HashMap<String, (String, usize)>, n: usize) -> Vec<String> {
        let mut v: Vec<(String, usize)> = counts.into_values().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        v.into_iter().take(n).map(|(name, _)| name).collect()
    }
    let mut genres: HashMap<String, (String, usize)> = HashMap::new();
    let mut artists: HashMap<String, (String, usize)> = HashMap::new();
    for t in library {
        for g in split_genres(&t.genre) {
            genres.entry(g.to_lowercase()).or_insert((g.clone(), 0)).1 += 1;
        }
        let a = if t.album_artist.trim().is_empty() { t.artist.trim() } else { t.album_artist.trim() };
        if !a.is_empty() {
            artists.entry(a.to_lowercase()).or_insert((a.to_owned(), 0)).1 += 1;
        }
    }
    let ids: HashMap<String, String> = favorite_artists.iter().map(|a| (a.name.to_lowercase(), a.id.clone())).collect();
    for a in &favorite_artists {
        artists.entry(a.name.to_lowercase()).or_insert((a.name.clone(), 0)).1 += 5;
    }
    let top_artists = top(artists, 8)
        .into_iter()
        .map(|name| ItemRef::new(ids.get(&name.to_lowercase()).cloned().unwrap_or_default(), name))
        .collect();
    ListeningSummary { top_genres: top(genres, 6), top_artists, playlists }
}

/// "Hear without": while `on` the player runs with no output profiles, no
/// sounds and no Try; off puts everything back. Nothing is saved, loudness
/// stays (the comparison is at the same level).
pub fn bypass(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    if on {
        let (eq, taste) = dsp::hear_without(&settings.eq, &settings.taste);
        state.player.set_eq(eq);
        state.player.set_taste(taste);
        state.player.set_audition(None);
    } else {
        state.player.set_eq(settings.eq);
        state.player.set_taste(settings.taste);
        let audition = live_audition(app, now_context(app).as_ref());
        state.player.set_audition(audition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(genre: &str, artist: &str) -> TrackView {
        TrackView {
            path: format!("/m/{artist}-{genre}.flac"),
            title: "Song".into(),
            artist: artist.into(),
            album: "Album".into(),
            album_artist: String::new(),
            track_number: 1,
            disc_number: 1,
            year: None,
            genre: genre.into(),
            duration_ms: 1,
            format: "FLAC".into(),
            sample_rate: None,
            bit_depth: None,
            lossless: true,
            quality: String::new(),
            isrc: None,
        }
    }

    #[test]
    fn summary_counts_genres_and_artists() {
        let lib =
            vec![track("Electronic, House", "Daft Punk"), track("Electronic", "Justice"), track("Jazz", "Bill Evans")];
        let s = summarize(&lib, vec![ItemRef::new("7", "Bill Evans")], vec![]);
        assert_eq!(s.top_genres[0], "Electronic");
        assert_eq!(s.top_artists[0], ItemRef::new("7", "Bill Evans"));
    }

    #[test]
    fn context_from_tags() {
        let t = TrackView {
            artist: "Daft Punk; Romanthony".into(),
            album_artist: "Daft Punk".into(),
            ..track("House, Electronic", "")
        };
        let c = context_of(
            Path::new("/m/a.flac"),
            Some(&t),
            Some(SourceContext::new("album", "", "Album")),
            Some("9".into()),
        );
        assert_eq!(c.genres, ["House", "Electronic"]);
        assert_eq!(c.artists.len(), 2);
        assert_eq!(c.album.unwrap().name, "Album");
        assert_eq!(c.track_id.as_deref(), Some("9"));
    }
}
