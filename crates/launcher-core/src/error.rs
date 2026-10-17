use std::path::PathBuf;

use thiserror::Error;

/// Error type shared by every service in this crate.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("could not determine your home directory")]
    NoHomeDir,
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("no network connection ({0})")]
    Offline(String),
    #[error("request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{url} answered HTTP {status}")]
    HttpStatus { url: String, status: u16 },
    #[error("unexpected response from {url}: {reason}")]
    BadResponse { url: String, reason: String },
    #[error("the downloaded file did not match its checksum ({path})")]
    ChecksumMismatch { path: PathBuf, expected: String, actual: String },
    #[error("archive error: {0}")]
    Archive(String),
    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io { context: context.into(), source }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Classifies a reqwest failure: connection-level problems become
    /// [`CoreError::Offline`] so callers can degrade gracefully.
    pub fn from_reqwest(url: &str, source: reqwest::Error) -> Self {
        if source.is_connect() || source.is_timeout() || (source.is_request() && source.status().is_none()) {
            Self::Offline(source.to_string())
        } else {
            Self::Http { url: url.to_string(), source }
        }
    }

    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline(_))
    }
}

/// Extension to attach a human context to io errors.
pub trait IoContext<T> {
    fn context(self, context: impl Into<String>) -> Result<T, CoreError>;
}

impl<T> IoContext<T> for Result<T, std::io::Error> {
    fn context(self, context: impl Into<String>) -> Result<T, CoreError> {
        self.map_err(|e| CoreError::io(context, e))
    }
}
