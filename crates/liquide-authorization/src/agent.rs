use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use liquide_common::event_log::EventLogService;

use crate::action::AuthorizationAction;
use crate::audit::{AuditEntry, AuditLog, AuditPolicy};
use crate::level::AuthLevel;
use crate::platform::{self, CredentialVerificationRequest, VerifyResult};
use crate::policy::AuthorizationPolicy;
use crate::policy_db::AuthDecision;
use crate::store::AuthorizationStore;
use crate::subject::{Resource, Subject};

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
    /// Optional in-memory audit log. When present, every terminal decision
    /// produced by [`AuthorizationAgent::request_authorization_audited`] is
    /// recorded here (subject to the log's [`AuditPolicy`]).
    audit: Option<AuditLog>,
    /// Optional structured event sink. When present, every terminal decision
    /// produced by [`AuthorizationAgent::request_authorization_audited`] is
    /// forwarded as an [`liquide_common::event_log::EventRecord`]. A sink error
    /// never upgrades a denial to a grant (fail-closed).
    sink: Option<Box<dyn EventLogService>>,
}

impl std::fmt::Debug for AuthorizationAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationAgent")
            .field("policy", &self.policy)
            .field("store", &self.store)
            .field("actions", &self.actions)
            .field("username", &self.username)
            .field("audit_enabled", &self.audit.is_some())
            .field("sink_enabled", &self.sink.is_some())
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
            audit: None,
            sink: None,
        }
    }

    /// Attach an in-memory audit log so audited authorization requests record
    /// every terminal decision. Additive: leaves the bare
    /// [`AuthorizationAgent::request_authorization`] flow unchanged.
    #[must_use]
    pub fn with_audit_log(mut self, audit: AuditLog) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Attach an audit log with the given policy.
    #[must_use]
    pub fn with_audit_policy(self, policy: AuditPolicy) -> Self {
        self.with_audit_log(AuditLog::new(policy))
    }

    /// Attach a structured event-log sink. Every audited terminal decision is
    /// forwarded to this sink as an [`liquide_common::event_log::EventRecord`].
    #[must_use]
    pub fn with_event_sink(mut self, sink: Box<dyn EventLogService>) -> Self {
        self.sink = Some(sink);
        self
    }

    /// Immutable access to the agent's audit log, if one is attached.
    #[must_use]
    pub fn audit_log(&self) -> Option<&AuditLog> {
        self.audit.as_ref()
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
                if username != verification_request.username || level != verification_request.level
                {
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

    /// Request authorization and record the terminal decision.
    ///
    /// This wraps [`AuthorizationAgent::request_authorization`] and, when an
    /// audit log and/or event sink are attached (via
    /// [`AuthorizationAgent::with_audit_log`] /
    /// [`AuthorizationAgent::with_event_sink`]), records every terminal
    /// grant/deny so privileged decisions are never silent
    /// (closes t49-e8-F2 / B6a within the crate).
    ///
    /// `subject` identifies the requester for the audit record. `resource`, if
    /// provided, attaches object-scoped context (resource-scoped authorization,
    /// B6d in-crate slice). `resource_scope` labels the kind of resource
    /// (`"window"`, `"session"`, `"device"`, ...).
    ///
    /// Returns the same [`AuthResult`] as the bare flow. Auditing is purely a
    /// side effect: a sink error is swallowed and never upgrades a denial to a
    /// grant (fail-closed).
    pub fn request_authorization_audited(
        &mut self,
        action: &AuthorizationAction,
        subject: &Subject,
        resource: Option<(&Resource, &str)>,
    ) -> AuthResult {
        let result = self.request_authorization(action);
        let decision = auth_result_to_decision(&result);
        let details = auth_result_details(&result);

        // Build a single canonical entry so the in-memory log and the
        // structured sink agree on the same record.
        let mut entry = AuditEntry::new(action.id.clone(), subject, decision.clone());
        if let Some((resource, scope)) = resource {
            entry = entry.for_resource(resource, scope);
        }
        if let Some(details) = &details {
            entry = entry.with_details(details.clone());
        }

        if let Some(audit) = self.audit.as_mut() {
            if let Some((resource, scope)) = resource {
                audit.record_resource(
                    &action.id,
                    subject,
                    &decision,
                    resource,
                    scope,
                    None,
                    details.as_deref(),
                );
            } else {
                audit.record(&action.id, subject, &decision, details.as_deref());
            }
        }

        if let Some(sink) = self.sink.as_mut() {
            // Swallow sink errors: auditing must not change the decision.
            let _ = sink.record_event(entry.to_event_record());
        }

        result
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
            audit: None,
            sink: None,
        };
        (agent, handle)
    }
}

