//! Tests for components, view models, API client, and theme.

use crate::api_client::{ApiError, ApiMethod, MockApiClient, RequestBuilder};
use crate::component::{
    Column, DataTable, MetricCard, Severity, SortDirection, StatusBadge, Toast, Trend,
};
use crate::theme::{PRESET_ALL, ThemePreset};
use crate::view_model::{
    AlertSeverityVM, AlertVM, DashboardVM, GatewayVM, PolicyVM, ServerStatusVM, ServerVM,
    SessionStatusVM, SessionVM, UserVM,
};

// ===========================================================================
// DataTable — sorting
// ===========================================================================

#[test]
fn test_data_table_creation() {
    let cols = vec![Column::new("name", "Name"), Column::new("status", "Status")];
    let table = DataTable::new(cols);
    assert_eq!(table.columns.len(), 2);
    assert!(table.rows.is_empty());
    assert_eq!(table.total_rows, 0);
}

#[test]
fn test_data_table_set_rows() {
    let cols = vec![Column::new("name", "Name")];
    let mut table = DataTable::new(cols);
    table.set_rows(vec![
        vec!["alpha".into()],
        vec!["beta".into()],
        vec!["gamma".into()],
    ]);
    assert_eq!(table.total_rows, 3);
    assert_eq!(table.rows.len(), 3);
}

#[test]
fn test_data_table_sort_ascending() {
    let cols = vec![Column::new("name", "Name")];
    let mut table = DataTable::new(cols);
    table.set_rows(vec![
        vec!["gamma".into()],
        vec!["alpha".into()],
        vec!["beta".into()],
    ]);
    table.sort_by("name", SortDirection::Ascending);
    assert_eq!(table.rows[0][0], "alpha");
    assert_eq!(table.rows[1][0], "beta");
    assert_eq!(table.rows[2][0], "gamma");
}

#[test]
fn test_data_table_sort_descending() {
    let cols = vec![Column::new("name", "Name")];
    let mut table = DataTable::new(cols);
    table.set_rows(vec![
        vec!["alpha".into()],
        vec!["gamma".into()],
        vec!["beta".into()],
    ]);
    table.sort_by("name", SortDirection::Descending);
    assert_eq!(table.rows[0][0], "gamma");
    assert_eq!(table.rows[1][0], "beta");
    assert_eq!(table.rows[2][0], "alpha");
}

// ===========================================================================
// DataTable — pagination
// ===========================================================================

#[test]
fn test_data_table_total_pages() {
    let cols = vec![Column::new("x", "X")];
    let mut table = DataTable::new(cols);
    table.per_page = 2;
    table.set_rows(vec![
        vec!["a".into()],
        vec!["b".into()],
        vec!["c".into()],
        vec!["d".into()],
        vec!["e".into()],
    ]);
    assert_eq!(table.total_pages(), 3); // ceil(5/2) = 3
}

#[test]
fn test_data_table_page_rows() {
    let cols = vec![Column::new("x", "X")];
    let mut table = DataTable::new(cols);
    table.per_page = 2;
    table.set_rows(vec![vec!["a".into()], vec!["b".into()], vec!["c".into()]]);
    // Page 1 (default)
    assert_eq!(table.page_rows().len(), 2);
    assert_eq!(table.page_rows()[0][0], "a");

    // Page 2
    table.go_to_page(2);
    assert_eq!(table.page_rows().len(), 1);
    assert_eq!(table.page_rows()[0][0], "c");
}

// ===========================================================================
// MetricCard
// ===========================================================================

#[test]
fn test_metric_card() {
    let card = MetricCard::new("CPU", "45")
        .with_unit("%")
        .with_trend(Trend::Up);
    assert_eq!(card.label, "CPU");
    assert_eq!(card.value, "45");
    assert_eq!(card.unit, Some("%".to_string()));
    assert_eq!(card.trend, Some(Trend::Up));
}

