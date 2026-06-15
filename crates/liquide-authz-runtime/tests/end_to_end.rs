//! End-to-end integration test for the authorization + audit/event-log planes.
//!
//! This drives the **real, non-`#[cfg(test)]`-gated public API** exactly as a
//! production consumer (session/shell) would: it constructs an
//! [`AuthorizationRuntime`] with a real append-only file sink, enforces both a
//! granted and a denied privileged operation, and then asserts that
//!   * the runtime's in-memory audit log recorded each decision, and
//!   * the on-disk event trail can be read back and contains the matching
//!     audit events (allow + deny), verifiable via the public read API.
//!
//! Proving this plane WORKS end-to-end through the public surface means the
//! only outstanding work is the call-site wiring (see the wiring spec in the
//! task log); the planes themselves are production-ready.

use std::sync::atomic::{AtomicU64, Ordering};

use liquide_authz_runtime::{AuditSinkConfig, AuthorizationRuntime, Resource, Subject};
use liquide_common::event_log::{AppendOnlyEventLog, EventCategory, EventLevel};

/// A unique temp audit path (never the real platform location).
fn temp_audit_path(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "liquide-authz-e2e-{}-{tag}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp audit dir");
    dir.push("events.log");
    dir
}

#[test]
fn enforce_grant_and_deny_audits_in_memory_and_to_file_then_read_back() {
    let path = temp_audit_path("grant-deny");
    assert!(!path.exists(), "audit file must not exist before any decision");

    // Production constructor: real append-only file sink + Checkpoint A catalog.
    let config = AuditSinkConfig::with_path(path.clone());
    let mut runtime = AuthorizationRuntime::with_audit_file("tester", config);

    let subject = Subject::new(1000, 4242, "session-e2e");

    // ── GRANT path ──────────────────────────────────────────────────────
    // `power.suspend` is gated + NoAuth → the real authorization flow returns
    // Granted deterministically (no platform credential prompt needed).
    assert_eq!(runtime.catalog().is_gated("power.suspend"), Some(true));
    let grant = runtime.authorize("power.suspend", &subject, None);
    assert!(grant.is_granted(), "gated NoAuth op must be granted");

    // ── DENY path ───────────────────────────────────────────────────────
    // Unknown catalog key → fail-closed Denied, audited as a deny.
    let resource = Resource::new(1000, "user:alice");
    let deny = runtime.authorize("accounts.no_such_op", &subject, Some(&resource));
    assert!(deny.is_denied(), "unknown op must be denied (fail-closed)");

    // ── In-memory audit log ─────────────────────────────────────────────
    let entries = runtime.audit().entries();
    assert_eq!(entries.len(), 2, "both decisions audited in memory");
    assert_eq!(entries[0].action_id, "power.suspend");
    assert!(entries[0].decision.is_allow());
    assert_eq!(entries[1].action_id, "accounts.no_such_op");
    assert!(entries[1].decision.is_deny());
    assert_eq!(entries[1].resource_id.as_deref(), Some("user:alice"));

    // ── On-disk trail, read back through the public read API ─────────────
    let reader = AppendOnlyEventLog::new(&path);
    let events = reader.read_all().expect("read on-disk audit trail back");
    assert_eq!(events.len(), 2, "both decisions appended to the file");

    // Every audit event is an Authorization-category record; the catalog key
    // flows through as the event id.
    assert!(events.iter().all(|e| e.category == EventCategory::Authorization));

    let grant_event = &events[0];
    assert_eq!(grant_event.event_id, "power.suspend");
    assert_eq!(grant_event.session_id.as_deref(), Some("session-e2e"));
    // Granted decisions are Debug-level (not a warning).
    assert_eq!(grant_event.level, EventLevel::Debug);
    assert_eq!(
        grant_event.context.get("decision").map(String::as_str),
        Some("Allow")
    );

    let deny_event = &events[1];
    assert_eq!(deny_event.event_id, "accounts.no_such_op");
    // Denials are Warn-level and carry the resource + reason context.
    assert_eq!(deny_event.level, EventLevel::Warn);
    assert_eq!(deny_event.resource_id.as_deref(), Some("user:alice"));
    assert_eq!(
        deny_event.context.get("decision").map(String::as_str),
        Some("Deny")
    );

    // Cleanup (best-effort).
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
