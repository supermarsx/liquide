use std::fmt;

/// Errors that can occur in locale operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocaleError {
    /// Failed to parse a locale string.
    InvalidLocale(String),
    /// Failed to parse a translation catalog.
    ParseError(String),
}

impl fmt::Display for LocaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLocale(s) => write!(f, "invalid locale: {}", s),
            Self::ParseError(s) => write!(f, "parse error: {}", s),
        }
    }
}

impl std::error::Error for LocaleError {}
