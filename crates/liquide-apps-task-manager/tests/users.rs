//! Tests for `users` module types.

use liquide_apps_task_manager::users::*;

// ---------------------------------------------------------------------------
// SessionType
// ---------------------------------------------------------------------------

#[test]
fn session_type_all_variants() {
    let variants = [
        SessionType::Console,
        SessionType::RemoteDesktop,
        SessionType::Vnc,
        SessionType::Ssh,
        SessionType::Citrix,
        SessionType::Wayland,
        SessionType::X11,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn session_type_serde_roundtrip() {
    let val = SessionType::RemoteDesktop;
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
        LoginEventType::Login,
        LoginEventType::Logout,
        LoginEventType::Lock,
        LoginEventType::Unlock,
        LoginEventType::RemoteConnect,
        LoginEventType::RemoteDisconnect,
        LoginEventType::SessionSwitch,
    ];
    assert_eq!(variants.len(), 7);
}

// ---------------------------------------------------------------------------
// UserAction
// ---------------------------------------------------------------------------

#[test]
fn user_action_all_variants() {
    let variants = [
        UserAction::Disconnect,
        UserAction::Logoff,
        UserAction::SendMessage,
        UserAction::RemoteControl,
        UserAction::SwitchTo,
        UserAction::Lock,
    ];
    assert_eq!(variants.len(), 6);
}

// ---------------------------------------------------------------------------
// UserSession construction
// ---------------------------------------------------------------------------

#[test]
fn user_session_construction() {
    let session = UserSession {
        username: "alice".into(),
        session_id: 1,
        session_type: SessionType::Console,
        status: SessionStatus::Active,
        client_name: None,
        client_address: None,
        login_time: Some("2026-02-12T10:00:00Z".into()),
        idle_time_secs: 0,
        cpu_percent: 12.5,
        mem_bytes: 1024 * 1024 * 512,
        disk_bytes_sec: 0,
        network_bytes_sec: 0,
        process_count: 42,
    };
    assert_eq!(session.username, "alice");
    assert_eq!(session.session_type, SessionType::Console);
    assert_eq!(session.status, SessionStatus::Active);
    assert_eq!(session.process_count, 42);
}

#[test]
fn user_session_serde_roundtrip() {
    let session = UserSession {
        username: "bob".into(),
        session_id: 2,
        session_type: SessionType::Ssh,
        status: SessionStatus::Active,
        client_name: Some("workstation".into()),
        client_address: Some("192.168.1.100".into()),
        login_time: Some("2026-02-12T10:00:00Z".into()),
        idle_time_secs: 120,
        cpu_percent: 1.0,
        mem_bytes: 1024 * 1024,
        disk_bytes_sec: 0,
        network_bytes_sec: 500,
        process_count: 5,
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
        event_type: LoginEventType::Login,
        username: "alice".into(),
        session_id: 1,
        source_address: None,
        success: true,
    };
    assert!(event.success);
    assert_eq!(event.event_type, LoginEventType::Login);
}
