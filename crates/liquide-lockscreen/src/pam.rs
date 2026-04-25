/// PAM authentication backend (abstract).
///
/// Provides trait abstractions for Pluggable Authentication Modules
/// without linking to libpam.  Real implementations would FFI into
/// `pam_authenticate(3)` and `pam_acct_mgmt(3)`.

// ---------------------------------------------------------------------------
// PamResult
// ---------------------------------------------------------------------------

/// Result of a PAM authentication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PamResult {
    /// Authentication succeeded.
    Success,
    /// Authentication failed with a message (PAM_AUTH_ERR).
    AuthError(String),
    /// Account has expired (PAM_ACCT_EXPIRED).
    AccountExpired,
    /// Password has expired and must be changed (PAM_NEW_AUTHTOK_REQD).
    PasswordExpired,
    /// Maximum retries exhausted (PAM_MAXTRIES).
    MaxRetries,
}

// ---------------------------------------------------------------------------
// PamConversation
// ---------------------------------------------------------------------------

/// Callback interface for PAM interactive prompts.
///
/// In a real PAM integration, the conversation function is called by
/// `pam_authenticate()` to obtain credentials interactively.
pub trait PamConversation: Send {
    /// Prompt the user for a text response (PAM_PROMPT_ECHO_ON).
    fn prompt_echo_on(&mut self, msg: &str) -> Option<String>;

    /// Prompt the user for a secret response (PAM_PROMPT_ECHO_OFF).
    fn prompt_echo_off(&mut self, msg: &str) -> Option<String>;

    /// Display an informational message (PAM_TEXT_INFO).
    fn info(&mut self, msg: &str);

    /// Display an error message (PAM_ERROR_MSG).
    fn error(&mut self, msg: &str);
}

// ---------------------------------------------------------------------------
// PamBackend trait
// ---------------------------------------------------------------------------

/// Abstract PAM authentication backend.
///
/// Implementations may call real libpam functions or provide mock
/// behaviour for testing.
pub trait PamBackend: Send {
    /// Authenticate the given username with the given password.
    ///
    /// This corresponds to `pam_start()` + `pam_authenticate()` +
    /// `pam_acct_mgmt()` + `pam_end()`.
    fn authenticate(&self, username: &str, password: &str) -> PamResult;

    /// Authenticate using an interactive conversation.
    ///
    /// The default implementation falls back to `authenticate()` by
    /// using `conv.prompt_echo_off()` to obtain the password.
    fn authenticate_conv(&self, username: &str, conv: &mut dyn PamConversation) -> PamResult {
        match conv.prompt_echo_off("Password: ") {
            Some(password) => self.authenticate(username, &password),
            None => PamResult::AuthError("No password provided.".into()),
        }
    }

    /// The PAM service name (e.g. `"login"`, `"gdm-password"`).
    fn service_name(&self) -> &str {
        "login"
    }
}

// ---------------------------------------------------------------------------
// MockPam
// ---------------------------------------------------------------------------

/// A mock PAM backend with configurable responses.
///
/// Useful for testing the greeter and session logic without requiring
/// real PAM infrastructure.
pub struct MockPam {
    /// Fixed result returned for every authentication attempt.
    result: PamResult,
    /// If set, only this username will match.
    expected_username: Option<String>,
    /// If set, only this password will match.
    expected_password: Option<String>,
    /// Custom service name.
    service: String,
}

impl MockPam {
    /// Create a mock that always succeeds.
    pub fn succeeding() -> Self {
        Self {
            result: PamResult::Success,
            expected_username: None,
            expected_password: None,
            service: "mock-login".into(),
        }
    }

    /// Create a mock that always fails.
    pub fn failing(msg: &str) -> Self {
        Self {
            result: PamResult::AuthError(msg.to_string()),
            expected_username: None,
            expected_password: None,
            service: "mock-login".into(),
        }
    }

    /// Create a mock that returns a specific result.
    pub fn with_result(result: PamResult) -> Self {
        Self {
            result,
            expected_username: None,
            expected_password: None,
            service: "mock-login".into(),
        }
    }

    /// Create a mock that succeeds only for a specific username/password.
    pub fn with_credentials(username: &str, password: &str) -> Self {
        Self {
            result: PamResult::Success,
            expected_username: Some(username.to_string()),
            expected_password: Some(password.to_string()),
            service: "mock-login".into(),
        }
    }

    /// Builder: set custom service name.
    pub fn with_service(mut self, service: &str) -> Self {
        self.service = service.to_string();
        self
    }
}

impl PamBackend for MockPam {
    fn authenticate(&self, username: &str, password: &str) -> PamResult {
        if let Some(ref expected_user) = self.expected_username {
            if username != expected_user {
                return PamResult::AuthError("User not found.".into());
            }
        }
        if let Some(ref expected_pass) = self.expected_password {
            if password != expected_pass {
                return PamResult::AuthError("Incorrect password.".into());
            }
        }
        self.result.clone()
    }

    fn service_name(&self) -> &str {
        &self.service
    }
}

// ---------------------------------------------------------------------------
// MockConversation (for testing)
// ---------------------------------------------------------------------------

/// A mock PAM conversation that returns pre-configured responses.
pub struct MockConversation {
    /// Responses to echo-off prompts (consumed in order).
    echo_off_responses: Vec<Option<String>>,
    /// Responses to echo-on prompts (consumed in order).
    echo_on_responses: Vec<Option<String>>,
    /// Info messages received.
    pub info_messages: Vec<String>,
    /// Error messages received.
    pub error_messages: Vec<String>,
    index_off: usize,
    index_on: usize,
}

