pub type Result<T, E = PlayerError> = std::result::Result<T, E>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum PlayerError {
    #[error("could not open {0}")]
    Open(String),
    #[error("unsupported file: {0}")]
    Unsupported(String),
    #[error("decoding failed: {0}")]
    Decode(String),
    #[error("audio output: {0}")]
    Output(String),
    #[error("the output device can not play {0} Hz")]
    RateUnsupported(u32),
    #[error("resampler: {0}")]
    Resample(String),
}

impl From<homefi_core::audio::decode::DecodeError> for PlayerError {
    fn from(e: homefi_core::audio::decode::DecodeError) -> Self {
        use homefi_core::audio::decode::DecodeError as D;
        match e {
            D::Open(m) => Self::Open(m),
            D::Unsupported(m) => Self::Unsupported(m),
            D::Decode(m) => Self::Decode(m),
        }
    }
}
