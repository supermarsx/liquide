use liquide_apps_task_manager::app_history::{AppHistoryEntry, TimePeriod};

// ===========================================================================
// TimePeriod (5 variants)
// ===========================================================================

#[test]
fn time_period_as_str_today() {
    assert_eq!(TimePeriod::Today.as_str(), "Today");
}

#[test]
fn time_period_as_str_yesterday() {
    assert_eq!(TimePeriod::Yesterday.as_str(), "Yesterday");
}

#[test]
fn time_period_as_str_last_week() {
    assert_eq!(TimePeriod::LastWeek.as_str(), "Last Week");
}

#[test]
fn time_period_as_str_last_month() {
    assert_eq!(TimePeriod::LastMonth.as_str(), "Last Month");
}

#[test]
fn time_period_as_str_all_time() {
    assert_eq!(TimePeriod::AllTime.as_str(), "All Time");
}

#[test]
fn time_period_display_all() {
    assert_eq!(format!("{}", TimePeriod::Today), "Today");
    assert_eq!(format!("{}", TimePeriod::Yesterday), "Yesterday");
    assert_eq!(format!("{}", TimePeriod::LastWeek), "Last Week");
    assert_eq!(format!("{}", TimePeriod::LastMonth), "Last Month");
    assert_eq!(format!("{}", TimePeriod::AllTime), "All Time");
}

#[test]
fn time_period_serde_roundtrip_today() {
    let tp = TimePeriod::Today;
    let json = serde_json::to_string(&tp).unwrap();
    assert_eq!(json, "\"today\"");
    let back: TimePeriod = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tp);
}

#[test]
fn time_period_serde_roundtrip_last_week() {
    let tp = TimePeriod::LastWeek;
    let json = serde_json::to_string(&tp).unwrap();
    assert_eq!(json, "\"last_week\"");
    let back: TimePeriod = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tp);
}

#[test]
fn time_period_serde_roundtrip_all_time() {
    let tp = TimePeriod::AllTime;
    let json = serde_json::to_string(&tp).unwrap();
    assert_eq!(json, "\"all_time\"");
    let back: TimePeriod = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tp);
}

#[test]
fn time_period_clone_eq() {
    let a = TimePeriod::LastMonth;
    let b = a;
    assert_eq!(a, b);
}

// ===========================================================================
// AppHistoryEntry (default)
// ===========================================================================

#[test]
fn app_history_entry_default_name_empty() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.name, "");
}

#[test]
fn app_history_entry_default_optional_fields_none() {
    let entry = AppHistoryEntry::default();
    assert!(entry.publisher.is_none());
    assert!(entry.power_usage_avg.is_none());
    assert!(entry.last_used.is_none());
    assert!(entry.first_seen.is_none());
}

#[test]
fn app_history_entry_default_cpu_fields_zero() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.cpu_time_total_ms, 0);
    assert_eq!(entry.cpu_time_foreground_ms, 0);
}

#[test]
fn app_history_entry_default_network_fields_zero() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.network_bytes_total, 0);
    assert_eq!(entry.network_bytes_foreground, 0);
    assert_eq!(entry.metered_network_bytes, 0);
}

#[test]
fn app_history_entry_default_gpu_fields_zero() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.gpu_time_ms, 0);
    assert_eq!(entry.gpu_dedicated_bytes_peak, 0);
    assert_eq!(entry.gpu_shared_bytes_peak, 0);
}

#[test]
fn app_history_entry_default_disk_fields_zero() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.disk_read_total_bytes, 0);
    assert_eq!(entry.disk_write_total_bytes, 0);
}

#[test]
fn app_history_entry_default_counters_zero() {
    let entry = AppHistoryEntry::default();
    assert_eq!(entry.tile_updates, 0);
    assert_eq!(entry.notifications_sent, 0);
    assert_eq!(entry.launch_count, 0);
}

// ===========================================================================
// AppHistoryEntry (serde roundtrip)
// ===========================================================================

#[test]
fn app_history_entry_serde_roundtrip_minimal() {
    let entry = AppHistoryEntry::default();
    let json = serde_json::to_string(&entry).unwrap();
    let back: AppHistoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "");
    assert_eq!(back.cpu_time_total_ms, 0);
    assert_eq!(back.launch_count, 0);
}

#[test]
fn app_history_entry_serde_roundtrip_populated() {
    let mut entry = AppHistoryEntry::default();
    entry.name = "Visual Studio Code".to_string();
    entry.publisher = Some("Microsoft".to_string());
    entry.cpu_time_total_ms = 3_600_000;
    entry.cpu_time_foreground_ms = 2_400_000;
    entry.network_bytes_total = 50_000_000;
    entry.network_bytes_foreground = 40_000_000;
    entry.metered_network_bytes = 1_000_000;
    entry.tile_updates = 10;
    entry.notifications_sent = 25;
    entry.gpu_time_ms = 120_000;
    entry.gpu_dedicated_bytes_peak = 512_000_000;
    entry.gpu_shared_bytes_peak = 128_000_000;
    entry.disk_read_total_bytes = 2_000_000_000;
    entry.disk_write_total_bytes = 500_000_000;
    entry.power_usage_avg = Some("Moderate".to_string());
    entry.launch_count = 42;
    entry.last_used = Some("2026-02-12T10:00:00Z".to_string());
    entry.first_seen = Some("2025-01-01T00:00:00Z".to_string());

    let json = serde_json::to_string(&entry).unwrap();
    let back: AppHistoryEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(back.name, "Visual Studio Code");
    assert_eq!(back.publisher.as_deref(), Some("Microsoft"));
    assert_eq!(back.cpu_time_total_ms, 3_600_000);
    assert_eq!(back.cpu_time_foreground_ms, 2_400_000);
    assert_eq!(back.network_bytes_total, 50_000_000);
    assert_eq!(back.metered_network_bytes, 1_000_000);
    assert_eq!(back.tile_updates, 10);
    assert_eq!(back.notifications_sent, 25);
    assert_eq!(back.gpu_time_ms, 120_000);
    assert_eq!(back.gpu_dedicated_bytes_peak, 512_000_000);
    assert_eq!(back.disk_read_total_bytes, 2_000_000_000);
    assert_eq!(back.disk_write_total_bytes, 500_000_000);
    assert_eq!(back.power_usage_avg.as_deref(), Some("Moderate"));
    assert_eq!(back.launch_count, 42);
    assert_eq!(back.last_used.as_deref(), Some("2026-02-12T10:00:00Z"));
    assert_eq!(back.first_seen.as_deref(), Some("2025-01-01T00:00:00Z"));
}

#[test]
fn app_history_entry_serde_json_contains_expected_keys() {
    let mut entry = AppHistoryEntry::default();
    entry.name = "test-app".to_string();
    let json = serde_json::to_string(&entry).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v.get("name").is_some());
    assert!(v.get("cpu_time_total_ms").is_some());
    assert!(v.get("network_bytes_total").is_some());
    assert!(v.get("gpu_time_ms").is_some());
    assert!(v.get("launch_count").is_some());
    assert!(v.get("publisher").is_some());
}