impl MockConversation {
    /// Create a conversation that provides a single password.
    pub fn with_password(password: &str) -> Self {
        Self {
            echo_off_responses: vec![Some(password.to_string())],
            echo_on_responses: Vec::new(),
            info_messages: Vec::new(),
            error_messages: Vec::new(),
            index_off: 0,
            index_on: 0,
        }
    }

    /// Create a conversation with no responses (all prompts return None).
    pub fn empty() -> Self {
        Self {
            echo_off_responses: Vec::new(),
            echo_on_responses: Vec::new(),
            info_messages: Vec::new(),
            error_messages: Vec::new(),
            index_off: 0,
            index_on: 0,
        }
    }
}

impl PamConversation for MockConversation {
    fn prompt_echo_on(&mut self, _msg: &str) -> Option<String> {
        let resp = self.echo_on_responses.get(self.index_on).cloned().flatten();
        self.index_on += 1;
        resp
    }

    fn prompt_echo_off(&mut self, _msg: &str) -> Option<String> {
        let resp = self
            .echo_off_responses
            .get(self.index_off)
            .cloned()
            .flatten();
        self.index_off += 1;
        resp
    }

    fn info(&mut self, msg: &str) {
        self.info_messages.push(msg.to_string());
    }

    fn error(&mut self, msg: &str) {
        self.error_messages.push(msg.to_string());
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PamResult tests --

    #[test]
    fn pam_result_variants() {
        assert_eq!(PamResult::Success, PamResult::Success);
        assert_ne!(PamResult::Success, PamResult::AccountExpired);
        assert_ne!(PamResult::PasswordExpired, PamResult::MaxRetries);
        let e1 = PamResult::AuthError("a".into());
        let e2 = PamResult::AuthError("a".into());
        assert_eq!(e1, e2);
    }

    // -- MockPam tests --

    #[test]
    fn mock_pam_succeeding() {
        let pam = MockPam::succeeding();
        assert_eq!(pam.authenticate("alice", "any"), PamResult::Success);
        assert_eq!(pam.service_name(), "mock-login");
    }

    #[test]
    fn mock_pam_failing() {
        let pam = MockPam::failing("bad password");
        let result = pam.authenticate("alice", "pass");
        assert_eq!(result, PamResult::AuthError("bad password".into()));
    }

    #[test]
    fn mock_pam_with_result() {
        let pam = MockPam::with_result(PamResult::AccountExpired);
        assert_eq!(pam.authenticate("alice", "x"), PamResult::AccountExpired);
    }

    #[test]
    fn mock_pam_password_expired() {
        let pam = MockPam::with_result(PamResult::PasswordExpired);
        assert_eq!(pam.authenticate("alice", "x"), PamResult::PasswordExpired);
    }

    #[test]
    fn mock_pam_max_retries() {
        let pam = MockPam::with_result(PamResult::MaxRetries);
        assert_eq!(pam.authenticate("alice", "x"), PamResult::MaxRetries);
    }

    #[test]
    fn mock_pam_with_credentials_success() {
        let pam = MockPam::with_credentials("alice", "secret");
        assert_eq!(pam.authenticate("alice", "secret"), PamResult::Success);
    }

    #[test]
    fn mock_pam_with_credentials_wrong_user() {
        let pam = MockPam::with_credentials("alice", "secret");
        let result = pam.authenticate("bob", "secret");
        assert!(matches!(result, PamResult::AuthError(_)));
    }

    #[test]
    fn mock_pam_with_credentials_wrong_password() {
        let pam = MockPam::with_credentials("alice", "secret");
        let result = pam.authenticate("alice", "wrong");
        assert!(matches!(result, PamResult::AuthError(_)));
    }

    #[test]
    fn mock_pam_custom_service() {
        let pam = MockPam::succeeding().with_service("gdm-password");
        assert_eq!(pam.service_name(), "gdm-password");
    }

    // -- MockConversation tests --

    #[test]
    fn mock_conv_with_password() {
        let mut conv = MockConversation::with_password("s3cret");
        assert_eq!(conv.prompt_echo_off("Password: "), Some("s3cret".into()));
        // Second call returns None (exhausted)
        assert_eq!(conv.prompt_echo_off("Password: "), None);
    }

    #[test]
    fn mock_conv_empty() {
        let mut conv = MockConversation::empty();
        assert_eq!(conv.prompt_echo_off("Password: "), None);
        assert_eq!(conv.prompt_echo_on("Username: "), None);
    }

    #[test]
    fn mock_conv_info_and_error() {
        let mut conv = MockConversation::empty();
        conv.info("Welcome");
        conv.error("Something went wrong");
        assert_eq!(conv.info_messages, vec!["Welcome"]);
        assert_eq!(conv.error_messages, vec!["Something went wrong"]);
    }

    // -- authenticate_conv tests --

    #[test]
    fn authenticate_conv_success() {
        let pam = MockPam::with_credentials("alice", "secret");
        let mut conv = MockConversation::with_password("secret");
        let result = pam.authenticate_conv("alice", &mut conv);
        assert_eq!(result, PamResult::Success);
    }

    #[test]
    fn authenticate_conv_no_password() {
        let pam = MockPam::succeeding();
        let mut conv = MockConversation::empty();
        let result = pam.authenticate_conv("alice", &mut conv);
        assert!(matches!(result, PamResult::AuthError(msg) if msg.contains("No password")));
    }

    #[test]
    fn authenticate_conv_wrong_password() {
        let pam = MockPam::with_credentials("alice", "secret");
        let mut conv = MockConversation::with_password("wrong");
        let result = pam.authenticate_conv("alice", &mut conv);
        assert!(matches!(result, PamResult::AuthError(_)));
    }
}
