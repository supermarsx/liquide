//! Tests for authentication state, roles, and session management.

use crate::auth::{AuthManager, AuthRole, AuthSession, AuthState, LoginCredentials};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_session() -> AuthSession {
    AuthSession::new("admin", AuthRole::SuperAdmin, "tok-abc", 2000)
}

// ===========================================================================
// AuthRole
// ===========================================================================

#[test]
fn test_role_display() {
    assert_eq!(AuthRole::Viewer.to_string(), "viewer");
    assert_eq!(AuthRole::Operator.to_string(), "operator");
    assert_eq!(AuthRole::Admin.to_string(), "admin");
    assert_eq!(AuthRole::SuperAdmin.to_string(), "super-admin");
}

#[test]
fn test_role_ordering() {
    assert!(AuthRole::Viewer < AuthRole::Operator);
    assert!(AuthRole::Operator < AuthRole::Admin);
    assert!(AuthRole::Admin < AuthRole::SuperAdmin);
}

#[test]
fn test_role_has_permission() {
    assert!(AuthRole::SuperAdmin.has_permission(AuthRole::Viewer));
    assert!(AuthRole::SuperAdmin.has_permission(AuthRole::SuperAdmin));
    assert!(AuthRole::Admin.has_permission(AuthRole::Operator));
    assert!(!AuthRole::Viewer.has_permission(AuthRole::Operator));
    assert!(!AuthRole::Operator.has_permission(AuthRole::Admin));
}

// ===========================================================================
// AuthSession
// ===========================================================================

#[test]
fn test_session_creation() {
    let session = make_session();
    assert_eq!(session.username, "admin");
    assert_eq!(session.role, AuthRole::SuperAdmin);
    assert_eq!(session.token, "tok-abc");
    assert_eq!(session.expires_at, 2000);
}

#[test]
fn test_session_not_expired() {
    let session = make_session();
    assert!(!session.is_expired(1000));
    assert!(!session.is_expired(1999));
}

#[test]
fn test_session_expired_at_boundary() {
    let session = make_session();
    assert!(session.is_expired(2000));
    assert!(session.is_expired(3000));
}

// ===========================================================================
// AuthManager — state transitions
// ===========================================================================

#[test]
fn test_initial_state_is_unauthenticated() {
    let mgr = AuthManager::new();
    assert!(matches!(mgr.state(), AuthState::Unauthenticated));
    assert!(mgr.current_session().is_none());
}

#[test]
fn test_login_transitions_to_authenticating() {
    let mut mgr = AuthManager::new();
    let creds = LoginCredentials::new("alice", "password123");
    mgr.login(&creds);
    assert!(matches!(mgr.state(), AuthState::Authenticating));
}

#[test]
fn test_complete_login_sets_authenticated() {
    let mut mgr = AuthManager::new();
    mgr.complete_login(make_session());
    assert!(matches!(mgr.state(), AuthState::Authenticated { .. }));
    assert!(mgr.current_session().is_some());
    assert!(mgr.is_authenticated(1000));
}

#[test]
fn test_fail_login_sets_failed() {
    let mut mgr = AuthManager::new();
    mgr.fail_login("bad password");
    assert!(matches!(mgr.state(), AuthState::Failed { .. }));
    assert!(mgr.current_session().is_none());
}

#[test]
fn test_logout_clears_session() {
    let mut mgr = AuthManager::new();
    mgr.complete_login(make_session());
    mgr.logout();
    assert!(matches!(mgr.state(), AuthState::Unauthenticated));
    assert!(mgr.current_session().is_none());
    assert!(!mgr.is_authenticated(1000));
}

#[test]
fn test_refresh_updates_token() {
    let mut mgr = AuthManager::new();
    mgr.complete_login(make_session());
    mgr.refresh("tok-new", 5000);
    let session = mgr.current_session().unwrap();
    assert_eq!(session.token, "tok-new");
    assert_eq!(session.expires_at, 5000);
    assert!(mgr.is_authenticated(4000));
}
