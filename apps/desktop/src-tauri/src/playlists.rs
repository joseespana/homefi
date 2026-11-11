//! HomeFi playlists ("Get ready for a concert", `src/lib/api.ts`): thin commands over
//! `homefi_core::concert` and `PlaylistStore`. Every rule lives in the core. The TIDAL
//! account is written only to save a HomeFi playlist there (the button, or the
//! "save to TIDAL automatically" setting) and to delete that copy (its own setting).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use homefi_core::ai::AiProvider;
use homefi_core::concert::{self, BuildContext, ConcertBuilt, ConcertProgress, ConcertRequest};
use homefi_core::humane::{self, Lang};
use homefi_core::party::{
    self, ChatBrief, PartyBrief, PartyBuilt, PartyContext, PartyProgress, PartySession, PartyTurn, SongSeed,
};
use homefi_core::playlist::{
    delete_from_tidal, save_stored_to_tidal, HomePlaylist, LibraryIndex, LibraryRef, PlaylistStore, TidalSaveReport,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{watch, Mutex as AsyncMutex};

use crate::commands::{err, lock, AppState, TrackView};

type CmdResult<T> = Result<T, String>;

/// Playlist state of the app: the store, the cancel switch of the running concert or exercise
/// build, and the open AI chats.
pub struct Playlists {
    store: PlaylistStore,
    /// Concert and exercise builds (one at a time, a new one cancels the last).
    cancel: Mutex<Option<watch::Sender<bool>>>,
    /// Open AI chats by the id the UI gave them; each has its own cancel switch.
    conversations: Mutex<HashMap<String, Conversation>>,
}

/// One open AI chat. The session is held across awaits, hence the async mutex.
struct Conversation {
    session: Arc<AsyncMutex<PartySession>>,
    cancel: watch::Sender<bool>,
}

impl Playlists {
    pub fn new(app_dir: &Path) -> Self {
        Self {
            store: PlaylistStore::in_app_dir(app_dir),
            cancel: Mutex::new(None),
            conversations: Mutex::new(HashMap::new()),
        }
    }

    /// The playlist store (the Exercise ride saves its playlist here).
    pub(crate) fn store(&self) -> &PlaylistStore {
        &self.store
    }

    /// Registers a new running job, cancelling the one before it.
    pub(crate) fn begin(&self) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        if let Some(previous) = lock(&self.cancel).replace(tx) {
            let _ = previous.send(true);
        }
        rx
    }

    /// Clears the switch, unless a newer job already took its place.
    pub(crate) fn finish(&self, rx: &watch::Receiver<bool>) {
        let mut slot = lock(&self.cancel);
        if slot.as_ref().is_some_and(|tx| tx.subscribe().same_channel(rx)) {
            *slot = None;
        }
    }

    pub(crate) fn cancel(&self) {
        if let Some(tx) = lock(&self.cancel).as_ref() {
            let _ = tx.send(true);
        }
    }

    /// Registers (or replaces) a chat. `Err` with the humane text when `max` other chats are
    /// already open (the UI enforces the cap first; this is the safety net).
    fn insert_conversation(&self, id: String, session: PartySession, max: u8, lang: Lang) -> CmdResult<()> {
        let mut open = lock(&self.conversations);
        if !open.contains_key(&id) && open.len() >= usize::from(max) {
            return Err(party::too_many_chats_message(lang, max));
        }
        let (cancel, _) = watch::channel(false);
        let conversation = Conversation { session: Arc::new(AsyncMutex::new(session)), cancel };
        if let Some(previous) = open.insert(id, conversation) {
            let _ = previous.cancel.send(true);
        }
        Ok(())
    }

    /// The chat's session; the std guard is dropped before the caller awaits anything.
    fn session(&self, id: &str) -> CmdResult<Arc<AsyncMutex<PartySession>>> {
        lock(&self.conversations)
            .get(id)
            .map(|c| Arc::clone(&c.session))
            .ok_or_else(|| party::no_session_message(humane::language()))
    }

    /// Starts a build: the session and a fresh cancel receiver. A build still running for this
    /// chat is cancelled (the old switch may already be `true` from an earlier cancel).
    fn begin_build(&self, id: &str) -> CmdResult<(Arc<AsyncMutex<PartySession>>, watch::Receiver<bool>)> {
        let mut open = lock(&self.conversations);
        let conversation = open.get_mut(id).ok_or_else(|| party::no_session_message(humane::language()))?;
        let (tx, rx) = watch::channel(false);
        let previous = std::mem::replace(&mut conversation.cancel, tx);
        let _ = previous.send(true);
        Ok((Arc::clone(&conversation.session), rx))
    }

    /// Stops the chat's build, if any; the chat stays open.
    fn cancel_conversation(&self, id: &str) {
        if let Some(conversation) = lock(&self.conversations).get(id) {
            let _ = conversation.cancel.send(true);
        }
    }

    /// Cancels the chat's build and forgets it. `true` is sent before the sender is dropped:
    /// [`cancelled`] pends forever on a dropped switch.
    fn close_conversation(&self, id: &str) {
        if let Some(conversation) = lock(&self.conversations).remove(id) {
            let _ = conversation.cancel.send(true);
        }
    }
}

