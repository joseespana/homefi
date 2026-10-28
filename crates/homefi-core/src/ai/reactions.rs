//! "How does it sound?": one tap (needs bass, too bright, perfect...) turns
//! into a taste rule for the music playing, with one line saying what changed
//! and an undo.
//!
//! Without AI the steps are fixed and safe ([`STEP_DB`] per tap, capped at
//! [`MAX_TILT_DB`]) and the rule goes to the rule shaping the music right now
//! (else the track's main genre, artist, album or the track). With AI one short call picks the scope
//! sensibly and words the line; its tilt is validated the same way, and any
//! failure falls back to the fixed steps.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::chat::{chat, Message, Tool};
use super::eq_assistant::{scope_arg, OutputKind};
use super::{resolve_model, AiConfig};
use crate::dsp::{
    resolve_taste, safe_tilt, LoudnessMode, LoudnessSettings, NowPlayingContext, TasteCreator, TasteRule, TasteScope,
    TasteSettings, Tilt,
};
use crate::error::Result;

/// dB changed per tap.
pub const STEP_DB: f32 = 2.0;
/// The AI may move each value at most this much per reaction.
const MAX_AI_STEP_DB: f32 = 3.0;
/// Target change (LUFS) per "too quiet" / "too loud".
const LOUDNESS_STEP: f32 = 2.0;
/// Undo entries kept.
const UNDO_DEPTH: usize = 20;

/// What the listener felt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoundReaction {
    NeedsBass,
    TooBoomy,
    /// Harsh, tiring highs.
    TooBright,
    /// Muffled, no sparkle.
    Dull,
    VocalsBuried,
    /// Some songs are quieter than the rest.
    TooQuiet,
    /// Some songs are louder than the rest.
    TooLoud,
    Perfect,
}

impl SoundReaction {
    pub const ALL: [SoundReaction; 8] = [
        Self::NeedsBass,
        Self::TooBoomy,
        Self::TooBright,
        Self::Dull,
        Self::VocalsBuried,
        Self::TooQuiet,
        Self::TooLoud,
        Self::Perfect,
    ];

    /// The fixed step of the reaction (zero for loudness and "perfect").
    pub fn step(self) -> Tilt {
        let t = |bass_db, treble_db, clarity_db| Tilt { bass_db, treble_db, clarity_db };
        match self {
            Self::NeedsBass => t(STEP_DB, 0.0, None),
            Self::TooBoomy => t(-STEP_DB, 0.0, None),
            Self::TooBright => t(0.0, -STEP_DB, None),
            Self::Dull => t(0.0, STEP_DB, None),
            Self::VocalsBuried => t(0.0, 0.0, Some(STEP_DB)),
            Self::TooQuiet | Self::TooLoud | Self::Perfect => Tilt::default(),
        }
    }

    /// Chip label.
    pub fn label(self, es: bool) -> &'static str {
        match (self, es) {
            (Self::NeedsBass, false) => "Needs more bass",
            (Self::NeedsBass, true) => "Le falta bajo",
            (Self::TooBoomy, false) => "Too boomy",
            (Self::TooBoomy, true) => "Retumba mucho",
            (Self::TooBright, false) => "Too bright / harsh",
            (Self::TooBright, true) => "Muy brillante / áspero",
            (Self::Dull, false) => "Dull, muffled",
            (Self::Dull, true) => "Apagado",
            (Self::VocalsBuried, false) => "Can't hear the vocals",
            (Self::VocalsBuried, true) => "No se oyen las voces",
            (Self::TooQuiet, false) => "Quieter than the rest",
            (Self::TooQuiet, true) => "Más bajo que el resto",
            (Self::TooLoud, false) => "Louder than the rest",
            (Self::TooLoud, true) => "Más fuerte que el resto",
            (Self::Perfect, false) => "Sounds perfect",
            (Self::Perfect, true) => "Suena perfecto",
        }
    }

    fn is_loudness(self) -> bool {
        matches!(self, Self::TooQuiet | Self::TooLoud)
    }
}

/// What a reaction knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ReactionContext {
    /// `es`, `en`...
    pub language: String,
    pub now_playing: NowPlayingContext,
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
    /// What the music plays on (built-in speakers get no deep bass).
    pub output_kind: OutputKind,
    /// Where the rule should go (a "Sound for this album" screen); `None`
    /// ("Auto"): see [`ReactionContext::default_scope`].
    pub scope_hint: Option<TasteScope>,
}

