//! The EQ conversation without AI: a fixed path (output → headphone model in
//! AutoEq → taste → where you listen → confirm) that builds the profile with
//! [`with_taste`](crate::dsp::with_taste). Same [`AssistantTurn`]s as the AI
//! assistant, so the apps draw both with one screen.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::eq_assistant::{finish_profile_for, AssistantContext, AssistantTurn, AudioOutput, EqProposal, OutputKind};
use crate::dsp::autoeq::{AutoEq, HeadphoneEntry};
use crate::dsp::{safe_tilt, EqProfile, EqSource, TasteCreator, TasteRule, TasteScope, Tilt};
use crate::error::Result;

/// Headphones offered after a search.
const CANDIDATES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GuidedStep {
    #[default]
    Output,
    Model,
    PickModel,
    Taste,
    Place,
    Confirm,
    /// "More bass for <genre>?" (a taste layer).
    TasteOffer,
    Done,
}

/// Where the wizard is; serializable to resume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GuidedState {
    pub context: AssistantContext,
    pub step: GuidedStep,
    pub output_id: Option<String>,
    #[serde(default)]
    pub candidates: Vec<HeadphoneEntry>,
    #[serde(default)]
    pub base: Option<EqProfile>,
    #[serde(default)]
    pub tilt: Tilt,
    #[serde(default)]
    pub proposal: Option<EqProposal>,
    /// The taste layer offered at [`GuidedStep::TasteOffer`].
    #[serde(default)]
    pub offer: Option<TasteRule>,
}

/// The guided EQ wizard.
pub struct GuidedEq {
    state: GuidedState,
    autoeq: Arc<AutoEq>,
}

/// A choice of the wizard: words that pick it, in both languages.
struct Choice {
    en: &'static str,
    es: &'static str,
    words: &'static [&'static str],
    tilt: Tilt,
}

const fn tilt(bass_db: f32, treble_db: f32, clarity_db: Option<f32>) -> Tilt {
    Tilt { bass_db, treble_db, clarity_db }
}

const TASTES: [Choice; 4] = [
    Choice {
        en: "Neutral, as it comes",
        es: "Neutral, tal cual",
        words: &["neutral", "tal cual", "as it", "normal", "igual"],
        tilt: tilt(0.0, 0.0, None),
    },
    Choice {
        en: "More bass",
        es: "Más bajos",
        words: &["bass", "bajo", "graves", "bajos"],
        tilt: tilt(4.0, 0.0, None),
    },
    Choice {
        en: "More clarity",
        es: "Más claridad",
        words: &["clar", "detail", "detalle", "voces", "voice", "vocal"],
        tilt: tilt(0.0, 1.0, Some(2.0)),
    },
    Choice {
        en: "Less harsh",
        es: "Menos áspero",
        words: &["harsh", "áspero", "aspero", "brillante", "bright", "tiring", "cansa", "suave", "soft"],
        tilt: tilt(0.0, -3.0, Some(-1.0)),
    },
];

const PLACES: [Choice; 3] = [
    Choice {
        en: "At home, quiet",
        es: "En casa, tranquilo",
        words: &["home", "casa", "quiet", "tranquil", "silenc"],
        tilt: tilt(0.0, 0.0, None),
    },
    Choice {
        en: "Out, noisy places",
        es: "Afuera, con ruido",
        words: &["noisy", "ruido", "calle", "street", "out", "afuera", "bus", "metro", "tren", "train"],
        tilt: tilt(2.0, 0.0, Some(1.0)),
    },
    Choice {
        en: "In the car",
        es: "En el carro",
        words: &["car", "carro", "coche", "auto"],
        tilt: tilt(2.0, -1.0, None),
    },
];

impl GuidedEq {
    /// A new wizard; AutoEq profiles are cached in `<app dir>/autoeq` unless
    /// [`with_autoeq`](Self::with_autoeq) says otherwise.
    pub fn new(context: AssistantContext) -> Self {
        Self::resume(GuidedState { context, ..Default::default() })
    }

