//! Tests for gateway management.

use crate::gateway_mgmt::{GatewayRegistry, GatewayStatus};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_registry() -> GatewayRegistry {
    let mut reg = GatewayRegistry::new();
    reg.register("gw-east".into(), "10.0.1.1:443".into());
    reg.register("gw-west".into(), "10.0.2.1:443".into());
    reg
}

// ===========================================================================
// Basic operations
// ===========================================================================

#[test]
fn test_new_registry_is_empty() {
    let reg = GatewayRegistry::new();
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.online_count(), 0);
}

#[test]
fn test_register_gateways() {
    let reg = make_registry();
    assert_eq!(reg.count(), 2);
    assert!(reg.get("gw-east").is_some());
    assert!(reg.get("gw-west").is_some());
}

#[test]
fn test_duplicate_register_ignored() {
    let mut reg = make_registry();
    reg.register("gw-east".into(), "10.0.99.1:443".into());
    assert_eq!(reg.count(), 2);
}

#[test]
fn test_initial_status_offline() {
    let reg = make_registry();
    let gw = reg.get("gw-east").unwrap();
    assert_eq!(gw.status, GatewayStatus::Offline);
}

// ===========================================================================
// Metric updates
// ===========================================================================

#[test]
fn test_update_gateway() {
    let mut reg = make_registry();
    reg.update("gw-east", GatewayStatus::Online, 3, 10, 500_000);
    let gw = reg.get("gw-east").unwrap();
    assert_eq!(gw.status, GatewayStatus::Online);
    assert_eq!(gw.connected_servers, 3);
    assert_eq!(gw.active_relays, 10);
    assert_eq!(gw.bandwidth_bps, 500_000);
}

#[test]
fn test_mark_offline() {
    let mut reg = make_registry();
    reg.update("gw-east", GatewayStatus::Online, 0, 0, 0);
    reg.mark_offline("gw-east");
    assert_eq!(reg.get("gw-east").unwrap().status, GatewayStatus::Offline);
}

// ===========================================================================
// Online count
// ===========================================================================

#[test]
fn test_online_count() {
    let mut reg = make_registry();
    reg.update("gw-east", GatewayStatus::Online, 0, 0, 0);
    assert_eq!(reg.online_count(), 1);
    reg.update("gw-west", GatewayStatus::Online, 0, 0, 0);
    assert_eq!(reg.online_count(), 2);
    reg.update("gw-east", GatewayStatus::Degraded, 0, 0, 0);
    assert_eq!(reg.online_count(), 1);
}

// ===========================================================================
// List
// ===========================================================================

#[test]
fn test_list() {
    let reg = make_registry();
    let all = reg.list();
    assert_eq!(all.len(), 2);
}

// ===========================================================================
// Display
// ===========================================================================

#[test]
fn test_gateway_status_display() {
    assert_eq!(GatewayStatus::Online.to_string(), "online");
    assert_eq!(GatewayStatus::Degraded.to_string(), "degraded");
    assert_eq!(GatewayStatus::Offline.to_string(), "offline");
}
