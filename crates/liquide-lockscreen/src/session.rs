/// Login session management.
///
/// Tracks the authentication flow for a single login session:
/// user selection, provider selection, credential entry, and
/// lockout enforcement.  Designed after GDM/SDDM session patterns.
use crate::auth::AuthResult;
use crate::provider::{CredentialField, ProviderRegistry};

// ---------------------------------------------------------------------------
// SessionState
// ---------------------------------------------------------------------------

/// State machine for the login session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Greeter is showing the user list — no user selected yet.
    SelectingUser,
    /// User selected, choosing an authentication provider.
    SelectingProvider,
    /// Credential fields are displayed, waiting for user input.
    EnteringCredentials,
    /// Authentication is in progress (awaiting backend response).
    Authenticating,
    /// Authentication succeeded.
    Authenticated,
    /// Authentication failed (with message).
    Failed(String),
}

// ---------------------------------------------------------------------------
// LoginSession
// ---------------------------------------------------------------------------

/// Manages a single login session.
///
/// Tracks state transitions, attempt counts, and lockout policy.
pub struct LoginSession {
    state: SessionState,
    /// Currently selected username (set by `select_user`).
    selected_user: Option<String>,
    /// Currently selected provider id (set by `select_provider`).
    selected_provider_id: Option<String>,
    /// Total failed attempts in this session.
    attempt_count: u32,
    /// Lock out after this many consecutive failures.
    lockout_after_n_failures: u32,
    /// Lockout duration in milliseconds.
    lockout_duration_ms: u64,
    /// Timestamp (ms) when the current lockout started, or `None`.
    lockout_start_ms: Option<u64>,
}

const USERNAME_FIELD_ID: &str = "username";

impl LoginSession {
    /// Create a new session with default lockout policy.
    pub fn new() -> Self {
        Self {
            state: SessionState::SelectingUser,
            selected_user: None,
            selected_provider_id: None,
            attempt_count: 0,
            lockout_after_n_failures: 5,
            lockout_duration_ms: 30_000,
            lockout_start_ms: None,
        }
    }

    /// Create a session with custom lockout parameters.
    pub fn with_lockout(max_failures: u32, lockout_ms: u64) -> Self {
        Self {
            lockout_after_n_failures: max_failures,
            lockout_duration_ms: lockout_ms,
            ..Self::new()
        }
    }

    // -- state accessors --

    /// Current session state.
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// The selected username, if any.
    pub fn selected_user(&self) -> Option<&str> {
        self.selected_user.as_deref()
    }

    /// The selected provider id, if any.
    pub fn selected_provider_id(&self) -> Option<&str> {
        self.selected_provider_id.as_deref()
    }

    /// Number of failed authentication attempts so far.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    // -- lockout --

    /// Whether the session is currently locked out at timestamp `now_ms`.
    pub fn is_locked_out(&self, now_ms: u64) -> bool {
        if let Some(start) = self.lockout_start_ms {
            now_ms < start + self.lockout_duration_ms
        } else {
            false
        }
    }

    /// Milliseconds remaining in the lockout, or 0 if not locked out.
    pub fn remaining_lockout_ms(&self, now_ms: u64) -> u64 {
        if let Some(start) = self.lockout_start_ms {
            let end = start + self.lockout_duration_ms;
            if now_ms < end { end - now_ms } else { 0 }
        } else {
            0
        }
    }

    // -- state transitions --

    /// Select a user.  Moves from `SelectingUser` to `SelectingProvider`.
    pub fn select_user(&mut self, username: &str) {
        self.selected_user = Some(username.to_string());
        self.selected_provider_id = None;
        self.state = SessionState::SelectingProvider;
    }

    /// Select a credential provider.  Moves to `EnteringCredentials`.
    ///
    /// Returns `false` if the provider id is not found in the registry.
    pub fn select_provider(&mut self, provider_id: &str, registry: &ProviderRegistry) -> bool {
        if self.selected_user.is_none() {
            return false;
        }
        if registry.get(provider_id).is_none() {
            return false;
        }
        self.selected_provider_id = Some(provider_id.to_string());
        self.state = SessionState::EnteringCredentials;
        true
    }

