use crate::invite::{InviteCode, InviteRegistry};
use crate::mode::AssistanceMode;

#[test]
fn test_generate_invite() {
    let invite = InviteCode::generate("owner-1".into(), AssistanceMode::ViewOnly, 300, 5);
    assert!(invite.code.starts_with("ASSIST-"));
    assert_eq!(invite.max_uses, 5);
    assert_eq!(invite.uses, 0);
}

#[test]
fn test_invite_validity() {
    let invite = InviteCode::generate("owner-1".into(), AssistanceMode::ViewOnly, 300, 2);
    assert!(invite.is_valid(100));
    assert!(!invite.is_valid(300));
}

#[test]
fn test_invite_redeem() {
    let mut invite = InviteCode::generate("owner-1".into(), AssistanceMode::Interactive, 300, 2);
    invite.redeem().unwrap();
    assert_eq!(invite.uses, 1);
    invite.redeem().unwrap();
    assert!(invite.is_exhausted());
    assert!(invite.redeem().is_err());
}

#[test]
fn test_registry_lookup() {
    let mut registry = InviteRegistry::new();
    let invite = InviteCode::generate("owner-1".into(), AssistanceMode::ViewOnly, 300, 1);
    let code = invite.code.clone();
    registry.register(invite);
    assert!(registry.lookup(&code).is_some());
    assert!(registry.lookup("nonexistent").is_none());
}

#[test]
fn test_registry_cleanup() {
    let mut registry = InviteRegistry::new();
    let invite = InviteCode::generate("owner-1".into(), AssistanceMode::ViewOnly, 100, 5);
    let code = invite.code.clone();
    registry.register(invite);
    assert!(registry.lookup(&code).is_some());
    registry.cleanup_expired(200);
    assert!(registry.lookup(&code).is_none());
}
