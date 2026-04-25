use crate::config::{AssistanceConfig, ModeConfig, PermissionsConfig, StealthConfig};
use crate::coordinator::AssistanceCoordinator;
use crate::message::EndReason;
use crate::mode::AssistanceMode;
use crate::observer::{Observer, ObserverRole};

fn make_coordinator() -> AssistanceCoordinator {
    AssistanceCoordinator::new(
        AssistanceConfig::default(),
        ModeConfig::default(),
        StealthConfig::default(),
        PermissionsConfig::default(),
    )
}

fn make_observer() -> Observer {
    Observer::new(
        "obs-1".into(),
        "Alice".into(),
        ObserverRole::HelpDesk,
        AssistanceMode::ViewOnly,
    )
}

#[test]
fn test_request_assistance() {
    let mut coord = make_coordinator();
    let obs = make_observer();
    let prompt = coord.request_assistance(&obs, "target-1", AssistanceMode::ViewOnly, "need help");
    assert!(prompt.is_ok());
    let prompt = prompt.unwrap();
    assert_eq!(prompt.observer_name, "Alice");
}

#[test]
fn test_request_disabled() {
    let mut cfg = AssistanceConfig::default();
    cfg.enabled = false;
    let mut coord = AssistanceCoordinator::new(
        cfg,
        ModeConfig::default(),
        StealthConfig::default(),
        PermissionsConfig::default(),
    );
    let obs = make_observer();
    let result = coord.request_assistance(&obs, "target-1", AssistanceMode::ViewOnly, "help");
    assert!(result.is_err());
}

#[test]
fn test_create_invite() {
    let mut coord = make_coordinator();
    let result = coord.create_invite("owner-1", AssistanceMode::ViewOnly, 300, 5);
    assert!(result.is_ok());
    let invite_msg = result.unwrap();
    assert!(invite_msg.code.starts_with("ASSIST-"));
    assert!(invite_msg.url.contains(&invite_msg.code));
}

#[test]
fn test_join_with_code() {
    let mut coord = make_coordinator();
    let invite_msg = coord
        .create_invite("owner-1", AssistanceMode::Interactive, 300, 5)
        .unwrap();
    let obs = make_observer();
    let granted = coord.join_with_code(&invite_msg.code, &obs);
    assert!(granted.is_ok());
    let granted = granted.unwrap();
    assert!(granted.shadow_session_id.starts_with("shadow-"));
}

#[test]
fn test_end_assistance() {
    let mut coord = make_coordinator();
    let invite_msg = coord
        .create_invite("owner-1", AssistanceMode::ViewOnly, 300, 1)
        .unwrap();
    let obs = make_observer();
    let granted = coord.join_with_code(&invite_msg.code, &obs).unwrap();
    let result = coord.end_assistance(&granted.shadow_session_id, EndReason::ObserverLeft);
    assert!(result.is_ok());
}

#[test]
fn test_drain_audit_events() {
    let mut coord = make_coordinator();
    let obs = make_observer();
    coord
        .request_assistance(&obs, "target-1", AssistanceMode::ViewOnly, "help")
        .unwrap();
    let events = coord.drain_audit_events();
    assert!(!events.is_empty());
    // After drain, events should be empty.
    let events2 = coord.drain_audit_events();
    assert!(events2.is_empty());
}