    pub fn resume(state: GuidedState) -> Self {
        Self { state, autoeq: Arc::new(AutoEq::new(crate::paths::app_dir().join("autoeq"))) }
    }

    pub fn with_autoeq(mut self, autoeq: Arc<AutoEq>) -> Self {
        self.autoeq = autoeq;
        self
    }

    pub fn state(&self) -> &GuidedState {
        &self.state
    }

    fn es(&self) -> bool {
        self.state.context.spanish()
    }

    fn lang(&self) -> crate::humane::Lang {
        if self.es() {
            crate::humane::Lang::Es
        } else {
            crate::humane::Lang::En
        }
    }

    fn say(&self, en: &str, es: &str) -> String {
        if self.es() { es } else { en }.to_owned()
    }

    fn output(&self) -> Option<&AudioOutput> {
        self.state.output_id.as_deref().and_then(|id| self.state.context.output(id))
    }

    /// The first question (or the first one that can't be skipped).
    pub async fn start(&mut self) -> Result<AssistantTurn> {
        let context = std::mem::take(&mut self.state.context);
        self.state = GuidedState { context, ..Default::default() };
        if let [only] = self.state.context.outputs.as_slice() {
            self.state.output_id = Some(only.id.clone());
            let intro = self.say("Let's set up the equalizer.", "Vamos a ajustar el ecualizador.");
            return Ok(self.after_output(Some(intro)));
        }
        Ok(self.ask_output(None))
    }

    /// The user's answer (typed, or one of the quick replies).
    pub async fn reply(&mut self, text: &str) -> Result<AssistantTurn> {
        let text = text.trim();
        Ok(match self.state.step {
            GuidedStep::Output => {
                let picked = pick_output(&self.state.context.outputs, text).map(|o| o.id.clone());
                match picked {
                    Some(id) => {
                        self.state.output_id = Some(id);
                        self.after_output(None)
                    }
                    None => self.ask_output(Some(self.say("I didn't catch which one.", "No entendí cuál."))),
                }
            }
            GuidedStep::Model if is_skip(text) || text.is_empty() => self.ask_taste(None),
            GuidedStep::Model => self.search(text).await,
            GuidedStep::PickModel => {
                if is_skip(text) {
                    return Ok(self.ask_taste(None));
                }
                let picked = pick_candidate(&self.state.candidates, text);
                match picked {
                    Some(entry) => match self.autoeq.profile(&entry).await {
                        Ok(profile) => {
                            self.state.base = Some(profile);
                            let note = self.say(
                                &format!("Got the {} measurements.", entry.name),
                                &format!("Tengo las mediciones de tus {}.", entry.name),
                            );
                            self.ask_taste(Some(note))
                        }
                        Err(e) => self.ask_taste(Some(format!(
                            "{} {}",
                            e.humane_in(self.lang()),
                            self.say("We'll go on with your taste.", "Seguimos con tu gusto.")
                        ))),
                    },
                    // something else: search again with it
                    None => self.search(text).await,
                }
            }
            GuidedStep::Taste => match pick_choice(&TASTES, text) {
                Some(c) => {
                    self.state.tilt = c.tilt;
                    self.ask_place()
                }
                None => self.ask_taste(Some(self.say("Pick one of these:", "Elige una de estas:"))),
            },
            GuidedStep::Place => match pick_choice(&PLACES, text) {
                Some(c) => {
                    let t = self.state.tilt;
                    self.state.tilt = Tilt {
                        bass_db: t.bass_db + c.tilt.bass_db,
                        treble_db: t.treble_db + c.tilt.treble_db,
                        clarity_db: match (t.clarity_db, c.tilt.clarity_db) {
                            (None, None) => None,
                            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
                        },
                    };
                    self.propose()
                }
                None => self.ask_place(),
            },
            GuidedStep::Confirm => {
                if is_yes(text) {
                    self.state.step = GuidedStep::Done;
                    let done = self.say("Done. Your equalizer is on.", "Listo. Tu ecualizador está activado.");
                    let mut turn = match self.offer_taste() {
                        Some(offer) => offer,
                        None => AssistantTurn { done: true, ..Default::default() },
                    };
                    turn.text = join(Some(done), turn.text);
                    turn.proposal = self.state.proposal.clone();
                    turn.confirmed = true;
                    turn
                } else {
                    self.state.step = GuidedStep::Done;
                    AssistantTurn {
                        text: self.say("OK, nothing changed.", "Va, no cambié nada."),
                        done: true,
                        ..Default::default()
                    }
                }
            }
            GuidedStep::TasteOffer => {
                self.state.step = GuidedStep::Done;
                match self.state.offer.take().filter(|_| is_yes(text)) {
                    Some(rule) => {
                        let label = rule.label.clone();
                        let mut taste = self.state.context.taste.clone();
                        taste.upsert(rule);
                        self.state.context.taste = taste.clone();
                        AssistantTurn {
                            text: self.say(
                                &format!("Done: “{label}”. You'll find it in Taste rules, to change or remove it."),
                                &format!("Listo: «{label}». Está en Reglas de gusto, para cambiarla o quitarla."),
                            ),
                            taste: Some(taste),
                            done: true,
                            ..Default::default()
                        }
                    }
                    None => AssistantTurn {
                        text: self.say("OK, nothing changed.", "Va, no cambié nada."),
                        done: true,
                        ..Default::default()
                    },
                }
            }
            GuidedStep::Done => {
                AssistantTurn { text: self.say("All set.", "Todo listo."), done: true, ..Default::default() }
            }
        })
    }

