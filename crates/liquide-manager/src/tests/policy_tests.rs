//! Tests for policy management.

use crate::policy_mgmt::{PolicyEntry, PolicyScope, PolicyStore};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn entry(key: &str, val: &str) -> PolicyEntry {
    PolicyEntry {
        key: key.to_string(),
        value: val.to_string(),
        scope: PolicyScope::Default,
        target: String::new(),
    }
}

fn make_store_with_versions() -> PolicyStore {
    let mut store = PolicyStore::new();
    store.commit(
        vec![entry("clipboard.enabled", "true"), entry("audio.enabled", "true")],
        "admin".into(),
        "initial".into(),
        1000,
    );
    store.commit(
        vec![entry("clipboard.enabled", "false"), entry("audio.enabled", "true")],
        "admin".into(),
        "disable clipboard".into(),
        2000,
    );
    store
}

// ===========================================================================
// Empty store
// ===========================================================================

#[test]
fn test_new_store_is_empty() {
    let store = PolicyStore::new();
    assert_eq!(store.current_version(), 0);
    assert_eq!(store.version_count(), 0);
    assert!(store.current_entries().is_empty());
}

// ===========================================================================
// Commit
// ===========================================================================

#[test]
fn test_commit_increments_version() {
    let mut store = PolicyStore::new();
    let v1 = store.commit(vec![entry("a", "1")], "admin".into(), "v1".into(), 100);
    assert_eq!(v1, 1);
    let v2 = store.commit(vec![entry("a", "2")], "admin".into(), "v2".into(), 200);
    assert_eq!(v2, 2);
    assert_eq!(store.current_version(), 2);
}

#[test]
fn test_commit_stores_entries() {
    let store = make_store_with_versions();
    let entries = store.current_entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].value, "false"); // clipboard disabled in v2
}

// ===========================================================================
// Version retrieval
// ===========================================================================

#[test]
fn test_get_version() {
    let store = make_store_with_versions();
    let v1 = store.get_version(1).unwrap();
    assert_eq!(v1.description, "initial");
    assert_eq!(v1.entries.len(), 2);
    assert_eq!(v1.entries[0].value, "true"); // clipboard enabled in v1
}

#[test]
fn test_get_unknown_version() {
    let store = make_store_with_versions();
    assert!(store.get_version(99).is_none());
}

// ===========================================================================
// History
// ===========================================================================

#[test]
fn test_history() {
    let store = make_store_with_versions();
    let history = store.history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, 1);
    assert_eq!(history[1].version, 2);
}

// ===========================================================================
// Diff
// ===========================================================================

#[test]
fn test_diff_between_versions() {
    let store = make_store_with_versions();
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1); // only clipboard changed
    assert_eq!(diffs[0].key, "clipboard.enabled");
    assert_eq!(diffs[0].old_value.as_deref(), Some("true"));
    assert_eq!(diffs[0].new_value.as_deref(), Some("false"));
}

#[test]
fn test_diff_added_entry() {
    let mut store = PolicyStore::new();
    store.commit(vec![], "admin".into(), "empty".into(), 100);
    store.commit(vec![entry("new.key", "val")], "admin".into(), "add".into(), 200);
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].old_value.is_none());
    assert_eq!(diffs[0].new_value.as_deref(), Some("val"));
}

#[test]
fn test_diff_removed_entry() {
    let mut store = PolicyStore::new();
    store.commit(vec![entry("old.key", "val")], "admin".into(), "has".into(), 100);
    store.commit(vec![], "admin".into(), "removed".into(), 200);
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].old_value.as_deref(), Some("val"));
    assert!(diffs[0].new_value.is_none());
}

// ===========================================================================
// Rollback
// ===========================================================================

#[test]
fn test_rollback() {
    let mut store = make_store_with_versions();
    let v3 = store.rollback(1, "admin".into(), 3000).unwrap();
    assert_eq!(v3, 3);
    assert_eq!(store.current_version(), 3);
    // entries should match v1
    let entries = store.current_entries();
    assert_eq!(entries[0].value, "true"); // clipboard re-enabled
}

#[test]
fn test_rollback_unknown_version() {
    let mut store = make_store_with_versions();
    let result = store.rollback(99, "admin".into(), 3000);
    assert!(result.is_err());
}

// ===========================================================================
// Scope display
// ===========================================================================

#[test]
fn test_policy_scope_display() {
    assert_eq!(PolicyScope::Default.to_string(), "default");
    assert_eq!(PolicyScope::Group.to_string(), "group");
    assert_eq!(PolicyScope::User.to_string(), "user");
    assert_eq!(PolicyScope::Session.to_string(), "session");
}
