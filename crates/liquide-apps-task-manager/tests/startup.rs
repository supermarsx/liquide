use liquide_apps_task_manager::startup::{
    BootHistoryEntry, BootPhase, ShutdownType, StartupEntry, StartupImpact, StartupType,
};

// ===========================================================================
// StartupImpact (5 variants)
// ===========================================================================

#[test]
fn startup_impact_as_str_all() {
    assert_eq!(StartupImpact::High.as_str(), "High");
    assert_eq!(StartupImpact::Medium.as_str(), "Medium");
    assert_eq!(StartupImpact::Low.as_str(), "Low");
    assert_eq!(StartupImpact::None.as_str(), "None");
    assert_eq!(StartupImpact::NotMeasured.as_str(), "Not Measured");
}

#[test]
fn startup_impact_display() {
    assert_eq!(format!("{}", StartupImpact::NotMeasured), "Not Measured");
    assert_eq!(format!("{}", StartupImpact::High), "High");
}

#[test]
fn startup_impact_serde_roundtrip() {
    let si = StartupImpact::Medium;
    let json = serde_json::to_string(&si).unwrap();
    assert_eq!(json, "\"medium\"");
    let back: StartupImpact = serde_json::from_str(&json).unwrap();
    assert_eq!(back, si);
}

#[test]
fn startup_impact_serde_none_variant() {
    let si = StartupImpact::None;
    let json = serde_json::to_string(&si).unwrap();
    assert_eq!(json, "\"none\"");
    let back: StartupImpact = serde_json::from_str(&json).unwrap();
    assert_eq!(back, si);
}

#[test]
fn startup_impact_clone_eq() {
    let a = StartupImpact::Low;
    let b = a;
    assert_eq!(a, b);
}

// ===========================================================================
// StartupType (4 variants)
// ===========================================================================

#[test]
fn startup_type_as_str_all() {
    assert_eq!(StartupType::Registry.as_str(), "Registry");
    assert_eq!(StartupType::Folder.as_str(), "Folder");
    assert_eq!(StartupType::Task.as_str(), "Task");
    assert_eq!(StartupType::Service.as_str(), "Service");
}

#[test]
fn startup_type_display() {
    assert_eq!(format!("{}", StartupType::Registry), "Registry");
    assert_eq!(format!("{}", StartupType::Task), "Task");
}

#[test]
fn startup_type_serde_roundtrip() {
    let st = StartupType::Folder;
    let json = serde_json::to_string(&st).unwrap();
    assert_eq!(json, "\"folder\"");
    let back: StartupType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, st);
}

