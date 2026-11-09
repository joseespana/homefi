//! Exercise mode (indoor cycling) on the desktop: the Tauri side of `homefi_core::exercise`.
//! Every rule (workouts, tempo fit, the ride clock, the TV contract) lives in the core; this file
//! wires it to the player, the library, the AI settings, the share hub and the windows.
//!
//! A ride moves through [`Stage`]s: `idle` -> `building` -> `ready` -> `riding` -> `finished`
//! (and back to `idle` on `exercise_stop`, or when the rider plays something else).
//!
//! Events:
//! - `ride` ([`RideTick`]): with every player snapshot while riding, and once more when the ride ends.
//!   The view is computed from the position the player reports (`RideClock::view`), the same one
//!   published to the TV on the same tick.
//! - `exercise-state` ([`ExerciseState`]): the stage, the TV and any notice changed.
//!
//! The ride plays in the normal player (the plan's playlist, saved as a HomeFi playlist), so
//! Sounds, lyrics and bit-perfect work as always. The TV is a LAN page (`TvServer`), optionally a
//! second window on the TV's screen, and, with `tv_audio = tv`, a listen-only share hub for the
//! ride whose join code is a one-ride token.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use homefi_core::dj::{analyze_file, AnalysisCache, DjAnalysis};
use homefi_core::exercise::tv::{ride_hub_config, ride_token, RideListen, TvConfig, TvLyrics, TvSession};
use homefi_core::exercise::{
    self as ex, Analyzer, BuildContext, CueKind, ExerciseSession, ExerciseTurn, PresetId, RideBuilt, RideClock,
    RidePlan, RideProgress, RideRequest, RideStore, RideSummary, RideView, TvAudio, TvPart, TvServer,
};
use homefi_core::export::ExportKind;
use homefi_core::humane::{self, Lang};
use homefi_core::playlist::{new_id, HomePlaylist, LibraryIndex, PlaylistEntry, PlaylistOrigin, RideOrigin};
use homefi_player::{RepeatMode, Snapshot, Status};
use serde::Serialize;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex as AsyncMutex;

use crate::commands::{err, AppState};
use crate::playlists::{cancelled, cancelled_text, library_refs, Playlists};
use crate::stream::StreamItem;

type CmdResult<T> = Result<T, String>;

/// Label of the TV window.
const TV_WINDOW: &str = "tv";
/// Size of the TV window when it is not fullscreen (logical px).
const TV_WINDOW_SIZE: (f64, f64) = (1280.0, 720.0);
/// Black while the page loads: never a white flash on the TV.
const TV_BACKGROUND: tauri::window::Color = tauri::window::Color(0, 0, 0, 255);
/// How long the ride hub waits for the first song to be in the player before it attaches.
const FIRST_SONG_WAIT: Duration = Duration::from_secs(25);
const FIRST_SONG_POLL: Duration = Duration::from_millis(100);
/// Name the TV server and the ride hub announce.
const TV_NAME: &str = "HomeFi ride";

/// Wall-clock milliseconds, the ride clock's `now_ms`.
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or_default()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ------------------------------------------------------------------ state

/// Where a ride is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Idle,
    /// The builder is searching and matching songs (`exercise_cancel` stops it).
    Building,
    /// A plan is waiting for `exercise_start`.
    Ready,
    Riding,
    /// The last song ended: the final view stays until `exercise_stop`.
    Finished,
}

/// The ride as the app holds it. Plain data behind one mutex; the transitions are methods so they
/// are tested without a window.
#[derive(Default)]
struct RideState {
    stage: Stage,
    /// The last build (plan and its playlist), until the next one.
    built: Option<RideBuilt>,
    /// The saved ride (`RideStore`) the build or the load came from: Keep and "ridden" refer to it.
    saved_id: Option<String>,
    /// The plan being ridden.
    plan: Option<Arc<RidePlan>>,
    /// Bumped by every start and stop: a task of an older ride sees it and gives up.
    run: u64,
    /// The newest view and whether the music plays (for `exercise_state`).
    last: Option<(RideView, bool)>,
    /// The first / the last song of the plan has been heard.
    seen_first: bool,
    seen_last: bool,
    /// Queue position of the song heard last (for plans that repeat a song).
    last_index: usize,
    /// The song (plan index) already told to move on at its cut (`PlannedSong::play_ms`), so the
    /// player is asked once per song.
    skip_requested_for: Option<usize>,
    /// The song (plan index) whose lyrics were last asked for the TV and whether they were wanted, so the
    /// lookup runs once per song (and again when the TV starts or `tv_lyrics` changes).
    lyrics_for: Option<(usize, bool)>,
    /// The ride's listen-only hub runs (it is ours to stop).
    hub: bool,
    /// What the TV does with the audio, after the conflict rules.
    tv_audio: TvAudio,
    /// The TV server was started by the ride (it stops with it); otherwise by `exercise_tv`.
    tv_by_ride: bool,
    /// Something the rider should know ("you are sharing...").
    notice: Option<String>,
}

impl RideState {
    /// `false` while riding: a build must not replace the music under a ride.
    fn begin_build(&mut self) -> bool {
        if matches!(self.stage, Stage::Riding | Stage::Building) {
            return false;
        }
        self.stage = Stage::Building;
        true
    }

    fn build_done(&mut self, built: RideBuilt, saved_id: Option<String>) {
        self.built = Some(built);
        self.saved_id = saved_id;
        self.stage = Stage::Ready;
    }

    /// A saved ride becomes the ready plan (no build). `false` while riding or building.
    fn load(&mut self, built: RideBuilt, saved_id: String) -> bool {
        if matches!(self.stage, Stage::Riding | Stage::Building) {
            return false;
        }
        self.build_done(built, Some(saved_id));
        true
    }

    /// A failed or cancelled build goes back to what there was before.
    fn build_failed(&mut self) {
        self.stage = if self.built.is_some() { Stage::Ready } else { Stage::Idle };
    }

    fn start(&mut self, plan: Arc<RidePlan>) -> u64 {
        self.run += 1;
        *self = Self {
            run: self.run,
            built: self.built.take(),
            saved_id: self.saved_id.take(),
            plan: Some(plan),
            stage: Stage::Riding,
            ..Self::default()
        };
        self.run
    }

    /// Ends the ride (finished or not): back to `idle`, keeping the last build to start again.
    fn stop(&mut self) {
        self.run += 1;
        *self = Self { run: self.run, built: self.built.take(), saved_id: self.saved_id.take(), ..Self::default() };
        if self.built.is_some() {
            self.stage = Stage::Ready;
        }
    }
}

/// Exercise state managed by Tauri next to [`AppState`].
#[derive(Default)]
pub struct Exercise {
    /// The open AI conversation (held across awaits, hence the async mutex).
    chat: AsyncMutex<Option<ExerciseSession>>,
    ride: Mutex<RideState>,
    /// The TV's LAN server, while it runs.
    tv: Mutex<Option<TvServer>>,
}

// ------------------------------------------------------------------ views

