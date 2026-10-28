//! Bring-your-own-key AI: one small client for the big providers (Anthropic,
//! OpenAI, Google Gemini) and the OpenAI-compatible ones (OpenRouter, Mistral,
//! Groq, xAI, DeepSeek, Ollama, any custom server), with tool calling.
//!
//! The core never stores, prints or logs a key: the apps keep it in the OS
//! keychain (macOS Keychain, Windows Credential Manager, Secret Service,
//! Android Keystore, iOS Keychain) and pass it in [`AiConfig`] on every call.
//! [`AiConfig`]'s `Debug` redacts it and errors scrub it from the provider's text.
//!
//! First user: [`eq_assistant`], which interviews the listener and sets up the
//! equalizer (and taste layers); [`guided::GuidedEq`] is the same conversation
//! without AI. [`reactions`] turns a "how does it sound?" tap into a taste rule.

mod anthropic;
mod chat;
pub mod eq_assistant;
mod error;
mod gemini;
pub mod guided;
mod openai;
pub mod reactions;

use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::humane::Lang;

pub use chat::{chat, ChatReply, Message, Tool, ToolCall, ToolResult};
pub use eq_assistant::{
    AssistantContext, AssistantState, AssistantTurn, AudioOutput, EqAssistant, EqProposal, EqSession, ListeningSummary,
    OutputKind, SavedSession, TranscriptLine,
};
pub use error::{AiError, AiErrorKind};
pub use guided::{GuidedEq, GuidedState, GuidedStep};
pub use reactions::{react, react_guided, ReactionContext, ReactionResult, SoundReaction, SoundSnapshot, UndoStack};

/// Who answers. Every provider but Anthropic and Gemini speaks the OpenAI API.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "id", rename_all = "snake_case")]
pub enum AiProvider {
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    Gemini,
    #[serde(rename = "openrouter")]
    OpenRouter,
    Mistral,
    Groq,
    #[serde(rename = "xai")]
    XAi,
    #[serde(rename = "deepseek")]
    DeepSeek,
    /// A local Ollama server: no key, its OpenAI-compatible API.
    Ollama,
    /// Any other OpenAI-compatible server (LM Studio, vLLM, a proxy...).
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible {
        base_url: String,
    },
}

/// Which wire format a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Api {
    Anthropic,
    /// OpenAI's Responses API (its newest models only call tools there).
    OpenAiResponses,
    /// `/chat/completions`, the format everyone else copied.
    ChatCompletions,
    Gemini,
}

impl AiProvider {
    /// Every provider, in the order a settings screen lists them (by [`rank`](Self::rank)).
    pub fn all() -> Vec<AiProvider> {
        vec![
            AiProvider::Anthropic,
            AiProvider::Gemini,
            AiProvider::OpenAi,
            AiProvider::OpenRouter,
            AiProvider::Ollama,
            AiProvider::Mistral,
            AiProvider::Groq,
            AiProvider::XAi,
            AiProvider::DeepSeek,
            AiProvider::OpenAiCompatible { base_url: String::new() },
        ]
    }

    /// Place in the recommendation list, 1 = the best pick for the EQ
    /// assistant. No benchmark measures "EQ skill": the acoustics come from
    /// AutoEq, so what matters is reliable tool calling, good Spanish and a
    /// low price.
    pub fn rank(&self) -> u8 {
        match self {
            AiProvider::Anthropic => 1,
            AiProvider::Gemini => 2,
            AiProvider::OpenAi => 3,
            AiProvider::OpenRouter => 4,
            AiProvider::Ollama => 5,
            AiProvider::Mistral => 6,
            AiProvider::Groq => 7,
            AiProvider::XAi => 8,
            AiProvider::DeepSeek => 9,
            AiProvider::OpenAiCompatible { .. } => 10,
        }
    }

    /// The label a settings screen pins on the provider, if any.
    pub fn badge(&self) -> Option<ProviderBadge> {
        match self {
            AiProvider::Anthropic => Some(ProviderBadge::Recommended),
            AiProvider::Gemini => Some(ProviderBadge::Cheapest),
            AiProvider::OpenRouter => Some(ProviderBadge::OneKeyForAll),
            AiProvider::Ollama => Some(ProviderBadge::PrivateLocal),
            _ => None,
        }
    }

