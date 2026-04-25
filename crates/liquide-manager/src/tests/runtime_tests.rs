//! Tests for the ManagerRuntime coordinator.

use crate::audit::{AuditLevel, ManagerAuditEvent};
use crate::config::{AdminRole, GatewayEntry, ManagerConfig, ServerEntry};
use crate::metrics::MetricsSnapshot;
use crate::policy_mgmt::{PolicyEntry, PolicyScope};
use crate::runtime::ManagerRuntime;
use crate::server_mgmt::ServerStatus;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_config() -> ManagerConfig {
    ManagerConfig {
        servers: vec![
            ServerEntry {
                name: "srv-a".into(),
                address: "10.0.0.1:9000".into(),
                api_key: "key-a".into(),
            },
            ServerEntry {
                name: "srv-b".into(),
                address: "10.0.0.2:9000".into(),
                api_key: "key-b".into(),
            },
        ],
        gateways: vec![GatewayEntry {
            name: "gw-1".into(),
            address: "10.0.1.1:443".into(),
            api_key: "gw-key".into(),
        }],
        ..ManagerConfig::default()
    }
}

fn make_runtime() -> ManagerRuntime {
    ManagerRuntime::new(make_config())
}

fn entry(key: &str, val: &str) -> PolicyEntry {
    PolicyEntry {
        key: key.to_string(),
        value: val.to_string(),
        scope: PolicyScope::Default,
        target: String::new(),
    }
}

// ===========================================================================
// Construction
// ===========================================================================

#[test]
fn test_runtime_creates_from_config() {
    let rt = make_runtime();
    assert_eq!(rt.servers().count(), 2);
    assert_eq!(rt.gateways().count(), 1);
    assert_eq!(rt.sessions().count(), 0);
    assert_eq!(rt.policies().current_version(), 0);
}

#[test]
fn test_runtime_has_default_admin() {
    let rt = make_runtime();
    let admin = rt.admins().get("admin").unwrap();
    assert_eq!(admin.role, AdminRole::SuperAdmin);
}

// ===========================================================================
// Dashboard
// ===========================================================================

#[test]
fn test_dashboard_with_no_data() {
    let rt = make_runtime();
    let dash = rt.dashboard(1000);
    // All servers start offline
    assert_eq!(dash.servers_offline, 2);
    assert_eq!(dash.servers_healthy, 0);
    assert_eq!(dash.gateways_offline, 1);
}

#[test]
fn test_dashboard_with_online_servers() {
    let mut rt = make_runtime();
    rt.update_server("srv-a", ServerStatus::Online, 5, 40.0, 60.0, 3600, 1000);
    rt.update_server("srv-b", ServerStatus::Unhealthy, 2, 90.0, 80.0, 1800, 1000);
    let dash = rt.dashboard(1000);
    assert_eq!(dash.servers_healthy, 1);
    assert_eq!(dash.servers_unhealthy, 1);
    assert_eq!(dash.total_sessions, 7);
}

// ===========================================================================
// Server management
// ===========================================================================

#[test]
fn test_drain_server() {
    let mut rt = make_runtime();
    rt.drain_server("srv-a", "admin").unwrap();
    let srv = rt.servers().get("srv-a").unwrap();
    assert_eq!(srv.status, ServerStatus::Draining);
    let events = rt.drain_audit_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ManagerAuditEvent::ServerDrained { .. }));
}

#[test]
fn test_drain_unknown_server() {
    let mut rt = make_runtime();
    let result = rt.drain_server("unknown", "admin");
    assert!(result.is_err());
}

#[test]
fn test_restart_server() {
    let mut rt = make_runtime();
    rt.restart_server("srv-a", "admin").unwrap();
    let events = rt.drain_audit_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        ManagerAuditEvent::ServerRestarted { .. }
    ));
}

#[test]
fn test_restart_unknown_server() {
    let mut rt = make_runtime();
    let result = rt.restart_server("unknown", "admin");
    assert!(result.is_err());
}

// ===========================================================================
// Session management
// ===========================================================================

#[test]
fn test_register_and_disconnect_session() {
    let mut rt = make_runtime();
    rt.register_session("s1".into(), "alice".into(), "srv-a".into(), 100);
    assert_eq!(rt.sessions().count(), 1);

    rt.disconnect_session("s1", "admin").unwrap();
    assert_eq!(rt.sessions().count(), 0);
    let events = rt.drain_audit_events();
    assert!(matches!(
        events[0],
        ManagerAuditEvent::SessionDisconnected { .. }
    ));
}

#[test]
fn test_disconnect_unknown_session() {
    let mut rt = make_runtime();
    let result = rt.disconnect_session("unknown", "admin");
    assert!(result.is_err());
}

