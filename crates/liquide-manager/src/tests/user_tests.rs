//! Tests for user/admin management.

use crate::config::AdminRole;
use crate::user_mgmt::AdminStore;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_store() -> AdminStore {
    let mut store = AdminStore::new();
    store.add("admin".into(), AdminRole::SuperAdmin);
    store.add("ops".into(), AdminRole::Operator);
    store.add("viewer".into(), AdminRole::Viewer);
    store
}

// ===========================================================================
// Basic operations
// ===========================================================================

#[test]
fn test_new_store_is_empty() {
    let store = AdminStore::new();
    assert_eq!(store.count(), 0);
}

#[test]
fn test_add_accounts() {
    let store = make_store();
    assert_eq!(store.count(), 3);
}

#[test]
fn test_duplicate_add_ignored() {
    let mut store = make_store();
    store.add("admin".into(), AdminRole::Viewer);
    assert_eq!(store.count(), 3);
    // role should not change
    let acct = store.get("admin").unwrap();
    assert_eq!(acct.role, AdminRole::SuperAdmin);
}

#[test]
fn test_get_account() {
    let store = make_store();
    let acct = store.get("ops").unwrap();
    assert_eq!(acct.username, "ops");
    assert_eq!(acct.role, AdminRole::Operator);
    assert!(!acct.locked);
}

#[test]
fn test_get_unknown_returns_none() {
    let store = make_store();
    assert!(store.get("nobody").is_none());
}

// ===========================================================================
// Authentication
// ===========================================================================

#[test]
fn test_authenticate_success() {
    let mut store = make_store();
    let result = store.authenticate("admin", 1000);
    assert!(result.is_ok());
    let acct = result.unwrap();
    assert_eq!(acct.role, AdminRole::SuperAdmin);
    assert_eq!(acct.last_login, Some(1000));
}

#[test]
fn test_authenticate_unknown_user() {
    let mut store = make_store();
    let result = store.authenticate("nobody", 1000);
    assert!(result.is_err());
}

#[test]
fn test_authenticate_locked_account() {
    let mut store = make_store();
    store.lock("ops").unwrap();
    let result = store.authenticate("ops", 1000);
    assert!(result.is_err());
}

#[test]
fn test_authenticate_lockout_expired() {
    let mut store = make_store();
    // Simulate lockout until t=500
    store.record_failure("ops", 1, 500, 0); // triggers lockout at t=0+500=500
    // Before expiry fails
    let result = store.authenticate("ops", 400);
    assert!(result.is_err());
    // After expiry succeeds
    let result = store.authenticate("ops", 600);
    assert!(result.is_ok());
}

// ===========================================================================
// Login failure tracking
// ===========================================================================

#[test]
fn test_record_failure_increments() {
    let mut store = make_store();
    let locked = store.record_failure("ops", 5, 900, 100);
    assert!(!locked);
    let acct = store.get("ops").unwrap();
    assert_eq!(acct.login_failures, 1);
}

#[test]
fn test_record_failure_triggers_lockout() {
    let mut store = make_store();
    for _ in 0..4 {
        store.record_failure("ops", 5, 900, 100);
    }
    let locked = store.record_failure("ops", 5, 900, 100);
    assert!(locked);
    let acct = store.get("ops").unwrap();
    assert_eq!(acct.lockout_until, Some(1000)); // 100 + 900
}

#[test]
fn test_record_failure_unknown_user_not_panics() {
    let mut store = make_store();
    let locked = store.record_failure("ghost", 5, 900, 100);
    assert!(!locked);
}

// ===========================================================================
// Role changes
// ===========================================================================

#[test]
fn test_set_role() {
    let mut store = make_store();
    store.set_role("viewer", AdminRole::Admin).unwrap();
    let acct = store.get("viewer").unwrap();
    assert_eq!(acct.role, AdminRole::Admin);
}

#[test]
fn test_set_role_unknown_user_errors() {
    let mut store = make_store();
    let result = store.set_role("nobody", AdminRole::Admin);
    assert!(result.is_err());
}

// ===========================================================================
// Lock / unlock
// ===========================================================================

#[test]
fn test_lock_account() {
    let mut store = make_store();
    store.lock("ops").unwrap();
    assert!(store.get("ops").unwrap().locked);
}

#[test]
fn test_unlock_account() {
    let mut store = make_store();
    store.lock("ops").unwrap();
    store.unlock("ops").unwrap();
    let acct = store.get("ops").unwrap();
    assert!(!acct.locked);
    assert_eq!(acct.login_failures, 0);
    assert!(acct.lockout_until.is_none());
}

#[test]
fn test_lock_unknown_errors() {
    let mut store = make_store();
    assert!(store.lock("ghost").is_err());
}

#[test]
fn test_unlock_unknown_errors() {
    let mut store = make_store();
    assert!(store.unlock("ghost").is_err());
}

// ===========================================================================
// List
// ===========================================================================

#[test]
fn test_list_accounts() {
    let store = make_store();
    assert_eq!(store.list().len(), 3);
}