/// What `exercise_state` and the `exercise-state` event carry.
#[derive(Debug, Clone, Serialize)]
pub struct ExerciseState {
    pub stage: Stage,
    /// The plan: when `ready`, `riding` or `finished`.
    pub plan: Option<RidePlan>,
    /// The newest ride view and whether the music plays (`None` before the first tick).
    pub view: Option<RideView>,
    pub playing: bool,
    pub tv: TvState,
    /// Words for the rider (the TV's sound stays on the computer because it is sharing...).
    pub notice: Option<String>,
    /// Id of the saved ride the plan belongs to (every build is saved; `exercise_ride_keep` keeps it).
    pub saved_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TvState {
    /// The LAN page is served.
    pub server: bool,
    /// Where the TV's browser opens it (`None` when the server is off or no LAN address was found).
    pub url: Option<String>,
    /// The second window is open.
    pub window: bool,
    /// The ride's listen-only hub runs (the TV plays the music).
    pub hub: bool,
    /// What the TV does with the audio now (`phone` after a conflict).
    pub audio: TvAudio,
}

/// `ride` event: one tick of the ride.
#[derive(Debug, Clone, Serialize)]
pub struct RideTick {
    pub stage: Stage,
    /// The music plays (paused: the view is frozen).
    pub playing: bool,
    pub view: RideView,
}

/// A built ride, as the screen gets it.
#[derive(Debug, Clone, Serialize)]
pub struct RideBuiltView {
    pub plan: RidePlan,
    /// The playlist the player will play (saved by `exercise_start`).
    pub playlist: HomePlaylist,
    /// The automatic save of this build (`None` if it could not be written): Keep / favorite it by its id.
    pub saved: Option<RideSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresetView {
    pub id: PresetId,
    /// "Easy ride", in the language asked for.
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorView {
    /// As the OS names it; pass it to `exercise_tv_window`.
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

fn state_of(app: &AppHandle) -> ExerciseState {
    let exercise = app.state::<Exercise>();
    let (stage, plan, last, hub, audio, notice, saved_id) = {
        let r = lock(&exercise.ride);
        let plan = match r.stage {
            Stage::Riding | Stage::Finished => r.plan.as_deref().cloned(),
            Stage::Ready => r.built.as_ref().map(|b| b.plan.clone()),
            Stage::Idle | Stage::Building => None,
        };
        (r.stage, plan, r.last.clone(), r.hub, r.tv_audio, r.notice.clone(), r.saved_id.clone())
    };
    let url = lock(&exercise.tv).as_ref().and_then(TvServer::url);
    let server = lock(&exercise.tv).is_some();
    let (view, playing) = last.map_or((None, false), |(v, p)| (Some(v), p));
    ExerciseState {
        stage,
        plan,
        view,
        playing,
        tv: TvState { server, url, window: app.get_webview_window(TV_WINDOW).is_some(), hub, audio },
        notice,
        saved_id,
    }
}

fn emit_state(app: &AppHandle) {
    let _ = app.emit("exercise-state", state_of(app));
}

#[tauri::command]
pub fn exercise_state(app: AppHandle) -> ExerciseState {
    state_of(&app)
}

// ------------------------------------------------------------------ setup helpers

/// The styles, in the language asked for ("en" or "es").
#[tauri::command]
pub fn exercise_presets(language: String) -> Vec<PresetView> {
    let lang = Lang::from_code(&language);
    PresetId::ALL.into_iter().map(|id| PresetView { id, name: ex::style_name(lang, id) }).collect()
}

/// The ride lengths the picker offers: 10, 15, ... 90.
#[tauri::command]
pub fn exercise_ride_minutes() -> Vec<u16> {
    (ex::MIN_RIDE_MINUTES..=ex::MAX_RIDE_MINUTES).step_by(usize::from(ex::RIDE_MINUTES_STEP)).collect()
}

/// An empty request: the settings' style and length, no wish.
#[tauri::command]
pub fn exercise_default_request(state: State<'_, AppState>) -> RideRequest {
    let s = state.settings().exercise.clamped();
    RideRequest { minutes: s.ride_minutes, style: s.default_preset, ..RideRequest::default() }
}

/// The "not medical advice" notice, shown once before the first ride.
#[tauri::command]
pub fn exercise_health_notice(language: String) -> String {
    ex::health_notice(Lang::from_code(&language))
}

// ------------------------------------------------------------------ chat

/// Opens (replacing) the conversation and answers its fixed opener (no AI call yet). The error is
/// the "needs an AI" text when none is set up: the screen then offers the styles only.
#[tauri::command]
pub async fn exercise_open(app: AppHandle, request: RideRequest, language: String) -> CmdResult<ExerciseTurn> {
    let lang = Lang::from_code(&language);
    let Some(config) = crate::ai::settings_config(&app).await else {
        return Err(ex::needs_ai_message(lang));
    };
    let mut session = ExerciseSession::new(config, request, lang);
    let turn = session.start();
    *app.state::<Exercise>().chat.lock().await = Some(session);
    Ok(turn)
}

fn no_session() -> String {
    match humane::language() {
        Lang::Es => "Abre la conversación primero.".into(),
        Lang::En => "Open the conversation first.".into(),
    }
}

/// The rider's answer; the turn carries the AI's words and the request as it now understands it.
#[tauri::command]
pub async fn exercise_reply(app: AppHandle, text: String) -> CmdResult<ExerciseTurn> {
    let exercise = app.state::<Exercise>();
    let mut slot = exercise.chat.lock().await;
    slot.as_mut().ok_or_else(no_session)?.reply(&text).await.map_err(err)
}

#[tauri::command]
pub async fn exercise_close(app: AppHandle) {
    *app.state::<Exercise>().chat.lock().await = None;
}

// ------------------------------------------------------------------ analysis

/// Reads the tempo of a song's audio with the DJ's decoder and analysis cache. A Tidal song is read
/// only when the stream cache already has it (the builder never downloads to analyze).
struct DesktopAnalyzer {
    stream_cache: PathBuf,
    memory_mb: u32,
}

impl DesktopAnalyzer {
    fn file_of(&self, entry: &PlaylistEntry) -> Option<PathBuf> {
        let local = entry.local_path.as_ref().map(PathBuf::from).filter(|p| p.is_file());
        local.or_else(|| cached_file(&self.stream_cache, entry.tidal_id.as_deref()?))
    }
}

/// `<stream cache>/<id>.<ext>`, when it is there.
fn cached_file(dir: &Path, id: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_file() && p.file_stem().is_some_and(|s| s == id))
}

impl Analyzer for DesktopAnalyzer {
    fn analyze<'a>(
        &'a self,
        entry: &'a PlaylistEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<DjAnalysis>> + Send + 'a>> {
        Box::pin(async move {
            let path = self.file_of(entry)?;
            let memory_mb = self.memory_mb;
            // decoding is heavy: off the async runtime, one song at a time (the builder is sequential)
            tauri::async_runtime::spawn_blocking(move || {
                analyze_file(&path, &AnalysisCache::in_app_dir(), memory_mb).ok()
            })
            .await
            .ok()
            .flatten()
        })
    }
}

// ------------------------------------------------------------------ build

/// Builds the ride (`progress` arrives over `on_progress`). `exercise_cancel` stops it. Starting
/// another build while one runs replaces it; building while riding is refused.
#[tauri::command]
pub async fn exercise_build(
    app: AppHandle,
    request: RideRequest,
    language: String,
    on_progress: Channel<RideProgress>,
) -> CmdResult<RideBuiltView> {
    let exercise = app.state::<Exercise>();
    if !lock(&exercise.ride).begin_build() {
        return Err(match humane::language() {
            Lang::Es => "Termina el recorrido en curso antes de armar otro.".into(),
            Lang::En => "Finish the ride in progress before building another.".into(),
        });
    }
    emit_state(&app);
    let playlists = app.state::<Playlists>();
    let rx = playlists.begin();
    let result = run_build(&app, &request, &language, on_progress, rx.clone()).await;
    playlists.finish(&rx);
    // every successful build is saved (so riding it again needs no AI); a failed save never fails the ride
    let saved = match &result {
        Ok(built) => auto_save(&app, &request, built, &language).await,
        Err(_) => None,
    };
    {
        let mut ride = lock(&exercise.ride);
        match &result {
            Ok(built) => ride.build_done(built.clone(), saved.as_ref().map(|s| s.id.clone())),
            Err(_) => ride.build_failed(),
        }
    }
    emit_state(&app);
    result.map(|b| RideBuiltView { plan: b.plan, playlist: b.playlist, saved })
}

async fn run_build(
    app: &AppHandle,
    request: &RideRequest,
    language: &str,
    on_progress: Channel<RideProgress>,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> CmdResult<RideBuilt> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let api = state.api().ok();
    let refs = library_refs(&lock(&state.library));
    let library = tauri::async_runtime::spawn_blocking(move || LibraryIndex::new(refs)).await.map_err(err)?;
    let ai = crate::ai::settings_config(app).await;
    let analyzer = DesktopAnalyzer { stream_cache: crate::stream::cache_dir(&state), memory_mb: settings.dj.memory_mb };
    let ctx = BuildContext {
        settings: &settings.exercise,
        ai: ai.as_ref(),
        api: api.as_ref(),
        library: &library,
        analyzer: Some(&analyzer),
        lang: Lang::from_code(language),
    };
    let progress = move |p: RideProgress| {
        let _ = on_progress.send(p);
    };
    tokio::select! {
        r = ex::build(request, &ctx, &progress) => r.map_err(err),
        () = cancelled(cancel) => Err(cancelled_text()),
    }
}

#[tauri::command]
pub fn exercise_cancel(playlists: State<'_, Playlists>) {
    playlists.cancel();
}

// ------------------------------------------------------------------ saved rides

/// The saved rides on disk (`~/.homefi/exercise/rides`), managed by Tauri.
pub struct Rides {
    store: RideStore,
}

impl Rides {
    pub fn new(app_dir: &Path) -> Self {
        Self { store: RideStore::in_app_dir(app_dir) }
    }
}

/// `exercise-rides` (no payload): the list of saved rides changed (saved, kept, deleted, imported...).
fn emit_rides(app: &AppHandle) {
    let _ = app.emit("exercise-rides", ());
}

/// Saves a finished build with the settings' history cap. `None` when it could not be written.
async fn auto_save(app: &AppHandle, request: &RideRequest, built: &RideBuilt, language: &str) -> Option<RideSummary> {
    let store = app.state::<Rides>().store.clone();
    let max = app.state::<AppState>().settings().exercise.clamped().ride_history_max;
    let (request, plan, playlist_id) = (request.clone(), built.plan.clone(), built.playlist.id.clone());
    let lang = Lang::from_code(language);
    let saved = tauri::async_runtime::spawn_blocking(move || {
        store.auto_save(&request, &plan, Some(playlist_id).filter(|id| !id.is_empty()), lang, max)
    })
    .await;
    match saved {
        Ok(Ok(ride)) => {
            emit_rides(app);
            Some(ride.summary())
        }
        Ok(Err(error)) => {
            tracing::warn!("could not save the ride: {error}");
            None
        }
        Err(error) => {
            tracing::warn!("could not save the ride: {error}");
            None
        }
    }
}

/// A saved ride made ready to start: what `exercise_ride_load` returns.
#[derive(Debug, Clone, Serialize)]
pub struct RideLoaded {
    pub request: RideRequest,
    pub plan: RidePlan,
    /// The playlist `exercise_start` will play and save (the ride's own one when it has one).
    pub playlist: HomePlaylist,
    pub saved: RideSummary,
}

fn ride_not_found() -> String {
    match humane::language() {
        Lang::Es => "No encuentro ese recorrido guardado.".into(),
        Lang::En => "That saved ride was not found.".into(),
    }
}

/// Runs a store call off the async runtime and turns every failure into a message.
async fn with_store<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(RideStore) -> homefi_core::Result<T> + Send + 'static,
) -> CmdResult<T> {
    let store = app.state::<Rides>().store.clone();
    tauri::async_runtime::spawn_blocking(move || job(store)).await.map_err(err)?.map_err(err)
}

/// The saved rides: favorites first, then the most recent.
#[tauri::command]
pub async fn exercise_rides(app: AppHandle) -> Vec<RideSummary> {
    let store = app.state::<Rides>().store.clone();
    tauri::async_runtime::spawn_blocking(move || store.list().iter().map(|r| r.summary()).collect())
        .await
        .unwrap_or_default()
}

/// Puts a saved ride's plan on the screen as `ready` (no build, no AI call); `exercise_start` then rides it.
/// Refused while a ride is on or being built.
#[tauri::command]
pub async fn exercise_ride_load(app: AppHandle, id: String) -> CmdResult<RideLoaded> {
    let ride = with_store(&app, move |store| store.get(&id)).await.map_err(|_| ride_not_found())?;
    let playlist = playlist_of(&ride.plan, ride.playlist_id.clone());
    let built = RideBuilt { plan: ride.plan.clone(), playlist: playlist.clone() };
    if !lock(&app.state::<Exercise>().ride).load(built, ride.id.clone()) {
        return Err(match humane::language() {
            Lang::Es => "Termina el recorrido en curso antes de cargar otro.".into(),
            Lang::En => "Finish the ride in progress before loading another.".into(),
        });
    }
    emit_state(&app);
    Ok(RideLoaded { request: ride.request.clone(), plan: ride.plan.clone(), playlist, saved: ride.summary() })
}

/// The rider keeps the ride (pruning never removes it), with a name of their own if given.
#[tauri::command]
pub async fn exercise_ride_keep(app: AppHandle, id: String, name: Option<String>) -> CmdResult<RideSummary> {
    let ride = with_store(&app, move |store| store.keep(&id, name.as_deref())).await?;
    emit_rides(&app);
    Ok(ride.summary())
}

/// Renames (and keeps) the ride; a blank name changes nothing.
#[tauri::command]
pub async fn exercise_ride_rename(app: AppHandle, id: String, name: String) -> CmdResult<RideSummary> {
    let ride = with_store(&app, move |store| store.rename(&id, &name)).await?;
    emit_rides(&app);
    Ok(ride.summary())
}

#[tauri::command]
pub async fn exercise_ride_favorite(app: AppHandle, id: String, favorite: bool) -> CmdResult<RideSummary> {
    let ride = with_store(&app, move |store| store.set_favorite(&id, favorite)).await?;
    emit_rides(&app);
    Ok(ride.summary())
}

/// Removes HomeFi's own file of the ride (not a library file, so no Trash). `false`: there was none.
#[tauri::command]
pub async fn exercise_ride_delete(app: AppHandle, id: String) -> CmdResult<bool> {
    let wanted = id.clone();
    let removed = with_store(&app, move |store| store.delete(&id)).await?;
    {
        let exercise = app.state::<Exercise>();
        let mut ride = lock(&exercise.ride);
        if ride.saved_id.as_deref() == Some(&*wanted) {
            ride.saved_id = None;
        }
    }
    emit_rides(&app);
    emit_state(&app);
    Ok(removed)
}

/// Writes the ride's `.homefiride` to `path` (picked with the dialog plugin; a Google Drive folder
/// is a folder). `.homefiride` is added when the name has another ending. Returns the path written.
#[tauri::command]
pub async fn exercise_ride_export(app: AppHandle, id: String, path: String) -> CmdResult<String> {
    let text = with_store(&app, move |store| ex::export_ride(&store.get(&id)?)).await?;
    crate::transfer::write_export(&path, ExportKind::Ride, &text).await
}

/// Reads a `.homefiride` from `path` and saves it as a new kept ride. Local songs are matched again
/// in this computer's library; the rest stay as TIDAL songs (a song with neither is skipped when riding).
#[tauri::command]
pub async fn exercise_ride_import(app: AppHandle, path: String) -> CmdResult<RideSummary> {
    let text = crate::transfer::read_export(&path).await?;
    let refs = library_refs(&lock(&app.state::<AppState>().library));
    let ride = with_store(&app, move |store| {
        let index = LibraryIndex::new(refs);
        let ride = ex::import_ride(&text, Some(&index))?;
        store.save(&ride)?;
        Ok(ride)
    })
    .await?;
    emit_rides(&app);
    Ok(ride.summary())
}

// ------------------------------------------------------------------ ride

/// The playlist to play for `plan`: the one the builder made when it is still the same ride,
/// otherwise one made from the plan's songs.
fn playlist_for(plan: &RidePlan, built: Option<&RideBuilt>) -> HomePlaylist {
    if let Some(b) = built.filter(|b| b.plan.workout.id == plan.workout.id) {
        let same = b.playlist.entries.len() == plan.songs.len()
            && b.playlist.entries.iter().zip(&plan.songs).all(|(a, s)| *a == s.entry);
        if same {
            return b.playlist.clone();
        }
    }
    playlist_of(plan, None)
}

/// A playlist made from the plan's songs, with `id` when given (a saved ride reuses its own).
fn playlist_of(plan: &RidePlan, id: Option<String>) -> HomePlaylist {
    let now = now_ms();
    HomePlaylist {
        id: id.filter(|id| !id.is_empty()).unwrap_or_else(new_id),
        name: plan.workout.name.clone(),
        created_ms: now,
        updated_ms: now,
        origin: PlaylistOrigin::Ride(RideOrigin { workout_id: plan.workout.id.clone(), wish: plan.wish.clone() }),
        entries: plan.songs.iter().map(|s| s.entry.clone()).collect(),
        ..HomePlaylist::default()
    }
}

/// The songs as the stream player takes them, each carrying the playlist as its source (Sounds).
/// A local file that is gone (a saved ride from another day or computer) plays from TIDAL when the
/// song has an id, otherwise it is skipped: the ride goes on with the rest.
fn stream_items(playlist: &HomePlaylist) -> Vec<StreamItem> {
    let source = playlist.source_context();
    playlist
        .entries
        .iter()
        .filter_map(|e| {
            let local = e.local_path.as_ref().filter(|p| Path::new(p.as_str()).is_file()).cloned();
            if local.is_none() && e.tidal_id.is_none() {
                return None;
            }
            Some(StreamItem {
                id: e.tidal_id.clone().unwrap_or_default(),
                title: e.title.clone(),
                local,
                artist: e.artist.clone(),
                album: e.album.clone(),
                cover_url: e.cover_url.clone(),
                duration_ms: Some(e.duration_ms).filter(|ms| *ms > 0),
                explicit: e.explicit,
                source: Some(source.clone()),
            })
        })
        .collect()
}

/// Which song of the plan plays: a library file by its path, a Tidal song by its id in the stream
/// cache's file name. Among repeats, the first at or after `from`.
fn song_index(plan: &RidePlan, path: &Path, stream_cache: &Path, from: usize) -> Option<usize> {
    let key = path.to_string_lossy();
    let cached_id = if path.parent() == Some(stream_cache) { path.file_stem().and_then(|s| s.to_str()) } else { None };
    let matches = |i: &usize| {
        let entry = &plan.songs[*i].entry;
        entry.local_path.as_deref() == Some(&*key) || (cached_id.is_some() && entry.tidal_id.as_deref() == cached_id)
    };
    (from..plan.songs.len()).find(matches).or_else(|| (0..from.min(plan.songs.len())).find(matches))
}

fn dj_on_text() -> String {
    match humane::language() {
        Lang::Es => "Apaga el modo DJ para salir a pedalear.".into(),
        Lang::En => "Turn DJ mode off to start a ride.".into(),
    }
}

fn nothing_to_play_text() -> String {
    match humane::language() {
        Lang::Es => "El recorrido no tiene canciones para reproducir.".into(),
        Lang::En => "The ride has no songs to play.".into(),
    }
}

fn skipped_notice(skipped: usize) -> String {
    match humane::language() {
        Lang::Es => format!("{skipped} canciones del recorrido no están en este equipo y se omitieron."),
        Lang::En => format!("{skipped} songs of this ride are not on this computer and were skipped."),
    }
}

fn sharing_notice() -> String {
    match humane::language() {
        Lang::Es => "Ya compartes tu música: el sonido del recorrido sigue en este equipo y la tele muestra solo las imágenes.".into(),
        Lang::En => "You are already sharing your music: the ride's sound stays on this computer and the TV shows only the pictures.".into(),
    }
}

fn tv_sound_failed_notice(reason: &str) -> String {
    match humane::language() {
        Lang::Es => format!("No pude preparar el sonido de la tele ({reason}): la música sigue en este equipo."),
        Lang::En => format!("Could not set up the TV's sound ({reason}): the music stays on this computer."),
    }
}

/// Saves the plan's playlist, loads it in the normal player (with its source context) and starts
/// the ride. With `tv_enabled` the TV server (and, for `tv_audio = tv`, the ride hub) start too.
#[tauri::command]
pub async fn exercise_start(app: AppHandle, plan: RidePlan) -> CmdResult<ExerciseState> {
    if app.state::<crate::dj::Dj>().is_on() {
        return Err(dj_on_text());
    }
    let exercise = app.state::<Exercise>();
    if matches!(lock(&exercise.ride).stage, Stage::Riding | Stage::Finished) {
        end_ride(&app).await;
    }
    let (playlist, ridden_id) = {
        let ride = lock(&exercise.ride);
        let same_ride = ride.built.as_ref().is_some_and(|b| b.plan.workout.id == plan.workout.id);
        (playlist_for(&plan, ride.built.as_ref()), ride.saved_id.clone().filter(|_| same_ride))
    };
    let items = stream_items(&playlist);
    if items.is_empty() {
        return Err(nothing_to_play_text());
    }
    let skipped = playlist.entries.len().saturating_sub(items.len());
    let store = app.state::<Playlists>().store().clone();
    let to_save = playlist.clone();
    tauri::async_runtime::spawn_blocking(move || store.save(&to_save)).await.map_err(err)?.map_err(err)?;

    // sharing as the host: the ride joins the shared queue like any "play", and the TV's sound stays here
    let sharing = app.state::<crate::share::Share>().in_session();
    let run = lock(&exercise.ride).start(Arc::new(plan));
    let notice = [(skipped > 0).then(|| skipped_notice(skipped)), sharing.then(sharing_notice)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if !notice.is_empty() {
        lock(&exercise.ride).notice = Some(notice);
    }
    play(&app, items);
    emit_state(&app);
    if let Some(id) = ridden_id {
        mark_ridden(&app, id);
    }

    if app.state::<AppState>().settings().exercise.tv_enabled {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(message) = tv_up(&app).await {
                tracing::warn!("ride TV: {message}");
                set_notice(&app, run, tv_sound_failed_notice(&message));
            } else {
                tv_for_ride(&app, run, sharing).await;
            }
            emit_state(&app);
        });
    }
    Ok(state_of(&app))
}

/// Counts the ride in its saved copy (best effort, off the ride's path).
fn mark_ridden(app: &AppHandle, id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(message) = with_store(&app, move |store| store.mark_ridden(&id)).await {
            tracing::warn!("could not count the ride: {message}");
        }
        emit_rides(&app);
    });
}

