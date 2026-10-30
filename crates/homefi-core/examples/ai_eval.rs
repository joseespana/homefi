//! Opt-in evaluation of the EQ assistant against a real provider. Never run
//! in CI: it spends tokens. See `docs/ai-eval.md`.
//!
//! ```bash
//! HOMEFI_EVAL_PROVIDER=anthropic HOMEFI_EVAL_KEY=... cargo run -p homefi-core --example ai_eval
//! ```
//!
//! Each scripted persona talks to [`EqAssistant`]: its replies are given in
//! order, a proposal on the table is answered with "yes", and the run is
//! scored. The key is read from the environment and never printed; neither
//! are the prompts.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;

use homefi_core::ai::eq_assistant::MAX_BOOST_DB;
use homefi_core::ai::{AiConfig, AiProvider, AssistantContext, AudioOutput, EqAssistant, Message, OutputKind};
use homefi_core::dsp::autoeq::AutoEq;
use homefi_core::dsp::{
    EqBandKind, TasteBand, TasteCreator, TasteIntensity, TasteRule, TasteScope, TasteSettings, Tilt, MAX_TASTE_BANDS,
    MAX_TASTE_BAND_DB, MAX_TILT_DB, SMALL_SPEAKER_MAX_BASS_DB,
};
use serde_json::Value;

/// User replies before giving up on a proposal.
const MAX_USER_TURNS: usize = 9;
/// The assistant promises at most this many questions.
const MAX_QUESTIONS: usize = 6;

struct Persona {
    name: &'static str,
    es: bool,
    outputs: Vec<AudioOutput>,
    /// The headphone is in AutoEq: the model should use its measurement.
    measured: bool,
    replies: &'static [&'static str],
    /// Taste layers they already have.
    taste: TasteSettings,
    /// Opened from this rule's editor ("Ask the AI to refine"): its index in `taste`.
    refine: Option<usize>,
    /// Asked for a neutral sound: the change doesn't have to be audible.
    neutral: bool,
    /// How many bands (a tilt knob counts as one) the proposal should have, when the persona asks for a clear change.
    bands: Option<(usize, usize)>,
}

/// David Guetta with a little bass and a soft 3.2 kHz cut: the refine persona edits that cut.
fn guetta() -> TasteSettings {
    let mut t = TasteSettings::default();
    t.upsert(
        TasteRule::new(
            TasteScope::Artist { id: String::new(), name: "David Guetta".into() },
            Tilt { bass_db: 2.0, ..Default::default() },
            "More punch for David Guetta",
            TasteCreator::Ai,
        )
        .with_bands(vec![TasteBand::new(EqBandKind::Peak, 3200.0, -1.5, 3.0).with_note("softens the harsh edge")]),
    );
    t
}

fn output(id: &str, name: &str, kind: OutputKind) -> AudioOutput {
    AudioOutput { id: id.into(), name: name.into(), kind, sample_rates: vec![44100, 48000] }
}

