//! "How does it sound?": one tap (needs bass, too bright, perfect...) turns
//! into a taste rule for the music playing, with one line saying what changed
//! and an undo.
//!
//! Without AI the steps are fixed and safe ([`STEP_DB`] per tap at the
//! Balanced intensity, scaled by [`crate::dsp::TasteIntensity::scale`], capped at
//! [`MAX_TILT_DB`]) and the rule goes to the rule shaping the music right now
//! (else the track's main genre, artist, album or the track). A step is a
//! tilt knob, or one well-placed band where a knob is too blunt ("too boomy"
//! cuts 120 Hz, "too bright" softens 8 kHz and the harsh 3.5 kHz edge). With
//! AI one short call picks the scope, the change (tilt and / or curve bands)
//! and words the line; it is validated the same way (at most
//! [`TasteLimits::step_db`] per knob or band and [`TasteLimits::bands_per_tap`]
//! bands per tap, from the listener's intensity), and any failure falls back
//! to the fixed steps.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::chat::{chat, Message, Tool};
use super::eq_assistant::{scope_arg, OutputKind};
use super::{resolve_model, AiConfig};
use crate::dsp::taste::hz;
use crate::dsp::{
    clamp_taste_curve, resolve_taste, safe_tilt, EqBand, EqBandKind, LoudnessMode, LoudnessSettings, NowPlayingContext,
    TasteBand, TasteChange, TasteCreator, TasteLimits, TasteRule, TasteScope, TasteSettings, Tilt, MAX_TASTE_BANDS,
    MAX_TILT_DB,
};
use crate::error::Result;

pub use crate::dsp::taste::tilt_words;

/// dB changed per fixed tap at the Balanced intensity.
pub const STEP_DB: f32 = 2.0;
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

impl ReactionResult {
    /// What this reaction changed in the rules, to merge into the settings as
    /// they are when it is saved (see [`TasteSettings::apply`]): the rule it
    /// made or edited, with the layers switched on. Empty when nothing changed.
    pub fn taste_change(&self) -> TasteChange {
        match &self.rule {
            Some(rule) => TasteChange { enabled: Some(true), ..TasteChange::upserting(rule.clone()) },
            None => TasteChange::default(),
        }
    }
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
    let (base, base_bands) = base_curve(ctx, &scope);
    // the listener's intensity makes every fixed step lighter or stronger
    let k = ctx.taste.intensity.scale();
    let tilt_step = |step: Tilt| (safe_tilt(base.plus(step.scaled(k)), ctx.small_speaker()), base_bands.clone());
    let (tilt, bands) = match guided_move(reaction, base, &base_bands) {
        Move::Tilt(step) => tilt_step(step),
        Move::Band(spot, delta) => match band_step(&base_bands, spot, delta * k, es) {
            Some(bands) => (base, bands),
            // a full curve with nothing at that place: the knob instead
            None => tilt_step(reaction.step()),
        },
    };
    let rule = rule_for(ctx, scope, tilt, bands, None, TasteCreator::Reaction);
    finish(&mut out, rule, base, &base_bands, es);
    out
}

/// A fixed place for a guided band (kind, frequency, Q).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spot(EqBandKind, f32, f32);

/// "Too boomy": the boom lives around 120 Hz.
const BOOM: Spot = Spot(EqBandKind::Peak, 120.0, 1.0);
/// "Too bright": the top octave and a half, gently.
const BRIGHT: Spot = Spot(EqBandKind::HighShelf, 8_000.0, 0.7);
/// The harsh edge (cymbals, "s", strained voices).
const HARSH: Spot = Spot(EqBandKind::Peak, 3_500.0, 2.0);

impl Spot {
    fn band(self, gain_db: f32) -> EqBand {
        EqBand::new(self.0, self.1, gain_db, self.2)
    }

    /// Gain of the curve's band at this place (0 when there is none).
    fn gain_in(self, bands: &[TasteBand]) -> f32 {
        let me = self.band(0.0);
        bands.iter().find(|b| b.band.enabled && b.same_place(&me)).map_or(0.0, |b| b.band.gain_db)
    }

    fn note(self, cut: bool, es: bool) -> &'static str {
        match (self, cut, es) {
            (BOOM, true, false) => "less boom",
            (BOOM, true, true) => "menos retumbe",
            (BRIGHT, true, false) => "softer highs",
            (BRIGHT, true, true) => "agudos más suaves",
            (HARSH, true, false) => "softens the harsh edge",
            (HARSH, true, true) => "suaviza la aspereza",
            _ => "",
        }
    }
}

