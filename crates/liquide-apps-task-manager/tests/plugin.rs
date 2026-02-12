//! Tests for `plugin` module types.

use liquide_apps_task_manager::plugin::*;

// ---------------------------------------------------------------------------
// PluginInfo
// ---------------------------------------------------------------------------

#[test]
fn plugin_info_construction() {
    let info = PluginInfo {
        name: "my-plugin".into(),
        version: "1.2.0".into(),
        author: "Test Author".into(),
        description: "A test plugin".into(),
        api_version: 1,
    };
    assert_eq!(info.name, "my-plugin");
    assert_eq!(info.version, "1.2.0");
    assert_eq!(info.api_version, 1);
}

#[test]
fn plugin_info_serde_roundtrip() {
    let info = PluginInfo {
        name: "network-monitor".into(),
        version: "0.5.0".into(),
        author: "LiquiDE".into(),
        description: "Extended network monitoring".into(),
        api_version: 2,
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: PluginInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "network-monitor");
    assert_eq!(back.api_version, 2);
}

// ---------------------------------------------------------------------------
// TabDefinition
// ---------------------------------------------------------------------------

#[test]
fn tab_definition_construction() {
    let tab = TabDefinition {
        id: "custom-tab".into(),
        label: "Custom Tab".into(),
        icon: Some("custom-icon".into()),
        order: 100,
    };
    assert_eq!(tab.id, "custom-tab");
    assert_eq!(tab.label, "Custom Tab");
    assert_eq!(tab.order, 100);
}

#[test]
fn tab_definition_serde_roundtrip() {
    let tab = TabDefinition {
        id: "perf-ext".into(),
        label: "Extended Performance".into(),
        icon: None,
        order: 50,
    };
    let json = serde_json::to_string(&tab).unwrap();
    let back: TabDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "perf-ext");
    assert_eq!(back.order, 50);
    assert!(back.icon.is_none());
}

// ---------------------------------------------------------------------------
// ColumnDefinition
// ---------------------------------------------------------------------------

#[test]
fn column_definition_construction() {
    let col = ColumnDefinition {
        key: "custom_metric".into(),
        label: "Custom Metric".into(),
        width_px: 120,
        sortable: true,
        default_visible: false,
    };
    assert_eq!(col.key, "custom_metric");
    assert!(col.sortable);
    assert!(!col.default_visible);
}

#[test]
fn column_definition_serde_roundtrip() {
    let col = ColumnDefinition {
        key: "latency".into(),
        label: "Latency (ms)".into(),
        width_px: 80,
        sortable: true,
        default_visible: true,
    };
    let json = serde_json::to_string(&col).unwrap();
    let back: ColumnDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.key, "latency");
    assert_eq!(back.width_px, 80);
}

// ---------------------------------------------------------------------------
// MenuItemDefinition
// ---------------------------------------------------------------------------

#[test]
fn menu_item_definition_construction() {
    let item = MenuItemDefinition {
        label: "Run Analysis".into(),
        action_id: "plugin.run_analysis".into(),
        shortcut: Some("Ctrl+Shift+A".into()),
        icon: None,
        separator_before: true,
    };
    assert_eq!(item.action_id, "plugin.run_analysis");
    assert!(item.separator_before);
    assert_eq!(item.shortcut.unwrap(), "Ctrl+Shift+A");
}

#[test]
fn menu_item_definition_serde_roundtrip() {
    let item = MenuItemDefinition {
        label: "Export Data".into(),
        action_id: "plugin.export".into(),
        shortcut: None,
        icon: Some("export-icon".into()),
        separator_before: false,
    };
    let json = serde_json::to_string(&item).unwrap();
    let back: MenuItemDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.label, "Export Data");
    assert!(!back.separator_before);
}

// ---------------------------------------------------------------------------
// SystemState
// ---------------------------------------------------------------------------

#[test]
fn system_state_construction() {
    let state = SystemState {
        process_count: 250,
        thread_count: 3000,
        cpu_percent: 35.5,
        memory_percent: 68.0,
        uptime_secs: 86400,
    };
    assert_eq!(state.process_count, 250);
    assert_eq!(state.uptime_secs, 86400);
}

#[test]
fn system_state_serde_roundtrip() {
    let state = SystemState {
        process_count: 100,
        thread_count: 1500,
        cpu_percent: 42.0,
        memory_percent: 55.5,
        uptime_secs: 3600,
    };
    let json = serde_json::to_string(&state).unwrap();
    let back: SystemState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.process_count, 100);
    assert_eq!(back.cpu_percent, 42.0);
}
