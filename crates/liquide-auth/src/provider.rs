//! Unified authentication provider trait and result types.

use serde::{Deserialize, Serialize};

/// Credentials supplied by a user during authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
