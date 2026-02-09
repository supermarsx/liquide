//! Common error types shared across the Liquide workspace.

use thiserror::Error;

/// The canonical error type for the Liquide project.
#[derive(Debug, Error)]
pub enum LiquideError {
    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration file could not be parsed.
    #[error("configuration error: {0}")]
    Config(String),

    /// A TOML deserialization error.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Authentication failed.
    #[error("authentication error: {0}")]
    Auth(String),

    /// Authorization / policy denied.
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    /// A requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An internal bug or invariant violation.
    #[error("internal error: {0}")]
    Internal(String),

    /// Catch-all for wrapped anyhow errors.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A convenience `Result` type that uses [`LiquideError`].
pub type Result<T> = std::result::Result<T, LiquideError>;
