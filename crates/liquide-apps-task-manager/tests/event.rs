//! Tests for `event` module types.

use liquide_apps_task_manager::event::*;

// ---------------------------------------------------------------------------
// TaskManagerEvent – all variants constructible
// ---------------------------------------------------------------------------

#[test]
fn event_process_created() {
    let e = TaskManagerEvent::ProcessCreated {
        pid: 1234,
        name: "test".into(),
    };
    assert_eq!(e.as_str(), "Process Created");
}

#[test]
fn event_process_exited() {
    let e = TaskManagerEvent::ProcessExited {
        pid: 1234,
        exit_code: 0,
    };
    assert_eq!(e.as_str(), "Process Exited");
}

#[test]
fn event_process_cpu_spike() {
    let e = TaskManagerEvent::ProcessCpuSpike {
        pid: 1234,
        percent: 95.0,
    };
    assert_eq!(e.as_str(), "Process CPU Spike");
}

#[test]
fn event_process_memory_spike() {
    let e = TaskManagerEvent::ProcessMemorySpike {
        pid: 1234,
        bytes: 1024 * 1024 * 1024,
    };
    assert_eq!(e.as_str(), "Process Memory Spike");
}

#[test]
fn event_process_not_responding() {
    let e = TaskManagerEvent::ProcessNotResponding { pid: 1234 };
    assert_eq!(e.as_str(), "Process Not Responding");
}

#[test]
fn event_process_resumed() {
    let e = TaskManagerEvent::ProcessResumed { pid: 1234 };
    assert_eq!(e.as_str(), "Process Resumed");
}

#[test]
fn event_cpu_threshold_exceeded() {
    let e = TaskManagerEvent::CpuThresholdExceeded { percent: 90.0 };
    assert_eq!(e.as_str(), "CPU Threshold Exceeded");
}

#[test]
fn event_memory_threshold_exceeded() {
    let e = TaskManagerEvent::MemoryThresholdExceeded { percent: 85.0 };
    assert_eq!(e.as_str(), "Memory Threshold Exceeded");
}

#[test]
fn event_disk_threshold_exceeded() {
    let e = TaskManagerEvent::DiskThresholdExceeded { percent: 95.0 };
    assert_eq!(e.as_str(), "Disk Threshold Exceeded");
}

#[test]
fn event_gpu_threshold_exceeded() {
    let e = TaskManagerEvent::GpuThresholdExceeded { percent: 99.0 };
    assert_eq!(e.as_str(), "GPU Threshold Exceeded");
}

#[test]
fn event_network_threshold_exceeded() {
    let e = TaskManagerEvent::NetworkThresholdExceeded {
        bytes_sec: 1_000_000,
    };
    assert_eq!(e.as_str(), "Network Threshold Exceeded");
}

#[test]
fn event_service_started() {
    let e = TaskManagerEvent::ServiceStarted {
        name: "sshd".into(),
    };
    assert_eq!(e.as_str(), "Service Started");
}

#[test]
fn event_service_stopped() {
    let e = TaskManagerEvent::ServiceStopped {
        name: "sshd".into(),
    };
    assert_eq!(e.as_str(), "Service Stopped");
}

#[test]
fn event_service_failed() {
    let e = TaskManagerEvent::ServiceFailed {
        name: "sshd".into(),
        error: "crash".into(),
    };
    assert_eq!(e.as_str(), "Service Failed");
}

#[test]
fn event_device_connected() {
    let e = TaskManagerEvent::DeviceConnected {
        device_id: "dev-1".into(),
    };
    assert_eq!(e.as_str(), "Device Connected");
}

#[test]
fn event_device_disconnected() {
    let e = TaskManagerEvent::DeviceDisconnected {
        device_id: "dev-1".into(),
    };
    assert_eq!(e.as_str(), "Device Disconnected");
}

#[test]
fn event_device_error() {
    let e = TaskManagerEvent::DeviceError {
        device_id: "dev-1".into(),
        error: "fail".into(),
    };
    assert_eq!(e.as_str(), "Device Error");
}

#[test]
fn event_user_logged_in() {
    let e = TaskManagerEvent::UserLoggedIn {
        username: "alice".into(),
    };
    assert_eq!(e.as_str(), "User Logged In");
}

#[test]
fn event_user_logged_out() {
    let e = TaskManagerEvent::UserLoggedOut {
        username: "alice".into(),
    };
    assert_eq!(e.as_str(), "User Logged Out");
}

#[test]
fn event_session_locked() {
    let e = TaskManagerEvent::SessionLocked { session_id: 1 };
    assert_eq!(e.as_str(), "Session Locked");
}

#[test]
fn event_session_unlocked() {
    let e = TaskManagerEvent::SessionUnlocked { session_id: 1 };
    assert_eq!(e.as_str(), "Session Unlocked");
}

#[test]
fn event_file_locked() {
    let e = TaskManagerEvent::FileLocked {
        path: "/tmp/test".into(),
        pid: 1234,
    };
    assert_eq!(e.as_str(), "File Locked");
}

#[test]
fn event_file_unlocked() {
    let e = TaskManagerEvent::FileUnlocked {
        path: "/tmp/test".into(),
    };
    assert_eq!(e.as_str(), "File Unlocked");
}

#[test]
fn event_network_connection_opened() {
    let e = TaskManagerEvent::NetworkConnectionOpened {
        pid: 1234,
        remote: "1.2.3.4:443".into(),
    };
    assert_eq!(e.as_str(), "Network Connection Opened");
}

