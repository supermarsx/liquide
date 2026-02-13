//! Error types for vector cursor rendering

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, VectorCursorError>;

/// Errors that can occur during vector cursor operations
#[derive(Debug, Error)]
pub enum VectorCursorError {
    /// SVG parsing error
    #[error("Failed to parse SVG: {0}")]
    SvgParse(String),

    /// Rendering error
    #[error("Failed to render cursor: {0}")]
    RenderFailed(String),

    /// Invalid cursor shape
    #[error("Invalid cursor shape: {0}")]
    InvalidShape(String),

    /// Invalid size
    #[error("Invalid size: {0}")]
    InvalidSize(String),

    /// Resource not found
    #[error("Cursor resource not found: {0}")]
    NotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Image error
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),
}
