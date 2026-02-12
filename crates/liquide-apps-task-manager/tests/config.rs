//! Tests for `config` module types.

use liquide_apps_task_manager::config::*;
use liquide_apps_task_manager::TaskManagerConfig;

// ---------------------------------------------------------------------------
// Default config
// ---------------------------------------------------------------------------

#[test]
fn default_config_constructs() {
    let config = TaskManagerConfig::default();
    assert!(!config.general.always_on_top);
    assert!(config.general.confirm_end_task);
    assert!(config.general.remember_window_position);
}

#[test]
fn default_general_config() {
    let cfg = GeneralConfig::default();
    assert_eq!(cfg.view_mode, liquide_apps_task_manager::ui::ViewMode::Standard);
    assert_eq!(cfg.show_tray_icon, TrayIconMode::WhenMinimized);
    assert_eq!(cfg.language, "system");
    assert!(!cfg.start_minimized);
}

#[test]
fn default_performance_config() {
    let cfg = PerformanceConfig::default();
    assert_eq!(cfg.update_interval_ms, 1000);
    assert_eq!(cfg.graph_time_range_s, 60);
    assert_eq!(cfg.graph_line_width, 2);
    assert!(cfg.graph_anti_aliasing);
    assert_eq!(cfg.graph_fill_opacity, 30);
    assert_eq!(cfg.graph_background, GraphBackground::Grid);
    assert_eq!(cfg.grid_line_style, GridLineStyle::Dashed);
    assert_eq!(cfg.graph_interpolation, GraphInterpolation::Bezier);
    assert_eq!(cfg.network_scale, NetworkScale::Auto);
}

#[test]
fn default_processes_config() {
    let cfg = ProcessesConfig::default();
    assert!(cfg.visible_columns.contains(&"name".to_string()));
    assert!(cfg.visible_columns.contains(&"pid".to_string()));
    assert_eq!(cfg.sort_column, "cpu_percent");
    assert!(cfg.sort_descending);
    assert!(cfg.show_process_icons);
    assert_eq!(cfg.row_height, RowHeight::Normal);
    assert_eq!(cfg.heat_map_intensity, Intensity::Medium);
    assert_eq!(cfg.process_name_display, ProcessNameDisplay::FriendlyName);
}

#[test]
fn default_app_history_config() {
    let cfg = AppHistoryConfig::default();
    assert_eq!(cfg.retention_days, 30);
    assert!(cfg.track_background);
    assert!(cfg.auto_delete_uninstalled);
    assert_eq!(cfg.max_db_size_mb, 100);
}

#[test]
fn default_startup_config() {
    let cfg = StartupConfig::default();
    assert!(!cfg.show_system_services);
    assert_eq!(cfg.boot_timeline_detail, DetailLevel::Standard);
    assert_eq!(cfg.boot_history_days, 30);
}

#[test]
fn default_services_config() {
    let cfg = ServicesConfig::default();
    assert!(!cfg.show_running_only);
    assert!(cfg.confirm_state_changes);
    assert_eq!(cfg.show_dependency_viewer, DependencyViewerMode::Dialog);
}

#[test]
fn default_files_in_use_config() {
    let cfg = FilesInUseConfig::default();
    assert_eq!(cfg.refresh_interval_ms, 5000);
    assert_eq!(cfg.max_results, 5000);
    assert_eq!(cfg.watch_notification, NotificationMode::Toast);
}

#[test]
fn default_unlock_config() {
    let cfg = UnlockConfig::default();
    assert_eq!(cfg.require_confirmation, "always");
    assert!(!cfg.auto_create_backup);
    assert_eq!(cfg.audit_log_max_mb, 50);
}

#[test]
fn default_network_traffic_config() {
    let cfg = NetworkTrafficConfig::default();
    assert_eq!(cfg.default_view, "overview");
    assert!(cfg.show_reverse_dns);
    assert!(cfg.dns_query_logging);
    assert_eq!(cfg.dns_log_retention_hours, 24);
    assert!(!cfg.traffic_shaping);
}

#[test]
fn default_energy_config() {
    let cfg = EnergyConfig::default();
    assert_eq!(cfg.temperature_unit, TemperatureUnit::Celsius);
    assert!(!cfg.carbon_tracking);
    assert!(cfg.battery_health_report);
    assert_eq!(cfg.energy_history_days, 90);
    assert_eq!(cfg.battery_cycle_warning, 500);
}

