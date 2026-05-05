use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::action::AuthorizationAction;
use crate::level::AuthLevel;
use crate::platform::{self, CredentialVerificationRequest, VerifyResult};
use crate::policy::AuthorizationPolicy;
use crate::store::AuthorizationStore;

/// The outcome of an authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    /// Authorization was granted. If keep-alive is enabled,
    /// `keep_alive_until` is the expiry timestamp (seconds since epoch).
    Granted { keep_alive_until: Option<u64> },
    /// Authorization was denied.
    Denied { reason: String },
    /// The user cancelled the authorization dialog.
    Cancelled,
    /// An error occurred during the authorization flow.
    Error(String),
}

impl AuthResult {
    /// Returns true if this result is `Granted`.
    #[must_use]
    pub fn is_granted(&self) -> bool {
        matches!(self, Self::Granted { .. })
    }

    /// Returns true if this result is `Denied`.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }
}

/// Provides a timestamp source that can be overridden for testing.
trait Clock {
    fn now_secs(&self) -> u64;
}

/// Real wall-clock time.
struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

trait CredentialVerifier {
    fn verify(&mut self, request: &CredentialVerificationRequest) -> VerifyResult;
}

#[derive(Debug, Default)]
struct PlatformCredentialVerifier;

impl CredentialVerifier for PlatformCredentialVerifier {
    fn verify(&mut self, request: &CredentialVerificationRequest) -> VerifyResult {
        platform::verify_authorization_request(request)
    }
}

/// The authorization agent manages the end-to-end flow of requesting,
/// verifying, and granting privileged actions.
///
/// It holds a policy (the rules), a store (the active grants), and a
/// registry of known actions. When `request_authorization` is called:
///
/// 1. Look up the action.
/// 2. Find the matching policy rule.
/// 3. If `NoAuth`, grant immediately.
/// 4. Check the keep-alive store — if a previous grant is still valid, reuse it.
/// 5. Otherwise, perform platform credential verification.
/// 6. On success, record the grant (with keep-alive if the rule allows it).
pub struct AuthorizationAgent {
    policy: AuthorizationPolicy,
    store: AuthorizationStore,
    actions: HashMap<String, AuthorizationAction>,
    clock: Box<dyn Clock>,
    verifier: Box<dyn CredentialVerifier>,
    /// The username used for credential verification.
    username: String,
}