/// What `play_tidal` does for a list: the shared queue when hosting, otherwise the stream player.
fn play(app: &AppHandle, items: Vec<StreamItem>) {
    if crate::share::host_play_tidal(app, &items, 0) {
        return;
    }
    let state = app.state::<AppState>();
    crate::sound::forget_sources(app);
    state.player.set_shuffle(false);
    state.player.set_repeat(RepeatMode::Off);
    crate::stream::play(app.clone(), items);
}

fn set_notice(app: &AppHandle, run: u64, text: String) {
    let exercise = app.state::<Exercise>();
    let mut ride = lock(&exercise.ride);
    if ride.run == run {
        ride.notice = Some(text);
    }
}

/// Ends the ride: the music stops, the ride's hub goes, the TV page goes back to waiting (and its
/// server stops when the ride started it).
#[tauri::command]
pub async fn exercise_stop(app: AppHandle) -> ExerciseState {
    end_ride(&app).await;
    emit_state(&app);
    state_of(&app)
}

async fn end_ride(app: &AppHandle) {
    let exercise = app.state::<Exercise>();
    let (was_active, had_hub, by_ride) = {
        let mut r = lock(&exercise.ride);
        let active = matches!(r.stage, Stage::Riding | Stage::Finished);
        let facts = (active, r.hub, r.tv_by_ride);
        r.stop();
        facts
    };
    if was_active {
        // a stream still fetching songs must not add them after the stop
        let state = app.state::<AppState>();
        state.stream_generation.fetch_add(1, Ordering::SeqCst);
        crate::stream::clear_upcoming(app);
        state.player.stop();
    }
    if had_hub {
        crate::share::stop_hub(app).await;
    }
    if let Some(tv) = lock(&exercise.tv).as_ref() {
        tv.clear();
    }
    if by_ride {
        tv_down(app).await;
    }
}

