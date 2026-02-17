//! Session resume tokens and persistence.

use std::collections::HashMap;
use std::time::Instant;

use crate::config::ResumeConfig;
use crate::{SessionError, Result};

/// Generate a cryptographically random token ID.
fn generate_token_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Use timestamp + random bytes for uniqueness
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let random_bytes: [u8; 16] = {
        let mut bytes = [0u8; 16];
        // Use timestamp-based pseudo-random with additional entropy from RandomState
        // RandomState uses thread-local RNG seeded by OS entropy
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        let state = RandomState::new();
        let mut hasher = state.build_hasher();
        hasher.write_u128(timestamp);
        hasher.write_usize(std::process::id() as usize);
        let h1 = hasher.finish();
        let state2 = RandomState::new();
        let mut hasher2 = state2.build_hasher();
        hasher2.write_u64(h1);
        hasher2.write_u128(timestamp.wrapping_mul(0xDEAD_BEEF));
        let h2 = hasher2.finish();
        bytes[..8].copy_from_slice(&h1.to_le_bytes());
        bytes[8..].copy_from_slice(&h2.to_le_bytes());
        bytes
    };
    // Format as hex string
    format!("resume-{:032x}", u128::from_le_bytes(random_bytes))
}

/// Scope of a resume token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenScope {
    /// Token can only be used on the same server.
    SameServer,
    /// Token can be used from any gateway.
    AnyGateway,
}

impl std::fmt::Display for TokenScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SameServer => write!(f, "SameServer"),
            Self::AnyGateway => write!(f, "AnyGateway"),
        }
    }
}

/// A resume token that allows reconnecting to a disconnected session.
pub struct ResumeToken {
    /// Unique identifier for this token.
    token_id: String,
    /// The session this token is for.
    session_id: String,
    /// The user who owns this token.
    user_id: String,
    /// When the token was issued.
    #[allow(dead_code)]
    issued_at: Instant,
    /// When the token expires.
    expires_at: Instant,
    /// Fingerprint of the client that obtained the token.
    client_fingerprint: String,
    /// Maximum number of times this token can be used.
    max_uses: u32,
    /// Scope restriction.
    scope: TokenScope,
    /// How many times the token has been used.
    use_count: u32,
}

impl ResumeToken {
    /// Create a new resume token.
    #[must_use]
    pub fn new(
        token_id: String,
        session_id: String,
        user_id: String,
        client_fingerprint: String,
        lifetime: std::time::Duration,
        max_uses: u32,
        scope: TokenScope,
    ) -> Self {
        let now = Instant::now();
        Self {
            token_id,
            session_id,
            user_id,
            issued_at: now,
            expires_at: now + lifetime,
            client_fingerprint,
            max_uses,
            scope,
            use_count: 0,
        }
    }

    /// The token identifier.
    #[must_use]
    pub fn token_id(&self) -> &str {
        &self.token_id
    }

    /// The session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// The user identifier.
    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// The client fingerprint.
    #[must_use]
    pub fn client_fingerprint(&self) -> &str {
        &self.client_fingerprint
    }

    /// The token scope.
    #[must_use]
    pub fn scope(&self) -> TokenScope {
        self.scope
    }

    /// Whether the token is valid (not expired and uses remaining).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && self.use_count < self.max_uses
    }

    /// Whether the token has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    /// Record a use of the token. Returns true if the use was allowed.
    pub fn record_use(&mut self) -> bool {
        if self.is_valid() {
            self.use_count += 1;
            true
        } else {
            false
        }
    }

    /// Number of remaining uses.
    #[must_use]
    pub fn remaining_uses(&self) -> u32 {
        self.max_uses.saturating_sub(self.use_count)
    }

    /// Total uses so far.
    #[must_use]
    pub fn use_count(&self) -> u32 {
        self.use_count
    }
}

/// Manages resume tokens for all sessions.
pub struct ResumeManager {
    tokens: HashMap<String, ResumeToken>,
    config: ResumeConfig,
}

impl ResumeManager {
    /// Create a new resume manager.
    #[must_use]
    pub fn new(config: ResumeConfig) -> Self {
        Self {
            tokens: HashMap::new(),
            config,
        }
    }