#[test]
fn startup_type_serde_all_variants() {
    for variant in [
        StartupType::Registry,
        StartupType::Folder,
        StartupType::Task,
        StartupType::Service,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: StartupType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

// ===========================================================================
// BootPhase (4 variants)
// ===========================================================================

#[test]
fn boot_phase_as_str_all() {
    assert_eq!(BootPhase::PreBoot.as_str(), "Pre-Boot");
    assert_eq!(BootPhase::Boot.as_str(), "Boot");
    assert_eq!(BootPhase::PostBoot.as_str(), "Post-Boot");
    assert_eq!(BootPhase::Login.as_str(), "Login");
}

#[test]
fn boot_phase_display() {
    assert_eq!(format!("{}", BootPhase::PreBoot), "Pre-Boot");
    assert_eq!(format!("{}", BootPhase::PostBoot), "Post-Boot");
}

#[test]
fn boot_phase_serde_roundtrip() {
    let bp = BootPhase::PostBoot;
    let json = serde_json::to_string(&bp).unwrap();
    assert_eq!(json, "\"post_boot\"");
    let back: BootPhase = serde_json::from_str(&json).unwrap();
    assert_eq!(back, bp);
}

// ===========================================================================
// ShutdownType (4 variants)
// ===========================================================================

#[test]
fn shutdown_type_as_str_all() {
    assert_eq!(ShutdownType::NormalShutdown.as_str(), "Normal Shutdown");
    assert_eq!(ShutdownType::NormalReboot.as_str(), "Normal Reboot");
    assert_eq!(ShutdownType::UnexpectedShutdown.as_str(), "Unexpected Shutdown");
    assert_eq!(ShutdownType::Bsod.as_str(), "BSOD");
}

#[test]
fn shutdown_type_display() {
    assert_eq!(format!("{}", ShutdownType::Bsod), "BSOD");
    assert_eq!(
        format!("{}", ShutdownType::UnexpectedShutdown),
        "Unexpected Shutdown"
    );
}

#[test]
fn shutdown_type_serde_roundtrip() {
    let st = ShutdownType::Bsod;
    let json = serde_json::to_string(&st).unwrap();
    assert_eq!(json, "\"bsod\"");
    let back: ShutdownType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, st);
}

#[test]
fn shutdown_type_serde_all_variants() {
    for variant in [
        ShutdownType::NormalShutdown,
        ShutdownType::NormalReboot,
        ShutdownType::UnexpectedShutdown,
        ShutdownType::Bsod,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: ShutdownType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

// ===========================================================================
// StartupEntry (default + serde)
// ===========================================================================

#[test]
fn startup_entry_default_key_fields() {
    let entry = StartupEntry::default();
    assert_eq!(entry.name, "");
    assert!(entry.publisher.is_none());
    assert_eq!(entry.command, "");
    assert_eq!(entry.startup_type, StartupType::Registry);
    assert!(entry.status_enabled);
    assert_eq!(entry.impact, StartupImpact::NotMeasured);
    assert_eq!(entry.boot_phase, BootPhase::Login);
}

#[test]
fn startup_entry_default_impact_fields() {
    let entry = StartupEntry::default();
    assert_eq!(entry.disk_impact_bytes, 0);
    assert_eq!(entry.cpu_impact_ms, 0);
    assert_eq!(entry.startup_delay_ms, 0);
}

#[test]
fn startup_entry_default_optional_fields() {
    let entry = StartupEntry::default();
    assert!(entry.last_disabled.is_none());
    assert!(entry.file_path.is_none());
    assert!(entry.file_size_bytes.is_none());
    assert!(entry.digital_signature.is_none());
    assert!(entry.description.is_none());
}

#[test]
fn startup_entry_serde_roundtrip() {
    let mut entry = StartupEntry::default();
    entry.name = "OneDrive".to_string();
    entry.publisher = Some("Microsoft".to_string());
    entry.command = "C:\\Program Files\\OneDrive\\OneDrive.exe /background".to_string();
    entry.startup_type = StartupType::Registry;
    entry.status_enabled = true;
    entry.impact = StartupImpact::High;
    entry.boot_phase = BootPhase::Login;
    entry.disk_impact_bytes = 50_000_000;
    entry.cpu_impact_ms = 2500;
    entry.startup_delay_ms = 4000;
    entry.file_path = Some("C:\\Program Files\\OneDrive\\OneDrive.exe".to_string());
    entry.file_size_bytes = Some(3_200_000);
    entry.digital_signature = Some("Signed".to_string());
    entry.description = Some("Cloud sync utility".to_string());

    let json = serde_json::to_string(&entry).unwrap();
    let back: StartupEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(back.name, "OneDrive");
    assert_eq!(back.publisher.as_deref(), Some("Microsoft"));
    assert_eq!(back.startup_type, StartupType::Registry);
    assert!(back.status_enabled);
    assert_eq!(back.impact, StartupImpact::High);
    assert_eq!(back.boot_phase, BootPhase::Login);
    assert_eq!(back.disk_impact_bytes, 50_000_000);
    assert_eq!(back.cpu_impact_ms, 2500);
    assert_eq!(back.startup_delay_ms, 4000);
    assert_eq!(
        back.file_path.as_deref(),
        Some("C:\\Program Files\\OneDrive\\OneDrive.exe")
    );
    assert_eq!(back.file_size_bytes, Some(3_200_000));
    assert_eq!(back.digital_signature.as_deref(), Some("Signed"));
    assert_eq!(back.description.as_deref(), Some("Cloud sync utility"));
}

// ===========================================================================
// BootHistoryEntry (default + serde)
// ===========================================================================

#[test]
fn boot_history_entry_default_key_fields() {
    let entry = BootHistoryEntry::default();
    assert_eq!(entry.date, "");
    assert_eq!(entry.boot_time_ms, 0);
    assert_eq!(entry.shutdown_type, ShutdownType::NormalShutdown);
    assert_eq!(entry.pre_boot_ms, 0);
    assert_eq!(entry.boot_ms, 0);
    assert_eq!(entry.post_boot_ms, 0);
    assert_eq!(entry.login_ms, 0);
    assert_eq!(entry.total_ms, 0);
}

#[test]
fn boot_history_entry_serde_roundtrip() {
    let entry = BootHistoryEntry {
        date: "2026-02-12T08:00:00Z".to_string(),
        boot_time_ms: 15_000,
        shutdown_type: ShutdownType::NormalReboot,
        pre_boot_ms: 3_000,
        boot_ms: 5_000,
        post_boot_ms: 4_000,
        login_ms: 3_000,
        total_ms: 15_000,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: BootHistoryEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(back.date, "2026-02-12T08:00:00Z");
    assert_eq!(back.boot_time_ms, 15_000);
    assert_eq!(back.shutdown_type, ShutdownType::NormalReboot);
    assert_eq!(back.pre_boot_ms, 3_000);
    assert_eq!(back.boot_ms, 5_000);
    assert_eq!(back.post_boot_ms, 4_000);
    assert_eq!(back.login_ms, 3_000);
    assert_eq!(back.total_ms, 15_000);
}

#[test]
fn boot_history_entry_serde_with_bsod() {
    let entry = BootHistoryEntry {
        date: "2026-01-15T03:22:00Z".to_string(),
        boot_time_ms: 45_000,
        shutdown_type: ShutdownType::Bsod,
        pre_boot_ms: 5_000,
        boot_ms: 20_000,
        post_boot_ms: 10_000,
        login_ms: 10_000,
        total_ms: 45_000,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: BootHistoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.shutdown_type, ShutdownType::Bsod);
    assert_eq!(back.total_ms, 45_000);
}
