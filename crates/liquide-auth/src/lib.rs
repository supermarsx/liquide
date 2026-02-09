#![doc = "Authentication providers for the Liquide session server."]
#![doc = ""]
#![doc = "Supports PAM, LDAP, OIDC, multi-factor authentication, and"]
#![doc = "X.509 certificate-based authentication through a unified"]
#![doc = "`AuthProvider` trait."]

pub mod ldap;
pub mod mfa;
pub mod oidc;
pub mod pam;
pub mod provider;

pub use provider::{AuthProvider, AuthResult, Credentials};

use thiserror::Error;

/// Errors produced by the authentication subsystem.
#[derive(Debug, Error)]
pub enum AuthError {
    /// The supplied credentials were invalid.
    #[error("invalid credentials for user {user:?}")]
    InvalidCredentials { user: String },

    /// The authentication backend is unreachable.
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    /// MFA challenge was not satisfied.
    #[error("MFA verification failed")]
    MfaFailed,

    /// The certificate presented by the client is invalid or expired.
    #[error("certificate authentication failed: {0}")]
    CertificateInvalid(String),

    /// An internal / unexpected error.
    #[error("internal auth error: {0}")]
    Internal(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, AuthError>;
