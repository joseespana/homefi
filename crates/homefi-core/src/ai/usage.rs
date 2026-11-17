//! What the AI costs: a small spending ledger and the guard that asks before spending.
//!
//! Every chat call notes its tokens, web searches and price (exact when the provider says it, as
//! OpenRouter does in `usage.cost`; otherwise estimated from [`PRICES`], or unknown) into the user
//! action it belongs to. The action is opened by the feature that starts it with [`track`] (a ride
//! build, a concert search, a party chat...), so the feature tag is set where the work begins, never
//! guessed from the prompt; nested [`track`]s join the outer one, and calls that share a `group` (the
//! turns of one chat) add up to one history item. The ledger keeps only feature, time, provider,
//! model, tokens, searches and cost: no prompt, no song, no key.
//!
//! [`Ledger::check`] is the one guard every app asks before an AI action ([`SpendDecision`]); the
//! monthly limit is also enforced on every call ([`ensure_allowed`]), so no platform can skip it.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use chrono::{DateTime, Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiConfig, AiError, AiErrorKind, AiProvider, Api};
use crate::humane::Lang;

/// The ledger's file in the app folder.
pub const LEDGER_FILENAME: &str = "ai_spending.json";
/// History items kept (the oldest go first): about a year of daily use, a few hundred KB at most.
pub const MAX_ACTIONS: usize = 500;
/// Past actions of a feature averaged for its estimate: recent enough to follow a model change.
pub const ESTIMATE_SAMPLE: usize = 10;
/// "Ask only above": allowed range and default (USD).
pub const MIN_ASK_ABOVE_USD: f64 = 0.01;
pub const MAX_ASK_ABOVE_USD: f64 = 100.0;
pub const DEFAULT_ASK_ABOVE_USD: f64 = 0.10;
/// Monthly limit: allowed range (USD); `None` is no limit.
pub const MIN_MONTHLY_LIMIT_USD: f64 = 0.10;
pub const MAX_MONTHLY_LIMIT_USD: f64 = 10_000.0;
/// What a new limit starts at when the user turns it on.
pub const DEFAULT_MONTHLY_LIMIT_USD: f64 = 5.0;
/// Tokens are priced per million.
const TOKENS_PER_PRICE_UNIT: f64 = 1_000_000.0;
/// Anthropic bills a cache read at a tenth of the input price and a cache write at 1.25 times it.
const ANTHROPIC_CACHE_READ_FACTOR: f64 = 0.1;
const ANTHROPIC_CACHE_WRITE_FACTOR: f64 = 1.25;
/// Longest model id kept (a custom server could send anything).
const MAX_MODEL_CHARS: usize = 96;

// ------------------------------------------------------------------ features

/// The thing the user did that called the AI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiFeature {
    /// Talking about a ride before it is built.
    RideChat,
    /// Building a ride with the AI's song ideas.
    RideBuild,
    /// Swapping one song of a ride with the AI's help.
    RideSwap,
    /// A concert playlist (the web research and its summary).
    Concert,
    /// A party playlist: its questions and the build.
    PartyPlaylist,
    /// A playlist made by chatting: the chat and the build.
    ChatPlaylist,
    /// The sound assistant (equalizer and taste).
    SoundAssistant,
    /// A "how does it sound?" tap.
    SoundReaction,
    /// Anything not tagged (should stay empty).
    #[default]
    #[serde(other)]
    Other,
}

impl AiFeature {
    pub const ALL: [AiFeature; 9] = [
        AiFeature::RideChat,
        AiFeature::RideBuild,
        AiFeature::RideSwap,
        AiFeature::Concert,
        AiFeature::PartyPlaylist,
        AiFeature::ChatPlaylist,
        AiFeature::SoundAssistant,
        AiFeature::SoundReaction,
        AiFeature::Other,
    ];

    /// Stable id, the same in JSON, Kotlin and TypeScript.
    pub fn id(self) -> &'static str {
        match self {
            AiFeature::RideChat => "ride_chat",
            AiFeature::RideBuild => "ride_build",
            AiFeature::RideSwap => "ride_swap",
            AiFeature::Concert => "concert",
            AiFeature::PartyPlaylist => "party_playlist",
            AiFeature::ChatPlaylist => "chat_playlist",
            AiFeature::SoundAssistant => "sound_assistant",
            AiFeature::SoundReaction => "sound_reaction",
            AiFeature::Other => "other",
        }
    }

    /// The feature for an [`id`](Self::id); unknown ids are [`AiFeature::Other`].
    pub fn from_id(id: &str) -> AiFeature {
        AiFeature::ALL.into_iter().find(|f| f.id() == id).unwrap_or(AiFeature::Other)
    }

    /// Plain words for a screen.
    pub fn label_in(self, lang: Lang) -> &'static str {
        let es = lang == Lang::Es;
        match self {
            AiFeature::RideChat if es => "Hablar de un recorrido",
            AiFeature::RideChat => "Talking about a ride",
            AiFeature::RideBuild if es => "Armar un recorrido",
            AiFeature::RideBuild => "Building a ride",
            AiFeature::RideSwap if es => "Cambiar una canción del recorrido",
            AiFeature::RideSwap => "Swapping a ride song",
            AiFeature::Concert if es => "Listas de concierto",
            AiFeature::Concert => "Concert playlists",
            AiFeature::PartyPlaylist if es => "Listas de fiesta",
            AiFeature::PartyPlaylist => "Party playlists",
            AiFeature::ChatPlaylist if es => "Listas conversando",
            AiFeature::ChatPlaylist => "Playlists by chat",
            AiFeature::SoundAssistant if es => "Asistente de sonido",
            AiFeature::SoundAssistant => "Sound assistant",
            AiFeature::SoundReaction if es => "«¿Cómo suena?»",
            AiFeature::SoundReaction => "“How does it sound?”",
            AiFeature::Other if es => "Otros",
            AiFeature::Other => "Other",
        }
    }
}

// --------------------------------------------------------------------- calls

/// How sure a cost is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostKind {
    /// The provider said it (OpenRouter), or the call is free (Ollama).
    Exact,
    /// Tokens times the list price in [`PRICES`]: shown with "≈".
    Estimated,
    /// No price for this model: only the tokens are known.
    #[default]
    #[serde(other)]
    Unknown,
}

impl CostKind {
    /// Exact + estimated is an estimate; anything unknown makes the total unknown.
    fn and(self, other: CostKind) -> CostKind {
        match (self, other) {
            (CostKind::Unknown, _) | (_, CostKind::Unknown) => CostKind::Unknown,
            (CostKind::Estimated, _) | (_, CostKind::Estimated) => CostKind::Estimated,
            (CostKind::Exact, CostKind::Exact) => CostKind::Exact,
        }
    }
}

