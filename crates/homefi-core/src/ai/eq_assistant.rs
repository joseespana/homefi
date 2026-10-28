//! The EQ assistant: a short, friendly interview (in the listener's language)
//! that ends in an equalizer profile for one of their outputs.
//!
//! The model can look headphones up in AutoEq, read their correction profile,
//! propose a profile (validated here: values in range, boosts capped at
//! [`MAX_BOOST_DB`], automatic preamp so nothing clips) and apply it, but only
//! after the user said yes. Every turn is an [`AssistantTurn`]; the guided
//! wizard without AI ([`super::GuidedEq`]) answers with the same type, so the
//! apps share one screen.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::chat::{chat, Message, Tool, ToolCall, ToolResult};
use super::guided::{GuidedEq, GuidedState};
use super::{resolve_model, AiConfig};
use crate::dsp::autoeq::{AutoEq, HeadphoneEntry};
use crate::dsp::{
    auto_preamp_db, safe_tilt, with_taste_for, EqBand, EqBandKind, EqProfile, EqSettings, EqSource, ItemRef,
    LoudnessMode, LoudnessSettings, NowPlayingContext, TasteCreator, TasteRule, TasteScope, TasteSettings, Tilt,
};
use crate::error::Result;

/// No band may boost more than this (dB), whatever the model asks.
pub const MAX_BOOST_DB: f32 = 12.0;
/// Each taste adjustment stays within ± this (dB).
pub const MAX_TASTE_DB: f32 = crate::dsp::MAX_TILT_DB;
/// Model calls per user message (tool rounds included).
const MAX_ROUNDS: usize = 6;
/// Questions the assistant should ask at most.
const MAX_QUESTIONS: u32 = 6;
/// Headphones returned to the model per search.
const SEARCH_RESULTS: usize = 8;
/// Tap-able answers per question.
const MAX_QUICK_REPLIES: usize = 4;

/// What kind of output a device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    UsbDac,
    /// Wired headphones (the jack).
    Headphones,
    Bluetooth,
    /// The device's own speakers.
    BuiltIn,
    Car,
    Hdmi,
    #[default]
    Other,
}

/// One of the user's audio outputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioOutput {
    /// The key of [`EqSettings::per_output`] (the desktop uses the device name).
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: OutputKind,
    #[serde(default)]
    pub sample_rates: Vec<u32>,
}

/// What the user listens to most (the app sums it up from its library and
/// the Tidal account), so the assistant can offer taste layers that matter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ListeningSummary {
    /// Most played / owned first.
    pub top_genres: Vec<String>,
    pub top_artists: Vec<ItemRef>,
    pub playlists: Vec<ItemRef>,
}

impl ListeningSummary {
    pub fn is_empty(&self) -> bool {
        self.top_genres.is_empty() && self.top_artists.is_empty() && self.playlists.is_empty()
    }
}

/// What the assistant knows before the first question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssistantContext {
    /// `es`, `en`, `es-MX`... (anything not Spanish is English).
    pub language: String,
    pub outputs: Vec<AudioOutput>,
    #[serde(default)]
    pub current: EqSettings,
    /// Anything the user wrote down about how they listen.
    #[serde(default)]
    pub notes: Option<String>,
    /// The taste rules as they are (the assistant edits a copy).
    #[serde(default)]
    pub taste: TasteSettings,
    #[serde(default)]
    pub loudness: LoudnessSettings,
    #[serde(default)]
    pub listening: Option<ListeningSummary>,
    #[serde(default)]
    pub now_playing: Option<NowPlayingContext>,
    /// `id` of the output playing now.
    #[serde(default)]
    pub active_output: Option<String>,
}

impl AssistantContext {
    pub(crate) fn spanish(&self) -> bool {
        self.language.to_lowercase().starts_with("es")
    }

    pub(crate) fn output(&self, id: &str) -> Option<&AudioOutput> {
        self.outputs
            .iter()
            .find(|o| o.id == id)
            .or_else(|| self.outputs.iter().find(|o| o.name.eq_ignore_ascii_case(id)))
    }

    /// The music plays on a built-in / phone / laptop speaker: no deep bass.
    pub(crate) fn on_small_speaker(&self) -> bool {
        let active = self.active_output.as_deref().and_then(|id| self.output(id));
        match active {
            Some(o) => o.kind == OutputKind::BuiltIn,
            None => !self.outputs.is_empty() && self.outputs.iter().all(|o| o.kind == OutputKind::BuiltIn),
        }
    }

    pub(crate) fn top_genre(&self) -> Option<&str> {
        let listened =
            self.listening.as_ref().and_then(|l| l.top_genres.iter().map(|g| g.trim()).find(|g| !g.is_empty()));
        listened.or_else(|| self.now_playing.as_ref().and_then(|n| n.main_genre()))
    }
}

/// A profile ready to apply to an output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqProposal {
    pub output_id: String,
    pub profile: EqProfile,
    /// Why, in plain words and the user's language.
    pub explanation: String,
}

/// One answer of the assistant (or of the guided wizard).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssistantTurn {
    pub text: String,
    /// Short answers to offer as buttons (the user may still type).
    pub quick_replies: Vec<String>,
    /// The profile on the table, to preview (and to apply when `confirmed`).
    pub proposal: Option<EqProposal>,
    /// The user said yes: save `proposal` for its output and turn the EQ on.
    pub confirmed: bool,
    /// The conversation is over.
    pub done: bool,
    /// Taste rules changed this turn (the user agreed): save and apply them.
    #[serde(default)]
    pub taste: Option<TasteSettings>,
    /// Loudness changed this turn: save and apply it.
    #[serde(default)]
    pub loudness: Option<LoudnessSettings>,
}

/// Everything needed to resume a conversation later (no key, no model).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssistantState {
    pub context: AssistantContext,
    pub messages: Vec<Message>,
    /// Headphones of the last search, so the model can refer to them by path.
    #[serde(default)]
    pub found: Vec<HeadphoneEntry>,
    #[serde(default)]
    pub proposal: Option<EqProposal>,
    /// User messages so far (the hidden kick-off not counted).
    #[serde(default)]
    pub user_turns: u32,
    /// `user_turns` when the proposal was made: applying needs a later "yes".
    #[serde(default)]
    pub proposed_at: u32,
    #[serde(default)]
    pub confirmed: bool,
}

/// A line of the conversation as the user saw it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptLine {
    pub from_user: bool,
    pub text: String,
}

impl AssistantState {
    /// The visible conversation, to redraw it when resuming.
    pub fn transcript(&self) -> Vec<TranscriptLine> {
        self.messages
            .iter()
            .skip(1) // the kick-off
            .filter_map(|m| match m {
                Message::User { text } => Some(TranscriptLine { from_user: true, text: text.clone() }),
                Message::Assistant { text, .. } if !text.is_empty() => {
                    Some(TranscriptLine { from_user: false, text: text.clone() })
                }
                _ => None,
            })
            .collect()
    }
}

/// The AI-driven EQ interview.
pub struct EqAssistant {
    config: AiConfig,
    state: AssistantState,
    autoeq: Arc<AutoEq>,
}