// ------------------------------------------------------------------ the player's tick

/// Every player snapshot (from `main.rs`, on the blocking pool): while riding, the ride's view at
/// the player's own song and position goes to the screen and to the TV.
pub fn on_player(app: &AppHandle, snapshot: &Snapshot) {
    let Some(exercise) = app.try_state::<Exercise>() else { return };
    let stream_cache = crate::stream::cache_dir(&app.state::<AppState>());
    let upcoming_empty = app.state::<AppState>().upcoming.lock().map(|u| u.is_empty()).unwrap_or(true);
    let playing = snapshot.status == Status::Playing;
    let current = snapshot.current.and_then(|i| snapshot.queue.get(i));
    let now = now_ms();
    let (want_lyrics, lyrics_offset_ms) = tv_lyrics_wanted(app);

    let (stage, view, skip, lyrics_job) = {
        let mut r = lock(&exercise.ride);
        if r.stage != Stage::Riding {
            return;
        }
        let Some(plan) = r.plan.clone() else { return };
        let index = current.and_then(|p| song_index(&plan, p, &stream_cache, r.last_index));
        match index {
            Some(i) => {
                r.seen_first = true;
                r.seen_last |= i + 1 == plan.songs.len();
                r.last_index = i;
                let view = RideClock::view(&plan, i, snapshot.position_ms, now);
                // a song longer than its part is cut there: the player moves on, once per song
                let skip = view.skip_to_next && r.skip_requested_for != Some(i);
                if skip {
                    r.skip_requested_for = Some(i);
                }
                r.last = Some((view.clone(), playing));
                let key = (i, want_lyrics);
                let lyrics_job = (r.lyrics_for != Some(key)).then(|| {
                    r.lyrics_for = Some(key);
                    (r.run, i, plan.songs[i].entry.clone())
                });
                (Stage::Riding, view, skip, lyrics_job)
            }
            // the queue ran out: the ride is over
            None if r.seen_first
                && (current.is_none() || snapshot.status == Status::Stopped)
                && (r.seen_last || upcoming_empty) =>
            {
                let view = RideClock::view(&plan, plan.songs.len(), 0, now);
                r.stage = Stage::Finished;
                r.last = Some((view.clone(), false));
                (Stage::Finished, view, false, None)
            }
            // something else plays: the rider left the ride
            None if r.seen_first => {
                drop(r);
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    end_ride(&app).await;
                    emit_state(&app);
                });
                return;
            }
            // the first song is still on its way
            None => return,
        }
    };
    if skip {
        // the last song's cut empties the queue; the next tick then finishes the ride
        app.state::<AppState>().player.next();
    }
    if let Some((run, index, entry)) = lyrics_job {
        let duration_ms = snapshot.duration_ms.unwrap_or(entry.duration_ms);
        tv_lyrics_for_song(
            app,
            TvLyricsJob { run, index, entry, wanted: want_lyrics, offset_ms: lyrics_offset_ms, duration_ms },
        );
    }
    if let Some(tv) = lock(&exercise.tv).as_ref() {
        tv.publish(view.clone(), playing && stage == Stage::Riding);
    }
    let _ = app.emit("ride", RideTick { stage, playing: playing && stage == Stage::Riding, view });
    if stage == Stage::Finished {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // the music is over: the TV's sound goes; the final frame stays on the page
            let exercise = app.state::<Exercise>();
            let had_hub = std::mem::take(&mut lock(&exercise.ride).hub);
            if had_hub {
                crate::share::stop_hub(&app).await;
            }
            emit_state(&app);
        });
    }
}