/// The AI the screen will use.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConcertAi {
    pub provider_id: String,
    pub provider_name: String,
    /// As set; empty means automatic.
    pub model: String,
    pub web_search: bool,
}

/// What the screen says before building.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConcertSources {
    /// `None`: no AI set up (or its key is missing).
    pub ai: Option<ConcertAi>,
    /// Signed in to Tidal.
    pub tidal: bool,
}

/// The library as the core's resolver wants it.
pub(crate) fn library_refs(tracks: &[TrackView]) -> Vec<LibraryRef> {
    tracks
        .iter()
        .map(|t| LibraryRef {
            path: t.path.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            isrc: t.isrc.clone(),
            duration_ms: t.duration_ms,
            bpm: t.bpm,
            genre: t.genre.clone(),
        })
        .collect()
}

pub(crate) fn cancelled_text() -> String {
    match humane::language() {
        Lang::Es => "cancelado".into(),
        Lang::En => "cancelled".into(),
    }
}

/// Resolves when `true` is sent; pends forever if the switch is dropped unused.
pub(crate) async fn cancelled(mut rx: watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// Everything a job needs from the app, taken without holding a lock across an await.
/// What every HomeFi playlist build reads: TIDAL, the playlist settings and the library.
struct Sources {
    api: Option<homefi_core::api::TidalApi>,
    settings: homefi_core::playlist::PlaylistSettings,
    library: LibraryIndex,
}

async fn sources(app: &AppHandle) -> CmdResult<Sources> {
    let state = app.state::<AppState>();
    let api = state.api().ok();
    let settings = state.settings().playlists;
    let refs = library_refs(&state.library.lock().unwrap());
    let library = tauri::async_runtime::spawn_blocking(move || LibraryIndex::new(refs)).await.map_err(err)?;
    Ok(Sources { api, settings, library })
}

/// Concert builds also need the AI config and the answer language.
struct Inputs {
    ai: Option<homefi_core::ai::AiConfig>,
    sources: Sources,
    lang: Lang,
}

async fn inputs(app: &AppHandle, language: &str) -> CmdResult<Inputs> {
    let sources = sources(app).await?;
    let ai = crate::ai::settings_config(app).await;
    Ok(Inputs { ai, sources, lang: Lang::from_code(language) })
}

impl Inputs {
    fn context(&self) -> BuildContext<'_> {
        BuildContext {
            settings: &self.sources.settings,
            ai: self.ai.as_ref(),
            api: self.sources.api.as_ref(),
            library: &self.sources.library,
            lang: self.lang,
        }
    }
}

#[tauri::command]
pub async fn concert_sources(app: AppHandle) -> ConcertSources {
    let state = app.state::<AppState>();
    let settings = state.settings().ai;
    let ai = if settings.provider.is_empty() || crate::ai::settings_config(&app).await.is_none() {
        None
    } else {
        AiProvider::from_id(&settings.provider, Some(settings.base_url.as_str())).map(|p| ConcertAi {
            provider_id: settings.provider.clone(),
            provider_name: p.short_name().to_owned(),
            model: settings.model.clone(),
            web_search: p.web_search(),
        })
    };
    ConcertSources { ai, tidal: state.api().is_ok() }
}