    /// One line on why (or when) to pick it, in `lang`.
    pub fn hint_in(&self, lang: Lang) -> &'static str {
        let es = lang == Lang::Es;
        match self {
            AiProvider::Anthropic if es => "El más fiable; por defecto Claude Sonnet 5.5",
            AiProvider::Anthropic => "Most reliable; default Claude Sonnet 5.5",
            AiProvider::Gemini if es => "Muy barato y rápido: elige un modelo Flash",
            AiProvider::Gemini => "Very cheap and fast — pick a Flash model",
            AiProvider::OpenAi if es => "Bueno y barato: un modelo mini",
            AiProvider::OpenAi => "Good and cheap — a mini model",
            AiProvider::OpenRouter if es => "Una sola llave para los modelos de todos",
            AiProvider::OpenRouter => "One key for everyone's models",
            AiProvider::Ollama if es => "Gratis y privado, pero los modelos pequeños se equivocan más (avanzado)",
            AiProvider::Ollama => "Free and private, but small models make more mistakes — advanced",
            AiProvider::Mistral if es => "Proveedor europeo",
            AiProvider::Mistral => "European provider",
            AiProvider::Groq if es => "Modelos abiertos, muy rápidos",
            AiProvider::Groq => "Open models, very fast",
            AiProvider::XAi if es => "Los modelos Grok",
            AiProvider::XAi => "The Grok models",
            AiProvider::DeepSeek if es => "De bajo costo",
            AiProvider::DeepSeek => "Low cost",
            AiProvider::OpenAiCompatible { .. } if es => "Tu propio servidor (LM Studio, vLLM...) (avanzado)",
            AiProvider::OpenAiCompatible { .. } => "Your own server (LM Studio, vLLM...) — advanced",
        }
    }

    /// Stable id, used in settings and as the keychain entry name.
    pub fn id(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "anthropic",
            AiProvider::OpenAi => "openai",
            AiProvider::Gemini => "gemini",
            AiProvider::OpenRouter => "openrouter",
            AiProvider::Mistral => "mistral",
            AiProvider::Groq => "groq",
            AiProvider::XAi => "xai",
            AiProvider::DeepSeek => "deepseek",
            AiProvider::Ollama => "ollama",
            AiProvider::OpenAiCompatible { .. } => "openai_compatible",
        }
    }

    /// The provider for an [`id`](Self::id); `base_url` is only used by the custom one.
    pub fn from_id(id: &str, base_url: Option<&str>) -> Option<AiProvider> {
        let custom = || AiProvider::OpenAiCompatible { base_url: base_url.unwrap_or_default().trim().to_owned() };
        Some(match id {
            "openai_compatible" | "custom" => custom(),
            other => AiProvider::all().into_iter().find(|p| p.id() == other)?,
        })
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "Anthropic (Claude)",
            AiProvider::OpenAi => "OpenAI (ChatGPT, Codex)",
            AiProvider::Gemini => "Google Gemini",
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Mistral => "Mistral",
            AiProvider::Groq => "Groq",
            AiProvider::XAi => "xAI (Grok)",
            AiProvider::DeepSeek => "DeepSeek",
            AiProvider::Ollama => "Ollama",
            AiProvider::OpenAiCompatible { .. } => "OpenAI-compatible",
        }
    }

    /// Short name for messages ("La llave de Gemini no es válida").
    pub fn short_name(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "Anthropic",
            AiProvider::OpenAi => "OpenAI",
            AiProvider::Gemini => "Gemini",
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Mistral => "Mistral",
            AiProvider::Groq => "Groq",
            AiProvider::XAi => "xAI",
            AiProvider::DeepSeek => "DeepSeek",
            AiProvider::Ollama => "Ollama",
            AiProvider::OpenAiCompatible { .. } => "OpenAI-compatible",
        }
    }

    /// Where to get a key (or, for Ollama, the app itself).
    pub fn key_url(&self) -> &'static str {
        match self {
            AiProvider::Anthropic => "https://platform.claude.com/settings/keys",
            AiProvider::OpenAi => "https://platform.openai.com/api-keys",
            AiProvider::Gemini => "https://aistudio.google.com/apikey",
            AiProvider::OpenRouter => "https://openrouter.ai/settings/keys",
            AiProvider::Mistral => "https://console.mistral.ai/api-keys",
            AiProvider::Groq => "https://console.groq.com/keys",
            AiProvider::XAi => "https://console.x.ai",
            AiProvider::DeepSeek => "https://platform.deepseek.com/api_keys",
            AiProvider::Ollama => "https://ollama.com/download",
            AiProvider::OpenAiCompatible { .. } => "",
        }
    }

    /// `false` for local servers (Ollama); a custom server may or may not want one.
    pub fn needs_key(&self) -> bool {
        !matches!(self, AiProvider::Ollama | AiProvider::OpenAiCompatible { .. })
    }

    /// Runs on this machine or network: "can't connect" means "is it open?", not "no internet".
    pub fn is_local(&self) -> bool {
        matches!(self, AiProvider::Ollama | AiProvider::OpenAiCompatible { .. })
    }

    /// The API root, without a trailing slash.
    pub fn default_base_url(&self) -> String {
        match self {
            AiProvider::Anthropic => "https://api.anthropic.com/v1",
            AiProvider::OpenAi => "https://api.openai.com/v1",
            AiProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            AiProvider::OpenRouter => "https://openrouter.ai/api/v1",
            AiProvider::Mistral => "https://api.mistral.ai/v1",
            AiProvider::Groq => "https://api.groq.com/openai/v1",
            AiProvider::XAi => "https://api.x.ai/v1",
            AiProvider::DeepSeek => "https://api.deepseek.com",
            AiProvider::Ollama => "http://localhost:11434/v1",
            AiProvider::OpenAiCompatible { base_url } => base_url.as_str(),
        }
        .trim_end_matches('/')
        .to_owned()
    }

    /// The model to use when the user picked none. Only Anthropic has a fixed
    /// one; the rest are chosen from [`list_models`] with [`pick_model`].
    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            AiProvider::Anthropic => Some("claude-sonnet-5-5"),
            _ => None,
        }
    }

    pub(crate) fn api(&self) -> Api {
        match self {
            AiProvider::Anthropic => Api::Anthropic,
            AiProvider::OpenAi => Api::OpenAiResponses,
            AiProvider::Gemini => Api::Gemini,
            _ => Api::ChatCompletions,
        }
    }

    /// What a settings screen shows about the provider, hint in the app's language.
    pub fn info(&self) -> ProviderInfo {
        self.info_in(crate::humane::language())
    }

    /// [`info`](Self::info) with the hint in `lang`.
    pub fn info_in(&self, lang: Lang) -> ProviderInfo {
        ProviderInfo {
            id: self.id().to_owned(),
            name: self.display_name().to_owned(),
            key_url: self.key_url().to_owned(),
            needs_key: self.needs_key(),
            default_base_url: self.default_base_url(),
            custom_base_url: matches!(self, AiProvider::OpenAiCompatible { .. } | AiProvider::Ollama),
            default_model: self.default_model().map(str::to_owned),
            rank: self.rank(),
            badge: self.badge(),
            hint: self.hint_in(lang).to_owned(),
        }
    }
}