impl std::fmt::Debug for AuthorizationAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationAgent")
            .field("policy", &self.policy)
            .field("store", &self.store)
            .field("actions", &self.actions)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl AuthorizationAgent {
    /// Create a new agent with the given policy and username.
    ///
    /// The agent starts with an empty action registry and grant store.
    /// Use `register_action` or `register_builtins` to populate actions.
    #[must_use]
    pub fn new(policy: AuthorizationPolicy, username: impl Into<String>) -> Self {
        Self {
            policy,
            store: AuthorizationStore::new(),
            actions: HashMap::new(),
            clock: Box::new(SystemClock),
            verifier: Box::new(PlatformCredentialVerifier),
            username: username.into(),
        }
    }

    /// Create an agent with default policies and all builtin actions registered.
    #[must_use]
    pub fn with_defaults(username: impl Into<String>) -> Self {
        let mut agent = Self::new(AuthorizationPolicy::with_defaults(), username);
        agent.register_builtins();
        agent
    }

    /// Register a single action.
    pub fn register_action(&mut self, action: AuthorizationAction) {
        self.actions.insert(action.id.clone(), action);
    }

    /// Register all builtin actions.
    pub fn register_builtins(&mut self) {
        for action in crate::builtin::builtin_actions() {
            self.actions.insert(action.id.clone(), action);
        }
    }

    /// Look up a registered action by ID.
    #[must_use]
    pub fn get_action(&self, action_id: &str) -> Option<&AuthorizationAction> {
        self.actions.get(action_id)
    }

    /// Request authorization for the given action.
    ///
    /// See the struct-level docs for the full flow.
    pub fn request_authorization(&mut self, action: &AuthorizationAction) -> AuthResult {
        // Find matching policy rule
        let rule = match self.policy.find_matching_rule(&action.id) {
            Some(r) => r.clone(),
            None => {
                return AuthResult::Denied {
                    reason: format!("No policy rule matched action: {}", action.id),
                };
            }
        };

        // Determine the effective level — the higher of the action's own
        // requirement and the policy rule's requirement.
        let effective_level = if action.required_level > rule.level {
            action.required_level
        } else {
            rule.level
        };

        // NoAuth → always grant
        if effective_level == AuthLevel::NoAuth {
            return AuthResult::Granted {
                keep_alive_until: None,
            };
        }

        // Check keep-alive store
        let now = self.clock.now_secs();
        if self.store.check(&action.id, now) {
            let expiry = self.store.expiry(&action.id);
            return AuthResult::Granted {
                keep_alive_until: expiry,
            };
        }

        let verification_request = CredentialVerificationRequest::new(
            action.id.clone(),
            self.username.clone(),
            effective_level,
        );
        let verify = self.verifier.verify(&verification_request);

        match verify {
            VerifyResult::Success { username, level } => {
                if username != verification_request.username || level != verification_request.level {
                    return AuthResult::Denied {
                        reason: format!(
                            "verification principal mismatch: requested '{}' at {}, got '{}' at {}",
                            verification_request.username,
                            verification_request.level,
                            username,
                            level
                        ),
                    };
                }

                let keep_alive_until = if rule.allow_keep_alive && rule.keep_alive_seconds > 0 {
                    let expiry = now + u64::from(rule.keep_alive_seconds);
                    self.store.grant(action.id.clone(), expiry);
                    Some(expiry)
                } else {
                    None
                };
                AuthResult::Granted { keep_alive_until }
            }
            VerifyResult::Cancelled => AuthResult::Cancelled,
            VerifyResult::Failed(reason) => AuthResult::Denied { reason },
            VerifyResult::Error(msg) => AuthResult::Error(msg),
        }
    }

    /// Check whether a previous keep-alive grant is still valid for the
    /// given action ID.
    #[must_use]
    pub fn check_keep_alive(&self, action_id: &str) -> bool {
        let now = self.clock.now_secs();
        self.store.check(action_id, now)
    }

    /// Revoke all active keep-alive grants.
    pub fn revoke_all(&mut self) {
        self.store.revoke_all();
    }

    /// Revoke the keep-alive grant for a specific action.
    pub fn revoke(&mut self, action_id: &str) {
        self.store.revoke(action_id);
    }

    /// Return an immutable reference to the policy.
    #[must_use]
    pub fn policy(&self) -> &AuthorizationPolicy {
        &self.policy
    }

    /// Return a mutable reference to the policy, allowing rule changes.
    pub fn policy_mut(&mut self) -> &mut AuthorizationPolicy {
        &mut self.policy
    }

    /// Clean up expired grants from the store.
    pub fn cleanup_expired(&mut self) {
        let now = self.clock.now_secs();
        self.store.cleanup_expired(now);
    }

    /// Create an agent with a fake clock for testing.
    /// Returns the agent and a handle to control the clock.
    #[cfg(test)]
    fn with_test_clock(
        policy: AuthorizationPolicy,
        username: impl Into<String>,
        initial_time: u64,
    ) -> (Self, TestClockHandle) {
        let clock = TestClock::new(initial_time);
        let handle = clock.handle();
        let agent = Self {
            policy,
            store: AuthorizationStore::new(),
            actions: HashMap::new(),
            clock: Box::new(clock),
            verifier: Box::new(PlatformCredentialVerifier),
            username: username.into(),
        };
        (agent, handle)
    }
}

/// A controllable clock for unit tests using shared state.
#[cfg(test)]
struct TestClock {
    now: std::rc::Rc<std::cell::Cell<u64>>,
}

#[cfg(test)]
impl TestClock {
    fn new(now: u64) -> Self {
        Self {
            now: std::rc::Rc::new(std::cell::Cell::new(now)),
        }
    }

    /// Create a handle that shares the same underlying time.
    fn handle(&self) -> TestClockHandle {
        TestClockHandle {
            now: self.now.clone(),
        }
    }
}

#[cfg(test)]
impl Clock for TestClock {
    fn now_secs(&self) -> u64 {
        self.now.get()
    }
}

/// A shareable handle to a `TestClock` that can advance time.
#[cfg(test)]
struct TestClockHandle {
    now: std::rc::Rc<std::cell::Cell<u64>>,
}