    /// Submit filled credential fields.  Performs authentication via the
    /// provider registry and updates state accordingly.
    ///
    /// `now_ms` is the current timestamp used for lockout tracking.
    pub fn submit(
        &mut self,
        fields: &[CredentialField],
        registry: &ProviderRegistry,
        now_ms: u64,
    ) -> AuthResult {
        // Check lockout first
        if self.is_locked_out(now_ms) {
            let remaining = self.remaining_lockout_ms(now_ms);
            return AuthResult::Locked(remaining);
        }

        // Clear expired lockout
        if let Some(start) = self.lockout_start_ms {
            if now_ms >= start + self.lockout_duration_ms {
                self.lockout_start_ms = None;
                self.attempt_count = 0;
            }
        }

        let provider_id = match &self.selected_provider_id {
            Some(id) => id.clone(),
            None => {
                self.state = SessionState::Failed("No provider selected.".into());
                return AuthResult::Failed("No provider selected.".into());
            }
        };

        let selected_user = match &self.selected_user {
            Some(username) => username.clone(),
            None => {
                self.state = SessionState::Failed("No user selected.".into());
                return AuthResult::Failed("No user selected.".into());
            }
        };

        let provider = match registry.get(&provider_id) {
            Some(p) => p,
            None => {
                self.state = SessionState::Failed("Provider not found.".into());
                return AuthResult::Failed("Provider not found.".into());
            }
        };

        let mut bound_fields = Vec::with_capacity(fields.len() + 1);
        bound_fields.push(CredentialField::new(USERNAME_FIELD_ID, &selected_user));
        bound_fields.extend(
            fields
                .iter()
                .filter(|field| field.descriptor_id != USERNAME_FIELD_ID)
                .cloned(),
        );

        self.state = SessionState::Authenticating;
        let result = provider.authenticate(&bound_fields);

        match &result {
            AuthResult::Success => {
                self.state = SessionState::Authenticated;
                self.attempt_count = 0;
                self.lockout_start_ms = None;
            }
            AuthResult::Failed(msg) => {
                self.attempt_count += 1;
                if self.attempt_count >= self.lockout_after_n_failures {
                    self.lockout_start_ms = Some(now_ms);
                    self.state = SessionState::Failed(format!(
                        "Too many attempts. Locked for {} seconds.",
                        self.lockout_duration_ms / 1000
                    ));
                } else {
                    self.state = SessionState::Failed(msg.clone());
                }
            }
            AuthResult::Locked(ms) => {
                self.lockout_start_ms = Some(now_ms);
                self.state =
                    SessionState::Failed(format!("Account locked for {} seconds.", ms / 1000));
            }
            AuthResult::RequiresMfa => {
                self.state = SessionState::Failed("Multi-factor authentication required.".into());
            }
        }

        result
    }

    /// Reset the session back to user selection.
    pub fn reset(&mut self) {
        self.state = SessionState::SelectingUser;
        self.selected_user = None;
        self.selected_provider_id = None;
        self.attempt_count = 0;
        self.lockout_start_ms = None;
    }

    /// Go back one step in the flow.
    pub fn go_back(&mut self) {
        match &self.state {
            SessionState::SelectingProvider => {
                self.state = SessionState::SelectingUser;
                self.selected_user = None;
            }
            SessionState::EnteringCredentials | SessionState::Failed(_) => {
                self.state = SessionState::SelectingProvider;
                self.selected_provider_id = None;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{PasswordProvider, PinProvider};

    fn make_registry() -> ProviderRegistry {
        let mut reg = ProviderRegistry::new();
        let mut pw = PasswordProvider::new();
        pw.add_user("alice", "secret");
        pw.add_user("bob", "bobpw");
        reg.register(Box::new(pw));

        let mut pin = PinProvider::new();
        pin.add_user("alice", "1234");
        reg.register(Box::new(pin));
        reg
    }

    #[test]
    fn initial_state() {
        let s = LoginSession::new();
        assert_eq!(*s.state(), SessionState::SelectingUser);
        assert!(s.selected_user().is_none());
        assert!(s.selected_provider_id().is_none());
        assert_eq!(s.attempt_count(), 0);
        assert!(!s.is_locked_out(0));
    }

    #[test]
    fn select_user_transitions() {
        let mut s = LoginSession::new();
        s.select_user("alice");
        assert_eq!(*s.state(), SessionState::SelectingProvider);
        assert_eq!(s.selected_user(), Some("alice"));
    }

    #[test]
    fn select_provider_transitions() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        assert!(s.select_provider("password", &reg));
        assert_eq!(*s.state(), SessionState::EnteringCredentials);
        assert_eq!(s.selected_provider_id(), Some("password"));
    }

    #[test]
    fn select_nonexistent_provider_returns_false() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        assert!(!s.select_provider("fingerprint", &reg));
        // State unchanged
        assert_eq!(*s.state(), SessionState::SelectingProvider);
    }

    #[test]
    fn select_provider_requires_selected_user() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        assert!(!s.select_provider("password", &reg));
        assert_eq!(*s.state(), SessionState::SelectingUser);
        assert!(s.selected_provider_id().is_none());
    }

