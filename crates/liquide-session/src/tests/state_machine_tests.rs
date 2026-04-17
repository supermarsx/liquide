use crate::SessionError;
use crate::state::{SessionState, SessionStateMachine};

fn make_state_machine(id: &str) -> SessionStateMachine {
    SessionStateMachine::new(id.to_string())
}

#[test]
fn test_state_machine_starts_in_created() {
    let sm = make_state_machine("sess-1");
    assert_eq!(sm.state(), SessionState::Created);
    assert_eq!(sm.session_id(), "sess-1");
    assert_eq!(sm.transition_count(), 0);
}

#[test]
fn test_state_display_variants() {
    assert_eq!(SessionState::Created.to_string(), "Created");
    assert_eq!(SessionState::Authenticating.to_string(), "Authenticating");
    assert_eq!(SessionState::Running.to_string(), "Running");
    assert_eq!(SessionState::Locked.to_string(), "Locked");
    assert_eq!(SessionState::Disconnected.to_string(), "Disconnected");
    assert_eq!(SessionState::Suspended.to_string(), "Suspended");
    assert_eq!(SessionState::Crashed.to_string(), "Crashed");
    assert_eq!(SessionState::Failed.to_string(), "Failed");
    assert_eq!(SessionState::Terminated.to_string(), "Terminated");
}

#[test]
fn test_valid_transitions_from_created() {
    let valid = SessionState::Created.valid_transitions();
    assert!(valid.contains(&SessionState::Authenticating));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Running));
}

#[test]
fn test_valid_transitions_from_authenticating() {
    let valid = SessionState::Authenticating.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Failed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Locked));
}

#[test]
fn test_valid_transitions_from_running() {
    let valid = SessionState::Running.valid_transitions();
    assert!(valid.contains(&SessionState::Locked));
    assert!(valid.contains(&SessionState::Disconnected));
    assert!(valid.contains(&SessionState::Suspended));
    assert!(valid.contains(&SessionState::Crashed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Authenticating));
}

#[test]
fn test_valid_transitions_from_locked() {
    let valid = SessionState::Locked.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Disconnected));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Suspended));
}

#[test]
fn test_valid_transitions_from_disconnected() {
    let valid = SessionState::Disconnected.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Suspended));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Locked));
}

#[test]
fn test_valid_transitions_from_suspended() {
    let valid = SessionState::Suspended.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Terminated));
    assert_eq!(valid.len(), 2);
}

#[test]
fn test_valid_transitions_from_crashed() {
    let valid = SessionState::Crashed.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Failed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Suspended));
}

#[test]
fn test_valid_transitions_from_failed() {
    let valid = SessionState::Failed.valid_transitions();
    assert_eq!(valid, &[SessionState::Terminated]);
}

#[test]
fn test_terminated_has_no_transitions() {
    let valid = SessionState::Terminated.valid_transitions();
    assert!(valid.is_empty());
}

#[test]
fn test_transition_created_to_authenticating() {
    let mut sm = make_state_machine("s1");
    assert!(sm.transition_to(SessionState::Authenticating).is_ok());
    assert_eq!(sm.state(), SessionState::Authenticating);
    assert_eq!(sm.transition_count(), 1);
}

#[test]
fn test_full_lifecycle_created_to_terminated() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Locked).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Disconnected).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Suspended).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Terminated).unwrap();
    assert_eq!(sm.state(), SessionState::Terminated);
    assert_eq!(sm.transition_count(), 9);
}

#[test]
fn test_crash_recovery_cycle() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Crashed).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    assert_eq!(sm.state(), SessionState::Running);
    assert_eq!(sm.transition_count(), 4);
}

#[test]
fn test_crash_to_failed_to_terminated() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Crashed).unwrap();
    sm.transition_to(SessionState::Failed).unwrap();
    sm.transition_to(SessionState::Terminated).unwrap();
    assert_eq!(sm.state(), SessionState::Terminated);
}

#[test]
fn test_invalid_transition_created_to_running() {
    let mut sm = make_state_machine("s1");
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    match result.unwrap_err() {
        SessionError::InvalidStateTransition { from, to } => {
            assert_eq!(from, "Created");
            assert_eq!(to, "Running");
        }
        other => panic!("expected InvalidStateTransition, got {:?}", other),
    }
    assert_eq!(sm.state(), SessionState::Created);
    assert_eq!(sm.transition_count(), 0);
}

#[test]
fn test_invalid_transition_terminated_to_running() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Terminated).unwrap();
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Terminated);
}

#[test]
fn test_invalid_transition_failed_to_running() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Failed).unwrap();
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Failed);
}

#[test]
fn test_invalid_transition_locked_to_suspended() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Locked).unwrap();
    let result = sm.transition_to(SessionState::Suspended);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Locked);
}

#[test]
fn test_invalid_transition_created_to_crashed() {
    let mut sm = make_state_machine("s1");
    let result = sm.transition_to(SessionState::Crashed);
    assert!(result.is_err());
}

#[test]
fn test_safe_mode_flag() {
    let mut sm = make_state_machine("s1");
    assert!(!sm.is_safe_mode());
    sm.set_safe_mode(true);
    assert!(sm.is_safe_mode());
    sm.set_safe_mode(false);
    assert!(!sm.is_safe_mode());
}

#[test]
fn test_valid_transitions_method_delegates() {
    let sm = make_state_machine("s1");
    assert_eq!(
        sm.valid_transitions(),
        SessionState::Created.valid_transitions()
    );
}

#[test]
fn test_uptime_and_last_transition_are_non_negative() {
    let sm = make_state_machine("s1");
    assert!(sm.uptime_seconds() < 5);
    assert!(sm.seconds_since_last_transition() < 5);
}

#[test]
fn test_invalid_transition_returns_error_with_state_names() {
    let mut sm = make_state_machine("err_test");
    // Created -> Running is invalid (must go through Authenticating first)
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Created"), "error should mention source state");
    assert!(msg.contains("Running"), "error should mention target state");
    // State should remain unchanged after failed transition
    assert_eq!(sm.state(), SessionState::Created);
    assert_eq!(sm.transition_count(), 0);
}
