//! Authorization audit logging.
//!
//! Records authorization decisions for security auditing and
//! troubleshooting. The log is append-only and can be filtered by
//! time range, action, or subject.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::policy_db::AuthDecision;
use crate::subject::Subject;

// ── AuditPolicy ─────────────────────────────────────────────────────

/// Controls which authorization events are recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditPolicy {
    /// Log every authorization decision.
    All,
    /// Log only denied decisions.
    DeniedOnly,
    /// Log only decisions involving admin-level actions.
    AdminOnly,
    /// Disable logging entirely.
    None,
}

impl Default for AuditPolicy {
    fn default() -> Self {
        Self::All
    }
}

// ── AuditEntry ──────────────────────────────────────────────────────

/// A single entry in the audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When this event occurred (seconds since UNIX epoch).
    pub timestamp: u64,

    /// The action that was requested.
    pub action_id: String,

    /// User ID of the requesting subject.
    pub subject_uid: u32,

    /// Process ID of the requesting subject.
    pub subject_pid: u32,

    /// Session ID of the requesting subject.
    pub subject_session: String,

    /// The authorization decision that was made.
    pub decision: AuthDecision,

    /// Optional free-form details (e.g., "matched rule #3", "timeout").
    pub details: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry with the current timestamp.
    #[must_use]
    pub fn new(action_id: impl Into<String>, subject: &Subject, decision: AuthDecision) -> Self {
        Self {
            timestamp: now_secs(),
            action_id: action_id.into(),
            subject_uid: subject.uid,
            subject_pid: subject.pid,
            subject_session: subject.session_id.clone(),
            decision,
            details: None,
        }
    }

    /// Create an entry with a specific timestamp (for testing or replaying).
    #[must_use]
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Attach details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl std::fmt::Display for AuditEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] action={} uid={} pid={} session={} decision={:?}",
            self.timestamp,
            self.action_id,
            self.subject_uid,
            self.subject_pid,
            self.subject_session,
            self.decision,
        )?;
        if let Some(ref details) = self.details {
            write!(f, " details={details}")?;
        }
        Ok(())
    }
}

// ── AuditLog ────────────────────────────────────────────────────────

/// An append-only authorization audit log.
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    policy: AuditPolicy,
}

impl AuditLog {
    /// Create a new audit log with the given policy.
    #[must_use]
    pub fn new(policy: AuditPolicy) -> Self {
        Self {
            entries: Vec::new(),
            policy,
        }
    }

    /// Return the current audit policy.
    #[must_use]
    pub fn policy(&self) -> AuditPolicy {
        self.policy
    }

    /// Change the audit policy. Existing entries are not affected.
    pub fn set_policy(&mut self, policy: AuditPolicy) {
        self.policy = policy;
    }

    /// Record an authorization event, subject to the current policy.
    ///
    /// Returns `true` if the entry was actually recorded (i.e., it
    /// passed the policy filter).
    pub fn record(
        &mut self,
        action_id: &str,
        subject: &Subject,
        decision: &AuthDecision,
        details: Option<&str>,
    ) -> bool {
        if !self.should_record(decision) {
            return false;
        }

        let mut entry = AuditEntry::new(action_id, subject, decision.clone());
        if let Some(d) = details {
            entry = entry.with_details(d);
        }
        self.entries.push(entry);
        true
    }

    /// Directly append a pre-built entry (bypasses policy filter).
    pub fn append(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// Return all entries.
    #[must_use]
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Return the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Query entries within a time range (inclusive on both ends).
    #[must_use]
    pub fn query_by_time(&self, from: u64, to: u64) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .collect()
    }

