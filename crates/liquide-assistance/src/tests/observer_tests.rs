use crate::mode::AssistanceMode;
use crate::observer::{Observer, ObserverRole};

#[test]
fn test_observer_new() {
    let obs = Observer::new("o1".into(), "Alice".into(), ObserverRole::Admin, AssistanceMode::Interactive);
    assert_eq!(obs.id, "o1");
    assert_eq!(obs.name, "Alice");
    assert_eq!(obs.role, ObserverRole::Admin);
    assert!(!obs.has_input_control);
}

#[test]
fn test_security_admin_can_stealth() {
    let obs = Observer::new("o1".into(), "Bob".into(), ObserverRole::SecurityAdmin, AssistanceMode::ViewOnly);
    assert!(obs.can_stealth());
}

#[test]
fn test_non_security_cannot_stealth() {
    let obs = Observer::new("o1".into(), "Carol".into(), ObserverRole::HelpDesk, AssistanceMode::ViewOnly);
    assert!(!obs.can_stealth());
}

#[test]
fn test_escalation_permissions() {
    let admin = Observer::new("o1".into(), "Admin".into(), ObserverRole::Admin, AssistanceMode::ViewOnly);
    assert!(admin.can_escalate_to(AssistanceMode::ViewOnly));
    assert!(admin.can_escalate_to(AssistanceMode::Interactive));
    assert!(admin.can_escalate_to(AssistanceMode::Exclusive));
    assert!(!admin.can_escalate_to(AssistanceMode::Stealth));
}

#[test]
fn test_observer_role_display() {
    assert_eq!(ObserverRole::HelpDesk.to_string(), "HelpDesk");
    assert_eq!(ObserverRole::SecurityAdmin.to_string(), "SecurityAdmin");
}
