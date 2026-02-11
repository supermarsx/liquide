//! Tests for server management.

use crate::server_mgmt::{ServerRegistry, ServerStatus};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_registry() -> ServerRegistry {
    let mut reg = ServerRegistry::new();
    reg.register("srv-a".into(), "10.0.0.1:9000".into());
    reg.register("srv-b".into(), "10.0.0.2:9000".into());
    reg
}

// ===========================================================================
// Registry basics
// ===========================================================================

#[test]
fn test_new_registry_is_empty() {
    let reg = ServerRegistry::new();
    assert_eq!(reg.count(), 0);
    assert!(reg.list().is_empty());
}

#[test]
fn test_register_servers() {
    let reg = make_registry();
    assert_eq!(reg.count(), 2);
    assert!(reg.get("srv-a").is_some());
    assert!(reg.get("srv-b").is_some());
    assert!(reg.get("srv-c").is_none());
}

#[test]
fn test_duplicate_register_ignored() {
    let mut reg = make_registry();
    reg.register("srv-a".into(), "10.0.0.99:9000".into());
    assert_eq!(reg.count(), 2);
    // address should not change
    let srv = reg.get("srv-a").unwrap();
    assert_eq!(srv.address, "10.0.0.1:9000");
}

#[test]
fn test_initial_status_is_offline() {
    let reg = make_registry();
    let srv = reg.get("srv-a").unwrap();
    assert_eq!(srv.status, ServerStatus::Offline);
}

// ===========================================================================
// Metric updates
// ===========================================================================

#[test]
fn test_update_metrics() {
    let mut reg = make_registry();
    reg.update_metrics("srv-a", ServerStatus::Online, 5, 45.0, 60.0, 3600, 1000);
    let srv = reg.get("srv-a").unwrap();
    assert_eq!(srv.status, ServerStatus::Online);
    assert_eq!(srv.active_sessions, 5);
    assert!((srv.cpu_percent - 45.0).abs() < f32::EPSILON);
    assert!((srv.memory_percent - 60.0).abs() < f32::EPSILON);
    assert_eq!(srv.uptime_seconds, 3600);
}

#[test]
fn test_update_unknown_server_is_noop() {
    let mut reg = make_registry();
    reg.update_metrics("unknown", ServerStatus::Online, 1, 10.0, 20.0, 100, 50);
    assert_eq!(reg.count(), 2);
}

// ===========================================================================
// Status changes
// ===========================================================================

#[test]
fn test_mark_offline() {
    let mut reg = make_registry();
    reg.update_metrics("srv-a", ServerStatus::Online, 0, 0.0, 0.0, 0, 0);
    reg.mark_offline("srv-a");
    assert_eq!(reg.get("srv-a").unwrap().status, ServerStatus::Offline);
}

#[test]
fn test_mark_draining() {
    let mut reg = make_registry();
    reg.update_metrics("srv-a", ServerStatus::Online, 3, 0.0, 0.0, 0, 0);
    reg.mark_draining("srv-a");
    assert_eq!(reg.get("srv-a").unwrap().status, ServerStatus::Draining);
}

// ===========================================================================
// Aggregates
// ===========================================================================

#[test]
fn test_count_by_status() {
    let mut reg = make_registry();
    reg.update_metrics("srv-a", ServerStatus::Online, 2, 0.0, 0.0, 0, 0);
    assert_eq!(reg.count_by_status(ServerStatus::Online), 1);
    assert_eq!(reg.count_by_status(ServerStatus::Offline), 1);
}

#[test]
fn test_total_sessions() {
    let mut reg = make_registry();
    reg.update_metrics("srv-a", ServerStatus::Online, 10, 0.0, 0.0, 0, 0);
    reg.update_metrics("srv-b", ServerStatus::Online, 7, 0.0, 0.0, 0, 0);
    assert_eq!(reg.total_sessions(), 17);
}

// ===========================================================================
// Display
// ===========================================================================

#[test]
fn test_server_status_display() {
    assert_eq!(ServerStatus::Online.to_string(), "online");
    assert_eq!(ServerStatus::Unhealthy.to_string(), "unhealthy");
    assert_eq!(ServerStatus::Offline.to_string(), "offline");
    assert_eq!(ServerStatus::Draining.to_string(), "draining");
}
