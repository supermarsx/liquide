//! PAM (Pluggable Authentication Modules) backend.

use crate::provider::{AuthProvider, AuthResult, Credentials};

/// PAM-based authentication provider for local Unix accounts.
pub struct PamProvider {
    /// The PAM service name (e.g. `"liquide"`).
    pub service: String,
}

impl PamProvider {
    /// Create a new PAM provider using the given service name.
    #[must_use]
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }
}

impl AuthProvider for PamProvider {
    fn name(&self) -> &str {
        "pam"
    }

    async fn authenticate(&self, _credentials: &Credentials) -> crate::Result<AuthResult> {
        todo!("PAM authentication")
    }

    fn supports(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Password { .. })
    }
}