/// Why a provider stands out in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBadge {
    /// The most reliable at calling tools; the default.
    Recommended,
    /// Cheapest per conversation.
    Cheapest,
    /// One key reaches the models of every vendor (OpenRouter).
    OneKeyForAll,
    /// Runs on this machine: free and private.
    PrivateLocal,
}

impl ProviderBadge {
    /// Short label of the badge in `lang`.
    pub fn label_in(self, lang: Lang) -> &'static str {
        let es = lang == Lang::Es;
        match self {
            ProviderBadge::Recommended if es => "Recomendado",
            ProviderBadge::Recommended => "Recommended",
            ProviderBadge::Cheapest if es => "El más barato",
            ProviderBadge::Cheapest => "Cheapest",
            ProviderBadge::OneKeyForAll if es => "Una llave para todo",
            ProviderBadge::OneKeyForAll => "One key for all",
            ProviderBadge::PrivateLocal if es => "Privado, local",
            ProviderBadge::PrivateLocal => "Private, local",
        }
    }
}

/// A provider as settings screens list it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// Where to create a key; empty for the custom server.
    pub key_url: String,
    pub needs_key: bool,
    pub default_base_url: String,
    /// The user may (Ollama) or must (custom) type the server's address.
    pub custom_base_url: bool,
    pub default_model: Option<String>,
    /// 1 = the best pick; [`providers`] comes sorted by it.
    #[serde(default)]
    pub rank: u8,
    #[serde(default)]
    pub badge: Option<ProviderBadge>,
    /// One line on why to pick it, in the app's language.
    #[serde(default)]
    pub hint: String,
}

