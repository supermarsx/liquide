//! Configuration for the task manager.
//!
//! Covers every setting category from the spec (§17) and the
//! `config.toml` format from Appendix A.

use serde::{Deserialize, Serialize};

use crate::export::ExportFormat;
use crate::filter::FilterPreset;
use crate::ui::{TabId, ViewMode};

// ── Top-level ──────────────────────────────────────────────────────

/// Complete task-manager configuration, serializable to/from TOML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskManagerConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub processes: ProcessesConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub app_history: AppHistoryConfig,
    #[serde(default)]
    pub startup: StartupConfig,
    #[serde(default)]
    pub services: ServicesConfig,
    #[serde(default)]
    pub files_in_use: FilesInUseConfig,
    #[serde(default)]
    pub unlock: UnlockConfig,
    #[serde(default)]
    pub network_traffic: NetworkTrafficConfig,
    #[serde(default)]
    pub energy: EnergyConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub keyboard: KeyboardConfig,
    #[serde(default)]
    pub accessibility: AccessibilityConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
    #[serde(default)]
    pub system_events: SystemEventsConfig,
}

// ── §17.1  General ─────────────────────────────────────────────────

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub default_tab: TabId,
    pub view_mode: ViewMode,
    pub always_on_top: bool,
    pub minimize_to_tray: bool,
    pub show_tray_icon: TrayIconMode,
    pub confirm_end_task: bool,
    pub show_full_account_name: bool,
    pub show_full_path_title: bool,
    pub remember_window_position: bool,
    pub remember_column_layout: bool,
    pub start_minimized: bool,
    pub launch_at_login: bool,
    pub language: String,
    pub date_time_format: DateTimeFormat,
    pub number_format: NumberFormat,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_tab: TabId::Processes,
            view_mode: ViewMode::Standard,
            always_on_top: false,
            minimize_to_tray: false,
            show_tray_icon: TrayIconMode::WhenMinimized,
            confirm_end_task: true,
            show_full_account_name: false,
            show_full_path_title: false,
            remember_window_position: true,
            remember_column_layout: true,
            start_minimized: false,
            launch_at_login: false,
            language: "system".into(),
            date_time_format: DateTimeFormat::System,
            number_format: NumberFormat::System,
        }
    }
}

/// System-tray icon visibility mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayIconMode {
    On,
    Off,
    WhenMinimized,
}

/// Date/time presentation format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateTimeFormat {
    System,
    Iso8601,
    Custom,
}

/// Number presentation format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    System,
    CommaDot,
    DotComma,
    Plain,
}

// ── §17.2  Processes ───────────────────────────────────────────────

/// Processes tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessesConfig {
    pub visible_columns: Vec<String>,
    pub sort_column: String,
    pub sort_descending: bool,
    pub grouping: String,
    pub heat_map: bool,
    pub heat_map_intensity: Intensity,
    pub row_height: RowHeight,
    pub show_process_icons: bool,
    pub show_status_icons: bool,
    pub show_inline_graphs: InlineGraphMode,
    pub inline_graph_width_px: u32,
    pub highlight_new: bool,
    pub highlight_new_color: String,
    pub highlight_new_duration_ms: u32,
    pub highlight_ending: bool,
    pub highlight_ending_color: String,
    pub highlight_ending_duration_ms: u32,
    pub show_suspended: bool,
    pub show_system_processes: bool,
    pub show_service_host_details: bool,
    pub process_name_display: ProcessNameDisplay,
    pub pid_display: PidDisplay,
    pub memory_display_unit: DisplayUnit,
    pub rate_display_unit: DisplayUnit,
    pub cumulative_io: CumulativeIoMode,
    pub saved_filters: Vec<FilterPreset>,
}