#[test]
fn default_audio_config() {
    let cfg = AudioConfig::default();
    assert_eq!(cfg.default_view, "output");
    assert_eq!(cfg.meter_type, "peak");
    assert_eq!(cfg.spectrum_fft_size, 4096);
    assert!(!cfg.spectrum_analyzer);
    assert!(cfg.per_stream_volume);
    assert_eq!(cfg.recording_bit_depth, 24);
}

#[test]
fn default_notifications_config() {
    let cfg = NotificationsConfig::default();
    assert_eq!(cfg.high_cpu_threshold, 0);
    assert_eq!(cfg.alert_sound, AlertSound::None);
    assert_eq!(cfg.alert_method, AlertMethod::StatusBar);
    assert_eq!(cfg.alert_cooldown_s, 30);
}

#[test]
fn default_export_config() {
    let cfg = ExportConfig::default();
    assert_eq!(cfg.format, liquide_apps_task_manager::export::ExportFormat::Csv);
    assert!(cfg.include_headers);
    assert_eq!(cfg.timestamp_format, TimestampFormat::Iso8601);
    assert_eq!(cfg.decimal_separator, '.');
}

#[test]
fn default_plugins_config() {
    let cfg = PluginsConfig::default();
    assert!(cfg.enabled);
    assert!(cfg.auto_update);
    assert_eq!(cfg.sandboxing, SandboxLevel::Strict);
}

#[test]
fn default_keyboard_config() {
    let cfg = KeyboardConfig::default();
    assert_eq!(cfg.shortcut_scheme, ShortcutScheme::Default);
    assert_eq!(cfg.single_click, ClickAction::Select);
    assert_eq!(cfg.double_click, DoubleClickAction::Properties);
    assert_eq!(cfg.middle_click, MiddleClickAction::None);
    assert_eq!(cfg.scroll_on_graph, ScrollGraphAction::Zoom);
}

#[test]
fn default_accessibility_config() {
    let cfg = AccessibilityConfig::default();
    assert_eq!(cfg.high_contrast, HighContrastMode::System);
    assert_eq!(cfg.font_size, 12);
    assert_eq!(cfg.screen_reader_level, ScreenReaderLevel::Standard);
    assert!(!cfg.reduce_motion);
    assert_eq!(cfg.color_blind_mode, ColorBlindMode::Off);
}

#[test]
fn default_advanced_config() {
    let cfg = AdvancedConfig::default();
    assert_eq!(cfg.sampling_rate_hz, 1.0);
    assert_eq!(cfg.ring_buffer_size, 300);
    assert!(!cfg.enable_etw);
    assert_eq!(cfg.debug_logging, LogLevel::Off);
    assert_eq!(cfg.process_scan_method, ScanMethod::Incremental);
}

// ---------------------------------------------------------------------------
// Serde roundtrip
// ---------------------------------------------------------------------------

#[test]
fn config_json_roundtrip() {
    let config = TaskManagerConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let back: TaskManagerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.performance.update_interval_ms, 1000);
}

#[test]
fn config_toml_roundtrip() {
    let config = TaskManagerConfig::default();
    let toml_str = toml::to_string(&config).unwrap();
    let back: TaskManagerConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(back.general.language, "system");
}

#[test]
fn config_partial_toml_deserialization() {
    let partial = r#"
[general]
always_on_top = true
"#;
    let config: TaskManagerConfig = toml::from_str(partial).unwrap();
    assert!(config.general.always_on_top);
    // Other fields should get defaults
    assert_eq!(config.performance.update_interval_ms, 1000);
}

// ---------------------------------------------------------------------------
// Config enums serde
// ---------------------------------------------------------------------------

#[test]
fn tray_icon_mode_serde_roundtrip() {
    let val = TrayIconMode::WhenMinimized;
    let json = serde_json::to_string(&val).unwrap();
    let back: TrayIconMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

#[test]
fn temperature_unit_serde_roundtrip() {
    let val = TemperatureUnit::Fahrenheit;
    let json = serde_json::to_string(&val).unwrap();
    let back: TemperatureUnit = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

#[test]
fn graph_background_all_variants() {
    let variants = [
        GraphBackground::Solid,
        GraphBackground::Grid,
        GraphBackground::None,
    ];
    assert_eq!(variants.len(), 3);
}

#[test]
fn color_blind_mode_all_variants() {
    let variants = [
        ColorBlindMode::Off,
        ColorBlindMode::Protanopia,
        ColorBlindMode::Deuteranopia,
        ColorBlindMode::Tritanopia,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn shortcut_scheme_all_variants() {
    let variants = [
        ShortcutScheme::Default,
        ShortcutScheme::Custom,
        ShortcutScheme::Vim,
        ShortcutScheme::Emacs,
    ];
    assert_eq!(variants.len(), 4);
}
