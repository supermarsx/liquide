#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

/// Credentials submitted by the user
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Authentication result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// Authentication succeeded.
    Success,
    /// Authentication failed with an error message.
    Failed(String),
    /// Account is temporarily locked; retry after the given number of milliseconds.
    Locked(u64),
    /// Multi-factor authentication is required.
    RequiresMfa,
}

/// Auth backend trait — platform-agnostic authentication interface.
pub trait AuthBackend: Send {
    /// Authenticate with username and credential string.
    fn authenticate(&self, username: &str, credential: &str) -> AuthResult;
}

/// Backwards-compatible wrapper that also accepts `Credentials`.
impl dyn AuthBackend {
    /// Authenticate using a `Credentials` struct.
    pub fn authenticate_creds(&self, creds: &Credentials) -> AuthResult {
        self.authenticate(&creds.username, &creds.password)
    }
}

// ---------------------------------------------------------------------------
// MockAuth — configurable success/fail for testing
// ---------------------------------------------------------------------------

/// A mock auth backend that returns a fixed result.
pub struct MockAuth {
    result: AuthResult,
}

impl MockAuth {
    /// Create a mock that always succeeds.
    pub fn succeeding() -> Self {
        Self {
            result: AuthResult::Success,
        }
    }

    /// Create a mock that always fails with the given message.
    pub fn failing(msg: &str) -> Self {
        Self {
            result: AuthResult::Failed(msg.to_string()),
        }
    }

    /// Create a mock that returns a specific result.
    pub fn with_result(result: AuthResult) -> Self {
        Self { result }
    }
}

impl AuthBackend for MockAuth {
    fn authenticate(&self, _username: &str, _credential: &str) -> AuthResult {
        self.result.clone()
    }
}

// ---------------------------------------------------------------------------
// PasswordAuth — simple hash-based auth for testing
// ---------------------------------------------------------------------------

/// Simple hash-based password authentication (for testing only).
///
/// Stores username/password pairs with a basic hash. NOT cryptographically
/// secure — real deployments should use platform PAM or equivalent.
pub struct PasswordAuth {
    entries: Vec<(String, u64)>,
}

impl PasswordAuth {
    /// Create a new empty password auth store.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a user with a password.
    pub fn add_user(&mut self, username: &str, password: &str) {
        let hash = Self::hash_password(password);
        self.entries.push((username.to_string(), hash));
    }

