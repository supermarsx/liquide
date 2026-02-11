use crate::config::{AssistanceConfig, ModeConfig, StealthConfig, PermissionsConfig, RecordingConfig};

#[test]
fn test_assistance_config_defaults() {
    let cfg = AssistanceConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.max_concurrent_observers, 5);
    assert_eq!(cfg.invitation_expiry_seconds, 300);
    assert_eq!(cfg.consent_timeout_seconds, 60);
}

#[test]
fn test_mode_config_defaults() {
    let cfg = ModeConfig::default();
    assert!(cfg.view_only);
    assert!(cfg.interactive);
    assert!(cfg.exclusive);
    assert!(!cfg.stealth);
}

#[test]
fn test_stealth_config_defaults() {
    let cfg = StealthConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.required_role, "security_admin");
    assert_eq!(cfg.audit_interval_seconds, 1);
    assert_eq!(cfg.max_duration_minutes, 60);
}

#[test]
fn test_permissions_config_defaults() {
    let cfg = PermissionsConfig::default();
    assert!(cfg.helpdesk_can_request);
    assert!(!cfg.admin_can_force);
    assert!(cfg.user_can_invite);
}

#[test]
fn test_recording_config_defaults() {
    let cfg = RecordingConfig::default();
    assert!(cfg.auto_record);
    assert!(cfg.include_chat);
}
