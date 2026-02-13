//! Error types for the font subsystem.

use thiserror::Error;

/// Errors produced by the font subsystem.
#[derive(Debug, Error)]
pub enum FontError {
    /// Font file not found.
    #[error("font file not found: {path}")]
    NotFound { path: String },

    /// Font family not found in catalog.
    #[error("font family not found: {family}")]
    FamilyNotFound { family: String },

    /// Font installation failed.
    #[error("font installation failed: {reason}")]
    InstallFailed { reason: String },

    /// Font uninstallation failed.
    #[error("font uninstallation failed: {reason}")]
    UninstallFailed { reason: String },

    /// Collection not found.
    #[error("collection not found: {name}")]
    CollectionNotFound { name: String },

    /// Invalid font format.
    #[error("invalid font format: {path}")]
    InvalidFormat { path: String },

    /// Network error (e.g. fetching from Google Fonts).
    #[error("network error: {reason}")]
    NetworkError { reason: String },

    /// URL import rejected for safety.
    #[error("URL import rejected: {reason}")]
    UnsafeUrl { reason: String },

    /// Configuration error.
    #[error("font config error: {reason}")]
    ConfigError { reason: String },

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serde(String),

    /// Index error.
    #[error("index error: {reason}")]
    IndexError { reason: String },

    /// Git import error.
    #[error("git import error: {reason}")]
    GitError { reason: String },

    /// Glyph manipulation error.
    #[error("glyph error: {reason}")]
    GlyphError { reason: String },
}

/// Result type alias for font operations.
pub type Result<T> = std::result::Result<T, FontError>;