impl EqAssistant {
    /// A new conversation. AutoEq profiles are cached in `<app dir>/autoeq`
    /// unless [`with_autoeq`](Self::with_autoeq) says otherwise.
    pub fn new(config: AiConfig, context: AssistantContext) -> Self {
        Self::resume(config, AssistantState { context, ..Default::default() })
    }

    /// Continues a conversation saved with [`state`](Self::state).
    pub fn resume(config: AiConfig, state: AssistantState) -> Self {
        Self { config, state, autoeq: Arc::new(AutoEq::new(crate::paths::app_dir().join("autoeq"))) }
    }

    pub fn with_autoeq(mut self, autoeq: Arc<AutoEq>) -> Self {
        self.autoeq = autoeq;
        self
    }

    pub fn state(&self) -> &AssistantState {
        &self.state
    }

    /// The first question.
    pub async fn start(&mut self) -> Result<AssistantTurn> {
        let kickoff = if self.state.context.spanish() {
            "(El usuario abrió el asistente del ecualizador. Salúdalo en una frase y haz la primera pregunta.)"
        } else {
            "(The user opened the equalizer assistant. Greet them in one sentence and ask the first question.)"
        };
        self.state.messages = vec![Message::User { text: kickoff.to_owned() }];
        self.state.user_turns = 0;
        self.run().await
    }

    /// A proactive suggestion for a card (one taste layer that fits what
    /// they listen to, with yes / no replies). The conversation goes on with
    /// [`reply`](Self::reply) when the user taps an answer.
    pub async fn suggest(&mut self) -> Result<AssistantTurn> {
        let kickoff = if self.state.context.spanish() {
            "(La app muestra una tarjeta de sugerencia. Sin saludar, propón en una o dos frases UNA capa de gusto concreta \
             según lo que escucha (listening_summary, lo que suena ahora y sus reglas), y llama suggest_replies con un sí y un no.)"
        } else {
            "(The app shows a suggestion card. Without greeting, propose in one or two sentences ONE concrete taste layer \
             for what they listen to (listening_summary, what plays now and their rules), and call suggest_replies with a yes and a no.)"
        };
        self.state.messages = vec![Message::User { text: kickoff.to_owned() }];
        self.state.user_turns = 0;
        self.run().await
    }

    /// The user's answer (typed, or one of the quick replies).
    pub async fn reply(&mut self, text: &str) -> Result<AssistantTurn> {
        if self.state.messages.is_empty() {
            return self.start().await;
        }
        self.state.messages.push(Message::User { text: text.trim().to_owned() });
        self.state.user_turns += 1;
        self.state.confirmed = false;
        self.run().await
    }

    async fn run(&mut self) -> Result<AssistantTurn> {
        if self.config.model.trim().is_empty() {
            self.config.model = resolve_model(&self.config).await?;
        }
        let tools = tools();
        let mut turn = AssistantTurn::default();
        let mut texts: Vec<String> = Vec::new();
        for _ in 0..MAX_ROUNDS {
            let system = system_prompt(&self.state);
            let reply = match chat(&self.config, &system, &self.state.messages, &tools).await {
                Ok(r) => r,
                Err(e) => {
                    // the user's words stay out of the history, so "try again" resends them cleanly
                    self.drop_unanswered_tail();
                    return Err(e);
                }
            };
            if !reply.text.is_empty() {
                texts.push(reply.text.clone());
            }
            let calls = reply.tool_calls.clone();
            self.state.messages.push(reply.into_message());
            if calls.is_empty() {
                break;
            }
            let mut results = Vec::new();
            let mut needs_model = false;
            for call in &calls {
                let (content, is_error, terminal) = self.run_tool(call, &mut turn).await;
                needs_model |= !terminal || is_error;
                results.push(ToolResult { call_id: call.id.clone(), name: call.name.clone(), content, is_error });
            }
            self.state.messages.push(Message::Tool { results });
            // only quick replies: the question is asked, wait for the user
            if !needs_model {
                break;
            }
        }
        turn.text = texts.join("\n\n");
        if turn.text.is_empty() {
            turn.text = if self.state.context.spanish() { "¿Seguimos?" } else { "Shall we go on?" }.to_owned();
        }
        turn.proposal = self.state.proposal.clone();
        turn.confirmed = self.state.confirmed;
        turn.done = self.state.confirmed;
        if turn.proposal.is_some() && !turn.confirmed {
            // the proposal card has its own Apply button
            turn.quick_replies.retain(|r| !super::guided::is_apply_or_cancel(r));
        }
        Ok(turn)
    }

    /// After a failed call: back to the state before the user's last message.
    fn drop_unanswered_tail(&mut self) {
        if let Some(Message::User { .. }) = self.state.messages.last() {
            if self.state.messages.len() > 1 {
                self.state.messages.pop();
                self.state.user_turns = self.state.user_turns.saturating_sub(1);
            }
        }
    }

    /// Runs one tool: (result for the model, failed, ends the turn).
    async fn run_tool(&mut self, call: &ToolCall, turn: &mut AssistantTurn) -> (String, bool, bool) {
        let args = &call.arguments;
        let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned();
        match call.name.as_str() {
            "suggest_replies" => {
                turn.quick_replies = quick_replies(args.get("options"));
                (json!({ "shown": turn.quick_replies.len() }).to_string(), false, true)
            }
            "search_headphones" => match self.autoeq.search(&text("query")).await {
                Ok(found) => {
                    self.state.found = found.into_iter().take(SEARCH_RESULTS).collect();
                    let list: Vec<Value> = self
                        .state
                        .found
                        .iter()
                        .map(|e| json!({ "name": e.name, "measuredBy": e.source, "type": e.kind, "path": e.path }))
                        .collect();
                    (json!({ "results": list }).to_string(), false, false)
                }
                Err(e) => (e.humane_in(self.lang()), true, false),
            },
            "get_headphone_profile" => match self.headphone_profile(&text("path")).await {
                Ok(p) => (serde_json::to_string(&p).unwrap_or_default(), false, false),
                Err(message) => (message, true, false),
            },
            "propose_eq" => match self.propose(args).await {
                Ok(summary) => (summary.to_string(), false, false),
                Err(message) => (message, true, false),
            },
            "apply_eq" => match self.apply(&text("output_id")) {
                Ok(()) => (json!({ "applied": true }).to_string(), false, false),
                Err(message) => (message, true, false),
            },
            "list_taste_rules" => {
                let t = &self.state.context.taste;
                (json!({ "layersOn": t.enabled, "rules": t.rules }).to_string(), false, false)
            }
            "listening_summary" => {
                let summary = match &self.state.context.listening {
                    Some(l) if !l.is_empty() => serde_json::to_value(l).unwrap_or_default(),
                    _ => json!({ "known": false }),
                };
                (summary.to_string(), false, false)
            }
            "set_taste_rule" => match self.set_taste_rule(args) {
                Ok(rule) => {
                    turn.taste = Some(self.state.context.taste.clone());
                    (json!({ "ok": true, "rule": rule }).to_string(), false, false)
                }
                Err(message) => (message, true, false),
            },
            "remove_taste_rule" => match self.agreed() {
                Ok(()) if self.state.context.taste.remove(&text("id")) => {
                    turn.taste = Some(self.state.context.taste.clone());
                    (json!({ "removed": true }).to_string(), false, false)
                }
                Ok(()) => ("no rule with that id: see list_taste_rules".into(), true, false),
                Err(message) => (message, true, false),
            },
            "set_loudness" => match self.set_loudness(args) {
                Ok(l) => {
                    turn.loudness = Some(l);
                    (json!({ "ok": true, "loudness": l }).to_string(), false, false)
                }
                Err(message) => (message, true, false),
            },
            other => (format!("unknown tool `{other}`"), true, false),
        }
    }