impl ReactionContext {
    fn es(&self) -> bool {
        self.language.to_lowercase().starts_with("es")
    }

    fn small_speaker(&self) -> bool {
        self.output_kind == OutputKind::BuiltIn
    }

    /// "Auto": the hint; else the scope of the taste rule playing now (what
    /// the listener hears is what they react to); else the main genre,
    /// artist, album, then the track. Same as the apps' "Auto" choice.
    pub fn default_scope(&self) -> Option<TasteScope> {
        if let Some(h) = &self.scope_hint {
            return Some(h.clone());
        }
        let scopes = self.now_playing.scopes();
        if let Some((rule, _)) = resolve_taste(&self.taste, &self.now_playing) {
            // the playing music's own scope (with its ids) when it is the same
            return Some(scopes.iter().find(|s| s.same_as(&rule.scope)).cloned().unwrap_or_else(|| rule.scope.clone()));
        }
        ["genre", "artist", "album", "track", "playlist"]
            .iter()
            .find_map(|k| scopes.iter().find(|s| s.kind() == *k).cloned())
    }
}

/// What a reaction changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ReactionResult {
    /// The rule made or changed (none for loudness and "perfect").
    pub rule: Option<TasteRule>,
    /// Every rule after the change: save and apply.
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
    /// One line in the user's language ("More bass for Electronic (+2 dB)").
    pub message: String,
    /// Give it to [`UndoStack::undo`] to put everything back. Empty: nothing changed.
    pub undo_token: String,
    /// The AI chose the change (else the fixed steps).
    pub by_ai: bool,
}

/// The fixed, safe reaction (no AI).
pub fn react_guided(reaction: SoundReaction, ctx: &ReactionContext) -> ReactionResult {
    let es = ctx.es();
    let mut out = ReactionResult { taste: ctx.taste.clone(), loudness: ctx.loudness, ..Default::default() };
    if reaction == SoundReaction::Perfect {
        out.message = match resolve_taste(&ctx.taste, &ctx.now_playing) {
            Some((r, _)) if es => format!("¡Qué bien! Se queda así («{}»).", r.label),
            Some((r, _)) => format!("Great, it stays as it is (“{}”).", r.label),
            None if es => "¡Qué bien! No cambio nada.".into(),
            None => "Great, nothing to change.".into(),
        };
        return out;
    }
    if reaction.is_loudness() {
        out.loudness = loudness_step(reaction, ctx.loudness);
        out.message = loudness_message(reaction, ctx.loudness, out.loudness, es);
        out.undo_token = new_token();
        return out;
    }
    let Some(scope) = ctx.default_scope() else {
        out.message =
            if es { "No sé qué está sonando; dale play primero." } else { "Nothing is playing; press play first." }
                .into();
        return out;
    };
    let base = base_tilt(ctx, &scope);
    let tilt = safe_tilt(base.plus(reaction.step()), ctx.small_speaker());
    let rule = rule_for(ctx, scope, tilt, None, TasteCreator::Reaction);
    finish(&mut out, rule, base, es);
    out
}

/// The reaction with AI: one short call that chooses the scope and words the
/// line (`set_taste_rule`), validated like [`react_guided`]; on any failure
/// (no key, offline, no tool call) the fixed steps answer instead.
pub async fn react(reaction: SoundReaction, ctx: &ReactionContext, config: Option<&AiConfig>) -> ReactionResult {
    let Some(config) = config.filter(|_| reaction != SoundReaction::Perfect && !reaction.is_loudness()) else {
        return react_guided(reaction, ctx);
    };
    match react_ai(reaction, ctx, config).await {
        Ok(Some(r)) => r,
        Ok(None) => react_guided(reaction, ctx),
        Err(e) => {
            tracing::warn!("AI reaction failed, using the fixed steps: {e}");
            react_guided(reaction, ctx)
        }
    }
}