enum Move {
    Tilt(Tilt),
    Band(Spot, f32),
}

/// The fixed step of a reaction: undo the opposite change first (a bass
/// boost before cutting the boom...), then a knob or a well-placed band.
fn guided_move(reaction: SoundReaction, tilt: Tilt, bands: &[TasteBand]) -> Move {
    let knob = Move::Tilt(reaction.step());
    match reaction {
        SoundReaction::NeedsBass if BOOM.gain_in(bands) < 0.0 => Move::Band(BOOM, STEP_DB),
        SoundReaction::TooBoomy if tilt.bass_db <= 0.0 => Move::Band(BOOM, -STEP_DB),
        SoundReaction::TooBright if tilt.treble_db <= 0.0 => {
            // spread it: the shelf first, then the harsh edge, alternating
            let (shelf, edge) = (BRIGHT.gain_in(bands), HARSH.gain_in(bands));
            let at_limit = |g: f32| g <= -MAX_TILT_DB + 0.05;
            let spot = match (at_limit(shelf), at_limit(edge)) {
                (false, true) => BRIGHT,
                (true, false) => HARSH,
                _ if shelf.abs() <= edge.abs() => BRIGHT,
                _ => HARSH,
            };
            Move::Band(spot, -STEP_DB)
        }
        SoundReaction::Dull if BRIGHT.gain_in(bands) < 0.0 => Move::Band(BRIGHT, STEP_DB),
        SoundReaction::Dull if HARSH.gain_in(bands) < 0.0 => Move::Band(HARSH, STEP_DB),
        SoundReaction::VocalsBuried if HARSH.gain_in(bands) < 0.0 => Move::Band(HARSH, STEP_DB),
        _ => knob,
    }
}

/// `bands` with `delta` dB at `spot` (joining the band already there, within
/// ±[`MAX_TILT_DB`]; a band back at 0 is removed). `None` when the curve is
/// full and has no band there.
fn band_step(bands: &[TasteBand], spot: Spot, delta: f32, es: bool) -> Option<Vec<TasteBand>> {
    let me = spot.band(0.0);
    let mut out = bands.to_vec();
    match out.iter().position(|b| b.band.enabled && b.same_place(&me)) {
        Some(i) => {
            let g = (out[i].band.gain_db + delta).clamp(-MAX_TILT_DB, MAX_TILT_DB);
            if g.abs() < 0.05 {
                out.remove(i);
            } else {
                out[i].band.gain_db = g;
            }
        }
        None if out.len() < MAX_TASTE_BANDS => {
            out.push(
                TasteBand::from(spot.band(delta.clamp(-MAX_TILT_DB, MAX_TILT_DB)))
                    .with_note(spot.note(delta < 0.0, es)),
            );
        }
        None => return None,
    }
    Some(out)
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
    let limits = ctx.taste.intensity.limits();
    let guided = react_guided(reaction, ctx);
    let playing = resolve_taste(&ctx.taste, &ctx.now_playing).map(|(r, _)| r.id.clone());
    let facts = json!({
        "reaction": reaction,
        "intensity": ctx.taste.intensity.id(),
        "nowPlaying": ctx.now_playing,
        "scopeHint": ctx.scope_hint,
        "builtInSpeaker": ctx.small_speaker(),
        "rules": ctx.taste.rules,
        "playingRuleId": playing,
        "suggestedByFixedSteps": guided.rule.as_ref().map(|r| json!({ "scope": r.scope, "tilt": r.tilt, "bands": r.bands })),
    });
    let system = reaction_system(es, &facts, &limits);
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
                "bands": {
                    "type": "array",
                    "maxItems": MAX_TASTE_BANDS,
                    "description": format!("The scope's whole curve after this tap: its current bands with at most {} changed \
                                    (added, moved or removed) by at most {} dB each. Omit to keep the curve as it is.",
                                    limits.bands_per_tap, limits.step_db),
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "enum": ["peak", "lowShelf", "highShelf", "highPass", "lowPass"] },
                            "freqHz": { "type": "number" }, "gainDb": { "type": "number" }, "q": { "type": "number" },
                            "note": { "type": "string", "description": "What it does, a few words in the user's language" }
                        },
                        "required": ["kind", "freqHz", "gainDb", "q"]
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
    let (base, base_bands) = base_curve(ctx, &scope);
    let wanted = args.get("tilt").map(tilt_arg).unwrap_or(base);
    // at most a few dB per tap, in range, safe for the speaker
    let step = |from: f32, to: f32| from + (to - from).clamp(-limits.step_db, limits.step_db);
    let tilt = Tilt {
        bass_db: step(base.bass_db, wanted.bass_db),
        treble_db: step(base.treble_db, wanted.treble_db),
        clarity_db: match (base.clarity_db, wanted.clarity_db) {
            (None, None) => None,
            (b, w) => Some(step(b.unwrap_or(0.0), w.unwrap_or(b.unwrap_or(0.0)))),
        },
    };
    let tilt = safe_tilt(tilt, ctx.small_speaker());
    let bands = match args.get("bands") {
        Some(v) => limit_band_step(&base_bands, &super::eq_assistant::bands_arg(v).0, &limits),
        None => base_bands.clone(),
    };
    let (bands, tilt, _) = clamp_taste_curve(&bands, tilt, ctx.small_speaker());
    if tilt == base && bands == base_bands {
        return Ok(None);
    }
    let label = args.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty());
    let rule = rule_for(ctx, scope, tilt, bands, label.map(|l| l.chars().take(60).collect()), TasteCreator::Ai);
    let mut out =
        ReactionResult { taste: ctx.taste.clone(), loudness: ctx.loudness, by_ai: true, ..Default::default() };
    finish(&mut out, rule, base, &base_bands, es);
    if let Some(m) = args.get("message").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty()) {
        out.message = m.lines().next().unwrap_or(m).chars().take(140).collect();
    }
    Ok(Some(out))
}

