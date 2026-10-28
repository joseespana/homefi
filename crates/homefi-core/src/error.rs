use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{user_message}, {status}/{sub_status}")]
    Api { status: u16, sub_status: String, user_message: String },

    #[error("{error}, {description}, {status}/{sub_status}")]
    Auth { status: u16, error: String, sub_status: String, description: String },

    #[error("not logged in: {0} (run `homefi auth login`)")]
    NotLoggedIn(&'static str),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("unexpected response from `{endpoint}`: {message}")]
    Decode { endpoint: String, message: String },

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("template error: {0}")]
    Template(String),

    #[error("could not parse stream: {0}")]
    Stream(String),

    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),

    #[error("metadata error in {path}: {message}")]
    Metadata { path: PathBuf, message: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("invalid resource: {0}")]
    Resource(String),

    /// Shared control: joining failed or the hub connection broke. The text is
    /// user-facing (e.g. the hub's `denied` reason).
    #[error("{0}")]
    Share(String),

    /// AutoEq (headphone profiles) failed; the text is already user-facing.
    #[error("{0}")]
    AutoEq(String),

    /// An AI provider call failed (bad key, no credit, offline...).
    #[error(transparent)]
    Ai(#[from] crate::ai::AiError),
}

impl Error {
    /// `true` when Tidal reports the device code is still waiting for the user.
    pub fn is_authorization_pending(&self) -> bool {
        matches!(self, Error::Auth { error, .. } if error == "authorization_pending")
    }

    /// `true` for failures worth trying again a bit later: Tidal busy
    /// (`429`, `5xx`) or a connection that could not be made / timed out.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Api { status, .. } => *status == 429 || (500..=599).contains(status) && *status != 501,
            Error::Http(e) => e.is_connect() || e.is_timeout(),
            _ => false,
        }
    }

    /// `true` when the device code expired before the user approved it.
    pub fn is_expired_token(&self) -> bool {
        matches!(self, Error::Auth { error, .. } if error == "expired_token")
    }
}
