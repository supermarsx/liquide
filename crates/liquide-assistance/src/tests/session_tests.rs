use crate::mode::AssistanceMode;
use crate::session::{ShadowSession, ShadowSessionState, SessionInfo};

#[test]
fn test_new_session_is_pending() {
    let s = ShadowSession::new("s1".into(), "target-1".into(), AssistanceMode::ViewOnly);
    assert_eq!(s.state(), ShadowSessionState::Pending);
    assert_eq!(s.id(), "s1");
    assert_eq!(s.target_session_id(), "target-1");
    assert!(!s.is_active());
}

#[test]
fn test_add_observer_activates() {
    let mut s = ShadowSession::new("s1".into(), "t1".into(), AssistanceMode::Interactive);
    s.add_observer("obs-1".into());
    assert_eq!(s.state(), ShadowSessionState::Active);
    assert!(s.is_active());
    assert_eq!(s.observers().len(), 1);
}

#[test]
fn test_remove_observer() {
    let mut s = ShadowSession::new("s1".into(), "t1".into(), AssistanceMode::ViewOnly);
    s.add_observer("obs-1".into());
    s.add_observer("obs-2".into());
    assert_eq!(s.observers().len(), 2);
    s.remove_observer("obs-1");
    assert_eq!(s.observers().len(), 1);
    assert_eq!(s.observers()[0], "obs-2");
}

#[test]
fn test_end_session() {
    let mut s = ShadowSession::new("s1".into(), "t1".into(), AssistanceMode::ViewOnly);
    s.add_observer("obs-1".into());
    s.end(1000);
    assert_eq!(s.state(), ShadowSessionState::Ended);
    assert_eq!(s.ended_at(), Some(1000));
    assert!(!s.is_active());
}

#[test]
fn test_session_info_from_session() {
    let mut s = ShadowSession::new("s1".into(), "t1".into(), AssistanceMode::Exclusive);
    s.add_observer("obs-1".into());
    let info = SessionInfo::from_session(&s);
    assert_eq!(info.id, "s1");
    assert_eq!(info.state, "Active");
    assert_eq!(info.observer_count, 1);
}
