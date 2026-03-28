//! Error types for font management operations.

use thiserror::Error;

/// Errors produced by font management operations.
#[derive(Debug, Error)]
pub enum FontError {
    /// Font file not found at the given path.
    #[error("font file not found: {path}")]
    NotFound {
        /// The path that was looked up.
        path: String,
    },

    /// The file has an unrecognized or unsupported format.
    #[error("unsupported font format: {path}")]
    UnsupportedFormat {
        /// Path to the offending file.
        path: String,
    },

    /// Cannot uninstall a system-installed font.
    #[error("cannot uninstall system font: {path}")]
    SystemFont {
        /// Path of the system font.
        path: String,
    },

    /// Font installation failed.
    #[error("install failed: {reason}")]
    InstallFailed {
        /// Human-readable reason.
        reason: String,
    },

    /// The user font directory could not be determined.
    #[error("user font directory not found")]
    NoUserFontDir,

    /// An I/O error occurred.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
