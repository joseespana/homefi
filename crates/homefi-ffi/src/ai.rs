//! Bring-your-own-key AI for Android and iOS: the providers, model lists, key
//! checks and the EQ assistant ([`EqAssistantSession`]).
//!
//! The key is never part of a record (Kotlin data classes and Swift structs
//! print every field): it is a separate `api_key` argument, read by the app
//! from the Android Keystore / iOS Keychain right before the call.
//! Profiles and settings are the records of [`crate::eq`].

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use futures::future::BoxFuture;
use futures::FutureExt;

use homefi_core::ai::{self, EqSession, SavedSession};

use crate::eq::{
    EqProfile, EqSettings, ItemRef, LoudnessSettings, NowPlayingContext, TasteRule, TasteScope, TasteSettings,
};
use crate::{HomefiError, Result};

fn failed(message: impl Into<String>) -> HomefiError {
    HomefiError::Failed { message: message.into() }
}

/// Who answers.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AiProvider {
    Anthropic,
    OpenAi,
    Gemini,
    OpenRouter,
    Mistral,
    Groq,
    XAi,
    DeepSeek,
    /// Local server, no key.
    Ollama,
    /// Any OpenAI-compatible server.
    OpenAiCompatible {
        base_url: String,
    },
}

impl From<AiProvider> for ai::AiProvider {
    fn from(p: AiProvider) -> Self {
        match p {
            AiProvider::Anthropic => Self::Anthropic,
            AiProvider::OpenAi => Self::OpenAi,
            AiProvider::Gemini => Self::Gemini,
            AiProvider::OpenRouter => Self::OpenRouter,
            AiProvider::Mistral => Self::Mistral,
            AiProvider::Groq => Self::Groq,
            AiProvider::XAi => Self::XAi,
            AiProvider::DeepSeek => Self::DeepSeek,
            AiProvider::Ollama => Self::Ollama,
            AiProvider::OpenAiCompatible { base_url } => Self::OpenAiCompatible { base_url },
        }
    }
}

impl From<ai::AiProvider> for AiProvider {
    fn from(p: ai::AiProvider) -> Self {
        match p {
            ai::AiProvider::Anthropic => Self::Anthropic,
            ai::AiProvider::OpenAi => Self::OpenAi,
            ai::AiProvider::Gemini => Self::Gemini,
            ai::AiProvider::OpenRouter => Self::OpenRouter,
            ai::AiProvider::Mistral => Self::Mistral,
            ai::AiProvider::Groq => Self::Groq,
            ai::AiProvider::XAi => Self::XAi,
            ai::AiProvider::DeepSeek => Self::DeepSeek,
            ai::AiProvider::Ollama => Self::Ollama,
            ai::AiProvider::OpenAiCompatible { base_url } => Self::OpenAiCompatible { base_url },
        }
    }
}

/// A provider as a settings screen lists it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AiProviderInfo {
    pub provider: AiProvider,
    /// Stable id (`anthropic`, `openai`, ... `openai_compatible`): settings and keystore entry name.
    pub id: String,
    pub name: String,
    /// Where to create a key; empty for the custom server.
    pub key_url: String,
    pub needs_key: bool,
    pub default_base_url: String,
    /// The user may (Ollama) or must (custom) type the server's address.
    pub custom_base_url: bool,
    /// Only Anthropic has one; the rest pick from [`ai_list_models`].
    pub default_model: Option<String>,
    /// 1 = the best pick; [`ai_providers`] comes sorted by it.
    #[uniffi(default = 0)]
    pub rank: u8,
    #[uniffi(default = None)]
    pub badge: Option<AiProviderBadge>,
    /// One line on why to pick it, in the language of [`crate::set_language`]
    /// (or the one given to [`ai_providers_in`]).
    #[uniffi(default = "")]
    pub hint: String,
}

/// Why a provider stands out in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiProviderBadge {
    Recommended,
    Cheapest,
    /// One key for every vendor's models (OpenRouter).
    OneKeyForAll,
    /// Runs on this device or network: free and private.
    PrivateLocal,
}

impl From<ai::ProviderBadge> for AiProviderBadge {
    fn from(b: ai::ProviderBadge) -> Self {
        match b {
            ai::ProviderBadge::Recommended => Self::Recommended,
            ai::ProviderBadge::Cheapest => Self::Cheapest,
            ai::ProviderBadge::OneKeyForAll => Self::OneKeyForAll,
            ai::ProviderBadge::PrivateLocal => Self::PrivateLocal,
        }
    }
}

