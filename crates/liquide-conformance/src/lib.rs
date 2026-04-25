//! Protocol conformance test runner for validating LiquiDE server implementations.
//!
//! Provides test case definitions, protocol validators, and a conformance
//! runner for exercising the LiquiDE protocol suites: handshake, authentication,
//! streaming, clipboard, security, and interoperability.

pub mod auth;
pub mod case;
pub mod css;
pub mod clipboard;
pub mod config;
pub mod handshake;
pub mod report;
pub mod runner;
pub mod security;
pub mod streaming;
pub mod suite;
pub mod validator;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the conformance runner.
#[derive(Debug, Error)]
pub enum ConformanceError {
    /// Unknown test suite name.
    #[error("unknown suite: {name}")]
    UnknownSuite { name: String },

    /// A specific test case failed with an explanation.
    #[error("test case failed: {id}: {reason}")]
    TestFailed { id: String, reason: String },

    /// The server could not be contacted.
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// Protocol-level error during conformance testing.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),

    /// Timeout waiting for server response.
    #[error("timeout after {ms}ms")]
    Timeout { ms: u64 },

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, ConformanceError>;

// Re-exports for convenience.
pub use config::ConformanceConfig;
pub use runner::ConformanceRunner;