#[cfg(test)]
impl TestClockHandle {
    fn advance(&self, secs: u64) {
        self.now.set(self.now.get() + secs);
    }

    #[allow(dead_code)]
    fn set(&self, t: u64) {
        self.now.set(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicyRule;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_action(id: &str, level: AuthLevel) -> AuthorizationAction {
        AuthorizationAction::new(id, "test action", "please authenticate", level)
    }

    struct RecordingVerifier {
        result: VerifyResult,
        seen: Rc<RefCell<Vec<CredentialVerificationRequest>>>,
    }

    impl CredentialVerifier for RecordingVerifier {
        fn verify(&mut self, request: &CredentialVerificationRequest) -> VerifyResult {
            self.seen.borrow_mut().push(request.clone());
            self.result.clone()
        }
    }

    fn agent_with_verifier(
        policy: AuthorizationPolicy,
        username: &str,
        result: VerifyResult,
        seen: Rc<RefCell<Vec<CredentialVerificationRequest>>>,
    ) -> AuthorizationAgent {
        AuthorizationAgent {
            policy,
            store: AuthorizationStore::new(),
            actions: HashMap::new(),
            clock: Box::new(TestClock::new(1000)),
            verifier: Box::new(RecordingVerifier { result, seen }),
            username: username.to_string(),
        }
    }

    #[test]
    fn noauth_granted_immediately() {
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.system.*", AuthLevel::NoAuth));

        let mut agent = AuthorizationAgent::new(policy, "testuser");
        let action = make_action("org.liquide.system.shutdown", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        let result = agent.request_authorization(&action);
        assert_eq!(
            result,
            AuthResult::Granted {
                keep_alive_until: None
            }
        );
    }

    #[test]
    fn denied_when_no_rule_matches() {
        let policy = AuthorizationPolicy::new(); // no rules at all
        let mut agent = AuthorizationAgent::new(policy, "testuser");
        let action = make_action("org.liquide.unknown", AuthLevel::AdminPassword);

        let result = agent.request_authorization(&action);
        assert!(result.is_denied());
    }

    #[test]
    fn verifier_receives_action_identity_and_effective_level() {
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new(
            "org.liquide.test",
            AuthLevel::AdminPassword,
        ));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut agent = agent_with_verifier(
            policy,
            "alice",
            VerifyResult::Success {
                username: "alice".to_string(),
                level: AuthLevel::AdminPassword,
            },
            seen.clone(),
        );
        let action = make_action("org.liquide.test", AuthLevel::UserPassword);

        let result = agent.request_authorization(&action);

        assert!(result.is_granted());
        let requests = seen.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].action_id, "org.liquide.test");
        assert_eq!(requests[0].username, "alice");
        assert_eq!(requests[0].level, AuthLevel::AdminPassword);
    }

    #[test]
    fn mismatched_verifier_success_is_denied() {
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new(
            "org.liquide.test",
            AuthLevel::UserPassword,
        ));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut agent = agent_with_verifier(
            policy,
            "alice",
            VerifyResult::Success {
                username: "mallory".to_string(),
                level: AuthLevel::UserPassword,
            },
            seen,
        );
        let action = make_action("org.liquide.test", AuthLevel::UserPassword);

        let result = agent.request_authorization(&action);

