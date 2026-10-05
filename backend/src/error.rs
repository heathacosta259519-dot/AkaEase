use serde::Serialize;

pub type Result<T> = std::result::Result<T, BackendError>;

/// Deliberately contains no raw response body, credential or signed URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum BackendError {
    #[error("local storage is unavailable or invalid")]
    Storage,
    #[error("another desktop process owns this state directory")]
    AlreadyRunning,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("network request failed")]
    Network,
    #[error("request timed out")]
    Timeout,
    #[error("HTTP status {0}")]
    Http(u16),
    #[error("service returned code {0}")]
    Service(i64),
    #[error("unexpected service response: {0}")]
    Protocol(String),
    #[error("track is unavailable for this session")]
    Unavailable,
    #[error("login required or session expired")]
    Unauthorized,
    #[error("operation belongs to an obsolete session")]
    StaleOperation,
    #[error("login check is in progress or polling too frequently")]
    Busy,
    #[error("backend service has shut down")]
    ServiceClosed,
    #[error("secure credential storage is unavailable")]
    CredentialStorage,
    #[error("audio operation failed: {0}")]
    Audio(String),
}

impl From<reqwest::Error> for BackendError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::Timeout
        } else {
            Self::Network
        }
    }
}