/// What one answer used, read from the provider's `usage`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct CallUsage {
    /// Every prompt token, cached or not.
    pub input_tokens: u64,
    /// Of those, read from Anthropic's prompt cache.
    pub cache_read_tokens: u64,
    /// Of those, written to Anthropic's prompt cache.
    pub cache_write_tokens: u64,
    /// The answer, thinking included.
    pub output_tokens: u64,
    pub web_searches: u32,
    /// The provider's own price for the call (OpenRouter's `usage.cost`), in USD.
    pub reported_cost_usd: Option<f64>,
}

fn count(v: &Value, pointer: &str) -> u64 {
    v.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

impl CallUsage {
    /// Reads the answer of `api`. `searched`: the request asked for web search (OpenRouter's
    /// plugin and Gemini's grounding are billed per request, not per query).
    pub(crate) fn read(api: Api, answer: &Value, searched: bool) -> CallUsage {
        match api {
            Api::Anthropic => {
                let (fresh, read, write) = (
                    count(answer, "/usage/input_tokens"),
                    count(answer, "/usage/cache_read_input_tokens"),
                    count(answer, "/usage/cache_creation_input_tokens"),
                );
                CallUsage {
                    input_tokens: fresh + read + write,
                    cache_read_tokens: read,
                    cache_write_tokens: write,
                    output_tokens: count(answer, "/usage/output_tokens"),
                    web_searches: count(answer, "/usage/server_tool_use/web_search_requests") as u32,
                    reported_cost_usd: None,
                }
            }
            Api::OpenAiResponses => CallUsage {
                input_tokens: count(answer, "/usage/input_tokens"),
                output_tokens: count(answer, "/usage/output_tokens"),
                web_searches: answer["output"]
                    .as_array()
                    .map_or(0, |items| items.iter().filter(|i| i["type"] == "web_search_call").count() as u32),
                ..CallUsage::default()
            },
            Api::ChatCompletions => CallUsage {
                input_tokens: count(answer, "/usage/prompt_tokens"),
                output_tokens: count(answer, "/usage/completion_tokens"),
                web_searches: u32::from(searched),
                reported_cost_usd: answer.pointer("/usage/cost").and_then(Value::as_f64).filter(|c| *c >= 0.0),
                ..CallUsage::default()
            },
            Api::Gemini => CallUsage {
                input_tokens: count(answer, "/usageMetadata/promptTokenCount"),
                output_tokens: count(answer, "/usageMetadata/candidatesTokenCount")
                    + count(answer, "/usageMetadata/thoughtsTokenCount"),
                web_searches: u32::from(searched && answer.pointer("/candidates/0/groundingMetadata").is_some()),
                ..CallUsage::default()
            },
        }
    }
}

// -------------------------------------------------------------------- prices

/// A model's list price, USD per million tokens, and per web search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Price {
    pub input: f64,
    pub output: f64,
    pub per_search: f64,
}

/// How a [`PRICES`] row matches a model id (vendor prefix, date and `-latest` already removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Matches {
    Exact,
    /// Every version of a family whose list price has not changed across versions.
    Family,
}

/// List prices (USD per million tokens) of the default and common models of the providers that
/// publish them, used **only for estimates** shown with "≈" (as of 2026; check the provider's page).
/// A model not listed here has an unknown cost: only its tokens are shown. OpenRouter needs no table
/// (it reports the exact cost). Cached prompt tokens are priced as fresh ones except on Anthropic,
/// so an estimate errs on the high side. Searches: Anthropic and OpenAI $10 per 1,000, Gemini's
/// grounding $35 per 1,000 prompts after its free daily allowance.
const PRICES: &[(&str, Matches, Price)] = &[
    // Claude Sonnet has cost $3 / $15 in every version since Claude 3
    ("claude-sonnet-", Matches::Family, Price { input: 3.0, output: 15.0, per_search: 0.01 }),
    ("claude-haiku-4-5", Matches::Exact, Price { input: 1.0, output: 5.0, per_search: 0.01 }),
    ("claude-opus-4-5", Matches::Exact, Price { input: 5.0, output: 25.0, per_search: 0.01 }),
    ("gpt-5", Matches::Exact, Price { input: 1.25, output: 10.0, per_search: 0.01 }),
    ("gpt-5-mini", Matches::Exact, Price { input: 0.25, output: 2.0, per_search: 0.01 }),
    ("gpt-5-nano", Matches::Exact, Price { input: 0.05, output: 0.40, per_search: 0.01 }),
    ("gpt-4.1", Matches::Exact, Price { input: 2.0, output: 8.0, per_search: 0.025 }),
    ("gpt-4.1-mini", Matches::Exact, Price { input: 0.40, output: 1.60, per_search: 0.025 }),
    ("gpt-4o", Matches::Exact, Price { input: 2.50, output: 10.0, per_search: 0.025 }),
    ("gpt-4o-mini", Matches::Exact, Price { input: 0.15, output: 0.60, per_search: 0.025 }),
    ("gemini-2.5-pro", Matches::Exact, Price { input: 1.25, output: 10.0, per_search: 0.035 }),
    ("gemini-2.5-flash", Matches::Exact, Price { input: 0.30, output: 2.50, per_search: 0.035 }),
    ("gemini-2.5-flash-lite", Matches::Exact, Price { input: 0.10, output: 0.40, per_search: 0.035 }),
];

/// A model id as the price table writes it: lower case, no `vendor/` prefix, no `-latest`, no
/// date snapshot (`-20250929`, `-2025-08-07`).
fn plain_model(id: &str) -> String {
    let id = id.trim().to_lowercase();
    let id = id.rsplit('/').next().unwrap_or_default();
    let id = id.strip_suffix("-latest").unwrap_or(id);
    let parts: Vec<&str> = id.split('-').collect();
    let is_year = |p: &str| p.len() == 4 && p.starts_with("20") && p.bytes().all(|b| b.is_ascii_digit());
    let keep = match parts.as_slice() {
        // …-2025-08-07
        [.., y, m, d] if is_year(y) && m.len() == 2 && d.len() == 2 => parts.len() - 3,
        // …-20250929
        [.., date] if date.len() == 8 && date.starts_with("20") && date.bytes().all(|b| b.is_ascii_digit()) => {
            parts.len() - 1
        }
        _ => parts.len(),
    };
    parts[..keep].join("-")
}

/// The list price of `model` on `provider`, if the table has it. Only the providers whose own
/// prices the table lists (OpenRouter forwards the same models at the same price).
pub(crate) fn price_of(provider: &AiProvider, model: &str) -> Option<Price> {
    if !matches!(provider, AiProvider::Anthropic | AiProvider::OpenAi | AiProvider::Gemini | AiProvider::OpenRouter) {
        return None;
    }
    let plain = plain_model(model);
    PRICES
        .iter()
        .find(|(id, how, _)| match how {
            Matches::Exact => plain == *id,
            Matches::Family => plain.starts_with(id),
        })
        .map(|(_, _, price)| *price)
}

