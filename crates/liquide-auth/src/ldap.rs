//! LDAP authentication backend.

use crate::provider::{AuthProvider, AuthResult, Credentials};

/// LDAP-based authentication provider.
pub struct LdapProvider {
    /// LDAP server URI (e.g. `ldaps://ldap.example.com`).
    pub uri: String,
    /// Base DN for user searches.
    pub base_dn: String,
}

impl LdapProvider {
    /// Create a new LDAP provider.
    #[must_use]
    pub fn new(uri: &str, base_dn: &str) -> Self {
        Self {
            uri: uri.to_string(),
            base_dn: base_dn.to_string(),
        }
    }
}

impl AuthProvider for LdapProvider {
    fn name(&self) -> &str {
        "ldap"
    }

    async fn authenticate(&self, _credentials: &Credentials) -> crate::Result<AuthResult> {
        todo!("LDAP authentication")
    }

    fn supports(&self, credentials: &Credentials) -> bool {
        matches!(credentials, Credentials::Password { .. })
    }
}
