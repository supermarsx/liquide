use crate::config::StealthConfig;
use crate::mode::AssistanceMode;
use crate::observer::{Observer, ObserverRole};
use crate::stealth::StealthSession;

fn make_security_admin() -> Observer {
    Observer::new(
        "sec-1".into(),
        "SecAdmin".into(),
        ObserverRole::SecurityAdmin,
        AssistanceMode::Stealth,
    )
}

fn make_helpdesk() -> Observer {
    Observer::new(
        "hd-1".into(),
        "HelpDesk".into(),
        ObserverRole::HelpDesk,
        AssistanceMode::ViewOnly,
    )
}

#[test]
fn test_stealth_creation_security_admin() {
    let obs = make_security_admin();
    let cfg = StealthConfig::default();
    let session = StealthSession::new(&obs, "target-1".into(), cfg);
    assert!(session.is_ok());
}

#[test]
fn test_stealth_creation_denied_for_helpdesk() {
    let obs = make_helpdesk();
    let cfg = StealthConfig::default();
    let session = StealthSession::new(&obs, "target-1".into(), cfg);
    assert!(session.is_err());
}

#[test]
fn test_stealth_audit_interval() {
    let obs = make_security_admin();
    let cfg = StealthConfig {
        audit_interval_seconds: 5,
        ..StealthConfig::default()
    };
    let mut session = StealthSession::new(&obs, "target-1".into(), cfg).unwrap();
    session.set_started_at(100);
    assert!(session.should_emit_audit(106));
    session.record_audit(106);
    assert!(!session.should_emit_audit(108));
    assert!(session.should_emit_audit(112));
}

#[test]
fn test_stealth_expiration() {
    let obs = make_security_admin();
    let cfg = StealthConfig {
        max_duration_minutes: 1,
        ..StealthConfig::default()
    };
    let mut session = StealthSession::new(&obs, "target-1".into(), cfg).unwrap();
    session.set_started_at(0);
    assert!(!session.is_expired(30));
    assert!(session.is_expired(60));
}

#[test]
fn test_stealth_duration() {
    let obs = make_security_admin();
    let cfg = StealthConfig::default();
    let mut session = StealthSession::new(&obs, "target-1".into(), cfg).unwrap();
    session.set_started_at(100);
    assert_eq!(session.duration_seconds(150), 50);
}
