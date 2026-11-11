//! The party playlist for Android and iOS: the optional chat
//! ([`PartySession`]) and the build ([`HomefiEngine::party_build`]).
//!
//! A thin bridge over `homefi_core::party`. The AI's key travels apart from
//! the config and is never stored in a record, like in [`crate::ai`].

use std::sync::Arc;

use homefi_core::humane::Lang;
use homefi_core::party as core_party;
use homefi_core::playlist as pl;

use crate::ai::{core_config, AiConfig};
use crate::playlists::{count, library_index, ChatBrief, HomePlaylist, LibraryRef, PartyBrief, PlaylistSettings};
use crate::{HomefiEngine, Result};

/// What the AI says in the chat, and the answers it offers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PartyTurn {
    pub text: String,
    pub quick_replies: Vec<String>,
    /// The brief as the AI now understands it (chat sessions only).
    pub chat_brief: Option<ChatBrief>,
    /// The conversation's title as the core sees it now (for the list of open chats).
    pub title: String,
}

impl From<core_party::PartyTurn> for PartyTurn {
    fn from(t: core_party::PartyTurn) -> Self {
        Self { text: t.text, quick_replies: t.quick_replies, chat_brief: t.chat_brief.map(Into::into), title: t.title }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PartyBuilt {
    pub playlist: HomePlaylist,
    pub note: Option<String>,
}

impl From<core_party::PartyBuilt> for PartyBuilt {
    fn from(b: core_party::PartyBuilt) -> Self {
        Self { playlist: b.playlist.into(), note: b.note }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PartyProgress {
    /// Waiting for the AI's songs (`round` 0 is the first ask, then the fill-ups).
    Asking {
        round: u8,
    },
    Resolving {
        done: u32,
        total: u32,
    },
}

impl From<core_party::PartyProgress> for PartyProgress {
    fn from(p: core_party::PartyProgress) -> Self {
        match p {
            core_party::PartyProgress::Asking { round } => Self::Asking { round },
            core_party::PartyProgress::Resolving { done, total } => {
                Self::Resolving { done: count(done), total: count(total) }
            }
        }
    }
}

/// Told as the build advances (called from a background thread).
#[uniffi::export(callback_interface)]
pub trait PartyListener: Send + Sync {
    fn on_progress(&self, progress: PartyProgress);
}

/// The optional chat before (and between) builds. Turns never overlap (one tokio mutex).
#[derive(uniffi::Object)]
pub struct PartySession {
    inner: Arc<tokio::sync::Mutex<core_party::PartySession>>,
}

impl PartySession {
    fn with_brief(config: AiConfig, api_key: Option<String>, brief: core_party::Brief, language: &str) -> Arc<Self> {
        let session = core_party::PartySession::new(core_config(config, api_key), brief, Lang::from_code(language));
        Arc::new(Self { inner: Arc::new(tokio::sync::Mutex::new(session)) })
    }
}

#[uniffi::export]
impl PartySession {
    #[uniffi::constructor]
    pub fn new(config: AiConfig, api_key: Option<String>, brief: PartyBrief, language: String) -> Arc<Self> {
        Self::with_brief(config, api_key, core_party::PartyBrief::from(brief).into(), &language)
    }

    /// A "Playlist with AI" session: the brief is what the user says in the chat.
    #[uniffi::constructor]
    pub fn new_chat(config: AiConfig, api_key: Option<String>, brief: ChatBrief, language: String) -> Arc<Self> {
        Self::with_brief(config, api_key, core_party::ChatBrief::from(brief).into(), &language)
    }

    /// The answers changed; the chat stays (party sessions).
    pub async fn set_brief(&self, brief: PartyBrief) {
        self.inner.lock().await.set_brief(core_party::PartyBrief::from(brief));
    }

    /// The conversation's title: the AI's, else the start of the wish, else fixed words (party: its name).
    pub async fn title(&self) -> String {
        self.inner.lock().await.title()
    }

    /// Clears the chat. Party: the AI's first question. Chat: the fixed opener, no network.
    pub async fn start(&self) -> Result<PartyTurn> {
        let inner = self.inner.clone();
        crate::runtime::run(async move { Ok(inner.lock().await.start().await?.into()) }).await
    }

    pub async fn reply(&self, text: String) -> Result<PartyTurn> {
        let inner = self.inner.clone();
        crate::runtime::run(async move { Ok(inner.lock().await.reply(&text).await?.into()) }).await
    }
}

/// "You already have N AI chats open. Close one…" in `language`, for when the app refuses a new chat.
#[uniffi::export]
pub fn too_many_chats_message(language: String, max: u8) -> String {
    core_party::too_many_chats_message(Lang::from_code(&language), max)
}

#[uniffi::export]
impl HomefiEngine {
    /// Builds the playlist from the session (not saved: call `home_playlist_save`).
    /// `replace_id`: the saved playlist this rebuild replaces ("Change answers");
    /// it keeps that id and creation time. Cancelling the coroutine stops the
    /// work and leaves the session as it was.
    pub async fn party_build(
        self: Arc<Self>,
        session: Arc<PartySession>,
        settings: PlaylistSettings,
        library: Vec<LibraryRef>,
        replace_id: Option<String>,
        listener: Box<dyn PartyListener>,
    ) -> Result<PartyBuilt> {
        crate::runtime::run(async move {
            // signed out is not an error: only the library is used
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api().ok();
            let index = library_index(library).await;
            let settings = pl::PlaylistSettings::from(settings);
            let ctx = core_party::PartyContext { settings: &settings, api: api.as_ref(), library: &index };
            let mut built = session.inner.lock().await.build(&ctx, &|p| listener.on_progress(p.into())).await?;
            if let Some(id) = replace_id {
                if let Ok(old) = pl::PlaylistStore::in_app_dir(self.session.app_dir()).load(&id) {
                    built.playlist.keep_identity_of(&old);
                }
            }
            Ok(built.into())
        })
        .await
    }
}

/// The questions' defaults from the settings (Friends, no ages, Dance, the settings' length/curve/explicit).
#[uniffi::export]
pub fn party_default_brief(settings: PlaylistSettings) -> PartyBrief {
    core_party::PartyBrief::from_settings(&pl::PlaylistSettings::from(settings)).into()
}

/// The chat's defaults (length, energy, explicit lyrics) from the settings; nothing asked yet.
#[uniffi::export]
pub fn chat_default_brief(settings: PlaylistSettings) -> ChatBrief {
    core_party::ChatBrief::from_settings(&pl::PlaylistSettings::from(settings)).into()
}

/// A song to build a playlist like.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SongSeed {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub year: Option<u16>,
    pub genre: Option<String>,
}

impl From<SongSeed> for core_party::SongSeed {
    fn from(s: SongSeed) -> Self {
        Self { title: s.title, artist: s.artist, album: s.album, year: s.year, genre: s.genre }
    }
}

/// The first chat message for "Playlist like this song" (`language`: "en" or "es").
#[uniffi::export]
pub fn chat_song_wish(seed: SongSeed, language: String) -> String {
    core_party::song_wish(&seed.into(), Lang::from_code(&language))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlists::{playlist_default_settings, EnergyCurve};

    #[test]
    fn progress_converts_and_saturates() {
        assert_eq!(
            PartyProgress::from(core_party::PartyProgress::Asking { round: 1 }),
            PartyProgress::Asking { round: 1 }
        );
        assert_eq!(
            PartyProgress::from(core_party::PartyProgress::Resolving { done: 9, total: 40 }),
            PartyProgress::Resolving { done: 9, total: 40 }
        );
        assert_eq!(
            PartyProgress::from(core_party::PartyProgress::Resolving { done: usize::MAX, total: 1 }),
            PartyProgress::Resolving { done: u32::MAX, total: 1 }
        );
    }

    #[test]
    fn turn_and_built_convert() {
        let turn = PartyTurn::from(core_party::PartyTurn {
            text: "Hi".into(),
            quick_replies: vec!["A".into()],
            chat_brief: None,
            title: String::new(),
        });
        assert_eq!(
            turn,
            PartyTurn { text: "Hi".into(), quick_replies: vec!["A".into()], chat_brief: None, title: String::new() }
        );
        let built =
            PartyBuilt::from(core_party::PartyBuilt { playlist: pl::HomePlaylist::default(), note: Some("n".into()) });
        assert_eq!(built.note.as_deref(), Some("n"));
    }

    #[test]
    fn turn_converts_the_chat_brief() {
        let brief = core_party::ChatBrief { wish: "w".into(), title: "T".into(), ..core_party::ChatBrief::default() };
        let turn = PartyTurn::from(core_party::PartyTurn {
            text: String::new(),
            quick_replies: Vec::new(),
            chat_brief: Some(brief.clone()),
            title: String::new(),
        });
        assert_eq!(turn.chat_brief, Some(brief.into()));
    }

    #[test]
    fn turn_converts_the_title() {
        let turn = PartyTurn::from(core_party::PartyTurn { title: "Sad 90s".into(), ..Default::default() });
        assert_eq!(turn.title, "Sad 90s");
    }

    #[test]
    fn the_cap_message_follows_the_language() {
        assert!(too_many_chats_message("en".into(), 3).starts_with("You already have 3 AI chats open"));
        assert!(too_many_chats_message("es".into(), 3).starts_with("Ya tienes 3 chats"));
    }

    #[test]
    fn song_wish_follows_the_language() {
        let seed = SongSeed {
            title: "Hotel California".into(),
            artist: "Eagles".into(),
            album: None,
            year: Some(1976),
            genre: None,
        };
        assert!(chat_song_wish(seed.clone(), "en".into())
            .starts_with("A playlist like “Hotel California” by Eagles (1976):"));
        assert!(
            chat_song_wish(seed, "es-MX".into()).starts_with("Una playlist como “Hotel California” de Eagles (1976):")
        );
    }

    #[test]
    fn default_chat_brief_matches_core() {
        let brief = chat_default_brief(playlist_default_settings());
        assert_eq!(brief, core_party::ChatBrief::default().into());
        assert_eq!((brief.minutes, brief.curve), (60, EnergyCurve::Steady));
        assert!(brief.allow_explicit && brief.wish.is_empty() && brief.title.is_empty());
    }

    #[tokio::test]
    async fn a_chat_session_starts_offline() {
        let config = AiConfig { provider: crate::ai::AiProvider::Ollama, model: String::new(), base_url: None };
        let session =
            PartySession::new_chat(config, None, chat_default_brief(playlist_default_settings()), "en".into());
        let turn = session.start().await.unwrap();
        assert_eq!(turn.quick_replies.len(), 4);
        assert!(turn.chat_brief.is_some());
    }

    #[test]
    fn default_brief_matches_core() {
        let brief = party_default_brief(playlist_default_settings());
        assert_eq!(brief, core_party::PartyBrief::default().into());
        assert_eq!(brief.minutes, 120);
        assert_eq!(brief.curve, EnergyCurve::Arc);
        assert!(brief.allow_explicit);
    }

    #[tokio::test]
    async fn session_builds_offline_and_takes_a_new_brief() {
        let config = AiConfig { provider: crate::ai::AiProvider::Ollama, model: String::new(), base_url: None };
        let brief = party_default_brief(playlist_default_settings());
        let session = PartySession::new(config, None, brief.clone(), "es".into());
        let mut other = brief;
        other.minutes = 60;
        session.set_brief(other).await;
        assert_eq!(session.inner.lock().await.brief().minutes(), 60);
    }
}