impl From<AiProviderBadge> for ai::ProviderBadge {
    fn from(b: AiProviderBadge) -> Self {
        match b {
            AiProviderBadge::Recommended => Self::Recommended,
            AiProviderBadge::Cheapest => Self::Cheapest,
            AiProviderBadge::OneKeyForAll => Self::OneKeyForAll,
            AiProviderBadge::PrivateLocal => Self::PrivateLocal,
        }
    }
}

fn lang(code: &str) -> homefi_core::humane::Lang {
    if code.to_lowercase().starts_with("es") {
        homefi_core::humane::Lang::Es
    } else {
        homefi_core::humane::Lang::En
    }
}

/// Provider and model, without the key (passed apart, see the module docs).
#[derive(Debug, Clone, uniffi::Record)]
pub struct AiConfig {
    pub provider: AiProvider,
    /// Empty: the provider's default or the best one it lists.
    pub model: String,
    /// Another address (an Ollama on another machine, a proxy); `None` = the provider's.
    pub base_url: Option<String>,
}

fn core_config(config: AiConfig, api_key: Option<String>) -> ai::AiConfig {
    ai::AiConfig {
        provider: config.provider.into(),
        model: config.model,
        api_key,
        base_url: config.base_url.filter(|u| !u.trim().is_empty()),
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// What "Automatic" picks.
    #[uniffi(default = false)]
    pub recommended: bool,
    /// Small by its id (`8b`, `nano`, `lite`...): cheap, but makes more mistakes.
    #[uniffi(default = false)]
    pub small: bool,
    /// The provider's cheaper tier that still works well (Claude Haiku).
    #[uniffi(default = false)]
    pub economical: bool,
}

impl From<ai::ModelInfo> for ModelInfo {
    fn from(m: ai::ModelInfo) -> Self {
        Self { id: m.id, name: m.name, recommended: m.recommended, small: m.small, economical: m.economical }
    }
}

/// Every provider with what a settings screen shows, best pick first, hints
/// in the language of [`crate::set_language`].
#[uniffi::export]
pub fn ai_providers() -> Vec<AiProviderInfo> {
    providers_in(homefi_core::humane::language())
}

/// [`ai_providers`] with the hints in `language` (`es`, `en-US`...).
#[uniffi::export]
pub fn ai_providers_in(language: String) -> Vec<AiProviderInfo> {
    providers_in(lang(&language))
}

/// Short label of a badge ("Recommended", "Recomendado"...).
#[uniffi::export]
pub fn ai_provider_badge_label(badge: AiProviderBadge, language: String) -> String {
    ai::ProviderBadge::from(badge).label_in(lang(&language)).to_owned()
}

fn providers_in(lang: homefi_core::humane::Lang) -> Vec<AiProviderInfo> {
    let mut all = ai::AiProvider::all();
    all.sort_by_key(ai::AiProvider::rank);
    all.into_iter()
        .map(|p| {
            let info = p.info_in(lang);
            AiProviderInfo {
                provider: p.into(),
                id: info.id,
                name: info.name,
                key_url: info.key_url,
                needs_key: info.needs_key,
                default_base_url: info.default_base_url,
                custom_base_url: info.custom_base_url,
                default_model: info.default_model,
                rank: info.rank,
                badge: info.badge.map(Into::into),
                hint: info.hint,
            }
        })
        .collect()
}

/// The chat models the key can use.
#[uniffi::export]
pub async fn ai_list_models(config: AiConfig, api_key: Option<String>) -> Result<Vec<ModelInfo>> {
    crate::runtime::run(async move {
        let models = ai::list_models(&core_config(config, api_key)).await?;
        Ok(models.into_iter().map(Into::into).collect())
    })
    .await
}

/// Checks the key (or that a local server answers) without spending tokens.
#[uniffi::export]
pub async fn ai_test_key(config: AiConfig, api_key: Option<String>) -> Result<()> {
    crate::runtime::run(async move { Ok(ai::test_key(&core_config(config, api_key)).await?) }).await
}

/// The model to use when none is chosen: the provider's default or a pick from its list.
#[uniffi::export]
pub async fn ai_default_model(config: AiConfig, api_key: Option<String>) -> Result<String> {
    crate::runtime::run(async move { Ok(ai::resolve_model(&core_config(config, api_key)).await?) }).await
}

// ------------------------------------------------------------- the assistant

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum OutputKind {
    UsbDac,
    Headphones,
    Bluetooth,
    BuiltIn,
    Car,
    Hdmi,
    Other,
}

impl From<OutputKind> for ai::OutputKind {
    fn from(k: OutputKind) -> Self {
        match k {
            OutputKind::UsbDac => Self::UsbDac,
            OutputKind::Headphones => Self::Headphones,
            OutputKind::Bluetooth => Self::Bluetooth,
            OutputKind::BuiltIn => Self::BuiltIn,
            OutputKind::Car => Self::Car,
            OutputKind::Hdmi => Self::Hdmi,
            OutputKind::Other => Self::Other,
        }
    }
}

/// One of the user's outputs.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AudioOutput {
    /// The key of the EQ settings' `perOutput`.
    pub id: String,
    pub name: String,
    pub kind: OutputKind,
    pub sample_rates: Vec<u32>,
}