    /// A proactive suggestion for a card: a taste layer for the top genre.
    pub fn suggest(&mut self) -> AssistantTurn {
        match self.offer_taste() {
            Some(turn) => turn,
            None => AssistantTurn {
                text: self.say(
                    "While you listen, tap “How does it sound?” in the player and I'll adjust it for that music.",
                    "Mientras escuchas, toca «¿Cómo suena?» en el reproductor y lo ajusto para esa música.",
                ),
                done: true,
                ..Default::default()
            },
        }
    }

    /// "More bass for Electronic?": a safe, fixed tilt for the genre they
    /// listen to most, unless it already has a layer.
    fn offer_taste(&mut self) -> Option<AssistantTurn> {
        let genre = self.state.context.top_genre()?.to_owned();
        let scope = TasteScope::Genre { name: genre.clone() };
        if self.state.context.taste.rule_for(&scope).is_some() {
            return None;
        }
        let es = self.es();
        let (tilt, en_what, es_what) = genre_tilt(&genre);
        let tilt = safe_tilt(tilt, self.state.context.on_small_speaker());
        let label = super::reactions::describe(&scope, tilt, es);
        let what = if es { es_what } else { en_what };
        self.state.offer = Some(TasteRule::new(scope, tilt, label, TasteCreator::Guided));
        self.state.step = GuidedStep::TasteOffer;
        Some(AssistantTurn {
            text: self.say(
                &format!(
                    "You listen to a lot of {genre}. Want {what} when it plays ({})? Only that music changes.",
                    super::reactions::tilt_words(tilt, false)
                ),
                &format!(
                    "Escuchas mucho {genre}. ¿Quieres {what} cuando suene ({})? Solo cambia esa música.",
                    super::reactions::tilt_words(tilt, true)
                ),
            ),
            quick_replies: vec![self.say("Yes, try it", "Sí, pruébalo"), self.say("No, thanks", "No, gracias")],
            ..Default::default()
        })
    }

    fn ask_output(&mut self, before: Option<String>) -> AssistantTurn {
        self.state.step = GuidedStep::Output;
        let q = self.say("What will you listen with?", "¿Con qué vas a escuchar?");
        AssistantTurn {
            text: join(before, q),
            quick_replies: self.state.context.outputs.iter().map(|o| o.name.clone()).take(6).collect(),
            ..Default::default()
        }
    }