// ------------------------------------------------------------------ TV

/// Longest the TV's lyrics lookup may take (a slow network must not leave a task hanging).
const LYRICS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether the TV should carry lyrics now (it runs and `tv_lyrics` is on) and their shift against the music (the
/// listening room's Bluetooth-delay setting). The settings are only read while the TV runs.
fn tv_lyrics_wanted(app: &AppHandle) -> (bool, i32) {
    if lock(&app.state::<Exercise>().tv).is_none() {
        return (false, 0);
    }
    let settings = app.state::<AppState>().settings();
    (settings.exercise.tv_lyrics, settings.room.lyrics_offset_ms)
}

/// The song of the ride whose lyrics the TV needs.
struct TvLyricsJob {
    run: u64,
    index: usize,
    entry: PlaylistEntry,
    wanted: bool,
    offset_ms: i32,
    duration_ms: u64,
}

/// The ride's song changed (or the TV or the setting did): the old lyrics go at once, and, when wanted, the
/// new ones are looked up off the player tick (a library file's own first, then Tidal's, from the cache when
/// it has them) and handed to the TV if that song is still the one playing. Never fails the ride.
fn tv_lyrics_for_song(app: &AppHandle, job: TvLyricsJob) {
    let exercise = app.state::<Exercise>();
    if let Some(tv) = lock(&exercise.tv).as_ref() {
        tv.set_lyrics(None);
    }
    if !job.wanted {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let TvLyricsJob { run, index, entry, offset_ms, duration_ms, .. } = job;
        let api = app.state::<AppState>().api().ok();
        let track_id = entry.tidal_id.as_deref().and_then(|id| id.parse::<i64>().ok());
        let path = entry.local_path.as_deref().map(Path::new);
        let found =
            tokio::time::timeout(LYRICS_LOOKUP_TIMEOUT, homefi_core::lyrics::for_track(api.as_ref(), track_id, path))
                .await;
        let lyrics = match found {
            Ok(Ok(Some(lyrics))) => lyrics,
            Ok(Ok(None)) => return,
            Ok(Err(e)) => return tracing::debug!("ride TV: no lyrics for `{}`: {e}", entry.title),
            Err(_) => return tracing::debug!("ride TV: lyrics for `{}` took too long", entry.title),
        };
        let key = format!("{index}:{}", entry.tidal_id.as_deref().unwrap_or(&entry.title));
        let Some(tv_lyrics) = TvLyrics::new(&key, &lyrics, offset_ms, duration_ms) else { return };
        let exercise = app.state::<Exercise>();
        // the ride moved on while the lookup ran: these are not the song's lyrics any more
        let current = {
            let r = lock(&exercise.ride);
            r.run == run && r.lyrics_for == Some((index, true))
        };
        if current {
            if let Some(tv) = lock(&exercise.tv).as_ref() {
                tv.set_lyrics(Some(tv_lyrics));
            }
        }
    });
}

