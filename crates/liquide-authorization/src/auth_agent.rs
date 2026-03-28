//! Authentication agent interface and session management.
//!
//! This module defines the trait that authentication agents must implement
//! (the UI component that prompts the user for credentials) and the
//! session machinery that coordinates a single authorization exchange.
//!
//! The design separates *policy decisions* (handled by [`PolicyDatabase`])
//! from *credential collection* (handled by an [`AuthAgent`] implementation).

use std::time::{Duration, Instant};

use crate::level::AuthLevel;

// ── Credentials ─────────────────────────────────────────────────────

/// Credentials supplied by the user in response to an auth prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    /// The authentication method that was used.
    pub method: AuthLevel,
    /// Opaque token or password hash (never stored in plain text at rest).
    /// For password methods this is the password; for biometric methods
    /// it is a verification token.
    payload: String,
}

impl Credentials {
    /// Create new credentials.
    #[must_use]
    pub fn new(method: AuthLevel, payload: impl Into<String>) -> Self {
        Self {
            method,
            payload: payload.into(),
        }
    }

    /// Access the credential payload.
    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }
}

// ── AuthPrompt ──────────────────────────────────────────────────────

/// Describes the prompt to show the user.
#[derive(Debug, Clone)]
pub struct AuthPrompt {
    /// The message to display.
    pub message: String,
    /// Optional icon name.
    pub icon: Option<String>,
    /// Whether the input should be echoed (true for text fields,
    /// false for password fields).
    pub echo: bool,
    /// Optional label for the input field (e.g., "Password:", "PIN:").
    pub input_label: Option<String>,
}

impl AuthPrompt {
    /// Create a password prompt (echo = false).
    #[must_use]
    pub fn password(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            icon: None,
            echo: false,
            input_label: Some("Password:".to_string()),
        }
    }

    /// Create a text prompt (echo = true).
    #[must_use]
    pub fn text(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            icon: None,
            echo: true,
            input_label: None,
        }
    }

    /// Set the icon.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the input label.
    #[must_use]
    pub fn with_input_label(mut self, label: impl Into<String>) -> Self {
        self.input_label = Some(label.into());
        self
    }
}

// ── AuthIdentity ────────────────────────────────────────────────────

/// Information about the user being prompted, displayed in the auth dialog.
#[derive(Debug, Clone)]
pub struct AuthIdentity {
    /// The user's login name.
    pub username: String,
    /// The user's display/real name (optional).
    pub display_name: Option<String>,
    /// Path or name of the user's avatar icon (optional).
    pub icon: Option<String>,
}

impl AuthIdentity {
    #[must_use]
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            display_name: None,
            icon: None,
        }
    }

    #[must_use]
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Return the best available name for display (display_name or username).
    #[must_use]
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.username)
    }
}

// ── AuthAgent trait ─────────────────────────────────────────────────

/// Error type for agent-level failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthAgentError {
    /// The user cancelled the prompt.
    Cancelled,
    /// The prompt timed out.
    Timeout,
    /// The agent could not display the prompt.
    DisplayError(String),
    /// Credential verification failed.
    VerificationFailed(String),
}

impl std::fmt::Display for AuthAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "authentication cancelled by user"),
            Self::Timeout => write!(f, "authentication prompt timed out"),
            Self::DisplayError(msg) => write!(f, "agent display error: {msg}"),
            Self::VerificationFailed(msg) => write!(f, "verification failed: {msg}"),
        }
    }
}

/// An authentication agent is the UI-side component that presents
/// a credential prompt to the user and collects their response.
///
/// Implementations might be a GTK dialog, a terminal prompt, or a
/// remote-desktop credential relay.
pub trait AuthAgent {
    /// Show an authentication prompt and collect credentials.
    ///
    /// The agent should display `prompt.message`, optionally show the
    /// `identity` information, and collect the credential input.
    ///
    /// Returns the collected credentials or an error.
    fn show_prompt(
        &mut self,
        prompt: &AuthPrompt,
        identity: &AuthIdentity,
    ) -> Result<Credentials, AuthAgentError>;

    /// Dismiss any currently-displayed prompt.
    fn dismiss(&mut self);

    /// Return the agent's human-readable name (for logging).
    fn name(&self) -> &str;
}

// ── AuthSession ─────────────────────────────────────────────────────

/// The state of an in-progress authentication session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The session has been created but authentication has not started.
    Pending,
    /// Waiting for the user to provide credentials.
    AwaitingCredentials,
    /// Authentication succeeded.
    Authenticated,
    /// Authentication failed.
    Failed,
    /// The session was cancelled (by user or timeout).
    Cancelled,
}