/// What the user listens to most, so the assistant offers layers that matter.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ListeningSummary {
    pub top_genres: Vec<String>,
    pub top_artists: Vec<ItemRef>,
    pub playlists: Vec<ItemRef>,
}

impl From<ListeningSummary> for ai::ListeningSummary {
    fn from(l: ListeningSummary) -> Self {
        Self {
            top_genres: l.top_genres,
            top_artists: l.top_artists.into_iter().map(Into::into).collect(),
            playlists: l.playlists.into_iter().map(Into::into).collect(),
        }
    }
}

/// What the assistant knows before the first question.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AssistantContext {
    /// `es`, `en`, `es-MX`...
    pub language: String,
    pub outputs: Vec<AudioOutput>,
    /// The equalizer as it is now; `None` = defaults.
    pub current: Option<EqSettings>,
    pub notes: Option<String>,
    /// The taste rules as they are; `None` = none.
    #[uniffi(default = None)]
    pub taste: Option<TasteSettings>,
    #[uniffi(default = None)]
    pub loudness: Option<LoudnessSettings>,
    #[uniffi(default = None)]
    pub listening: Option<ListeningSummary>,
    #[uniffi(default = None)]
    pub now_playing: Option<NowPlayingContext>,
    /// `id` of the output playing now.
    #[uniffi(default = None)]
    pub active_output: Option<String>,
    /// "Ask the AI to refine" from a rule's editor: that rule's id. The AI
    /// starts from its curve (the guided wizard ignores it).
    #[uniffi(default = None)]
    pub focus_rule: Option<String>,
}

impl From<AssistantContext> for ai::AssistantContext {
    fn from(c: AssistantContext) -> Self {
        Self {
            language: c.language,
            outputs: c
                .outputs
                .into_iter()
                .map(|o| ai::AudioOutput { id: o.id, name: o.name, kind: o.kind.into(), sample_rates: o.sample_rates })
                .collect(),
            current: c.current.map(Into::into).unwrap_or_default(),
            notes: c.notes.filter(|n| !n.trim().is_empty()),
            taste: c.taste.map(Into::into).unwrap_or_default(),
            loudness: c.loudness.map(Into::into).unwrap_or_default(),
            listening: c.listening.map(Into::into),
            now_playing: c.now_playing.map(Into::into),
            active_output: c.active_output.filter(|o| !o.trim().is_empty()),
            focus_rule: c.focus_rule.filter(|r| !r.trim().is_empty()),
        }
    }
}

/// A profile ready to apply.
#[derive(Debug, Clone, uniffi::Record)]
pub struct EqProposal {
    pub output_id: String,
    /// Validated, boosts capped, automatic preamp set.
    pub profile: EqProfile,
    pub explanation: String,
}

/// One answer of the assistant or of the guided wizard.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AssistantTurn {
    pub text: String,
    pub quick_replies: Vec<String>,
    /// The profile on the table (preview it; apply it when `confirmed`).
    pub proposal: Option<EqProposal>,
    /// The user said yes: save `proposal` for its output and turn the EQ on.
    pub confirmed: bool,
    pub done: bool,
    /// Taste rules changed this turn (the user agreed): save and apply them.
    #[uniffi(default = None)]
    pub taste: Option<TasteSettings>,
    /// Loudness changed this turn: save and apply it.
    #[uniffi(default = None)]
    pub loudness: Option<LoudnessSettings>,
    /// A taste layer proposed this turn (not saved): show its curve and band
    /// notes; "apply" replies yes, and the saved rules come in `taste`.
    #[uniffi(default = None)]
    pub taste_proposal: Option<TasteRule>,
}