    /// Headphone-like outputs get the model question; the rest go to taste.
    fn after_output(&mut self, before: Option<String>) -> AssistantTurn {
        let kind = self.output().map(|o| o.kind).unwrap_or_default();
        match kind {
            OutputKind::BuiltIn | OutputKind::Car | OutputKind::Hdmi => self.ask_taste(before),
            _ => {
                self.state.step = GuidedStep::Model;
                let q = self.say(
                    "Which headphones are they? Type the brand and model (e.g. Sony WH-1000XM5).",
                    "¿Qué audífonos son? Escribe la marca y el modelo (ej. Sony WH-1000XM5).",
                );
                AssistantTurn {
                    text: join(before, q),
                    quick_replies: vec![self.say("I don't know", "No sé")],
                    ..Default::default()
                }
            }
        }
    }

    async fn search(&mut self, query: &str) -> AssistantTurn {
        match self.autoeq.search(query).await {
            Ok(found) if !found.is_empty() => {
                self.state.candidates = found.into_iter().take(CANDIDATES).collect();
                self.state.step = GuidedStep::PickModel;
                let mut replies: Vec<String> = self.state.candidates.iter().map(label).collect();
                replies.push(self.say("None of these", "Ninguno de estos"));
                AssistantTurn {
                    text: self.say("Which one is yours?", "¿Cuál es el tuyo?"),
                    quick_replies: replies,
                    ..Default::default()
                }
            }
            Ok(_) => {
                let note = self.say(
                    &format!("I couldn't find “{query}” among the measured headphones. We'll tune it to your taste."),
                    &format!("No encontré «{query}» entre los audífonos medidos. Lo ajustamos a tu gusto."),
                );
                self.ask_taste(Some(note))
            }
            Err(e) => {
                let note = format!(
                    "{} {}",
                    e.humane_in(self.lang()),
                    self.say("We'll tune it to your taste.", "Lo ajustamos a tu gusto.")
                );
                self.ask_taste(Some(note))
            }
        }
    }

    fn ask_taste(&mut self, before: Option<String>) -> AssistantTurn {
        self.state.step = GuidedStep::Taste;
        let q = self.say("How do you like the sound?", "¿Cómo te gusta el sonido?");
        let es = self.es();
        AssistantTurn {
            text: join(before, q),
            quick_replies: TASTES.iter().map(|c| if es { c.es } else { c.en }.to_owned()).collect(),
            ..Default::default()
        }
    }

    fn ask_place(&mut self) -> AssistantTurn {
        self.state.step = GuidedStep::Place;
        let es = self.es();
        AssistantTurn {
            text: self.say("Where do you listen most?", "¿Dónde escuchas más?"),
            quick_replies: PLACES.iter().map(|c| if es { c.es } else { c.en }.to_owned()).collect(),
            ..Default::default()
        }
    }