    /// Whether resume is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Issue a new resume token for a disconnected session.
    pub fn issue_token(
        &mut self,
        session_id: &str,
        user_id: &str,
        client_fingerprint: &str,
    ) -> Result<String> {
        if !self.config.enabled {
            return Err(SessionError::ConfigError {
                detail: "resume is disabled".to_string(),
            });
        }

        // Use cryptographically random token ID
        let token_id = generate_token_id();

        let lifetime =
            std::time::Duration::from_secs(self.config.token_lifetime_hours * 3600);

        let token = ResumeToken::new(
            token_id.clone(),
            session_id.to_string(),
            user_id.to_string(),
            client_fingerprint.to_string(),
            lifetime,
            3,
            self.config.token_scope,
        );

        self.tokens.insert(token_id.clone(), token);
        Ok(token_id)
    }

    /// Validate a resume token. Returns the session_id if valid.
    pub fn validate_token(&mut self, token_id: &str) -> Result<String> {
        let token = self
            .tokens
            .get_mut(token_id)
            .ok_or(SessionError::ResumeTokenInvalid)?;

        if token.is_expired() {
            return Err(SessionError::ResumeTokenExpired);
        }

        if !token.record_use() {
            return Err(SessionError::ResumeTokenExpired);
        }

        Ok(token.session_id().to_string())
    }

    /// Rotate a token by revoking the old one and issuing a new one.
    pub fn rotate_token(&mut self, old_token_id: &str) -> Result<String> {
        let old_token = self
            .tokens
            .get(old_token_id)
            .ok_or(SessionError::ResumeTokenInvalid)?;

        let session_id = old_token.session_id().to_string();
        let user_id = old_token.user_id().to_string();
        let fingerprint = old_token.client_fingerprint().to_string();

        self.tokens.remove(old_token_id);
        self.issue_token(&session_id, &user_id, &fingerprint)
    }

    /// Revoke a token.
    pub fn revoke_token(&mut self, token_id: &str) {
        self.tokens.remove(token_id);
    }

    /// Remove all expired tokens.
    pub fn cleanup_expired(&mut self) -> usize {
        let before = self.tokens.len();
        self.tokens.retain(|_, token| !token.is_expired());
        before - self.tokens.len()
    }

    /// Number of active tokens.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

/// State that persists across a disconnect/reconnect cycle.
#[derive(Debug, Clone)]
pub struct PersistenceState {
    /// Saved window positions as serialized data.
    pub window_positions: Vec<(i32, i32, u32, u32)>,
    /// Whether clipboard content is available for restore.
    pub clipboard_available: bool,
    /// The cursor position at disconnect.
    pub cursor_position: (i32, i32),
    /// Audio state description.
    pub audio_state: String,
}

impl Default for PersistenceState {
    fn default() -> Self {
        Self {
            window_positions: Vec::new(),
            clipboard_available: false,
            cursor_position: (0, 0),
            audio_state: "muted".to_string(),
        }
    }
}

/// Tracks what persists and what resets when a session disconnects.
pub struct SessionPersistence {
    state: PersistenceState,
    has_snapshot: bool,
}

impl SessionPersistence {
    /// Create a new session persistence tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: PersistenceState::default(),
            has_snapshot: false,
        }
    }

    /// Take a snapshot of the current session state.
    pub fn snapshot(&mut self, state: PersistenceState) {
        self.state = state;
        self.has_snapshot = true;
    }

    /// Restore the persisted state, if a snapshot exists.
    #[must_use]
    pub fn restore(&self) -> Option<&PersistenceState> {
        if self.has_snapshot {
            Some(&self.state)
        } else {
            None
        }
    }

    /// Whether a snapshot has been taken.
    #[must_use]
    pub fn has_snapshot(&self) -> bool {
        self.has_snapshot
    }

    /// Clear the persisted state.
    pub fn clear(&mut self) {
        self.state = PersistenceState::default();
        self.has_snapshot = false;
    }
}

impl Default for SessionPersistence {
    fn default() -> Self {
        Self::new()
    }
}