/// The cost of one call and how sure it is.
pub(crate) fn call_cost(provider: &AiProvider, model: &str, usage: &CallUsage) -> (f64, CostKind) {
    if let Some(cost) = usage.reported_cost_usd {
        return (cost, CostKind::Exact);
    }
    if *provider == AiProvider::Ollama {
        return (0.0, CostKind::Exact);
    }
    let Some(price) = price_of(provider, model) else {
        return (0.0, CostKind::Unknown);
    };
    let (read, write) = match provider {
        AiProvider::Anthropic => (usage.cache_read_tokens, usage.cache_write_tokens),
        _ => (0, 0),
    };
    let fresh = usage.input_tokens.saturating_sub(read + write) as f64;
    let input = fresh + read as f64 * ANTHROPIC_CACHE_READ_FACTOR + write as f64 * ANTHROPIC_CACHE_WRITE_FACTOR;
    let tokens = (input * price.input + usage.output_tokens as f64 * price.output) / TOKENS_PER_PRICE_UNIT;
    (tokens + f64::from(usage.web_searches) * price.per_search, CostKind::Estimated)
}

// ------------------------------------------------------------------- actions

/// One history item: a user action and everything its AI calls used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpendAction {
    pub id: String,
    #[serde(default)]
    pub feature: AiFeature,
    /// When the action began (ms since the epoch); its month is the one it counts in.
    pub at_ms: i64,
    /// Provider id ([`AiProvider::id`]).
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub calls: u32,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub web_searches: u32,
    /// USD; `None` when [`CostKind::Unknown`].
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub cost_kind: CostKind,
    /// Actions with the same group (the turns of one chat) are one item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

impl SpendAction {
    fn new(feature: AiFeature, group: Option<String>, at_ms: i64) -> Self {
        Self {
            id: crate::json_dir::new_id("ai"),
            feature,
            at_ms,
            provider: String::new(),
            model: String::new(),
            calls: 0,
            input_tokens: 0,
            output_tokens: 0,
            web_searches: 0,
            cost_usd: Some(0.0),
            cost_kind: CostKind::Exact,
            group,
        }
    }

    /// Adds one call.
    fn add_call(&mut self, config: &AiConfig, usage: &CallUsage) {
        let (cost, kind) = call_cost(&config.provider, &config.model, usage);
        self.provider = config.provider.id().to_owned();
        self.model = config.model.trim().chars().take(MAX_MODEL_CHARS).collect();
        self.calls = self.calls.saturating_add(1);
        self.input_tokens = self.input_tokens.saturating_add(usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(usage.output_tokens);
        self.web_searches = self.web_searches.saturating_add(usage.web_searches);
        self.add_cost(if kind == CostKind::Unknown { None } else { Some(cost) }, kind);
    }

    fn add_cost(&mut self, cost: Option<f64>, kind: CostKind) {
        self.cost_kind = self.cost_kind.and(kind);
        self.cost_usd = match (self.cost_kind, self.cost_usd, cost) {
            (CostKind::Unknown, ..) => None,
            (_, Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
    }

    /// Folds a later part of the same group in (another chat turn, the build after the chat).
    fn absorb(&mut self, later: SpendAction) {
        self.provider = later.provider;
        self.model = later.model;
        self.calls = self.calls.saturating_add(later.calls);
        self.input_tokens = self.input_tokens.saturating_add(later.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(later.output_tokens);
        self.web_searches = self.web_searches.saturating_add(later.web_searches);
        self.add_cost(later.cost_usd, later.cost_kind);
    }

    /// The cost when it is known.
    fn priced(&self) -> Option<f64> {
        self.cost_usd.filter(|_| self.cost_kind != CostKind::Unknown)
    }
}

// --------------------------------------------------------------------- guard

/// When to ask before an AI action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AskMode {
    /// Before every AI action.
    Always,
    /// When the action usually costs more than [`SpendGuard::ask_above_usd`] (or nobody knows yet).
    Above,
    /// Never ask (the default).
    #[default]
    #[serde(other)]
    Never,
}

/// "Ask before spending" and the monthly limit. Off by default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpendGuard {
    #[serde(default)]
    pub ask: AskMode,
    #[serde(default = "default_ask_above")]
    pub ask_above_usd: f64,
    /// Blocks AI actions once the month's spending reaches it; `None` is no limit.
    #[serde(default)]
    pub monthly_limit_usd: Option<f64>,
}

fn default_ask_above() -> f64 {
    DEFAULT_ASK_ABOVE_USD
}

impl Default for SpendGuard {
    fn default() -> Self {
        Self { ask: AskMode::Never, ask_above_usd: DEFAULT_ASK_ABOVE_USD, monthly_limit_usd: None }
    }
}

impl SpendGuard {
    /// Every number inside its range (a non-number falls back to the default).
    pub fn clamped(self) -> Self {
        let ask_above_usd = if self.ask_above_usd.is_finite() {
            self.ask_above_usd.clamp(MIN_ASK_ABOVE_USD, MAX_ASK_ABOVE_USD)
        } else {
            DEFAULT_ASK_ABOVE_USD
        };
        let monthly_limit_usd = self
            .monthly_limit_usd
            .filter(|l| l.is_finite() && *l > 0.0)
            .map(|l| l.clamp(MIN_MONTHLY_LIMIT_USD, MAX_MONTHLY_LIMIT_USD));
        Self { ask: self.ask, ask_above_usd, monthly_limit_usd }
    }
}

/// What the app does before an AI action.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpendDecision {
    Allow,
    /// Show a dialog with [`SpendPrompt`] and go on only on yes.
    Confirm {
        estimate_usd: Option<f64>,
        estimated: bool,
        month_total_usd: f64,
        prompt: SpendPrompt,
    },
    /// The monthly limit is reached: say so, don't start.
    Blocked {
        month_total_usd: f64,
        limit_usd: f64,
        prompt: SpendPrompt,
    },
}

/// The words of the confirmation (or of the "limit reached" notice), in the app's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpendPrompt {
    pub title: String,
    pub text: String,
}

// -------------------------------------------------------------------- ledger

#[derive(Debug, Default, Serialize, Deserialize)]
struct Book {
    /// Read leniently too: an unreadable guard is the default one.
    #[serde(default)]
    guard: Value,
    /// Read leniently: an unreadable item is dropped, not the whole history.
    #[serde(default)]
    actions: Vec<Value>,
}

/// The spending history and the guard, in one JSON file. Thread-safe; every change is written
/// atomically.
#[derive(Debug)]
pub struct Ledger {
    path: PathBuf,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    guard: SpendGuard,
    /// Oldest first.
    actions: Vec<SpendAction>,
}