/// Every provider with its display info, best pick first.
pub fn providers() -> Vec<ProviderInfo> {
    providers_in(crate::humane::language())
}

/// [`providers`] with the hints in `lang`.
pub fn providers_in(lang: Lang) -> Vec<ProviderInfo> {
    let mut all: Vec<ProviderInfo> = AiProvider::all().iter().map(|p| p.info_in(lang)).collect();
    all.sort_by_key(|p| p.rank);
    all
}

/// What to call: provider, model and the user's key (never stored by the core).
#[derive(Clone, PartialEq, Eq)]
pub struct AiConfig {
    pub provider: AiProvider,
    /// Empty: [`AiProvider::default_model`] or one picked from [`list_models`].
    pub model: String,
    pub api_key: Option<String>,
    /// Overrides the provider's address (another Ollama host, a proxy, tests).
    pub base_url: Option<String>,
}

impl fmt::Debug for AiConfig {
    /// Never prints the key, only whether there is one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl AiConfig {
    pub fn new(provider: AiProvider, model: impl Into<String>, api_key: Option<String>) -> Self {
        Self { provider, model: model.into(), api_key, base_url: None }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// The API root actually used.
    pub(crate) fn base(&self) -> String {
        match self.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            Some(url) => url.trim_end_matches('/').to_owned(),
            None => self.provider.default_base_url(),
        }
    }

    /// The key, if a real one was given (blank counts as none).
    pub(crate) fn key(&self) -> Option<&str> {
        self.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty())
    }

    /// Fails early, without a request, when a key is required and missing.
    pub(crate) fn check(&self) -> Result<(), AiError> {
        if self.provider.needs_key() && self.key().is_none() {
            return Err(AiError::new(self, AiErrorKind::NoKey, "no key"));
        }
        if self.base().is_empty() {
            return Err(AiError::new(self, AiErrorKind::NotFound, "no server address"));
        }
        Ok(())
    }
}

/// A model a provider offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// What goes in [`AiConfig::model`].
    pub id: String,
    /// Human name when the provider gives one, otherwise the id.
    pub name: String,
    /// What [`pick_model`] would choose (the "Automatic" model).
    #[serde(default)]
    pub recommended: bool,
    /// A small model by its id (`8b`, `nano`, `lite`...): cheap, but it makes
    /// more mistakes with tools and instructions.
    #[serde(default)]
    pub small: bool,
    /// The provider's cheaper tier that still works well (Claude Haiku).
    #[serde(default)]
    pub economical: bool,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let id = id.into();
        Self { small: is_small(&id), name: name.into(), id, recommended: false, economical: false }
    }
}

/// Size words in model ids that mean "small": `nano`, `tiny`, `lite`, and a
/// parameter count up to 8B (`8b`, `3b`, `1.5b`, Gemma's `e4b`).
pub fn is_small(id: &str) -> bool {
    let id = id.to_lowercase();
    id.split(['-', ':', '/', '_', ' ', '@']).any(|token| {
        if matches!(token, "nano" | "tiny" | "lite" | "micro") {
            return true;
        }
        let Some(size) = token.strip_suffix('b') else { return false };
        let size = size.strip_prefix('e').unwrap_or(size);
        size.parse::<f32>().is_ok_and(|b| b > 0.0 && b <= 8.0)
    })
}

/// Marks the recommended pick (the provider's default when listed, else
/// [`pick_model`]'s) and, for Anthropic, Haiku as the economical one.
pub fn annotate_models(provider: &AiProvider, models: &mut [ModelInfo]) {
    let pick = provider
        .default_model()
        .filter(|d| models.iter().any(|m| m.id == *d))
        .map(str::to_owned)
        .or_else(|| pick_model(provider, models));
    for m in models.iter_mut() {
        m.small = is_small(&m.id);
        m.recommended = pick.as_deref() == Some(m.id.as_str());
        m.economical = *provider == AiProvider::Anthropic && m.id.to_lowercase().contains("haiku");
    }
}

/// The models the key can use, from the provider's models endpoint; only
/// those that can chat (no embeddings, speech, images...).
pub async fn list_models(config: &AiConfig) -> crate::Result<Vec<ModelInfo>> {
    config.check()?;
    let models = match config.provider.api() {
        Api::Anthropic => anthropic::list_models(config).await?,
        Api::Gemini => gemini::list_models(config).await?,
        Api::OpenAiResponses | Api::ChatCompletions => openai::list_models(config).await?,
    };
    let mut models: Vec<ModelInfo> = models.into_iter().filter(|m| can_chat(&m.id)).collect();
    annotate_models(&config.provider, &mut models);
    Ok(models)
}

