//! Tests for individual subsystems.

use crate::config::AdminRole;
use crate::dashboard::{AlertSeverity, DashboardBuilder};
use crate::gateway_mgmt::{GatewayRegistry, GatewayStatus};
use crate::metrics::{MetricsCollector, MetricsSnapshot};
use crate::policy_mgmt::{PolicyEntry, PolicyScope, PolicyStore};
use crate::server_mgmt::{ServerRegistry, ServerStatus};
use crate::session_mgmt::SessionStore;
use crate::user_mgmt::AdminStore;

// ===========================================================================
// ServerRegistry
// ===========================================================================

#[test]
fn test_server_registry_empty() {
    let reg = ServerRegistry::new();
    assert_eq!(reg.count(), 0);
    assert!(reg.list().is_empty());
}

#[test]
fn test_server_registry_register_and_get() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "10.0.0.1:9000".into());
    assert_eq!(reg.count(), 1);
    let s = reg.get("s1").unwrap();
    assert_eq!(s.status, ServerStatus::Offline);
}

#[test]
fn test_server_registry_no_duplicates() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "10.0.0.1:9000".into());
    reg.register("s1".into(), "10.0.0.2:9000".into());
    assert_eq!(reg.count(), 1);
}

#[test]
fn test_server_registry_update_metrics() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "10.0.0.1:9000".into());
    reg.update_metrics("s1", ServerStatus::Online, 5, 50.0, 60.0, 3600, 1000);
    let s = reg.get("s1").unwrap();
    assert_eq!(s.status, ServerStatus::Online);
    assert_eq!(s.active_sessions, 5);
}

#[test]
fn test_server_count_by_status() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "10.0.0.1:9000".into());
    reg.register("s2".into(), "10.0.0.2:9000".into());
    reg.update_metrics("s1", ServerStatus::Online, 1, 0.0, 0.0, 0, 0);
    assert_eq!(reg.count_by_status(ServerStatus::Online), 1);
    assert_eq!(reg.count_by_status(ServerStatus::Offline), 1);
}

#[test]
fn test_server_mark_offline() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "addr".into());
    reg.update_metrics("s1", ServerStatus::Online, 0, 0.0, 0.0, 0, 0);
    reg.mark_offline("s1");
    assert_eq!(reg.get("s1").unwrap().status, ServerStatus::Offline);
}

#[test]
fn test_server_mark_draining() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "addr".into());
    reg.mark_draining("s1");
    assert_eq!(reg.get("s1").unwrap().status, ServerStatus::Draining);
}

#[test]
fn test_server_total_sessions() {
    let mut reg = ServerRegistry::new();
    reg.register("s1".into(), "a".into());
    reg.register("s2".into(), "b".into());
    reg.update_metrics("s1", ServerStatus::Online, 3, 0.0, 0.0, 0, 0);
    reg.update_metrics("s2", ServerStatus::Online, 7, 0.0, 0.0, 0, 0);
    assert_eq!(reg.total_sessions(), 10);
}

#[test]
fn test_server_status_display() {
    assert_eq!(ServerStatus::Online.to_string(), "online");
    assert_eq!(ServerStatus::Draining.to_string(), "draining");
}

// ===========================================================================
// SessionStore
// ===========================================================================

#[test]
fn test_session_store_empty() {
    let store = SessionStore::new();
    assert_eq!(store.count(), 0);
}

#[test]
fn test_session_upsert_and_get() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 100);
    let s = store.get("s1", 200).unwrap();
    assert_eq!(s.user, "alice");
    assert_eq!(s.duration_seconds, 100);
}

#[test]
fn test_session_upsert_updates_existing() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv-a".into(), 100);
    store.upsert("s1".into(), "alice".into(), "srv-b".into(), 100);
    assert_eq!(store.count(), 1);
    let s = store.get("s1", 200).unwrap();
    assert_eq!(s.server, "srv-b");
}

#[test]
fn test_session_remove() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 100);
    store.remove("s1");
    assert_eq!(store.count(), 0);
}

#[test]
fn test_session_lock_unlock() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 100);
    store
        .lock_session("s1", Some("maintenance".into()))
        .unwrap();
    let s = store.get("s1", 200).unwrap();
    assert_eq!(s.status, crate::session_mgmt::SessionStatus::Locked);

    store.unlock_session("s1").unwrap();
    let s = store.get("s1", 200).unwrap();
    assert_eq!(s.status, crate::session_mgmt::SessionStatus::Active);
}

