use crate::consent::{ConsentFlow, ConsentState};
use crate::mode::{AssistanceMode, Restriction};

fn make_flow() -> ConsentFlow {
    ConsentFlow::new(
        "obs-1".into(),
        "Alice".into(),
        "Admin".into(),
        AssistanceMode::Interactive,
        "need help".into(),
        60,
    )
}

#[test]
fn test_consent_initial_state() {
    let flow = make_flow();
    assert_eq!(*flow.state(), ConsentState::Pending);
}

#[test]
fn test_consent_prompt() {
    let mut flow = make_flow();
    let prompt = flow.prompt(100).unwrap();
    assert_eq!(prompt.observer_name, "Alice");
    assert_eq!(prompt.timeout_seconds, 60);
    assert!(matches!(*flow.state(), ConsentState::AwaitingResponse { .. }));
}

#[test]
fn test_consent_approve() {
    let mut flow = make_flow();
    flow.prompt(100).unwrap();
    let state = flow.respond(true, vec![Restriction::NoAudio]).unwrap();
    assert!(matches!(state, ConsentState::Approved { .. }));
}

#[test]
fn test_consent_deny() {
    let mut flow = make_flow();
    flow.prompt(100).unwrap();
    let state = flow.respond(false, vec![]).unwrap();
    assert_eq!(state, ConsentState::Denied);
}

#[test]
fn test_consent_timeout() {
    let mut flow = make_flow();
    flow.prompt(100).unwrap();
    assert!(!flow.check_timeout(150));
    assert!(flow.check_timeout(200));
    assert_eq!(*flow.state(), ConsentState::TimedOut);
}