/// Checks that the key works (and, for local servers, that they answer),
/// without spending tokens.
pub async fn test_key(config: &AiConfig) -> crate::Result<()> {
    config.check()?;
    match config.provider {
        // its model list is public: ask about the key itself
        AiProvider::OpenRouter => openai::check_openrouter_key(config).await,
        _ => list_models(config).await.map(drop),
    }
}

/// The model to call: the configured one, the provider's default, or the
/// best guess from what the key can use.
pub async fn resolve_model(config: &AiConfig) -> crate::Result<String> {
    if !config.model.trim().is_empty() {
        return Ok(config.model.trim().to_owned());
    }
    if let Some(model) = config.provider.default_model() {
        return Ok(model.to_owned());
    }
    let models = list_models(config).await?;
    pick_model(&config.provider, &models)
        .ok_or_else(|| AiError::new(config, AiErrorKind::ModelNotFound, "the provider lists no chat model").into())
}

/// Names of models that are not for chatting.
const NOT_CHAT: &[&str] = &[
    "embed",
    "tts",
    "whisper",
    "dall-e",
    "image",
    "audio",
    "realtime",
    "moderation",
    "transcribe",
    "davinci",
    "babbage",
    "sora",
    "guard",
    "rerank",
    "ocr",
    "aqa",
    "imagen",
    "veo",
    "lyria",
    "computer-use",
    "speech",
];

fn can_chat(id: &str) -> bool {
    let id = id.to_lowercase();
    !NOT_CHAT.iter().any(|w| id.contains(w))
}

/// Picks a sensible chat model from a provider's list, without hard-coding
/// ids that change every few months: a preferred family for the job (fast
/// models that are good at tools), stable over preview, newest version first.
pub fn pick_model(provider: &AiProvider, models: &[ModelInfo]) -> Option<String> {
    // families in order of preference; later entries are fallbacks
    let families: &[&str] = match provider {
        AiProvider::Anthropic => &["sonnet", "haiku", "opus"],
        AiProvider::OpenAi => &["gpt-mini", "gpt", "o"],
        AiProvider::Gemini => &["gemini-flash", "gemini-pro", "gemini"],
        AiProvider::OpenRouter => &["anthropic/claude-sonnet", "openai/gpt-mini", "google/gemini-flash", "openai/gpt"],
        AiProvider::Mistral => &["mistral-medium", "mistral-large", "mistral-small"],
        AiProvider::Groq => &["gpt-oss", "llama", "qwen", "kimi"],
        AiProvider::XAi => &["grok-fast", "grok-mini", "grok"],
        AiProvider::DeepSeek => &["deepseek-chat", "deepseek"],
        AiProvider::Ollama | AiProvider::OpenAiCompatible { .. } => &[],
    };
    // a family "a-b" matches ids containing "a" and then "b" (e.g. gpt-5.6-mini for "gpt-mini")
    let rank = |id: &str| -> usize {
        let id = id.to_lowercase();
        families
            .iter()
            .position(|f| {
                let mut rest = id.as_str();
                f.split('-').all(|part| match rest.find(part) {
                    Some(at) => {
                        rest = &rest[at + part.len()..];
                        true
                    }
                    None => false,
                })
            })
            .unwrap_or(families.len())
    };
    let unstable = |id: &str| {
        let id = id.to_lowercase();
        ["preview", "exp", "beta", "alpha", "deprecated"].iter().any(|w| id.contains(w)) as u8
    };
    // a dated snapshot (…-2025-08-07, …-20250807) after its plain alias
    let dated = |id: &str| id.split(|c: char| !c.is_ascii_digit()).any(|n| n.len() >= 4) as u8;
    let mut candidates: Vec<&ModelInfo> = models.iter().filter(|m| can_chat(&m.id)).collect();
    // OpenAI also lists many special-purpose models: keep the chat families
    if *provider == AiProvider::OpenAi {
        candidates.retain(|m| {
            let id = m.id.to_lowercase();
            (id.starts_with("gpt-") || id.starts_with('o'))
                && !["codex", "search", "oss", "instruct", "chat-latest"].iter().any(|w| id.contains(w))
        });
    }
    // a small model (flash-lite, 8b) only when nothing bigger is listed
    candidates.sort_by(|a, b| {
        (rank(&a.id), unstable(&a.id), is_small(&a.id), dated(&a.id))
            .cmp(&(rank(&b.id), unstable(&b.id), is_small(&b.id), dated(&b.id)))
            .then_with(|| version(&b.id).cmp(&version(&a.id)))
            .then_with(|| a.id.len().cmp(&b.id.len()))
    });
    candidates.first().map(|m| m.id.clone())
}