impl Ledger {
    /// Loads `path` (a missing or broken file is an empty ledger; a broken one is kept until the
    /// next write).
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let book: Book = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut actions: Vec<SpendAction> =
            book.actions.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
        actions.sort_by_key(|a| a.at_ms);
        let state =
            State { guard: serde_json::from_value::<SpendGuard>(book.guard).unwrap_or_default().clamped(), actions };
        Self { path, state: Mutex::new(state) }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self, state: &State) -> std::io::Result<()> {
        let book = serde_json::json!({ "guard": state.guard, "actions": state.actions });
        let bytes = serde_json::to_vec(&book).map_err(std::io::Error::other)?;
        crate::paths::write_atomic(&self.path, &bytes)
    }

    /// Adds an action, or folds it into the item of its group.
    pub fn record(&self, action: SpendAction) {
        if action.calls == 0 {
            return;
        }
        let mut state = self.lock();
        let same_group =
            action.group.as_deref().and_then(|g| state.actions.iter().rposition(|a| a.group.as_deref() == Some(g)));
        match same_group {
            Some(at) => state.actions[at].absorb(action),
            None => state.actions.push(action),
        }
        let extra = state.actions.len().saturating_sub(MAX_ACTIONS);
        state.actions.drain(..extra);
        if let Err(e) = self.save(&state) {
            tracing::warn!("couldn't save the AI spending: {e}");
        }
    }

    pub fn guard(&self) -> SpendGuard {
        self.lock().guard
    }

    pub fn set_guard(&self, guard: SpendGuard) -> crate::Result<()> {
        let mut state = self.lock();
        state.guard = guard.clamped();
        Ok(self.save(&state)?)
    }

    /// Empties the history (the guard stays).
    pub fn clear(&self) -> crate::Result<()> {
        let mut state = self.lock();
        state.actions.clear();
        Ok(self.save(&state)?)
    }

    /// The month's total, the breakdown and the history, newest first.
    pub fn summary(&self, lang: Lang) -> SpendSummary {
        self.summary_at(lang, &Local::now())
    }

    pub fn summary_at<Tz: TimeZone>(&self, lang: Lang, now: &DateTime<Tz>) -> SpendSummary {
        let state = self.lock();
        summarize(&state.actions, state.guard, month_start_ms(now), lang)
    }

    /// The guard's answer before starting `feature`.
    pub fn check(&self, feature: AiFeature, lang: Lang) -> SpendDecision {
        self.check_at(feature, lang, &Local::now())
    }

    pub fn check_at<Tz: TimeZone>(&self, feature: AiFeature, lang: Lang, now: &DateTime<Tz>) -> SpendDecision {
        let state = self.lock();
        decide(&state.actions, state.guard, feature, month_start_ms(now), lang)
    }

    /// The monthly limit is set and reached.
    pub fn over_limit_at<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> bool {
        let state = self.lock();
        state.guard.monthly_limit_usd.is_some_and(|limit| month_total(&state.actions, month_start_ms(now)).0 >= limit)
    }
}

/// The first millisecond of `now`'s calendar month in its own time zone.
fn month_start_ms<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    now.timezone()
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .earliest()
        .map_or(0, |start| start.timestamp_millis())
}

/// The month's priced total, whether part of it is an estimate, and how many actions have no price.
fn month_total(actions: &[SpendAction], month_start_ms: i64) -> (f64, bool, u32) {
    actions.iter().filter(|a| a.at_ms >= month_start_ms).fold((0.0, false, 0), |(sum, est, unknown), a| {
        match a.priced() {
            Some(c) => (sum + c, est || a.cost_kind == CostKind::Estimated, unknown),
            None => (sum, est, unknown + 1),
        }
    })
}

/// The average price of the last [`ESTIMATE_SAMPLE`] priced actions of `feature`, and whether any
/// of them was an estimate.
fn feature_estimate(actions: &[SpendAction], feature: AiFeature) -> Option<(f64, bool)> {
    let recent: Vec<&SpendAction> =
        actions.iter().rev().filter(|a| a.feature == feature && a.priced().is_some()).take(ESTIMATE_SAMPLE).collect();
    if recent.is_empty() {
        return None;
    }
    let sum: f64 = recent.iter().filter_map(|a| a.priced()).sum();
    Some((sum / recent.len() as f64, recent.iter().any(|a| a.cost_kind == CostKind::Estimated)))
}

fn decide(
    actions: &[SpendAction],
    guard: SpendGuard,
    feature: AiFeature,
    month_start_ms: i64,
    lang: Lang,
) -> SpendDecision {
    let (month_total_usd, month_estimated, _) = month_total(actions, month_start_ms);
    if let Some(limit_usd) = guard.monthly_limit_usd.filter(|limit| month_total_usd >= *limit) {
        let prompt = words::blocked(lang, month_total_usd, limit_usd);
        return SpendDecision::Blocked { month_total_usd, limit_usd, prompt };
    }
    let estimate = feature_estimate(actions, feature);
    let ask = match guard.ask {
        AskMode::Never => false,
        AskMode::Always => true,
        // nobody knows yet what it costs: better to ask
        AskMode::Above => estimate.is_none_or(|(usd, _)| usd > guard.ask_above_usd),
    };
    if !ask {
        return SpendDecision::Allow;
    }
    let estimated = estimate.is_none_or(|(_, est)| est) || month_estimated;
    let estimate_usd = estimate.map(|(usd, _)| usd);
    let prompt = words::confirm(lang, feature, estimate_usd, month_total_usd, guard.monthly_limit_usd);
    SpendDecision::Confirm { estimate_usd, estimated, month_total_usd, prompt }
}

/// What the AI menu shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpendSummary {
    pub month_total_usd: f64,
    /// Part of the month's total is an estimate.
    pub month_estimated: bool,
    /// Actions this month whose cost is unknown (left out of the total).
    pub month_unknown: u32,
    pub month_actions: u32,
    /// "≈$1.20", ready to show.
    pub month_text: String,
    /// Every feature used, most spent first (over the whole kept history).
    pub features: Vec<FeatureSpend>,
    /// Newest first.
    pub history: Vec<SpendItem>,
    pub guard: SpendGuard,
}

/// One line of the breakdown.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeatureSpend {
    pub feature: AiFeature,
    pub label: String,
    pub times: u32,
    pub total_usd: f64,
    pub average_usd: Option<f64>,
    pub estimated: bool,
    /// "$0.12 in 4 times · ≈$0.03 each".
    pub text: String,
}

/// One history line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpendItem {
    pub id: String,
    pub feature: AiFeature,
    pub label: String,
    pub at_ms: i64,
    pub provider: String,
    pub model: String,
    pub calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub web_searches: u32,
    pub cost_usd: Option<f64>,
    pub cost_kind: CostKind,
    /// "≈$0.04", "$0.04" or "Cost unknown".
    pub cost_text: String,
    /// "claude-sonnet-5-5 · 3 calls · 12.3k tokens · 2 web searches".
    pub detail: String,
}