impl Default for ProcessesConfig {
    fn default() -> Self {
        Self {
            visible_columns: vec![
                "name".into(),
                "pid".into(),
                "status".into(),
                "cpu_percent".into(),
                "mem_working".into(),
                "disk_read".into(),
                "disk_write".into(),
                "gpu_percent".into(),
                "cmdline".into(),
                "user".into(),
                "elevated".into(),
            ],
            sort_column: "cpu_percent".into(),
            sort_descending: true,
            grouping: "type".into(),
            heat_map: false,
            heat_map_intensity: Intensity::Medium,
            row_height: RowHeight::Normal,
            show_process_icons: true,
            show_status_icons: true,
            show_inline_graphs: InlineGraphMode::Off,
            inline_graph_width_px: 100,
            highlight_new: true,
            highlight_new_color: "#4caf50".into(),
            highlight_new_duration_ms: 3000,
            highlight_ending: true,
            highlight_ending_color: "#ef5350".into(),
            highlight_ending_duration_ms: 2000,
            show_suspended: true,
            show_system_processes: true,
            show_service_host_details: true,
            process_name_display: ProcessNameDisplay::FriendlyName,
            pid_display: PidDisplay::Decimal,
            memory_display_unit: DisplayUnit::Auto,
            rate_display_unit: DisplayUnit::Auto,
            cumulative_io: CumulativeIoMode::SinceProcessStart,
            saved_filters: Vec::new(),
        }
    }
}

/// Intensity level for heat maps and similar features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    Low,
    Medium,
    High,
}

/// Process table row height preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowHeight {
    Compact,
    Normal,
    Comfortable,
}

/// Inline graph column mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InlineGraphMode {
    Off,
    Cpu,
    Memory,
    Disk,
    Gpu,
    All,
}

/// How the process name is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessNameDisplay {
    ExecutableName,
    FriendlyName,
    Both,
}

/// How PIDs are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PidDisplay {
    Decimal,
    Hexadecimal,
    Both,
}

/// Automatic or fixed display unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayUnit {
    Auto,
    Kb,
    Mb,
    Gb,
}

/// Cumulative I/O accounting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CumulativeIoMode {
    SinceProcessStart,
    SinceTabOpened,
    SinceLastReset,
}

// ── §17.3  Performance ─────────────────────────────────────────────

/// Performance tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub update_interval_ms: u32,
    pub graph_time_range_s: u32,
    pub graph_line_width: u8,
    pub graph_anti_aliasing: bool,
    pub graph_fill_opacity: u8,
    pub graph_background: GraphBackground,
    pub grid_line_style: GridLineStyle,
    pub grid_line_color: String,
    pub graph_interpolation: GraphInterpolation,
    pub show_axis_labels: bool,
    pub show_current_value: bool,
    pub show_min_max_avg: bool,
    pub cpu_graph_default: String,
    pub cpu_show_temperature: bool,
    pub cpu_show_clock_speed: bool,
    pub memory_show_composition: bool,
    pub disk_show_latency: bool,
    pub gpu_show_all_engines: bool,
    pub network_scale: NetworkScale,
    pub hardware_counters: bool,
    pub show_sidebar_sparklines: bool,
    pub colors: GraphColors,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            update_interval_ms: 1000,
            graph_time_range_s: 60,
            graph_line_width: 2,
            graph_anti_aliasing: true,
            graph_fill_opacity: 30,
            graph_background: GraphBackground::Grid,
            grid_line_style: GridLineStyle::Dashed,
            grid_line_color: "#333333".into(),
            graph_interpolation: GraphInterpolation::Bezier,
            show_axis_labels: true,
            show_current_value: true,
            show_min_max_avg: false,
            cpu_graph_default: "overall".into(),
            cpu_show_temperature: false,
            cpu_show_clock_speed: false,
            memory_show_composition: true,
            disk_show_latency: false,
            gpu_show_all_engines: false,
            network_scale: NetworkScale::Auto,
            hardware_counters: false,
            show_sidebar_sparklines: true,
            colors: GraphColors::default(),
        }
    }
}

/// Graph background style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphBackground {
    Solid,
    Grid,
    None,
}

/// Grid-line rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridLineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Line interpolation mode for performance graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphInterpolation {
    Linear,
    Bezier,
    Step,
}

/// Network graph Y-axis scaling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkScale {
    Auto,
    Fixed,
}

/// Per-metric graph colour palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphColors {
    pub cpu: String,
    pub cpu_kernel: String,
    pub cpu_user: String,
    pub memory_in_use: String,
    pub memory_standby: String,
    pub memory_modified: String,
    pub memory_free: String,
    pub disk_read: String,
    pub disk_write: String,
    pub gpu_3d: String,
    pub gpu_copy: String,
    pub gpu_decode: String,
    pub gpu_encode: String,
    pub gpu_compute: String,
    pub gpu_vram: String,
    pub network_send: String,
    pub network_recv: String,
    pub temperature: String,
    pub power: String,
}