    fn propose(&mut self) -> AssistantTurn {
        let output = self.output().cloned().or_else(|| self.state.context.outputs.first().cloned());
        let Some(output) = output else {
            self.state.step = GuidedStep::Done;
            return AssistantTurn {
                text: self.say("There's no output to set up.", "No hay una salida para ajustar."),
                done: true,
                ..Default::default()
            };
        };
        let base = self.state.base.clone().unwrap_or_else(|| EqProfile::flat(""));
        let measured = matches!(base.source, EqSource::AutoEq { .. });
        let (mut profile, _) = finish_profile_for(&base, self.state.tilt, output.kind == OutputKind::BuiltIn);
        profile.source = EqSource::Guided;
        profile.name = match &base.source {
            EqSource::AutoEq { model, .. } => model.clone(),
            _ => self.say(&format!("My sound · {}", output.name), &format!("Mi sonido · {}", output.name)),
        };
        let explanation = match (measured, profile.bands_flat()) {
            (_, true) => self.say(
                "No changes: it plays as it comes, still bit-perfect.",
                "Sin cambios: suena tal cual, sigue siendo bit-perfect.",
            ),
            (true, false) => self.say(
                "It corrects your headphones from their measurements and adds your taste. With the equalizer on, the sound is processed (no longer bit-perfect); the volume is lowered a little so it never distorts.",
                "Corrige tus audífonos con sus mediciones y suma tu gusto. Con el ecualizador encendido el sonido se procesa (ya no es bit-perfect); el volumen baja un poco para que nunca distorsione.",
            ),
            (false, false) => self.say(
                "It adjusts the sound to your taste. With the equalizer on, the sound is processed (no longer bit-perfect); the volume is lowered a little so it never distorts.",
                "Ajusta el sonido a tu gusto. Con el ecualizador encendido el sonido se procesa (ya no es bit-perfect); el volumen baja un poco para que nunca distorsione.",
            ),
        };
        let proposal = EqProposal { output_id: output.id.clone(), profile, explanation: explanation.clone() };
        self.state.proposal = Some(proposal.clone());
        self.state.step = GuidedStep::Confirm;
        AssistantTurn {
            text: join(Some(explanation), self.say("Apply it?", "¿Lo aplico?")),
            // the proposal card has its own Apply / Keep talking buttons
            quick_replies: Vec::new(),
            proposal: Some(proposal),
            ..Default::default()
        }
    }
}

fn join(before: Option<String>, text: String) -> String {
    match before {
        Some(b) if !b.is_empty() => format!("{b}\n\n{text}"),
        _ => text,
    }
}

/// `Sennheiser HD 650 (oratory1990)`: the source tells same-named entries apart.
fn label(e: &HeadphoneEntry) -> String {
    format!("{} ({})", e.name, e.source)
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

fn is_skip(text: &str) -> bool {
    let t = norm(text);
    ["no sé", "no se", "i don't know", "i dont know", "skip", "saltar", "ninguno", "none", "no lo sé"]
        .iter()
        .any(|w| t.starts_with(w))
}

/// A quick reply that only says "apply" or "cancel" (the proposal card does that).
pub(crate) fn is_apply_or_cancel(text: &str) -> bool {
    let t: String = norm(text)
        .trim_end_matches(['.', '!'])
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            other => other,
        })
        .collect();
    let only = |words: &[&str]| words.iter().any(|w| t == *w);
    let starts = |words: &[&str]| words.iter().any(|w| t.starts_with(w));
    only(&["si", "yes", "ok", "no", "cancel", "cancelar", "no, gracias", "no thanks", "no, thanks", "listo", "dale"])
        || starts(&["aplic", "apply", "si, aplic", "si aplic", "yes, apply", "cancel", "no, cancel"])
}

/// A gentle, fixed tilt for a genre: (tilt, what it does in English, in Spanish).
pub(crate) fn genre_tilt(genre: &str) -> (Tilt, &'static str, &'static str) {
    let g = norm(genre);
    let any = |words: &[&str]| words.iter().any(|w| g.contains(w));
    if any(&[
        "electr", "dance", "house", "techno", "edm", "hip", "rap", "reggaet", "trap", "urban", "dubstep", "drum",
        "dub", "bass",
    ]) {
        (tilt(3.0, 0.0, None), "a bit more bass", "un poco más de bajo")
    } else if any(&["rock", "metal", "punk", "alternativ", "indie", "grunge"]) {
        (tilt(2.0, 1.0, None), "more punch", "más pegada")
    } else if any(&[
        "classic",
        "clásic",
        "jazz",
        "acoustic",
        "acústic",
        "folk",
        "soundtrack",
        "ambient",
        "opera",
        "piano",
    ]) {
        (tilt(0.0, 1.0, Some(1.5)), "a touch more detail", "un toque más de detalle")
    } else if any(&["pop", "latin", "latin", "r&b", "soul", "salsa", "bachata", "cumbia", "regional", "country"]) {
        (tilt(2.0, 0.0, Some(1.0)), "more warmth and clearer voices", "más calidez y voces más claras")
    } else {
        (tilt(2.0, 0.0, None), "a little more bass", "un poco más de bajo")
    }
}

