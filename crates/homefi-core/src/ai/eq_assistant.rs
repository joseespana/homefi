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
    auto_preamp_db, clamp_taste_curve, clamp_taste_to, curve_summary, log_freqs, resolve_taste, response_db,
    taste_profile, with_taste_for, with_taste_rule, EqBand, EqBandKind, EqProfile, EqSettings, EqSource, ItemRef,
    LoudnessMode, LoudnessSettings, NowPlayingContext, TasteBand, TasteChange, TasteCreator, TasteIntensity, TasteRule,
    TasteScope, TasteSettings, Tilt, MAX_TASTE_BANDS, MAX_TASTE_BAND_DB, MAX_TASTE_BOOST_DB,
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
    /// Opened from one rule's editor ("Ask the AI to refine"): the id of that
    /// rule; the conversation starts from its curve.
    #[serde(default)]
    pub focus_rule: Option<String>,
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

    /// The rule being refined, if the assistant was opened for one.
    pub(crate) fn focus(&self) -> Option<&TasteRule> {
        let id = self.focus_rule.as_deref()?.trim();
        self.taste.rules.iter().find(|r| r.id == id)
    }

    /// Replaces the copies of the user's settings with the ones they have now
    /// (rules deleted or edited elsewhere since the conversation began), so
    /// the model never works from, or writes back, stale rules.
    pub fn sync(&mut self, taste: TasteSettings, loudness: LoudnessSettings, eq: EqSettings) {
        self.taste = taste;
        self.loudness = loudness;
        self.current = eq;
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
    /// What changed in the taste rules this turn (the user agreed): merge it
    /// into the settings as they are now ([`TasteSettings::apply`]), save and
    /// apply. A change, not a snapshot: a copy would bring back rules deleted
    /// since the conversation began.
    #[serde(default)]
    pub taste_change: Option<TasteChange>,
    /// Loudness changed this turn: save and apply it.
    #[serde(default)]
    pub loudness: Option<LoudnessSettings>,
    /// A taste layer proposed this turn, not saved: show its curve and the
    /// notes of its bands; "apply" answers yes (then it comes in `taste`).
    #[serde(default)]
    pub taste_proposal: Option<TasteRule>,
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
    /// The taste layer on the table (see [`AssistantTurn::taste_proposal`]).
    #[serde(default)]
    pub taste_proposal: Option<TasteRule>,
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

    /// See [`AssistantContext::sync`].
    pub fn sync(&mut self, taste: TasteSettings, loudness: LoudnessSettings, eq: EqSettings) {
        self.state.context.sync(taste, loudness, eq);
    }

    /// The first question (or, opened for one rule, what its curve does now
    /// and what to change).
    pub async fn start(&mut self) -> Result<AssistantTurn> {
        let es = self.state.context.spanish();
        let kickoff = match self.state.context.focus() {
            Some(r) if es => format!(
                "(El usuario abrió el asistente desde el editor de la capa «{}» ({}: {}) para afinarla. Llama analyze_sound con \
                 ese scope, di en una o dos frases qué hace su curva ahora y pregunta qué quiere cambiar, con suggest_replies.)",
                r.label,
                r.scope.kind(),
                r.scope.name()
            ),
            Some(r) => format!(
                "(The user opened the assistant from the editor of the taste layer “{}” ({}: {}) to refine it. Call \
                 analyze_sound with that scope, say in one or two sentences what its curve does now and ask what they'd like \
                 to change, with suggest_replies.)",
                r.label,
                r.scope.kind(),
                r.scope.name()
            ),
            None if es => {
                "(El usuario abrió el asistente del ecualizador. Salúdalo en una frase y haz la primera pregunta.)".to_owned()
            }
            None => "(The user opened the equalizer assistant. Greet them in one sentence and ask the first question.)"
                .to_owned(),
        };
        self.state.messages = vec![Message::User { text: kickoff }];
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
        if turn.proposal.is_some() && !turn.confirmed || turn.taste_proposal.is_some() {
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
                Ok((rule, adjusted)) => {
                    turn.taste_change.get_or_insert_with(TasteChange::default).push_upsert(rule.clone());
                    turn.taste_proposal = None;
                    let summary = curve_summary(&rule, self.state.context.spanish());
                    (
                        json!({ "ok": true, "rule": rule, "summary": summary, "adjusted": adjusted }).to_string(),
                        false,
                        false,
                    )
                }
                Err(message) => (message, true, false),
            },
            "propose_taste_rule" => match self.taste_curve(args, false) {
                Ok((rule, adjusted)) => {
                    let summary = curve_summary(&rule, self.state.context.spanish());
                    self.state.taste_proposal = Some(rule.clone());
                    turn.taste_proposal = Some(rule.clone());
                    let next = "The app shows the curve and your band notes. Explain each band in one plain line, mention \
                                anything in `adjusted`, and ask whether to apply it (suggest_replies). After a clear yes call \
                                set_taste_rule with the same scope (the bands are kept).";
                    (
                        json!({ "ok": true, "rule": rule, "summary": summary, "adjusted": adjusted, "next": next })
                            .to_string(),
                        false,
                        false,
                    )
                }
                Err(message) => (message, true, false),
            },
            "analyze_sound" => (self.analyze_sound(args).to_string(), false, false),
            "remove_taste_rule" => match self.agreed() {
                Ok(()) if self.state.context.taste.remove(&text("id")) => {
                    turn.taste_change.get_or_insert_with(TasteChange::default).push_remove(text("id"));
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
        let limits = self.state.context.taste.intensity.limits();
        let asked_tilt = args.get("taste").map(taste_arg).unwrap_or_default();
        let tilt = asked_tilt.clamped(limits.tilt_db);
        let (mut profile, mut notes) = finish_profile_for(&base, tilt, output.kind == OutputKind::BuiltIn);
        // the model's own values out of range: say so, so it can explain
        for b in base.bands.iter().take(crate::dsp::MAX_BANDS) {
            let c = b.clamped();
            if (c.freq_hz - b.freq_hz).abs() > 0.5 || (c.q - b.q).abs() > 0.005 {
                notes.push(format!("{:.0} Hz band brought in range (20..20000 Hz, Q 0.1..10)", c.freq_hz));
            }
        }
        if tilt != asked_tilt.clamped(MAX_TASTE_DB) {
            notes.push(format!("taste knobs limited to ±{:.0} dB at this intensity", limits.tilt_db));
        }
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

    /// Saves a taste layer (after a yes): its own values, else the proposal
    /// on the table for that scope. Returns the rule and what was clamped.
    fn set_taste_rule(&mut self, args: &Value) -> std::result::Result<(TasteRule, Vec<String>), String> {
        self.agreed()?;
        let (rule, notes) = self.taste_curve(args, true)?;
        let stored = self.state.context.taste.upsert(rule);
        if self.state.taste_proposal.as_ref().is_some_and(|p| p.scope.same_as(&stored.scope)) {
            self.state.taste_proposal = None;
        }
        Ok((stored, notes))
    }

    /// A taste rule from the model's arguments, validated for the output
    /// playing (not saved). Missing `bands` / `tilt` keep the ones it starts
    /// from: the proposal on the table for that scope (`from_proposal`), else
    /// the saved rule.
    fn taste_curve(&self, args: &Value, from_proposal: bool) -> std::result::Result<(TasteRule, Vec<String>), String> {
        let ctx = &self.state.context;
        let scope = scope_arg(args.get("scope"))?;
        let proposal = self.state.taste_proposal.as_ref().filter(|p| from_proposal && p.scope.same_as(&scope));
        let start = proposal.or_else(|| ctx.taste.rule_for(&scope));
        let mut notes = Vec::new();
        let bands = match args.get("bands") {
            Some(v) => {
                let (bands, skipped) = bands_arg(v);
                notes.extend(skipped);
                bands
            }
            None => start.map(|r| r.bands.clone()).unwrap_or_default(),
        };
        let tilt = match args.get("tilt") {
            Some(v) => taste_arg(v),
            None => start.map(|r| r.tilt).unwrap_or_default(),
        };
        let (bands, tilt, adjusted) = clamp_taste_curve(&bands, tilt, ctx.on_small_speaker());
        notes.extend(adjusted);
        // what the model proposes stays within the listener's intensity; a curve it only keeps is left alone
        let (bands, tilt) = if args.get("bands").is_some() || args.get("tilt").is_some() {
            let (bands, tilt, narrowed) = clamp_taste_to(&bands, tilt, &ctx.taste.intensity.limits());
            notes.extend(narrowed);
            (bands, tilt)
        } else {
            (bands, tilt)
        };
        if tilt.is_neutral() && bands.iter().all(|b| b.band.is_neutral()) {
            return Err(
                "that changes nothing (no audible band, a flat tilt); to drop a rule use remove_taste_rule".into()
            );
        }
        let text = |k: &str, max: usize| {
            args.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| l.chars().take(max).collect::<String>())
        };
        let es = ctx.spanish();
        let label = text("label", 60)
            .or_else(|| proposal.map(|p| p.label.clone()))
            .unwrap_or_else(|| super::reactions::describe_curve(&scope, tilt, &bands, es));
        let explanation =
            text("explanation", 400).or_else(|| proposal.map(|p| p.explanation.clone())).unwrap_or_default();
        let rule = TasteRule { bands, explanation, ..TasteRule::new(scope, tilt, label, TasteCreator::Ai) };
        Ok((rule, notes))
    }

    /// What plays now on the active output (or for `scope`): the device
    /// profile, the taste rule's curve and the summed response at 31 points.
    fn analyze_sound(&self, args: &Value) -> Value {
        let ctx = &self.state.context;
        let output = ctx.active_output.as_deref().and_then(|id| ctx.output(id)).or(ctx.outputs.first());
        let output_id = output.map(|o| o.id.clone()).unwrap_or_default();
        let device = ctx.current.profile_for(&output_id).filter(|_| ctx.current.enabled).map(EqProfile::clamped);
        let asked = args.get("scope").filter(|v| !v.is_null()).map(|v| scope_arg(Some(v)));
        let (rule, from) = match &asked {
            Some(Ok(scope)) => match self.state.taste_proposal.as_ref().filter(|p| p.scope.same_as(scope)) {
                Some(p) => (Some(p.clone()), "proposal on the table"),
                None => (ctx.taste.rule_for(scope).cloned(), "saved rule for that scope"),
            },
            _ => {
                let now = ctx.now_playing.clone().unwrap_or_default();
                (resolve_taste(&ctx.taste, &now).map(|(r, _)| r.clone()), "rule playing now")
            }
        };
        let small = output.is_some_and(|o| o.kind == OutputKind::BuiltIn);
        let base = device.clone().unwrap_or_else(|| EqProfile::flat(""));
        let (bands, tilt) = rule.as_ref().map(|r| (r.bands.clone(), r.tilt)).unwrap_or_default();
        let freqs = log_freqs(31);
        let round = |v: f32| (v * 10.0).round() / 10.0;
        let dev = response_db(&base, &freqs, 48_000);
        let taste = response_db(&taste_profile(&bands, tilt), &freqs, 48_000);
        let total_profile = with_taste_rule(&base, &bands, tilt, small);
        let total = response_db(&total_profile, &freqs, 48_000);
        let points: Vec<Value> = freqs
            .iter()
            .enumerate()
            .map(|(i, f)| json!({ "hz": f.round(), "device": round(dev[i]), "taste": round(taste[i]), "total": round(total[i]) }))
            .collect();
        // extremes on a fine grid: 31 points can miss a narrow band
        let fine = log_freqs(512);
        let dense = response_db(&total_profile, &fine, 48_000);
        let at = |pick: fn(f32, f32) -> bool| {
            dense.iter().zip(&fine).fold((0f32, 1000f32), |acc, (&db, &f)| if pick(db, acc.0) { (db, f) } else { acc })
        };
        let ((max, max_hz), (min, min_hz)) = (at(|a, b| a > b), at(|a, b| a < b));
        json!({
            "output": output.map(|o| json!({ "id": o.id, "name": o.name, "kind": o.kind })),
            "builtInSpeaker": small,
            "equalizerOn": ctx.current.enabled,
            "deviceProfile": device.map(|p| json!({ "name": p.name, "source": p.source, "bands": p.bands })),
            "scopeError": match &asked { Some(Err(e)) => Some(e.clone()), _ => None },
            "tasteRule": rule.as_ref().map(|r| json!({
                "from": from, "id": r.id, "label": r.label, "scope": r.scope, "tilt": r.tilt, "bands": r.bands,
                "summary": curve_summary(r, ctx.spanish()), "enabled": r.enabled,
            })),
            "tasteBandsLeft": MAX_TASTE_BANDS.saturating_sub(bands.len()),
            "response": points,
            "maxBoostDb": round(max),
            "maxBoostHz": max_hz.round(),
            "maxCutDb": round(min),
            "maxCutHz": min_hz.round(),
            "preampDb": round(auto_preamp_db(&total_profile)),
            "tip": "Edit a band that is already there (same kind, about the same frequency) instead of stacking another one on \
                    it, and don't redo what the device profile already corrects.",
        })
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

    /// Gives the conversation the user's settings as they are now: call it
    /// before every reply, since rules may have changed since it began.
    pub fn sync(&mut self, taste: TasteSettings, loudness: LoudnessSettings, eq: EqSettings) {
        match self {
            Self::Ai(a) => a.sync(taste, loudness, eq),
            Self::Guided(g) => g.sync(taste, loudness, eq),
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

/// A taste curve from the model: an array (or JSON text of one) of
/// `{kind, freqHz, gainDb, q, note}`, lenient about names (`freq`, `gain`,
/// `bell`, `hp`...). Bands without a frequency or of an unknown kind are
/// skipped, and said so.
pub(crate) fn bands_arg(v: &Value) -> (Vec<TasteBand>, Vec<String>) {
    let v = match v {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    };
    let items = v.as_array().or_else(|| v.get("bands").and_then(Value::as_array)).cloned().unwrap_or_default();
    let mut bands = Vec::new();
    let mut notes = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let num = |keys: &[&str]| keys.iter().find_map(|k| item.get(*k).and_then(Value::as_f64)).map(|x| x as f32);
        let text = |keys: &[&str]| keys.iter().find_map(|k| item.get(*k).and_then(Value::as_str)).map(str::trim);
        let kind_text = text(&["kind", "type"]).unwrap_or("peak").to_lowercase().replace(['_', '-', ' '], "");
        let kind = match kind_text.as_str() {
            "peak" | "peaking" | "bell" | "pk" | "eq" => EqBandKind::Peak,
            "lowshelf" | "ls" | "lsc" | "bassshelf" => EqBandKind::LowShelf,
            "highshelf" | "hs" | "hsc" | "trebleshelf" => EqBandKind::HighShelf,
            "highpass" | "hp" | "hpf" | "lowcut" => EqBandKind::HighPass,
            "lowpass" | "lp" | "lpf" | "highcut" => EqBandKind::LowPass,
            other => {
                notes.push(format!(
                    "band {}: unknown kind `{other}`, skipped (peak, lowShelf, highShelf, highPass, lowPass)",
                    i + 1
                ));
                continue;
            }
        };
        let Some(freq) = num(&["freqHz", "freq_hz", "freq", "frequency", "hz"]) else {
            notes.push(format!("band {}: no freqHz, skipped", i + 1));
            continue;
        };
        let q =
            num(&["q", "Q"]).unwrap_or(if kind == EqBandKind::Peak { 1.0 } else { std::f32::consts::FRAC_1_SQRT_2 });
        let gain = num(&["gainDb", "gain_db", "gain", "db"]).unwrap_or(0.0);
        let mut band = TasteBand::new(kind, freq, gain, q);
        band.band.enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        band.note = text(&["note", "why", "explanation", "reason"]).unwrap_or_default().to_owned();
        bands.push(band);
    }
    (bands, notes)
}

fn tilt_schema() -> Value {
    json!({
        "type": "object",
        "description": "Broad adjustments, each -6..6 dB",
        "properties": {
            "bassDb": { "type": "number", "description": "low shelf at 105 Hz (on built-in speakers: a lift at 200 Hz, max +3)" },
            "trebleDb": { "type": "number", "description": "high shelf at 10 kHz" },
            "clarityDb": { "type": "number", "description": "wide peak at 3 kHz (voices, detail)" }
        }
    })
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
    let taste_band = json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["peak", "lowShelf", "highShelf", "highPass", "lowPass"] },
            "freqHz": { "type": "number", "minimum": 20, "maximum": 20000 },
            "gainDb": { "type": "number", "minimum": -MAX_TASTE_BAND_DB, "maximum": MAX_TASTE_BAND_DB, "description": "unused by passes" },
            "q": { "type": "number", "minimum": 0.3, "maximum": 8, "description": "peaks: 0.5 wide .. 4 narrow; shelves and passes ~0.7" },
            "note": { "type": "string", "description": "What it does, one plain line in the user's language" }
        },
        "required": ["kind", "freqHz", "gainDb", "q", "note"]
    });
    let scope = json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["genre", "playlist", "artist", "album", "track"] },
            "id": { "type": "string", "description": "Tidal id when known (playlists: their uuid)" },
            "name": { "type": "string" }
        },
        "required": ["kind", "name"]
    });
    let taste_rule = |extra: &str| {
        json!({
            "type": "object",
            "properties": {
                "scope": scope.clone(),
                "label": { "type": "string", "description": "Short name the user sees, in their language (\"More punch for Electronic\")" },
                "explanation": { "type": "string", "description": "Why, in one or two plain sentences in the user's language" },
                "bands": {
                    "type": "array", "items": taste_band.clone(), "maxItems": MAX_TASTE_BANDS,
                    "description": format!("The whole parametric curve of the layer (replaces the old one). {extra}")
                },
                "tilt": tilt_schema()
            },
            "required": ["scope"]
        })
    };
    let tilt = tilt_schema();
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
            "analyze_sound",
            "What plays now: the output's device profile, the taste layer's curve (for `scope`, else the one playing now) and \
             their summed response in dB at 31 points from 20 Hz to 20 kHz. Call it before designing or changing a curve, so \
             you edit what is there instead of stacking bands.",
            json!({ "type": "object", "properties": { "scope": scope.clone() } }),
        ),
        tool(
            "propose_taste_rule",
            "Show a taste layer to the user without saving it: the app draws its curve with your band notes. Then ask; after a \
             clear yes call set_taste_rule with the same scope. Values are validated and clamped (see `adjusted`).",
            taste_rule("Omit to keep the current curve."),
        ),
        tool(
            "set_taste_rule",
            "Save the taste layer for one genre, playlist, artist, album or track (one rule per scope; the most specific one \
             plays: track > album > playlist > artist > genre). Only after the user clearly said yes to it. Give `bands` (a \
             parametric curve, few bands, each within the intensity's caps) and / or `tilt` (broad knobs, each -6..6 dB). Omitted fields keep \
             the proposal on the table for that scope, else the saved rule's. The whole layer never boosts more than +12 dB.",
            taste_rule("Omit to keep the proposal's / saved curve; [] removes it."),
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

/// Plain phrases and the change each one usually needs at the Balanced
/// intensity (in context examples align text with EQ better than rules
/// alone: Aalborg/B&O, arXiv 2601.09448). The phrases are in the user's
/// language; [`intensity_ladder`] says how to scale them and the caps below
/// still apply.
fn phrase_examples(es: bool) -> &'static str {
    if es {
        "| Dicen | Bandas propias (`profile`) | Sobre AutoEq (`taste`) | Curva de una capa (`bands`, con su `note`) |
|---|---|---|---|
| \"le falta bajo\", \"suena flaco\" | lowShelf ~90 Hz +3..4 dB (built-in: peak 200 Hz +2..3, Q 0.9) | bassDb +3..4 | lowShelf 90 Hz +3.5 dB Q 0.7: \"más cuerpo abajo\"; peak 250 Hz -1.5 dB Q 1: \"sin embarrar\" |
| \"le falta pegada\", \"el bombo no golpea\" | peak 60-80 Hz +3..4 dB, Q ~1.2 | bassDb +3 | peak 70 Hz +3.5 dB Q 1.2: \"más golpe del bombo\"; peak 300 Hz -2 dB Q 1.2: \"lo mantiene limpio\" |
| \"retumba\", \"el bajo embarra\" | peak 80-200 Hz -3 dB, Q ~1 | bassDb -3 | peak 150 Hz -3 dB Q 1: \"menos retumbe\"; peak 300 Hz -2 dB Q 1.2: \"quita lo embarrado\" |
| \"suena frío\", \"le falta calidez\" | peak 150-250 Hz +2..3 dB, Q 0.7 | bassDb +2 | peak 180 Hz +2.5 dB Q 0.6 (ancho): \"calidez\" |
| \"chillón\", \"áspero\", \"las eses pican\" | peak 3-8 kHz -3 dB, Q ~2 (sibilance: 6-8 kHz) | trebleDb -3 | peak 3.2 kHz -3 dB Q 2.5: \"suaviza el filo de los platillos\"; eses: peak 7 kHz -3 dB Q 4: \"eses más suaves\" |
| \"apagado\", \"sin brillo\" | highShelf 8-10 kHz +3 dB | trebleDb +3 | highShelf 9 kHz +3 dB Q 0.7: \"más aire\"; peak 3 kHz +1.5 dB Q 1: \"más definición\" |
| \"las voces se oyen lejos\", \"no se oyen las voces\" | peak 1-3 kHz +2.5..3 dB, Q ~1 | clarityDb +3 | peak 2 kHz +3 dB Q 1: \"acerca las voces\"; peak 300 Hz -1.5 dB Q 1: \"más aire en las voces\" |
| \"me cansa a volumen alto\" | highShelf 10 kHz -1.5..2 dB | trebleDb -2, and suggest a lower volume (and set_loudness if songs jump in level) | peak 3 kHz -1.5 dB Q 1.5 y highShelf 10 kHz -1.5 dB: \"menos fatiga\" |
| \"el parlante distorsiona\", \"zumba\" | highPass 60-100 Hz | - | highPass 80 Hz Q 0.7: \"no le pide bajo que no puede dar\" |"
    } else {
        "| They say | Own bands (`profile`) | On top of AutoEq (`taste`) | A layer's curve (`bands`, with their `note`) |
|---|---|---|---|
| \"thin\", \"needs more bass\" | lowShelf ~90 Hz +3..4 dB (built-in: peak 200 Hz +2..3, Q 0.9) | bassDb +3..4 | lowShelf 90 Hz +3.5 dB Q 0.7: \"fuller low end\"; peak 250 Hz -1.5 dB Q 1: \"keeps it from getting muddy\" |
| \"no punch\", \"weak kick\" | peak 60-80 Hz +3..4 dB, Q ~1.2 | bassDb +3 | peak 70 Hz +3.5 dB Q 1.2: \"more kick\"; peak 300 Hz -2 dB Q 1.2: \"keeps it clean\" |
| \"boomy\", \"muddy bass\" | peak 80-200 Hz -3 dB, Q ~1 | bassDb -3 | peak 150 Hz -3 dB Q 1: \"less boom\"; peak 300 Hz -2 dB Q 1.2: \"clears the mud\" |
| \"cold\", \"lacks warmth\" | peak 150-250 Hz +2..3 dB, Q 0.7 | bassDb +2 | peak 180 Hz +2.5 dB Q 0.6 (wide): \"warmth\" |
| \"harsh\", \"shrill\", \"sibilant\" | peak 3-8 kHz -3 dB, Q ~2 (sibilance: 6-8 kHz) | trebleDb -3 | peak 3.2 kHz -3 dB Q 2.5: \"softens the harsh edge of the cymbals\"; sibilance: peak 7 kHz -3 dB Q 4: \"softer s's\" |
| \"dull\", \"muffled\" | highShelf 8-10 kHz +3 dB | trebleDb +3 | highShelf 9 kHz +3 dB Q 0.7: \"more air\"; peak 3 kHz +1.5 dB Q 1: \"more definition\" |
| \"vocals buried\", \"voices sound far away\" | peak 1-3 kHz +2.5..3 dB, Q ~1 | clarityDb +3 | peak 2 kHz +3 dB Q 1: \"brings voices forward\"; peak 300 Hz -1.5 dB Q 1: \"opens up the voices\" |
| \"fatiguing at high volume\" | highShelf 10 kHz -1.5..2 dB | trebleDb -2, and suggest a lower volume (and set_loudness if songs jump in level) | peak 3 kHz -1.5 dB Q 1.5 and highShelf 10 kHz -1.5 dB: \"less fatigue\" |
| \"the speaker distorts\", \"it buzzes\" | highPass 60-100 Hz | - | highPass 80 Hz Q 0.7: \"doesn't ask for bass it can't play\" |"
    }
}

