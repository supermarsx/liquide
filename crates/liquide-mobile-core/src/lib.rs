//! Shared mobile client core library for LiquiDE.
//!
//! Provides cross-platform types and logic for the iOS and Android
//! mobile clients, including touch input handling, gesture recognition,
//! adaptive quality control, codec negotiation, and platform abstraction.

pub mod codec;
pub mod config;
pub mod connection;
pub mod display;
pub mod gesture;
pub mod input;
pub mod keyboard;
pub mod platform;
pub mod policy;
pub mod quality;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the mobile core library.
#[derive(Debug, Error)]
pub enum MobileError {
    /// The client is not currently connected to a server.
    #[error("not connected to server")]
    NotConnected,

    /// A connection attempt failed.
    #[error("connection failed: {reason}")]
    ConnectionFailed { reason: String },

    /// Authentication with the server was rejected.
    #[error("authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    /// The requested codec is not supported on this platform.
    #[error("codec not supported: {codec}")]
    CodecNotSupported { codec: String },

    /// The session has expired according to policy.
    #[error("session expired")]
    SessionExpired,

    /// An action was blocked by policy.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// A platform-specific operation failed.
    #[error("platform error: {0}")]
    PlatformError(String),

    /// A gesture could not be recognized from the input.
    #[error("gesture not recognized")]
    GestureNotRecognized,

    /// An I/O error occurred.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, MobileError>;

// Re-exports for convenience.
pub use config::MobileConfig;
