//! DJ mode engine: two decks, a mixer and a sampler on one output stream.
//!
//! - [`DjPlayer`] owns the stream; the app calls [`DjPlayer::control`], [`DjPlayer::load`] and
//!   [`DjPlayer::snapshot`].
//! - [`DjMixer`] is what the audio callback runs: pure over its state, tested offline.
//! - The callback never allocates, blocks or frees: commands arrive through a lock-free ring,
//!   replaced songs go back through another one, and its state is published with a `try_lock`.
//!
//! DJ mode is never bit-perfect; the normal [`crate::Player`] is untouched while it is off.

mod control;
mod deck;
mod error;
mod loader;
mod mixer;
mod player;

pub use control::{
    AutoMixSnapshot, DeckId, DeckSnapshot, DjControl, DjEqBand, DjEvent, DjOutputInfo, DjSnapshot, DjTrackInfo,
};
pub use deck::DeckAudio;
pub use error::DjError;
pub use loader::{load_track, LoadedTrack};
pub use mixer::{AutoMixView, Command, DeckView, DjMixer, DjShared, MixerView};
pub use player::DjPlayer;

#[cfg(test)]
mod tests;
