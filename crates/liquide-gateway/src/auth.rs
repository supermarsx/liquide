//! Authentication handlers for the gateway.

use liquide_auth::provider::{
    AuthProvider, Credentials as ProviderCredentials,
};
use liquide_auth::pam::PamProvider;

use crate::{GatewayError, Result};
use crate::config::ManagementApiConfig;
use crate::management::constant_time_eq;

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
///
/// Delegates username/password authentication to `liquide-auth`'s
/// [`PamProvider`] when configured for [`GatewayAuthMethod::UsernamePassword`].
pub struct AuthHandler {
    management_config: ManagementApiConfig,
    /// PAM provider for username/password authentication.
    pam_provider: PamProvider,
}

impl AuthHandler {
    /// Create a new auth handler.
    #[must_use]
    pub fn new(management_config: ManagementApiConfig) -> Self {
        Self {
            management_config,
            pam_provider: PamProvider::new("liquide"),
        }
    }

    /// Authenticate a client.
    ///
    /// For `UsernamePassword`, delegates to the `liquide-auth` PAM provider.
    /// Other methods use local validation stubs.
    pub fn authenticate(
        &self,
        method: GatewayAuthMethod,
        credential: &str,
    ) -> Result<AuthResult> {
        match method {
            GatewayAuthMethod::Token => self.validate_token(credential),
            GatewayAuthMethod::UsernamePassword => {
                self.authenticate_username_password(credential)
            }
            GatewayAuthMethod::Oidc => {
                if credential.is_empty() {
                    Ok(AuthResult::Denied {
                        reason: "empty OIDC assertion".to_string(),
                    })
                } else {
                    Ok(AuthResult::Denied {
                        reason: "OIDC authentication backend not implemented".to_string(),
                    })
                }
            }
            GatewayAuthMethod::ClientCertificate => {
                if credential.is_empty() {
                    Ok(AuthResult::Denied {
                        reason: "no client certificate presented".to_string(),
                    })
                } else {
                    Ok(AuthResult::Denied {
                        reason: "client certificate authentication backend not implemented".to_string(),
                    })
                }
            }
            GatewayAuthMethod::ApiKey => self.validate_api_key(credential),
        }
    }

    /// Authenticate using the PAM provider from `liquide-auth`.
    fn authenticate_username_password(&self, credential: &str) -> Result<AuthResult> {
        let Some((user, pass)) = credential.split_once(':') else {
            return Err(GatewayError::AuthenticationFailed {
                method: "username_password".to_string(),
                reason: "credential must be in user:pass format".to_string(),
            });
        };
        if user.is_empty() || pass.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "empty username or password".to_string(),
            });
        }

        // Build liquide-auth credentials and check provider support.
        let creds = ProviderCredentials::Password {
            username: user.to_string(),
            password: pass.to_string(),
        };
        if !self.pam_provider.supports(&creds) {
            return Ok(AuthResult::Denied {
                reason: "PAM provider does not support this credential type".to_string(),
            });
        }

        // The PAM provider's `authenticate()` is async. For the synchronous
        // code path, return a safe denial. Use `authenticate_username_password_async`
        // when calling from an async context (e.g. handle_tcp_connection).
        Ok(AuthResult::Denied {
            reason: "PAM authentication requires async context — use async auth path".to_string(),
        })
    }

    /// Async version of username/password authentication that calls the PAM
    /// provider directly.
    pub async fn authenticate_username_password_async(
        &self,
        credential: &str,
    ) -> Result<AuthResult> {
        let Some((user, pass)) = credential.split_once(':') else {
            return Err(GatewayError::AuthenticationFailed {
                method: "username_password".to_string(),
                reason: "credential must be in user:pass format".to_string(),
            });
        };
        if user.is_empty() || pass.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "empty username or password".to_string(),
            });
        }

        let creds = ProviderCredentials::Password {
            username: user.to_string(),
            password: pass.to_string(),
        };
        if !self.pam_provider.supports(&creds) {
            return Ok(AuthResult::Denied {
                reason: "PAM provider does not support this credential type".to_string(),
            });
        }

        match self.pam_provider.authenticate(&creds).await {
            Ok(liquide_auth::provider::AuthResult::Success { user_id, .. }) => {
                Ok(AuthResult::Authenticated {
                    user_id,
                    roles: vec!["user".to_string()],
                })
            }
            Ok(liquide_auth::provider::AuthResult::Failure { reason }) => {
                Ok(AuthResult::Denied { reason })
            }
            Ok(liquide_auth::provider::AuthResult::MfaRequired { challenge }) => {
                Ok(AuthResult::MfaRequired {
                    challenge: AuthChallenge {
                        challenge_type: "mfa".to_string(),
                        challenge_data: challenge,
                    },
                })
            }
            Err(e) => Ok(AuthResult::Denied {
                reason: format!("PAM backend error: {e}"),
            }),
        }
    }

    /// Async authenticate dispatcher — calls the async-capable backend for
    /// methods that need it.
    pub async fn authenticate_async(
        &self,
        method: GatewayAuthMethod,
        credential: &str,
    ) -> Result<AuthResult> {
        match method {
            GatewayAuthMethod::UsernamePassword => {
                self.authenticate_username_password_async(credential).await
            }
            // All other methods are synchronous and can delegate directly.
            other => self.authenticate(other, credential),
        }
    }

    /// Validate a bearer token.
    pub fn validate_token(&self, token: &str) -> Result<AuthResult> {
        if token.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "empty token".to_string(),
            });
        }
        Ok(AuthResult::Denied {
            reason: "token authentication backend not implemented".to_string(),
        })
    }

    /// Validate a management API key.
    pub fn validate_api_key(&self, key: &str) -> Result<AuthResult> {
        if self.management_config.api_key.is_empty() {
            return Ok(AuthResult::Denied {
                reason: "no API key configured".to_string(),
            });
        }
        if constant_time_eq(key.as_bytes(), self.management_config.api_key.as_bytes()) {
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