impl From<ai::AssistantTurn> for AssistantTurn {
    fn from(t: ai::AssistantTurn) -> Self {
        Self {
            text: t.text,
            quick_replies: t.quick_replies,
            proposal: t.proposal.map(|p| EqProposal {
                output_id: p.output_id,
                profile: p.profile.into(),
                explanation: p.explanation,
            }),
            confirmed: t.confirmed,
            done: t.done,
            taste: t.taste.map(Into::into),
            loudness: t.loudness.map(Into::into),
            taste_proposal: t.taste_proposal.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct TranscriptLine {
    pub from_user: bool,
    pub text: String,
}

/// An EQ conversation: with AI ([`EqAssistantSession::new`]) or the guided
/// wizard ([`EqAssistantSession::guided`]). One call at a time.
#[derive(uniffi::Object)]
pub struct EqAssistantSession {
    /// Held for a whole turn (an AI call may take up to two minutes), so
    /// turns never overlap.
    inner: Arc<tokio::sync::Mutex<EqSession>>,
    /// Copy of the conversation after the last turn: saving and redrawing
    /// read it at once, without waiting for a turn in progress.
    saved: Arc<Mutex<Snapshot>>,
    ai: bool,
}

#[derive(Default)]
struct Snapshot {
    json: String,
    transcript: Vec<TranscriptLine>,
}

impl Snapshot {
    fn of(session: &EqSession) -> Self {
        Self {
            json: serde_json::to_string(&session.save()).unwrap_or_default(),
            transcript: session
                .transcript()
                .into_iter()
                .map(|l| TranscriptLine { from_user: l.from_user, text: l.text })
                .collect(),
        }
    }
}

impl EqAssistantSession {
    fn wrap(session: EqSession, cache_dir: String) -> Arc<Self> {
        // the engine's client for that folder: the parsed headphone list is shared
        let session = session.with_autoeq(crate::eq::autoeq(PathBuf::from(cache_dir)));
        Arc::new(Self {
            ai: session.is_ai(),
            saved: Arc::new(Mutex::new(Snapshot::of(&session))),
            inner: Arc::new(tokio::sync::Mutex::new(session)),
        })
    }

    /// Runs one turn on the runtime and refreshes the snapshot after it
    /// (a failed call leaves the conversation as it was, and says so).
    async fn turn<F>(&self, f: F) -> Result<AssistantTurn>
    where
        F: for<'a> FnOnce(&'a mut EqSession) -> BoxFuture<'a, homefi_core::Result<ai::AssistantTurn>> + Send + 'static,
    {
        let (inner, saved) = (self.inner.clone(), self.saved.clone());
        crate::runtime::run(async move {
            let mut session = inner.lock().await;
            let turn = f(&mut session).await;
            *saved.lock().unwrap_or_else(|e| e.into_inner()) = Snapshot::of(&session);
            Ok(turn?.into())
        })
        .await
    }
}

#[uniffi::export]
impl EqAssistantSession {
    /// The AI assistant. `cache_dir` keeps AutoEq's headphone list and
    /// profiles: pass `<app dir>/autoeq-cache` to share the cache of `autoeq_search`.
    #[uniffi::constructor]
    pub fn new(config: AiConfig, api_key: Option<String>, context: AssistantContext, cache_dir: String) -> Arc<Self> {
        Self::wrap(EqSession::ai(core_config(config, api_key), context.into()), cache_dir)
    }

    /// The fixed wizard, no AI and no key.
    #[uniffi::constructor]
    pub fn guided(context: AssistantContext, cache_dir: String) -> Arc<Self> {
        Self::wrap(EqSession::guided(context.into()), cache_dir)
    }

    /// Continues a session saved with [`state_json`](Self::state_json). An AI
    /// session needs `config` (and its key) or it starts over as the wizard.
    #[uniffi::constructor(default(config = None, api_key = None))]
    pub fn resume(
        state_json: String,
        cache_dir: String,
        config: Option<AiConfig>,
        api_key: Option<String>,
    ) -> Result<Arc<Self>> {
        let saved: SavedSession =
            serde_json::from_str(&state_json).map_err(|e| failed(format!("saved conversation: {e}")))?;
        Ok(Self::wrap(EqSession::resume(saved, config.map(|c| core_config(c, api_key))), cache_dir))
    }

    pub fn is_ai(&self) -> bool {
        self.ai
    }

    pub async fn start(&self) -> Result<AssistantTurn> {
        self.turn(|s| s.start().boxed()).await
    }

    /// A proactive taste suggestion for a card (instead of [`start`](Self::start));
    /// the user's answer goes to [`reply`](Self::reply).
    pub async fn suggest(&self) -> Result<AssistantTurn> {
        self.turn(|s| s.suggest().boxed()).await
    }

    pub async fn reply(&self, text: String) -> Result<AssistantTurn> {
        self.turn(move |s| async move { s.reply(&text).await }.boxed()).await
    }

    /// The conversation to save (no key in it), as of the last finished turn:
    /// answers at once even while a turn is in progress.
    pub async fn state_json(&self) -> String {
        self.saved.lock().unwrap_or_else(|e| e.into_inner()).json.clone()
    }

    /// What was said so far, to redraw a resumed AI conversation (as of the
    /// last finished turn; never waits for one in progress).
    pub async fn transcript(&self) -> Vec<TranscriptLine> {
        self.saved.lock().unwrap_or_else(|e| e.into_inner()).transcript.clone()
    }
}

// ------------------------------------------------------------- reactions

/// "How does it sound?"
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SoundReaction {
    NeedsBass,
    TooBoomy,
    TooBright,
    Dull,
    VocalsBuried,
    TooQuiet,
    TooLoud,
    Perfect,
}

impl From<SoundReaction> for ai::SoundReaction {
    fn from(r: SoundReaction) -> Self {
        match r {
            SoundReaction::NeedsBass => Self::NeedsBass,
            SoundReaction::TooBoomy => Self::TooBoomy,
            SoundReaction::TooBright => Self::TooBright,
            SoundReaction::Dull => Self::Dull,
            SoundReaction::VocalsBuried => Self::VocalsBuried,
            SoundReaction::TooQuiet => Self::TooQuiet,
            SoundReaction::TooLoud => Self::TooLoud,
            SoundReaction::Perfect => Self::Perfect,
        }
    }
}

/// What a reaction knows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ReactionContext {
    pub language: String,
    pub now_playing: NowPlayingContext,
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
    /// What plays the music (built-in speakers get no deep bass).
    pub output_kind: OutputKind,
    /// Where the rule goes; `None`: the main genre (else artist, album, track).
    #[uniffi(default = None)]
    pub scope_hint: Option<TasteScope>,
}

/// What a reaction changed: save `taste` and `loudness` and apply them.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ReactionResult {
    pub rule: Option<TasteRule>,
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
    /// One line for a toast, in the user's language.
    pub message: String,
    /// For [`sound_undo`]; empty when nothing changed.
    pub undo_token: String,
    pub by_ai: bool,
}