/// The small numbers in an id (`gemini-3.8-flash` → [3, 8]), dates left out.
fn version(id: &str) -> Vec<u32> {
    id.split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .take_while(|n| n.len() < 4)
        .filter_map(|n| n.parse().ok())
        .collect()
}

/// The HTTP client of every AI call: no total timeout set here (each request
/// sets its own), but a stalled connection is given up.
pub(crate) fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("homefi/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest client")
    })
}

/// How long a model may think before we give up.
pub(crate) const CHAT_TIMEOUT: Duration = Duration::from_secs(120);
/// Listing models and testing keys are quick.
pub(crate) const LIST_TIMEOUT: Duration = Duration::from_secs(20);

#[cfg(test)]
mod tests {
    use super::*;

    fn models(ids: &[&str]) -> Vec<ModelInfo> {
        ids.iter().map(|id| ModelInfo::new(*id, *id)).collect()
    }

    #[test]
    fn the_key_never_shows_in_debug() {
        let c = AiConfig::new(AiProvider::OpenAi, "x", Some("sk-very-secret".into()));
        let text = format!("{c:?}");
        assert!(!text.contains("sk-very-secret"));
        assert!(text.contains("<redacted>"));
    }

    #[test]
    fn providers_round_trip_by_id() {
        for p in AiProvider::all() {
            assert_eq!(AiProvider::from_id(p.id(), Some("")), Some(p.clone()));
            let json = serde_json::to_value(&p).unwrap();
            assert_eq!(json["id"], p.id());
            assert_eq!(serde_json::from_value::<AiProvider>(json).unwrap(), p);
        }
        assert_eq!(
            AiProvider::from_id("openai_compatible", Some("http://box:8080/v1/ ")).unwrap().default_base_url(),
            "http://box:8080/v1"
        );
        assert!(AiProvider::from_id("nope", None).is_none());
        assert!(!AiProvider::Ollama.needs_key() && AiProvider::Gemini.needs_key());
        assert_eq!(providers().len(), 10);
    }

    #[test]
    fn missing_key_fails_before_any_request() {
        let c = AiConfig::new(AiProvider::Anthropic, "", Some("  ".into()));
        assert_eq!(c.check().unwrap_err().kind, AiErrorKind::NoKey);
        assert!(AiConfig::new(AiProvider::Ollama, "", None).check().is_ok());
        let blank = AiConfig::new(AiProvider::OpenAiCompatible { base_url: String::new() }, "", None);
        assert_eq!(blank.check().unwrap_err().kind, AiErrorKind::NotFound);
    }

    #[test]
    fn picks_a_fast_stable_model_that_is_good_at_tools() {
        let openai = models(&[
            "text-embedding-3-large",
            "gpt-5.6",
            "gpt-5.6-mini",
            "gpt-5.4-mini",
            "gpt-5.6-mini-2026-07-01",
            "gpt-realtime",
            "o4-mini",
            "gpt-5.6-codex",
        ]);
        assert_eq!(pick_model(&AiProvider::OpenAi, &openai).as_deref(), Some("gpt-5.6-mini"));
        let gemini = models(&[
            "gemini-3.8-pro",
            "gemini-3.8-flash-preview",
            "gemini-3.5-flash",
            "gemini-3.8-flash",
            "text-embedding-004",
        ]);
        assert_eq!(pick_model(&AiProvider::Gemini, &gemini).as_deref(), Some("gemini-3.8-flash"));
        let mistral =
            models(&["mistral-small-latest", "mistral-medium-latest", "mistral-medium-2508", "codestral-latest"]);
        assert_eq!(pick_model(&AiProvider::Mistral, &mistral).as_deref(), Some("mistral-medium-latest"));
        // nothing preferred: still something
        let local = models(&["qwen3:8b"]);
        assert_eq!(pick_model(&AiProvider::Ollama, &local).as_deref(), Some("qwen3:8b"));
        assert_eq!(pick_model(&AiProvider::Groq, &models(&["whisper-large-v3"])), None);
    }