#[test]
fn event_network_connection_closed() {
    let e = TaskManagerEvent::NetworkConnectionClosed {
        pid: 1234,
        remote: "1.2.3.4:443".into(),
    };
    assert_eq!(e.as_str(), "Network Connection Closed");
}

#[test]
fn event_dns_query_blocked() {
    let e = TaskManagerEvent::DnsQueryBlocked {
        domain: "ads.example.com".into(),
    };
    assert_eq!(e.as_str(), "DNS Query Blocked");
}

#[test]
fn event_firewall_rule_triggered() {
    let e = TaskManagerEvent::FirewallRuleTriggered {
        rule_name: "Block SSH".into(),
    };
    assert_eq!(e.as_str(), "Firewall Rule Triggered");
}

#[test]
fn event_battery_low() {
    let e = TaskManagerEvent::BatteryLow { percent: 5.0 };
    assert_eq!(e.as_str(), "Battery Low");
}

#[test]
fn event_battery_charging() {
    let e = TaskManagerEvent::BatteryCharging;
    assert_eq!(e.as_str(), "Battery Charging");
}

#[test]
fn event_battery_discharging() {
    let e = TaskManagerEvent::BatteryDischarging;
    assert_eq!(e.as_str(), "Battery Discharging");
}

#[test]
fn event_power_source_changed() {
    let e = TaskManagerEvent::PowerSourceChanged {
        source: "AC".into(),
    };
    assert_eq!(e.as_str(), "Power Source Changed");
}

#[test]
fn event_thermal_warning() {
    let e = TaskManagerEvent::ThermalWarning {
        sensor: "CPU".into(),
        celsius: 85.0,
    };
    assert_eq!(e.as_str(), "Thermal Warning");
}

#[test]
fn event_thermal_critical() {
    let e = TaskManagerEvent::ThermalCritical {
        sensor: "CPU".into(),
        celsius: 100.0,
    };
    assert_eq!(e.as_str(), "Thermal Critical");
}

#[test]
fn event_fan_speed_changed() {
    let e = TaskManagerEvent::FanSpeedChanged {
        fan: "CPU Fan".into(),
        rpm: 3000,
    };
    assert_eq!(e.as_str(), "Fan Speed Changed");
}

#[test]
fn event_audio_device_added() {
    let e = TaskManagerEvent::AudioDeviceAdded {
        device_id: "dev-1".into(),
    };
    assert_eq!(e.as_str(), "Audio Device Added");
}

#[test]
fn event_audio_device_removed() {
    let e = TaskManagerEvent::AudioDeviceRemoved {
        device_id: "dev-1".into(),
    };
    assert_eq!(e.as_str(), "Audio Device Removed");
}

#[test]
fn event_audio_glitch() {
    let e = TaskManagerEvent::AudioGlitch {
        device_id: "dev-1".into(),
    };
    assert_eq!(e.as_str(), "Audio Glitch");
}

#[test]
fn event_volume_changed() {
    let e = TaskManagerEvent::VolumeChanged {
        device_id: "dev-1".into(),
        percent: 75.0,
    };
    assert_eq!(e.as_str(), "Volume Changed");
}

#[test]
fn event_plugin_loaded() {
    let e = TaskManagerEvent::PluginLoaded {
        name: "my-plugin".into(),
    };
    assert_eq!(e.as_str(), "Plugin Loaded");
}

#[test]
fn event_plugin_unloaded() {
    let e = TaskManagerEvent::PluginUnloaded {
        name: "my-plugin".into(),
    };
    assert_eq!(e.as_str(), "Plugin Unloaded");
}

#[test]
fn event_config_changed() {
    let e = TaskManagerEvent::ConfigChanged {
        key: "general.always_on_top".into(),
    };
    assert_eq!(e.as_str(), "Config Changed");
}

// ---------------------------------------------------------------------------
// Display trait
// ---------------------------------------------------------------------------

#[test]
fn event_display_matches_as_str() {
    let e = TaskManagerEvent::ProcessCreated {
        pid: 1,
        name: "test".into(),
    };
    assert_eq!(e.to_string(), e.as_str());
}

// ---------------------------------------------------------------------------
// Serde roundtrip
// ---------------------------------------------------------------------------

#[test]
fn event_serde_roundtrip() {
    let e = TaskManagerEvent::ProcessCreated {
        pid: 1234,
        name: "firefox".into(),
    };
    let json = serde_json::to_string(&e).unwrap();
    let back: TaskManagerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), "Process Created");
}

#[test]
fn event_battery_charging_serde() {
    let e = TaskManagerEvent::BatteryCharging;
    let json = serde_json::to_string(&e).unwrap();
    let back: TaskManagerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), "Battery Charging");
}

// ---------------------------------------------------------------------------
// EventFilter
// ---------------------------------------------------------------------------

#[test]
fn event_filter_default() {
    let f = EventFilter::default();
    assert!(f.event_types.is_none());
    assert!(f.pids.is_none());
    assert!(f.min_severity.is_none());
}

#[test]
fn event_filter_construction() {
    let f = EventFilter {
        event_types: Some(vec!["process_created".into(), "process_exited".into()]),
        pids: Some(vec![1234, 5678]),
        min_severity: Some("warning".into()),
    };
    assert_eq!(f.event_types.as_ref().unwrap().len(), 2);
    assert_eq!(f.pids.as_ref().unwrap().len(), 2);
}

#[test]
fn event_filter_serde_roundtrip() {
    let f = EventFilter {
        event_types: Some(vec!["process_created".into()]),
        pids: None,
        min_severity: None,
    };
    let json = serde_json::to_string(&f).unwrap();
    let back: EventFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(back.event_types.unwrap().len(), 1);
}
