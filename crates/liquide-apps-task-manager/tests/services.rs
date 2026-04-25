use liquide_apps_task_manager::services::{
    RecoveryAction, ServiceAction, ServiceInfo, ServiceStartupType, ServiceStatus, ServiceType,
};

// ===========================================================================
// ServiceStatus (7 variants)
// ===========================================================================

#[test]
fn service_status_as_str_all() {
    assert_eq!(ServiceStatus::Running.as_str(), "Running");
    assert_eq!(ServiceStatus::Stopped.as_str(), "Stopped");
    assert_eq!(ServiceStatus::Paused.as_str(), "Paused");
    assert_eq!(ServiceStatus::StartPending.as_str(), "Start Pending");
    assert_eq!(ServiceStatus::StopPending.as_str(), "Stop Pending");
    assert_eq!(ServiceStatus::PausePending.as_str(), "Pause Pending");
    assert_eq!(ServiceStatus::ContinuePending.as_str(), "Continue Pending");
}

#[test]
fn service_status_display() {
    assert_eq!(
        format!("{}", ServiceStatus::ContinuePending),
        "Continue Pending"
    );
}

#[test]
fn service_status_serde_roundtrip() {
    let s = ServiceStatus::StartPending;
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(json, "\"start_pending\"");
    let back: ServiceStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn service_status_clone_eq() {
    let a = ServiceStatus::Paused;
    let b = a;
    assert_eq!(a, b);
}

// ===========================================================================
// ServiceStartupType (5 variants)
// ===========================================================================

#[test]
fn service_startup_type_as_str_all() {
    assert_eq!(ServiceStartupType::Automatic.as_str(), "Automatic");
    assert_eq!(
        ServiceStartupType::AutomaticDelayed.as_str(),
        "Automatic (Delayed)"
    );
    assert_eq!(ServiceStartupType::Manual.as_str(), "Manual");
    assert_eq!(ServiceStartupType::Disabled.as_str(), "Disabled");
    assert_eq!(ServiceStartupType::Boot.as_str(), "Boot");
}

#[test]
fn service_startup_type_display() {
    assert_eq!(
        format!("{}", ServiceStartupType::AutomaticDelayed),
        "Automatic (Delayed)"
    );
}

#[test]
fn service_startup_type_serde_roundtrip() {
    let st = ServiceStartupType::AutomaticDelayed;
    let json = serde_json::to_string(&st).unwrap();
    assert_eq!(json, "\"automatic_delayed\"");
    let back: ServiceStartupType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, st);
}

// ===========================================================================
// ServiceType (5 variants)
// ===========================================================================

#[test]
fn service_type_as_str_all() {
    assert_eq!(ServiceType::KernelDriver.as_str(), "Kernel Driver");
    assert_eq!(ServiceType::FileSystemDriver.as_str(), "File System Driver");
    assert_eq!(ServiceType::OwnProcess.as_str(), "Own Process");
    assert_eq!(ServiceType::ShareProcess.as_str(), "Share Process");
    assert_eq!(ServiceType::Interactive.as_str(), "Interactive");
}

#[test]
fn service_type_display() {
    assert_eq!(
        format!("{}", ServiceType::FileSystemDriver),
        "File System Driver"
    );
}

#[test]
fn service_type_serde_roundtrip() {
    let st = ServiceType::KernelDriver;
    let json = serde_json::to_string(&st).unwrap();
    assert_eq!(json, "\"kernel_driver\"");
    let back: ServiceType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, st);
}

// ===========================================================================
// ServiceAction (6 variants)
// ===========================================================================

#[test]
fn service_action_as_str_all() {
    assert_eq!(ServiceAction::Start.as_str(), "Start");
    assert_eq!(ServiceAction::Stop.as_str(), "Stop");
    assert_eq!(ServiceAction::Pause.as_str(), "Pause");
    assert_eq!(ServiceAction::Resume.as_str(), "Resume");
    assert_eq!(ServiceAction::Restart.as_str(), "Restart");
    assert_eq!(ServiceAction::Configure.as_str(), "Configure");
}