async fn react_ai(reaction: SoundReaction, ctx: &ReactionContext, config: &AiConfig) -> Result<Option<ReactionResult>> {
    let mut config = config.clone();
    if config.model.trim().is_empty() {
        config.model = resolve_model(&config).await?;
    }
    let es = ctx.es();
    let guided = react_guided(reaction, ctx);
    let facts = json!({
        "reaction": reaction,
        "nowPlaying": ctx.now_playing,
        "scopeHint": ctx.scope_hint,
        "builtInSpeaker": ctx.small_speaker(),
        "rules": ctx.taste.rules,
        "suggestedByFixedSteps": guided.rule.as_ref().map(|r| json!({ "scope": r.scope, "tilt": r.tilt })),
    });
    let system = reaction_system(es, &facts);
    let tool = Tool {
        name: "set_taste_rule".into(),
        description: "Create or update the taste rule for one scope.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["genre", "playlist", "artist", "album", "track"] },
                        "id": { "type": "string" },
                        "name": { "type": "string" }
                    },
                    "required": ["kind", "name"]
                },
                "tilt": {
                    "type": "object",
                    "properties": {
                        "bassDb": { "type": "number" }, "trebleDb": { "type": "number" }, "clarityDb": { "type": "number" }
                    }
                },
                "label": { "type": "string", "description": "Short rule name in the user's language" },
                "message": { "type": "string", "description": "One line for the user" }
            },
            "required": ["scope", "tilt", "message"]
        }),
    };
    let kick = if es { "(El oyente tocó una reacción.)" } else { "(The listener tapped a reaction.)" };
    let reply = chat(&config, &system, &[Message::User { text: kick.into() }], std::slice::from_ref(&tool)).await?;
    let Some(call) = reply.tool_calls.into_iter().find(|c| c.name == "set_taste_rule") else { return Ok(None) };
    let args = &call.arguments;
    let Ok(scope) = scope_arg(args.get("scope")) else { return Ok(None) };
    let scope = valid_scope(scope, ctx).or_else(|| ctx.default_scope());
    let Some(scope) = scope else { return Ok(None) };
    let base = base_tilt(ctx, &scope);
    let wanted = args.get("tilt").map(tilt_arg).unwrap_or_default();
    // at most a few dB per tap, in range, safe for the speaker
    let step = |from: f32, to: f32| from + (to - from).clamp(-MAX_AI_STEP_DB, MAX_AI_STEP_DB);
    let tilt = Tilt {
        bass_db: step(base.bass_db, wanted.bass_db),
        treble_db: step(base.treble_db, wanted.treble_db),
        clarity_db: match (base.clarity_db, wanted.clarity_db) {
            (None, None) => None,
            (b, w) => Some(step(b.unwrap_or(0.0), w.unwrap_or(b.unwrap_or(0.0)))),
        },
    };
    let tilt = safe_tilt(tilt, ctx.small_speaker());
    if tilt == base {
        return Ok(None);
    }
    let label = args.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty());
    let rule = rule_for(ctx, scope, tilt, label.map(|l| l.chars().take(60).collect()), TasteCreator::Ai);
    let mut out =
        ReactionResult { taste: ctx.taste.clone(), loudness: ctx.loudness, by_ai: true, ..Default::default() };
    finish(&mut out, rule, base, es);
    if let Some(m) = args.get("message").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty()) {
        out.message = m.lines().next().unwrap_or(m).chars().take(140).collect();
    }
    Ok(Some(out))
}