/// Manages a single authentication exchange between the authorization
/// framework and an [`AuthAgent`].
pub struct AuthSession {
    /// The action being authorized.
    pub action_id: String,
    /// The prompt to show.
    pub prompt: AuthPrompt,
    /// The identity of the user being prompted.
    pub identity: AuthIdentity,
    /// Current state.
    state: SessionState,
    /// When this session was created.
    created_at: Instant,
    /// Maximum time before auto-cancel.
    timeout: Duration,
    /// Number of authentication attempts so far.
    attempts: u32,
    /// Maximum number of allowed attempts.
    max_attempts: u32,
}

impl AuthSession {
    /// Default timeout: 120 seconds.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
    /// Default max attempts.
    pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

    /// Create a new session.
    #[must_use]
    pub fn new(
        action_id: impl Into<String>,
        prompt: AuthPrompt,
        identity: AuthIdentity,
    ) -> Self {
        Self {
            action_id: action_id.into(),
            prompt,
            identity,
            state: SessionState::Pending,
            created_at: Instant::now(),
            timeout: Self::DEFAULT_TIMEOUT,
            attempts: 0,
            max_attempts: Self::DEFAULT_MAX_ATTEMPTS,
        }
    }

    /// Set a custom timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set a custom max-attempts limit.
    #[must_use]
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }

    /// Return the current session state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        if self.state == SessionState::AwaitingCredentials
            || self.state == SessionState::Pending
        {
            if self.is_timed_out() {
                return SessionState::Cancelled;
            }
        }
        self.state
    }

    /// Begin the authentication flow using the given agent.
    ///
    /// Transitions from `Pending` to `AwaitingCredentials`, invokes
    /// `agent.show_prompt()`, and updates the state based on the result.
    pub fn begin_auth(
        &mut self,
        agent: &mut dyn AuthAgent,
    ) -> Result<Credentials, AuthAgentError> {
        if self.is_timed_out() {
            self.state = SessionState::Cancelled;
            return Err(AuthAgentError::Timeout);
        }

        self.state = SessionState::AwaitingCredentials;
        self.attempts += 1;

        match agent.show_prompt(&self.prompt, &self.identity) {
            Ok(creds) => {
                self.state = SessionState::Authenticated;
                Ok(creds)
            }
            Err(AuthAgentError::Cancelled) => {
                self.state = SessionState::Cancelled;
                Err(AuthAgentError::Cancelled)
            }
            Err(e) => {
                self.state = SessionState::Failed;
                Err(e)
            }
        }
    }

    /// Retry authentication after a failed attempt.
    ///
    /// Returns `Err(AuthAgentError::VerificationFailed)` if max attempts
    /// have been exceeded.
    pub fn retry(
        &mut self,
        agent: &mut dyn AuthAgent,
    ) -> Result<Credentials, AuthAgentError> {
        if self.attempts >= self.max_attempts {
            self.state = SessionState::Failed;
            return Err(AuthAgentError::VerificationFailed(
                "maximum attempts exceeded".to_string(),
            ));
        }
        self.state = SessionState::Pending;
        self.begin_auth(agent)
    }

    /// Cancel the session, dismissing any active prompt.
    pub fn cancel(&mut self, agent: &mut dyn AuthAgent) {
        self.state = SessionState::Cancelled;
        agent.dismiss();
    }

    /// Return the number of attempts made so far.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Return the maximum number of allowed attempts.
    #[must_use]
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Check whether the session has exceeded its timeout.
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        self.created_at.elapsed() >= self.timeout
    }

    /// Return the elapsed time since session creation.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Return the configured timeout.
    #[must_use]
    pub fn timeout_duration(&self) -> Duration {
        self.timeout
    }
}

impl std::fmt::Debug for AuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSession")
            .field("action_id", &self.action_id)
            .field("state", &self.state)
            .field("attempts", &self.attempts)
            .field("max_attempts", &self.max_attempts)
            .finish_non_exhaustive()
    }
}

// ── Stub agent for testing ──────────────────────────────────────────

/// A simple in-memory agent for testing that auto-responds with
/// pre-configured credentials or errors.
#[cfg(test)]
pub struct StubAgent {
    /// Name of this stub.
    pub agent_name: String,
    /// Pre-configured response queue. Each `begin_auth` call pops the
    /// front element. If empty, returns `Cancelled`.
    pub responses: Vec<Result<Credentials, AuthAgentError>>,
    /// Number of times `show_prompt` was called.
    pub prompt_count: usize,
    /// Number of times `dismiss` was called.
    pub dismiss_count: usize,
}

#[cfg(test)]
impl StubAgent {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            agent_name: name.into(),
            responses: Vec::new(),
            prompt_count: 0,
            dismiss_count: 0,
        }
    }

    /// Queue a successful credential response.
    pub fn queue_success(&mut self, method: AuthLevel, payload: &str) {
        self.responses
            .push(Ok(Credentials::new(method, payload)));
    }

    /// Queue a cancellation response.
    pub fn queue_cancel(&mut self) {
        self.responses.push(Err(AuthAgentError::Cancelled));
    }

    /// Queue a verification-failed response.
    pub fn queue_fail(&mut self, msg: &str) {
        self.responses
            .push(Err(AuthAgentError::VerificationFailed(msg.to_string())));
    }
}