#[test]
fn test_trend_display() {
    assert_eq!(Trend::Up.to_string(), "up");
    assert_eq!(Trend::Down.to_string(), "down");
    assert_eq!(Trend::Stable.to_string(), "stable");
}

// ===========================================================================
// StatusBadge
// ===========================================================================

#[test]
fn test_status_badge() {
    let badge = StatusBadge::new("Online", Severity::Success);
    assert_eq!(badge.label, "Online");
    assert_eq!(badge.severity, Severity::Success);
}

#[test]
fn test_severity_display() {
    assert_eq!(Severity::Info.to_string(), "info");
    assert_eq!(Severity::Success.to_string(), "success");
    assert_eq!(Severity::Warning.to_string(), "warning");
    assert_eq!(Severity::Error.to_string(), "error");
}

// ===========================================================================
// Toast
// ===========================================================================

#[test]
fn test_toast_convenience_constructors() {
    let info = Toast::info("connected");
    assert_eq!(info.severity, Severity::Info);
    assert_eq!(info.auto_dismiss_ms, 5000);

    let err = Toast::error("disconnect").with_auto_dismiss_ms(0);
    assert_eq!(err.severity, Severity::Error);
    assert_eq!(err.auto_dismiss_ms, 0);
}

// ===========================================================================
// RequestBuilder
// ===========================================================================

#[test]
fn test_request_builder_get() {
    let req = RequestBuilder::get("/api/v1/sessions")
        .query("page", "2")
        .query("per_page", "10")
        .build();
    assert_eq!(req.method, ApiMethod::Get);
    assert_eq!(req.path, "/api/v1/sessions");
    assert_eq!(req.query_params.get("page"), Some(&"2".to_string()));
    assert!(req.body.is_none());
}

