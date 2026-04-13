//! Account subsystem error types.

/// Errors produced by account management operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AccountError {
    /// No account with the given UID exists.
    #[error("account not found")]
    NotFound,
    /// The caller does not have permission to perform the operation.
    #[error("permission denied")]
    PermissionDenied,
    /// A user with the requested username already exists.
    #[error("account already exists")]
    AlreadyExists,
    /// The supplied password does not meet the password policy.
    #[error("weak password: {0}")]
    WeakPassword(String),
    /// A platform-specific error occurred.
    #[error("platform error: {0}")]
    PlatformError(String),
    /// The requested username is invalid (contains illegal characters, etc.).
    #[error("invalid username: {0}")]
    InvalidUsername(String),
}