    fn lang(&self) -> crate::humane::Lang {
        if self.state.context.spanish() {
            crate::humane::Lang::Es
        } else {
            crate::humane::Lang::En
        }
    }

    async fn headphone_profile(&self, path: &str) -> std::result::Result<EqProfile, String> {
        let entry = entry_for(&self.state.found, path)
            .ok_or_else(|| "unknown path: use one from search_headphones".to_owned())?;
        self.autoeq.profile(&entry).await.map_err(|e| e.humane_in(self.lang()))
    }

    async fn propose(&mut self, args: &Value) -> std::result::Result<Value, String> {
        let output = self.pick_output(args.get("output_id").and_then(Value::as_str))?;
        // the base: the model's own bands, else a headphone's correction, else flat
        let mut base = args.get("profile").and_then(profile_arg).filter(|p| !p.bands.is_empty());
        if base.is_none() {
            if let Some(path) = args.get("headphone_path").and_then(Value::as_str).filter(|p| !p.trim().is_empty()) {
                base = Some(self.headphone_profile(path.trim()).await?);
            }
        }
        let base = base.unwrap_or_else(|| EqProfile::flat(""));
        let tilt = args.get("taste").map(taste_arg).unwrap_or_default();
        let (mut profile, notes) = finish_profile_for(&base, tilt, output.kind == OutputKind::BuiltIn);
        profile.source = EqSource::Assistant;
        profile.name = args
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(|n| n.chars().take(60).collect())
            .unwrap_or_else(|| default_name(&base, &output.name, self.state.context.spanish()));
        let explanation = args.get("explanation").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
        let summary = json!({
            "ok": true,
            "outputId": output.id,
            "name": profile.name,
            "preampDb": profile.preamp_db,
            "bands": profile.bands.len(),
            "adjusted": notes,
            "next": "Explain it in two or three plain sentences and ask whether to apply it (suggest_replies).",
        });
        self.state.proposal = Some(EqProposal { output_id: output.id.clone(), profile, explanation });
        self.state.proposed_at = self.state.user_turns;
        Ok(summary)
    }

    fn apply(&mut self, output_id: &str) -> std::result::Result<(), String> {
        let Some(proposal) = &self.state.proposal else {
            return Err("nothing proposed yet: call propose_eq first".into());
        };
        if !output_id.is_empty() && self.state.context.output(output_id).is_some_and(|o| o.id != proposal.output_id) {
            return Err(format!("the proposal is for `{}`; propose again for another output", proposal.output_id));
        }
        if self.state.user_turns <= self.state.proposed_at {
            return Err(
                "the user hasn't answered the proposal yet: ask them first and apply only after a clear yes".into()
            );
        }
        self.state.confirmed = true;
        Ok(())
    }

    /// Changes to rules and loudness need a "yes": never on the first, unprompted turn.
    fn agreed(&self) -> std::result::Result<(), String> {
        if self.state.user_turns == 0 {
            return Err(
                "the user hasn't answered yet: propose it in plain words, ask, and apply only after a clear yes".into(),
            );
        }
        Ok(())
    }

    fn set_taste_rule(&mut self, args: &Value) -> std::result::Result<TasteRule, String> {
        self.agreed()?;
        let scope = scope_arg(args.get("scope"))?;
        let tilt =
            safe_tilt(args.get("tilt").map(taste_arg).unwrap_or_default(), self.state.context.on_small_speaker());
        if tilt.is_neutral() {
            return Err("that tilt changes nothing; to drop a rule use remove_taste_rule".into());
        }
        let es = self.state.context.spanish();
        let label = args
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| l.chars().take(60).collect())
            .unwrap_or_else(|| super::reactions::describe(&scope, tilt, es));
        Ok(self.state.context.taste.upsert(TasteRule::new(scope, tilt, label, TasteCreator::Ai)))
    }

    fn set_loudness(&mut self, args: &Value) -> std::result::Result<LoudnessSettings, String> {
        self.agreed()?;
        let mode = match args.get("mode").and_then(Value::as_str).map(|m| m.trim().to_lowercase()).as_deref() {
            Some("off") => LoudnessMode::Off,
            Some("track") => LoudnessMode::Track,
            Some("album") => LoudnessMode::Album,
            Some("auto") => LoudnessMode::Auto,
            _ => return Err("mode must be one of off, track, album, auto".into()),
        };
        let mut l = self.state.context.loudness;
        l.mode = mode;
        if let Some(t) = args.get("target_lufs").or_else(|| args.get("targetLufs")).and_then(Value::as_f64) {
            l.target_lufs = t as f32;
        }
        let l = l.clamped();
        self.state.context.loudness = l;
        Ok(l)
    }

    fn pick_output(&self, id: Option<&str>) -> std::result::Result<AudioOutput, String> {
        let outputs = &self.state.context.outputs;
        match id.map(str::trim).filter(|i| !i.is_empty()) {
            Some(id) => self.state.context.output(id).cloned().ok_or_else(|| {
                let ids: Vec<&str> = outputs.iter().map(|o| o.id.as_str()).collect();
                format!("unknown output_id `{id}`; use one of {ids:?}")
            }),
            None if outputs.len() == 1 => Ok(outputs[0].clone()),
            None => Err("output_id is required".into()),
        }
    }
}

/// Either conversation behind one handle, for the apps: the AI assistant or
/// the guided wizard, saved and resumed the same way.
pub enum EqSession {
    Ai(Box<EqAssistant>),
    Guided(Box<GuidedEq>),
}

/// A saved [`EqSession`] (no key: it comes back from the keychain on resume).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "state", rename_all = "snake_case")]
// built once per save / resume: boxing would only complicate its users
#[allow(clippy::large_enum_variant)]
pub enum SavedSession {
    Ai(AssistantState),
    Guided(GuidedState),
}

impl EqSession {
    pub fn ai(config: AiConfig, context: AssistantContext) -> Self {
        Self::Ai(Box::new(EqAssistant::new(config, context)))
    }

    pub fn guided(context: AssistantContext) -> Self {
        Self::Guided(Box::new(GuidedEq::new(context)))
    }

    /// Continues a saved session. An AI one needs `config` (with the key);
    /// without it, it can't go on and starts over as the guided wizard.
    pub fn resume(saved: SavedSession, config: Option<AiConfig>) -> Self {
        match (saved, config) {
            (SavedSession::Ai(state), Some(config)) => Self::Ai(Box::new(EqAssistant::resume(config, state))),
            (SavedSession::Ai(state), None) => Self::guided(state.context),
            (SavedSession::Guided(state), _) => Self::Guided(Box::new(GuidedEq::resume(state))),
        }
    }

