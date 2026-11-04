//! HomeFi playlists ("Get ready for a concert", `src/lib/api.ts`): thin commands over
//! `homefi_core::concert` and `PlaylistStore`. Every rule lives in the core; nothing
//! here writes to the Tidal account.

use std::path::Path;
use std::sync::Mutex;

use homefi_core::ai::AiProvider;
use homefi_core::concert::{self, BuildContext, ConcertBuilt, ConcertProgress, ConcertRequest};
use homefi_core::humane::{self, Lang};
use homefi_core::party::{
    self, ChatBrief, PartyBrief, PartyBuilt, PartyContext, PartyProgress, PartySession, PartyTurn, SongSeed,
};
use homefi_core::playlist::{HomePlaylist, LibraryIndex, LibraryRef, PlaylistStore};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::{watch, Mutex as AsyncMutex};

use crate::commands::{err, AppState, TrackView};

type CmdResult<T> = Result<T, String>;

/// Playlist state of the app: the store and the cancel switch of the running build.
pub struct Playlists {
    store: PlaylistStore,
    cancel: Mutex<Option<watch::Sender<bool>>>,
    /// The open party conversation (held across awaits, hence the async mutex).
    party: AsyncMutex<Option<PartySession>>,
}

impl Playlists {
    pub fn new(app_dir: &Path) -> Self {
        Self { store: PlaylistStore::in_app_dir(app_dir), cancel: Mutex::new(None), party: AsyncMutex::new(None) }
    }

    /// Registers a new running job, cancelling the one before it.
    fn begin(&self) -> watch::Receiver<bool> {
        let (tx, rx) = watch::channel(false);
        if let Some(previous) = self.cancel.lock().unwrap().replace(tx) {
            let _ = previous.send(true);
        }
        rx
    }

    /// Clears the switch, unless a newer job already took its place.
    fn finish(&self, rx: &watch::Receiver<bool>) {
        let mut slot = self.cancel.lock().unwrap();
        if slot.as_ref().is_some_and(|tx| tx.subscribe().same_channel(rx)) {
            *slot = None;
        }
    }

    fn cancel(&self) {
        if let Some(tx) = self.cancel.lock().unwrap().as_ref() {
            let _ = tx.send(true);
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
fn library_refs(tracks: &[TrackView]) -> Vec<LibraryRef> {
    tracks
        .iter()
        .map(|t| LibraryRef {
            path: t.path.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            isrc: t.isrc.clone(),
            duration_ms: t.duration_ms,
        })
        .collect()
}

fn cancelled_text() -> String {
    match humane::language() {
        Lang::Es => "cancelado".into(),
        Lang::En => "cancelled".into(),
    }
}

/// Resolves when `true` is sent; pends forever if the switch is dropped unused.
async fn cancelled(mut rx: watch::Receiver<bool>) {
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

/// Opens (replacing) the party conversation; the error is the "needs an AI" text when none is set up.
#[tauri::command]
pub async fn party_open(app: AppHandle, brief: PartyBrief, language: String) -> CmdResult<()> {
    let lang = Lang::from_code(&language);
    let Some(config) = crate::ai::settings_config(&app).await else {
        return Err(party::needs_ai_message(lang));
    };
    *app.state::<Playlists>().party.lock().await = Some(PartySession::new(config, brief, lang));
    Ok(())
}

/// Opens (replacing) the "Playlist with AI" conversation in the same slot; `party_start` gives the opener.
#[tauri::command]
pub async fn chat_open(app: AppHandle, brief: ChatBrief, language: String) -> CmdResult<()> {
    let lang = Lang::from_code(&language);
    let Some(config) = crate::ai::settings_config(&app).await else {
        return Err(party::needs_ai_message(lang));
    };
    *app.state::<Playlists>().party.lock().await = Some(PartySession::new(config, brief, lang));
    Ok(())
}

/// The first chat message for "Playlist like this song" (`language`: "en" or "es").
#[tauri::command]
pub fn chat_song_wish(seed: SongSeed, language: String) -> String {
    party::song_wish(&seed, Lang::from_code(&language))
}

#[tauri::command]
pub async fn party_set_brief(app: AppHandle, brief: PartyBrief) -> CmdResult<()> {
    let playlists = app.state::<Playlists>();
    let mut slot = playlists.party.lock().await;
    slot.as_mut().ok_or_else(|| party::no_session_message(humane::language()))?.set_brief(brief);
    Ok(())
}

#[tauri::command]
pub async fn party_start(app: AppHandle) -> CmdResult<PartyTurn> {
    let playlists = app.state::<Playlists>();
    let mut slot = playlists.party.lock().await;
    let session = slot.as_mut().ok_or_else(|| party::no_session_message(humane::language()))?;
    session.start().await.map_err(err)
}

#[tauri::command]
pub async fn party_reply(app: AppHandle, text: String) -> CmdResult<PartyTurn> {
    let playlists = app.state::<Playlists>();
    let mut slot = playlists.party.lock().await;
    let session = slot.as_mut().ok_or_else(|| party::no_session_message(humane::language()))?;
    session.reply(&text).await.map_err(err)
}

#[tauri::command]
pub async fn party_build(
    app: AppHandle,
    replace_id: Option<String>,
    on_progress: Channel<PartyProgress>,
) -> CmdResult<PartyBuilt> {
    let playlists = app.state::<Playlists>();
    let rx = playlists.begin();
    let result = async {
        let mut slot = playlists.party.lock().await;
        let session = slot.as_mut().ok_or_else(|| party::no_session_message(humane::language()))?;
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
pub fn party_cancel(playlists: State<'_, Playlists>) {
    playlists.cancel();
}

#[tauri::command]
pub async fn party_close(app: AppHandle) {
    *app.state::<Playlists>().party.lock().await = None;
}

#[tauri::command]
pub async fn home_playlists(app: AppHandle) -> CmdResult<Vec<HomePlaylist>> {
    let dir = app.state::<Playlists>().store.clone();
    tauri::async_runtime::spawn_blocking(move || dir.list()).await.map_err(err)
}

#[tauri::command]
pub async fn home_playlist_save(app: AppHandle, playlist: HomePlaylist) -> CmdResult<()> {
    let store = app.state::<Playlists>().store.clone();
    tauri::async_runtime::spawn_blocking(move || store.save(&playlist)).await.map_err(err)?.map_err(err)
}

/// Moves the playlist's JSON to the system Trash (never a hard delete); `false` when unknown.
#[tauri::command]
pub async fn home_playlist_delete(app: AppHandle, id: String) -> CmdResult<bool> {
    let store = app.state::<Playlists>().store.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<bool> {
        let Some(file) = store.file_path(&id) else {
            return Ok(false);
        };
        match trash::delete(&file) {
            Ok(()) => Ok(true),
            Err(_) if !file.exists() => Ok(false),
            Err(e) => {
                tracing::warn!("could not move {} to the trash: {e}", file.display());
                Err(err(e))
            }
        }
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn the_party_slot_starts_empty() {
        let p = Playlists::new(Path::new("/nonexistent"));
        assert!(p.party.lock().await.is_none());
    }

    #[tokio::test]
    async fn a_new_job_cancels_the_previous_one() {
        let p = Playlists::new(Path::new("/nonexistent"));
        let first = p.begin();
        let second = p.begin();
        cancelled(first.clone()).await;
        p.finish(&first);
        assert!(p.cancel.lock().unwrap().is_some());
        p.cancel();
        cancelled(second).await;
    }
}