#[test]
fn test_lock_and_unlock_session() {
    let mut rt = make_runtime();
    rt.register_session("s1".into(), "alice".into(), "srv-a".into(), 100);

    rt.lock_session("s1", "admin", Some("maint".into()))
        .unwrap();
    let events = rt.drain_audit_events();
    assert!(matches!(events[0], ManagerAuditEvent::SessionLocked { .. }));

    rt.unlock_session("s1", "admin").unwrap();
    let events = rt.drain_audit_events();
    assert!(matches!(
        events[0],
        ManagerAuditEvent::SessionUnlocked { .. }
    ));
}

// ===========================================================================
// Policy management
// ===========================================================================

#[test]
fn test_update_policies() {
    let mut rt = make_runtime();
    let v = rt.update_policies(
        vec![entry("clipboard.enabled", "true")],
        "admin",
        "initial".into(),
        1000,
    );
    assert_eq!(v, 1);
    assert_eq!(rt.policies().current_version(), 1);
    let events = rt.drain_audit_events();
    assert!(matches!(
        events[0],
        ManagerAuditEvent::PolicyUpdated { version: 1, .. }
    ));
}

#[test]
fn test_rollback_policy() {
    let mut rt = make_runtime();
    rt.update_policies(vec![entry("a", "1")], "admin", "v1".into(), 100);
    rt.update_policies(vec![entry("a", "2")], "admin", "v2".into(), 200);
    rt.drain_audit_events(); // clear

    let v = rt.rollback_policy(1, "admin", 300).unwrap();
    assert_eq!(v, 3);
    let events = rt.drain_audit_events();
    assert!(matches!(
        events[0],
        ManagerAuditEvent::PolicyRolledBack {
            from_version: 2,
            to_version: 1,
            ..
        }
    ));
}

#[test]
fn test_rollback_unknown_version() {
    let mut rt = make_runtime();
    let result = rt.rollback_policy(99, "admin", 100);
    assert!(result.is_err());
}

// ===========================================================================
// Authentication
// ===========================================================================

#[test]
fn test_login_success() {
    let mut rt = make_runtime();
    let role = rt.login("admin", "127.0.0.1", 1000).unwrap();
    assert_eq!(role, AdminRole::SuperAdmin);
    let events = rt.drain_audit_events();
    assert!(matches!(events[0], ManagerAuditEvent::AdminLogin { .. }));
}

#[test]
fn test_login_failure() {
    let mut rt = make_runtime();
    let result = rt.login("nobody", "127.0.0.1", 1000);
    assert!(result.is_err());
    let events = rt.drain_audit_events();
    assert!(matches!(events[0], ManagerAuditEvent::LoginFailed { .. }));
}

// ===========================================================================
// Metrics
// ===========================================================================

#[test]
fn test_record_metrics() {
    let mut rt = make_runtime();
    rt.record_metrics(MetricsSnapshot {
        total_sessions: 10,
        total_users: 5,
        avg_fps: 60.0,
        avg_latency_ms: 4.0,
        total_bandwidth_in_bps: 1_000_000,
        total_bandwidth_out_bps: 500_000,
        timestamp: 1000,
    });
    let latest = rt.metrics().latest_snapshot().unwrap();
    assert_eq!(latest.total_sessions, 10);
}

// ===========================================================================
// Audit events
// ===========================================================================

#[test]
fn test_drain_audit_events() {
    let mut rt = make_runtime();
    rt.drain_server("srv-a", "admin").unwrap();
    rt.restart_server("srv-b", "admin").unwrap();
    let events = rt.drain_audit_events();
    assert_eq!(events.len(), 2);
    // Second drain should be empty.
    let events2 = rt.drain_audit_events();
    assert!(events2.is_empty());
}

#[test]
fn test_audit_event_levels() {
    assert_eq!(
        ManagerAuditEvent::AdminLogin {
            username: "a".into(),
            ip: "1".into(),
        }
        .level(),
        AuditLevel::Info
    );
    assert_eq!(
        ManagerAuditEvent::LoginFailed {
            username: "a".into(),
            ip: "1".into(),
            reason: "bad".into(),
        }
        .level(),
        AuditLevel::Warning
    );
    assert_eq!(
        ManagerAuditEvent::ServerDrained {
            server: "s".into(),
            admin: "a".into(),
        }
        .level(),
        AuditLevel::Warning
    );
}

#[test]
fn test_audit_event_names() {
    let event = ManagerAuditEvent::PolicyUpdated {
        admin: "a".into(),
        version: 1,
    };
    assert_eq!(event.event_name(), "policy_updated");

    let event = ManagerAuditEvent::SessionDisconnected {
        session_id: "s".into(),
        admin: "a".into(),
    };
    assert_eq!(event.event_name(), "session_disconnected");
}

// ===========================================================================
// Accessors
// ===========================================================================

#[test]
fn test_config_accessor() {
    let rt = make_runtime();
    assert_eq!(rt.config().servers.len(), 2);
    assert_eq!(rt.config().gateways.len(), 1);
}