    #[test]
    fn versions_skip_dates() {
        assert_eq!(version("gpt-5.6-mini-2026-07-01"), vec![5, 6]);
        assert_eq!(version("claude-sonnet-5-5"), vec![5, 5]);
    }

    #[test]
    fn providers_come_ranked_with_badges_and_hints() {
        let all = providers_in(Lang::En);
        let ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(&ids[..5], ["anthropic", "gemini", "openai", "openrouter", "ollama"]);
        assert!(all.windows(2).all(|w| w[0].rank < w[1].rank));
        assert_eq!(all[0].badge, Some(ProviderBadge::Recommended));
        assert_eq!(all[0].hint, "Most reliable; default Claude Sonnet 5.5");
        assert_eq!(all[1].badge, Some(ProviderBadge::Cheapest));
        assert_eq!((all[2].rank, all[2].badge), (3, None));
        assert_eq!(all[3].badge, Some(ProviderBadge::OneKeyForAll));
        assert_eq!(all[4].badge, Some(ProviderBadge::PrivateLocal));
        assert!(all.iter().all(|p| !p.hint.is_empty()));
        let es = providers_in(Lang::Es);
        assert!(es[1].hint.contains("Flash") && es[1].hint.contains("barato"), "{}", es[1].hint);
        assert!(es.iter().zip(&all).all(|(e, n)| e.hint != n.hint && e.id == n.id));
        // the apps read it as JSON
        let json = serde_json::to_value(&all[3]).unwrap();
        assert_eq!(json["badge"], "one_key_for_all");
        assert_eq!(ProviderBadge::PrivateLocal.label_in(Lang::Es), "Privado, local");
        // older JSON without the new fields still parses
        let old = r#"{"id":"x","name":"X","key_url":"","needs_key":true,"default_base_url":"","custom_base_url":false,"default_model":null}"#;
        let p: ProviderInfo = serde_json::from_str(old).unwrap();
        assert_eq!((p.rank, p.badge, p.hint.as_str()), (0, None, ""));
    }

    #[test]
    fn small_models_by_their_ids() {
        for id in [
            "qwen3:8b",
            "llama3.2:3b",
            "llama-3.1-8b-instant",
            "qwen2.5:1.5b",
            "gemma3n:e4b",
            "gpt-5.6-nano",
            "gemini-3.8-flash-lite",
            "TinyLlama/tiny",
            "meta-llama/llama-3.2-1b-instruct",
        ] {
            assert!(is_small(id), "{id}");
        }
        for id in [
            "claude-haiku-5-5",
            "gpt-5.6-mini",
            "gemini-3.8-flash",
            "llama3.3:70b",
            "gemma3:12b",
            "mixtral-8x7b",
            "mistral-small-latest",
            "deepseek-chat",
            "gpt-oss-120b",
        ] {
            assert!(!is_small(id), "{id}");
        }
    }

    #[test]
    fn models_are_marked_recommended_small_and_economical() {
        let mut anthropic = models(&["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"]);
        annotate_models(&AiProvider::Anthropic, &mut anthropic);
        let flags: Vec<(bool, bool, bool)> = anthropic.iter().map(|m| (m.recommended, m.small, m.economical)).collect();
        assert_eq!(flags, [(false, false, false), (true, false, false), (false, false, true)]);

        let mut gemini = models(&["gemini-3.8-flash-lite", "gemini-3.8-flash", "gemini-3.8-pro"]);
        annotate_models(&AiProvider::Gemini, &mut gemini);
        assert!(gemini[0].small && !gemini[0].recommended);
        assert!(gemini[1].recommended && !gemini[1].small);
        assert!(gemini.iter().all(|m| !m.economical));

        // a bigger local model is preferred to an 8B one
        let mut local = models(&["qwen3:8b", "llama3.3:70b"]);
        annotate_models(&AiProvider::Ollama, &mut local);
        assert!(local[0].small && !local[0].recommended && local[1].recommended);
        assert_eq!(local.iter().filter(|m| m.recommended).count(), 1);

        // listed JSON from an older app (no flags) still parses
        let m: ModelInfo = serde_json::from_str(r#"{"id":"a","name":"A"}"#).unwrap();
        assert!(!m.recommended && !m.small && !m.economical);
    }
}