impl Default for GraphColors {
    fn default() -> Self {
        Self {
            cpu: "#4fc3f7".into(),
            cpu_kernel: "#ef5350".into(),
            cpu_user: "#42a5f5".into(),
            memory_in_use: "#ab47bc".into(),
            memory_standby: "#66bb6a".into(),
            memory_modified: "#ffa726".into(),
            memory_free: "#bdbdbd".into(),
            disk_read: "#29b6f6".into(),
            disk_write: "#ef5350".into(),
            gpu_3d: "#66bb6a".into(),
            gpu_copy: "#ffa726".into(),
            gpu_decode: "#ab47bc".into(),
            gpu_encode: "#26c6da".into(),
            gpu_compute: "#ec407a".into(),
            gpu_vram: "#7e57c2".into(),
            network_send: "#42a5f5".into(),
            network_recv: "#ffa726".into(),
            temperature: "#ef5350".into(),
            power: "#ffee58".into(),
        }
    }
}

// ── §17.4  App History ─────────────────────────────────────────────

/// App history tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHistoryConfig {
    pub retention_days: u32,
    pub track_background: bool,
    pub track_system: bool,
    pub track_network_by_type: bool,
    pub default_time_range: String,
    pub auto_delete_uninstalled: bool,
    pub storage_location: Option<String>,
    pub max_db_size_mb: u32,
}

impl Default for AppHistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            track_background: true,
            track_system: false,
            track_network_by_type: true,
            default_time_range: "7d".into(),
            auto_delete_uninstalled: true,
            storage_location: None,
            max_db_size_mb: 100,
        }
    }
}

// ── §17.5  Startup ─────────────────────────────────────────────────

/// Startup tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupConfig {
    pub show_system_services: bool,
    pub measure_boot_impact: bool,
    pub boot_timeline_detail: DetailLevel,
    pub boot_history_days: u32,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            show_system_services: false,
            measure_boot_impact: false,
            boot_timeline_detail: DetailLevel::Standard,
            boot_history_days: 30,
        }
    }
}

/// Generic detail level used across several config sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Minimal,
    Standard,
    Detailed,
}

// ── §17.6  Services ────────────────────────────────────────────────

/// Services tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub show_running_only: bool,
    pub show_drivers: bool,
    pub show_dependency_viewer: DependencyViewerMode,
    pub group_by_status: bool,
    pub confirm_state_changes: bool,
}

impl Default for ServicesConfig {
    fn default() -> Self {
        Self {
            show_running_only: false,
            show_drivers: false,
            show_dependency_viewer: DependencyViewerMode::Dialog,
            group_by_status: false,
            confirm_state_changes: true,
        }
    }
}

/// How the service dependency viewer is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyViewerMode {
    Embedded,
    Dialog,
    Off,
}

// ── §17.7  Files In Use ────────────────────────────────────────────

/// Files-in-use tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesInUseConfig {
    pub refresh_interval_ms: u32,
    pub show_system: bool,
    pub show_kernel_handles: bool,
    pub max_results: u32,
    pub watch_notification: NotificationMode,
}

impl Default for FilesInUseConfig {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 5000,
            show_system: false,
            show_kernel_handles: false,
            max_results: 5000,
            watch_notification: NotificationMode::Toast,
        }
    }
}

/// How notifications are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationMode {
    Toast,
    Sound,
    Both,
    None,
}

// ── §17.8  Unlock ──────────────────────────────────────────────────

/// Resource unlocking settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockConfig {
    pub require_confirmation: String,
    pub auto_create_backup: bool,
    pub create_dump_before_kill: bool,
    pub audit_log_location: Option<String>,
    pub audit_log_max_mb: u32,
}

impl Default for UnlockConfig {
    fn default() -> Self {
        Self {
            require_confirmation: "always".into(),
            auto_create_backup: false,
            create_dump_before_kill: false,
            audit_log_location: None,
            audit_log_max_mb: 50,
        }
    }
}

// ── §17.9  Network Traffic ─────────────────────────────────────────

