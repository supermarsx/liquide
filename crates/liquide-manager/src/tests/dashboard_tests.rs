//! Tests for dashboard aggregation.

use crate::dashboard::{AlertSeverity, DashboardBuilder};

// ===========================================================================
// Builder basics
// ===========================================================================

#[test]
fn test_empty_dashboard() {
    let builder = DashboardBuilder::new();
    let data = builder.build();
    assert_eq!(data.total_sessions, 0);
    assert_eq!(data.total_users, 0);
    assert_eq!(data.servers_healthy, 0);
    assert_eq!(data.servers_unhealthy, 0);
    assert_eq!(data.servers_offline, 0);
    assert_eq!(data.gateways_online, 0);
    assert_eq!(data.gateways_offline, 0);
    assert!(data.alerts.is_empty());
}

#[test]
fn test_add_healthy_server() {
    let mut builder = DashboardBuilder::new();
    builder.add_server(true, 5, 3, 1000, 2000);
    let data = builder.build();
    assert_eq!(data.servers_healthy, 1);
    assert_eq!(data.servers_unhealthy, 0);
    assert_eq!(data.total_sessions, 5);
    assert_eq!(data.total_users, 3);
    assert_eq!(data.bandwidth_in_bps, 1000);
    assert_eq!(data.bandwidth_out_bps, 2000);
}

#[test]
fn test_add_unhealthy_server() {
    let mut builder = DashboardBuilder::new();
    builder.add_server(false, 2, 1, 0, 0);
    let data = builder.build();
    assert_eq!(data.servers_healthy, 0);
    assert_eq!(data.servers_unhealthy, 1);
    assert_eq!(data.total_sessions, 2);
}

#[test]
fn test_add_offline_server() {
    let mut builder = DashboardBuilder::new();
    builder.add_offline_server();
    let data = builder.build();
    assert_eq!(data.servers_offline, 1);
}

#[test]
fn test_multiple_servers_aggregate() {
    let mut builder = DashboardBuilder::new();
    builder.add_server(true, 10, 5, 1000, 500);
    builder.add_server(true, 3, 2, 500, 250);
    builder.add_server(false, 1, 1, 100, 50);
    builder.add_offline_server();
    let data = builder.build();
    assert_eq!(data.servers_healthy, 2);
    assert_eq!(data.servers_unhealthy, 1);
    assert_eq!(data.servers_offline, 1);
    assert_eq!(data.total_sessions, 14);
    assert_eq!(data.total_users, 8);
    assert_eq!(data.bandwidth_in_bps, 1600);
    assert_eq!(data.bandwidth_out_bps, 800);
}

// ===========================================================================
// Gateways
// ===========================================================================

#[test]
fn test_add_gateways() {
    let mut builder = DashboardBuilder::new();
    builder.add_gateway(true);
    builder.add_gateway(true);
    builder.add_gateway(false);
    let data = builder.build();
    assert_eq!(data.gateways_online, 2);
    assert_eq!(data.gateways_offline, 1);
}

// ===========================================================================
// Alerts
// ===========================================================================

#[test]
fn test_add_alerts() {
    let mut builder = DashboardBuilder::new();
    builder.add_alert(AlertSeverity::Warning, "high cpu".into(), 1000, Some("srv-a".into()));
    builder.add_alert(AlertSeverity::Critical, "disk full".into(), 1001, None);
    let data = builder.build();
    assert_eq!(data.alerts.len(), 2);
    assert_eq!(data.alerts[0].severity, AlertSeverity::Warning);
    assert_eq!(data.alerts[1].severity, AlertSeverity::Critical);
    assert_eq!(data.alerts[0].server, Some("srv-a".into()));
    assert!(data.alerts[1].server.is_none());
}

// ===========================================================================
// Display
// ===========================================================================

#[test]
fn test_alert_severity_display() {
    assert_eq!(AlertSeverity::Info.to_string(), "info");
    assert_eq!(AlertSeverity::Warning.to_string(), "warning");
    assert_eq!(AlertSeverity::Critical.to_string(), "critical");
}

// ===========================================================================
// Serde
// ===========================================================================

#[test]
fn test_dashboard_data_serde() {
    let mut builder = DashboardBuilder::new();
    builder.add_server(true, 5, 3, 100, 200);
    builder.add_gateway(true);
    let data = builder.build();
    let json = serde_json::to_string(&data).unwrap();
    let parsed: crate::dashboard::DashboardData = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_sessions, 5);
    assert_eq!(parsed.gateways_online, 1);
}
