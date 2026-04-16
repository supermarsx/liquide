//! Common error types shared across the Liquide workspace.
//!
//! Every crate in the workspace defines its own fine-grained error enum for
//! internal use.  [`LiquideError`] acts as the *unified cross-crate* error
//! type: crates that need to propagate errors outward should provide a
//! `From<CrateError> for LiquideError` impl (typically a one-liner that maps
//! into the appropriate variant below).

use thiserror::Error;

/// The canonical error type for the Liquide project.
///
/// Crate-level error types should implement `From<CrateError> for LiquideError`
/// so that error propagation across crate boundaries is seamless.
#[derive(Debug, Error)]
pub enum LiquideError {
    // ── generic ────────────────────────────────────────────────────────

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

    /// A requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An internal bug or invariant violation.
    #[error("internal error: {0}")]
    Internal(String),

    // ── security / identity ────────────────────────────────────────────

    /// Authentication failed.
    #[error("authentication error: {0}")]
    Auth(String),

    /// Authorization / policy denied.
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    /// Cryptographic operation failed.
    #[error("crypto error: {0}")]
    Crypto(String),

    // ── networking / protocol ──────────────────────────────────────────

    /// Transport-level error (QUIC, WebSocket, TLS).
    #[error("transport error: {0}")]
    Transport(String),

    /// Wire-protocol error (framing, version mismatch, CRC).
    #[error("protocol error: {0}")]
    Protocol(String),

    // ── media / peripherals ────────────────────────────────────────────

    /// Audio subsystem error.
    #[error("audio error: {0}")]
    Audio(String),

    /// Display / rendering error.
    #[error("display error: {0}")]
    Display(String),

    // ── extensibility ──────────────────────────────────────────────────

    /// Plugin subsystem error.
    #[error("plugin error: {0}")]
    Plugin(String),

    /// Catch-all for wrapped anyhow errors.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A convenience `Result` type that uses [`LiquideError`].
pub type Result<T> = std::result::Result<T, LiquideError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_display_io() {
        let err = LiquideError::Io(io::Error::new(io::ErrorKind::NotFound, "file missing"));
        let msg = format!("{err}");
        assert!(msg.contains("I/O error"));
    }

    #[test]
    fn test_error_display_config() {
        let err = LiquideError::Config("bad value".to_string());
        assert_eq!(format!("{err}"), "configuration error: bad value");
    }

    #[test]
    fn test_error_display_not_found() {
        let err = LiquideError::NotFound("widget".to_string());
        assert_eq!(format!("{err}"), "not found: widget");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err: LiquideError = io_err.into();
        assert!(matches!(err, LiquideError::Io(_)));
    }

    #[test]
    fn test_error_from_toml() {
        let toml_result: std::result::Result<toml::Value, _> = toml::from_str("{{{invalid");
        let toml_err = toml_result.unwrap_err();
        let err: LiquideError = toml_err.into();
        assert!(matches!(err, LiquideError::Toml(_)));
    }
}
