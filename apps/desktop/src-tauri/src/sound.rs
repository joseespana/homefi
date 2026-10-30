//! Taste layers, loudness and "How does it sound?" (`src/lib/api.ts` →
//! "sound"): the settings, what the engine needs to know about each file
//! (genre, album, artist, where the queue came from) and the reactions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use homefi_core::ai::{
    self, AssistantTurn, ListeningSummary, OutputKind, ReactionContext, ReactionResult, SoundReaction, SoundSnapshot,
    UndoStack,
};
use homefi_core::dsp::{
    self, split_genres, CrossfadeSettings, ItemRef, LoudnessSettings, NowPlayingContext, SourceContext, TasteRule,
    TasteScope, TasteSettings, TrackSound,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::{AppState, TrackView};

type CmdResult<T> = Result<T, String>;

/// How long the listening summary is reused.
const SUMMARY_TTL: Duration = Duration::from_secs(30 * 60);
/// Paths remembered with their source (the queue rarely holds more).
const MAX_SOURCES: usize = 5000;

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
}

/// Remembers that `path` was queued from `source` (and is Tidal track `tidal_id`).
pub fn remember<R: Runtime>(app: &AppHandle<R>, path: &Path, source: Option<SourceContext>, tidal_id: Option<String>) {
    let Some(sound) = app.try_state::<Sound>() else { return };
    let mut map = sound.sources.lock().unwrap();
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
        sound.sources.lock().unwrap().clear();
    }
}

/// What the engine needs to know about a file.
pub fn track_sound<R: Runtime>(app: &AppHandle<R>, path: &Path) -> TrackSound {
    let Some(state) = app.try_state::<AppState>() else { return TrackSound::default() };
    let (source, tidal_id) =
        app.try_state::<Sound>().and_then(|s| s.sources.lock().unwrap().get(path).cloned()).unwrap_or_default();
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

/// The track playing and what taste layers can say about it.
#[derive(Debug, Clone, Serialize)]
pub struct NowSound {
    pub context: Option<NowPlayingContext>,
    /// Scopes to offer, most specific first.
    pub scopes: Vec<TasteScope>,
    /// The rule that plays now.
    pub rule: Option<TasteRule>,
}

#[tauri::command(async)]
pub fn sound_now(app: AppHandle) -> NowSound {
    let taste = app.state::<AppState>().settings().taste;
    let context = now_context(&app);
    let scopes = context.as_ref().map(NowPlayingContext::scopes).unwrap_or_default();
    let rule = context.as_ref().and_then(|c| dsp::resolve_taste(&taste, c).map(|(r, _)| r.clone()));
    NowSound { context, scopes, rule }
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
pub fn taste_preview(app: AppHandle, rule: TasteRule) -> TastePreview {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let output = state.player.snapshot().stream.map(|s| s.device).or(settings.output_device.clone());
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
    if let Some((at, s)) = sound.summary.lock().unwrap().as_ref() {
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
    *sound.summary.lock().unwrap() = Some((Instant::now(), s.clone()));
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

/// "Hear without EQ" also mutes the taste layers (loudness stays: the
/// comparison is at the same level).
pub fn bypass(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    let mut taste = state.settings().taste;
    if on {
        taste.enabled = false;
    }
    state.player.set_taste(taste);
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