/// Starts the TV's LAN page (once) and answers where the TV's browser opens it. With a ride going
/// the page shows it (and the ride hub starts for `tv_audio = tv`).
#[tauri::command]
pub async fn exercise_tv(app: AppHandle, on: bool) -> CmdResult<Option<String>> {
    let exercise = app.state::<Exercise>();
    if !on {
        let had_hub = std::mem::take(&mut lock(&exercise.ride).hub);
        if had_hub {
            crate::share::stop_hub(&app).await;
        }
        tv_down(&app).await;
        emit_state(&app);
        return Ok(None);
    }
    let url = tv_on(&app).await?;
    emit_state(&app);
    Ok(Some(url))
}

/// Starts the server and, with a ride going, gives it the ride (and the ride hub for `tv_audio = tv`).
async fn tv_on(app: &AppHandle) -> CmdResult<String> {
    let exercise = app.state::<Exercise>();
    let url = tv_up(app).await?;
    let riding = {
        let r = lock(&exercise.ride);
        (r.stage == Stage::Riding).then_some(r.run)
    };
    if let Some(run) = riding {
        let sharing = app.state::<crate::share::Share>().in_session() && !lock(&exercise.ride).hub;
        tv_for_ride(app, run, sharing).await;
    }
    Ok(url)
}

/// Starts the server when it is not running; returns its URL.
async fn tv_up(app: &AppHandle) -> CmdResult<String> {
    let exercise = app.state::<Exercise>();
    let running = lock(&exercise.tv).as_ref().and_then(TvServer::url);
    if let Some(url) = running {
        return Ok(url);
    }
    let settings = app.state::<AppState>().settings().exercise.clamped();
    let config = TvConfig { port: settings.tv_port, name: TV_NAME.into(), lang: humane::language(), announce: true };
    let server = TvServer::start(config).await.map_err(err)?;
    let Some(url) = server.url() else {
        server.stop().await;
        return Err(match humane::language() {
            Lang::Es => "No encontré una dirección de red local para la tele.".into(),
            Lang::En => "No local network address found for the TV.".into(),
        });
    };
    let spare = {
        let mut slot = lock(&exercise.tv);
        if slot.is_some() {
            Some(server)
        } else {
            *slot = Some(server);
            None
        }
    };
    if let Some(server) = spare {
        // another call started it meanwhile
        server.stop().await;
        return Ok(url);
    }
    // the ride that asked for it owns it (it stops with the ride); `exercise_tv` without a ride keeps it
    let mut ride = lock(&exercise.ride);
    ride.tv_by_ride = ride.stage == Stage::Riding;
    Ok(url)
}

async fn tv_down(app: &AppHandle) {
    let server = lock(&app.state::<Exercise>().tv).take();
    if let Some(server) = server {
        server.stop().await;
    }
    // its page is gone with the server
    if let Some(window) = app.get_webview_window(TV_WINDOW) {
        let _ = window.close();
    }
    lock(&app.state::<Exercise>().ride).tv_by_ride = false;
}