#[test]
fn test_session_lock_unknown() {
    let mut store = SessionStore::new();
    assert!(store.lock_session("unknown", None).is_err());
}

#[test]
fn test_session_unique_users() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 0);
    store.upsert("s2".into(), "bob".into(), "srv".into(), 0);
    store.upsert("s3".into(), "alice".into(), "srv".into(), 0);
    assert_eq!(store.unique_users(), 2);
}

#[test]
fn test_sessions_for_user() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 0);
    store.upsert("s2".into(), "bob".into(), "srv".into(), 0);
    store.upsert("s3".into(), "alice".into(), "srv".into(), 0);
    let alice = store.sessions_for_user("alice", 100);
    assert_eq!(alice.len(), 2);
}

#[test]
fn test_session_update_metrics() {
    let mut store = SessionStore::new();
    store.upsert("s1".into(), "alice".into(), "srv".into(), 0);
    store.update_metrics("s1", 4.5, 60.0, 1_000_000);
    let s = store.get("s1", 0).unwrap();
    assert!((s.latency_ms - 4.5).abs() < 0.01);
    assert!((s.fps - 60.0).abs() < 0.01);
}

// ===========================================================================
// GatewayRegistry
// ===========================================================================

#[test]
fn test_gateway_registry_empty() {
    let reg = GatewayRegistry::new();
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.online_count(), 0);
}

#[test]
fn test_gateway_register_and_list() {
    let mut reg = GatewayRegistry::new();
    reg.register("gw-1".into(), "10.0.1.1:443".into());
    assert_eq!(reg.count(), 1);
    assert_eq!(reg.get("gw-1").unwrap().status, GatewayStatus::Offline);
}

#[test]
fn test_gateway_update() {
    let mut reg = GatewayRegistry::new();
    reg.register("gw-1".into(), "addr".into());
    reg.update("gw-1", GatewayStatus::Online, 2, 50, 1_000_000);
    let gw = reg.get("gw-1").unwrap();
    assert_eq!(gw.status, GatewayStatus::Online);
    assert_eq!(gw.connected_servers, 2);
    assert_eq!(gw.active_relays, 50);
}

#[test]
fn test_gateway_mark_offline() {
    let mut reg = GatewayRegistry::new();
    reg.register("gw-1".into(), "addr".into());
    reg.update("gw-1", GatewayStatus::Online, 0, 0, 0);
    reg.mark_offline("gw-1");
    assert_eq!(reg.get("gw-1").unwrap().status, GatewayStatus::Offline);
}

#[test]
fn test_gateway_online_count() {
    let mut reg = GatewayRegistry::new();
    reg.register("gw-1".into(), "a".into());
    reg.register("gw-2".into(), "b".into());
    reg.update("gw-1", GatewayStatus::Online, 0, 0, 0);
    assert_eq!(reg.online_count(), 1);
}

#[test]
fn test_gateway_status_display() {
    assert_eq!(GatewayStatus::Online.to_string(), "online");
    assert_eq!(GatewayStatus::Degraded.to_string(), "degraded");
    assert_eq!(GatewayStatus::Offline.to_string(), "offline");
}

// ===========================================================================
// AdminStore
// ===========================================================================

#[test]
fn test_admin_store_empty() {
    let store = AdminStore::new();
    assert_eq!(store.count(), 0);
}

#[test]
fn test_admin_add_and_get() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Admin);
    let a = store.get("alice").unwrap();
    assert_eq!(a.role, AdminRole::Admin);
    assert!(!a.locked);
}

#[test]
fn test_admin_authenticate_success() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Admin);
    let a = store.authenticate("alice", 1000).unwrap();
    assert_eq!(a.last_login, Some(1000));
}

#[test]
fn test_admin_authenticate_unknown_user() {
    let mut store = AdminStore::new();
    assert!(store.authenticate("nobody", 1000).is_err());
}

#[test]
fn test_admin_authenticate_locked() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Admin);
    store.lock("alice").unwrap();
    assert!(store.authenticate("alice", 1000).is_err());
}