fn is_yes(text: &str) -> bool {
    let t = norm(text);
    ["sí", "si", "yes", "ok", "va", "dale", "aplicar", "apply", "claro", "sure", "listo", "pruébalo", "try it"]
        .iter()
        .any(|w| t == *w || t.starts_with(&format!("{w} ")) || t.starts_with(&format!("{w},")))
}

/// By number (1-based), exact name, or a name that contains the text.
fn pick_output<'a>(outputs: &'a [AudioOutput], text: &str) -> Option<&'a AudioOutput> {
    let t = norm(text);
    if let Ok(n) = t.parse::<usize>() {
        return outputs.get(n.checked_sub(1)?);
    }
    outputs.iter().find(|o| norm(&o.name) == t || norm(&o.id) == t).or_else(|| {
        outputs.iter().find(|o| !t.is_empty() && (norm(&o.name).contains(&t) || t.contains(&norm(&o.name))))
    })
}

fn pick_candidate(candidates: &[HeadphoneEntry], text: &str) -> Option<HeadphoneEntry> {
    let t = norm(text);
    if let Ok(n) = t.parse::<usize>() {
        return candidates.get(n.checked_sub(1)?).cloned();
    }
    candidates.iter().find(|e| norm(&label(e)) == t || norm(&e.name) == t).cloned()
}

