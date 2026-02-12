//! Tests for `users` module types.

use liquide_apps_task_manager::users::*;

// ---------------------------------------------------------------------------
// SessionType
// ---------------------------------------------------------------------------

#[test]
fn session_type_all_variants() {
    let variants = [
        SessionType::Console,
        SessionType::Rdp,
        SessionType::Vnc,
        SessionType::Ssh,
        SessionType::Virtual,
        SessionType::Service,
        SessionType::Other,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn session_type_display() {
    assert_eq!(SessionType::Console.as_str(), "Console");
    assert_eq!(SessionType::Rdp.as_str(), "RDP");
    assert_eq!(SessionType::Vnc.as_str(), "VNC");
    assert_eq!(SessionType::Ssh.as_str(), "SSH");
    assert_eq!(SessionType::Virtual.as_str(), "Virtual");
    assert_eq!(SessionType::Service.as_str(), "Service");
    assert_eq!(SessionType::Other.as_str(), "Other");
}

#[test]
fn session_type_serde_roundtrip() {
    let val = SessionType::Rdp;
    let json = serde_json::to_string(&val).unwrap();
    let back: SessionType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// SessionStatus
// ---------------------------------------------------------------------------

#[test]
fn session_status_all_variants() {
    let variants = [
        SessionStatus::Active,
        SessionStatus::Disconnected,
        SessionStatus::Locked,
        SessionStatus::Idle,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn session_status_display() {
    assert_eq!(SessionStatus::Active.as_str(), "Active");
    assert_eq!(SessionStatus::Disconnected.as_str(), "Disconnected");
    assert_eq!(SessionStatus::Locked.as_str(), "Locked");
    assert_eq!(SessionStatus::Idle.as_str(), "Idle");
}

#[test]
fn session_status_serde_roundtrip() {
    let val = SessionStatus::Locked;
    let json = serde_json::to_string(&val).unwrap();
    let back: SessionStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// LoginEventType
// ---------------------------------------------------------------------------

#[test]
fn login_event_type_all_variants() {
    let variants = [
        LoginEventType::Interactive,
        LoginEventType::Remote,
        LoginEventType::Unlock,
        LoginEventType::Service,
        LoginEventType::Batch,
        LoginEventType::NetworkCleartext,
        LoginEventType::RunAs,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn login_event_type_display() {
    assert_eq!(LoginEventType::Interactive.as_str(), "Interactive");
    assert_eq!(LoginEventType::Remote.as_str(), "Remote");
    assert_eq!(LoginEventType::Unlock.as_str(), "Unlock");
    assert_eq!(LoginEventType::RunAs.as_str(), "RunAs");
}

// ---------------------------------------------------------------------------
// UserAction
// ---------------------------------------------------------------------------

#[test]
fn user_action_all_variants() {
    let variants = [
        UserAction::Logoff,
        UserAction::Disconnect,
        UserAction::SendMessage,
        UserAction::RemoteControl,
        UserAction::ResetSession,
        UserAction::SwitchTo,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn user_action_display() {
    assert_eq!(UserAction::Logoff.as_str(), "Log Off");
    assert_eq!(UserAction::Disconnect.as_str(), "Disconnect");
    assert_eq!(UserAction::SendMessage.as_str(), "Send Message");
    assert_eq!(UserAction::SwitchTo.as_str(), "Switch To");
}

// ---------------------------------------------------------------------------
// UserSession construction
// ---------------------------------------------------------------------------

#[test]
fn user_session_construction() {
    let session = UserSession {
        session_id: 1,
        username: "alice".into(),
        domain: Some("CORP".into()),
        session_type: SessionType::Console,
        status: SessionStatus::Active,
        login_time: "2026-02-12T10:00:00Z".into(),
        idle_time_secs: 0,
        process_count: 42,
        cpu_percent: 12.5,
        memory_bytes: 1024 * 1024 * 512,
        client_name: None,
        client_address: None,
        display_resolution: Some("1920x1080".into()),
    };
    assert_eq!(session.username, "alice");
    assert_eq!(session.session_type, SessionType::Console);
    assert_eq!(session.status, SessionStatus::Active);
    assert_eq!(session.process_count, 42);
}

#[test]
fn user_session_serde_roundtrip() {
    let session = UserSession {
        session_id: 2,
        username: "bob".into(),
        domain: None,
        session_type: SessionType::Ssh,
        status: SessionStatus::Active,
        login_time: "2026-02-12T10:00:00Z".into(),
        idle_time_secs: 120,
        process_count: 5,
        cpu_percent: 1.0,
        memory_bytes: 1024 * 1024,
        client_name: Some("workstation".into()),
        client_address: Some("192.168.1.100".into()),
        display_resolution: None,
    };
    let json = serde_json::to_string(&session).unwrap();
    let back: UserSession = serde_json::from_str(&json).unwrap();
    assert_eq!(back.username, "bob");
    assert_eq!(back.session_type, SessionType::Ssh);
}

// ---------------------------------------------------------------------------
// LoginEvent
// ---------------------------------------------------------------------------

#[test]
fn login_event_construction() {
    let event = LoginEvent {
        timestamp: "2026-02-12T10:00:00Z".into(),
        username: "alice".into(),
        event_type: LoginEventType::Interactive,
        success: true,
        source_address: None,
        session_id: Some(1),
        failure_reason: None,
    };
    assert!(event.success);
    assert_eq!(event.event_type, LoginEventType::Interactive);
}