fn summarize(actions: &[SpendAction], guard: SpendGuard, month_start_ms: i64, lang: Lang) -> SpendSummary {
    let (month_total_usd, month_estimated, month_unknown) = month_total(actions, month_start_ms);
    let month_actions = actions.iter().filter(|a| a.at_ms >= month_start_ms).count() as u32;
    let mut features: Vec<FeatureSpend> = AiFeature::ALL
        .into_iter()
        .filter_map(|feature| {
            let mine: Vec<&SpendAction> = actions.iter().filter(|a| a.feature == feature).collect();
            if mine.is_empty() {
                return None;
            }
            let priced: Vec<f64> = mine.iter().filter_map(|a| a.priced()).collect();
            let total_usd: f64 = priced.iter().sum();
            let average_usd = (!priced.is_empty()).then(|| total_usd / priced.len() as f64);
            let estimated = mine.iter().any(|a| a.cost_kind == CostKind::Estimated);
            let times = mine.len() as u32;
            let text = words::feature_line(lang, times, total_usd, average_usd, estimated, priced.len() as u32);
            Some(FeatureSpend {
                feature,
                label: feature.label_in(lang).to_owned(),
                times,
                total_usd,
                average_usd,
                estimated,
                text,
            })
        })
        .collect();
    features.sort_by(|a, b| b.total_usd.total_cmp(&a.total_usd).then(b.times.cmp(&a.times)));
    let history = actions
        .iter()
        .rev()
        .map(|a| SpendItem {
            id: a.id.clone(),
            feature: a.feature,
            label: a.feature.label_in(lang).to_owned(),
            at_ms: a.at_ms,
            provider: a.provider.clone(),
            model: a.model.clone(),
            calls: a.calls,
            input_tokens: a.input_tokens,
            output_tokens: a.output_tokens,
            web_searches: a.web_searches,
            cost_usd: a.priced(),
            cost_kind: a.cost_kind,
            cost_text: words::cost(lang, a.priced(), a.cost_kind),
            detail: words::detail(lang, a),
        })
        .collect();
    SpendSummary {
        month_total_usd,
        month_estimated,
        month_unknown,
        month_actions,
        month_text: words::usd(month_total_usd, month_estimated),
        features,
        history,
        guard,
    }
}

// ----------------------------------------------------------- the app's ledger

static LEDGER: RwLock<Option<Arc<Ledger>>> = RwLock::new(None);

/// "$1.20" (no "≈"), as the spending screens write money.
pub fn usd_text(usd: f64) -> String {
    words::usd(usd, false)
}

/// Opens the app's ledger in `app_dir` (once at startup); until then nothing is recorded.
pub fn open(app_dir: &Path) -> Arc<Ledger> {
    let ledger = Arc::new(Ledger::open(app_dir.join(LEDGER_FILENAME)));
    *LEDGER.write().unwrap_or_else(PoisonError::into_inner) = Some(ledger.clone());
    ledger
}

