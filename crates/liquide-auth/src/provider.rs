//! Unified authentication provider trait and result types.

use serde::{Deserialize, Serialize};

/// Credentials supplied by a user during authentication.
#[derive(Clone, Serialize, Deserialize)]
pub enum Credentials {
    /// Username and password.
    Password { username: String, password: String },
    /// An X.509 client certificate (DER-encoded).
    Certificate { der: Vec<u8> },
    /// An OIDC / OAuth2 bearer token.
    OidcToken { token: String },
    /// A TOTP or other MFA code paired with primary credentials.
    Mfa {
        primary: Box<Credentials>,
        code: String,
    },
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password { username, .. } => f
                .debug_struct("Password")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::Certificate { der } => f
                .debug_struct("Certificate")
                .field("der", &format_args!("[REDACTED {} bytes]", der.len()))
                .finish(),
            Self::OidcToken { .. } => f
                .debug_struct("OidcToken")
                .field("token", &"[REDACTED]")
                .finish(),
            Self::Mfa { primary, .. } => f
                .debug_struct("Mfa")
                .field("primary", primary)
                .field("code", &"[REDACTED]")
                .finish(),
        }
    }
}

/// The outcome of an authentication attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthResult {
    /// Authentication succeeded; contains the authenticated user identity.
    Success { user_id: String, display_name: String },
    /// Authentication failed with a human-readable reason.
    Failure { reason: String },
    /// An additional MFA challenge is required before the attempt can complete.
    MfaRequired { challenge: String },
}

/// Trait implemented by each authentication backend.
#[allow(async_fn_in_trait)]
pub trait AuthProvider: Send + Sync {
    /// The human-readable name of this provider (e.g. `"pam"`, `"ldap"`).
    fn name(&self) -> &str;

    /// Attempt to authenticate the given credentials.
    async fn authenticate(&self, credentials: &Credentials) -> super::Result<AuthResult>;

    /// Check whether this provider supports the given credential kind.
    fn supports(&self, credentials: &Credentials) -> bool;
}