    /// Simple non-cryptographic hash (FNV-1a style). For testing only.
    fn hash_password(password: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in password.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl AuthBackend for PasswordAuth {
    fn authenticate(&self, username: &str, credential: &str) -> AuthResult {
        let hash = Self::hash_password(credential);
        for (user, stored_hash) in &self.entries {
            if user == username {
                if *stored_hash == hash {
                    return AuthResult::Success;
                } else {
                    return AuthResult::Failed("Incorrect password.".into());
                }
            }
        }
        AuthResult::Failed("User not found.".into())
    }
}

// ---------------------------------------------------------------------------
// BiometricAuth — placeholder for fingerprint/face
// ---------------------------------------------------------------------------

/// Placeholder biometric authentication backend.
///
/// Always returns `Failed` — real implementation would interface with
/// platform biometric APIs (fprintd on Linux, Windows Hello, etc.).
pub struct BiometricAuth;

impl BiometricAuth {
    pub fn new() -> Self {
        Self
    }
}

impl AuthBackend for BiometricAuth {
    fn authenticate(&self, _username: &str, _credential: &str) -> AuthResult {
        AuthResult::Failed("Biometric authentication not available.".into())
    }
}

// ---------------------------------------------------------------------------
// AuthChain — tries multiple backends in order
// ---------------------------------------------------------------------------

/// Tries multiple auth backends in sequence until one succeeds.
///
/// If a backend returns `Success`, the chain stops. If all backends fail,
/// the last failure result is returned.
pub struct AuthChain {
    backends: Vec<Box<dyn AuthBackend>>,
}

impl AuthChain {
    /// Create an empty auth chain.
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Add a backend to the chain.
    pub fn add(&mut self, backend: Box<dyn AuthBackend>) {
        self.backends.push(backend);
    }

    /// Number of backends in the chain.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Whether the chain has no backends.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

impl AuthBackend for AuthChain {
    fn authenticate(&self, username: &str, credential: &str) -> AuthResult {
        let mut last_result = AuthResult::Failed("No auth backends configured.".into());
        for backend in &self.backends {
            let result = backend.authenticate(username, credential);
            if result == AuthResult::Success {
                return AuthResult::Success;
            }
            last_result = result;
        }
        last_result
    }
}

// ---------------------------------------------------------------------------
// AttemptsTracker — lockout after too many failures
// ---------------------------------------------------------------------------

/// Tracks authentication attempts and enforces lockout after too many failures.
pub struct AttemptsTracker {
    /// Maximum allowed attempts before lockout.
    max_attempts: u32,
    /// Lockout duration in milliseconds.
    lockout_ms: u64,
    /// Number of consecutive failed attempts.
    failed_count: u32,
    /// Timestamp (ms) when the lockout started, if any.
    lockout_start_ms: Option<u64>,
}

impl AttemptsTracker {
    /// Create a new tracker with the given limits.
    pub fn new(max_attempts: u32, lockout_ms: u64) -> Self {
        Self {
            max_attempts,
            lockout_ms,
            failed_count: 0,
            lockout_start_ms: None,
        }
    }

    /// Record an authentication attempt.
    /// Returns `true` if the account is now locked out.
    pub fn record_attempt(&mut self, success: bool, now_ms: u64) -> bool {
        // Check if lockout has expired
        if let Some(start) = self.lockout_start_ms {
            if now_ms >= start + self.lockout_ms {
                self.lockout_start_ms = None;
                self.failed_count = 0;
            }
        }

        if success {
            self.failed_count = 0;
            self.lockout_start_ms = None;
            false
        } else {
            self.failed_count += 1;
            if self.failed_count >= self.max_attempts {
                self.lockout_start_ms = Some(now_ms);
                true
            } else {
                false
            }
        }
    }

    /// Whether the account is currently locked out.
    pub fn is_locked_out(&self, now_ms: u64) -> bool {
        if let Some(start) = self.lockout_start_ms {
            now_ms < start + self.lockout_ms
        } else {
            false
        }
    }

    /// Number of remaining attempts before lockout.
    pub fn remaining_attempts(&self) -> u32 {
        self.max_attempts.saturating_sub(self.failed_count)
    }

    /// Reset the tracker (e.g., after successful auth).
    pub fn reset(&mut self) {
        self.failed_count = 0;
        self.lockout_start_ms = None;
    }

    /// Current number of consecutive failures.
    pub fn failed_count(&self) -> u32 {
        self.failed_count
    }
}

// ---------------------------------------------------------------------------
// PlatformAuth — real platform authentication
// ---------------------------------------------------------------------------

/// Platform-specific authentication implementation.
pub struct PlatformAuth;

impl PlatformAuth {
    pub fn new() -> Self {
        Self
    }

    /// Get the current username from environment.
    pub fn current_username() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "user".into())
    }

    /// Get the user's display name.
    pub fn user_display_name() -> String {
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("getent")
                .args(["passwd", &Self::current_username()])
                .output()
            {
                let s = String::from_utf8_lossy(&output.stdout);
                if let Some(gecos) = s.split(':').nth(4) {
                    let name = gecos.split(',').next().unwrap_or("").trim();
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
        Self::current_username()
    }

    /// Get the user's avatar path, if one exists.
    pub fn user_avatar_path() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            let path = format!(
                "/var/lib/AccountsService/icons/{}",
                Self::current_username()
            );
            if std::path::Path::new(&path).exists() {
                return Some(path);
            }
            if let Ok(home) = std::env::var("HOME") {
                let face = format!("{}/.face", home);
                if std::path::Path::new(&face).exists() {
                    return Some(face);
                }
            }
        }
        None
    }
}

impl AuthBackend for PlatformAuth {
    fn authenticate(&self, username: &str, credential: &str) -> AuthResult {
        #[cfg(target_os = "linux")]
        {
            return authenticate_linux(username, credential);
        }
        #[cfg(target_os = "macos")]
        {
            return authenticate_macos(username, credential);
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (username, credential);
            AuthResult::Failed("Platform authentication not implemented.".into())
        }
    }
}

#[cfg(target_os = "linux")]
fn authenticate_linux(username: &str, credential: &str) -> AuthResult {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = match Command::new("su")
        .arg(username)
        .arg("-c")
        .arg("true")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return AuthResult::Failed(format!("failed to spawn su: {}", e)),
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "{}", credential);
    }

    match child.wait() {
        Ok(status) if status.success() => AuthResult::Success,
        Ok(_) => AuthResult::Failed("Incorrect password.".into()),
        Err(e) => AuthResult::Failed(format!("su failed: {}", e)),
    }
}