/// The app's ledger, if [`open`]ed.
pub fn ledger() -> Option<Arc<Ledger>> {
    LEDGER.read().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Fails a call before it is sent when the monthly limit is reached (local servers are free).
pub(crate) fn ensure_allowed(config: &AiConfig) -> Result<(), AiError> {
    if config.provider.is_local() {
        return Ok(());
    }
    match ledger() {
        Some(ledger) if ledger.over_limit_at(&Local::now()) => {
            Err(AiError::new(config, AiErrorKind::SpendLimit, "monthly AI spending limit reached"))
        }
        _ => Ok(()),
    }
}

// ------------------------------------------------------------------- tracking

tokio::task_local! {
    static ACTION: Arc<Mutex<SpendAction>>;
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Runs `work` as one user action of `feature`: every AI call inside adds to one history item,
/// recorded when it ends (finished, failed or cancelled). Inside another [`track`] it joins that
/// one. `group` folds actions together (the turns of one chat); [`new_group`] makes one.
pub async fn track<F: Future>(feature: AiFeature, group: Option<&str>, work: F) -> F::Output {
    if ACTION.try_with(|_| ()).is_ok() {
        return work.await;
    }
    let action = Arc::new(Mutex::new(SpendAction::new(feature, group.map(str::to_owned), now_ms())));
    let _record = RecordOnDrop(action.clone());
    ACTION.scope(action, work).await
}

/// An id that ties several actions into one history item.
pub fn new_group() -> String {
    crate::json_dir::new_id("g")
}

/// Records the action however its future ends.
struct RecordOnDrop(Arc<Mutex<SpendAction>>);

impl Drop for RecordOnDrop {
    fn drop(&mut self) {
        let action = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(ledger) = ledger() {
            ledger.record(action);
        }
    }
}

/// Notes one answer of `api` into the running action (or, outside any, as an untagged one).
pub(crate) fn note(config: &AiConfig, api: Api, answer: &Value, searched: bool) {
    let usage = CallUsage::read(api, answer, searched);
    let in_action = ACTION.try_with(|action| {
        action.lock().unwrap_or_else(PoisonError::into_inner).add_call(config, &usage);
    });
    if in_action.is_err() {
        tracing::debug!("AI call outside a tracked action");
        let mut action = SpendAction::new(AiFeature::Other, None, now_ms());
        action.add_call(config, &usage);
        if let Some(ledger) = ledger() {
            ledger.record(action);
        }
    }
}

/// [`track`] that hands back the action instead of recording it (tests).
#[cfg(test)]
pub(crate) async fn tracked<F: Future>(feature: AiFeature, work: F) -> (F::Output, SpendAction) {
    let action = Arc::new(Mutex::new(SpendAction::new(feature, None, now_ms())));
    let out = ACTION.scope(action.clone(), work).await;
    let action = action.lock().unwrap_or_else(PoisonError::into_inner).clone();
    (out, action)
}

// --------------------------------------------------------------------- words

mod words {
    use super::{AiFeature, CostKind, SpendAction, SpendPrompt};
    use crate::humane::Lang;

    /// "$0.04", "$0.004", "<$0.001"; "≈" in front of an estimate.
    pub(super) fn usd(value: f64, estimated: bool) -> String {
        let approx = if estimated && value > 0.0 { "≈" } else { "" };
        let amount = if value <= 0.0 {
            "$0.00".to_owned()
        } else if value < 0.001 {
            "<$0.001".to_owned()
        } else if value < 0.01 {
            format!("${value:.3}")
        } else {
            format!("${value:.2}")
        };
        format!("{approx}{amount}")
    }

    pub(super) fn cost(lang: Lang, cost: Option<f64>, kind: CostKind) -> String {
        match (cost, lang) {
            (Some(c), _) => usd(c, kind == CostKind::Estimated),
            (None, Lang::Es) => "Costo desconocido".into(),
            (None, Lang::En) => "Cost unknown".into(),
        }
    }

    /// 950, "12.3k", "1.2M" (Spanish with a decimal comma).
    fn tokens(n: u64, lang: Lang) -> String {
        let text = match n {
            0..=999 => n.to_string(),
            1_000..=999_999 => format!("{:.1}k", n as f64 / 1_000.0),
            _ => format!("{:.1}M", n as f64 / 1_000_000.0),
        };
        if lang == Lang::Es {
            text.replace('.', ",")
        } else {
            text
        }
    }

    pub(super) fn detail(lang: Lang, a: &SpendAction) -> String {
        let es = lang == Lang::Es;
        let mut parts = Vec::with_capacity(4);
        if !a.model.is_empty() {
            parts.push(a.model.clone());
        }
        parts.push(match (a.calls, es) {
            (1, true) => "1 llamada".to_owned(),
            (1, false) => "1 call".to_owned(),
            (n, true) => format!("{n} llamadas"),
            (n, false) => format!("{n} calls"),
        });
        parts.push(format!("{} tokens", tokens(a.input_tokens + a.output_tokens, lang)));
        match (a.web_searches, es) {
            (0, _) => {}
            (1, true) => parts.push("1 búsqueda web".into()),
            (1, false) => parts.push("1 web search".into()),
            (n, true) => parts.push(format!("{n} búsquedas web")),
            (n, false) => parts.push(format!("{n} web searches")),
        }
        parts.join(" · ")
    }

    pub(super) fn feature_line(
        lang: Lang,
        times: u32,
        total: f64,
        average: Option<f64>,
        estimated: bool,
        priced: u32,
    ) -> String {
        let es = lang == Lang::Es;
        let times_text = match (times, es) {
            (1, true) => "1 vez".to_owned(),
            (1, false) => "once".to_owned(),
            (n, true) => format!("{n} veces"),
            (n, false) => format!("{n} times"),
        };
        let Some(average) = average else {
            return if es {
                format!("{times_text} · costo desconocido")
            } else {
                format!("{times_text} · cost unknown")
            };
        };
        let missing = times.saturating_sub(priced);
        let mut text = if es {
            format!("{} en {times_text} · {} cada una", usd(total, estimated), usd(average, true))
        } else {
            format!("{} in {times_text} · {} each", usd(total, estimated), usd(average, true))
        };
        if missing > 0 {
            text.push_str(&if es {
                format!(" ({missing} sin precio)")
            } else {
                format!(" ({missing} without a price)")
            });
        }
        text
    }

    pub(super) fn confirm(
        lang: Lang,
        feature: AiFeature,
        estimate: Option<f64>,
        month: f64,
        limit: Option<f64>,
    ) -> SpendPrompt {
        let es = lang == Lang::Es;
        let what = feature.label_in(lang);
        let cost = match (estimate, es) {
            (Some(e), true) => format!("Esto suele costar {}.", usd(e, true)),
            (Some(e), false) => format!("This usually costs {}.", usd(e, true)),
            (None, true) => "Aún no sabemos cuánto cuesta: es la primera vez o el modelo no publica su precio.".into(),
            (None, false) => {
                "We don't know yet what this costs: it's the first time, or the model doesn't publish its price.".into()
            }
        };
        let so_far = match (limit, es) {
            (Some(l), true) => format!("Llevas {} este mes, de un límite de {}.", usd(month, false), usd(l, false)),
            (Some(l), false) => format!("You've spent {} this month, of a {} limit.", usd(month, false), usd(l, false)),
            (None, true) => format!("Llevas {} este mes.", usd(month, false)),
            (None, false) => format!("You've spent {} this month.", usd(month, false)),
        };
        SpendPrompt {
            title: if es { format!("¿Usar la IA? ({what})") } else { format!("Use the AI? ({what})") },
            text: format!("{cost} {so_far} {}", if es { "¿Continuar?" } else { "Continue?" }),
        }
    }

    pub(super) fn blocked(lang: Lang, month: f64, limit: f64) -> SpendPrompt {
        if lang == Lang::Es {
            SpendPrompt {
                title: "Llegaste a tu límite de este mes".into(),
                text: format!(
                    "Llevas {} en IA este mes y tu límite es {}. Así no se te va todo el saldo. Puedes subirlo o \
                     quitarlo en Ajustes → IA, o esperar al próximo mes.",
                    usd(month, false),
                    usd(limit, false)
                ),
            }
        } else {
            SpendPrompt {
                title: "You reached this month's limit".into(),
                text: format!(
                    "You've spent {} on AI this month and your limit is {}, so your balance is safe. You can raise \
                     or remove it in Settings → AI, or wait for next month.",
                    usd(month, false),
                    usd(limit, false)
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};
    use serde_json::json;

    fn action(feature: AiFeature, at_ms: i64, cost: Option<f64>, kind: CostKind) -> SpendAction {
        SpendAction {
            calls: 1,
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: cost,
            cost_kind: kind,
            ..SpendAction::new(feature, None, at_ms)
        }
    }

    fn utc(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    fn ledger_in(dir: &tempfile::TempDir) -> Ledger {
        Ledger::open(dir.path().join(LEDGER_FILENAME))
    }

    #[test]
    fn usage_is_read_from_every_api() {
        let anthropic = json!({"usage": {"input_tokens": 10, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 200,
            "output_tokens": 40, "server_tool_use": {"web_search_requests": 3}}});
        let u = CallUsage::read(Api::Anthropic, &anthropic, true);
        assert_eq!(
            (u.input_tokens, u.cache_read_tokens, u.cache_write_tokens, u.output_tokens, u.web_searches),
            (1210, 1000, 200, 40, 3)
        );
        let responses = json!({"usage": {"input_tokens": 500, "output_tokens": 70},
            "output": [{"type": "web_search_call"}, {"type": "web_search_call"}, {"type": "message"}]});
        let u = CallUsage::read(Api::OpenAiResponses, &responses, true);
        assert_eq!((u.input_tokens, u.output_tokens, u.web_searches), (500, 70, 2));
        let chat = json!({"usage": {"prompt_tokens": 9, "completion_tokens": 3, "cost": 0.00042}});
        let u = CallUsage::read(Api::ChatCompletions, &chat, false);
        assert_eq!((u.input_tokens, u.output_tokens, u.web_searches, u.reported_cost_usd), (9, 3, 0, Some(0.00042)));
        let gemini = json!({"usageMetadata": {"promptTokenCount": 20, "candidatesTokenCount": 5, "thoughtsTokenCount": 7},
            "candidates": [{"groundingMetadata": {}}]});
        let u = CallUsage::read(Api::Gemini, &gemini, true);
        assert_eq!((u.input_tokens, u.output_tokens, u.web_searches), (20, 12, 1));
        // no usage at all: zeros, not a failure
        assert_eq!(CallUsage::read(Api::ChatCompletions, &json!({}), false), CallUsage::default());
    }

    #[test]
    fn model_ids_lose_vendor_dates_and_latest() {
        assert_eq!(plain_model("anthropic/claude-sonnet-4-5-20250929"), "claude-sonnet-4-5");
        assert_eq!(plain_model("gpt-5-mini-2025-08-07"), "gpt-5-mini");
        assert_eq!(plain_model("Gemini-2.5-Flash"), "gemini-2.5-flash");
        assert_eq!(plain_model("mistral-medium-latest"), "mistral-medium");
        assert_eq!(plain_model("gpt-4.1"), "gpt-4.1");
    }

    #[test]
    fn estimates_use_the_list_price_and_unknown_models_stay_unknown() {
        let usage = CallUsage { input_tokens: 1_000_000, output_tokens: 100_000, ..CallUsage::default() };
        let (cost, kind) = call_cost(&AiProvider::OpenAi, "gpt-5-mini", &usage);
        assert_eq!(kind, CostKind::Estimated);
        assert!((cost - (0.25 + 0.2)).abs() < 1e-9, "{cost}");
        // the Sonnet family, any version
        let (cost, _) = call_cost(&AiProvider::Anthropic, "claude-sonnet-5-5", &usage);
        assert!((cost - (3.0 + 1.5)).abs() < 1e-9, "{cost}");
        // Anthropic's cache: reads at a tenth, writes at 1.25x
        let cached = CallUsage {
            input_tokens: 1_000_000,
            cache_read_tokens: 500_000,
            cache_write_tokens: 100_000,
            ..CallUsage::default()
        };
        let (cost, _) = call_cost(&AiProvider::Anthropic, "claude-sonnet-5-5", &cached);
        assert!((cost - 3.0 * (0.4 + 0.05 + 0.125)).abs() < 1e-9, "{cost}");
        // searches
        let searched = CallUsage { web_searches: 2, ..CallUsage::default() };
        assert!((call_cost(&AiProvider::Anthropic, "claude-sonnet-5-5", &searched).0 - 0.02).abs() < 1e-9);
        // never invented
        assert_eq!(call_cost(&AiProvider::Mistral, "mistral-medium-latest", &usage).1, CostKind::Unknown);
        assert_eq!(call_cost(&AiProvider::Groq, "gpt-5", &usage).1, CostKind::Unknown);
        assert_eq!(call_cost(&AiProvider::OpenAi, "gpt-9-ultra", &usage).1, CostKind::Unknown);
        assert_eq!(
            call_cost(&AiProvider::OpenAiCompatible { base_url: "x".into() }, "gpt-5", &usage).1,
            CostKind::Unknown
        );
        // free and exact
        assert_eq!(call_cost(&AiProvider::Ollama, "qwen3:8b", &usage), (0.0, CostKind::Exact));
        // the provider's own number wins
        let reported = CallUsage { reported_cost_usd: Some(0.0123), ..usage };
        assert_eq!(call_cost(&AiProvider::OpenRouter, "x/y", &reported), (0.0123, CostKind::Exact));
    }

    #[test]
    fn an_action_adds_its_calls_and_any_unknown_makes_it_unknown() {
        let config = AiConfig::new(AiProvider::OpenRouter, "anthropic/claude-x", None);
        let mut a = SpendAction::new(AiFeature::Concert, None, 0);
        a.add_call(
            &config,
            &CallUsage { input_tokens: 10, output_tokens: 5, reported_cost_usd: Some(0.01), ..Default::default() },
        );
        a.add_call(
            &config,
            &CallUsage {
                input_tokens: 20,
                output_tokens: 5,
                web_searches: 1,
                reported_cost_usd: Some(0.02),
                ..Default::default()
            },
        );
        assert_eq!(
            (a.calls, a.input_tokens, a.output_tokens, a.web_searches, a.cost_kind),
            (2, 30, 10, 1, CostKind::Exact)
        );
        assert!((a.cost_usd.unwrap() - 0.03).abs() < 1e-9);
        let gpt = AiConfig::new(AiProvider::OpenAi, "gpt-5-mini", None);
        a.add_call(&gpt, &CallUsage { input_tokens: 1000, ..Default::default() });
        assert_eq!(a.cost_kind, CostKind::Estimated);
        let unknown = AiConfig::new(AiProvider::Mistral, "mistral-small", None);
        a.add_call(&unknown, &CallUsage { input_tokens: 1000, ..Default::default() });
        assert_eq!((a.cost_kind, a.cost_usd, a.calls), (CostKind::Unknown, None, 4));
    }

    #[test]
    fn the_ledger_groups_bounds_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ledger_in(&dir);
        // two turns of one chat are one item
        let mut first = action(AiFeature::PartyPlaylist, 1, Some(0.01), CostKind::Exact);
        first.group = Some("g1".into());
        let mut second = action(AiFeature::PartyPlaylist, 2, Some(0.02), CostKind::Exact);
        second.group = Some("g1".into());
        ledger.record(first);
        ledger.record(second);
        // an action without calls is nothing
        ledger.record(SpendAction::new(AiFeature::Concert, None, 3));
        let summary = ledger.summary_at(Lang::En, &utc(1970, 1, 1));
        assert_eq!(summary.history.len(), 1);
        assert_eq!(summary.history[0].calls, 2);
        assert!((summary.history[0].cost_usd.unwrap() - 0.03).abs() < 1e-9);
        // bounded, oldest out
        for i in 0..(MAX_ACTIONS as i64 + 20) {
            ledger.record(action(AiFeature::SoundReaction, 10 + i, Some(0.001), CostKind::Exact));
        }
        let again = ledger_in(&dir);
        let history = again.summary_at(Lang::En, &utc(1970, 1, 1)).history;
        assert_eq!(history.len(), MAX_ACTIONS);
        assert!(history.iter().all(|h| h.feature == AiFeature::SoundReaction), "the oldest went first");
        assert_eq!(history[0].at_ms, 10 + MAX_ACTIONS as i64 + 19, "newest first");
        // nothing but numbers and names is stored
        let text = std::fs::read_to_string(dir.path().join(LEDGER_FILENAME)).unwrap();
        assert!(!text.contains("prompt") && !text.contains("key"));
        again.clear().unwrap();
        assert!(ledger_in(&dir).summary_at(Lang::En, &utc(1970, 1, 1)).history.is_empty());
    }

    #[test]
    fn a_broken_item_or_file_does_not_lose_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LEDGER_FILENAME);
        std::fs::write(&path, r#"{"guard":{"ask":"always","ask_above_usd":"x"},"actions":[{"id":"a","at_ms":5,"feature":"concert","calls":1,"cost_usd":0.5,"cost_kind":"exact"},{"nope":1},{"id":"b","at_ms":6,"feature":"new_thing","calls":1}]}"#).unwrap();
        let ledger = Ledger::open(&path);
        let s = ledger.summary_at(Lang::En, &utc(1970, 1, 1));
        assert_eq!(s.history.len(), 2);
        assert_eq!(s.history[0].feature, AiFeature::Other);
        assert_eq!(s.history[0].cost_kind, CostKind::Unknown);
        // the guard falls back as a whole when a field is unreadable
        assert_eq!(s.guard, SpendGuard::default());
        std::fs::write(&path, "not json").unwrap();
        assert!(Ledger::open(&path).summary_at(Lang::En, &utc(1970, 1, 1)).history.is_empty());
    }

    #[test]
    fn the_month_rolls_over_on_the_first_in_local_time() {
        let actions = vec![
            action(AiFeature::Concert, utc(2026, 10, 31).timestamp_millis(), Some(1.0), CostKind::Exact),
            action(AiFeature::Concert, utc(2026, 11, 2).timestamp_millis(), Some(0.25), CostKind::Estimated),
            action(AiFeature::Concert, utc(2026, 11, 3).timestamp_millis(), None, CostKind::Unknown),
        ];
        let (total, estimated, unknown) = month_total(&actions, month_start_ms(&utc(2026, 11, 16)));
        assert!((total - 0.25).abs() < 1e-9);
        assert!(estimated);
        assert_eq!(unknown, 1);
        assert_eq!(month_total(&actions, month_start_ms(&utc(2026, 12, 1))).0, 0.0);
        // midnight of the 1st where the user lives, not in UTC
        let mexico = FixedOffset::west_opt(6 * 3600).unwrap();
        let now = mexico.with_ymd_and_hms(2026, 11, 1, 0, 30, 0).unwrap();
        assert_eq!(month_start_ms(&now), mexico.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap().timestamp_millis());
    }

    #[test]
    fn the_guard_allows_asks_or_blocks() {
        let now = utc(2026, 11, 16);
        let start = month_start_ms(&now);
        let at = utc(2026, 11, 10).timestamp_millis();
        let past = vec![
            action(AiFeature::Concert, at, Some(0.04), CostKind::Exact),
            action(AiFeature::Concert, at + 1, Some(0.08), CostKind::Exact),
        ];
        let guard = |ask, above, limit| SpendGuard { ask, ask_above_usd: above, monthly_limit_usd: limit };
        // off by default
        assert_eq!(decide(&past, SpendGuard::default(), AiFeature::Concert, start, Lang::En), SpendDecision::Allow);
        // always: the average of the same feature and the month so far
        match decide(&past, guard(AskMode::Always, 1.0, None), AiFeature::Concert, start, Lang::Es) {
            SpendDecision::Confirm { estimate_usd: Some(e), estimated: false, month_total_usd, prompt } => {
                assert!((e - 0.06).abs() < 1e-9 && (month_total_usd - 0.12).abs() < 1e-9);
                assert!(prompt.text.contains("≈$0.06") && prompt.text.contains("$0.12"), "{}", prompt.text);
            }
            other => panic!("{other:?}"),
        }
        // above: only when it usually costs more, or nobody knows yet
        assert_eq!(
            decide(&past, guard(AskMode::Above, 0.10, None), AiFeature::Concert, start, Lang::En),
            SpendDecision::Allow
        );
        assert!(matches!(
            decide(&past, guard(AskMode::Above, 0.05, None), AiFeature::Concert, start, Lang::En),
            SpendDecision::Confirm { .. }
        ));
        assert!(matches!(
            decide(&past, guard(AskMode::Above, 0.05, None), AiFeature::RideBuild, start, Lang::En),
            SpendDecision::Confirm { estimate_usd: None, .. }
        ));
        // the limit blocks every feature, even with asking off
        match decide(&past, guard(AskMode::Never, 1.0, Some(0.12)), AiFeature::SoundReaction, start, Lang::Es) {
            SpendDecision::Blocked { limit_usd, prompt, .. } => {
                assert_eq!(limit_usd, 0.12);
                assert_eq!(prompt.title, "Llegaste a tu límite de este mes");
            }
            other => panic!("{other:?}"),
        }
        // and lifts next month
        let december = month_start_ms(&utc(2026, 12, 1));
        assert_eq!(
            decide(&past, guard(AskMode::Never, 1.0, Some(0.12)), AiFeature::Concert, december, Lang::En),
            SpendDecision::Allow
        );
    }

    #[test]
    fn the_ledger_enforces_its_limit() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = ledger_in(&dir);
        let now = utc(2026, 11, 16);
        ledger.record(action(AiFeature::Concert, utc(2026, 11, 2).timestamp_millis(), Some(2.0), CostKind::Exact));
        assert!(!ledger.over_limit_at(&now));
        ledger.set_guard(SpendGuard { monthly_limit_usd: Some(1.5), ..SpendGuard::default() }).unwrap();
        assert!(ledger.over_limit_at(&now));
        assert!(matches!(ledger.check_at(AiFeature::Concert, Lang::En, &now), SpendDecision::Blocked { .. }));
        // the guard is kept with the history
        assert_eq!(ledger_in(&dir).guard().monthly_limit_usd, Some(1.5));
        assert!(!ledger.over_limit_at(&utc(2026, 12, 1)));
    }

    #[test]
    fn the_guard_numbers_stay_in_range() {
        let g = SpendGuard { ask: AskMode::Above, ask_above_usd: f64::NAN, monthly_limit_usd: Some(-3.0) }.clamped();
        assert_eq!((g.ask_above_usd, g.monthly_limit_usd), (DEFAULT_ASK_ABOVE_USD, None));
        let g = SpendGuard { ask: AskMode::Above, ask_above_usd: 1e9, monthly_limit_usd: Some(0.0001) }.clamped();
        assert_eq!((g.ask_above_usd, g.monthly_limit_usd), (MAX_ASK_ABOVE_USD, Some(MIN_MONTHLY_LIMIT_USD)));
    }

    #[test]
    fn the_breakdown_says_how_much_each_thing_costs() {
        let at = utc(2026, 11, 10).timestamp_millis();
        let actions = vec![
            action(AiFeature::PartyPlaylist, at, Some(0.03), CostKind::Estimated),
            action(AiFeature::PartyPlaylist, at + 1, Some(0.05), CostKind::Estimated),
            action(AiFeature::SoundReaction, at + 2, None, CostKind::Unknown),
        ];
        let s = summarize(&actions, SpendGuard::default(), month_start_ms(&utc(2026, 11, 16)), Lang::Es);
        assert_eq!(s.features[0].label, "Listas de fiesta");
        assert_eq!(s.features[0].text, "≈$0.08 en 2 veces · ≈$0.04 cada una");
        assert_eq!(s.features[1].text, "1 vez · costo desconocido");
        assert_eq!((s.month_text.as_str(), s.month_unknown, s.month_actions), ("≈$0.08", 1, 3));
        assert_eq!(s.history[0].cost_text, "Costo desconocido");
        assert_eq!(s.history[1].cost_text, "≈$0.05");
        assert_eq!(s.history[1].detail, "1 llamada · 150 tokens");
        assert_eq!(words::usd(0.0042, false), "$0.004");
        assert_eq!(words::usd(0.0, true), "$0.00");
    }

    #[test]
    fn feature_ids_round_trip() {
        for f in AiFeature::ALL {
            assert_eq!(AiFeature::from_id(f.id()), f);
            assert_eq!(serde_json::to_value(f).unwrap(), f.id());
        }
        assert_eq!(AiFeature::from_id("nope"), AiFeature::Other);
    }

    #[tokio::test]
    async fn nested_actions_join_the_outer_one() {
        let config = AiConfig::new(AiProvider::OpenRouter, "m", None);
        let answer = json!({"usage": {"prompt_tokens": 1, "completion_tokens": 1, "cost": 0.5}});
        let ((), action) = tracked(AiFeature::RideSwap, async {
            note(&config, Api::ChatCompletions, &answer, false);
            track(AiFeature::RideBuild, None, async { note(&config, Api::ChatCompletions, &answer, false) }).await;
        })
        .await;
        assert_eq!((action.feature, action.calls, action.cost_usd), (AiFeature::RideSwap, 2, Some(1.0)));
    }
}