#[test]
fn test_request_builder_post_with_body() {
    let req = RequestBuilder::post("/api/v1/sessions/s1/lock")
        .body(r#"{"reason":"maintenance"}"#)
        .build();
    assert_eq!(req.method, ApiMethod::Post);
    assert!(req.body.is_some());
}

#[test]
fn test_request_builder_put_and_delete() {
    let put = RequestBuilder::put("/api/v1/policies").build();
    assert_eq!(put.method, ApiMethod::Put);

    let del = RequestBuilder::delete("/api/v1/sessions/s1").build();
    assert_eq!(del.method, ApiMethod::Delete);
}

// ===========================================================================
// MockApiClient
// ===========================================================================

#[test]
fn test_mock_client_ok_response() {
    let mut client = MockApiClient::new();
    client.enqueue_ok(r#"{"status":"ok"}"#);
    let req = RequestBuilder::get("/test").build();
    let resp = client.send(req);
    assert!(resp.is_ok());
    assert_eq!(resp.unwrap(), r#"{"status":"ok"}"#);
    assert_eq!(client.recorded().len(), 1);
}

#[test]
fn test_mock_client_err_response() {
    let mut client = MockApiClient::new();
    client.enqueue_err(ApiError::Unauthorized);
    let req = RequestBuilder::get("/test").build();
    let resp = client.send(req);
    assert!(resp.is_err());
    assert_eq!(resp.unwrap_err(), ApiError::Unauthorized);
}

#[test]
fn test_mock_client_no_response_queued() {
    let mut client = MockApiClient::new();
    let req = RequestBuilder::get("/test").build();
    let resp = client.send(req);
    assert!(resp.is_err()); // Network error when nothing is queued
}

// ===========================================================================
// View models — serde round-trip
// ===========================================================================

#[test]
fn test_dashboard_vm_serde() {
    let vm = DashboardVM {
        total_sessions: 42,
        total_users: 10,
        servers_healthy: 3,
        servers_unhealthy: 1,
        servers_offline: 0,
        gateways_online: 2,
        gateways_offline: 0,
        bandwidth_in: 1_000_000,
        bandwidth_out: 500_000,
        alerts: vec![AlertVM::new(
            AlertSeverityVM::Warning,
            "high cpu",
            1000,
            Some("srv-a".into()),
        )],
    };
    let json = serde_json::to_string(&vm).unwrap();
    let parsed: DashboardVM = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_sessions, 42);
    assert_eq!(parsed.alerts.len(), 1);
    assert_eq!(parsed.alerts[0].severity, AlertSeverityVM::Warning);
}

#[test]
fn test_server_vm_status_css() {
    assert_eq!(ServerStatusVM::Online.css_class(), "status-online");
    assert_eq!(ServerStatusVM::Unhealthy.css_class(), "status-unhealthy");
    assert_eq!(ServerStatusVM::Offline.css_class(), "status-offline");
    assert_eq!(ServerStatusVM::Draining.css_class(), "status-draining");
}

#[test]
fn test_session_vm_status_display() {
    assert_eq!(SessionStatusVM::Active.to_string(), "active");
    assert_eq!(SessionStatusVM::Locked.to_string(), "locked");
    assert_eq!(SessionStatusVM::Suspended.to_string(), "suspended");
    assert_eq!(SessionStatusVM::Disconnecting.to_string(), "disconnecting");
}

#[test]
fn test_session_vm_creation() {
    let vm = SessionVM::new("s1", "alice", "srv-a", SessionStatusVM::Active);
    assert_eq!(vm.session_id, "s1");
    assert_eq!(vm.user, "alice");
    assert_eq!(vm.server, "srv-a");
    assert_eq!(vm.status, SessionStatusVM::Active);
}

#[test]
fn test_user_vm() {
    let vm = UserVM::new("alice", "admin");
    assert_eq!(vm.username, "alice");
    assert_eq!(vm.role, "admin");
    assert_eq!(vm.active_sessions, 0);
    assert!(vm.last_login.is_none());
}

#[test]
fn test_policy_vm() {
    let vm = PolicyVM::new("default-policy", 3);
    assert_eq!(vm.name, "default-policy");
    assert_eq!(vm.version, 3);
    assert!(vm.active);
}

#[test]
fn test_gateway_vm() {
    let vm = GatewayVM::new("gw-east", "10.0.0.1:443", true);
    assert_eq!(vm.name, "gw-east");
    assert!(vm.online);
    assert_eq!(vm.servers_count, 0);
}

#[test]
fn test_server_vm_serde() {
    let vm = ServerVM::new("srv-a", "10.0.0.1:8443", ServerStatusVM::Online);
    let json = serde_json::to_string(&vm).unwrap();
    let parsed: ServerVM = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "srv-a");
    assert_eq!(parsed.status, ServerStatusVM::Online);
}

// ===========================================================================
// Theme presets
// ===========================================================================

#[test]
fn test_all_presets_produce_themes() {
    for preset in PRESET_ALL {
        let theme = preset.to_theme();
        assert!(!theme.name.is_empty(), "{preset:?} produced empty name");
        assert!(theme.sidebar_width > 0);
        assert!(theme.font_size > 0);
    }
}

#[test]
fn test_preset_display() {
    assert_eq!(ThemePreset::LiquidGlass.to_string(), "liquid-glass");
    assert_eq!(ThemePreset::Dark.to_string(), "dark");
    assert_eq!(ThemePreset::Light.to_string(), "light");
    assert_eq!(ThemePreset::HighContrast.to_string(), "high-contrast");
}

#[test]
fn test_column_builder() {
    let col = Column::new("name", "Name")
        .with_sortable(false)
        .with_width(200);
    assert!(!col.sortable);
    assert_eq!(col.width, Some(200));
}

#[test]
fn test_alert_severity_vm_css() {
    assert_eq!(AlertSeverityVM::Info.css_class(), "alert-info");
    assert_eq!(AlertSeverityVM::Warning.css_class(), "alert-warning");
    assert_eq!(AlertSeverityVM::Critical.css_class(), "alert-critical");
}