/// Network traffic tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTrafficConfig {
    pub default_view: String,
    pub show_reverse_dns: bool,
    pub show_geoip: bool,
    pub geoip_database: String,
    pub geoip_auto_update: bool,
    pub dns_query_logging: bool,
    pub dns_log_retention_hours: u32,
    pub protocol_detection: String,
    pub traffic_shaping: bool,
    pub packet_capture: bool,
    pub capture_buffer_mb: u32,
    pub capture_auto_delete: bool,
    pub bandwidth_quota_gb: u32,
    pub quota_reset_day: u8,
    pub network_map_discovery: String,
    pub show_firewall_rules: bool,
    pub connection_rate_limit_alert: u32,
    pub show_loopback: bool,
    pub resolve_port_names: bool,
    pub tls_certificate_display: String,
    pub data_usage_retention_days: u32,
    pub speed_test_server: String,
    pub metered_detection: String,
    pub traffic_classification: bool,
}

impl Default for NetworkTrafficConfig {
    fn default() -> Self {
        Self {
            default_view: "overview".into(),
            show_reverse_dns: true,
            show_geoip: true,
            geoip_database: "built-in".into(),
            geoip_auto_update: true,
            dns_query_logging: true,
            dns_log_retention_hours: 24,
            protocol_detection: "port-based".into(),
            traffic_shaping: false,
            packet_capture: false,
            capture_buffer_mb: 10,
            capture_auto_delete: true,
            bandwidth_quota_gb: 0,
            quota_reset_day: 1,
            network_map_discovery: "all".into(),
            show_firewall_rules: true,
            connection_rate_limit_alert: 0,
            show_loopback: false,
            resolve_port_names: true,
            tls_certificate_display: "subject".into(),
            data_usage_retention_days: 30,
            speed_test_server: "auto".into(),
            metered_detection: "auto".into(),
            traffic_classification: true,
        }
    }
}

// ── §17.10  Energy ─────────────────────────────────────────────────

/// Energy & power tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyConfig {
    pub default_view: String,
    pub power_estimation: String,
    pub temperature_unit: TemperatureUnit,
    pub fan_control: String,
    pub fan_profile: String,
    pub carbon_tracking: bool,
    pub carbon_source: String,
    pub electricity_rate_per_kwh: f64,
    pub energy_history_days: u32,
    pub battery_health_report: bool,
    pub battery_calibration_reminder: bool,
    pub wake_lock_alerts: bool,
    pub power_plan_quick_switch: bool,
    pub scheduled_profiles: bool,
    pub per_process_energy: bool,
    pub show_carbon_dashboard: bool,
    pub efficiency_scoring: bool,
    pub thermal_map_style: String,
    pub show_peripheral_power: bool,
    pub battery_cycle_warning: u32,
}

impl Default for EnergyConfig {
    fn default() -> Self {
        Self {
            default_view: "overview".into(),
            power_estimation: "auto".into(),
            temperature_unit: TemperatureUnit::Celsius,
            fan_control: "read-only".into(),
            fan_profile: "balanced".into(),
            carbon_tracking: false,
            carbon_source: "electricity-maps".into(),
            electricity_rate_per_kwh: 0.12,
            energy_history_days: 90,
            battery_health_report: true,
            battery_calibration_reminder: true,
            wake_lock_alerts: true,
            power_plan_quick_switch: true,
            scheduled_profiles: false,
            per_process_energy: true,
            show_carbon_dashboard: false,
            efficiency_scoring: true,
            thermal_map_style: "schematic".into(),
            show_peripheral_power: false,
            battery_cycle_warning: 500,
        }
    }
}

/// Temperature display unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

// ── §17.11  Audio ──────────────────────────────────────────────────