/// The model's new curve, applied to `base` one tap's worth: at most
/// [`TasteLimits::bands_per_tap`] bands change (the biggest changes), each by
/// at most [`TasteLimits::step_db`]; the rest stays as it was.
fn limit_band_step(base: &[TasteBand], wanted: &[TasteBand], limits: &TasteLimits) -> Vec<TasteBand> {
    let gain = |b: &TasteBand| if b.band.kind.has_gain() { b.band.gain_db } else { 0.0 };
    // (base index, wanted index, size of the change)
    let mut used = vec![false; base.len()];
    let mut changes: Vec<(Option<usize>, Option<usize>, f32)> = Vec::new();
    for (wi, w) in wanted.iter().enumerate() {
        let at = base.iter().enumerate().position(|(i, b)| !used[i] && b.same_place(&w.band));
        let size = match at {
            Some(bi) => {
                used[bi] = true;
                let b = &base[bi];
                let moved = (w.band.freq_hz / b.band.freq_hz).log2().abs() > 0.02 || (w.band.q - b.band.q).abs() > 0.01;
                (gain(w) - gain(b)).abs().max(if moved { 0.1 } else { 0.0 })
            }
            // a new pass filter counts as a change too
            None => gain(w).abs().max(if w.band.kind.has_gain() { 0.0 } else { 1.0 }),
        };
        changes.push((at, Some(wi), size));
    }
    for (bi, b) in base.iter().enumerate().filter(|(i, _)| !used[*i]) {
        changes.push((Some(bi), None, gain(b).abs().max(if b.band.kind.has_gain() { 0.0 } else { 1.0 })));
    }
    changes.retain(|c| c.2 >= 0.05);
    changes.sort_by(|a, b| b.2.total_cmp(&a.2));
    changes.truncate(limits.bands_per_tap);
    let step = |d: f32| d.clamp(-limits.step_db, limits.step_db);
    let mut out: Vec<Option<TasteBand>> = base.iter().cloned().map(Some).collect();
    let mut added = Vec::new();
    for (bi, wi, _) in changes {
        match (bi, wi) {
            (Some(bi), Some(wi)) => {
                let (b, w) = (&base[bi], &wanted[wi]);
                let mut next = w.clone();
                next.band.gain_db = gain(b) + step(gain(w) - gain(b));
                if next.note.is_empty() {
                    next.note = b.note.clone();
                }
                out[bi] = Some(next);
            }
            (None, Some(wi)) => {
                let mut next = wanted[wi].clone();
                next.band.gain_db = step(gain(&next));
                added.push(next);
            }
            (Some(bi), None) => {
                let b = &base[bi];
                let left = gain(b) - step(gain(b));
                out[bi] = (b.band.kind.has_gain() && left.abs() >= 0.05).then(|| {
                    let mut kept = b.clone();
                    kept.band.gain_db = left;
                    kept
                });
            }
            (None, None) => {}
        }
    }
    out.into_iter().flatten().chain(added).take(MAX_TASTE_BANDS).collect()
}

