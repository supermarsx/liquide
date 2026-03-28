use std::fmt;

/// Errors returned by datetime operations.
#[derive(Debug, Clone)]
pub enum TimeError {
    /// The requested timezone ID was not found in the database.
    UnknownTimezone(String),
    /// A platform command failed or returned unexpected output.
    PlatformError(String),
    /// An invalid date or time value was provided.
    InvalidValue(String),
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeError::UnknownTimezone(id) => write!(f, "unknown timezone: {}", id),
            TimeError::PlatformError(msg) => write!(f, "platform error: {}", msg),
            TimeError::InvalidValue(msg) => write!(f, "invalid value: {}", msg),
        }
    }
}

impl std::error::Error for TimeError {}