/// Audio tab settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub default_view: String,
    pub meter_type: String,
    pub meter_scale: String,
    pub meter_ballistics: String,
    pub peak_hold_time_s: u32,
    pub peak_hold_decay: String,
    pub spectrum_analyzer: bool,
    pub spectrum_fft_size: u32,
    pub spectrum_window: String,
    pub spectrum_mode: String,
    pub show_lufs: bool,
    pub show_waveform: bool,
    pub routing_view: String,
    pub virtual_audio_cables: bool,
    pub per_stream_volume: bool,
    pub per_stream_effects: bool,
    pub midi_monitoring: bool,
    pub event_logging: bool,
    pub event_log_retention_hours: u32,
    pub glitch_sensitivity: String,
    pub show_latency: bool,
    pub show_dsp_load: bool,
    pub test_tone_type: String,
    pub test_tone_frequency_hz: u32,
    pub test_tone_volume_dbfs: i32,
    pub show_spatial_controls: bool,
    pub show_midi: String,
    pub bluetooth_codec_display: bool,
    pub recording_format: String,
    pub recording_sample_rate: String,
    pub recording_bit_depth: u8,
    pub auto_switch_headphones: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            default_view: "output".into(),
            meter_type: "peak".into(),
            meter_scale: "dbfs".into(),
            meter_ballistics: "fast".into(),
            peak_hold_time_s: 3,
            peak_hold_decay: "gradual".into(),
            spectrum_analyzer: false,
            spectrum_fft_size: 4096,
            spectrum_window: "hann".into(),
            spectrum_mode: "1/3-octave".into(),
            show_lufs: false,
            show_waveform: false,
            routing_view: "diagram".into(),
            virtual_audio_cables: false,
            per_stream_volume: true,
            per_stream_effects: false,
            midi_monitoring: false,
            event_logging: true,
            event_log_retention_hours: 24,
            glitch_sensitivity: "medium".into(),
            show_latency: true,
            show_dsp_load: true,
            test_tone_type: "sine".into(),
            test_tone_frequency_hz: 1000,
            test_tone_volume_dbfs: -20,
            show_spatial_controls: true,
            show_midi: "auto".into(),
            bluetooth_codec_display: true,
            recording_format: "wav".into(),
            recording_sample_rate: "device".into(),
            recording_bit_depth: 24,
            auto_switch_headphones: true,
        }
    }
}

// ── §17.12  Notifications ──────────────────────────────────────────

/// Global notification thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub high_cpu_threshold: u8,
    pub high_memory_threshold: u8,
    pub high_disk_threshold: u8,
    pub high_gpu_threshold: u8,
    pub high_temp_threshold: u16,
    pub process_crash_alert: bool,
    pub service_failure_alert: bool,
    pub new_process_alert: bool,
    pub alert_sound: AlertSound,
    pub alert_method: AlertMethod,
    pub alert_cooldown_s: u32,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            high_cpu_threshold: 0,
            high_memory_threshold: 0,
            high_disk_threshold: 0,
            high_gpu_threshold: 0,
            high_temp_threshold: 0,
            process_crash_alert: false,
            service_failure_alert: false,
            new_process_alert: false,
            alert_sound: AlertSound::None,
            alert_method: AlertMethod::StatusBar,
            alert_cooldown_s: 30,
        }
    }
}

/// Alert sound source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSound {
    None,
    System,
    Custom,
}

/// How alerts are presented to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertMethod {
    Toast,
    StatusBar,
    Dialog,
    All,
}

// ── §17.13  Export ──────────────────────────────────────────────────

/// Data export settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub include_headers: bool,
    pub timestamp_format: TimestampFormat,
    pub decimal_separator: char,
    pub encoding: String,
    pub auto_export_interval: Option<u32>,
    pub auto_export_path: Option<String>,
    pub include_performance_snapshots: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            include_headers: true,
            timestamp_format: TimestampFormat::Iso8601,
            decimal_separator: '.',
            encoding: "utf-8".into(),
            auto_export_interval: None,
            auto_export_path: None,
            include_performance_snapshots: false,
        }
    }
}

/// Timestamp serialization format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    Iso8601,
    Unix,
    Local,
}

// ── §17.14  Plugins ────────────────────────────────────────────────

/// Plugin subsystem settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub enabled: bool,
    pub directory: Option<String>,
    pub auto_update: bool,
    pub sandboxing: SandboxLevel,
    pub show_tabs: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: None,
            auto_update: true,
            sandboxing: SandboxLevel::Strict,
            show_tabs: true,
        }
    }
}

/// Plugin sandbox strictness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxLevel {
    Strict,
    Permissive,
    Off,
}

// ── §17.15  Keyboard ───────────────────────────────────────────────

/// Keyboard & mouse settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardConfig {
    pub shortcut_scheme: ShortcutScheme,
    pub single_click: ClickAction,
    pub double_click: DoubleClickAction,
    pub middle_click: MiddleClickAction,
    pub scroll_on_graph: ScrollGraphAction,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            shortcut_scheme: ShortcutScheme::Default,
            single_click: ClickAction::Select,
            double_click: DoubleClickAction::Properties,
            middle_click: MiddleClickAction::None,
            scroll_on_graph: ScrollGraphAction::Zoom,
        }
    }
}