    pub fn with_autoeq(self, autoeq: Arc<AutoEq>) -> Self {
        match self {
            Self::Ai(a) => Self::Ai(Box::new(a.with_autoeq(autoeq))),
            Self::Guided(g) => Self::Guided(Box::new(g.with_autoeq(autoeq))),
        }
    }

    pub fn is_ai(&self) -> bool {
        matches!(self, Self::Ai(_))
    }

    pub async fn start(&mut self) -> Result<AssistantTurn> {
        match self {
            Self::Ai(a) => a.start().await,
            Self::Guided(g) => g.start().await,
        }
    }

    /// A proactive taste suggestion (a card); answers go to [`reply`](Self::reply).
    pub async fn suggest(&mut self) -> Result<AssistantTurn> {
        match self {
            Self::Ai(a) => a.suggest().await,
            Self::Guided(g) => Ok(g.suggest()),
        }
    }

    pub async fn reply(&mut self, text: &str) -> Result<AssistantTurn> {
        match self {
            Self::Ai(a) => a.reply(text).await,
            Self::Guided(g) => g.reply(text).await,
        }
    }

    pub fn save(&self) -> SavedSession {
        match self {
            Self::Ai(a) => SavedSession::Ai(a.state().clone()),
            Self::Guided(g) => SavedSession::Guided(g.state().clone()),
        }
    }

    /// The AI conversation so far (the guided wizard keeps none).
    pub fn transcript(&self) -> Vec<TranscriptLine> {
        match self {
            Self::Ai(a) => a.state().transcript(),
            Self::Guided(_) => Vec::new(),
        }
    }
}

/// The headphone `path` refers to: one of the last search, else rebuilt from the path.
pub(crate) fn entry_for(found: &[HeadphoneEntry], path: &str) -> Option<HeadphoneEntry> {
    let path = path.trim().trim_start_matches("./");
    if let Some(e) = found.iter().find(|e| e.path == path || e.name.eq_ignore_ascii_case(path)) {
        return Some(e.clone());
    }
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [source, kind, name] if !source.is_empty() && !name.is_empty() => Some(HeadphoneEntry {
            name: (*name).to_owned(),
            source: (*source).to_owned(),
            kind: kind.rsplit(' ').next().unwrap_or(kind).to_owned(),
            rig: None,
            path: path.to_owned(),
        }),
        _ => None,
    }
}

/// `base` with `tilt` applied, every value in range, boosts capped and the
/// automatic preamp; plus what had to be changed (for the model).
#[cfg(test)]
pub(crate) fn finish_profile(base: &EqProfile, tilt: Tilt) -> (EqProfile, Vec<String>) {
    finish_profile_for(base, tilt, false)
}

/// [`finish_profile`] for the kind of output: on a `small_speaker` (built-in,
/// phone, laptop) deep bass boosts become a gentle low-mid lift with a
/// high-pass below (see [`with_taste_for`]).
pub(crate) fn finish_profile_for(base: &EqProfile, tilt: Tilt, small_speaker: bool) -> (EqProfile, Vec<String>) {
    let tilt = tilt.clamped(MAX_TASTE_DB);
    let mut notes = Vec::new();
    if base.bands.len() > crate::dsp::MAX_BANDS {
        notes.push(format!("only the first {} bands are kept", crate::dsp::MAX_BANDS));
    }
    let base = if small_speaker { small_speaker_safe(base, &mut notes) } else { base.clone() };
    let mut profile = with_taste_for(&base, tilt, small_speaker);
    for b in &mut profile.bands {
        if b.kind.has_gain() && b.gain_db > MAX_BOOST_DB {
            notes.push(format!("{:.0} Hz boost lowered from {:.1} to {MAX_BOOST_DB} dB", b.freq_hz, b.gain_db));
            b.gain_db = MAX_BOOST_DB;
        }
    }
    profile.preamp_db = auto_preamp_db(&profile);
    (profile, notes)
}

/// Small speakers can't play deep bass: bass boosts below 150 Hz become a
/// ≤ 3 dB lift at 200 Hz, with a high-pass at 90 Hz.
fn small_speaker_safe(base: &EqProfile, notes: &mut Vec<String>) -> EqProfile {
    use crate::dsp::{SMALL_SPEAKER_BASS_HZ, SMALL_SPEAKER_HIGHPASS_HZ, SMALL_SPEAKER_MAX_BASS_DB};
    let mut p = base.clamped();
    let mut moved = false;
    for b in &mut p.bands {
        let boost = b.enabled && b.gain_db > 0.0 && matches!(b.kind, EqBandKind::LowShelf | EqBandKind::Peak);
        if boost && b.freq_hz < 150.0 {
            notes.push(format!(
                "built-in speaker: the {:.0} Hz bass boost became a {SMALL_SPEAKER_BASS_HZ:.0} Hz lift (they can't play deep bass)",
                b.freq_hz
            ));
            *b = EqBand::new(EqBandKind::Peak, SMALL_SPEAKER_BASS_HZ, b.gain_db.min(SMALL_SPEAKER_MAX_BASS_DB), 0.9);
            moved = true;
        }
    }
    if moved
        && !p.bands.iter().any(|b| b.enabled && b.kind == EqBandKind::HighPass)
        && p.bands.len() < crate::dsp::MAX_BANDS
    {
        p.bands.push(EqBand::new(EqBandKind::HighPass, SMALL_SPEAKER_HIGHPASS_HZ, 0.0, 0.707));
    }
    p
}

/// A scope from the model: `{kind, id?, name}`.
pub(crate) fn scope_arg(v: Option<&Value>) -> std::result::Result<TasteScope, String> {
    let v = v.ok_or("scope is required")?;
    let v = match v {
        Value::String(s) => serde_json::from_str(s).map_err(|_| "scope must be an object {kind, id, name}")?,
        other => other.clone(),
    };
    let field = |k: &str| v.get(k).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned();
    let (kind, id, name) = (field("kind").to_lowercase(), field("id"), field("name"));
    if name.is_empty() && id.is_empty() {
        return Err("scope needs a name (or an id)".into());
    }
    Ok(match kind.as_str() {
        "genre" if !name.is_empty() => TasteScope::Genre { name },
        "playlist" => TasteScope::Playlist { id, name },
        "artist" => TasteScope::Artist { id, name },
        "album" => TasteScope::Album { id, name },
        "track" => TasteScope::Track { id, name },
        _ => {
            return Err(
                "scope.kind must be one of genre, playlist, artist, album, track (a genre needs its name)".into()
            )
        }
    })
}

fn default_name(base: &EqProfile, output: &str, es: bool) -> String {
    match &base.source {
        EqSource::AutoEq { model, .. } => model.clone(),
        _ if es => format!("Asistente · {output}"),
        _ => format!("Assistant · {output}"),
    }
}