#[tauri::command]
pub async fn concert_build(
    app: AppHandle,
    request: ConcertRequest,
    replace_id: Option<String>,
    language: String,
    on_progress: Channel<ConcertProgress>,
) -> CmdResult<ConcertBuilt> {
    let playlists = app.state::<Playlists>();
    let rx = playlists.begin();
    let result = async {
        let inputs = inputs(&app, &language).await?;
        let ctx = inputs.context();
        let progress = move |p: ConcertProgress| {
            let _ = on_progress.send(p);
        };
        tokio::select! {
            r = concert::build(&request, &ctx, &progress) => r.map_err(err),
            () = cancelled(rx.clone()) => Err(cancelled_text()),
        }
    }
    .await;
    playlists.finish(&rx);
    let mut built = result?;
    if let Some(id) = replace_id {
        if let Ok(old) = playlists.store.load(&id) {
            built.playlist.keep_identity_of(&old);
        }
    }
    Ok(built)
}

#[tauri::command]
pub async fn concert_rearrange(
    app: AppHandle,
    playlist: HomePlaylist,
    request: ConcertRequest,
    language: String,
    on_progress: Channel<ConcertProgress>,
) -> CmdResult<HomePlaylist> {
    let playlists = app.state::<Playlists>();
    let rx = playlists.begin();
    let result = async {
        let inputs = inputs(&app, &language).await?;
        let ctx = inputs.context();
        let progress = move |p: ConcertProgress| {
            let _ = on_progress.send(p);
        };
        tokio::select! {
            r = concert::rearrange(&playlist, &request, &ctx, &progress) => r.map_err(err),
            () = cancelled(rx.clone()) => Err(cancelled_text()),
        }
    }
    .await;
    playlists.finish(&rx);
    result
}

#[tauri::command]
pub fn concert_cancel(playlists: State<'_, Playlists>) {
    playlists.cancel();
}

#[tauri::command]
pub fn party_default_brief(app: AppHandle) -> PartyBrief {
    PartyBrief::from_settings(&app.state::<AppState>().settings().playlists)
}

/// The chat's defaults (length, energy, explicit lyrics) from the settings; nothing asked yet.
#[tauri::command]
pub fn chat_default_brief(app: AppHandle) -> ChatBrief {
    ChatBrief::from_settings(&app.state::<AppState>().settings().playlists)
}

/// The cap from the settings, read when a chat opens.
fn max_open_chats(app: &AppHandle) -> u8 {
    app.state::<AppState>().settings().playlists.chat_max_open
}

/// Registers a chat under `id`; its title is what the list of open chats shows. The error is the
/// "needs an AI" text when none is set up, or the humane "too many chats" text at the cap.
async fn open_conversation(
    app: &AppHandle,
    id: String,
    brief: impl Into<party::Brief>,
    language: &str,
) -> CmdResult<String> {
    let lang = Lang::from_code(language);
    let Some(config) = crate::ai::settings_config(app).await else {
        return Err(party::needs_ai_message(lang));
    };
    let session = PartySession::new(config, brief, lang);
    let title = session.title();
    app.state::<Playlists>().insert_conversation(id, session, max_open_chats(app), lang)?;
    Ok(title)
}

/// Opens (replacing the same id) a party conversation; answers its title.
#[tauri::command]
pub async fn party_open(
    app: AppHandle,
    conversation: String,
    brief: PartyBrief,
    language: String,
) -> CmdResult<String> {
    open_conversation(&app, conversation, brief, &language).await
}

/// Opens (replacing the same id) a "Playlist with AI" conversation; `party_start` gives the opener.
#[tauri::command]
pub async fn chat_open(app: AppHandle, conversation: String, brief: ChatBrief, language: String) -> CmdResult<String> {
    open_conversation(&app, conversation, brief, &language).await
}

/// The first chat message for "Playlist like this song" (`language`: "en" or "es").
#[tauri::command]
pub fn chat_song_wish(seed: SongSeed, language: String) -> String {
    party::song_wish(&seed, Lang::from_code(&language))
}

/// New answers for the chat; answers its title.
#[tauri::command]
pub async fn party_set_brief(app: AppHandle, conversation: String, brief: PartyBrief) -> CmdResult<String> {
    let session = app.state::<Playlists>().session(&conversation)?;
    let mut session = session.lock().await;
    session.set_brief(brief);
    Ok(session.title())
}

#[tauri::command]
pub async fn party_start(app: AppHandle, conversation: String) -> CmdResult<PartyTurn> {
    let session = app.state::<Playlists>().session(&conversation)?;
    let mut session = session.lock().await;
    session.start().await.map_err(err)
}

