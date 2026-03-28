//! Account subsystem error types.

use std::fmt;

/// Errors produced by account management operations.
#[derive(Debug, Clone)]
pub enum AccountError {
    /// No account with the given UID exists.
    NotFound,
    /// The caller does not have permission to perform the operation.
    PermissionDenied,
    /// A user with the requested username already exists.
    AlreadyExists,
    /// The supplied password does not meet the password policy.
    WeakPassword(String),
    /// A platform-specific error occurred.
    PlatformError(String),
    /// The requested username is invalid (contains illegal characters, etc.).
    InvalidUsername(String),
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "account not found"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::AlreadyExists => write!(f, "account already exists"),
            Self::WeakPassword(reason) => write!(f, "weak password: {reason}"),
            Self::PlatformError(msg) => write!(f, "platform error: {msg}"),
            Self::InvalidUsername(reason) => write!(f, "invalid username: {reason}"),
        }
    }
}

impl std::error::Error for AccountError {}
