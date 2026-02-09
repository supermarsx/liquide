//! OpenID Connect (OIDC) authentication backend.

use crate::provider::{AuthProvider, AuthResult, Credentials};

/// OIDC provider configuration.
pub struct OidcProvider {
    /// The OIDC issuer URL.
    pub issuer: String,
    /// Client ID registered with the identity provider.
    pub client_id: String,
}

impl OidcProvider {
    /// Create a new OIDC provider.
    #[must_use]
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
        }
    }
}

impl AuthProvider for OidcProvider {
    fn name(&self) -> &str {
        "oidc"
    }

    async fn authenticate(&self, _credentials: &Credentials) -> crate::Result<AuthResult> {
        todo!("OIDC authentication")
    }

    fn supports(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::OidcToken { .. })
    }
}