#[tauri::command]
pub async fn party_reply(app: AppHandle, conversation: String, text: String) -> CmdResult<PartyTurn> {
    let session = app.state::<Playlists>().session(&conversation)?;
    let mut session = session.lock().await;
    session.reply(&text).await.map_err(err)
}

/// Builds from the chat. Only this chat's own switch cancels it (`party_cancel`/`party_close`);
/// concert and exercise builds don't touch it.
#[tauri::command]
pub async fn party_build(
    app: AppHandle,
    conversation: String,
    replace_id: Option<String>,
    on_progress: Channel<PartyProgress>,
) -> CmdResult<PartyBuilt> {
    let playlists = app.state::<Playlists>();
    let (session, rx) = playlists.begin_build(&conversation)?;
    let result = async {
        let mut session = session.lock().await;
        let sources = sources(&app).await?;
        let ctx = PartyContext { settings: &sources.settings, api: sources.api.as_ref(), library: &sources.library };
        let progress = move |p: PartyProgress| {
            let _ = on_progress.send(p);
        };
        tokio::select! {
            r = session.build(&ctx, &progress) => r.map_err(err),
            () = cancelled(rx.clone()) => Err(cancelled_text()),
        }
    }
    .await;
    let mut built = result?;
    if let Some(id) = replace_id {
        if let Ok(old) = playlists.store.load(&id) {
            built.playlist.keep_identity_of(&old);
        }
    }
    Ok(built)
}

#[tauri::command]
pub fn party_cancel(playlists: State<'_, Playlists>, conversation: String) {
    playlists.cancel_conversation(&conversation);
}

#[tauri::command]
pub fn party_close(playlists: State<'_, Playlists>, conversation: String) {
    playlists.close_conversation(&conversation);
}

#[tauri::command]
pub async fn home_playlists(app: AppHandle) -> CmdResult<Vec<HomePlaylist>> {
    let dir = app.state::<Playlists>().store.clone();
    tauri::async_runtime::spawn_blocking(move || dir.list()).await.map_err(err)
}

/// Event with the outcome of an automatic save to TIDAL (the screen updates the playlist).
pub const TIDAL_SYNC_EVENT: &str = "home-playlist-tidal";

/// What a save to TIDAL did, with its summary in words.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TidalSaveView {
    #[serde(flatten)]
    pub report: TidalSaveReport,
    /// "Saved 24 songs to TIDAL · 2 aren't on TIDAL".
    pub summary: String,
}

/// The stored playlist after a save to TIDAL (it now has its `tidal_uuid`) and the report.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TidalSaved {
    pub playlist: HomePlaylist,
    pub report: TidalSaveView,
}

/// [`TIDAL_SYNC_EVENT`]'s payload.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TidalSyncView {
    Saved(Box<TidalSaved>),
    Failed { id: String, message: String },
}

async fn save_to_tidal(app: &AppHandle, id: &str, lang: Lang) -> CmdResult<TidalSaved> {
    let api = app.state::<AppState>().api().map_err(err)?;
    let store = app.state::<Playlists>().store.clone();
    let (playlist, report) = save_stored_to_tidal(&api, &store, id).await.map_err(err)?;
    let summary = report.summary(lang);
    Ok(TidalSaved { playlist, report: TidalSaveView { report, summary } })
}

/// Saves HomeFi's copy and answers it as stored (a copy that didn't know its TIDAL twin gets
/// it back). With "save to TIDAL automatically" on (and TIDAL signed in) it is then saved to
/// TIDAL in the background; [`TIDAL_SYNC_EVENT`] tells how that went.
#[tauri::command]
pub async fn home_playlist_save(app: AppHandle, playlist: HomePlaylist) -> CmdResult<HomePlaylist> {
    let store = app.state::<Playlists>().store.clone();
    let id = playlist.id.clone();
    let stored = tauri::async_runtime::spawn_blocking(move || -> homefi_core::Result<HomePlaylist> {
        store.save(&playlist)?;
        Ok(store.load(&playlist.id).unwrap_or(playlist))
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let state = app.state::<AppState>();
    if state.settings().playlists.tidal_auto_save && state.api().is_ok() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let view = match save_to_tidal(&app, &id, humane::language()).await {
                Ok(saved) => TidalSyncView::Saved(Box::new(saved)),
                Err(message) => TidalSyncView::Failed { id, message },
            };
            let _ = app.emit(TIDAL_SYNC_EVENT, view);
        });
    }
    Ok(stored)
}