/// Map an [`AuthResult`] to an audit-log [`AuthDecision`].
///
/// Fail-closed: every non-`Granted` outcome maps to [`AuthDecision::Deny`], so
/// a cancelled prompt or an error is audited as a denial, never an allow.
fn auth_result_to_decision(result: &AuthResult) -> AuthDecision {
    match result {
        AuthResult::Granted { .. } => AuthDecision::Allow,
        AuthResult::Denied { .. } | AuthResult::Cancelled | AuthResult::Error(_) => {
            AuthDecision::Deny
        }
    }
}

/// Human-readable detail string for the audit record, if any.
fn auth_result_details(result: &AuthResult) -> Option<String> {
    match result {
        AuthResult::Granted { .. } => None,
        AuthResult::Denied { reason } => Some(format!("denied: {reason}")),
        AuthResult::Cancelled => Some("cancelled by user".to_string()),
        AuthResult::Error(msg) => Some(format!("error: {msg}")),
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
            audit: None,
            sink: None,
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
        policy.add_rule(PolicyRule::new("org.liquide.test", AuthLevel::UserPassword));
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

    // ── Audited authorization (B6a) ─────────────────────────────────

    use crate::audit::AuditPolicy;
    use crate::policy_db::AuthDecision;
    use crate::subject::{Resource, Subject};
    use liquide_common::event_log::{EventCategory, EventLogService, EventRecord};

    /// A sink that records into shared state so tests can assert what the
    /// agent forwarded.
    struct SharedSink {
        records: Rc<RefCell<Vec<EventRecord>>>,
    }

    impl EventLogService for SharedSink {
        fn record_event(&mut self, record: EventRecord) -> liquide_common::Result<()> {
            self.records.borrow_mut().push(record);
            Ok(())
        }
    }

    /// A sink that always fails — proves a sink error never changes the
    /// decision (fail-closed) and is swallowed.
    struct FailingSink;

    impl EventLogService for FailingSink {
        fn record_event(&mut self, _record: EventRecord) -> liquide_common::Result<()> {
            Err(liquide_common::LiquideError::Internal(
                "sink down".to_string(),
            ))
        }
    }

    fn audit_subject() -> Subject {
        Subject::new(1000, 42, "session-audit")
    }

    #[test]
    fn audited_grant_is_recorded_to_log_and_sink() {
        // A NoAuth action grants without verification — exercises the grant
        // audit path deterministically.
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.system.*", AuthLevel::NoAuth));

        let records = Rc::new(RefCell::new(Vec::new()));
        let mut agent = AuthorizationAgent::new(policy, "testuser")
            .with_audit_policy(AuditPolicy::All)
            .with_event_sink(Box::new(SharedSink {
                records: records.clone(),
            }));
        let action = make_action("org.liquide.system.shutdown", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        let result = agent.request_authorization_audited(&action, &audit_subject(), None);
        assert!(result.is_granted());

        // Audit log captured an Allow.
        let log = agent.audit_log().expect("audit log attached");
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries()[0].decision, AuthDecision::Allow);
        assert_eq!(log.entries()[0].action_id, "org.liquide.system.shutdown");

        // Sink received exactly one authorization event.
        let sunk = records.borrow();
        assert_eq!(sunk.len(), 1);
        assert_eq!(sunk[0].category, EventCategory::Authorization);
    }

    #[test]
    fn audited_deny_records_a_denial_negative_path() {
        // No matching rule → Denied. The denial MUST be audited (privileged
        // ops are never silent) and forwarded as a Deny.
        let policy = AuthorizationPolicy::new(); // no rules
        let records = Rc::new(RefCell::new(Vec::new()));
        let mut agent = AuthorizationAgent::new(policy, "testuser")
            .with_audit_policy(AuditPolicy::All)
            .with_event_sink(Box::new(SharedSink {
                records: records.clone(),
            }));
        let action = make_action("org.liquide.unknown", AuthLevel::AdminPassword);

        let result = agent.request_authorization_audited(&action, &audit_subject(), None);
        assert!(result.is_denied());

        let log = agent.audit_log().expect("audit log attached");
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries()[0].decision, AuthDecision::Deny);

        let sunk = records.borrow();
        assert_eq!(sunk.len(), 1);
        // The forwarded event carries the deny context.
        assert_eq!(
            sunk[0].context.get("decision").map(String::as_str),
            Some("Deny")
        );
    }

    #[test]
    fn audited_resource_scope_flows_into_record() {
        // Resource-scoped authorization: the resource id + scope must reach the
        // audit entry and the forwarded event.
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.window.*", AuthLevel::NoAuth));
        let records = Rc::new(RefCell::new(Vec::new()));
        let mut agent = AuthorizationAgent::new(policy, "testuser")
            .with_audit_policy(AuditPolicy::All)
            .with_event_sink(Box::new(SharedSink {
                records: records.clone(),
            }));
        let action = make_action("org.liquide.window.capture", AuthLevel::NoAuth);
        agent.register_action(action.clone());
        let resource = Resource::new(1000, "window:42");

        let result = agent.request_authorization_audited(
            &action,
            &audit_subject(),
            Some((&resource, "window")),
        );
        assert!(result.is_granted());

        let log = agent.audit_log().expect("audit log attached");
        assert_eq!(log.entries()[0].resource_id.as_deref(), Some("window:42"));
        assert_eq!(log.entries()[0].resource_scope.as_deref(), Some("window"));

        let sunk = records.borrow();
        assert_eq!(sunk[0].resource_id.as_deref(), Some("window:42"));
        assert_eq!(
            sunk[0].context.get("resource_scope").map(String::as_str),
            Some("window")
        );
    }

    #[test]
    fn audited_sink_error_does_not_change_decision() {
        // A failing sink must NOT turn a grant into a deny, and must NOT panic;
        // the audit log still records the decision (fail-closed isolation).
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.system.*", AuthLevel::NoAuth));
        let mut agent = AuthorizationAgent::new(policy, "testuser")
            .with_audit_policy(AuditPolicy::All)
            .with_event_sink(Box::new(FailingSink));
        let action = make_action("org.liquide.system.suspend", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        let result = agent.request_authorization_audited(&action, &audit_subject(), None);
        assert!(
            result.is_granted(),
            "a sink error must not downgrade a grant"
        );
        assert_eq!(agent.audit_log().expect("log").len(), 1);
    }

    #[test]
    fn audited_without_sink_or_log_is_silent_but_returns_same_result() {
        // Additive contract: with no audit log and no sink attached, the
        // audited call behaves exactly like the bare flow.
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.system.*", AuthLevel::NoAuth));
        let mut agent = AuthorizationAgent::new(policy, "testuser");
        let action = make_action("org.liquide.system.shutdown", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        let audited = agent.request_authorization_audited(&action, &audit_subject(), None);
        assert!(audited.is_granted());
        assert!(agent.audit_log().is_none());
    }

    #[test]
    fn audited_denied_only_policy_skips_grant_records() {
        // With AuditPolicy::DeniedOnly, a granted decision is NOT recorded in
        // the log, but the sink still receives the structured event (the sink
        // is policy-independent — it observes all forwarded decisions).
        let mut policy = AuthorizationPolicy::new();
        policy.add_rule(PolicyRule::new("org.liquide.system.*", AuthLevel::NoAuth));
        let records = Rc::new(RefCell::new(Vec::new()));
        let mut agent = AuthorizationAgent::new(policy, "testuser")
            .with_audit_policy(AuditPolicy::DeniedOnly)
            .with_event_sink(Box::new(SharedSink {
                records: records.clone(),
            }));
        let action = make_action("org.liquide.system.shutdown", AuthLevel::NoAuth);
        agent.register_action(action.clone());

        agent.request_authorization_audited(&action, &audit_subject(), None);
        assert_eq!(
            agent.audit_log().expect("log").len(),
            0,
            "DeniedOnly suppresses grant records"
        );
        assert_eq!(
            records.borrow().len(),
            1,
            "sink still observes the decision"
        );
    }
}
