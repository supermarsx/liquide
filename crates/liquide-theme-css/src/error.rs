//! Error types for theme system

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, ThemeError>;

/// Errors that can occur in the theme system
#[derive(Debug, Error)]
pub enum ThemeError {
    /// CSS parsing error
    #[error("CSS parse error at {location}: {message}")]
    ParseError { message: String, location: String },

    /// Invalid selector
    #[error("Invalid selector: {0}")]
    InvalidSelector(String),

    /// Invalid property value
    #[error("Invalid property value for '{property}': {value}")]
    InvalidValue { property: String, value: String },

    /// Property not found
    #[error("Property '{0}' not found")]
    PropertyNotFound(String),

    /// Selector not found
    #[error("No styles found for selector '{0}'")]
    SelectorNotFound(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// File watch error
    #[error("File watch error: {0}")]
    Watch(#[from] notify::Error),

    /// Color parse error
    #[error("Failed to parse color '{0}': {1}")]
    ColorParse(String, String),

    /// Missing required property
    #[error("Missing required property: {0}")]
    MissingProperty(String),
}
