//! HomeFi desktop audio engine.
//!
//! Decodes with symphonia (FLAC, ALAC, WAV, AIFF, MP3, AAC, Vorbis, ...),
//! plays through cpal on the device the user picks (USB DAC, jack, Bluetooth,
//! HDMI, ...) and opens the device at the file's sample rate when it can; only
//! when the device can't, a sinc resampler (rubato) converts.

mod decoder;
mod engine;
mod error;
mod output;
mod visual;

pub use decoder::Source;
pub use engine::{Listener, Player, RepeatMode, Snapshot, Status, StreamInfo};
pub use error::{PlayerError, Result};
pub use output::{list_devices, OutputDevice};
pub use visual::Visual;