/// What a reaction replaced.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SoundSnapshot {
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
}

/// The "before" of the last reactions, for [`sound_undo`].
static UNDO: LazyLock<ai::UndoStack> = LazyLock::new(Default::default);

/// Turns a reaction into a taste rule (or a loudness change). With `config`
/// one short AI call picks the scope and words the message; without it (or
/// when the call fails) fixed safe steps answer: ±2 dB per tap, capped at ±6.
#[uniffi::export]
pub async fn sound_react(
    reaction: SoundReaction,
    context: ReactionContext,
    config: Option<AiConfig>,
    api_key: Option<String>,
) -> ReactionResult {
    crate::runtime::run(async move {
        let ctx = ai::ReactionContext {
            language: context.language,
            now_playing: context.now_playing.into(),
            taste: context.taste.into(),
            loudness: context.loudness.into(),
            output_kind: context.output_kind.into(),
            scope_hint: context.scope_hint.map(Into::into),
        };
        let config = config.map(|c| core_config(c, api_key));
        let r = ai::react(reaction.into(), &ctx, config.as_ref()).await;
        UNDO.push(&r.undo_token, ai::SoundSnapshot { taste: ctx.taste.clone(), loudness: ctx.loudness });
        ReactionResult {
            rule: r.rule.map(Into::into),
            taste: r.taste.into(),
            loudness: r.loudness.into(),
            message: r.message,
            undo_token: r.undo_token,
            by_ai: r.by_ai,
        }
    })
    .await
}

