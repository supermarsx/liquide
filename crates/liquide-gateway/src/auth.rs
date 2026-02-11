//! Authentication handlers for the gateway.

use crate::{GatewayError, Result};
use crate::config::ManagementApiConfig;

/// Supported authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayAuthMethod {
    /// Bearer token.
    Token,
    /// Username and password.
    UsernamePassword,
    /// OpenID Connect.
    Oidc,
    /// Mutual TLS client certificate.
    ClientCertificate,
    /// Static API key (primarily for management API).
    ApiKey,
}

impl std::fmt::Display for GatewayAuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token => write!(f, "token"),
            Self::UsernamePassword => write!(f, "username_password"),
            Self::Oidc => write!(f, "oidc"),
            Self::ClientCertificate => write!(f, "client_certificate"),
            Self::ApiKey => write!(f, "api_key"),
        }
    }
}

/// The result of an authentication attempt.
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authentication succeeded.
    Authenticated {
        /// Unique user identifier.
        user_id: String,
        /// Roles the user holds.
        roles: Vec<String>,
    },
    /// Authentication was denied.
    Denied {
        /// Human-readable denial reason.
        reason: String,
    },
    /// Multi-factor authentication is required.
    MfaRequired {
        /// Challenge to present to the client.
        challenge: AuthChallenge,
    },
}

/// A challenge the client must answer for MFA.
#[derive(Debug, Clone)]
pub struct AuthChallenge {
    /// Type of challenge (e.g. "totp", "webauthn").
    pub challenge_type: String,
    /// Opaque challenge payload.
    pub challenge_data: String,
}

/// Handles authentication for incoming client connections.
pub struct AuthHandler {
    management_config: ManagementApiConfig,
}

impl AuthHandler {
    /// Create a new auth handler.
    #[must_use]
    pub fn new(management_config: ManagementApiConfig) -> Self {
        Self { management_config }
    }

    /// Authenticate a client.
    ///
    /// In a real deployment this would delegate to OIDC, LDAP, etc.
    /// The stub implementation accepts any non-empty token.
    pub fn authenticate(
        &self,
        method: GatewayAuthMethod,
        credential: &str,
    ) -> Result<AuthResult> {
        match method {
            GatewayAuthMethod::Token => self.validate_token(credential),
            GatewayAuthMethod::UsernamePassword => {
                // Simple stub: accept if credential contains a colon separator.
                if let Some((user, pass)) = credential.split_once(':') {
                    if !user.is_empty() && !pass.is_empty() {
                        Ok(AuthResult::Authenticated {
                            user_id: user.to_string(),
                            roles: vec!["user".to_string()],
                        })
                    } else {
                        Ok(AuthResult::Denied {
                            reason: "empty username or password".to_string(),
                        })
                    }
                } else {
                    Err(GatewayError::AuthenticationFailed {
                        method: method.to_string(),
                        reason: "credential must be in user:pass format".to_string(),
                    })
                }
            }
            GatewayAuthMethod::Oidc => {
                // Stub: accept any non-empty credential as an OIDC assertion.
                if credential.is_empty() {
                    Ok(AuthResult::Denied {
                        reason: "empty OIDC assertion".to_string(),
                    })
                } else {
                    Ok(AuthResult::Authenticated {
                        user_id: format!("oidc-{}", &credential[..credential.len().min(8)]),
                        roles: vec!["user".to_string()],
                    })
                }
            }
            GatewayAuthMethod::ClientCertificate => {
                // Stub: accept any non-empty certificate fingerprint.
                if credential.is_empty() {
                    Ok(AuthResult::Denied {
                        reason: "no client certificate presented".to_string(),
                    })
                } else {
                    Ok(AuthResult::Authenticated {
                        user_id: format!("cert-{}", &credential[..credential.len().min(8)]),
                        roles: vec!["user".to_string()],
                    })
                }
            }
            GatewayAuthMethod::ApiKey => self.validate_api_key(credential),
        }
    }

    /// Validate a bearer token.
    pub fn validate_token(&self, token: &str) -> Result<AuthResult> {
        if token.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "empty token".to_string(),
            });
        }
        // Stub: accept any non-empty token.
        Ok(AuthResult::Authenticated {
            user_id: format!("token-{}", &token[..token.len().min(8)]),
            roles: vec!["user".to_string()],
        })
    }

    /// Validate a management API key.
    pub fn validate_api_key(&self, key: &str) -> Result<AuthResult> {
        if self.management_config.api_key.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "no API key configured".to_string(),
            });
        }
        if key == self.management_config.api_key {
            Ok(AuthResult::Authenticated {
                user_id: "admin".to_string(),
                roles: vec!["admin".to_string()],
            })
        } else {
            Ok(AuthResult::Denied {
                reason: "invalid API key".to_string(),
            })
        }
    }
}
