use crate::mode::{AssistanceMode, ModeCapabilities, Restriction};

#[test]
fn test_view_only_capabilities() {
    let caps = AssistanceMode::ViewOnly.capabilities();
    assert!(caps.can_see_screen);
    assert!(caps.can_hear_audio);
    assert!(!caps.can_move_mouse);
    assert!(!caps.can_keyboard);
    assert_eq!(caps.max_concurrent_observers, 5);
}

#[test]
fn test_interactive_capabilities() {
    let caps = AssistanceMode::Interactive.capabilities();
    assert!(caps.can_see_screen);
    assert!(caps.can_move_mouse);
    assert!(caps.can_keyboard);
    assert!(caps.can_clipboard_read);
    assert!(caps.can_request_escalation);
    assert_eq!(caps.max_concurrent_observers, 2);
}

#[test]
fn test_exclusive_capabilities() {
    let caps = AssistanceMode::Exclusive.capabilities();
    assert!(caps.can_move_mouse);
    assert!(caps.can_keyboard);
    assert!(!caps.can_request_escalation);
    assert_eq!(caps.max_concurrent_observers, 1);
}

#[test]
fn test_stealth_capabilities() {
    let caps = AssistanceMode::Stealth.capabilities();
    assert!(caps.can_see_screen);
    assert!(!caps.can_hear_audio);
    assert!(!caps.can_move_mouse);
    assert!(!caps.status_indicator);
    assert_eq!(caps.max_concurrent_observers, 3);
}

#[test]
fn test_mode_display() {
    assert_eq!(AssistanceMode::ViewOnly.to_string(), "ViewOnly");
    assert_eq!(AssistanceMode::Interactive.to_string(), "Interactive");
    assert_eq!(AssistanceMode::Exclusive.to_string(), "Exclusive");
    assert_eq!(AssistanceMode::Stealth.to_string(), "Stealth");
}