fn personas() -> Vec<Persona> {
    vec![
        Persona {
            name: "XM5 BT, reggaetón, más bajo",
            es: true,
            outputs: vec![output("bt", "WH-1000XM5", OutputKind::Bluetooth)],
            measured: true,
            replies: &[
                "Unos Sony WH-1000XM5",
                "Reggaetón y trap",
                "Quiero más bajo",
                "En la calle y en el metro",
                "Bastante alto",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "MacBook speakers, podcasts",
            es: true,
            outputs: vec![output("mac", "MacBook Pro Speakers", OutputKind::BuiltIn)],
            measured: false,
            replies: &[
                "Los parlantes de la MacBook",
                "Podcasts y algo de música",
                "Que las voces se entiendan mejor",
                "En casa, volumen medio",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "HD 650 USB DAC, jazz, neutral",
            es: false,
            outputs: vec![output("dac", "USB DAC", OutputKind::UsbDac)],
            measured: true,
            replies: &[
                "Sennheiser HD 650",
                "A small USB DAC, nothing special",
                "Jazz, mostly acoustic",
                "As neutral as possible",
                "At home, moderate volume",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: true,
            bands: None,
        },
        Persona {
            name: "Car stereo, rock, noisy",
            es: false,
            outputs: vec![output("car", "CarPlay", OutputKind::Car)],
            measured: false,
            replies: &["In my car", "Rock", "It's noisy on the highway and the vocals get lost", "Pretty loud"],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "AirPods Pro 2, pop, chillón",
            es: true,
            outputs: vec![output("bt", "AirPods Pro", OutputKind::Bluetooth)],
            measured: true,
            replies: &[
                "AirPods Pro 2",
                "Pop y baladas",
                "A veces suena chillón y me cansa",
                "En la oficina, volumen bajo",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "Phone speaker, hip-hop, thin",
            es: false,
            outputs: vec![output("phone", "Phone speaker", OutputKind::BuiltIn)],
            measured: false,
            replies: &["The phone's own speaker", "Hip-hop", "It sounds thin, I want more bass", "In my room"],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "DT 770 + DAC, clásica, apagado",
            es: true,
            outputs: vec![
                output("speaker", "JBL Flip 6", OutputKind::Bluetooth),
                output("dac", "USB DAC", OutputKind::UsbDac),
            ],
            measured: true,
            replies: &[
                "Los audífonos con el DAC",
                "Beyerdynamic DT 770 Pro 80 ohm",
                "Música clásica",
                "Lo siento apagado",
                "En casa, volumen medio",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "Galaxy Buds2 Pro, house, boomy",
            es: false,
            outputs: vec![output("bt", "Galaxy Buds2 Pro", OutputKind::Bluetooth)],
            measured: true,
            replies: &[
                "Samsung Galaxy Buds2 Pro",
                "Electronic and house",
                "The bass is boomy and muddy",
                "Commuting on the train",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "Refine Guetta layer, cymbals hurt",
            es: false,
            outputs: vec![output("dac", "USB DAC", OutputKind::UsbDac)],
            measured: false,
            replies: &["The cymbals hurt a bit and the s's hiss", "Just that, keep the bass"],
            taste: guetta(),
            refine: Some(0),
            neutral: false,
            bands: None,
        },
        Persona {
            name: "Capa para Bad Bunny, bocina del celular",
            es: true,
            outputs: vec![output("phone", "Phone speaker", OutputKind::BuiltIn)],
            measured: false,
            replies: &["Quiero una capa para Bad Bunny con más pegada", "Sí, en el celular", "Que no distorsione"],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            bands: None,
        },
        Persona {
            name: "SOAD nu-metal, HD 650 + DAC, que pegue más",
            es: true,
            outputs: vec![output("dac", "USB DAC", OutputKind::UsbDac)],
            measured: true,
            replies: &[
                "Sennheiser HD 650",
                "Un DAC USB sencillo",
                "System of a Down y nu-metal",
                "Que pegue más",
                "En casa, volumen medio",
            ],
            taste: TasteSettings::default(),
            refine: None,
            neutral: false,
            // 2 to 4 bands, the biggest at least as big as Balanced's smallest main move
            bands: Some((2, 4)),
        },
    ]
}

/// What one conversation scored.
#[derive(Default)]
struct Score {
    user_turns: usize,
    /// Assistant turn (1-based) with the first proposal.
    proposed_at: Option<usize>,
    questions: usize,
    used_autoeq: bool,
    caps_ok: bool,
    /// Taste curves: valid bands, analyze_sound before designing, no two bands at one place.
    curves_ok: bool,
    /// Taste curves designed (proposed or saved with bands).
    curves: usize,
    /// The biggest move the model asked for (dB: a band, a profile band or a tilt knob).
    main_move_db: f32,
    /// Bands of its biggest curve (a tilt knob counts as one).
    curve_bands: usize,
    language_ok: bool,
    claims_ok: bool,
    applied: bool,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let Some(provider) = env::var("HOMEFI_EVAL_PROVIDER").ok().filter(|p| !p.trim().is_empty()) else {
        eprintln!("Set HOMEFI_EVAL_PROVIDER (anthropic, gemini, openai, openrouter, ollama...), HOMEFI_EVAL_KEY and optionally HOMEFI_EVAL_MODEL. See docs/ai-eval.md.");
        return ExitCode::from(2);
    };
    let base_url = env::var("HOMEFI_EVAL_BASE_URL").ok().filter(|u| !u.trim().is_empty());
    let Some(provider) = AiProvider::from_id(provider.trim(), base_url.as_deref()) else {
        eprintln!("Unknown provider `{provider}`.");
        return ExitCode::from(2);
    };
    let key = env::var("HOMEFI_EVAL_KEY").ok().filter(|k| !k.trim().is_empty());
    let model = env::var("HOMEFI_EVAL_MODEL").unwrap_or_default();
    let only = env::var("HOMEFI_EVAL_ONLY").ok().map(|o| o.to_lowercase());
    let verbose = env::var("HOMEFI_EVAL_VERBOSE").is_ok_and(|v| v == "1");
    let mut config = AiConfig::new(provider, model.trim(), key);
    config.base_url = base_url;
    let model = match homefi_core::ai::resolve_model(&config).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Can't pick a model: {e}");
            return ExitCode::FAILURE;
        }
    };
    config.model = model.clone();
    let cache = env::temp_dir().join("homefi-ai-eval-autoeq");
    let autoeq = Arc::new(AutoEq::new(cache));

    println!("provider {} · model {model}\n", config.provider.id());
    let mut rows = Vec::new();
    for persona in personas() {
        if only.as_ref().is_some_and(|o| !persona.name.to_lowercase().contains(o.as_str())) {
            continue;
        }
        let score = run(&config, &persona, autoeq.clone(), verbose).await;
        rows.push((persona, score));
    }
    print_table(&rows);
    ExitCode::SUCCESS
}

async fn run(config: &AiConfig, persona: &Persona, autoeq: Arc<AutoEq>, verbose: bool) -> Score {
    let context = AssistantContext {
        language: if persona.es { "es".into() } else { "en".into() },
        outputs: persona.outputs.clone(),
        taste: persona.taste.clone(),
        focus_rule: persona.refine.and_then(|i| persona.taste.rules.get(i)).map(|r| r.id.clone()),
        ..Default::default()
    };
    let mut assistant = EqAssistant::new(config.clone(), context).with_autoeq(autoeq);
    let mut score = Score::default();
    let mut texts: Vec<String> = Vec::new();
    let mut replies = persona.replies.iter();
    let yes = if persona.es { "Sí, aplícalo" } else { "Yes, apply it" };
    let dunno = if persona.es { "No sé, decide tú" } else { "I don't know, you decide" };
    let mut turn = assistant.start().await;
    let mut assistant_turns = 0;
    loop {
        let t = match turn {
            Ok(t) => t,
            Err(e) => {
                score.error = Some(e.to_string().chars().take(80).collect());
                break;
            }
        };
        assistant_turns += 1;
        if verbose {
            println!("  [{}] {}", persona.name, t.text.replace('\n', " "));
        }
        texts.push(t.text.clone());
        if t.confirmed || t.taste_change.is_some() {
            score.applied = true;
            break;
        }
        if (t.proposal.is_some() || t.taste_proposal.is_some()) && score.proposed_at.is_none() {
            score.proposed_at = Some(assistant_turns);
        } else if score.proposed_at.is_none() && t.text.contains('?') {
            score.questions += 1;
        }
        if t.done || score.user_turns >= MAX_USER_TURNS {
            break;
        }
        let on_table = t.proposal.is_some() || t.taste_proposal.is_some();
        let answer = if on_table { yes } else { replies.next().copied().unwrap_or(dunno) };
        if verbose {
            println!("  [{}] > {answer}", persona.name);
        }
        score.user_turns += 1;
        turn = assistant.reply(answer).await;
    }
    let small_speaker = persona.outputs.iter().all(|o| o.kind == OutputKind::BuiltIn);
    let calls: Vec<(&str, &Value)> = assistant
        .state()
        .messages
        .iter()
        .filter_map(|m| match m {
            Message::Assistant { tool_calls, .. } => Some(tool_calls),
            _ => None,
        })
        .flatten()
        .map(|c| (c.name.as_str(), &c.arguments))
        .collect();
    score.used_autoeq = calls.iter().any(|(name, args)| {
        *name == "get_headphone_profile"
            || (*name == "propose_eq" && args["headphone_path"].as_str().is_some_and(|p| !p.is_empty()))
    });
    score.caps_ok =
        calls.iter().filter(|(name, _)| *name == "propose_eq").all(|(_, args)| caps_ok(args, small_speaker));
    (score.main_move_db, score.curve_bands) = biggest_move(&calls);
    let (curves_ok, curves) = curves_ok(&calls, small_speaker);
    score.curves_ok = curves_ok;
    score.curves = curves;
    score.language_ok = texts.iter().all(|t| language_ok(t, persona.es));
    score.claims_ok = texts.iter().all(|t| claims_ok(t));
    score
}

/// The biggest move the model asked for and how many bands its biggest curve
/// has, from its own numbers (before the app clamps them): taste bands,
/// profile bands and tilt knobs of `propose_taste_rule`, `set_taste_rule` and
/// `propose_eq`. A tilt knob counts as one band.
fn biggest_move(calls: &[(&str, &Value)]) -> (f32, usize) {
    let num = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    let (mut main_move_db, mut curve_bands) = (0f32, 0usize);
    for (name, args) in calls {
        if !matches!(*name, "propose_taste_rule" | "set_taste_rule" | "propose_eq") {
            continue;
        }
        let tilt = if *name == "propose_eq" { &args["taste"] } else { &args["tilt"] };
        let knobs: Vec<f32> = ["bassDb", "trebleDb", "clarityDb"].iter().map(|k| num(&tilt[k]).abs()).collect();
        let bands = args["bands"].as_array().or_else(|| args["profile"]["bands"].as_array());
        let band_gains = bands.into_iter().flatten().map(|b| num(&b["gainDb"]).abs());
        let moves = knobs.iter().copied().chain(band_gains);
        main_move_db = main_move_db.max(moves.fold(0f32, f32::max));
        let audible_knobs = knobs.iter().filter(|k| **k >= 0.5).count();
        curve_bands = curve_bands.max(bands.map_or(0, Vec::len) + audible_knobs);
    }
    (main_move_db, curve_bands)
}

/// The model's own numbers (before the app clamps them) respect the caps.
fn caps_ok(args: &Value, small_speaker: bool) -> bool {
    let num = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    let taste = &args["taste"];
    let tilt_ok = ["bassDb", "trebleDb", "clarityDb"].iter().all(|k| num(&taste[k]).abs() <= MAX_TILT_DB);
    let bass_ok = !small_speaker || num(&taste["bassDb"]) <= SMALL_SPEAKER_MAX_BASS_DB;
    let bands = args["profile"]["bands"].as_array().cloned().unwrap_or_default();
    let bands_ok = bands.iter().all(|b| {
        let gain = num(&b["gainDb"]);
        gain <= MAX_BOOST_DB && !(small_speaker && gain > 0.0 && num(&b["freqHz"]) < 150.0)
    });
    tilt_ok && bass_ok && bands_ok
}

/// Every taste curve the model sent: bands within the caps (and no deep
/// boost on a small speaker), analyze_sound called before it, and no two
/// bands of one kind within a third of an octave (stacking). Also counts them.
fn curves_ok(calls: &[(&str, &Value)], small_speaker: bool) -> (bool, usize) {
    let num = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    let mut ok = true;
    let mut curves = 0;
    for (i, (name, args)) in calls.iter().enumerate() {
        if !matches!(*name, "propose_taste_rule" | "set_taste_rule") {
            continue;
        }
        let Some(bands) = args["bands"].as_array().filter(|b| !b.is_empty()) else { continue };
        curves += 1;
        let analyzed = calls[..i].iter().any(|(n, _)| *n == "analyze_sound");
        let valid = bands.len() <= MAX_TASTE_BANDS
            && bands.iter().all(|b| {
                let (gain, q, f) = (num(&b["gainDb"]), num(&b["q"]), num(&b["freqHz"]));
                let peak = b["kind"].as_str() == Some("peak");
                let kind_ok = ["peak", "lowShelf", "highShelf", "highPass", "lowPass"]
                    .contains(&b["kind"].as_str().unwrap_or(""));
                kind_ok
                    && gain.abs() <= MAX_TASTE_BAND_DB
                    && (!peak || (0.3..=8.0).contains(&q))
                    && (20.0..=20_000.0).contains(&f)
                    && !(small_speaker && gain > 0.0 && f < 150.0)
                    && b["note"].as_str().is_some_and(|n| !n.trim().is_empty())
            });
        let stacked = bands.iter().enumerate().any(|(a, x)| {
            bands[a + 1..].iter().any(|y| {
                x["kind"] == y["kind"]
                    && (num(&x["freqHz"]).max(1.0) / num(&y["freqHz"]).max(1.0)).log2().abs() < 1.0 / 3.0
            })
        });
        ok &= analyzed && valid && !stacked;
    }
    (ok, curves)
}

/// A rough guess from common words; texts too short to tell pass.
fn language_ok(text: &str, es: bool) -> bool {
    const ES: &[&str] = &[
        "el", "la", "los", "las", "que", "qué", "de", "y", "tu", "tus", "para", "con", "una", "es", "más", "cómo",
        "cuál", "sí", "te", "lo", "por", "usas", "escuchas",
    ];
    const EN: &[&str] = &[
        "the", "and", "you", "your", "to", "is", "for", "with", "what", "how", "which", "do", "it", "of", "more",
        "are", "listen",
    ];
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).collect();
    let accents = lower.chars().filter(|c| "áéíóúñ¿¡".contains(*c)).count();
    let es_n = words.iter().filter(|w| ES.contains(w)).count() + accents;
    let en_n = words.iter().filter(|w| EN.contains(w)).count();
    if es_n + en_n < 3 {
        return true;
    }
    if es {
        es_n > en_n
    } else {
        en_n > es_n
    }
}

/// No promises of studio quality or a perfect sound.
fn claims_ok(text: &str) -> bool {
    let lower = text.to_lowercase();
    !["studio quality", "studio-quality", "calidad de estudio", "perfect sound", "sonido perfecto"]
        .iter()
        .any(|c| lower.contains(c))
}

/// The change is clearly audible: the biggest move reaches the smallest main
/// move of the Balanced intensity (neutral personas are exempt), and a persona
/// that expects a number of bands gets them.
fn audible_ok(p: &Persona, s: &Score) -> bool {
    let loud_enough = p.neutral || s.main_move_db >= TasteIntensity::Balanced.limits().main_move_db.0;
    let bands_ok = p.bands.is_none_or(|(lo, hi)| (lo..=hi).contains(&s.curve_bands));
    loud_enough && bands_ok
}

fn print_table(rows: &[(Persona, Score)]) {
    let mark = |ok: bool| if ok { "ok" } else { "FAIL" };
    println!(
        "{:<44} {:<4} {:>5} {:>9} {:>7} {:>5} {:>7} {:>9} {:>6} {:>5} {:>7} {:>7} {:>6}",
        "persona",
        "lang",
        "turns",
        "proposal",
        "autoeq",
        "caps",
        "curves",
        "audible",
        "<=6 q",
        "lang",
        "claims",
        "applied",
        "score"
    );
    let mut total = 0;
    let mut max = 0;
    for (p, s) in rows {
        let proposal_ok = s.proposed_at.is_some();
        let checks = [
            proposal_ok,
            !p.measured || s.used_autoeq,
            s.caps_ok,
            s.curves_ok && (p.refine.is_none() || s.curves > 0),
            audible_ok(p, s),
            s.questions <= MAX_QUESTIONS,
            s.language_ok,
            s.claims_ok,
            s.applied,
        ];
        let passed = checks.iter().filter(|c| **c).count();
        total += passed;
        max += checks.len();
        println!(
            "{:<44} {:<4} {:>5} {:>9} {:>7} {:>5} {:>7} {:>9} {:>6} {:>5} {:>7} {:>7} {:>4}/{}",
            p.name,
            if p.es { "es" } else { "en" },
            s.user_turns,
            s.proposed_at.map_or("FAIL".to_owned(), |t| format!("turn {t}")),
            if p.measured { mark(s.used_autoeq) } else { "-" },
            mark(s.caps_ok),
            if s.curves == 0 && p.refine.is_none() {
                "-".to_owned()
            } else {
                format!("{} {}", s.curves, mark(s.curves_ok && s.curves > 0))
            },
            if p.neutral { "-".to_owned() } else { format!("{:.1} {}", s.main_move_db, mark(audible_ok(p, s))) },
            format!("{} {}", s.questions, if s.questions <= MAX_QUESTIONS { "ok" } else { "!" }),
            mark(s.language_ok),
            mark(s.claims_ok),
            mark(s.applied),
            passed,
            checks.len(),
        );
        if let Some(e) = &s.error {
            println!("    error: {e}");
        }
    }
    if max > 0 {
        println!("\ntotal {total}/{max} ({:.0}%)", 100.0 * total as f32 / max as f32);
    }
}