#[test]
fn service_action_display() {
    assert_eq!(format!("{}", ServiceAction::Configure), "Configure");
}

#[test]
fn service_action_serde_roundtrip() {
    let a = ServiceAction::Restart;
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(json, "\"restart\"");
    let back: ServiceAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, a);
}

// ===========================================================================
// RecoveryAction (construction + serde)
// ===========================================================================

#[test]
fn recovery_action_construction_and_serde() {
    let ra = RecoveryAction {
        action: "restart_service".to_string(),
        delay_ms: 5000,
        command: None,
    };
    let json = serde_json::to_string(&ra).unwrap();
    let back: RecoveryAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back.action, "restart_service");
    assert_eq!(back.delay_ms, 5000);
    assert!(back.command.is_none());
}

#[test]
fn recovery_action_with_command() {
    let ra = RecoveryAction {
        action: "run_program".to_string(),
        delay_ms: 10_000,
        command: Some("/usr/bin/notify-admin".to_string()),
    };
    let json = serde_json::to_string(&ra).unwrap();
    let back: RecoveryAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back.action, "run_program");
    assert_eq!(back.delay_ms, 10_000);
    assert_eq!(back.command.as_deref(), Some("/usr/bin/notify-admin"));
}

// ===========================================================================
// ServiceInfo (default + serde)
// ===========================================================================

#[test]
fn service_info_default_key_fields() {
    let info = ServiceInfo::default();
    assert_eq!(info.name, "");
    assert_eq!(info.display_name, "");
    assert_eq!(info.status, ServiceStatus::Stopped);
    assert_eq!(info.startup_type, ServiceStartupType::Manual);
    assert_eq!(info.service_type, ServiceType::OwnProcess);
    assert!(info.pid.is_none());
    assert_eq!(info.account, "");
    assert!(info.dependencies.is_empty());
    assert!(info.dependent_services.is_empty());
    assert_eq!(info.error_control, "Normal");
}

#[test]
fn service_info_default_resource_fields() {
    let info = ServiceInfo::default();
    assert!((info.cpu_percent - 0.0).abs() < f64::EPSILON);
    assert_eq!(info.mem_bytes, 0);
    assert_eq!(info.disk_read_bytes_sec, 0);
    assert_eq!(info.disk_write_bytes_sec, 0);
    assert_eq!(info.handles, 0);
    assert_eq!(info.threads, 0);
}

#[test]
fn service_info_default_recovery_fields() {
    let info = ServiceInfo::default();
    assert!(info.recovery_first.is_none());
    assert!(info.recovery_second.is_none());
    assert!(info.recovery_subsequent.is_none());
    assert!(info.reset_period_secs.is_none());
    assert!(info.load_order_group.is_none());
    assert!(info.start_time.is_none());
}

#[test]
fn service_info_serde_roundtrip() {
    let mut info = ServiceInfo::default();
    info.name = "sshd".to_string();
    info.display_name = "OpenSSH Server".to_string();
    info.status = ServiceStatus::Running;
    info.startup_type = ServiceStartupType::Automatic;
    info.pid = Some(1234);
    info.binary_path = "/usr/sbin/sshd".to_string();
    info.account = "root".to_string();
    info.dependencies = vec!["network.target".to_string()];
    info.recovery_first = Some(RecoveryAction {
        action: "restart_service".to_string(),
        delay_ms: 3000,
        command: None,
    });

    let json = serde_json::to_string(&info).unwrap();
    let back: ServiceInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(back.name, "sshd");
    assert_eq!(back.display_name, "OpenSSH Server");
    assert_eq!(back.status, ServiceStatus::Running);
    assert_eq!(back.startup_type, ServiceStartupType::Automatic);
    assert_eq!(back.pid, Some(1234));
    assert_eq!(back.binary_path, "/usr/sbin/sshd");
    assert_eq!(back.account, "root");
    assert_eq!(back.dependencies.len(), 1);
    assert_eq!(back.dependencies[0], "network.target");
    assert_eq!(
        back.recovery_first.as_ref().unwrap().action,
        "restart_service"
    );
    assert_eq!(back.recovery_first.as_ref().unwrap().delay_ms, 3000);
}
