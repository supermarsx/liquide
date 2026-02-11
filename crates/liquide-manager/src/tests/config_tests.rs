//! Tests for config types and admin role permissions.

use crate::config::{AdminRole, AuthConfig, AuthMode, ManagerConfig, MetricsConfig, TlsConfig, UiConfig};

// ===========================================================================
// AdminRole
// ===========================================================================

#[test]
fn test_role_ordering() {
    assert!(AdminRole::Viewer < AdminRole::Operator);
    assert!(AdminRole::Operator < AdminRole::Admin);
    assert!(AdminRole::Admin < AdminRole::SuperAdmin);
}

#[test]
fn test_has_permission_same_level() {
    assert!(AdminRole::Viewer.has_permission(AdminRole::Viewer));
    assert!(AdminRole::Admin.has_permission(AdminRole::Admin));
}

#[test]
fn test_has_permission_higher_level() {
    assert!(AdminRole::SuperAdmin.has_permission(AdminRole::Viewer));
    assert!(AdminRole::Admin.has_permission(AdminRole::Operator));
}

#[test]
fn test_lacks_permission() {
    assert!(!AdminRole::Viewer.has_permission(AdminRole::Admin));
    assert!(!AdminRole::Operator.has_permission(AdminRole::SuperAdmin));
}

#[test]
fn test_role_display() {
    assert_eq!(AdminRole::Viewer.to_string(), "viewer");
    assert_eq!(AdminRole::Operator.to_string(), "operator");
    assert_eq!(AdminRole::Admin.to_string(), "admin");
    assert_eq!(AdminRole::SuperAdmin.to_string(), "super-admin");
}

// ===========================================================================
// AuthMode
// ===========================================================================

#[test]
fn test_auth_mode_display() {
    assert_eq!(AuthMode::Local.to_string(), "local");
    assert_eq!(AuthMode::Oidc.to_string(), "oidc");
    assert_eq!(AuthMode::Mtls.to_string(), "mtls");
}

// ===========================================================================
// Default configurations
// ===========================================================================

#[test]
fn test_manager_config_defaults() {
    let cfg = ManagerConfig::default();
    assert_eq!(cfg.listen_addr, "127.0.0.1:8443");
    assert!(cfg.servers.is_empty());
    assert!(cfg.gateways.is_empty());
}

#[test]
fn test_tls_config_defaults() {
    let tls = TlsConfig::default();
    assert!(tls.enabled);
    assert!(tls.auto_generate_self_signed);
}

#[test]
fn test_auth_config_defaults() {
    let auth = AuthConfig::default();
    assert_eq!(auth.mode, AuthMode::Local);
    assert_eq!(auth.max_login_attempts, 5);
    assert_eq!(auth.lockout_duration_min, 15);
    assert_eq!(auth.session_timeout_min, 30);
}

#[test]
fn test_metrics_config_defaults() {
    let m = MetricsConfig::default();
    assert_eq!(m.retention_hours, 24);
    assert_eq!(m.polling_interval_sec, 5);
    assert!(m.external_tsdb_url.is_empty());
}

#[test]
fn test_ui_config_defaults() {
    let ui = UiConfig::default();
    assert_eq!(ui.theme, "liquid-glass");
    assert_eq!(ui.items_per_page, 25);
    assert_eq!(ui.auto_refresh_sec, 5);
}

// ===========================================================================
// Serialization round-trips
// ===========================================================================

#[test]
fn test_config_serde_roundtrip() {
    let cfg = ManagerConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let recovered: ManagerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.listen_addr, cfg.listen_addr);
    assert_eq!(recovered.auth.mode, cfg.auth.mode);
}

#[test]
fn test_admin_role_serde_roundtrip() {
    let role = AdminRole::SuperAdmin;
    let json = serde_json::to_string(&role).unwrap();
    let recovered: AdminRole = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, role);
}
