use std::time::Duration;

use crate::config::ResumeConfig;
use crate::resume::{ResumeManager, ResumeToken, TokenScope};

fn make_resume_config() -> ResumeConfig {
    ResumeConfig {
        enabled: true,
        token_lifetime_hours: 168,
        token_rotation: true,
        token_scope: TokenScope::SameServer,
        max_disconnected_minutes: 60,
        require_mfa_on_resume: false,
        require_mfa_after_hours: 24,
    }
}

// ---------------------------------------------------------------------------
// Resume tokens
// ---------------------------------------------------------------------------

#[test]
fn test_resume_token_creation() {
    let token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "user-a".into(),
        "fp-abc".into(),
        Duration::from_secs(3600),
        3,
        TokenScope::SameServer,
    );
    assert_eq!(token.token_id(), "tok-1");
    assert_eq!(token.session_id(), "sess-1");
    assert_eq!(token.user_id(), "user-a");
    assert_eq!(token.client_fingerprint(), "fp-abc");
    assert_eq!(token.scope(), TokenScope::SameServer);
    assert!(token.is_valid());
    assert!(!token.is_expired());
    assert_eq!(token.remaining_uses(), 3);
    assert_eq!(token.use_count(), 0);
}

#[test]
fn test_resume_token_scope_display() {
    assert_eq!(TokenScope::SameServer.to_string(), "SameServer");
    assert_eq!(TokenScope::AnyGateway.to_string(), "AnyGateway");
}

#[test]
fn test_resume_token_record_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "user-a".into(),
        "fp".into(),
        Duration::from_secs(3600),
        2,
        TokenScope::SameServer,
    );
    assert!(token.record_use());
    assert_eq!(token.use_count(), 1);
    assert_eq!(token.remaining_uses(), 1);
    assert!(token.is_valid());

    assert!(token.record_use());
    assert_eq!(token.use_count(), 2);
    assert_eq!(token.remaining_uses(), 0);
    assert!(!token.is_valid());
}

#[test]
fn test_resume_token_exhausted_denies_further_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::from_secs(3600),
        1,
        TokenScope::SameServer,
    );
    assert!(token.record_use());
    assert!(!token.record_use());
    assert_eq!(token.use_count(), 1);
}

#[test]
fn test_resume_token_zero_lifetime_is_expired() {
    let token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::ZERO,
        5,
        TokenScope::AnyGateway,
    );
    // A zero-duration token expires immediately.
    assert!(token.is_expired());
    assert!(!token.is_valid());
}

#[test]
fn test_resume_token_expired_denies_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::ZERO,
        5,
        TokenScope::SameServer,
    );
    assert!(!token.record_use());
    assert_eq!(token.use_count(), 0);
}

// ---------------------------------------------------------------------------
// Resume manager
// ---------------------------------------------------------------------------

#[test]
fn test_resume_manager_issue_and_validate() {
    let mut mgr = ResumeManager::new(make_resume_config());
    assert!(mgr.is_enabled());

    let token_id = mgr.issue_token("sess-1", "user-a", "fp-1").unwrap();
    assert!(token_id.starts_with("resume-"));
    assert_eq!(mgr.token_count(), 1);

    let session_id = mgr.validate_token(&token_id).unwrap();
    assert_eq!(session_id, "sess-1");
}

#[test]
fn test_resume_manager_disabled() {
    let config = ResumeConfig {
        enabled: false,
        ..make_resume_config()
    };
    let mut mgr = ResumeManager::new(config);
    assert!(!mgr.is_enabled());

    let result = mgr.issue_token("sess-1", "user-a", "fp-1");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_validate_invalid_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let result = mgr.validate_token("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_token_ids_increment() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let t1 = mgr.issue_token("s1", "u1", "fp").unwrap();
    let t2 = mgr.issue_token("s2", "u2", "fp").unwrap();
    // Tokens should be unique
    assert_ne!(t1, t2);
    // Tokens should start with "resume-" prefix and have random hex suffix
    assert!(t1.starts_with("resume-"), "token should start with 'resume-': {}", t1);
    assert!(t2.starts_with("resume-"), "token should start with 'resume-': {}", t2);
    // Token ID portion should be 32 hex characters (128 bits)
    assert_eq!(t1.len(), 7 + 32, "token should be 'resume-' + 32 hex chars");
    assert_eq!(t2.len(), 7 + 32, "token should be 'resume-' + 32 hex chars");
    assert_eq!(mgr.token_count(), 2);
}

#[test]
fn test_resume_manager_validate_consumes_use() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let tid = mgr.issue_token("s1", "u1", "fp").unwrap();

    // Tokens issued by ResumeManager have max_uses = 3.
    mgr.validate_token(&tid).unwrap(); // use 1
    mgr.validate_token(&tid).unwrap(); // use 2
    mgr.validate_token(&tid).unwrap(); // use 3

    // Fourth use should fail (token exhausted).
    let result = mgr.validate_token(&tid);
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_revoke_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let tid = mgr.issue_token("s1", "u1", "fp").unwrap();
    assert_eq!(mgr.token_count(), 1);

    mgr.revoke_token(&tid);
    assert_eq!(mgr.token_count(), 0);

    let result = mgr.validate_token(&tid);
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_revoke_nonexistent_is_noop() {
    let mut mgr = ResumeManager::new(make_resume_config());
    mgr.revoke_token("does-not-exist");
    assert_eq!(mgr.token_count(), 0);
}

#[test]
fn test_resume_manager_rotate_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let old_tid = mgr.issue_token("s1", "u1", "fp-1").unwrap();
    assert_eq!(mgr.token_count(), 1);

    let new_tid = mgr.rotate_token(&old_tid).unwrap();
    assert_ne!(old_tid, new_tid);
    assert_eq!(mgr.token_count(), 1);

    // Old token is revoked.
    let result = mgr.validate_token(&old_tid);
    assert!(result.is_err());

    // New token is valid.
    let session_id = mgr.validate_token(&new_tid).unwrap();
    assert_eq!(session_id, "s1");
}

#[test]
fn test_resume_manager_rotate_nonexistent_fails() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let result = mgr.rotate_token("no-such-token");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_cleanup_expired() {
    let mut mgr = ResumeManager::new(ResumeConfig {
        token_lifetime_hours: 0, // tokens expire immediately
        ..make_resume_config()
    });
    mgr.issue_token("s1", "u1", "fp").unwrap();
    mgr.issue_token("s2", "u2", "fp").unwrap();
    assert_eq!(mgr.token_count(), 2);

    let removed = mgr.cleanup_expired();
    assert_eq!(removed, 2);
    assert_eq!(mgr.token_count(), 0);
}

#[test]
fn test_resume_manager_cleanup_preserves_valid_tokens() {
    let mut mgr = ResumeManager::new(make_resume_config());
    mgr.issue_token("s1", "u1", "fp").unwrap();
    let removed = mgr.cleanup_expired();
    assert_eq!(removed, 0);
    assert_eq!(mgr.token_count(), 1);
}