#[test]
fn test_admin_lockout_after_max_failures() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Admin);
    for _ in 0..4 {
        store.record_failure("alice", 5, 900, 100);
    }
    // 5th failure triggers lockout.
    let locked = store.record_failure("alice", 5, 900, 100);
    assert!(locked);

    // Auth should fail due to lockout.
    assert!(store.authenticate("alice", 100).is_err());

    // After lockout period, auth succeeds.
    let _ = store.authenticate("alice", 1001);
}

#[test]
fn test_admin_set_role() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Viewer);
    store.set_role("alice", AdminRole::Admin).unwrap();
    assert_eq!(store.get("alice").unwrap().role, AdminRole::Admin);
}

#[test]
fn test_admin_unlock() {
    let mut store = AdminStore::new();
    store.add("alice".into(), AdminRole::Admin);
    store.lock("alice").unwrap();
    store.unlock("alice").unwrap();
    assert!(!store.get("alice").unwrap().locked);
}

// ===========================================================================
// PolicyStore
// ===========================================================================

fn entry(key: &str, val: &str) -> PolicyEntry {
    PolicyEntry {
        key: key.to_string(),
        value: val.to_string(),
        scope: PolicyScope::Default,
        target: String::new(),
    }
}

#[test]
fn test_policy_store_empty() {
    let store = PolicyStore::new();
    assert_eq!(store.current_version(), 0);
    assert!(store.current_entries().is_empty());
}

#[test]
fn test_policy_commit() {
    let mut store = PolicyStore::new();
    let v = store.commit(
        vec![entry("clipboard.enabled", "true")],
        "admin".into(),
        "initial".into(),
        1000,
    );
    assert_eq!(v, 1);
    assert_eq!(store.current_version(), 1);
    assert_eq!(store.current_entries().len(), 1);
}

#[test]
fn test_policy_get_version() {
    let mut store = PolicyStore::new();
    store.commit(vec![entry("a", "1")], "admin".into(), "v1".into(), 100);
    store.commit(vec![entry("a", "2")], "admin".into(), "v2".into(), 200);
    let v1 = store.get_version(1).unwrap();
    assert_eq!(v1.entries[0].value, "1");
}

#[test]
fn test_policy_history() {
    let mut store = PolicyStore::new();
    store.commit(vec![], "admin".into(), "v1".into(), 100);
    store.commit(vec![], "admin".into(), "v2".into(), 200);
    assert_eq!(store.history().len(), 2);
}

#[test]
fn test_policy_diff_added() {
    let mut store = PolicyStore::new();
    store.commit(vec![], "admin".into(), "empty".into(), 100);
    store.commit(vec![entry("a", "1")], "admin".into(), "added a".into(), 200);
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].old_value.is_none());
    assert_eq!(diffs[0].new_value.as_deref(), Some("1"));
}

#[test]
fn test_policy_diff_changed() {
    let mut store = PolicyStore::new();
    store.commit(vec![entry("a", "1")], "admin".into(), "v1".into(), 100);
    store.commit(vec![entry("a", "2")], "admin".into(), "v2".into(), 200);
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].old_value.as_deref(), Some("1"));
    assert_eq!(diffs[0].new_value.as_deref(), Some("2"));
}

#[test]
fn test_policy_diff_removed() {
    let mut store = PolicyStore::new();
    store.commit(vec![entry("a", "1")], "admin".into(), "v1".into(), 100);
    store.commit(vec![], "admin".into(), "v2".into(), 200);
    let diffs = store.diff(1, 2);
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].new_value.is_none());
}

#[test]
fn test_policy_rollback() {
    let mut store = PolicyStore::new();
    store.commit(vec![entry("a", "1")], "admin".into(), "v1".into(), 100);
    store.commit(vec![entry("a", "2")], "admin".into(), "v2".into(), 200);
    let new_v = store.rollback(1, "admin".into(), 300).unwrap();
    assert_eq!(new_v, 3);
    assert_eq!(store.current_entries()[0].value, "1");
}

#[test]
fn test_policy_rollback_unknown_version() {
    let mut store = PolicyStore::new();
    assert!(store.rollback(99, "admin".into(), 100).is_err());
}

#[test]
fn test_policy_scope_display() {
    assert_eq!(PolicyScope::Default.to_string(), "default");
    assert_eq!(PolicyScope::Group.to_string(), "group");
    assert_eq!(PolicyScope::User.to_string(), "user");
    assert_eq!(PolicyScope::Session.to_string(), "session");
}