        assert!(matches!(
            result,
            AuthResult::Denied { reason } if reason.contains("principal mismatch")
        ));
    }

    #[test]
    fn keep_alive_reuse() {
        let mut policy = AuthorizationPolicy::new();
        // NoAuth rule with keep-alive so we can test the store without
        // needing actual credential verification.
        policy
            .add_rule(PolicyRule::new("org.liquide.test", AuthLevel::NoAuth).with_keep_alive(300));

        let (mut agent, clock_handle) =
            AuthorizationAgent::with_test_clock(policy, "testuser", 1000);
        let action = make_action("org.liquide.test", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        // First request — granted (NoAuth)
        let r = agent.request_authorization(&action);
        assert!(r.is_granted());

        // Manually seed the store to simulate a previous password-verified grant
        agent.store.grant("org.liquide.test".to_string(), 1300);
        assert!(agent.check_keep_alive("org.liquide.test"));

        // Advance past expiry
        clock_handle.advance(301);
        // Keep-alive should now be expired
        assert!(!agent.check_keep_alive("org.liquide.test"));
    }

    #[test]
    fn revoke_all_clears_grants() {
        let mut agent = AuthorizationAgent::new(AuthorizationPolicy::with_defaults(), "testuser");
        agent.store.grant("a".to_string(), u64::MAX);
        agent.store.grant("b".to_string(), u64::MAX);
        assert!(agent.check_keep_alive("a"));

        agent.revoke_all();
        assert!(!agent.check_keep_alive("a"));
        assert!(!agent.check_keep_alive("b"));
    }

    #[test]
    fn revoke_single_grant() {
        let mut agent = AuthorizationAgent::new(AuthorizationPolicy::with_defaults(), "testuser");
        agent.store.grant("a".to_string(), u64::MAX);
        agent.store.grant("b".to_string(), u64::MAX);

        agent.revoke("a");
        assert!(!agent.check_keep_alive("a"));
        assert!(agent.check_keep_alive("b"));
    }

    #[test]
    fn with_defaults_has_builtins() {
        let agent = AuthorizationAgent::with_defaults("testuser");
        assert!(agent.get_action("org.liquide.system.shutdown").is_some());
        assert!(agent.get_action("org.liquide.package.install").is_some());
        assert!(agent.get_action("org.liquide.service.start").is_some());
    }

    #[test]
    fn effective_level_uses_higher_of_action_and_policy() {
        // When action says AdminPassword but policy says NoAuth, the
        // effective level should be AdminPassword (the higher of the two).
        // We test this indirectly: if the effective level were NoAuth, the
        // agent would return Granted immediately without touching the store.
        // With AdminPassword, it first checks keep-alive, then does platform
        // verification. We seed the keep-alive store so that the agent finds
        // a valid grant and returns Granted *with* a keep_alive_until value,
        // proving it took the password path (which checks the store), not the
        // NoAuth path (which skips the store entirely).
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.test", AuthLevel::NoAuth));

        let (mut agent, _handle) = AuthorizationAgent::with_test_clock(policy, "testuser", 1000);
        let action = make_action("org.liquide.test", AuthLevel::AdminPassword);
        agent.register_action(action.clone());

        // Seed a keep-alive grant in the store
        agent.store.grant("org.liquide.test".to_string(), 2000);

        let result = agent.request_authorization(&action);
        // If effective level were NoAuth, it would return Granted { keep_alive_until: None }
        // without checking the store. Since it IS AdminPassword, it checks the
        // store, finds a valid grant, and returns Granted { keep_alive_until: Some(2000) }.
        assert_eq!(
            result,
            AuthResult::Granted {
                keep_alive_until: Some(2000)
            }
        );
    }

    #[test]
    fn cleanup_expired_works() {
        let (mut agent, _handle) = AuthorizationAgent::with_test_clock(
            AuthorizationPolicy::with_defaults(),
            "testuser",
            500,
        );
        agent.store.grant("early".to_string(), 100); // already expired at t=500
        agent.store.grant("later".to_string(), 1000);
        assert_eq!(agent.store.len(), 2);

        agent.cleanup_expired();
        assert_eq!(agent.store.len(), 1);
        assert!(!agent.check_keep_alive("early"));
        assert!(agent.check_keep_alive("later"));
    }

    #[test]
    fn auth_result_predicates() {
        assert!(
            AuthResult::Granted {
                keep_alive_until: None
            }
            .is_granted()
        );
        assert!(
            !AuthResult::Granted {
                keep_alive_until: None
            }
            .is_denied()
        );
        assert!(
            AuthResult::Denied {
                reason: "no".into()
            }
            .is_denied()
        );
        assert!(!AuthResult::Cancelled.is_granted());
        assert!(!AuthResult::Error("x".into()).is_granted());
    }

    #[test]
    fn policy_mut_allows_modification() {
        let mut agent = AuthorizationAgent::with_defaults("testuser");
        let before = agent.policy().len();
        agent
            .policy_mut()
            .add_rule(PolicyRule::new("org.custom.action", AuthLevel::Fingerprint));
        assert_eq!(agent.policy().len(), before + 1);
    }

    #[test]
    fn debug_impl() {
        let agent = AuthorizationAgent::with_defaults("testuser");
        let dbg = format!("{:?}", agent);
        assert!(dbg.contains("AuthorizationAgent"));
        assert!(dbg.contains("testuser"));
    }
}