/// Where each part of the spectrum lives and what people call it.
const FREQUENCY_VOCABULARY: &str = "| Range | What it is | People say |
|---|---|---|
| 20-60 Hz | sub-bass | rumble, \"the floor shakes\" |
| 60-120 Hz | punch, kick | punch, thump, \"the kick hits\" |
| 120-250 Hz | warmth, boom | warm, fat, boomy |
| 250-500 Hz | mud, boxiness | muddy, boxy, cluttered |
| 500 Hz-2 kHz | body (1-1.5 kHz: nasal) | full, hollow, nasal |
| 2-5 kHz | presence, attack | close, forward, harsh, shouty |
| 5-9 kHz | sibilance, edge | \"s\" hiss, sizzle, bite |
| 10 kHz and up | air | open, airy, dull when missing |";

/// Starting points per genre at the Balanced intensity (always begin from
/// analyze_sound, and never stack on top of the AutoEq device profile).
const GENRE_STARTING_POINTS: &str =
    "- Metal / nu-metal: lowShelf 90 Hz +3..4, peak 300 Hz -2..3 Q 1, peak 3 kHz +2..3 Q 1.2 \
(if it is harsh: -2..3 at 3-4 kHz instead), highShelf 10 kHz -1..2 if it is tiring.
- Rock: +2..3 at 100 Hz, +2 at 3-4 kHz, -2 at 8 kHz if it is harsh.
- EDM / house: lowShelf 70 Hz +4..5, peak 250 Hz -2 Q 1, highShelf 10 kHz +2..3.
- Hip-hop / trap / reggaetón: peak 55 Hz +4..5 Q 1, lowShelf 100 Hz +2, peak 2.5 kHz +1..2 Q 1.
- Pop: +2..3 at 100 Hz, +2 at 3 kHz, highShelf 12 kHz +2..3.
- Jazz / acoustic: peak 180 Hz +1.5..2 Q 0.6, highShelf 10 kHz +1.5..2, -1..2 at 3 kHz if it sounds forward.
- Classical: ±1..2 at most; suggest loudness normalization (set_loudness) instead of a big curve.
- Podcasts / voice: highPass 80 Hz, peak 250 Hz -2..3, peak 2.5 kHz +3..4 Q 1, peak 7 kHz -2..3 Q 4.";