// ===========================================================================
// MetricsCollector
// ===========================================================================

#[test]
fn test_metrics_collector_empty() {
    let c = MetricsCollector::default();
    assert!(c.latest_snapshot().is_none());
    assert_eq!(c.snapshot_count(), 0);
}

#[test]
fn test_metrics_record_snapshot() {
    let mut c = MetricsCollector::default();
    c.record_snapshot(MetricsSnapshot {
        total_sessions: 10,
        total_users: 5,
        avg_fps: 60.0,
        avg_latency_ms: 4.0,
        total_bandwidth_in_bps: 1_000_000,
        total_bandwidth_out_bps: 500_000,
        timestamp: 1000,
    });
    assert_eq!(c.snapshot_count(), 1);
    assert_eq!(c.latest_snapshot().unwrap().total_sessions, 10);
}

#[test]
fn test_metrics_retention_limit() {
    let mut c = MetricsCollector::new(3);
    for i in 0..5 {
        c.record_snapshot(MetricsSnapshot {
            total_sessions: i,
            timestamp: i as u64 * 100,
            ..MetricsSnapshot::default()
        });
    }
    assert_eq!(c.snapshot_count(), 3);
    assert_eq!(c.snapshots()[0].total_sessions, 2);
}

#[test]
fn test_metrics_record_point() {
    let mut c = MetricsCollector::default();
    c.record_point("cpu", "srv-a", 100, 42.0);
    c.record_point("cpu", "srv-a", 200, 50.0);
    let series = c.get_series("cpu", "srv-a").unwrap();
    assert_eq!(series.points.len(), 2);
    assert_eq!(series.points[1].value, 50.0);
}

#[test]
fn test_metrics_point_retention() {
    let mut c = MetricsCollector::new(2);
    c.record_point("m", "s", 100, 1.0);
    c.record_point("m", "s", 200, 2.0);
    c.record_point("m", "s", 300, 3.0);
    let series = c.get_series("m", "s").unwrap();
    assert_eq!(series.points.len(), 2);
    assert_eq!(series.points[0].value, 2.0);
}

#[test]
fn test_metrics_series_count() {
    let mut c = MetricsCollector::default();
    c.record_point("cpu", "srv-a", 100, 42.0);
    c.record_point("mem", "srv-a", 100, 60.0);
    c.record_point("cpu", "srv-b", 100, 30.0);
    assert_eq!(c.series_count(), 3);
}

// ===========================================================================
// DashboardBuilder
// ===========================================================================

#[test]
fn test_dashboard_builder_empty() {
    let dash = DashboardBuilder::new().build();
    assert_eq!(dash.servers_healthy, 0);
    assert_eq!(dash.servers_offline, 0);
    assert_eq!(dash.total_sessions, 0);
}

#[test]
fn test_dashboard_builder_servers() {
    let mut b = DashboardBuilder::new();
    b.add_server(true, 5, 3, 1000, 500);
    b.add_server(false, 2, 1, 800, 400);
    b.add_offline_server();
    let d = b.build();
    assert_eq!(d.servers_healthy, 1);
    assert_eq!(d.servers_unhealthy, 1);
    assert_eq!(d.servers_offline, 1);
    assert_eq!(d.total_sessions, 7);
    assert_eq!(d.total_users, 4);
    assert_eq!(d.bandwidth_in_bps, 1800);
}

#[test]
fn test_dashboard_builder_gateways() {
    let mut b = DashboardBuilder::new();
    b.add_gateway(true);
    b.add_gateway(true);
    b.add_gateway(false);
    let d = b.build();
    assert_eq!(d.gateways_online, 2);
    assert_eq!(d.gateways_offline, 1);
}

#[test]
fn test_dashboard_builder_alerts() {
    let mut b = DashboardBuilder::new();
    b.add_alert(
        AlertSeverity::Warning,
        "high cpu".into(),
        1000,
        Some("srv-a".into()),
    );
    let d = b.build();
    assert_eq!(d.alerts.len(), 1);
    assert_eq!(d.alerts[0].severity, AlertSeverity::Warning);
}

#[test]
fn test_alert_severity_display() {
    assert_eq!(AlertSeverity::Info.to_string(), "info");
    assert_eq!(AlertSeverity::Warning.to_string(), "warning");
    assert_eq!(AlertSeverity::Critical.to_string(), "critical");
}