    /// Query entries matching a specific action ID (exact match).
    #[must_use]
    pub fn query_by_action(&self, action_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.action_id == action_id)
            .collect()
    }

    /// Query entries from a specific subject (by uid).
    #[must_use]
    pub fn query_by_uid(&self, uid: u32) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.subject_uid == uid)
            .collect()
    }

    /// Query entries with a specific decision type.
    #[must_use]
    pub fn query_by_decision(&self, decision: &AuthDecision) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| &e.decision == decision)
            .collect()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Check whether a decision should be recorded under the current policy.
    fn should_record(&self, decision: &AuthDecision) -> bool {
        match self.policy {
            AuditPolicy::All => true,
            AuditPolicy::None => false,
            AuditPolicy::DeniedOnly => decision.is_deny(),
            AuditPolicy::AdminOnly => matches!(
                decision,
                AuthDecision::AuthRequired(crate::policy_db::AuthType::AdminPassword)
            ) || decision.is_deny(),
        }
    }
}

/// Get the current time in seconds since UNIX epoch.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_db::AuthType;

    fn test_subject() -> Subject {
        Subject::new(1000, 42, "session-1")
    }

    fn admin_subject() -> Subject {
        Subject::new(0, 1, "session-root")
    }

    // ── AuditEntry tests ────────────────────────────────────────────

    #[test]
    fn entry_new() {
        let entry = AuditEntry::new(
            "org.liquide.test",
            &test_subject(),
            AuthDecision::Allow,
        );
        assert_eq!(entry.action_id, "org.liquide.test");
        assert_eq!(entry.subject_uid, 1000);
        assert_eq!(entry.subject_pid, 42);
        assert_eq!(entry.subject_session, "session-1");
        assert_eq!(entry.decision, AuthDecision::Allow);
        assert!(entry.details.is_none());
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn entry_with_timestamp() {
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Deny)
            .with_timestamp(12345);
        assert_eq!(entry.timestamp, 12345);
    }

    #[test]
    fn entry_with_details() {
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Deny)
            .with_details("matched catch-all deny rule");
        assert_eq!(entry.details.as_deref(), Some("matched catch-all deny rule"));
    }

    #[test]
    fn entry_display() {
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Allow)
            .with_timestamp(99999);
        let s = entry.to_string();
        assert!(s.contains("99999"));
        assert!(s.contains("org.liquide.test"));
        assert!(s.contains("uid=1000"));
    }

    #[test]
    fn entry_display_with_details() {
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Allow)
            .with_timestamp(100)
            .with_details("test detail");
        let s = entry.to_string();
        assert!(s.contains("details=test detail"));
    }

    #[test]
    fn entry_serde_roundtrip() {
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Deny)
            .with_timestamp(55555)
            .with_details("denied");
        let json = serde_json::to_string(&entry).unwrap();
        let back: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.action_id, entry.action_id);
        assert_eq!(back.timestamp, entry.timestamp);
        assert_eq!(back.decision, entry.decision);
    }

    // ── AuditLog tests ──────────────────────────────────────────────

    #[test]
    fn log_new() {
        let log = AuditLog::new(AuditPolicy::All);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.policy(), AuditPolicy::All);
    }

    #[test]
    fn log_record_all() {
        let mut log = AuditLog::new(AuditPolicy::All);
        let s = test_subject();

        assert!(log.record("org.liquide.test", &s, &AuthDecision::Allow, None));
        assert!(log.record("org.liquide.test", &s, &AuthDecision::Deny, None));
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn log_record_denied_only() {
        let mut log = AuditLog::new(AuditPolicy::DeniedOnly);
        let s = test_subject();

        assert!(!log.record("org.liquide.test", &s, &AuthDecision::Allow, None));
        assert!(log.record("org.liquide.test", &s, &AuthDecision::Deny, None));
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn log_record_none() {
        let mut log = AuditLog::new(AuditPolicy::None);
        let s = test_subject();

        assert!(!log.record("org.liquide.test", &s, &AuthDecision::Allow, None));
        assert!(!log.record("org.liquide.test", &s, &AuthDecision::Deny, None));
        assert!(log.is_empty());
    }

    #[test]
    fn log_record_admin_only() {
        let mut log = AuditLog::new(AuditPolicy::AdminOnly);
        let s = test_subject();

        // Allow events are not logged
        assert!(!log.record("org.liquide.test", &s, &AuthDecision::Allow, None));
        // Deny events are logged
        assert!(log.record("org.liquide.test", &s, &AuthDecision::Deny, None));
        // Admin auth-required events are logged
        assert!(log.record(
            "org.liquide.test",
            &s,
            &AuthDecision::AuthRequired(AuthType::AdminPassword),
            None,
        ));
        // User auth-required events are NOT logged under AdminOnly
        assert!(!log.record(
            "org.liquide.test",
            &s,
            &AuthDecision::AuthRequired(AuthType::UserPassword),
            None,
        ));
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn log_record_with_details() {
        let mut log = AuditLog::new(AuditPolicy::All);
        let s = test_subject();
        log.record(
            "org.liquide.test",
            &s,
            &AuthDecision::Allow,
            Some("rule #1 matched"),
        );
        assert_eq!(
            log.entries()[0].details.as_deref(),
            Some("rule #1 matched")
        );
    }

    #[test]
    fn log_append_bypasses_policy() {
        let mut log = AuditLog::new(AuditPolicy::None);
        let entry = AuditEntry::new("org.liquide.test", &test_subject(), AuthDecision::Allow)
            .with_timestamp(100);
        log.append(entry);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn log_query_by_time() {
        let mut log = AuditLog::new(AuditPolicy::All);
        log.append(
            AuditEntry::new("a", &test_subject(), AuthDecision::Allow).with_timestamp(100),
        );
        log.append(
            AuditEntry::new("b", &test_subject(), AuthDecision::Deny).with_timestamp(200),
        );
        log.append(
            AuditEntry::new("c", &test_subject(), AuthDecision::Allow).with_timestamp(300),
        );

        let results = log.query_by_time(150, 250);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action_id, "b");

        let results = log.query_by_time(100, 300);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn log_query_by_action() {
        let mut log = AuditLog::new(AuditPolicy::All);
        let s = test_subject();
        log.append(
            AuditEntry::new("org.liquide.a", &s, AuthDecision::Allow).with_timestamp(1),
        );
        log.append(
            AuditEntry::new("org.liquide.b", &s, AuthDecision::Deny).with_timestamp(2),
        );
        log.append(
            AuditEntry::new("org.liquide.a", &s, AuthDecision::Deny).with_timestamp(3),
        );

        let results = log.query_by_action("org.liquide.a");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn log_query_by_uid() {
        let mut log = AuditLog::new(AuditPolicy::All);
        log.append(
            AuditEntry::new("a", &test_subject(), AuthDecision::Allow).with_timestamp(1),
        );
        log.append(
            AuditEntry::new("b", &admin_subject(), AuthDecision::Allow).with_timestamp(2),
        );

        let results = log.query_by_uid(1000);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject_uid, 1000);

        let results = log.query_by_uid(0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn log_query_by_decision() {
        let mut log = AuditLog::new(AuditPolicy::All);
        let s = test_subject();
        log.append(AuditEntry::new("a", &s, AuthDecision::Allow).with_timestamp(1));
        log.append(AuditEntry::new("b", &s, AuthDecision::Deny).with_timestamp(2));
        log.append(AuditEntry::new("c", &s, AuthDecision::Allow).with_timestamp(3));

        let results = log.query_by_decision(&AuthDecision::Deny);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action_id, "b");
    }

    #[test]
    fn log_clear() {
        let mut log = AuditLog::new(AuditPolicy::All);
        let s = test_subject();
        log.record("a", &s, &AuthDecision::Allow, None);
        log.record("b", &s, &AuthDecision::Deny, None);
        assert_eq!(log.len(), 2);

        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn log_set_policy() {
        let mut log = AuditLog::new(AuditPolicy::All);
        assert_eq!(log.policy(), AuditPolicy::All);

        log.set_policy(AuditPolicy::DeniedOnly);
        assert_eq!(log.policy(), AuditPolicy::DeniedOnly);
    }

    #[test]
    fn audit_policy_default() {
        assert_eq!(AuditPolicy::default(), AuditPolicy::All);
    }
}