fn pick_choice<'a>(choices: &'a [Choice], text: &str) -> Option<&'a Choice> {
    let t = norm(text);
    if let Ok(n) = t.parse::<usize>() {
        return choices.get(n.checked_sub(1)?);
    }
    choices
        .iter()
        .find(|c| norm(c.en) == t || norm(c.es) == t)
        .or_else(|| choices.iter().find(|c| c.words.iter().any(|w| t.contains(w))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::eq_assistant::tests::{context, fake_autoeq};
    use crate::ai::AssistantContext;
    use crate::dsp::{auto_preamp_db, EqBandKind};
    use wiremock::MockServer;

    #[tokio::test]
    async fn walks_from_the_device_to_a_profile() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let mut g = GuidedEq::new(context()).with_autoeq(fake_autoeq(&server, dir.path()).await);

        let t = g.start().await.unwrap();
        assert_eq!(t.text, "¿Con qué vas a escuchar?");
        assert_eq!(t.quick_replies, ["USB DAC", "WH-1000XM5"]);

        let t = g.reply("usb dac").await.unwrap();
        assert!(t.text.starts_with("¿Qué audífonos son?"), "{}", t.text);

        let t = g.reply("hd650").await.unwrap();
        assert_eq!(t.quick_replies, ["Sennheiser HD 650 (oratory1990)", "Ninguno de estos"]);

        let t = g.reply("Sennheiser HD 650 (oratory1990)").await.unwrap();
        assert!(t.text.contains("Tengo las mediciones"), "{}", t.text);
        assert_eq!(t.quick_replies.len(), 4);

        let t = g.reply("Más bajos").await.unwrap();
        assert_eq!(t.text, "¿Dónde escuchas más?");

        let t = g.reply("en el metro, hay ruido").await.unwrap();
        let p = t.proposal.clone().unwrap();
        assert_eq!(p.output_id, "dac");
        assert_eq!(p.profile.name, "Sennheiser HD 650");
        assert_eq!(p.profile.source, EqSource::Guided);
        // AutoEq's 6.4 dB shelf + 4 (bass) + 2 (noisy)
        let shelf = p.profile.bands.iter().find(|b| b.kind == EqBandKind::LowShelf).unwrap();
        assert!((shelf.gain_db - 12.0).abs() < 0.01, "{shelf:?}");
        assert_eq!(p.profile.preamp_db, auto_preamp_db(&p.profile));
        assert!(t.text.contains("bit-perfect") && !t.confirmed);
        assert!(t.quick_replies.is_empty(), "the proposal card has Apply / Keep talking");

        // resumes from the saved state
        let saved = serde_json::to_string(g.state()).unwrap();
        let mut g = GuidedEq::resume(serde_json::from_str(&saved).unwrap());
        let t = g.reply("Aplicar").await.unwrap();
        assert!(t.confirmed && t.done && t.proposal.is_some());
    }

    #[tokio::test]
    async fn phone_speakers_get_a_low_mid_lift_and_a_taste_offer_follows() {
        let mut ctx = context();
        ctx.language = "en".into();
        ctx.outputs = vec![AudioOutput {
            id: "phone".into(),
            name: "Phone speaker".into(),
            kind: OutputKind::BuiltIn,
            sample_rates: vec![],
        }];
        ctx.listening =
            Some(crate::ai::ListeningSummary { top_genres: vec!["Reggaeton".into()], ..Default::default() });
        let mut g = GuidedEq::new(ctx);
        g.start().await.unwrap();
        let t = g.reply("More bass").await.unwrap();
        assert_eq!(t.text, "Where do you listen most?");
        let t = g.reply("At home").await.unwrap();
        let p = t.proposal.unwrap();
        assert!(!p.profile.bands.iter().any(|b| b.kind == EqBandKind::LowShelf), "{:?}", p.profile.bands);
        let lift = p.profile.bands.iter().find(|b| b.kind == EqBandKind::Peak).unwrap();
        assert!(lift.freq_hz >= 150.0 && lift.freq_hz <= 250.0 && lift.gain_db <= 3.0, "{lift:?}");
        assert!(p.profile.bands.iter().any(|b| b.kind == EqBandKind::HighPass));
        // applying offers a taste layer for the top genre
        let t = g.reply("Apply").await.unwrap();
        assert!(t.confirmed && !t.done && t.text.contains("Reggaeton"), "{}", t.text);
        assert_eq!(t.quick_replies, ["Yes, try it", "No, thanks"]);
        let t = g.reply("Yes, try it").await.unwrap();
        let taste = t.taste.unwrap();
        assert!(taste.enabled && t.done && !t.confirmed);
        let r = &taste.rules[0];
        assert_eq!(r.created_by, TasteCreator::Guided);
        assert!(r.tilt.bass_db > 0.0 && r.tilt.bass_db <= crate::dsp::SMALL_SPEAKER_MAX_BASS_DB);
        // a suggestion when the genre already has a layer: just the reactions tip
        let mut g = GuidedEq::new(AssistantContext { taste, ..g.state().context.clone() });
        assert!(g.suggest().done);
    }

    #[test]
    fn apply_and_cancel_chips_are_recognized() {
        for t in ["Apply", "Aplicar", "Sí, aplícalo", "Cancel", "No", "No, thanks"] {
            assert!(is_apply_or_cancel(t), "{t}");
        }
        for t in ["Más bajos", "No, más agudos", "Menos agudos", "Other headphones"] {
            assert!(!is_apply_or_cancel(t), "{t}");
        }
    }

    #[tokio::test]
    async fn speakers_skip_the_model_and_no_means_no() {
        let mut ctx = context();
        ctx.language = "en".into();
        ctx.outputs = vec![AudioOutput {
            id: "car".into(),
            name: "Car stereo".into(),
            kind: OutputKind::Car,
            sample_rates: vec![],
        }];
        let mut g = GuidedEq::new(ctx);
        let t = g.start().await.unwrap();
        assert!(t.text.ends_with("How do you like the sound?"), "{}", t.text);
        g.reply("neutral").await.unwrap();
        let t = g.reply("3").await.unwrap();
        let p = t.proposal.unwrap();
        assert!(p.profile.bands.iter().any(|b| b.kind == EqBandKind::LowShelf && b.gain_db == 2.0));
        assert_eq!(p.profile.name, "My sound · Car stereo");
        let t = g.reply("Cancel").await.unwrap();
        assert!(t.done && !t.confirmed);
    }
}
