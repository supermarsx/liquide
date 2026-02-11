use crate::audit::{AssistanceAuditEvent, AuditLevel};

#[test]
fn test_requested_event() {
    let evt = AssistanceAuditEvent::Requested {
        observer_id: "obs-1".into(),
        target_session_id: "sess-1".into(),
        mode: "ViewOnly".into(),
    };
    assert_eq!(evt.event_name(), "requested");
    assert_eq!(evt.level(), AuditLevel::Info);
}

#[test]
fn test_consent_denied_event() {
    let evt = AssistanceAuditEvent::ConsentDenied {
        observer_id: "obs-1".into(),
        target_session_id: "sess-1".into(),
    };
    assert_eq!(evt.event_name(), "consent_denied");
    assert_eq!(evt.level(), AuditLevel::Warn);
}

#[test]
fn test_stealth_active_event() {
    let evt = AssistanceAuditEvent::StealthActive {
        observer_id: "sec-1".into(),
        target_session_id: "sess-1".into(),
        duration_seconds: 120,
    };
    assert_eq!(evt.event_name(), "stealth_active");
    assert_eq!(evt.level(), AuditLevel::Debug);
}

#[test]
fn test_all_event_names_unique() {
    let events: Vec<AssistanceAuditEvent> = vec![
        AssistanceAuditEvent::Requested { observer_id: String::new(), target_session_id: String::new(), mode: String::new() },
        AssistanceAuditEvent::ConsentGranted { observer_id: String::new(), target_session_id: String::new() },
        AssistanceAuditEvent::ConsentDenied { observer_id: String::new(), target_session_id: String::new() },
        AssistanceAuditEvent::ConsentTimeout { observer_id: String::new(), target_session_id: String::new() },
        AssistanceAuditEvent::Started { session_id: String::new(), observer_id: String::new(), mode: String::new() },
        AssistanceAuditEvent::Escalated { session_id: String::new(), new_mode: String::new() },
        AssistanceAuditEvent::OwnerReclaimed { session_id: String::new() },
        AssistanceAuditEvent::Ended { session_id: String::new(), reason: String::new() },
        AssistanceAuditEvent::StealthStarted { observer_id: String::new(), target_session_id: String::new() },
        AssistanceAuditEvent::StealthActive { observer_id: String::new(), target_session_id: String::new(), duration_seconds: 0 },
        AssistanceAuditEvent::StealthEnded { observer_id: String::new(), target_session_id: String::new(), duration_seconds: 0 },
        AssistanceAuditEvent::ChatMessageSent { session_id: String::new(), sender: String::new() },
        AssistanceAuditEvent::InviteCreated { code: String::new(), created_by: String::new() },
        AssistanceAuditEvent::InviteUsed { code: String::new(), used_by: String::new() },
    ];
    let names: Vec<&str> = events.iter().map(|e| e.event_name()).collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(names.len(), unique.len());
}