/// The taste rules and loudness from before the reaction `token` (once);
/// `None` when unknown or already undone.
#[uniffi::export]
pub fn sound_undo(token: String) -> Option<SoundSnapshot> {
    UNDO.undo(&token).map(|s| SoundSnapshot { taste: s.taste.into(), loudness: s.loudness.into() })
}

/// Chip label of a reaction (`es` or `en`).
#[uniffi::export]
pub fn sound_reaction_label(reaction: SoundReaction, language: String) -> String {
    ai::SoundReaction::from(reaction).label(language.to_lowercase().starts_with("es")).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_come_with_their_info() {
        let all = ai_providers();
        assert_eq!(all.len(), 10);
        let anthropic = all.iter().find(|p| p.id == "anthropic").unwrap();
        assert_eq!(anthropic.default_model.as_deref(), Some("claude-sonnet-5-5"));
        assert!(anthropic.needs_key && !anthropic.key_url.is_empty());
        assert!(all.iter().any(|p| p.provider == AiProvider::Ollama && !p.needs_key));
        // ranked, with badges and hints
        assert_eq!(all[0].provider, AiProvider::Anthropic);
        assert_eq!(all[0].badge, Some(AiProviderBadge::Recommended));
        assert!(all.windows(2).all(|w| w[0].rank < w[1].rank));
        let es = ai_providers_in("es-MX".into());
        assert_eq!(es[1].provider, AiProvider::Gemini);
        assert_eq!(es[1].badge, Some(AiProviderBadge::Cheapest));
        assert!(es[1].hint.contains("barato"), "{}", es[1].hint);
        assert_eq!(ai_provider_badge_label(AiProviderBadge::OneKeyForAll, "en".into()), "One key for all");
        let m: ModelInfo = homefi_core::ai::ModelInfo::new("qwen3:8b", "Qwen 3 8B").into();
        assert!(m.small && !m.recommended && !m.economical);
    }

    #[tokio::test]
    async fn guided_sessions_save_and_resume() {
        let dir = std::env::temp_dir().join("homefi-ffi-ai-test");
        let context = AssistantContext {
            language: "en".into(),
            outputs: vec![AudioOutput {
                id: "car".into(),
                name: "Car".into(),
                kind: OutputKind::Car,
                sample_rates: vec![],
            }],
            current: None,
            notes: None,
            taste: None,
            loudness: None,
            listening: None,
            now_playing: None,
            active_output: None,
            focus_rule: None,
        };
        let s = EqAssistantSession::guided(context, dir.display().to_string());
        assert!(!s.is_ai());
        let t = s.start().await.unwrap();
        assert!(!t.quick_replies.is_empty());
        let saved = s.state_json().await;
        let again = EqAssistantSession::resume(saved, dir.display().to_string(), None, None).unwrap();
        let t = again.reply("More bass".into()).await.unwrap();
        assert!(t.text.contains("Where"), "{}", t.text);

        // saving never waits for a turn in progress (an AI call can take minutes)
        let busy = again.inner.clone();
        let _turn = busy.lock().await;
        let saved = tokio::time::timeout(std::time::Duration::from_millis(200), again.state_json()).await;
        assert!(saved.is_ok_and(|json| json.contains("guided")), "state_json waited for the turn");
        assert!(tokio::time::timeout(std::time::Duration::from_millis(200), again.transcript()).await.is_ok());
    }

    #[tokio::test]
    async fn reactions_without_ai_and_undo() {
        let ctx = ReactionContext {
            language: "en".into(),
            now_playing: NowPlayingContext {
                track_id: Some("1".into()),
                path: None,
                title: Some("x".into()),
                album: None,
                artists: vec![],
                genres: vec!["Rock".into()],
                source: None,
            },
            taste: crate::eq::taste_default_settings(),
            loudness: crate::eq::loudness_default_settings(),
            output_kind: OutputKind::Headphones,
            scope_hint: None,
        };
        let r = sound_react(SoundReaction::NeedsBass, ctx, None, None).await;
        assert!(!r.by_ai && r.taste.enabled);
        assert_eq!(r.rule.unwrap().scope, TasteScope::Genre { name: "Rock".into() });
        let before = sound_undo(r.undo_token.clone()).unwrap();
        assert!(before.taste.rules.is_empty());
        assert!(sound_undo(r.undo_token).is_none());
        assert_eq!(sound_reaction_label(SoundReaction::Dull, "es-MX".into()), "Apagado");
    }
}
