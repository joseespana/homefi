//! HomeFi desktop audio engine.
//!
//! Decodes with symphonia (FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis, ...),
//! plays through cpal on the device the user picks (USB DAC, jack, Bluetooth,
//! HDMI, ...) and opens the device at the file's sample rate when it can; only
//! when the device can't, a sinc resampler (rubato) converts.
//! A parametric equalizer (`homefi_core::dsp`) can run in the output callback
//! ([`Player::set_eq`]), per output device, with taste layers per track
//! ([`Player::set_taste`]) and loudness normalization ([`Player::set_loudness`]).
//! Crossfade ([`Player::set_crossfade`]) mixes the next track in as the
//! current one fades out; off by default (gapless, bit-perfect). Stereo on a
//! surround device is spread as the user picks ([`Player::set_output_options`]).

mod deck;
mod decoder;
mod engine;
mod error;
mod output;
mod visual;

pub use deck::{ResamplerQuality, SurroundMode};
pub use decoder::Source;
pub use engine::{Fading, Listener, OutputOptions, Player, RepeatMode, Snapshot, SoundSource, Status, StreamInfo};
pub use error::{PlayerError, Result};
pub use homefi_core::dsp::crossfade::{CrossfadeMode, CrossfadeSettings, FadeCurve};
pub use output::{list_devices, OutputDevice};
pub use visual::Visual;
