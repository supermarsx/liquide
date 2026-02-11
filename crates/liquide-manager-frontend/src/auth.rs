//! Authentication state, roles, and session management.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Authentication state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthState {
    /// No credentials have been supplied.
    Unauthenticated,
    /// Login is in progress.
    Authenticating,
    /// Successfully authenticated.
    Authenticated {
        username: String,
        role: AuthRole,
        token: String,
        expires_at: u64,
    },
    /// Authentication failed.
    Failed { reason: String },
}

impl Default for AuthState {
    fn default() -> Self {
        Self::Unauthenticated
    }
}

/// Frontend authentication role (mirrors backend `AdminRole`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AuthRole {
    /// Read-only access.
    Viewer,
    /// Viewer + session control.
    Operator,
    /// Operator + policy/config editing.
    Admin,
    /// Full access including user management.
    SuperAdmin,
}

impl fmt::Display for AuthRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Viewer => write!(f, "viewer"),
            Self::Operator => write!(f, "operator"),
            Self::Admin => write!(f, "admin"),
            Self::SuperAdmin => write!(f, "super-admin"),
        }
    }
}

impl AuthRole {
    /// Whether this role has at least the given permission level.
    #[must_use]
    pub fn has_permission(&self, required: AuthRole) -> bool {
        *self >= required
    }
}

/// Login credentials submitted by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    /// Username.
    pub username: String,
    /// Password (plaintext for transit — TLS protects the wire).
    pub password: String,
}

impl LoginCredentials {
    /// Create new login credentials.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

/// An active authentication session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    /// Authenticated username.
    pub username: String,
    /// Role granted to this session.
    pub role: AuthRole,
    /// Bearer token.
    pub token: String,
    /// Epoch-seconds at which this session expires.
    pub expires_at: u64,
}

impl AuthSession {
    /// Create a new session.
    #[must_use]
    pub fn new(
        username: impl Into<String>,
        role: AuthRole,
        token: impl Into<String>,
        expires_at: u64,
    ) -> Self {
        Self {
            username: username.into(),
            role,
            token: token.into(),
            expires_at,
        }
    }

    /// Whether this session has expired relative to the given timestamp.
    #[must_use]
    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }
}

/// Manages authentication state for the frontend application.
#[derive(Debug, Clone)]
pub struct AuthManager {
    state: AuthState,
    session: Option<AuthSession>,
}

impl AuthManager {
    /// Create a new unauthenticated manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AuthState::Unauthenticated,
            session: None,
        }
    }

    /// Current authentication state.
    #[must_use]
    pub fn state(&self) -> &AuthState {
        &self.state
    }

    /// Current session, if authenticated.
    #[must_use]
    pub fn current_session(&self) -> Option<&AuthSession> {
        self.session.as_ref()
    }

    /// Begin a login attempt (transitions to `Authenticating`).
    pub fn login(&mut self, _credentials: &LoginCredentials) {
        self.state = AuthState::Authenticating;
    }

    /// Complete a successful login by storing the session.
    pub fn complete_login(&mut self, session: AuthSession) {
        self.state = AuthState::Authenticated {
            username: session.username.clone(),
            role: session.role,
            token: session.token.clone(),
            expires_at: session.expires_at,
        };
        self.session = Some(session);
    }

    /// Record a login failure.
    pub fn fail_login(&mut self, reason: impl Into<String>) {
        self.state = AuthState::Failed {
            reason: reason.into(),
        };
        self.session = None;
    }

    /// Log out and clear the session.
    pub fn logout(&mut self) {
        self.state = AuthState::Unauthenticated;
        self.session = None;
    }

    /// Refresh the session token and expiry.
    pub fn refresh(&mut self, new_token: impl Into<String>, new_expires_at: u64) {
        if let Some(session) = &mut self.session {
            session.token = new_token.into();
            session.expires_at = new_expires_at;
            self.state = AuthState::Authenticated {
                username: session.username.clone(),
                role: session.role,
                token: session.token.clone(),
                expires_at: session.expires_at,
            };
        }
    }

    /// Whether there is an active, non-expired session.
    #[must_use]
    pub fn is_authenticated(&self, now: u64) -> bool {
        matches!(&self.session, Some(s) if !s.is_expired(now))
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}