/// How big the moves are at the listener's intensity.
fn intensity_ladder(intensity: TasteIntensity) -> String {
    let rows = TasteIntensity::ALL.map(|i| {
        let l = i.limits();
        format!(
            "| {}{} | {:.1}-{:.0} dB | x{} | ±{:.0} dB | ±{:.0} dB | +{:.0} dB |",
            i.id(),
            if i == intensity { " (the listener's)" } else { "" },
            l.main_move_db.0,
            l.main_move_db.1,
            i.scale(),
            l.band_db,
            l.tilt_db,
            l.boost_db
        )
    });
    format!(
        "| Intensity | Main move (the biggest band) | Scale of the examples | Per band | Tilt knobs | Whole curve boosts at most |
|---|---|---|---|---|---|
{}",
        rows.join("\n")
    )
}

fn system_prompt(state: &AssistantState) -> String {
    let ctx = &state.context;
    let language = if ctx.spanish() { "Spanish (neutral Latin American)" } else { "English" };
    let examples = phrase_examples(ctx.spanish());
    let intensity = ctx.taste.intensity;
    let ladder = intensity_ladder(intensity);
    let (vocabulary, genres) = (FREQUENCY_VOCABULARY, GENRE_STARTING_POINTS);
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
        "builtInSpeaker": ctx.on_small_speaker(),
        "equalizerOn": ctx.current.enabled,
        "intensity": intensity.id(),
        "notes": ctx.notes,
        "questionsAskedSoFar": state.user_turns,
        "proposalOnTheTable": state.proposal.as_ref().map(|p| json!({ "outputId": p.output_id, "name": p.profile.name })),
        "tasteProposalOnTheTable": state.taste_proposal.as_ref().map(|r| json!({ "scope": r.scope, "label": r.label })),
        "tasteLayers": ctx.taste.rules.len(),
        "refiningRule": ctx.focus().map(|r| json!({ "id": r.id, "label": r.label, "scope": r.scope, "tilt": r.tilt, "bands": r.bands })),
        "loudness": ctx.loudness.mode,
        "nowPlaying": ctx.now_playing.as_ref().map(|n| json!({
            "title": n.title, "album": n.album.as_ref().map(|a| &a.name),
            "artists": n.artists.iter().map(|a| &a.name).collect::<Vec<_>>(), "genres": n.genres,
            "from": n.source.as_ref().map(|s| json!({ "kind": s.kind, "id": s.id, "name": s.name })),
        })),
        "topGenres": ctx.listening.as_ref().map(|l| l.top_genres.iter().take(5).collect::<Vec<_>>()),
    });
    format!(
        "You are the sound engineer of HomeFi's listening room, a hi-fi music player. You help someone who is not an audio expert \
shape the sound of what they play. Your goal is a clearly audible change that matches what they asked for, and then to \
calibrate it with them: a change nobody can hear is a failed answer, and so is one that makes the music worse.

Language: always answer in {language}, whatever language the tools use. Plain, warm words; no jargon unless you explain it in a \
few words. Keep every message short (at most three sentences).

Listening context (the JSON at the end): their outputs and whether the equalizer is on; `builtInSpeaker`; the device profile \
of each output (`currentProfile`, an AutoEq correction when there is one: it already fixes the headphone towards a neutral \
target, so never stack your changes on top of what it corrects); and their `intensity`, how strong your proposals may be.

How to talk:
- One question at a time, and call suggest_replies with 2 to 4 short answers for it.
- At most {MAX_QUESTIONS} questions in total. Skip whatever you can infer: if there is only one output, don't ask which one; \
the output kind (Bluetooth, USB DAC, car...) tells you a lot; use the notes.
- Useful things to learn, in this order and only if they matter: which output they'll use; the exact headphone or speaker \
model (then search_headphones; if several match, ask which one; if none, say so and go on with taste alone); a DAC or amp \
only if relevant; the music they like; their taste (more bass, more clarity, less harsh, neutral); where they listen (home, \
street, car, noisy places); how loud they usually listen and whether highs feel tiring.
- When you know enough (or after {MAX_QUESTIONS} questions), call get_headphone_profile if there is a headphone, then propose_eq \
with it as `headphone_path` plus a `taste` that makes the difference they asked for (size it with the intensity table below). \
Without a measured headphone (car, speakers, built-in), propose only `taste` or a few broad bands.
- After proposing, explain in plain words what changes and ask whether to apply it. Call apply_eq only after a clear yes; if \
they want something different, propose again.

Built-in speakers (laptop, phone, kind built_in): they can't play deep bass. Never boost below 150 Hz there; for \"more bass\" \
use a modest lift around 150-250 Hz (at most +3 dB) and the app adds a high-pass at 90 Hz; there, prefer cuts for anything else. \
A car is different: it has real speakers.

Where things live in the spectrum (use these words with the user):
{vocabulary}

How big to go. The listener's intensity is {intensity_id}; the main move is the biggest band of your proposal, and the examples \
below are written for Balanced (scale by intensity: Subtle x0.6, Bold x1.5). The app narrows anything beyond these caps and \
lists it in `adjusted`.
{ladder}

What people say and what it usually needs (starting points at Balanced; two or three bands at a time, never past the caps):
{examples}

Taste layers (after the device step, or right away when the device is already set up):
- Taste layers change the sound for some music only: a genre, a playlist, an artist, an album or one track, on top of the \
output's profile. The most specific one plays (track > album > playlist > artist > genre). A layer is a simple `tilt` (bass, \
treble, clarity knobs) and / or a parametric curve of up to {MAX_TASTE_BANDS} `bands`: as precise as the user's own equalizer.
- Look at listening_summary and list_taste_rules, then offer at most two concrete layers that fit what they listen to (for \
example \"more bass for Electronic\"). Show a curve with propose_taste_rule (the app draws it with your band notes), \
say what it does in plain words and ask; call set_taste_rule only after a clear yes. Keep it to few questions; if they don't \
care, leave it.
- When the user opened you to refine one layer (`refiningRule`), work on that layer only.

Designing a curve (`bands` of propose_taste_rule / set_taste_rule):
- First call analyze_sound (with the scope) to see what already plays: the device profile, the layer's curve and the summed \
response. Edit a band that is already there (same kind, about the same frequency) instead of stacking another one on it, and \
never redo what the device profile already corrects.
- Think in zones and problems. Broad character (warmth, body, air, bass weight) is a shelf or a wide peak (Q 0.6-1); a specific \
problem (a harsh spot, sibilance, a resonance) is a narrower peak (Q 2-4). Never a surgical notch (Q above 4 is for sibilance \
only). Make the main move at least as big as the low end of the main-move range for the listener's intensity: a timid +1 dB \
is inaudible.
- Starting points by genre at Balanced (adapt them to what analyze_sound shows and to what the user said):
{genres}
- Cut when the complaint is a problem (harsh, boomy, muddy, tiring); boost when something is missing (bass, air, voices). The \
app lowers the preamp automatically so a boost never clips. Use few bands: usually 2 to 4. Peaks Q 0.3-8; the whole layer \
never boosts more than +{MAX_TASTE_BOOST_DB:.0} dB anywhere and no band passes ±{MAX_TASTE_BAND_DB:.0} dB.
- Explain each band in one plain line in the user's language, as its `note` and in your message: \"-3 dB at 3.2 kHz, narrow: \
softens the harsh edge of the cymbals\".
- After the layer is applied (a clear yes and set_taste_rule), ask ONE calibration question with suggest_replies: \"More\" / \
\"Less\" / \"Perfect\" (in the user's language). A tap on More is itself the yes: call analyze_sound, edit the SAME bands about \
x1.5 and call set_taste_rule again; Less is about x0.6 the same way; Perfect ends it. Don't ask anything else after that.
- The app validates every value and lists in `adjusted` what it had to change: tell the user when that matters.
- Loudness normalization (set_loudness) evens out songs that are much louder or quieter than the rest. Suggest it when they \
mention jumps in volume; it changes the level, so the sound is no longer bit-perfect.

Be honest:
- With the equalizer on, the sound is processed, so it is no longer bit-perfect. Say so once, simply, when you propose.
- Never promise \"studio quality\", miracles or a \"perfect\" sound. AutoEq corrections come from measurements of one unit \
towards an average target (Harman); fit and each person's ears change the result, so invite them to try it and adjust.
- The app caps boosts at +12 dB and lowers the preamp automatically so it never clips (device profile and taste layer together). Suggest moderate volume. If they mention \
pain, ringing or hearing loss, suggest lowering the volume and seeing a professional; you are not a doctor.
- Don't invent headphone measurements: use the tools.

What the app knows (JSON): {context}",
        intensity_id = intensity.id(),
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
        assert!(t.taste_change.is_none(), "nothing changes before a yes");
        assert_eq!(t.quick_replies, ["Sí", "No"]);
        let t = a.reply("Sí").await.unwrap();
        let change = t.taste_change.expect("rules changed");
        assert_eq!((change.upsert.len(), change.remove.len()), (1, 0));
        let r = &change.upsert[0];
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

    #[tokio::test]
    async fn taste_curves_are_proposed_analyzed_and_saved_after_a_yes() {
        let server = MockServer::start().await;
        let guetta = json!({"kind": "artist", "name": "David Guetta"});
        let curve = json!([
            {"kind": "peak", "freqHz": 3200, "gainDb": -2, "q": 3, "note": "suaviza el filo de los platillos"},
            {"kind": "lowShelf", "freqHz": 90, "gainDb": 2, "q": 0.7, "note": "más cuerpo abajo"},
            {"kind": "peak", "freqHz": 7000, "gainDb": -14, "q": 12, "note": "eses más suaves"},
            {"kind": "notch", "freqHz": 50},
        ]);
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let mut ctx = context();
        ctx.active_output = Some("dac".into());
        ctx.current.enabled = true;
        ctx.current.per_output.insert(
            "dac".into(),
            EqProfile {
                name: "HD 650".into(),
                bands: vec![EqBand::new(EqBandKind::LowShelf, 105.0, 6.0, 0.7)],
                ..Default::default()
            },
        );
        let mut a = EqAssistant::new(config, ctx);
        // drive the scripted turns directly with run_tool, as the model would
        let mut turn = AssistantTurn::default();
        let call = |id: &str, name: &str, input: Value| ToolCall { id: id.into(), name: name.into(), arguments: input };
        let (out, err, _) = a.run_tool(&call("a1", "analyze_sound", json!({"scope": guetta})), &mut turn).await;
        assert!(!err);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["response"].as_array().unwrap().len(), 31);
        assert_eq!(v["deviceProfile"]["name"], "HD 650");
        assert!(v["tasteRule"].is_null());
        assert!(v["response"][0]["device"].as_f64().unwrap() > 5.0, "{}", v["response"][0]);

        let (out, err, _) = a
            .run_tool(&call("a2", "propose_taste_rule", json!({"scope": guetta, "label": "Más pegada para Guetta", "explanation": "Menos filo, más cuerpo.", "bands": curve})), &mut turn)
            .await;
        assert!(!err, "{out}");
        let v: Value = serde_json::from_str(&out).unwrap();
        let adjusted: Vec<String> = serde_json::from_value(v["adjusted"].clone()).unwrap();
        assert!(adjusted.iter().any(|n| n.contains("notch")), "{adjusted:?}");
        assert!(adjusted.iter().any(|n| n.contains("-9 dB")), "{adjusted:?}");
        let proposal = turn.taste_proposal.clone().expect("a curve on the table");
        assert_eq!(proposal.bands.len(), 3);
        // -14 dB: the hard cap says -9, the Balanced intensity -7
        let balanced = TasteIntensity::Balanced.limits().band_db;
        assert!(balanced < MAX_TASTE_BAND_DB);
        assert_eq!((proposal.bands[2].band.gain_db, proposal.bands[2].band.q), (-balanced, 8.0));
        assert!(adjusted.iter().any(|n| n.contains("at this intensity")), "{adjusted:?}");
        assert_eq!(proposal.bands[0].note, "suaviza el filo de los platillos");
        assert!(a.state.context.taste.rules.is_empty(), "nothing saved yet");
        // analyze again: the proposal is what it looks at
        let (out, _, _) = a.run_tool(&call("a1b", "analyze_sound", json!({"scope": guetta})), &mut turn).await;
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["tasteRule"]["from"], "proposal on the table");
        assert_eq!(v["tasteRule"]["bands"].as_array().unwrap().len(), 3);

        // saving before the user answered is refused
        let (_, err, _) = a.run_tool(&call("a4", "set_taste_rule", json!({"scope": guetta})), &mut turn).await;
        assert!(err);
        a.state.user_turns = 1;
        let mut turn = AssistantTurn::default();
        let (out, err, _) = a.run_tool(&call("a5", "set_taste_rule", json!({"scope": guetta})), &mut turn).await;
        assert!(!err, "{out}");
        let change = turn.taste_change.clone().expect("saved");
        let r = &change.upsert[0];
        assert_eq!(
            (r.label.as_str(), r.explanation.as_str(), r.created_by),
            ("Más pegada para Guetta", "Menos filo, más cuerpo.", TasteCreator::Ai)
        );
        assert_eq!(r.bands, proposal.bands);
        assert!(a.state.taste_proposal.is_none());
        // and the next analysis sees the saved rule playing on top of the HD 650
        let (out, _, _) = a.run_tool(&call("a6", "analyze_sound", json!({"scope": guetta})), &mut turn).await;
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["tasteRule"]["from"], "saved rule for that scope");
        assert!(v["maxCutDb"].as_f64().unwrap() < -5.0);
        // a flat curve is refused
        let (_, err, _) = a
            .run_tool(&call("a7", "set_taste_rule", json!({"scope": guetta, "bands": [], "tilt": {}})), &mut turn)
            .await;
        assert!(err);
    }

    /// The bug "I delete it and it comes back": a turn used to carry a copy of
    /// every rule, made when the conversation began.
    #[tokio::test]
    async fn a_turn_reports_a_change_not_a_snapshot() {
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into()));
        let mut ctx = context();
        let old = ctx.taste.upsert(TasteRule::new(
            TasteScope::Genre { name: "Jazz".into() },
            Tilt { bass_db: 1.0, ..Default::default() },
            "Jazz",
            TasteCreator::Manual,
        ));
        let mut a = EqAssistant::new(config, ctx);
        a.state.user_turns = 1;
        let call = |id: &str, name: &str, input: Value| ToolCall { id: id.into(), name: name.into(), arguments: input };
        let rock = json!({"kind": "genre", "name": "Rock"});
        let mut turn = AssistantTurn::default();
        let (out, err, _) =
            a.run_tool(&call("c1", "set_taste_rule", json!({"scope": rock, "tilt": {"bassDb": 3}})), &mut turn).await;
        assert!(!err, "{out}");
        let change = turn.taste_change.clone().expect("a change");
        // only the rule of this turn: not the Jazz one the user already had
        assert_eq!((change.upsert.len(), change.remove.len()), (1, 0));
        assert_eq!(change.upsert[0].scope.name(), "Rock");

        // the user deletes the Jazz rule on the settings screen; merging the change leaves it deleted
        let mut now = TasteSettings::default();
        now.upsert(change.upsert[0].clone());
        now.apply(&change);
        assert_eq!(now.rules.len(), 1);
        assert!(!now.rules.iter().any(|r| r.id == old.id));

        // set then remove in one turn nets out to a removal; removing records the id
        let rock_id = change.upsert[0].id.clone();
        let (_, err, _) = a.run_tool(&call("c2", "remove_taste_rule", json!({"id": rock_id})), &mut turn).await;
        assert!(!err);
        let change = turn.taste_change.unwrap();
        assert!(change.upsert.is_empty() && change.remove == [rock_id]);
    }

    #[test]
    fn sync_replaces_the_copy() {
        let mut ctx = context();
        let rule = ctx.taste.upsert(TasteRule::new(
            TasteScope::Genre { name: "Jazz".into() },
            Tilt { bass_db: 1.0, ..Default::default() },
            "Jazz",
            TasteCreator::Manual,
        ));
        let mut session = EqSession::guided(ctx);
        // the rule is deleted, loudness and the equalizer changed since the conversation began
        let loudness = LoudnessSettings { mode: LoudnessMode::Album, ..Default::default() };
        let taste = TasteSettings { intensity: TasteIntensity::Bold, ..Default::default() };
        let eq = EqSettings { enabled: true, ..Default::default() };
        session.sync(taste.clone(), loudness, eq.clone());
        let EqSession::Guided(g) = &session else { panic!("guided") };
        let c = &g.state().context;
        assert!(c.taste.rule_for(&rule.scope).is_none() && c.taste == taste);
        assert_eq!((c.loudness, c.current.enabled), (loudness, true));
        // the AI conversation too, and the model sees the new intensity
        let mut session = EqSession::ai(AiConfig::new(AiProvider::Anthropic, "m", None), context());
        session.sync(taste, loudness, eq);
        let EqSession::Ai(a) = &session else { panic!("ai") };
        assert!(a.state().context.taste.rules.is_empty());
        assert!(system_prompt(a.state()).contains("\"intensity\":\"bold\""));
    }

    #[test]
    fn bands_from_the_model_are_read_leniently() {
        let (b, notes) = bands_arg(&json!(
            r#"[{"type":"HP","freq":80},{"kind":"bell","frequency":2000,"gain":1.5,"why":"voces"},{"gainDb":3}]"#
        ));
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].band.kind, b[0].band.freq_hz), (EqBandKind::HighPass, 80.0));
        assert_eq!(
            (b[1].band.kind, b[1].band.gain_db, b[1].band.q, b[1].note.as_str()),
            (EqBandKind::Peak, 1.5, 1.0, "voces")
        );
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn refining_a_rule_starts_from_it() {
        let mut ctx = context();
        let rule = ctx.taste.upsert(
            TasteRule::new(
                TasteScope::Artist { id: String::new(), name: "Bach".into() },
                Tilt::default(),
                "Bach suave",
                TasteCreator::Manual,
            )
            .with_bands(vec![TasteBand::new(EqBandKind::Peak, 3000.0, -2.0, 2.0)]),
        );
        ctx.focus_rule = Some(rule.id.clone());
        let state = AssistantState { context: ctx, ..Default::default() };
        let prompt = system_prompt(&state);
        assert!(prompt.contains("refiningRule") && prompt.contains("Bach suave"));
        assert!(prompt.contains("analyze_sound") && prompt.contains("never redo what the device profile"));
        assert!(prompt.contains("softens the harsh edge of the cymbals") || prompt.contains("suaviza el filo"));
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
            // Balanced starting points, and the caps still there
            for change in [
                "lowShelf ~90 Hz +3..4 dB",
                "peak 200 Hz",
                "peak 80-200 Hz -3 dB, Q ~1",
                "peak 3-8 kHz -3 dB",
                "highShelf 8-10 kHz +3 dB",
                "peak 1-3 kHz +2.5..3 dB",
            ] {
                assert!(prompt.contains(change), "{change}");
            }
            assert!(prompt.contains("set_loudness if songs jump"));
            assert!(prompt.contains("caps boosts at +12 dB") && prompt.contains("at most +3 dB"));
            // vocabulary, genre starting points and the intensity ladder
            for part in [
                "| 60-120 Hz | punch, kick |",
                "| 250-500 Hz | mud, boxiness |",
                "| 10 kHz and up | air |",
                "Metal / nu-metal: lowShelf 90 Hz +3..4",
                "Hip-hop / trap / reggaetón: peak 55 Hz +4..5 Q 1",
                "Podcasts / voice: highPass 80 Hz",
                "| subtle | 1.5-3 dB | x0.6 |",
                "| balanced (the listener's) | 3.0-5 dB | x1 |",
                "| bold | 5.0-8 dB | x1.5 |",
                "Subtle x0.6, Bold x1.5",
                "ONE calibration question",
            ] {
                assert!(prompt.contains(part), "{part}");
            }
            // no timid defaults any more
            for timid in ["gentle", "usually within ±3", "usually 1 to 4 dB", "Prefer cuts over boosts"] {
                assert!(!prompt.contains(timid), "{timid}");
            }
            assert!(prompt.contains("studio quality"));
        }
    }
}