/// Each reaction with its label in the user's language and the usual change
/// at the Balanced intensity (in-context examples align text with EQ better:
/// arXiv 2601.09448).
fn reaction_examples(es: bool) -> String {
    let rows = [
        (
            SoundReaction::NeedsBass,
            "bassDb +3, or lowShelf 90 Hz +3 dB Q 0.7 (built-in speaker: the app lifts 200 Hz instead, at most +3 in \
             total); first undo a cut band around 100-150 Hz if the curve has one",
        ),
        (SoundReaction::TooBoomy, "bassDb -3 if the tilt boosts bass, else a band: peak 120-150 Hz -3 dB, Q 1"),
        (
            SoundReaction::TooBright,
            "trebleDb -3 (and clarityDb -1.5 if the voices are the harsh part), or bands: highShelf 8 kHz -3 dB Q 0.7 for \
             brightness, peak 3-4 kHz -3 dB Q 2 for a harsh edge, peak 6-8 kHz -3 dB Q 4 for sibilance",
        ),
        (SoundReaction::Dull, "trebleDb +3, or raise a treble cut band of the curve first, or highShelf 9 kHz +3 dB"),
        (SoundReaction::VocalsBuried, "clarityDb +3, or peak 2 kHz +3 dB Q 1 (raise a 3-4 kHz cut first)"),
    ];
    rows.iter()
        .map(|(r, change)| {
            let id = serde_json::to_value(r).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            format!("- {id} (\"{}\"): {change}", r.label(es))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn reaction_system(es: bool, facts: &Value, limits: &TasteLimits) -> String {
    let language = if es { "Spanish (neutral Latin American)" } else { "English" };
    let examples = reaction_examples(es);
    let (step_db, bands_per_tap) = (limits.step_db, limits.bands_per_tap);
    format!(
        "You adjust the sound of HomeFi, a music player, from one tap of the listener (\"how does it sound?\"). The tap is a \
complaint the listener wants to HEAR fixed: make a clearly audible change, not a timid one. Call set_taste_rule exactly once: \
pick the scope (the scope hint if there is one; else usually the main genre; the artist, album or track when the rules show \
the genre already sounds right for other music), and the change, starting from the current tilt and curve of that scope (or \
of the rule in effect, `playingRuleId`): move the tilt 2 to {step_db} dB in the direction of the reaction, and / or send \
`bands`, the scope's whole curve with at most {bands_per_tap} bands changed. Bass is a low shelf at 105 Hz, treble a high \
shelf at 10 kHz, clarity a wide peak at 3 kHz; bands are for anything more precise. Edit a band that is already there \
instead of adding another at the same place. Cut when the complaint is a problem (harsh, boomy, tiring); boost when \
something is missing (bass, air, voices). On a built-in speaker never add more than +3 dB of bass, nor boost below \
150 Hz. Typical changes per tap at the Balanced intensity (scale by intensity: Subtle x0.6, Bold x1.5; the app allows at \
most {step_db} dB per tap, per knob or band, at the listener's intensity, `intensity` in the facts):
{examples}
`message`: one short line in {language} saying what changed and for which music (\"-3 dB at 3.5 kHz for Rock: softer \
cymbals\"). Facts (JSON): {facts}"
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

/// The tilt and curve to start from: the scope's own rule, else the one
/// playing now (so a narrower rule doesn't lose what a broader one did).
fn base_curve(ctx: &ReactionContext, scope: &TasteScope) -> (Tilt, Vec<TasteBand>) {
    match ctx.taste.rule_for(scope) {
        Some(r) => (r.tilt, r.bands.clone()),
        None => resolve_taste(&ctx.taste, &ctx.now_playing).map(|(r, t)| (t, r.bands.clone())).unwrap_or_default(),
    }
}

fn rule_for(
    ctx: &ReactionContext,
    scope: TasteScope,
    tilt: Tilt,
    bands: Vec<TasteBand>,
    label: Option<String>,
    by: TasteCreator,
) -> TasteRule {
    let es = ctx.es();
    let label = label.unwrap_or_else(|| describe_curve(&scope, tilt, &bands, es));
    match ctx.taste.rule_for(&scope) {
        // keep its id and who made it first (a manual rule stays manual)
        Some(old) => TasteRule { tilt, bands, label, enabled: true, ..old.clone() },
        None => TasteRule { bands, ..TasteRule::new(scope, tilt, label, by) },
    }
}

fn finish(out: &mut ReactionResult, rule: TasteRule, before: Tilt, before_bands: &[TasteBand], es: bool) {
    let stored = out.taste.upsert(rule);
    out.taste.enabled = true;
    let change = Tilt {
        bass_db: stored.tilt.bass_db - before.bass_db,
        treble_db: stored.tilt.treble_db - before.treble_db,
        clarity_db: Some(stored.tilt.clarity_db.unwrap_or(0.0) - before.clarity_db.unwrap_or(0.0)),
    };
    let mut words = Vec::new();
    if !change.is_neutral() {
        words.push(tilt_words(change, es));
    }
    words.extend(band_changes(before_bands, &stored.bands, es));
    let name = stored.scope.name().to_owned();
    out.message = if words.is_empty() {
        if es {
            format!("Ya está al máximo seguro para {name}.")
        } else {
            format!("That's already the safe limit for {name}.")
        }
    } else if es {
        format!("{} para {name}.", capitalize(&words.join(", ")))
    } else {
        format!("{} for {name}.", capitalize(&words.join(", ")))
    };
    out.rule = Some(stored);
    out.undo_token = new_token();
}

/// What changed in a curve: `-2 dB at 8 kHz`, `high-pass at 80 Hz` / `-2 dB en 8 kHz`.
fn band_changes(before: &[TasteBand], after: &[TasteBand], es: bool) -> Vec<String> {
    let gain = |b: &EqBand| if b.kind.has_gain() { b.gain_db } else { 0.0 };
    let words = |b: &EqBand, delta: f32| -> Option<String> {
        if !b.kind.has_gain() {
            let what = match (b.kind == EqBandKind::HighPass, es) {
                (true, false) => "high-pass",
                (true, true) => "paso alto",
                (false, false) => "low-pass",
                (false, true) => "paso bajo",
            };
            return Some(format!("{what} {} {}", if es { "en" } else { "at" }, hz(b.freq_hz)));
        }
        (delta.abs() >= 0.05).then(|| {
            let v = format!("{delta:+.1}").replace(".0", "");
            format!("{v} dB {} {}", if es { "en" } else { "at" }, hz(b.freq_hz))
        })
    };
    let mut out = Vec::new();
    let mut matched = vec![false; before.len()];
    for a in after {
        let at = before.iter().enumerate().position(|(i, b)| !matched[i] && a.same_place(&b.band));
        match at {
            Some(i) => {
                matched[i] = true;
                if a.band.kind.has_gain() {
                    out.extend(words(&a.band, gain(&a.band) - gain(&before[i].band)));
                }
            }
            None => out.extend(words(&a.band, gain(&a.band))),
        }
    }
    for (b, _) in before.iter().zip(&matched).filter(|(_, m)| !**m) {
        if b.band.kind.has_gain() {
            out.extend(words(&b.band, -gain(&b.band)));
        }
    }
    out
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

/// A rule's default label from its biggest change, a tilt knob or a curve
/// band: "Less harsh · Rock" / "Menos áspero · Rock".
pub fn describe_curve(scope: &TasteScope, t: Tilt, bands: &[TasteBand], es: bool) -> String {
    let knob = [t.bass_db.abs(), t.treble_db.abs(), t.clarity_db.unwrap_or(0.0).abs()].into_iter().fold(0f32, f32::max);
    let biggest = bands
        .iter()
        .map(|b| &b.band)
        .filter(|b| b.kind.has_gain() && !b.is_neutral())
        .max_by(|a, b| a.gain_db.abs().total_cmp(&b.gain_db.abs()));
    let what = match biggest {
        Some(b) if b.gain_db.abs() > knob => band_label(b, es),
        None if knob < 0.05 && bands.iter().any(|b| b.band.enabled && b.band.kind == EqBandKind::HighPass) => {
            if es {
                "Bajo más limpio"
            } else {
                "Cleaner bass"
            }
        }
        _ => return describe(scope, t, es),
    };
    format!("{what} · {}", scope.name())
}

fn band_label(b: &EqBand, es: bool) -> &'static str {
    let up = b.gain_db > 0.0;
    let f = b.freq_hz;
    match (f, up, es) {
        (f, true, false) if f < 250.0 => "More bass",
        (f, true, true) if f < 250.0 => "Más bajo",
        (f, false, false) if f < 250.0 => "Tighter bass",
        (f, false, true) if f < 250.0 => "Bajo más firme",
        (f, true, false) if f < 1000.0 => "Fuller mids",
        (f, true, true) if f < 1000.0 => "Medios más llenos",
        (f, false, false) if f < 1000.0 => "Cleaner mids",
        (f, false, true) if f < 1000.0 => "Medios más limpios",
        (f, true, false) if f < 5000.0 => "Clearer vocals",
        (f, true, true) if f < 5000.0 => "Voces más claras",
        (f, false, false) if f < 5000.0 => "Less harsh",
        (f, false, true) if f < 5000.0 => "Menos áspero",
        (f, false, false) if f < 9000.0 && b.kind == EqBandKind::Peak => "Softer s's",
        (f, false, true) if f < 9000.0 && b.kind == EqBandKind::Peak => "Eses más suaves",
        (_, true, false) => "More air",
        (_, true, true) => "Más aire",
        (_, false, false) => "Softer highs",
        (_, false, true) => "Agudos más suaves",
    }
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

/// What a reaction replaced, to put it back as an inverse change (not by
/// writing a copy of every rule: that would bring back rules deleted since).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundSnapshot {
    /// The rule of that scope before the reaction; `None` when the reaction made it.
    pub previous_rule: Option<TasteRule>,
    /// Id of the rule the reaction wrote (empty: it only changed loudness).
    pub rule_id: String,
    /// The layers were on before the reaction.
    pub was_enabled: bool,
    pub loudness: LoudnessSettings,
}

impl SoundSnapshot {
    /// The "before" of `result`, which `ctx` produced.
    pub fn before(ctx: &ReactionContext, result: &ReactionResult) -> Self {
        let written = result.rule.as_ref();
        Self {
            previous_rule: written.and_then(|r| ctx.taste.rule_for(&r.scope)).cloned(),
            rule_id: written.map(|r| r.id.clone()).unwrap_or_default(),
            was_enabled: ctx.taste.enabled,
            loudness: ctx.loudness,
        }
    }

    /// The change that puts the rule back, for the settings as `current` is
    /// now: the previous rule again (only if it still exists there: a rule
    /// the listener deleted since stays deleted), or no rule at all when the
    /// reaction made it. The layers go back off only if the reaction switched
    /// them on.
    pub fn undo_change(&self, current: &TasteSettings) -> TasteChange {
        let still_there = self.previous_rule.as_ref().is_some_and(|p| current.rules.iter().any(|r| r.id == p.id));
        self.inverse(still_there)
    }

    /// [`undo_change`](Self::undo_change) without knowing the current rules:
    /// the previous rule comes back even if it was deleted meanwhile.
    pub fn restore_change(&self) -> TasteChange {
        self.inverse(true)
    }

    fn inverse(&self, restore_previous: bool) -> TasteChange {
        let mut change = TasteChange::default();
        if self.rule_id.is_empty() {
            return change;
        }
        match &self.previous_rule {
            Some(previous) if restore_previous => change.push_upsert(previous.clone()),
            Some(_) => {}
            None => change.push_remove(self.rule_id.clone()),
        }
        if !self.was_enabled {
            change.enabled = Some(false);
        }
        change
    }
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
    use crate::dsp::{ItemRef, SourceContext, TasteIntensity};
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
        // keeps the genre's +2 bass, softens the top with a band
        assert_eq!((rule.tilt.bass_db, rule.tilt.treble_db), (2.0, 0.0));
        assert_eq!(rule.bands.len(), 1);
        let b = rule.bands[0].band;
        assert_eq!((b.kind, b.freq_hz, b.gain_db), (EqBandKind::HighShelf, 8000.0, -STEP_DB));
        assert_eq!(rule.bands[0].note, "agudos más suaves");
        assert_eq!(r.message, "-2 dB en 8 kHz para Homework.");
        assert_eq!(r.taste.rules.len(), 2);
    }

    #[test]
    fn guided_bands_go_where_a_knob_is_too_blunt() {
        let mut c = ctx("en");
        let tap = |c: &mut ReactionContext, r: SoundReaction| {
            let out = react_guided(r, c);
            c.taste = out.taste.clone();
            out
        };
        // too bright: the 8 kHz shelf, then the harsh 3.5 kHz edge, alternating
        let r = tap(&mut c, SoundReaction::TooBright);
        assert_eq!(r.message, "-2 dB at 8 kHz for Electronic.");
        let r = tap(&mut c, SoundReaction::TooBright);
        assert_eq!(r.message, "-2 dB at 3.5 kHz for Electronic.");
        let rule = r.rule.unwrap();
        assert_eq!(rule.bands.len(), 2);
        assert_eq!((rule.bands[1].band.kind, rule.bands[1].band.q), (EqBandKind::Peak, 2.0));
        assert_eq!(rule.label, "Less harsh · Electronic");
        for _ in 0..10 {
            tap(&mut c, SoundReaction::TooBright);
        }
        let bands = &c.taste.rules[0].bands;
        assert_eq!(bands.len(), 2, "taps join the same bands");
        assert!(bands.iter().all(|b| b.band.gain_db == -MAX_TILT_DB), "{bands:?}");
        // dull undoes the shelf cut first
        let r = tap(&mut c, SoundReaction::Dull);
        assert_eq!(r.message, "+2 dB at 8 kHz for Electronic.");
        assert_eq!(c.taste.rules[0].tilt.treble_db, 0.0);
        // too boomy without a bass boost: a cut at 120 Hz; more bass takes it back out
        let mut c = ctx("en");
        let r = tap(&mut c, SoundReaction::TooBoomy);
        assert_eq!(r.message, "-2 dB at 120 Hz for Electronic.");
        let r = tap(&mut c, SoundReaction::NeedsBass);
        assert!(r.rule.unwrap().bands.is_empty(), "back at 0: the band goes");
        assert_eq!(c.taste.rules[0].tilt.bass_db, 0.0);
    }

    #[tokio::test]
    async fn ai_curves_change_two_bands_a_tap_at_most() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{"type": "tool_use", "id": "c1", "name": "set_taste_rule", "input": {
                    "scope": {"kind": "genre", "name": "Electronic"},
                    "bands": [
                        {"kind": "peak", "freqHz": 3400, "gainDb": -8, "q": 2.5, "note": "menos filo"},
                        {"kind": "peak", "freqHz": 7000, "gainDb": -2, "q": 4},
                        {"kind": "highShelf", "freqHz": 9000, "gainDb": -1, "q": 0.7},
                        {"kind": "bell", "freqHz": 60, "gainDb": 1, "q": 1}
                    ],
                    "message": "Menos filo para Electronic."
                }}],
                "stop_reason": "tool_use"
            })))
            .mount(&server)
            .await;
        let config = AiConfig::new(AiProvider::Anthropic, "m", Some("k".into())).with_base_url(server.uri());
        let mut c = ctx("es");
        // the genre already has a 3.5 kHz cut
        c.taste.upsert(
            TasteRule::new(TasteScope::Genre { name: "Electronic".into() }, Tilt::default(), "x", TasteCreator::Manual)
                .with_bands(vec![TasteBand::new(EqBandKind::Peak, 3500.0, -2.0, 2.0)]),
        );
        let r = react(SoundReaction::TooBright, &c, Some(&config)).await;
        assert!(r.by_ai, "{}", r.message);
        let rule = r.rule.unwrap();
        assert_eq!(rule.created_by, TasteCreator::Manual, "a manual rule stays manual");
        // the two biggest changes: the existing cut (-6 → capped at -3 more) and the new 7 kHz cut
        assert_eq!(rule.bands.len(), 2, "{:?}", rule.bands);
        assert_eq!(rule.bands[0].band.gain_db, -5.0);
        assert_eq!(rule.bands[0].band.freq_hz, 3400.0);
        assert_eq!(rule.bands[0].note, "menos filo");
        assert_eq!((rule.bands[1].band.freq_hz, rule.bands[1].band.gain_db), (7000.0, -2.0));
        assert_eq!(r.message, "Menos filo para Electronic.");
    }

    #[test]
    fn band_steps_are_limited() {
        let base = vec![
            TasteBand::new(EqBandKind::Peak, 100.0, -6.0, 1.0),
            TasteBand::new(EqBandKind::HighPass, 40.0, 0.0, 0.7),
        ];
        // the model drops everything: the cut eases by 3 dB, the high-pass (1 change) goes too
        let limits = TasteIntensity::Balanced.limits();
        let out = limit_band_step(&base, &[], &limits);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].band.gain_db, -3.0);
        // nothing changes: the same curve
        assert_eq!(limit_band_step(&base, &base, &limits), base);
        let words = band_changes(&base, &out, false);
        assert_eq!(words, ["+3 dB at 100 Hz"]);
        assert_eq!(band_changes(&[], &base[1..], true), ["paso alto en 40 Hz"]);
        assert_eq!(
            describe_curve(&TasteScope::Genre { name: "Rock".into() }, Tilt::default(), &base, true),
            "Bajo más firme · Rock"
        );
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
        undo.push(&r.undo_token, SoundSnapshot::before(&c, &r));
        let back = undo.undo(&r.undo_token).unwrap();
        assert_eq!(back.previous_rule, None, "the reaction made the rule");
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
        assert_eq!(rule.tilt.bass_db, TasteIntensity::Balanced.limits().step_db, "at most 3 dB per tap");
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
        let limits = TasteIntensity::Balanced.limits();
        let es = reaction_system(true, &facts, &limits);
        for line in [
            "needsBass (\"Le falta bajo\"): bassDb +3",
            "tooBoomy (\"Retumba mucho\"): bassDb -3",
            "tooBright (\"Muy brillante / áspero\"): trebleDb -3",
            "dull (\"Apagado\"): trebleDb +3",
            "vocalsBuried (\"No se oyen las voces\"): clarityDb +3",
        ] {
            assert!(es.contains(line), "{line}\n{es}");
        }
        assert!(es.contains("Spanish") && es.contains("at most 3 dB per tap"));
        assert!(es.contains("Subtle x0.6, Bold x1.5") && es.contains("move the tilt 2 to 3 dB"));
        let en = reaction_system(false, &facts, &limits);
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
        for timid in ["gentle", "prefer cuts"] {
            assert!(!en.contains(timid) && !es.contains(timid), "{timid}");
        }
        // the limits come from the intensity
        let bold = reaction_system(false, &facts, &TasteIntensity::Bold.limits());
        assert!(bold.contains("at most 4 dB per tap") && bold.contains("with at most 3 bands changed"));
    }

    #[test]
    fn undo_is_the_inverse_change_not_a_copy() {
        let mut c = ctx("en");
        let tap = |c: &mut ReactionContext, r: SoundReaction| {
            let out = react_guided(r, c);
            let before = SoundSnapshot::before(c, &out);
            c.taste.apply(&out.taste_change());
            (out, before)
        };
        // the first tap makes the rule: undoing removes it, and the layers go back off
        let (first, snap) = tap(&mut c, SoundReaction::NeedsBass);
        let id = first.rule.as_ref().unwrap().id.clone();
        assert_eq!(first.taste_change().enabled, Some(true));
        let undo = snap.undo_change(&c.taste);
        assert_eq!((undo.remove.clone(), undo.upsert.len(), undo.enabled), (vec![id.clone()], 0, Some(false)));

        // another rule made after the tap survives the undo; the copy of the settings would have dropped it
        let mut now = c.taste.clone();
        now.upsert(TasteRule::new(
            TasteScope::Artist { id: String::new(), name: "Bach".into() },
            Tilt { treble_db: 1.0, ..Default::default() },
            "Bach",
            TasteCreator::Manual,
        ));
        now.apply(&undo);
        assert_eq!(now.rules.len(), 1);
        assert_eq!(now.rules[0].scope.name(), "Bach");

        // a second tap edits the rule: undoing puts the first version back
        let (second, snap) = tap(&mut c, SoundReaction::NeedsBass);
        assert_eq!(second.rule.as_ref().unwrap().tilt.bass_db, 2.0 * STEP_DB);
        let undo = snap.undo_change(&c.taste);
        let mut back = c.taste.clone();
        back.apply(&undo);
        assert_eq!(back.rules[0].tilt.bass_db, STEP_DB);
        assert_eq!(back.rules[0].id, id);
        assert!(back.enabled, "it was on before the second tap");

        // the user deleted the rule meanwhile: undo doesn't bring it back
        let mut gone = c.taste.clone();
        gone.remove(&id);
        let undo = snap.undo_change(&gone);
        gone.apply(&undo);
        assert!(gone.rules.is_empty());
    }

    #[test]
    fn intensity_scales_the_fixed_steps() {
        let mut c = ctx("en");
        c.taste.intensity = TasteIntensity::Bold;
        let r = react_guided(SoundReaction::NeedsBass, &c);
        assert_eq!(r.rule.unwrap().tilt.bass_db, STEP_DB * 1.5);
        c.taste.intensity = TasteIntensity::Subtle;
        let r = react_guided(SoundReaction::NeedsBass, &c);
        assert!((r.rule.unwrap().tilt.bass_db - STEP_DB * 0.6).abs() < 1e-6);
    }
}