/// A profile from the model: an object, or JSON text of one.
fn profile_arg(v: &Value) -> Option<EqProfile> {
    match v {
        Value::String(s) => serde_json::from_str(s).ok(),
        Value::Object(_) => serde_json::from_value(v.clone()).ok(),
        _ => None,
    }
}

fn taste_arg(v: &Value) -> Tilt {
    let n = |k: &str, alt: &str| v.get(k).or_else(|| v.get(alt)).and_then(Value::as_f64).map(|x| x as f32);
    Tilt {
        bass_db: n("bassDb", "bass_db").unwrap_or(0.0),
        treble_db: n("trebleDb", "treble_db").unwrap_or(0.0),
        clarity_db: n("clarityDb", "clarity_db"),
    }
}

fn quick_replies(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for o in v.and_then(Value::as_array).into_iter().flatten() {
        let Some(s) = o.as_str().map(str::trim).filter(|s| !s.is_empty()) else { continue };
        let s: String = s.chars().take(48).collect();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out.truncate(MAX_QUICK_REPLIES);
    out
}

fn tools() -> Vec<Tool> {
    let tool = |name: &str, description: &str, parameters: Value| Tool {
        name: name.into(),
        description: description.into(),
        parameters,
    };
    let band = json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["peak", "lowShelf", "highShelf", "lowPass", "highPass"] },
            "freqHz": { "type": "number", "minimum": 20, "maximum": 20000 },
            "gainDb": { "type": "number", "minimum": -24, "maximum": MAX_BOOST_DB },
            "q": { "type": "number", "minimum": 0.1, "maximum": 10 }
        },
        "required": ["kind", "freqHz", "gainDb", "q"]
    });
    let tilt = json!({
        "type": "object",
        "description": "Broad adjustments, each -6..6 dB",
        "properties": {
            "bassDb": { "type": "number", "description": "low shelf at 105 Hz (on built-in speakers: a lift at 200 Hz, max +3)" },
            "trebleDb": { "type": "number", "description": "high shelf at 10 kHz" },
            "clarityDb": { "type": "number", "description": "wide peak at 3 kHz (voices, detail)" }
        }
    });
    vec![
        tool(
            "suggest_replies",
            "Show 2 to 4 short answers the user can tap for the question you just asked. Call it with every question.",
            json!({
                "type": "object",
                "properties": { "options": { "type": "array", "items": { "type": "string" }, "maxItems": MAX_QUICK_REPLIES } },
                "required": ["options"]
            }),
        ),
        tool(
            "search_headphones",
            "Search the AutoEq database of measured headphones and earphones by brand and model (e.g. \"Sony WH-1000XM5\"). Returns matches with a `path`.",
            json!({ "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }),
        ),
        tool(
            "get_headphone_profile",
            "Get the AutoEq correction profile (preamp and bands, towards the Harman target) of a headphone found with search_headphones.",
            json!({ "type": "object", "properties": { "path": { "type": "string", "description": "`path` of a search result" } }, "required": ["path"] }),
        ),
        tool(
            "propose_eq",
            "Propose an equalizer profile for one output; the app shows it to the user but does not apply it. Start from a headphone \
             (`headphone_path`) or your own `profile`, and add the user's `taste`. Values are validated, boosts capped at +12 dB and \
             the preamp is set automatically so it never clips.",
            json!({
                "type": "object",
                "properties": {
                    "output_id": { "type": "string", "description": "`id` of one of the user's outputs" },
                    "explanation": { "type": "string", "description": "Why, in two or three plain sentences in the user's language" },
                    "name": { "type": "string", "description": "Short profile name" },
                    "headphone_path": { "type": "string", "description": "AutoEq `path` to start from" },
                    "profile": { "type": "object", "properties": { "bands": { "type": "array", "items": band, "maxItems": crate::dsp::MAX_BANDS } } },
                    "taste": {
                        "type": "object",
                        "description": "Broad adjustments on top, each -6..6 dB",
                        "properties": tilt["properties"].clone()
                    }
                },
                "required": ["output_id", "explanation"]
            }),
        ),
        tool(
            "apply_eq",
            "Apply the last proposal. Only after the user clearly said yes to it.",
            json!({ "type": "object", "properties": { "output_id": { "type": "string" } }, "required": ["output_id"] }),
        ),
        tool(
            "listening_summary",
            "What the user listens to most: top genres, top artists and their playlists (may be unknown).",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_taste_rules",
            "The user's taste layers: rules that tilt the sound for a genre, playlist, artist, album or track, on top of the output's profile.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "set_taste_rule",
            "Create or update the taste layer for one genre, playlist, artist, album or track (one rule per scope; the most \
             specific one plays: track > album > playlist > artist > genre). Only after the user clearly said yes to it. Each \
             value -6..6 dB, usually 1 to 3.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "enum": ["genre", "playlist", "artist", "album", "track"] },
                            "id": { "type": "string", "description": "Tidal id when known (playlists: their uuid)" },
                            "name": { "type": "string" }
                        },
                        "required": ["kind", "name"]
                    },
                    "tilt": tilt,
                    "label": { "type": "string", "description": "Short name the user sees, in their language (\"More punch for Electronic\")" }
                },
                "required": ["scope", "tilt"]
            }),
        ),
        tool(
            "remove_taste_rule",
            "Remove a taste layer by its id (from list_taste_rules). Only after the user said yes.",
            json!({ "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] }),
        ),
        tool(
            "set_loudness",
            "Loudness normalization from ReplayGain: `off` (bit-perfect), `track` (every song as loud), `album` (albums keep their \
             dynamics) or `auto` (album when an album plays in order, else track). Only after the user said yes.",
            json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["off", "track", "album", "auto"] },
                    "target_lufs": { "type": "number", "minimum": -24, "maximum": -8, "description": "default -14" }
                },
                "required": ["mode"]
            }),
        ),
    ]
}

/// Plain phrases and the conservative change each one usually needs (in
/// context examples align text with EQ better than rules alone: Aalborg/B&O,
/// arXiv 2601.09448). The phrases are in the user's language; the caps below
/// still apply.
fn phrase_examples(es: bool) -> &'static str {
    if es {
        "| Dicen | Bandas propias (`profile`) | Sobre AutoEq (`taste`) |
|---|---|---|
| \"le falta bajo\", \"suena flaco\" | lowShelf ~100 Hz +2..3 dB (built-in: peak 200 Hz +2..3, Q 0.9) | bassDb +2..3 |
| \"retumba\", \"el bajo embarra\" | peak 80-200 Hz -2..3 dB, Q ~1 | bassDb -2..3 |
| \"chillón\", \"áspero\", \"las eses pican\" | peak 3-8 kHz -2 dB, Q ~2 (sibilance: 6-8 kHz) | trebleDb -2 |
| \"apagado\", \"sin brillo\" | highShelf 8-10 kHz +2 dB | trebleDb +2 |
| \"las voces se oyen lejos\", \"no se oyen las voces\" | peak 1-3 kHz +1.5..2 dB, Q ~1 | clarityDb +1.5..2 |
| \"me cansa a volumen alto\" | highShelf 10 kHz -1..2 dB | trebleDb -1..2, and suggest a lower volume (and set_loudness if songs jump in level) |"
    } else {
        "| They say | Own bands (`profile`) | On top of AutoEq (`taste`) |
|---|---|---|
| \"thin\", \"needs more bass\" | lowShelf ~100 Hz +2..3 dB (built-in: peak 200 Hz +2..3, Q 0.9) | bassDb +2..3 |
| \"boomy\", \"muddy bass\" | peak 80-200 Hz -2..3 dB, Q ~1 | bassDb -2..3 |
| \"harsh\", \"shrill\", \"sibilant\" | peak 3-8 kHz -2 dB, Q ~2 (sibilance: 6-8 kHz) | trebleDb -2 |
| \"dull\", \"muffled\" | highShelf 8-10 kHz +2 dB | trebleDb +2 |
| \"vocals buried\", \"voices sound far away\" | peak 1-3 kHz +1.5..2 dB, Q ~1 | clarityDb +1.5..2 |
| \"fatiguing at high volume\" | highShelf 10 kHz -1..2 dB | trebleDb -1..2, and suggest a lower volume (and set_loudness if songs jump in level) |"
    }
}