#[cfg(target_os = "macos")]
fn authenticate_macos(username: &str, credential: &str) -> AuthResult {
    match Command::new("dscl")
        .args(["/Local/Default", "-authonly", username, credential])
        .output()
    {
        Ok(output) if output.status.success() => AuthResult::Success,
        Ok(_) => AuthResult::Failed("Incorrect password.".into()),
        Err(e) => AuthResult::Failed(format!("auth check failed: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// NullAuth — always succeeds (for testing)
// ---------------------------------------------------------------------------

/// Null auth backend (always succeeds) for testing.
pub struct NullAuth {
    pub username: String,
}

impl NullAuth {
    pub fn new() -> Self {
        Self {
            username: "testuser".into(),
        }
    }
}

impl AuthBackend for NullAuth {
    fn authenticate(&self, _username: &str, _credential: &str) -> AuthResult {
        AuthResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- MockAuth tests --

    #[test]
    fn mock_auth_succeeding() {
        let auth = MockAuth::succeeding();
        assert_eq!(auth.authenticate("alice", "pass"), AuthResult::Success);
    }

    #[test]
    fn mock_auth_failing() {
        let auth = MockAuth::failing("bad password");
        assert_eq!(
            auth.authenticate("alice", "pass"),
            AuthResult::Failed("bad password".into())
        );
    }

    #[test]
    fn mock_auth_custom_result() {
        let auth = MockAuth::with_result(AuthResult::RequiresMfa);
        assert_eq!(auth.authenticate("alice", "pass"), AuthResult::RequiresMfa);
    }

    #[test]
    fn mock_auth_locked() {
        let auth = MockAuth::with_result(AuthResult::Locked(5000));
        assert_eq!(auth.authenticate("alice", "pass"), AuthResult::Locked(5000));
    }

    // -- PasswordAuth tests --

    #[test]
    fn password_auth_success() {
        let mut auth = PasswordAuth::new();
        auth.add_user("alice", "secret123");
        assert_eq!(auth.authenticate("alice", "secret123"), AuthResult::Success);
    }

    #[test]
    fn password_auth_wrong_password() {
        let mut auth = PasswordAuth::new();
        auth.add_user("alice", "secret123");
        let result = auth.authenticate("alice", "wrong");
        assert!(matches!(result, AuthResult::Failed(_)));
    }

    #[test]
    fn password_auth_user_not_found() {
        let auth = PasswordAuth::new();
        let result = auth.authenticate("nobody", "pass");
        assert!(matches!(result, AuthResult::Failed(msg) if msg.contains("not found")));
    }

    #[test]
    fn password_auth_multiple_users() {
        let mut auth = PasswordAuth::new();
        auth.add_user("alice", "pass_a");
        auth.add_user("bob", "pass_b");
        assert_eq!(auth.authenticate("alice", "pass_a"), AuthResult::Success);
        assert_eq!(auth.authenticate("bob", "pass_b"), AuthResult::Success);
        assert!(matches!(
            auth.authenticate("alice", "pass_b"),
            AuthResult::Failed(_)
        ));
    }

    #[test]
    fn password_auth_empty_password() {
        let mut auth = PasswordAuth::new();
        auth.add_user("alice", "");
        assert_eq!(auth.authenticate("alice", ""), AuthResult::Success);
        assert!(matches!(
            auth.authenticate("alice", "x"),
            AuthResult::Failed(_)
        ));
    }

    // -- BiometricAuth tests --

    #[test]
    fn biometric_auth_fails() {
        let auth = BiometricAuth::new();
        let result = auth.authenticate("alice", "");
        assert!(matches!(result, AuthResult::Failed(msg) if msg.contains("not available")));
    }

    // -- AuthChain tests --

    #[test]
    fn chain_empty_fails() {
        let chain = AuthChain::new();
        let result = chain.authenticate("alice", "pass");
        assert!(matches!(result, AuthResult::Failed(_)));
    }

    #[test]
    fn chain_first_succeeds() {
        let mut chain = AuthChain::new();
        chain.add(Box::new(MockAuth::succeeding()));
        chain.add(Box::new(MockAuth::failing("should not reach")));
        assert_eq!(chain.authenticate("alice", "pass"), AuthResult::Success);
    }

    #[test]
    fn chain_fallback_to_second() {
        let mut chain = AuthChain::new();
        chain.add(Box::new(MockAuth::failing("first failed")));
        chain.add(Box::new(MockAuth::succeeding()));
        assert_eq!(chain.authenticate("alice", "pass"), AuthResult::Success);
    }

    #[test]
    fn chain_all_fail_returns_last() {
        let mut chain = AuthChain::new();
        chain.add(Box::new(MockAuth::failing("first")));
        chain.add(Box::new(MockAuth::failing("second")));
        let result = chain.authenticate("alice", "pass");
        assert_eq!(result, AuthResult::Failed("second".into()));
    }

    #[test]
    fn chain_len() {
        let mut chain = AuthChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        chain.add(Box::new(MockAuth::succeeding()));
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }

    // -- AttemptsTracker tests --

    #[test]
    fn tracker_initial_state() {
        let tracker = AttemptsTracker::new(3, 30_000);
        assert_eq!(tracker.remaining_attempts(), 3);
        assert!(!tracker.is_locked_out(0));
        assert_eq!(tracker.failed_count(), 0);
    }

    #[test]
    fn tracker_success_resets() {
        let mut tracker = AttemptsTracker::new(3, 30_000);
        tracker.record_attempt(false, 1000);
        assert_eq!(tracker.remaining_attempts(), 2);
        tracker.record_attempt(true, 2000);
        assert_eq!(tracker.remaining_attempts(), 3);
        assert_eq!(tracker.failed_count(), 0);
    }

    #[test]
    fn tracker_lockout_after_max() {
        let mut tracker = AttemptsTracker::new(3, 30_000);
        tracker.record_attempt(false, 1000);
        tracker.record_attempt(false, 2000);
        let locked = tracker.record_attempt(false, 3000);
        assert!(locked);
        assert!(tracker.is_locked_out(3000));
        assert!(tracker.is_locked_out(32_999));
        assert_eq!(tracker.remaining_attempts(), 0);
    }

    #[test]
    fn tracker_lockout_expires() {
        let mut tracker = AttemptsTracker::new(3, 30_000);
        tracker.record_attempt(false, 0);
        tracker.record_attempt(false, 100);
        tracker.record_attempt(false, 200); // locked at 200
        assert!(tracker.is_locked_out(200));
        assert!(tracker.is_locked_out(30_199));
        assert!(!tracker.is_locked_out(30_200)); // expired
    }

    #[test]
    fn tracker_reset_after_lockout_expiry() {
        let mut tracker = AttemptsTracker::new(2, 5_000);
        tracker.record_attempt(false, 0);
        tracker.record_attempt(false, 100); // locked
        assert!(tracker.is_locked_out(100));

        // After lockout expires, a new attempt should reset the counter
        let locked = tracker.record_attempt(false, 10_000); // lockout expired
        assert!(!locked); // only 1 failure now
        assert_eq!(tracker.remaining_attempts(), 1);
    }

    #[test]
    fn tracker_reset_manual() {
        let mut tracker = AttemptsTracker::new(3, 30_000);
        tracker.record_attempt(false, 0);
        tracker.record_attempt(false, 100);
        tracker.reset();
        assert_eq!(tracker.remaining_attempts(), 3);
        assert!(!tracker.is_locked_out(100));
    }

    #[test]
    fn tracker_not_locked_without_failures() {
        let tracker = AttemptsTracker::new(5, 60_000);
        assert!(!tracker.is_locked_out(0));
        assert!(!tracker.is_locked_out(999_999));
    }

    #[test]
    fn tracker_single_attempt_allowed() {
        let mut tracker = AttemptsTracker::new(1, 1_000);
        let locked = tracker.record_attempt(false, 0);
        assert!(locked);
        assert!(tracker.is_locked_out(0));
    }

    // -- NullAuth tests --

    #[test]
    fn null_auth_always_succeeds() {
        let auth = NullAuth::new();
        assert_eq!(auth.authenticate("anyone", "anything"), AuthResult::Success);
    }

    #[test]
    fn null_auth_username() {
        let auth = NullAuth::new();
        assert_eq!(auth.username, "testuser");
    }

    // -- Credentials tests --

    #[test]
    fn credentials_clone() {
        let creds = Credentials {
            username: "alice".into(),
            password: "secret".into(),
        };
        let cloned = creds.clone();
        assert_eq!(cloned.username, "alice");
        assert_eq!(cloned.password, "secret");
    }

    // -- AuthResult tests --

    #[test]
    fn auth_result_variants() {
        assert_eq!(AuthResult::Success, AuthResult::Success);
        assert_ne!(AuthResult::Success, AuthResult::RequiresMfa);
        let f1 = AuthResult::Failed("a".into());
        let f2 = AuthResult::Failed("a".into());
        assert_eq!(f1, f2);
        assert_ne!(
            AuthResult::Failed("a".into()),
            AuthResult::Failed("b".into())
        );
        assert_eq!(AuthResult::Locked(100), AuthResult::Locked(100));
        assert_ne!(AuthResult::Locked(100), AuthResult::Locked(200));
    }

    #[test]
    fn platform_auth_current_username_returns_something() {
        let name = PlatformAuth::current_username();
        assert!(!name.is_empty());
    }

    // -- authenticate_creds wrapper --

    #[test]
    fn authenticate_creds_wrapper() {
        let auth: Box<dyn AuthBackend> = Box::new(MockAuth::succeeding());
        let creds = Credentials {
            username: "alice".into(),
            password: "pass".into(),
        };
        assert_eq!(auth.authenticate_creds(&creds), AuthResult::Success);
    }
}