/// Each reaction with its label in the user's language and the usual change
/// (in-context examples align text with EQ better: arXiv 2601.09448).
fn reaction_examples(es: bool) -> String {
    let rows = [
        (
            SoundReaction::NeedsBass,
            "bassDb +2..3 (built-in speaker: the app lifts 200 Hz instead, at most +3 in total)",
        ),
        (SoundReaction::TooBoomy, "bassDb -2..3"),
        (SoundReaction::TooBright, "trebleDb -2 (and clarityDb -1 if the voices are the harsh part)"),
        (SoundReaction::Dull, "trebleDb +2"),
        (SoundReaction::VocalsBuried, "clarityDb +1.5..2"),
    ];
    rows.iter()
        .map(|(r, change)| {
            let id = serde_json::to_value(r).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            format!("- {id} (\"{}\"): {change}", r.label(es))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn reaction_system(es: bool, facts: &Value) -> String {
    let language = if es { "Spanish (neutral Latin American)" } else { "English" };
    let examples = reaction_examples(es);
    format!(
        "You adjust the sound of HomeFi, a music player, from one tap of the listener (\"how does it sound?\"). Call \
set_taste_rule exactly once: pick the scope (the scope hint if there is one; else usually the main genre; the artist, album \
or track when the rules show the genre already sounds right for other music), and the new tilt: the current one of that scope \
(or the rule in effect) moved 1 to 3 dB in the direction of the reaction. Bass is a low shelf at 105 Hz, treble a high shelf \
at 10 kHz, clarity a wide peak at 3 kHz. On a built-in speaker never add more than +3 dB of bass. Usual changes per tap \
(gentle; the app allows at most {MAX_AI_STEP_DB} dB per tap):
{examples}
`message`: one short line in {language} saying what changed and for which music. Facts (JSON): {facts}"
    )
}

/// The model's scope, if it is something that plays now (or the hint).
fn valid_scope(scope: TasteScope, ctx: &ReactionContext) -> Option<TasteScope> {
    if ctx.scope_hint.as_ref().is_some_and(|h| h.same_as(&scope)) {
        return ctx.scope_hint.clone();
    }
    // a scope of what plays: use the app's own (with ids)
    ctx.now_playing
        .scopes()
        .into_iter()
        .chain(ctx.now_playing.genres.iter().map(|g| TasteScope::Genre { name: g.clone() }))
        .find(|s| s.same_as(&scope))
}

/// The tilt to start from: the scope's own rule, else the one playing now
/// (so a narrower rule doesn't lose what a broader one did).
fn base_tilt(ctx: &ReactionContext, scope: &TasteScope) -> Tilt {
    match ctx.taste.rule_for(scope) {
        Some(r) => r.tilt,
        None => resolve_taste(&ctx.taste, &ctx.now_playing).map(|(_, t)| t).unwrap_or_default(),
    }
}

fn rule_for(
    ctx: &ReactionContext,
    scope: TasteScope,
    tilt: Tilt,
    label: Option<String>,
    by: TasteCreator,
) -> TasteRule {
    let es = ctx.es();
    let label = label.unwrap_or_else(|| describe(&scope, tilt, es));
    match ctx.taste.rule_for(&scope) {
        // keep its id and who made it first (a manual rule stays manual)
        Some(old) => TasteRule { tilt, label, enabled: true, ..old.clone() },
        None => TasteRule::new(scope, tilt, label, by),
    }
}

fn finish(out: &mut ReactionResult, rule: TasteRule, before: Tilt, es: bool) {
    let stored = out.taste.upsert(rule);
    out.taste.enabled = true;
    let change = Tilt {
        bass_db: stored.tilt.bass_db - before.bass_db,
        treble_db: stored.tilt.treble_db - before.treble_db,
        clarity_db: Some(stored.tilt.clarity_db.unwrap_or(0.0) - before.clarity_db.unwrap_or(0.0)),
    };
    let name = stored.scope.name().to_owned();
    out.message = if change.is_neutral() {
        if es {
            format!("Ya está al máximo seguro para {name}.")
        } else {
            format!("That's already the safe limit for {name}.")
        }
    } else if es {
        format!("{} para {name}.", capitalize(&tilt_words(change, true)))
    } else {
        format!("{} for {name}.", capitalize(&tilt_words(change, false)))
    };
    out.rule = Some(stored);
    out.undo_token = new_token();
}

fn loudness_step(reaction: SoundReaction, l: LoudnessSettings) -> LoudnessSettings {
    if l.mode == LoudnessMode::Off {
        return LoudnessSettings { mode: LoudnessMode::Auto, ..l }.clamped();
    }
    let delta = if reaction == SoundReaction::TooQuiet { LOUDNESS_STEP } else { -LOUDNESS_STEP };
    LoudnessSettings { target_lufs: l.target_lufs + delta, ..l }.clamped()
}

fn loudness_message(reaction: SoundReaction, before: LoudnessSettings, after: LoudnessSettings, es: bool) -> String {
    match (before.mode == LoudnessMode::Off, es) {
        (true, true) => "Activé la normalización: las canciones suenan parejas (ya no es bit-perfect).".into(),
        (true, false) => {
            "Turned on loudness normalization: songs play at an even level (no longer bit-perfect).".into()
        }
        (false, _) if after.target_lufs == before.target_lufs => {
            if es { "Ya está en el límite." } else { "That's already the limit." }.into()
        }
        (false, true) => format!(
            "Normalización {} a {:.0} LUFS.",
            if reaction == SoundReaction::TooQuiet { "subida" } else { "bajada" },
            after.target_lufs
        ),
        (false, false) => format!(
            "Normalization {} to {:.0} LUFS.",
            if reaction == SoundReaction::TooQuiet { "raised" } else { "lowered" },
            after.target_lufs
        ),
    }
}

fn tilt_arg(v: &Value) -> Tilt {
    let n = |k: &str, alt: &str| v.get(k).or_else(|| v.get(alt)).and_then(Value::as_f64).map(|x| x as f32);
    Tilt {
        bass_db: n("bassDb", "bass_db").unwrap_or(0.0),
        treble_db: n("trebleDb", "treble_db").unwrap_or(0.0),
        clarity_db: n("clarityDb", "clarity_db"),
    }
}

/// `+2 dB bass, -1 dB treble` / `+2 dB de bajo, -1 dB de agudos`.
pub fn tilt_words(t: Tilt, es: bool) -> String {
    let mut parts = Vec::new();
    let mut push = |db: f32, en: &str, es_word: &str| {
        if db.abs() >= 0.05 {
            let v = format!("{db:+.1}").replace(".0", "");
            parts.push(if es { format!("{v} dB de {es_word}") } else { format!("{v} dB {en}") });
        }
    };
    push(t.bass_db, "bass", "bajo");
    push(t.treble_db, "treble", "agudos");
    push(t.clarity_db.unwrap_or(0.0), "clarity", "claridad");
    if parts.is_empty() {
        return if es { "sin cambios" } else { "no change" }.into();
    }
    parts.join(", ")
}

/// A rule's default label: "More bass · Electronic" / "Más bajo · Electronic".
pub fn describe(scope: &TasteScope, t: Tilt, es: bool) -> String {
    let c = t.clarity_db.unwrap_or(0.0);
    let biggest = [(t.bass_db.abs(), 0), (t.treble_db.abs(), 1), (c.abs(), 2)]
        .into_iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
        .unwrap_or(0);
    let what = match (biggest, es) {
        (0, false) if t.bass_db >= 0.0 => "More bass",
        (0, false) => "Less bass",
        (1, false) if t.treble_db >= 0.0 => "Brighter",
        (1, false) => "Softer highs",
        (_, false) if c >= 0.0 => "Clearer vocals",
        (_, false) => "Smoother mids",
        (0, true) if t.bass_db >= 0.0 => "Más bajo",
        (0, true) => "Menos bajo",
        (1, true) if t.treble_db >= 0.0 => "Más brillo",
        (1, true) => "Agudos más suaves",
        (_, true) if c >= 0.0 => "Voces más claras",
        (_, true) => "Medios más suaves",
    };
    format!("{what} · {}", scope.name())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn new_token() -> String {
    format!("u{}", crate::dsp::taste::new_id())
}

/// What a reaction replaced, to put it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundSnapshot {
    pub taste: TasteSettings,
    pub loudness: LoudnessSettings,
}

/// The last few reactions' "before", by undo token (for the apps to hold).
#[derive(Debug, Default)]
pub struct UndoStack {
    entries: Mutex<VecDeque<(String, SoundSnapshot)>>,
}

impl UndoStack {
    /// Remembers `before` under `token` (an empty token is ignored).
    pub fn push(&self, token: &str, before: SoundSnapshot) {
        if token.is_empty() {
            return;
        }
        let mut e = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        e.push_back((token.to_owned(), before));
        while e.len() > UNDO_DEPTH {
            e.pop_front();
        }
    }

    /// What `token` replaced; `None` when unknown or already undone.
    pub fn undo(&self, token: &str) -> Option<SoundSnapshot> {
        let mut e = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let at = e.iter().position(|(t, _)| t == token)?;
        e.remove(at).map(|(_, s)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiProvider;
    use crate::dsp::{ItemRef, SourceContext, MAX_TILT_DB};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn ctx(lang: &str) -> ReactionContext {
        ReactionContext {
            language: lang.into(),
            now_playing: NowPlayingContext {
                track_id: Some("11".into()),
                title: Some("Around the World".into()),
                album: Some(ItemRef::new("22", "Homework")),
                artists: vec![ItemRef::named("Daft Punk")],
                genres: vec!["Electronic".into(), "House".into()],
                source: Some(SourceContext::new("playlist", "p1", "Friday")),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn guided_steps_go_to_the_main_genre_and_are_capped() {
        let mut c = ctx("en");
        let r = react_guided(SoundReaction::NeedsBass, &c);
        let rule = r.rule.clone().unwrap();
        assert_eq!(rule.scope, TasteScope::Genre { name: "Electronic".into() });
        assert_eq!(rule.tilt.bass_db, STEP_DB);
        assert_eq!(rule.created_by, TasteCreator::Reaction);
        assert!(r.taste.enabled && !r.undo_token.is_empty() && !r.by_ai);
        assert_eq!(r.message, "+2 dB bass for Electronic.");
        // tap after tap: capped at ±6
        for _ in 0..5 {
            c.taste = react_guided(SoundReaction::NeedsBass, &c).taste;
        }
        assert_eq!(c.taste.rules.len(), 1);
        assert_eq!(c.taste.rules[0].tilt.bass_db, MAX_TILT_DB);
        let r = react_guided(SoundReaction::NeedsBass, &c);
        assert!(r.message.contains("safe limit"), "{}", r.message);
        // the opposite reaction goes back down
        let r = react_guided(SoundReaction::TooBoomy, &c);
        assert_eq!(r.rule.unwrap().tilt.bass_db, MAX_TILT_DB - STEP_DB);
    }

    #[test]
    fn a_narrower_scope_starts_from_what_plays() {
        let mut c = ctx("es");
        c.taste = react_guided(SoundReaction::NeedsBass, &c).taste;
        c.scope_hint = Some(TasteScope::Album { id: "22".into(), name: "Homework".into() });
        let r = react_guided(SoundReaction::TooBright, &c);
        let rule = r.rule.unwrap();
        assert_eq!(rule.scope.kind(), "album");
        // keeps the genre's +2 bass, adds -2 treble
        assert_eq!((rule.tilt.bass_db, rule.tilt.treble_db), (2.0, -2.0));
        assert_eq!(r.message, "-2 dB de agudos para Homework.");
        assert_eq!(r.taste.rules.len(), 2);
    }

    #[test]
    fn auto_goes_to_the_rule_playing_now() {
        let mut c = ctx("en");
        // the listener had made this album its own sound
        c.taste = react_guided(
            SoundReaction::NeedsBass,
            &ReactionContext {
                scope_hint: Some(TasteScope::Album { id: "22".into(), name: "Homework".into() }),
                ..c.clone()
            },
        )
        .taste;
        // "Auto" changes that album rule, not the genre
        let r = react_guided(SoundReaction::NeedsBass, &c);
        let rule = r.rule.unwrap();
        assert_eq!(rule.scope, TasteScope::Album { id: "22".into(), name: "Homework".into() });
        assert_eq!(rule.tilt.bass_db, 2.0 * STEP_DB);
        assert_eq!(r.taste.rules.len(), 1);

        // a rule that is switched off doesn't play: back to the main genre
        c.taste.rules[0].enabled = false;
        assert_eq!(c.default_scope(), Some(TasteScope::Genre { name: "Electronic".into() }));
        // nor do rules while the layers are off
        c.taste.rules[0].enabled = true;
        c.taste.enabled = false;
        assert_eq!(c.default_scope(), Some(TasteScope::Genre { name: "Electronic".into() }));
    }

    #[test]
    fn small_speakers_cap_the_bass() {
        let mut c = ctx("en");
        c.output_kind = OutputKind::BuiltIn;
        for _ in 0..4 {
            c.taste = react_guided(SoundReaction::NeedsBass, &c).taste;
        }
        assert_eq!(c.taste.rules[0].tilt.bass_db, crate::dsp::SMALL_SPEAKER_MAX_BASS_DB);
    }

    #[test]
    fn loudness_perfect_and_undo() {
        let c = ctx("en");
        let r = react_guided(SoundReaction::TooQuiet, &c);
        assert_eq!(r.loudness.mode, LoudnessMode::Auto);
        assert!(r.rule.is_none() && r.message.contains("bit-perfect"));
        let mut c2 = c.clone();
        c2.loudness = r.loudness;
        assert_eq!(react_guided(SoundReaction::TooLoud, &c2).loudness.target_lufs, -16.0);
        let p = react_guided(SoundReaction::Perfect, &c);
        assert!(p.undo_token.is_empty() && p.rule.is_none());

        let undo = UndoStack::default();
        let r = react_guided(SoundReaction::Dull, &c);
        undo.push(&r.undo_token, SoundSnapshot { taste: c.taste.clone(), loudness: c.loudness });
        let back = undo.undo(&r.undo_token).unwrap();
        assert_eq!(back.taste, c.taste);
        assert!(undo.undo(&r.undo_token).is_none(), "once");
        // nothing playing
        let none = react_guided(SoundReaction::NeedsBass, &ReactionContext::default());
        assert!(none.rule.is_none() && none.undo_token.is_empty());
    }

    #[tokio::test]
    async fn ai_picks_the_scope_and_is_capped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "tool_use", "id": "c1", "name": "set_taste_rule", "input": {
                    "scope": {"kind": "artist", "name": "daft punk"},
                    "tilt": {"bassDb": 12},
                    "label": "Daft Punk con más bajo",
                    "message": "Le subí el bajo a Daft Punk.\nOtra línea"
                }}],
                "stop_reason": "tool_use"
            })))
            .mount(&server)
            .await;
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let r = react(SoundReaction::NeedsBass, &ctx("es"), Some(&config)).await;
        assert!(r.by_ai);
        let rule = r.rule.unwrap();
        assert_eq!(rule.scope, TasteScope::Artist { id: String::new(), name: "Daft Punk".into() });
        assert_eq!(rule.tilt.bass_db, MAX_AI_STEP_DB, "at most 3 dB per tap");
        assert_eq!(rule.created_by, TasteCreator::Ai);
        assert_eq!(r.message, "Le subí el bajo a Daft Punk.");
    }

    #[tokio::test]
    async fn ai_failures_fall_back_to_the_fixed_steps() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let r = react(SoundReaction::VocalsBuried, &ctx("en"), Some(&config)).await;
        assert!(!r.by_ai);
        assert_eq!(r.rule.unwrap().tilt.clarity_db, Some(STEP_DB));
        // a scope that isn't playing is replaced by the default one
        let c = ctx("en");
        assert!(valid_scope(TasteScope::Artist { id: String::new(), name: "Bach".into() }, &c).is_none());
    }

    #[test]
    fn words_and_labels() {
        let t = Tilt { bass_db: 2.0, treble_db: -1.5, clarity_db: None };
        assert_eq!(tilt_words(t, false), "+2 dB bass, -1.5 dB treble");
        assert_eq!(describe(&TasteScope::Genre { name: "Rock".into() }, t, true), "Más bajo · Rock");
        assert_eq!(SoundReaction::ALL.len(), 8);
        assert_eq!(serde_json::to_value(SoundReaction::TooBright).unwrap(), "tooBright");
    }

    #[test]
    fn the_prompt_has_examples_in_the_users_language() {
        let facts = json!({ "reaction": "needsBass" });
        let es = reaction_system(true, &facts);
        for line in [
            "needsBass (\"Le falta bajo\"): bassDb +2..3",
            "tooBoomy (\"Retumba mucho\"): bassDb -2..3",
            "tooBright (\"Muy brillante / áspero\"): trebleDb -2",
            "dull (\"Apagado\"): trebleDb +2",
            "vocalsBuried (\"No se oyen las voces\"): clarityDb +1.5..2",
        ] {
            assert!(es.contains(line), "{line}\n{es}");
        }
        assert!(es.contains("Spanish") && es.contains("at most 3 dB per tap"));
        let en = reaction_system(false, &facts);
        for line in [
            "(\"Needs more bass\")",
            "(\"Too boomy\")",
            "(\"Too bright / harsh\")",
            "(\"Dull, muffled\")",
            "(\"Can't hear the vocals\")",
        ] {
            assert!(en.contains(line), "{line}");
        }
        assert!(!en.contains("Le falta bajo") && en.contains("never add more than +3 dB of bass"));
    }
}
