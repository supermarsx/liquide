use crate::audit::{AuditLevel, UsbAuditEvent};

#[test]
fn test_audit_event_forwarded() {
    let event = UsbAuditEvent::DeviceForwarded {
        user: "alice".to_string(),
        device_name: "USB Drive".to_string(),
        vid_pid: "046D:C534".to_string(),
        class: "Filesystem".to_string(),
        session_id: "sess-1".to_string(),
    };
    assert_eq!(event.level(), AuditLevel::Info);
    assert_eq!(event.event_name(), "device_forwarded");
}

#[test]
fn test_audit_event_blocked() {
    let event = UsbAuditEvent::DeviceBlocked {
        user: "bob".to_string(),
        device_name: "Unknown".to_string(),
        vid_pid: "DEAD:BEEF".to_string(),
        class: "RawUsb".to_string(),
        block_reason: "blocked by policy".to_string(),
    };
    assert_eq!(event.level(), AuditLevel::Warn);
    assert_eq!(event.event_name(), "device_blocked");
}

#[test]
fn test_audit_event_security_key() {
    let event = UsbAuditEvent::SecurityKeyForwardAttempt {
        user: "charlie".to_string(),
        device_name: "YubiKey".to_string(),
        vid_pid: "1050:0407".to_string(),
        allowed: false,
    };
    assert_eq!(event.level(), AuditLevel::Warn);
    assert_eq!(event.event_name(), "security_key_forward_attempt");
}

#[test]
fn test_audit_event_serialize() {
    let event = UsbAuditEvent::PolicyViolation {
        user: "dave".to_string(),
        device_name: "Printer".to_string(),
        vid_pid: "1234:5678".to_string(),
        policy_rule: "blocked_class".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let decoded: UsbAuditEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.event_name(), "policy_violation");
}

#[test]
fn test_audit_event_disconnected() {
    let event = UsbAuditEvent::DeviceDisconnected {
        user: "eve".to_string(),
        device_name: "Flash Drive".to_string(),
        vid_pid: "0001:0002".to_string(),
        reason: "user initiated".to_string(),
    };
    assert_eq!(event.level(), AuditLevel::Info);
    assert_eq!(event.event_name(), "device_disconnected");
}