/// "Save to TIDAL" / "Update on TIDAL" on a stored playlist (`language`: "en" or "es").
#[tauri::command]
pub async fn home_playlist_save_to_tidal(app: AppHandle, id: String, language: String) -> CmdResult<TidalSaved> {
    save_to_tidal(&app, &id, Lang::from_code(&language)).await
}

/// Moves the playlist's JSON to the system Trash (never a hard delete); `false` when unknown.
/// With "delete the TIDAL copy too" on, its copy in the TIDAL account is deleted as well
/// (best effort: HomeFi's copy goes even when TIDAL can't be reached).
#[tauri::command]
pub async fn home_playlist_delete(app: AppHandle, id: String) -> CmdResult<bool> {
    let store = app.state::<Playlists>().store.clone();
    let delete_on_tidal = app.state::<AppState>().settings().playlists.tidal_delete_with;
    let (removed, stored) = tauri::async_runtime::spawn_blocking(move || -> CmdResult<(bool, Option<HomePlaylist>)> {
        let Some(file) = store.file_path(&id) else {
            return Ok((false, None));
        };
        let stored = if delete_on_tidal { store.load(&id).ok() } else { None };
        match trash::delete(&file) {
            Ok(()) => Ok((true, stored)),
            Err(_) if !file.exists() => Ok((false, None)),
            Err(e) => {
                tracing::warn!("could not move {} to the trash: {e}", file.display());
                Err(err(e))
            }
        }
    })
    .await
    .map_err(err)??;
    if let Some(playlist) = stored.filter(|p| p.tidal_uuid.is_some()) {
        let deleted = match app.state::<AppState>().api() {
            Ok(api) => delete_from_tidal(&api, &playlist).await.map(drop),
            Err(e) => Err(e),
        };
        if let Err(e) = deleted {
            tracing::warn!("could not delete the TIDAL copy of {}: {e}", playlist.id);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn library_refs_map_every_field() {
        let track = TrackView {
            path: "/m/a.flac".into(),
            title: "Blind".into(),
            artist: "Korn".into(),
            album: "Korn".into(),
            album_artist: String::new(),
            track_number: 1,
            disc_number: 1,
            year: None,
            genre: String::new(),
            duration_ms: 259_000,
            format: "FLAC".into(),
            sample_rate: None,
            bit_depth: None,
            lossless: true,
            quality: "FLAC".into(),
            isrc: Some("USSM19400123".into()),
            bpm: Some(96),
        };
        let refs = library_refs(&[track]);
        assert_eq!(
            refs,
            vec![LibraryRef {
                path: "/m/a.flac".into(),
                title: "Blind".into(),
                artist: "Korn".into(),
                album: "Korn".into(),
                isrc: Some("USSM19400123".into()),
                duration_ms: 259_000,
                bpm: Some(96),
                genre: String::new(),
            }]
        );
    }

    #[test]
    fn sources_json_shape() {
        let none = ConcertSources { ai: None, tidal: false };
        assert_eq!(serde_json::to_string(&none).unwrap(), r#"{"ai":null,"tidal":false}"#);
        let full = ConcertSources {
            ai: Some(ConcertAi {
                provider_id: "anthropic".into(),
                provider_name: "Claude".into(),
                model: String::new(),
                web_search: true,
            }),
            tidal: true,
        };
        assert_eq!(
            serde_json::to_string(&full).unwrap(),
            r#"{"ai":{"provider_id":"anthropic","provider_name":"Claude","model":"","web_search":true},"tidal":true}"#
        );
    }

    #[test]
    fn default_brief_json_shape() {
        let brief = PartyBrief::from_settings(&homefi_core::playlist::PlaylistSettings::default());
        assert_eq!(
            serde_json::to_string(&brief).unwrap(),
            r#"{"guests":"friends","ages":[],"vibe":"dance","minutes":120,"curve":"arc","allow_explicit":true,"notes":""}"#
        );
    }

    #[test]
    fn default_chat_brief_json_shape() {
        let brief = ChatBrief::from_settings(&homefi_core::playlist::PlaylistSettings::default());
        assert_eq!(
            serde_json::to_string(&brief).unwrap(),
            r#"{"wish":"","title":"","minutes":60,"curve":"steady","allow_explicit":true}"#
        );
    }

    #[test]
    fn song_wish_reads_the_json_seed() {
        let seed: SongSeed =
            serde_json::from_str(r#"{"title":"Hotel California","artist":"Eagles","year":1976}"#).unwrap();
        assert_eq!(
            chat_song_wish(seed, "en".into()),
            "A playlist like “Hotel California” by Eagles (1976): same style, era and mood. Start with it."
        );
    }

    #[test]
    fn tidal_sync_json_shape() {
        let report = TidalSaveReport { uuid: "u".into(), created: true, saved: 2, skipped: vec!["X · Y".into()] };
        let view = TidalSaveView { summary: report.summary(Lang::En), report };
        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            serde_json::json!({"uuid":"u","created":true,"saved":2,"skipped":["X · Y"],"summary":"Saved 2 songs to TIDAL · 1 isn't on TIDAL"})
        );
        let failed = TidalSyncView::Failed { id: "p1".into(), message: "m".into() };
        assert_eq!(serde_json::to_string(&failed).unwrap(), r#"{"kind":"failed","id":"p1","message":"m"}"#);
    }

    fn chat() -> PartySession {
        let config = homefi_core::ai::AiConfig::new(AiProvider::Anthropic, "m", Some("k".into()));
        PartySession::new(config, ChatBrief::default(), Lang::En)
    }

    #[test]
    fn conversations_start_empty() {
        let p = Playlists::new(Path::new("/nonexistent"));
        assert!(lock(&p.conversations).is_empty());
        assert!(p.session("x").is_err());
    }

    #[tokio::test]
    async fn closing_a_conversation_cancels_only_its_build() {
        let p = Playlists::new(Path::new("/nonexistent"));
        p.insert_conversation("a".into(), chat(), 3, Lang::En).unwrap();
        p.insert_conversation("b".into(), chat(), 3, Lang::En).unwrap();
        let (_, rx_a) = p.begin_build("a").unwrap();
        let (_, rx_b) = p.begin_build("b").unwrap();
        p.close_conversation("a");
        cancelled(rx_a).await;
        let still_running = tokio::time::timeout(Duration::from_millis(50), cancelled(rx_b.clone())).await;
        assert!(still_running.is_err(), "the other chat's build keeps going");
        assert!(p.session("a").is_err() && p.session("b").is_ok());
        p.cancel_conversation("b");
        cancelled(rx_b).await;
    }

    #[tokio::test]
    async fn a_cancelled_chat_can_build_again() {
        let p = Playlists::new(Path::new("/nonexistent"));
        p.insert_conversation("a".into(), chat(), 3, Lang::En).unwrap();
        let (_, first) = p.begin_build("a").unwrap();
        p.cancel_conversation("a");
        cancelled(first).await;
        let (_, second) = p.begin_build("a").unwrap();
        let fresh = tokio::time::timeout(Duration::from_millis(50), cancelled(second)).await;
        assert!(fresh.is_err(), "a new build starts with a clean switch");
    }

    #[tokio::test]
    async fn a_concert_job_does_not_cancel_a_chat() {
        let p = Playlists::new(Path::new("/nonexistent"));
        p.insert_conversation("a".into(), chat(), 3, Lang::En).unwrap();
        let (_, rx) = p.begin_build("a").unwrap();
        let _job = p.begin();
        p.cancel();
        assert!(tokio::time::timeout(Duration::from_millis(50), cancelled(rx)).await.is_err());
    }

    #[test]
    fn the_cap_refuses_the_next_open() {
        let p = Playlists::new(Path::new("/nonexistent"));
        for id in ["a", "b"] {
            p.insert_conversation(id.into(), chat(), 2, Lang::En).unwrap();
        }
        let refused = p.insert_conversation("c".into(), chat(), 2, Lang::En).unwrap_err();
        assert_eq!(refused, "You already have 2 AI chats open. Close one to start another.");
        assert!(p.session("c").is_err());
        p.insert_conversation("a".into(), chat(), 2, Lang::En).expect("reopening an id is not a new chat");
        p.close_conversation("b");
        p.insert_conversation("c".into(), chat(), 2, Lang::En).unwrap();
    }

    #[tokio::test]
    async fn a_new_job_cancels_the_previous_one() {
        let p = Playlists::new(Path::new("/nonexistent"));
        let first = p.begin();
        let second = p.begin();
        cancelled(first.clone()).await;
        p.finish(&first);
        assert!(lock(&p.cancel).is_some());
        p.cancel();
        cancelled(second).await;
    }
}