/// Keyboard shortcut scheme preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutScheme {
    Default,
    Custom,
    Vim,
    Emacs,
}

/// Single-click behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickAction {
    Select,
    Expand,
}

/// Double-click behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickAction {
    Properties,
    OpenFileLocation,
    None,
}

/// Middle-click behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiddleClickAction {
    EndTask,
    None,
}

/// Scroll wheel on graph behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollGraphAction {
    Zoom,
    Scroll,
}

// ── §17.16  Accessibility ──────────────────────────────────────────

/// Accessibility settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    pub high_contrast: HighContrastMode,
    pub font_size: u8,
    pub font_family: String,
    pub screen_reader_level: ScreenReaderLevel,
    pub reduce_motion: bool,
    pub color_blind_mode: ColorBlindMode,
    pub focus_indicator: FocusIndicatorStyle,
    pub tab_navigation: TabNavMode,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            high_contrast: HighContrastMode::System,
            font_size: 12,
            font_family: "system".into(),
            screen_reader_level: ScreenReaderLevel::Standard,
            reduce_motion: false,
            color_blind_mode: ColorBlindMode::Off,
            focus_indicator: FocusIndicatorStyle::Default,
            tab_navigation: TabNavMode::Standard,
        }
    }
}

/// High-contrast theme behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighContrastMode {
    System,
    ForceOn,
    ForceOff,
}

/// Screen-reader announcement verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenReaderLevel {
    Minimal,
    Standard,
    Verbose,
}

/// Colour-blind assistance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorBlindMode {
    Off,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

/// Keyboard focus indicator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusIndicatorStyle {
    Default,
    HighVisibility,
}

/// Tab-key navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabNavMode {
    Standard,
    GraphNavigation,
}

// ── §17.17  Advanced ───────────────────────────────────────────────

/// Advanced / internal settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    pub sampling_rate_hz: f64,
    pub ring_buffer_size: u32,
    pub enable_etw: bool,
    pub debug_logging: LogLevel,
    pub log_file_path: Option<String>,
    pub max_log_file_size_mb: u32,
    pub process_scan_method: ScanMethod,
    pub gpu_backend: String,
    pub temp_source: String,
    pub show_debug_tab: bool,
    pub enable_perf_counters: bool,
    pub hardware_counter_access: String,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            sampling_rate_hz: 1.0,
            ring_buffer_size: 300,
            enable_etw: false,
            debug_logging: LogLevel::Off,
            log_file_path: None,
            max_log_file_size_mb: 50,
            process_scan_method: ScanMethod::Incremental,
            gpu_backend: "auto".into(),
            temp_source: "auto".into(),
            show_debug_tab: false,
            enable_perf_counters: true,
            hardware_counter_access: "auto".into(),
        }
    }
}

/// Debug log verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Off,
    Info,
    Debug,
    Trace,
}

/// Process scanning strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMethod {
    Snapshot,
    Incremental,
}

// ── §17.18  System Events ──────────────────────────────────────────

/// System event viewer settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEventsConfig {
    /// Default log source view on tab open.
    pub default_view: String,
    /// Maximum number of events to load at startup.
    pub max_events_loaded: u32,
    /// Auto-refresh interval in milliseconds (0 = off).
    pub auto_refresh_ms: u32,
    /// Whether to show verbose-level events by default.
    pub show_verbose: bool,
    /// Whether to show informational events by default.
    pub show_information: bool,
    /// Whether to show warning events by default.
    pub show_warnings: bool,
    /// Whether to show error events by default.
    pub show_errors: bool,
    /// Whether to show critical events by default.
    pub show_critical: bool,
    /// Enable desktop notifications for critical events.
    pub notify_critical: bool,
    /// Enable desktop notifications for error events.
    pub notify_errors: bool,
    /// Date range in hours to load on initial view (0 = all).
    pub default_hours_range: u32,
    /// Whether to resolve SIDs to user names.
    pub resolve_sids: bool,
}

impl Default for SystemEventsConfig {
    fn default() -> Self {
        Self {
            default_view: "all".into(),
            max_events_loaded: 10000,
            auto_refresh_ms: 5000,
            show_verbose: false,
            show_information: true,
            show_warnings: true,
            show_errors: true,
            show_critical: true,
            notify_critical: true,
            notify_errors: false,
            default_hours_range: 24,
            resolve_sids: true,
        }
    }
}
