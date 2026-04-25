use crate::config::*;

// ---------------------------------------------------------------------------
// ShellConfig::default()
// ---------------------------------------------------------------------------

#[test]
fn config_default_creates_valid_config() {
    let config = ShellConfig::default();
    // All sub-configs should be present and at their defaults.
    // Just accessing them is enough to verify the struct was populated.
    let _dock = &config.dock;
    let _bar = &config.status_bar;
    let _launcher = &config.launcher;
    let _tiling = &config.tiling;
    let _notifications = &config.notifications;
    let _seamless = &config.seamless;
}

// ---------------------------------------------------------------------------
// Verify specific default values
// ---------------------------------------------------------------------------

#[test]
fn config_default_dock_icon_size() {
    let config = ShellConfig::default();
    assert_eq!(config.dock.icon_size, 48);
}

#[test]
fn config_default_dock_auto_hide() {
    let config = ShellConfig::default();
    assert!(!config.dock.auto_hide);
}

#[test]
fn config_default_status_bar_enabled() {
    let config = ShellConfig::default();
    assert!(config.status_bar.enabled);
}

#[test]
fn config_default_tiling_gap() {
    let config = ShellConfig::default();
    assert!(
        (config.tiling.gap - 8.0).abs() < f32::EPSILON,
        "expected tiling.gap = 8.0, got {}",
        config.tiling.gap
    );
}

#[test]
fn config_default_notifications_max_visible() {
    let config = ShellConfig::default();
    assert_eq!(config.notifications.max_visible, 5);
}

#[test]
fn config_default_launcher_calculator_enabled() {
    let config = ShellConfig::default();
    assert!(config.launcher.calculator_enabled);
}

#[test]
fn config_default_seamless_disabled() {
    let config = ShellConfig::default();
    assert!(!config.seamless.enabled);
}

#[test]
fn config_default_tiling_enabled() {
    let config = ShellConfig::default();
    assert!(config.tiling.enabled);
}

#[test]
fn config_default_dock_magnification_factor() {
    let config = ShellConfig::default();
    assert!(
        (config.dock.magnification_factor - 1.5).abs() < f32::EPSILON,
        "expected 1.5, got {}",
        config.dock.magnification_factor
    );
}

#[test]
fn config_default_notifications_default_timeout() {
    let config = ShellConfig::default();
    assert_eq!(config.notifications.default_timeout_ms, 5000);
}

#[test]
fn config_default_status_bar_height() {
    let config = ShellConfig::default();
    assert_eq!(config.status_bar.height, 34.0);
}

// ---------------------------------------------------------------------------
// Serde JSON roundtrip
// ---------------------------------------------------------------------------

#[test]
fn config_serde_json_roundtrip() {
    let original = ShellConfig::default();
    let json = serde_json::to_string(&original).expect("serialize to JSON");
    let restored: ShellConfig = serde_json::from_str(&json).expect("deserialize from JSON");

    // Compare key fields to verify the roundtrip preserved data.
    assert_eq!(restored.dock.icon_size, original.dock.icon_size);
    assert_eq!(restored.dock.auto_hide, original.dock.auto_hide);
    assert_eq!(restored.status_bar.enabled, original.status_bar.enabled);
    assert_eq!(restored.status_bar.height, original.status_bar.height);
    assert!(
        (restored.tiling.gap - original.tiling.gap).abs() < f32::EPSILON,
        "tiling.gap mismatch after roundtrip"
    );
    assert_eq!(
        restored.notifications.max_visible,
        original.notifications.max_visible
    );
    assert_eq!(
        restored.launcher.calculator_enabled,
        original.launcher.calculator_enabled
    );
    assert_eq!(restored.seamless.enabled, original.seamless.enabled);
}

// ---------------------------------------------------------------------------
// Display impl
// ---------------------------------------------------------------------------

#[test]
fn config_display_default() {
    let config = ShellConfig::default();
    let display = format!("{config}");
    // Default: dock visible, status_bar on, launcher calc, tiling on
    assert!(
        display.contains("visible"),
        "display should mention dock visible: {display}"
    );
    assert!(
        display.contains("status_bar=on"),
        "display should mention status_bar=on: {display}"
    );
    assert!(
        display.contains("calc"),
        "display should mention calc for launcher: {display}"
    );
    assert!(
        display.contains("tiling=on"),
        "display should mention tiling=on: {display}"
    );
}

#[test]
fn config_display_auto_hide_dock() {
    let mut config = ShellConfig::default();
    config.dock.auto_hide = true;
    let display = format!("{config}");
    assert!(
        display.contains("auto-hide"),
        "display should show auto-hide for dock: {display}"
    );
}

// ---------------------------------------------------------------------------
// Access nested config fields
// ---------------------------------------------------------------------------

#[test]
fn config_access_dock_show_running_indicators() {
    let config = ShellConfig::default();
    assert!(config.dock.show_running_indicators);
}

#[test]
fn config_access_dock_auto_hide_delay() {
    let config = ShellConfig::default();
    assert_eq!(config.dock.auto_hide_delay_ms, 500);
}

#[test]
fn config_access_tiling_master_ratio() {
    let config = ShellConfig::default();
    assert!(
        (config.tiling.master_ratio - 0.55).abs() < f32::EPSILON,
        "expected 0.55, got {}",
        config.tiling.master_ratio
    );
}

#[test]
fn config_access_launcher_max_favorites() {
    let config = ShellConfig::default();
    assert_eq!(config.launcher.max_favorites, 9);
}

#[test]
fn config_access_notifications_history_capacity() {
    let config = ShellConfig::default();
    assert_eq!(config.notifications.history_capacity, 100);
}

#[test]
fn config_access_status_bar_clock_format() {
    let config = ShellConfig::default();
    assert_eq!(config.status_bar.clock_format, "%H:%M");
}

#[test]
fn config_access_seamless_forward_notifications() {
    let config = ShellConfig::default();
    assert!(config.seamless.forward_notifications);
}