fn system_prompt(state: &AssistantState) -> String {
    let ctx = &state.context;
    let language = if ctx.spanish() { "Spanish (neutral Latin American)" } else { "English" };
    let examples = phrase_examples(ctx.spanish());
    let outputs: Vec<Value> = ctx
        .outputs
        .iter()
        .map(|o| {
            let current = ctx.current.per_output.get(&o.id).map(|p| p.name.clone());
            json!({ "id": o.id, "name": o.name, "kind": o.kind, "sampleRates": o.sample_rates, "currentProfile": current })
        })
        .collect();
    let context = json!({
        "outputs": outputs,
        "activeOutput": ctx.active_output,
        "equalizerOn": ctx.current.enabled,
        "notes": ctx.notes,
        "questionsAskedSoFar": state.user_turns,
        "proposalOnTheTable": state.proposal.as_ref().map(|p| json!({ "outputId": p.output_id, "name": p.profile.name })),
        "tasteLayers": ctx.taste.rules.len(),
        "loudness": ctx.loudness.mode,
        "nowPlaying": ctx.now_playing.as_ref().map(|n| json!({
            "title": n.title, "album": n.album.as_ref().map(|a| &a.name),
            "artists": n.artists.iter().map(|a| &a.name).collect::<Vec<_>>(), "genres": n.genres,
            "from": n.source.as_ref().map(|s| json!({ "kind": s.kind, "id": s.id, "name": s.name })),
        })),
        "topGenres": ctx.listening.as_ref().map(|l| l.top_genres.iter().take(5).collect::<Vec<_>>()),
    });
    format!(
        "You are the equalizer assistant of HomeFi, a hi-fi music player. You help someone who is not an audio expert set up the \
equalizer for the device they listen on.

Language: always answer in {language}, whatever language the tools use. Plain, warm words; no jargon unless you explain it in a \
few words. Keep every message short (at most three sentences).

How to talk:
- One question at a time, and call suggest_replies with 2 to 4 short answers for it.
- At most {MAX_QUESTIONS} questions in total. Skip whatever you can infer: if there is only one output, don't ask which one; \
the output kind (Bluetooth, USB DAC, car...) tells you a lot; use the notes.
- Useful things to learn, in this order and only if they matter: which output they'll use; the exact headphone or speaker \
model (then search_headphones; if several match, ask which one; if none, say so and go on with taste alone); a DAC or amp \
only if relevant; the music they like; their taste (more bass, more clarity, less harsh, neutral); where they listen (home, \
street, car, noisy places); how loud they usually listen and whether highs feel tiring.
- When you know enough (or after {MAX_QUESTIONS} questions), call get_headphone_profile if there is a headphone, then propose_eq \
with it as `headphone_path` plus a gentle `taste` (usually 1 to 4 dB). Without a measured headphone (car, speakers, built-in), \
propose only `taste` or a few broad bands.
- After proposing, explain in plain words what changes and ask whether to apply it. Call apply_eq only after a clear yes; if \
they want something different, propose again.

Built-in speakers (laptop, phone, kind built_in): they can't play deep bass. Never boost below 150 Hz there; for \"more bass\" \
use a gentle lift around 150-250 Hz (at most +3 dB) and the app adds a high-pass at 90 Hz. A car is different: it has real \
speakers.

What people say and what it usually needs (starting points, gentle; one or two changes at a time, never past the caps):
{examples}

Taste layers (after the device step, or right away when the device is already set up):
- Taste layers tilt the sound for some music only: a genre, a playlist, an artist, an album or one track, on top of the \
output's profile. The most specific one plays (track > album > playlist > artist > genre).
- Look at listening_summary and list_taste_rules, then offer at most two concrete layers that fit what they listen to (for \
example \"a bit more bass for Electronic, +2 dB\"). Say what each does in plain words and ask; call set_taste_rule only after a \
clear yes. Keep it to few questions; if they don't care, leave it.
- Loudness normalization (set_loudness) evens out songs that are much louder or quieter than the rest. Suggest it when they \
mention jumps in volume; it changes the level, so the sound is no longer bit-perfect.

Be honest:
- With the equalizer on, the sound is processed, so it is no longer bit-perfect. Say so once, simply, when you propose.
- Never promise \"studio quality\", miracles or a \"perfect\" sound. AutoEq corrections come from measurements of one unit \
towards an average target (Harman); fit and each person's ears change the result, so invite them to try it and adjust.
- The app caps boosts at +12 dB and lowers the preamp automatically so it never clips. Suggest moderate volume. If they mention \
pain, ringing or hearing loss, suggest lowering the volume and seeing a professional; you are not a doctor.
- Don't invent headphone measurements: use the tools.

What the app knows (JSON): {context}"
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ai::AiProvider;
    use crate::dsp::{EqBand, EqBandKind};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    pub(crate) const INDEX: &str = "# Index\n\
        - [Sennheiser HD 650](./oratory1990/over-ear/Sennheiser%20HD%20650) by oratory1990\n\
        - [Sony WH-1000XM5](./oratory1990/over-ear/Sony%20WH-1000XM5) by oratory1990\n";
    pub(crate) const HD650: &str = "Preamp: -6.1 dB\n\
        Filter 1: ON LSC Fc 105 Hz Gain 6.4 dB Q 0.70\n\
        Filter 2: ON PK Fc 8800 Hz Gain 5.1 dB Q 1.42\n\
        Filter 3: ON HSC Fc 10000 Hz Gain -2.1 dB Q 0.70\n";

    /// AutoEq served by wiremock, cached in a temp dir.
    pub(crate) async fn fake_autoeq(server: &MockServer, dir: &std::path::Path) -> Arc<AutoEq> {
        Mock::given(method("GET"))
            .and(path("/INDEX.md"))
            .respond_with(ResponseTemplate::new(200).set_body_string(INDEX))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/oratory1990/over-ear/Sennheiser%20HD%20650/Sennheiser%20HD%20650%20ParametricEQ.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_string(HD650))
            .mount(server)
            .await;
        Arc::new(AutoEq::with_base_url(dir, &server.uri()))
    }

    pub(crate) fn context() -> AssistantContext {
        AssistantContext {
            language: "es".into(),
            outputs: vec![
                AudioOutput {
                    id: "dac".into(),
                    name: "USB DAC".into(),
                    kind: OutputKind::UsbDac,
                    sample_rates: vec![44100, 96000],
                },
                AudioOutput {
                    id: "bt".into(),
                    name: "WH-1000XM5".into(),
                    kind: OutputKind::Bluetooth,
                    sample_rates: vec![44100],
                },
            ],
            ..Default::default()
        }
    }

    /// Plays a fixed script of Anthropic answers, one per request.
    struct Script(std::sync::Mutex<Vec<Value>>);

    impl Respond for Script {
        fn respond(&self, _: &Request) -> ResponseTemplate {
            let mut answers = self.0.lock().unwrap();
            let next = if answers.is_empty() {
                json!({"content": [{"type": "text", "text": "fin"}]})
            } else {
                answers.remove(0)
            };
            ResponseTemplate::new(200).set_body_json(next)
        }
    }

    fn says(text: &str, calls: &[(&str, &str, Value)]) -> Value {
        let mut content = vec![json!({"type": "text", "text": text})];
        for (id, name, input) in calls {
            content.push(json!({"type": "tool_use", "id": id, "name": name, "input": input}));
        }
        json!({"content": content, "stop_reason": "tool_use"})
    }

    #[tokio::test]
    async fn interviews_proposes_and_applies_only_after_yes() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let autoeq = fake_autoeq(&server, dir.path()).await;
        let script = vec![
            // start: a question with quick replies (ends the turn without another request)
            says(
                "¡Hola! ¿Con qué vas a escuchar?",
                &[("t1", "suggest_replies", json!({"options": ["USB DAC", "WH-1000XM5"]}))],
            ),
            // "USB DAC": asks the model
            says("¿Qué audífonos conectas?", &[("t2", "suggest_replies", json!({"options": ["No sé"]}))]),
            // "HD 650": search, then profile, then propose, then explain (and try to apply too early)
            says("", &[("t3", "search_headphones", json!({"query": "HD 650"}))]),
            says("", &[("t4", "get_headphone_profile", json!({"path": "oratory1990/over-ear/Sennheiser HD 650"}))]),
            says(
                "",
                &[(
                    "t5",
                    "propose_eq",
                    json!({"output_id": "dac", "explanation": "Corrige tus HD 650 y suma un poco de bajo.", "headphone_path": "oratory1990/over-ear/Sennheiser HD 650", "taste": {"bassDb": 40}}),
                )],
            ),
            says(
                "Te propongo esto. ¿Lo aplico?",
                &[
                    ("t6", "apply_eq", json!({"output_id": "dac"})),
                    ("t7", "suggest_replies", json!({"options": ["Sí, aplícalo", "Más bajos", "No"]})),
                ],
            ),
            says("Pregunto primero: ¿lo aplico?", &[]),
            // "Sí"
            says("", &[("t8", "apply_eq", json!({"output_id": "dac"}))]),
            says("Listo, ya suena con el nuevo ecualizador.", &[]),
        ];
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(Script(std::sync::Mutex::new(script)))
            .mount(&server)
            .await;
        let config =
            AiConfig::new(AiProvider::Anthropic, "claude-sonnet-5-5", Some("k".into())).with_base_url(server.uri());
        let mut a = EqAssistant::new(config.clone(), context()).with_autoeq(autoeq.clone());

        let t = a.start().await.unwrap();
        assert_eq!(t.text, "¡Hola! ¿Con qué vas a escuchar?");
        assert_eq!(t.quick_replies, ["USB DAC", "WH-1000XM5"]);
        assert!(t.proposal.is_none() && !t.done);

        let t = a.reply("USB DAC").await.unwrap();
        assert_eq!(t.quick_replies, ["No sé"]);

        let t = a.reply("Unos HD 650").await.unwrap();
        let p = t.proposal.clone().expect("a proposal");
        assert_eq!(p.output_id, "dac");
        assert_eq!(p.profile.source, EqSource::Assistant);
        assert_eq!(p.profile.name, "Sennheiser HD 650");
        // the taste was capped at +6 and joined AutoEq's low shelf: 6.4 + 6
        assert!((p.profile.bands[0].gain_db - 12.0).abs() < 0.01, "{:?}", p.profile.bands[0]);
        assert!(p.profile.preamp_db < -6.0);
        assert!(!t.confirmed, "apply_eq before the user answered is refused");
        // the proposal card has Apply: only the other answers stay as chips
        assert_eq!(t.quick_replies, ["Más bajos"]);
        assert!(t.text.contains("¿Lo aplico?"));

        // resuming from the saved state keeps going
        let saved = serde_json::to_string(a.state()).unwrap();
        let mut a = EqAssistant::resume(config, serde_json::from_str(&saved).unwrap()).with_autoeq(autoeq);
        let t = a.reply("Sí").await.unwrap();
        assert!(t.confirmed && t.done);
        assert_eq!(t.text, "Listo, ya suena con el nuevo ecualizador.");
        let lines = a.state().transcript();
        assert_eq!(lines.first().map(|l| l.text.as_str()), Some("¡Hola! ¿Con qué vas a escuchar?"));
        assert!(lines.iter().any(|l| l.from_user && l.text == "Sí"));
    }

    #[tokio::test]
    async fn a_failed_call_leaves_the_history_as_it_was() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow"}})),
            )
            .mount(&server)
            .await;
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let mut a = EqAssistant::new(config, context());
        a.state.messages = vec![
            Message::User { text: "kick".into() },
            Message::Assistant { text: "¿Qué?".into(), tool_calls: vec![], native: None },
        ];
        let e = a.reply("hola").await.unwrap_err();
        assert!(e.humane_in(crate::humane::Lang::Es).contains("más despacio"));
        assert_eq!(a.state().messages.len(), 2);
        assert_eq!(a.state().user_turns, 0);
    }

    #[test]
    fn proposals_are_capped_and_never_clip() {
        let wild = EqProfile {
            bands: vec![
                EqBand::new(EqBandKind::Peak, 60.0, 20.0, 1.0),
                EqBand::new(EqBandKind::Peak, 3000.0, f32::NAN, 1.0),
            ],
            ..Default::default()
        };
        let (p, notes) = finish_profile(&wild, Tilt { bass_db: f32::INFINITY, treble_db: -30.0, clarity_db: None });
        assert!(p.bands.iter().all(|b| b.gain_db <= MAX_BOOST_DB && b.gain_db.is_finite()));
        assert!(!notes.is_empty());
        assert_eq!(p.preamp_db, auto_preamp_db(&p));
        assert!(p.preamp_db <= -MAX_BOOST_DB + 0.5);
        let high = p.bands.iter().find(|b| b.kind == EqBandKind::HighShelf).unwrap();
        assert_eq!(high.gain_db, -MAX_TASTE_DB);
    }

    #[tokio::test]
    async fn taste_tools_need_a_yes_and_are_validated() {
        let server = MockServer::start().await;
        let electronic = json!({"kind": "genre", "name": "Electronic"});
        let script = vec![
            // suggestion card: tries to apply before anyone answered
            says(
                "¿Un poco más de bajo para Electronic?",
                &[
                    ("s0", "listening_summary", json!({})),
                    ("s1", "set_taste_rule", json!({"scope": electronic, "tilt": {"bassDb": 2}})),
                ],
            ),
            says("¿Te lo pongo?", &[("s2", "suggest_replies", json!({"options": ["Sí", "No"]}))]),
            // "Sí": rule (capped), loudness, a bad scope and an unknown rule
            says(
                "",
                &[
                    (
                        "s3",
                        "set_taste_rule",
                        json!({"scope": electronic, "tilt": {"bassDb": 9}, "label": "Más pegada"}),
                    ),
                    ("s4", "set_loudness", json!({"mode": "auto"})),
                    ("s5", "set_taste_rule", json!({"scope": {"kind": "mood", "name": "x"}, "tilt": {"bassDb": 1}})),
                    ("s6", "remove_taste_rule", json!({"id": "nope"})),
                    ("s7", "list_taste_rules", json!({})),
                ],
            ),
            says("Listo: más bajo para Electronic.", &[]),
        ];
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(Script(std::sync::Mutex::new(script)))
            .mount(&server)
            .await;
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let mut ctx = context();
        ctx.listening = Some(ListeningSummary { top_genres: vec!["Electronic".into()], ..Default::default() });
        // playing on the laptop's speakers: no deep bass
        ctx.outputs.push(AudioOutput {
            id: "mac".into(),
            name: "MacBook Speakers".into(),
            kind: OutputKind::BuiltIn,
            sample_rates: vec![],
        });
        ctx.active_output = Some("mac".into());
        let mut a = EqAssistant::new(config, ctx);
        let t = a.suggest().await.unwrap();
        assert!(t.taste.is_none(), "nothing changes before a yes");
        assert_eq!(t.quick_replies, ["Sí", "No"]);
        let t = a.reply("Sí").await.unwrap();
        let taste = t.taste.expect("rules changed");
        assert_eq!(taste.rules.len(), 1);
        let r = &taste.rules[0];
        assert_eq!((r.label.as_str(), r.created_by), ("Más pegada", TasteCreator::Ai));
        assert_eq!(r.tilt.bass_db, crate::dsp::SMALL_SPEAKER_MAX_BASS_DB);
        assert_eq!(t.loudness.map(|l| l.mode), Some(LoudnessMode::Auto));
        // errors went back to the model as tool errors
        let results: Vec<&ToolResult> = a
            .state()
            .messages
            .iter()
            .filter_map(|m| match m {
                Message::Tool { results } => Some(results),
                _ => None,
            })
            .flatten()
            .collect();
        assert!(results.iter().any(|r| r.call_id == "s1" && r.is_error));
        assert!(results.iter().any(|r| r.call_id == "s5" && r.is_error));
        assert!(results.iter().any(|r| r.call_id == "s6" && r.is_error));
        assert!(results.iter().any(|r| r.call_id == "s0" && r.content.contains("Electronic")));
        assert!(results.iter().any(|r| r.call_id == "s7" && r.content.contains("Más pegada")));
    }

    #[test]
    fn built_in_speakers_never_get_deep_bass() {
        let hd = EqProfile {
            bands: vec![
                EqBand::new(EqBandKind::LowShelf, 105.0, 6.0, 0.7),
                EqBand::new(EqBandKind::Peak, 60.0, 4.0, 1.0),
            ],
            ..Default::default()
        };
        let (p, notes) = finish_profile_for(&hd, Tilt { bass_db: 4.0, ..Default::default() }, true);
        assert!(!notes.is_empty());
        let deep = crate::dsp::response_db(&p, &[40.0, 60.0], 48_000);
        assert!(deep.iter().all(|db| *db < 0.5), "{deep:?} {:?}", p.bands);
        assert!(p.bands.iter().all(|b| !(b.kind.has_gain() && b.gain_db > 0.0 && b.freq_hz < 150.0)));
        assert!(p.bands.iter().any(|b| b.kind == EqBandKind::HighPass));
        assert!(p.bands.iter().filter(|b| b.kind == EqBandKind::Peak).all(|b| b.gain_db <= 6.0));
    }

    #[test]
    fn saved_sessions_say_what_they_are() {
        let s = EqSession::guided(context());
        assert!(!s.is_ai());
        let json = serde_json::to_value(s.save()).unwrap();
        assert_eq!(json["kind"], "guided");
        let back: SavedSession = serde_json::from_value(json).unwrap();
        assert!(!EqSession::resume(back, None).is_ai());
        // an AI session without its key goes on as the wizard
        let ai = SavedSession::Ai(AssistantState { context: context(), ..Default::default() });
        assert!(!EqSession::resume(ai.clone(), None).is_ai());
        let config = AiConfig::new(AiProvider::Ollama, "m", None);
        assert!(EqSession::resume(ai, Some(config)).is_ai());
    }

    #[test]
    fn entries_resolve_from_search_or_path() {
        let e = entry_for(&[], "./oratory1990/over-ear/Sennheiser HD 650").unwrap();
        assert_eq!(
            (e.name.as_str(), e.source.as_str(), e.kind.as_str()),
            ("Sennheiser HD 650", "oratory1990", "over-ear")
        );
        assert!(entry_for(&[], "nonsense").is_none());
        assert_eq!(quick_replies(Some(&json!(["a", " a ", "", "b", "c", "d", "e"]))), ["a", "b", "c", "d"]);
    }

    #[test]
    fn the_prompt_has_phrase_examples_in_the_users_language() {
        let es = system_prompt(&AssistantState { context: context(), ..Default::default() });
        for phrase in
            ["le falta bajo", "retumba", "chillón", "apagado", "voces se oyen lejos", "me cansa a volumen alto"]
        {
            assert!(es.contains(phrase), "{phrase}");
        }
        let en = system_prompt(&AssistantState {
            context: AssistantContext { language: "en".into(), ..context() },
            ..Default::default()
        });
        for phrase in ["thin", "boomy", "harsh", "sibilant", "dull", "vocals buried", "fatiguing at high volume"] {
            assert!(en.contains(phrase), "{phrase}");
        }
        assert!(!en.contains("le falta bajo") && !es.contains("vocals buried"));
        for prompt in [&es, &en] {
            // the conservative changes, and the caps still there
            for change in
                ["lowShelf ~100 Hz +2..3 dB", "peak 200 Hz", "peak 80-200 Hz -2..3 dB, Q ~1", "peak 3-8 kHz -2 dB"]
            {
                assert!(prompt.contains(change), "{change}");
            }
            assert!(prompt.contains("highShelf 8-10 kHz +2 dB") && prompt.contains("peak 1-3 kHz +1.5..2 dB"));
            assert!(prompt.contains("set_loudness if songs jump"));
            assert!(prompt.contains("caps boosts at +12 dB") && prompt.contains("at most +3 dB"));
            assert!(prompt.contains("studio quality"));
        }
    }
}
