//! Opt-in check of the song-picking prompts against a real provider and the real Tidal catalog (read-only:
//! searches only, nothing is saved). Never run in CI: it spends the key's tokens. See `docs/ai-eval.md`.
//!
//! ```bash
//! HOMEFI_EVAL_PROVIDER=openrouter HOMEFI_EVAL_KEY=... [HOMEFI_EVAL_MODEL=google/gemini-2.5-flash] \
//!   [HOMEFI_EVAL_ONLY=ride|concert] [HOMEFI_EVAL_ARTISTS="Korn,Dawid Podsiadło"] \
//!   cargo run -p homefi-core --example music_eval
//! ```
//!
//! It builds a ride from "polish music" with decades and genres, and a concert playlist for each artist, then
//! prints the songs found. The key is read from the environment and never printed.

use std::env;
use std::process::ExitCode;

use homefi_core::ai::{AiConfig, AiProvider};
use homefi_core::concert::{self, ConcertRequest};
use homefi_core::exercise::{self, ExerciseSettings, MusicWish, RideRequest, RideTaste};
use homefi_core::humane::Lang;
use homefi_core::playlist::{LibraryIndex, PlaylistSettings};
use homefi_core::session::Session;

const DEFAULT_ARTISTS: &str = "Korn,Dawid Podsiadło";

#[tokio::main]
async fn main() -> ExitCode {
    let Some(provider) = env::var("HOMEFI_EVAL_PROVIDER").ok().filter(|p| !p.trim().is_empty()) else {
        eprintln!("Set HOMEFI_EVAL_PROVIDER, HOMEFI_EVAL_KEY and optionally HOMEFI_EVAL_MODEL. See docs/ai-eval.md.");
        return ExitCode::from(2);
    };
    let Some(provider) = AiProvider::from_id(provider.trim(), None) else {
        eprintln!("Unknown provider `{provider}`.");
        return ExitCode::from(2);
    };
    let key = env::var("HOMEFI_EVAL_KEY").ok().map(|k| k.trim().to_owned()).filter(|k| !k.is_empty());
    let model = env::var("HOMEFI_EVAL_MODEL").unwrap_or_default();
    let only = env::var("HOMEFI_EVAL_ONLY").ok().map(|o| o.to_lowercase());
    let mut config = AiConfig::new(provider, model.trim(), key);
    match homefi_core::ai::resolve_model(&config).await {
        Ok(m) => config.model = m,
        Err(e) => {
            eprintln!("Can't pick a model: {e}");
            return ExitCode::FAILURE;
        }
    }
    let api = match Session::open_default().and_then(|s| s.api()) {
        Ok(api) => api,
        Err(e) => {
            eprintln!("Sign in to Tidal first (homefi auth login): {e}");
            return ExitCode::FAILURE;
        }
    };
    let library = LibraryIndex::default();
    println!("provider {} · model {}\n", config.provider.id(), config.model);

    if only.as_deref().is_none_or(|o| o == "ride") {
        let request = RideRequest {
            minutes: 30,
            wish: MusicWish {
                genres: vec!["rock".into(), "pop".into()],
                mix: true,
                sentence: "polish music".into(),
                taste: RideTaste { eras: vec![1990, 2000, 2010], ..RideTaste::default() },
            },
            ..RideRequest::default()
        };
        let settings = ExerciseSettings::default();
        let ctx = exercise::BuildContext {
            settings: &settings,
            ai: Some(&config),
            api: Some(&api),
            library: &library,
            analyzer: None,
            lang: Lang::Es,
        };
        println!("== Ride: \"polish music\", rock + pop, 1990s-2010s");
        match exercise::build(&request, &ctx, &|_| {}).await {
            Ok(built) => {
                for s in &built.plan.songs {
                    println!(
                        "  {} — {}  [{} BPM]",
                        s.entry.artist,
                        s.entry.title,
                        s.bpm.map_or("?".into(), |b| b.to_string())
                    );
                }
                for note in &built.plan.notes {
                    println!("  note: {note}");
                }
            }
            Err(e) => println!("  FAILED: {}", e.humane_in(Lang::Es)),
        }
        println!();
    }

    if only.as_deref().is_none_or(|o| o == "concert") {
        let artists = env::var("HOMEFI_EVAL_ARTISTS").unwrap_or_else(|_| DEFAULT_ARTISTS.into());
        let settings = PlaylistSettings::default();
        let ctx = concert::BuildContext {
            settings: &settings,
            ai: Some(&config),
            api: Some(&api),
            library: &library,
            lang: Lang::Es,
        };
        for artist in artists.split(',').map(str::trim).filter(|a| !a.is_empty()) {
            println!("== Concert: {artist}");
            let request = ConcertRequest { artist: artist.into(), ..ConcertRequest::default() };
            match concert::build(&request, &ctx, &|_| {}).await {
                Ok(built) => {
                    if let Some(note) = &built.fallback_note {
                        println!("  fallback: {note}");
                    }
                    for e in &built.playlist.entries {
                        println!("  {} — {}", e.artist, e.title);
                    }
                    for s in &built.playlist.sources {
                        println!("  source: {}", s.url);
                    }
                }
                Err(e) => println!("  FAILED: {}", e.humane_in(Lang::Es)),
            }
            println!();
        }
    }
    ExitCode::SUCCESS
}