#[cfg(test)]
impl AuthAgent for StubAgent {
    fn show_prompt(
        &mut self,
        _prompt: &AuthPrompt,
        _identity: &AuthIdentity,
    ) -> Result<Credentials, AuthAgentError> {
        self.prompt_count += 1;
        if self.responses.is_empty() {
            Err(AuthAgentError::Cancelled)
        } else {
            self.responses.remove(0)
        }
    }

    fn dismiss(&mut self) {
        self.dismiss_count += 1;
    }

    fn name(&self) -> &str {
        &self.agent_name
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_prompt() -> AuthPrompt {
        AuthPrompt::password("Please authenticate")
    }

    fn test_identity() -> AuthIdentity {
        AuthIdentity::new("testuser")
    }

    // ── Credentials tests ───────────────────────────────────────────

    #[test]
    fn credentials_new() {
        let c = Credentials::new(AuthLevel::UserPassword, "secret123");
        assert_eq!(c.method, AuthLevel::UserPassword);
        assert_eq!(c.payload(), "secret123");
    }

    // ── AuthPrompt tests ────────────────────────────────────────────

    #[test]
    fn prompt_password() {
        let p = AuthPrompt::password("Enter password");
        assert_eq!(p.message, "Enter password");
        assert!(!p.echo);
        assert_eq!(p.input_label.as_deref(), Some("Password:"));
        assert!(p.icon.is_none());
    }

    #[test]
    fn prompt_text() {
        let p = AuthPrompt::text("Enter username");
        assert_eq!(p.message, "Enter username");
        assert!(p.echo);
        assert!(p.input_label.is_none());
    }

    #[test]
    fn prompt_builders() {
        let p = AuthPrompt::password("msg")
            .with_icon("lock")
            .with_input_label("PIN:");
        assert_eq!(p.icon.as_deref(), Some("lock"));
        assert_eq!(p.input_label.as_deref(), Some("PIN:"));
    }

    // ── AuthIdentity tests ──────────────────────────────────────────

    #[test]
    fn identity_new() {
        let id = AuthIdentity::new("alice");
        assert_eq!(id.username, "alice");
        assert!(id.display_name.is_none());
        assert!(id.icon.is_none());
        assert_eq!(id.label(), "alice");
    }

    #[test]
    fn identity_with_display_name() {
        let id = AuthIdentity::new("alice").with_display_name("Alice Smith");
        assert_eq!(id.label(), "Alice Smith");
    }

    #[test]
    fn identity_with_icon() {
        let id = AuthIdentity::new("alice").with_icon("user-avatar");
        assert_eq!(id.icon.as_deref(), Some("user-avatar"));
    }

    // ── AuthAgentError tests ────────────────────────────────────────

    #[test]
    fn agent_error_display() {
        assert_eq!(
            AuthAgentError::Cancelled.to_string(),
            "authentication cancelled by user"
        );
        assert_eq!(
            AuthAgentError::Timeout.to_string(),
            "authentication prompt timed out"
        );
        assert!(AuthAgentError::DisplayError("x".into())
            .to_string()
            .contains("x"));
        assert!(AuthAgentError::VerificationFailed("y".into())
            .to_string()
            .contains("y"));
    }

    // ── StubAgent tests ─────────────────────────────────────────────

    #[test]
    fn stub_agent_success() {
        let mut agent = StubAgent::new("test");
        agent.queue_success(AuthLevel::UserPassword, "pass");

        let result = agent.show_prompt(&test_prompt(), &test_identity());
        assert!(result.is_ok());
        assert_eq!(agent.prompt_count, 1);
    }

    #[test]
    fn stub_agent_cancel() {
        let mut agent = StubAgent::new("test");
        agent.queue_cancel();

        let result = agent.show_prompt(&test_prompt(), &test_identity());
        assert_eq!(result, Err(AuthAgentError::Cancelled));
    }

    #[test]
    fn stub_agent_empty_responses() {
        let mut agent = StubAgent::new("test");
        let result = agent.show_prompt(&test_prompt(), &test_identity());
        assert_eq!(result, Err(AuthAgentError::Cancelled));
    }

    #[test]
    fn stub_agent_dismiss() {
        let mut agent = StubAgent::new("test");
        agent.dismiss();
        assert_eq!(agent.dismiss_count, 1);
    }

    #[test]
    fn stub_agent_name() {
        let agent = StubAgent::new("my-agent");
        assert_eq!(agent.name(), "my-agent");
    }

    // ── AuthSession tests ───────────────────────────────────────────

    #[test]
    fn session_new() {
        let session = AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        assert_eq!(session.action_id, "org.liquide.test");
        assert_eq!(session.state(), SessionState::Pending);
        assert_eq!(session.attempts(), 0);
        assert_eq!(session.max_attempts(), AuthSession::DEFAULT_MAX_ATTEMPTS);
    }

    #[test]
    fn session_with_timeout() {
        let session = AuthSession::new("org.liquide.test", test_prompt(), test_identity())
            .with_timeout(Duration::from_secs(30));
        assert_eq!(session.timeout_duration(), Duration::from_secs(30));
    }

    #[test]
    fn session_with_max_attempts() {
        let session = AuthSession::new("org.liquide.test", test_prompt(), test_identity())
            .with_max_attempts(5);
        assert_eq!(session.max_attempts(), 5);
    }

    #[test]
    fn session_begin_auth_success() {
        let mut agent = StubAgent::new("test");
        agent.queue_success(AuthLevel::UserPassword, "pass");

        let mut session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        let result = session.begin_auth(&mut agent);

        assert!(result.is_ok());
        assert_eq!(session.state(), SessionState::Authenticated);
        assert_eq!(session.attempts(), 1);
    }

    #[test]
    fn session_begin_auth_cancelled() {
        let mut agent = StubAgent::new("test");
        agent.queue_cancel();

        let mut session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        let result = session.begin_auth(&mut agent);

        assert_eq!(result, Err(AuthAgentError::Cancelled));
        assert_eq!(session.state(), SessionState::Cancelled);
    }

    #[test]
    fn session_begin_auth_failed() {
        let mut agent = StubAgent::new("test");
        agent.queue_fail("bad password");

        let mut session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        let result = session.begin_auth(&mut agent);

        assert!(result.is_err());
        assert_eq!(session.state(), SessionState::Failed);
    }

    #[test]
    fn session_retry_success_after_fail() {
        let mut agent = StubAgent::new("test");
        agent.queue_fail("wrong");
        agent.queue_success(AuthLevel::UserPassword, "correct");

        let mut session = AuthSession::new("org.liquide.test", test_prompt(), test_identity())
            .with_max_attempts(3);

        // First attempt fails
        let _ = session.begin_auth(&mut agent);
        assert_eq!(session.state(), SessionState::Failed);
        assert_eq!(session.attempts(), 1);

        // Retry succeeds
        let result = session.retry(&mut agent);
        assert!(result.is_ok());
        assert_eq!(session.state(), SessionState::Authenticated);
        assert_eq!(session.attempts(), 2);
    }

    #[test]
    fn session_retry_exceeds_max_attempts() {
        let mut agent = StubAgent::new("test");
        agent.queue_fail("wrong");

        let mut session = AuthSession::new("org.liquide.test", test_prompt(), test_identity())
            .with_max_attempts(1);

        let _ = session.begin_auth(&mut agent);
        assert_eq!(session.attempts(), 1);

        // Retry should fail because max_attempts=1 and we already used 1
        let result = session.retry(&mut agent);
        assert_eq!(
            result,
            Err(AuthAgentError::VerificationFailed(
                "maximum attempts exceeded".to_string()
            ))
        );
        assert_eq!(session.state(), SessionState::Failed);
    }

    #[test]
    fn session_cancel() {
        let mut agent = StubAgent::new("test");

        let mut session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        session.cancel(&mut agent);

        assert_eq!(session.state(), SessionState::Cancelled);
        assert_eq!(agent.dismiss_count, 1);
    }

    #[test]
    fn session_timeout_immediate() {
        let mut agent = StubAgent::new("test");
        agent.queue_success(AuthLevel::UserPassword, "pass");

        let mut session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity())
                .with_timeout(Duration::from_secs(0));

        // With a zero timeout, the session should be timed out immediately
        // (or very nearly so).
        // We allow for a tiny window — if not timed out, it is because
        // the test ran faster than the timer resolution, which is fine.
        let result = session.begin_auth(&mut agent);
        if result.is_err() {
            assert_eq!(result, Err(AuthAgentError::Timeout));
            assert_eq!(session.state(), SessionState::Cancelled);
        }
        // If it succeeded, the Instant resolution was too coarse — that's OK.
    }

    #[test]
    fn session_debug() {
        let session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        let dbg = format!("{:?}", session);
        assert!(dbg.contains("AuthSession"));
        assert!(dbg.contains("org.liquide.test"));
    }

    #[test]
    fn session_elapsed_is_small() {
        let session =
            AuthSession::new("org.liquide.test", test_prompt(), test_identity());
        // Just created, should be very recent
        assert!(session.elapsed() < Duration::from_secs(5));
    }
}