    #[test]
    fn submit_success() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("password", &reg);
        let fields = vec![CredentialField::new("password", "secret")];
        let result = s.submit(&fields, &reg, 1000);
        assert_eq!(result, AuthResult::Success);
        assert_eq!(*s.state(), SessionState::Authenticated);
        assert_eq!(s.attempt_count(), 0);
    }

    #[test]
    fn submit_overwrites_conflicting_username_with_selected_user() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("password", &reg);
        let fields = vec![
            CredentialField::new("username", "bob"),
            CredentialField::new("password", "secret"),
        ];
        let result = s.submit(&fields, &reg, 1000);
        assert_eq!(result, AuthResult::Success);
        assert_eq!(*s.state(), SessionState::Authenticated);
    }

    #[test]
    fn submit_requires_selected_user() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        assert!(s.select_provider("password", &reg) == false);
        let fields = vec![CredentialField::new("password", "secret")];
        let result = s.submit(&fields, &reg, 0);
        assert!(matches!(result, AuthResult::Failed(msg) if msg.contains("No provider")));
    }

    #[test]
    fn submit_failure_increments_count() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("password", &reg);
        let fields = vec![CredentialField::new("password", "wrong")];
        let result = s.submit(&fields, &reg, 1000);
        assert!(matches!(result, AuthResult::Failed(_)));
        assert_eq!(s.attempt_count(), 1);
        assert!(matches!(s.state(), SessionState::Failed(_)));
    }

    #[test]
    fn lockout_after_max_failures() {
        let reg = make_registry();
        let mut s = LoginSession::with_lockout(3, 10_000);
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 1000);
        s.submit(&bad, &reg, 2000);
        s.submit(&bad, &reg, 3000); // 3rd failure -> lockout
        assert!(s.is_locked_out(3000));
        assert_eq!(s.attempt_count(), 3);
    }

    #[test]
    fn locked_out_submit_returns_locked() {
        let reg = make_registry();
        let mut s = LoginSession::with_lockout(2, 5_000);
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 100);
        s.submit(&bad, &reg, 200); // locked at 200

        let good = vec![CredentialField::new("password", "secret")];
        let result = s.submit(&good, &reg, 1000); // still locked
        assert!(matches!(result, AuthResult::Locked(_)));
    }

    #[test]
    fn lockout_expires() {
        let reg = make_registry();
        let mut s = LoginSession::with_lockout(2, 5_000);
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 100);
        s.submit(&bad, &reg, 200); // locked at 200

        assert!(s.is_locked_out(5199));
        assert!(!s.is_locked_out(5200)); // expired

        let good = vec![CredentialField::new("password", "secret")];
        let result = s.submit(&good, &reg, 5200);
        assert_eq!(result, AuthResult::Success);
    }

    #[test]
    fn remaining_lockout_ms() {
        let reg = make_registry();
        let mut s = LoginSession::with_lockout(1, 10_000);
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 1000); // locked at 1000

        assert_eq!(s.remaining_lockout_ms(1000), 10_000);
        assert_eq!(s.remaining_lockout_ms(6000), 5_000);
        assert_eq!(s.remaining_lockout_ms(11_000), 0);
    }

    #[test]
    fn remaining_lockout_ms_when_not_locked() {
        let s = LoginSession::new();
        assert_eq!(s.remaining_lockout_ms(0), 0);
        assert_eq!(s.remaining_lockout_ms(99999), 0);
    }

    #[test]
    fn submit_with_pin_provider() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("pin", &reg);
        let fields = vec![CredentialField::new("pin", "1234")];
        let result = s.submit(&fields, &reg, 0);
        assert_eq!(result, AuthResult::Success);
    }

    #[test]
    fn submit_no_provider_selected() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        // Don't select a provider
        let fields = vec![CredentialField::new("password", "secret")];
        let result = s.submit(&fields, &reg, 0);
        assert!(matches!(result, AuthResult::Failed(msg) if msg.contains("No provider")));
    }

    #[test]
    fn reset_clears_everything() {
        let reg = make_registry();
        let mut s = LoginSession::with_lockout(2, 5000);
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 100);
        s.submit(&bad, &reg, 200);

        s.reset();
        assert_eq!(*s.state(), SessionState::SelectingUser);
        assert!(s.selected_user().is_none());
        assert!(s.selected_provider_id().is_none());
        assert_eq!(s.attempt_count(), 0);
        assert!(!s.is_locked_out(200));
    }

    #[test]
    fn go_back_from_entering_credentials() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("password", &reg);
        assert_eq!(*s.state(), SessionState::EnteringCredentials);

        s.go_back();
        assert_eq!(*s.state(), SessionState::SelectingProvider);
        assert!(s.selected_provider_id().is_none());
    }

    #[test]
    fn go_back_from_selecting_provider() {
        let mut s = LoginSession::new();
        s.select_user("alice");
        assert_eq!(*s.state(), SessionState::SelectingProvider);

        s.go_back();
        assert_eq!(*s.state(), SessionState::SelectingUser);
        assert!(s.selected_user().is_none());
    }

    #[test]
    fn go_back_from_failed() {
        let reg = make_registry();
        let mut s = LoginSession::new();
        s.select_user("alice");
        s.select_provider("password", &reg);
        let bad = vec![CredentialField::new("password", "wrong")];
        s.submit(&bad, &reg, 0);
        assert!(matches!(s.state(), SessionState::Failed(_)));

        s.go_back();
        assert_eq!(*s.state(), SessionState::SelectingProvider);
    }

    #[test]
    fn go_back_from_selecting_user_is_noop() {
        let mut s = LoginSession::new();
        s.go_back();
        assert_eq!(*s.state(), SessionState::SelectingUser);
    }

    #[test]
    fn custom_lockout_params() {
        let s = LoginSession::with_lockout(10, 60_000);
        assert_eq!(s.attempt_count(), 0);
        // Verify the custom params are used (we need to fail 10 times)
        assert!(!s.is_locked_out(0));
    }
}
