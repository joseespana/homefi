//! DJ mode: the pure parts (no audio device, Android-safe).
//!
//! - [`settings`]: every option, with defaults and ranges in one place.
//! - [`analysis`]: BPM, beat grid, colored overview and loudness of a song, plus its disk cache.
//! - [`mixer_math`]: channel EQ with kill, bipolar filter, crossfader curves, auto-gain.
//! - [`sync`]: tempo and phase matching between two beat grids.
//! - [`sfx`]: the eight synthesized sampler one-shots.
//! - [`keymap`]: optional keyboard shortcuts (defaults, overrides, conflicts).
//! - [`layout`]: section order / visibility and the sound of each pad.
//! - [`setups`]: named setups (the whole settings), exportable as `.homefidj`.
//!
//! The real-time engine lives in `homefi-player::dj` and only calls into this module.

pub mod analysis;
#[cfg(feature = "decode")]
pub mod file;
pub mod keymap;
pub mod layout;
pub mod mixer_math;
pub mod settings;
pub mod setups;
pub mod sfx;
pub mod sync;

pub use analysis::{analyze, AnalysisCache, DjAnalysis, OverviewSlice};
#[cfg(feature = "decode")]
pub use file::{analyze_file, FileAnalysisError};
pub use layout::{DjLayout, DjSection, DjSectionKind};
pub use mixer_math::{CrossfaderCurve, EqGains};
pub use settings::{
    free_deck, DeckUse, DjLatency, DjSettings, HomifitoDj, HomifitoMode, HomifitoPlace, LoadTarget, MixerConfig,
};
pub use setups::{export_setup, import_setup, DjSetup, DjSetupStore};
pub use sfx::SfxKind;
pub use sync::Grid;

/// Decks in DJ mode.
pub const DECK_COUNT: usize = 2;
/// Hot cues per deck.
pub const HOT_CUES: usize = 8;
/// Sampler pads.
pub const PAD_COUNT: usize = 8;