/// Gives the running TV server the ride's session. For `tv_audio = tv` the ride's hub is started
/// (unless the computer already shares or is in a session: then the sound stays here, with a notice)
/// and the TV is told how to join it once it is up.
async fn tv_for_ride(app: &AppHandle, run: u64, sharing: bool) {
    let exercise = app.state::<Exercise>();
    let (plan, wants_audio) = {
        let r = lock(&exercise.ride);
        let Some(plan) = r.plan.clone().filter(|_| r.run == run) else { return };
        (plan, app.state::<AppState>().settings().exercise.tv_audio == TvAudio::Tv)
    };
    let (pulse_style, course_look, course_summits) = {
        let s = app.state::<AppState>().settings().exercise;
        (s.pulse_style, s.course_look, s.course_summits)
    };
    let session = |audio: TvAudio, listen: Option<RideListen>| TvSession {
        title: plan.workout.name.clone(),
        profile: plan.profile.clone(),
        parts: TvPart::of(&plan.workout),
        audio,
        listen,
        pulse_style,
        course_look,
        course_summits,
    };
    let begin = |audio: TvAudio, listen: Option<RideListen>| {
        if let Some(tv) = lock(&exercise.tv).as_ref() {
            tv.begin(session(audio, listen));
        }
        let mut r = lock(&exercise.ride);
        if r.run == run {
            r.tv_audio = audio;
        }
    };
    if !wants_audio || sharing {
        if wants_audio && sharing {
            set_notice(app, run, sharing_notice());
        }
        begin(TvAudio::Phone, None);
        return;
    }
    begin(TvAudio::Tv, None);
    // the hub attaches to what plays: wait for the first song to be in the player
    if !wait_first_song(app, run).await {
        return;
    }
    let settings = app.state::<AppState>().settings();
    let config = ride_hub_config(&plan.workout.name, settings.share.port, &ride_token());
    match crate::share::start_ride_hub(app, config).await {
        Ok(()) => {
            let listen = app.state::<crate::share::Share>().hub().map(|h| RideListen {
                port: h.info.port,
                code: h.info.code.clone(),
                name: h.info.session_name.clone(),
            });
            let current = lock(&exercise.ride).run == run;
            if current {
                lock(&exercise.ride).hub = true;
                begin(TvAudio::Tv, listen);
            } else {
                // the ride ended while the hub was starting
                crate::share::stop_hub(app).await;
            }
        }
        Err(None) => {
            set_notice(app, run, sharing_notice());
            begin(TvAudio::Phone, None);
        }
        Err(Some(reason)) => {
            set_notice(app, run, tv_sound_failed_notice(&reason));
            begin(TvAudio::Phone, None);
        }
    }
}

/// Waits (bounded) until the player holds a song of this ride; `false` when the ride ended or it took too long.
async fn wait_first_song(app: &AppHandle, run: u64) -> bool {
    let deadline = tokio::time::Instant::now() + FIRST_SONG_WAIT;
    loop {
        if lock(&app.state::<Exercise>().ride).run != run {
            return false;
        }
        if app.state::<AppState>().player.snapshot().current.is_some() && lock(&app.state::<Exercise>().ride).seen_first
        {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(FIRST_SONG_POLL).await;
    }
}

/// The screens the TV window can go to.
#[tauri::command]
pub fn exercise_monitors(app: AppHandle) -> Vec<MonitorView> {
    let primary = app.primary_monitor().ok().flatten();
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| MonitorView {
            name: m.name().cloned(),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            primary: primary.as_ref().is_some_and(|p| p.name() == m.name() && p.position() == m.position()),
        })
        .collect()
}

/// Opens (or closes) the second window of the ride. It loads the TV server's own page
/// (`http://127.0.0.1:<tv_port>/`, started if needed): the one renderer every TV shows. `monitor` is a
/// name from `exercise_monitors`; without one the `tv_window_display` setting decides (`auto`: a screen
/// other than the main window's when there is one). Fullscreen follows `tv_window_fullscreen`. The page
/// has no app access, so only `exercise_tv_window(false)` closes it.
#[tauri::command]
pub async fn exercise_tv_window(app: AppHandle, on: bool, monitor: Option<String>) -> CmdResult<()> {
    if !on {
        if let Some(window) = app.get_webview_window(TV_WINDOW) {
            window.close().map_err(err)?;
        }
        emit_state(&app);
        return Ok(());
    }
    if let Some(window) = app.get_webview_window(TV_WINDOW) {
        return window.set_focus().map_err(err);
    }
    let settings = app.state::<AppState>().settings().exercise.clamped();
    tv_on(&app).await?;
    let port = lock(&app.state::<Exercise>().tv)
        .as_ref()
        .map(TvServer::port)
        .ok_or_else(|| "The TV server stopped.".to_owned())?;
    let page = tauri::Url::parse(&format!("http://127.0.0.1:{port}/")).map_err(err)?;
    let fullscreen = settings.tv_window_fullscreen;
    let chosen = monitor.or_else(|| Some(settings.tv_window_display).filter(|d| d != ex::TV_DISPLAY_AUTO));
    let (width, height) = TV_WINDOW_SIZE;
    let window = tauri::WebviewWindowBuilder::new(&app, TV_WINDOW, tauri::WebviewUrl::External(page))
        .title("HomeFi")
        .inner_size(width, height)
        .background_color(TV_BACKGROUND)
        .visible(false)
        .build()
        .map_err(err)?;
    if let Some(target) = pick_monitor(&app, chosen.as_deref()) {
        // the window lands on that screen first, then goes fullscreen there
        let _ = window.set_position(*target.position());
    }
    let _ = window.set_fullscreen(fullscreen);
    window.show().map_err(err)?;
    emit_state(&app);
    Ok(())
}

/// The named screen, else one other than the main window's, else the primary one.
fn pick_monitor(app: &AppHandle, name: Option<&str>) -> Option<tauri::Monitor> {
    let monitors = app.available_monitors().ok()?;
    if let Some(name) = name {
        if let Some(found) = monitors.iter().find(|m| m.name().is_some_and(|n| n == name)) {
            return Some(found.clone());
        }
    }
    let main = app.get_webview_window("main").and_then(|w| w.current_monitor().ok().flatten());
    monitors
        .iter()
        .find(|m| main.as_ref().is_none_or(|c| c.position() != m.position()))
        .cloned()
        .or_else(|| app.primary_monitor().ok().flatten())
}

// ------------------------------------------------------------------ cues

/// A cue as a mono 16-bit WAV file (an ArrayBuffer on the screen's side): `new Audio(blobUrl)` or
/// WebAudio plays it. Synthesized, nothing to ship.
#[tauri::command]
pub fn exercise_cue_wav(kind: CueKind, sample_rate: u32) -> Response {
    Response::new(ex::cues::wav(kind, sample_rate))
}

