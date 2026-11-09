use crate::PlayerError;

/// Why DJ mode could not start or load a song. The messages are written for the user.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DjError {
    #[error("this song needs about {needed_mb} MB of memory, and DJ mode may use {limit_mb} MB. Raise the DJ memory limit in the settings or pick a shorter song")]
    TooBig { needed_mb: u64, limit_mb: u64 },
    #[error("{0}")]
    Unreadable(String),
    #[error("the song is empty")]
    Empty,
    #[error("the load was replaced by a newer one")]
    Cancelled,
    #[error("could not start the DJ audio: {0}")]
    Output(String),
}

impl From<PlayerError> for DjError {
    fn from(e: PlayerError) -> Self {
        match e {
            PlayerError::Output(m) => Self::Output(m),
            other => Self::Unreadable(other.to_string()),
        }
    }
}

impl From<homefi_core::audio::decode::DecodeError> for DjError {
    fn from(e: homefi_core::audio::decode::DecodeError) -> Self {
        Self::Unreadable(e.to_string())
    }
}
