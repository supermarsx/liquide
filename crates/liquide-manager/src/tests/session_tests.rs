//! Tests for session management.

use crate::session_mgmt::{SessionStatus, SessionStore};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_store() -> SessionStore {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv-a".into(), 100);
    store.upsert("s2".into(), "bob".into(), "srv-a".into(), 200);
    store.upsert("s3".into(), "alice".into(), "srv-b".into(), 300);
    store
}

// ===========================================================================
// Basic operations
// ===========================================================================

#[test]
fn test_new_store_is_empty() {
    let store = SessionStore::new();
    assert_eq!(store.count(), 0);
    assert_eq!(store.unique_users(), 0);
}

#[test]
fn test_upsert_and_count() {
    let store = make_store();
    assert_eq!(store.count(), 3);
}

#[test]
fn test_upsert_updates_existing() {
    let mut store = make_store();
    store.upsert("s1".into(), "charlie".into(), "srv-c".into(), 500);
    // count should not change
    assert_eq!(store.count(), 3);
    let s = store.get("s1", 600).unwrap();
    assert_eq!(s.user, "charlie");
    assert_eq!(s.server, "srv-c");
}

#[test]
fn test_get_returns_none_for_unknown() {
    let store = make_store();
    assert!(store.get("unknown", 0).is_none());
}

#[test]
fn test_initial_status_is_active() {
    let store = make_store();
    let s = store.get("s1", 500).unwrap();
    assert_eq!(s.status, SessionStatus::Active);
}

#[test]
fn test_duration_calculation() {
    let store = make_store();
    let s = store.get("s1", 500).unwrap();
    assert_eq!(s.duration_seconds, 400); // 500 - 100
}

// ===========================================================================
// Unique users
// ===========================================================================

#[test]
fn test_unique_users() {
    let store = make_store();
    assert_eq!(store.unique_users(), 2); // alice, bob
}

// ===========================================================================
// User queries
// ===========================================================================

#[test]
fn test_sessions_for_user() {
    let store = make_store();
    let alice = store.sessions_for_user("alice", 500);
    assert_eq!(alice.len(), 2);
    let bob = store.sessions_for_user("bob", 500);
    assert_eq!(bob.len(), 1);
    let nobody = store.sessions_for_user("nobody", 500);
    assert!(nobody.is_empty());
}

// ===========================================================================
// Lock / unlock
// ===========================================================================

#[test]
fn test_lock_session() {
    let mut store = make_store();
    store
        .lock_session("s1", Some("maintenance".into()))
        .unwrap();
    let s = store.get("s1", 500).unwrap();
    assert_eq!(s.status, SessionStatus::Locked);
}

#[test]
fn test_unlock_session() {
    let mut store = make_store();
    store.lock_session("s1", None).unwrap();
    store.unlock_session("s1").unwrap();
    let s = store.get("s1", 500).unwrap();
    assert_eq!(s.status, SessionStatus::Active);
}

#[test]
fn test_lock_unknown_session_errors() {
    let mut store = make_store();
    let result = store.lock_session("unknown", None);
    assert!(result.is_err());
}

#[test]
fn test_unlock_unknown_session_errors() {
    let mut store = make_store();
    let result = store.unlock_session("unknown");
    assert!(result.is_err());
}

// ===========================================================================
// Remove
// ===========================================================================

#[test]
fn test_remove_session() {
    let mut store = make_store();
    store.remove("s1");
    assert_eq!(store.count(), 2);
    assert!(store.get("s1", 0).is_none());
}

#[test]
fn test_remove_unknown_is_noop() {
    let mut store = make_store();
    store.remove("unknown");
    assert_eq!(store.count(), 3);
}

// ===========================================================================
// Metrics update
// ===========================================================================

#[test]
fn test_update_metrics() {
    let mut store = make_store();
    store.update_metrics("s1", 12.5, 60.0, 1_000_000);
    let s = store.get("s1", 500).unwrap();
    assert!((s.latency_ms - 12.5).abs() < f32::EPSILON);
    assert!((s.fps - 60.0).abs() < f32::EPSILON);
    assert_eq!(s.bandwidth_bps, 1_000_000);
}

// ===========================================================================
// List
// ===========================================================================

#[test]
fn test_list() {
    let store = make_store();
    let all = store.list(500);
    assert_eq!(all.len(), 3);
}

// ===========================================================================
// Display
// ===========================================================================

#[test]
fn test_session_status_display() {
    assert_eq!(SessionStatus::Active.to_string(), "active");
    assert_eq!(SessionStatus::Locked.to_string(), "locked");
    assert_eq!(SessionStatus::Suspended.to_string(), "suspended");
    assert_eq!(SessionStatus::Disconnecting.to_string(), "disconnecting");
}