/// Closing the app: the TV page stops (the hub is stopped by `share::shutdown`).
pub fn shutdown(app: &AppHandle) {
    let server = lock(&app.state::<Exercise>().tv).take();
    if let Some(server) = server {
        tauri::async_runtime::block_on(server.stop());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ex::{PlannedSong, Segment, Workout};

    fn entry(title: &str, local: Option<&str>, tidal: Option<&str>) -> PlaylistEntry {
        PlaylistEntry {
            title: title.into(),
            local_path: local.map(Into::into),
            tidal_id: tidal.map(Into::into),
            duration_ms: 200_000,
            ..PlaylistEntry::default()
        }
    }

    fn plan(entries: Vec<PlaylistEntry>) -> RidePlan {
        let songs: Vec<PlannedSong> = entries
            .into_iter()
            .enumerate()
            .map(|(i, entry)| PlannedSong { segment_index: i, entry, ..Default::default() })
            .collect();
        let segments = songs.iter().map(|_| Segment { seconds: 200, ..Segment::default() }).collect();
        RidePlan {
            workout: Workout { id: "w1".into(), name: "Easy".into(), segments, ..Workout::default() },
            songs,
            ..RidePlan::default()
        }
    }

    #[test]
    fn the_state_machine_moves_idle_building_ready_riding_and_back() {
        let mut r = RideState::default();
        assert_eq!(r.stage, Stage::Idle);
        assert!(r.begin_build());
        assert_eq!(r.stage, Stage::Building);
        assert!(!r.begin_build(), "one build at a time");
        r.build_failed();
        assert_eq!(r.stage, Stage::Idle, "nothing built before");
        assert!(r.begin_build());
        r.build_done(RideBuilt { plan: plan(vec![]), playlist: HomePlaylist::default() }, None);
        assert_eq!(r.stage, Stage::Ready);
        let run = r.start(Arc::new(plan(vec![])));
        assert_eq!((r.stage, run), (Stage::Riding, 1));
        assert!(!r.begin_build(), "no build under a ride");
        r.stop();
        assert_eq!((r.stage, r.run), (Stage::Ready, 2), "the build stays to ride again; the run changed");
        assert!(r.begin_build());
        r.build_failed();
        assert_eq!(r.stage, Stage::Ready, "a cancelled rebuild keeps the previous plan");
    }

    #[test]
    fn a_stop_clears_what_the_ride_owned() {
        let mut r = RideState::default();
        r.start(Arc::new(plan(vec![])));
        r.hub = true;
        r.tv_by_ride = true;
        r.notice = Some("x".into());
        r.stop();
        assert!(!r.hub && !r.tv_by_ride && r.notice.is_none() && r.plan.is_none());
    }

    #[test]
    fn songs_are_found_by_library_path_or_by_the_cached_tidal_id() {
        let p = plan(vec![
            entry("A", Some("/m/a.flac"), None),
            entry("B", None, Some("123")),
            entry("A again", Some("/m/a.flac"), None),
        ]);
        let cache = Path::new("/app/stream-cache");
        assert_eq!(song_index(&p, Path::new("/m/a.flac"), cache, 0), Some(0));
        assert_eq!(song_index(&p, Path::new("/m/a.flac"), cache, 1), Some(2), "a repeat: the next one");
        assert_eq!(song_index(&p, Path::new("/app/stream-cache/123.flac"), cache, 0), Some(1));
        assert_eq!(song_index(&p, Path::new("/elsewhere/123.flac"), cache, 0), None, "only the cache's files count");
        assert_eq!(song_index(&p, Path::new("/m/other.flac"), cache, 0), None);
    }

    #[test]
    fn stream_items_carry_the_playlist_as_source_and_skip_unplayable_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.flac");
        std::fs::write(&file, b"x").unwrap();
        let file = file.to_string_lossy().into_owned();
        let mut playlist = playlist_for(&plan(vec![entry("A", Some(&file), None), entry("B", None, Some("9"))]), None);
        playlist.entries.push(entry("Ghost", None, None));
        let items = stream_items(&playlist);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].local.as_deref(), Some(file.as_str()));
        assert_eq!(items[1].id, "9");
        assert!(items.iter().all(|i| i.source.is_some() && i.duration_ms == Some(200_000)));
    }

    #[test]
    fn a_missing_local_file_falls_back_to_tidal_or_is_skipped() {
        let mut with_id = entry("Gone but on TIDAL", Some("/nowhere/a.flac"), Some("7"));
        with_id.tidal_id = Some("7".into());
        let playlist = playlist_for(&plan(vec![with_id, entry("Gone", Some("/nowhere/b.flac"), None)]), None);
        let items = stream_items(&playlist);
        assert_eq!(items.len(), 1, "the song with no other source is skipped");
        assert_eq!((items[0].local.as_deref(), items[0].id.as_str()), (None, "7"));
        assert!(skipped_notice(1).contains('1'));
    }

    #[test]
    fn a_saved_ride_keeps_its_playlist_id() {
        let p = plan(vec![entry("A", None, Some("1"))]);
        assert_eq!(playlist_of(&p, Some("p-saved".into())).id, "p-saved");
        assert_ne!(playlist_of(&p, Some(String::new())).id, "");
    }

    #[test]
    fn the_builders_playlist_is_kept_only_for_the_same_ride() {
        let p = plan(vec![entry("A", Some("/m/a.flac"), None)]);
        let mut named = playlist_for(&p, None);
        named.id = "saved-by-builder".into();
        let built = RideBuilt { plan: p.clone(), playlist: named };
        assert_eq!(playlist_for(&p, Some(&built)).id, "saved-by-builder");
        let mut other = p.clone();
        other.songs[0].entry.title = "Changed".into();
        assert_ne!(playlist_for(&other, Some(&built)).id, "saved-by-builder", "an edited plan gets a new playlist");
        let made = playlist_for(&p, None);
        assert!(matches!(made.origin, PlaylistOrigin::Ride(ref o) if o.workout_id == "w1"));
        assert_eq!(made.name, "Easy");
    }

    #[test]
    fn the_analyzer_reads_library_files_and_only_cached_tidal_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("77.flac"), b"x").unwrap();
        std::fs::write(dir.path().join("778.flac"), b"x").unwrap();
        let analyzer = DesktopAnalyzer { stream_cache: dir.path().to_path_buf(), memory_mb: 64 };
        assert_eq!(analyzer.file_of(&entry("T", None, Some("77"))), Some(dir.path().join("77.flac")));
        assert_eq!(analyzer.file_of(&entry("T", None, Some("5"))), None, "not downloaded for analysis");
        assert_eq!(analyzer.file_of(&entry("L", Some("/nonexistent.flac"), None)), None);
        let lib = dir.path().join("lib.flac");
        std::fs::write(&lib, b"x").unwrap();
        assert_eq!(analyzer.file_of(&entry("L", lib.to_str(), None)), Some(lib));
    }

    #[test]
    fn json_shapes_of_the_contract() {
        let tick = RideTick { stage: Stage::Riding, playing: true, view: RideView::default() };
        let json = serde_json::to_value(&tick).unwrap();
        assert_eq!(json["stage"], "riding");
        assert_eq!(json["playing"], true);
        assert!(json["view"]["segment_index"].is_number());
        assert_eq!(serde_json::to_value(Stage::Finished).unwrap(), "finished");
        let presets = exercise_presets("es".into());
        assert_eq!(presets.len(), PresetId::ALL.len());
        assert_eq!(serde_json::to_value(&presets[0]).unwrap()["id"], "first_ride");
        let minutes = exercise_ride_minutes();
        assert_eq!((minutes.first(), minutes.last(), minutes.len()), (Some(&10), Some(&90), 17));
    }
}
