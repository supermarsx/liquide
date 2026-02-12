use liquide_apps_task_manager::process::{
    ConnectionSummary, GroupingMode, HandleInfo, IoPriority, ModuleInfo, ProcessAction,
    ProcessInfo, ProcessStatus, ProcessType, SchedulingPriority, ThreadInfo,
};

// ===========================================================================
// ProcessStatus (9 variants)
// ===========================================================================

#[test]
fn process_status_as_str_running() {
    assert_eq!(ProcessStatus::Running.as_str(), "Running");
}

#[test]
fn process_status_as_str_not_responding() {
    assert_eq!(ProcessStatus::NotResponding.as_str(), "Not Responding");
}

#[test]
fn process_status_as_str_disk_sleep() {
    assert_eq!(ProcessStatus::DiskSleep.as_str(), "Disk Sleep");
}

#[test]
fn process_status_display_all_variants() {
    let variants = [
        (ProcessStatus::Running, "Running"),
        (ProcessStatus::Sleeping, "Sleeping"),
        (ProcessStatus::Stopped, "Stopped"),
        (ProcessStatus::Zombie, "Zombie"),
        (ProcessStatus::Idle, "Idle"),
        (ProcessStatus::NotResponding, "Not Responding"),
        (ProcessStatus::Suspended, "Suspended"),
        (ProcessStatus::Waiting, "Waiting"),
        (ProcessStatus::DiskSleep, "Disk Sleep"),
    ];
    for (variant, expected) in &variants {
        assert_eq!(format!("{}", variant), *expected);
    }
}

#[test]
fn process_status_serde_roundtrip() {
    let status = ProcessStatus::NotResponding;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"not_responding\"");
    let back: ProcessStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, status);
}

#[test]
fn process_status_clone_eq() {
    let a = ProcessStatus::Zombie;
    let b = a;
    assert_eq!(a, b);
}

// ===========================================================================
// ProcessType (5 variants)
// ===========================================================================

#[test]
fn process_type_as_str_all() {
    assert_eq!(ProcessType::App.as_str(), "App");
    assert_eq!(ProcessType::Background.as_str(), "Background");
    assert_eq!(ProcessType::Service.as_str(), "Service");
    assert_eq!(ProcessType::System.as_str(), "System");
    assert_eq!(ProcessType::Shell.as_str(), "Shell");
}

#[test]
fn process_type_display() {
    assert_eq!(format!("{}", ProcessType::Shell), "Shell");
}

#[test]
fn process_type_serde_roundtrip() {
    let pt = ProcessType::Background;
    let json = serde_json::to_string(&pt).unwrap();
    assert_eq!(json, "\"background\"");
    let back: ProcessType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, pt);
}

// ===========================================================================
// GroupingMode (8 variants)
// ===========================================================================

#[test]
fn grouping_mode_as_str_all() {
    assert_eq!(GroupingMode::Type.as_str(), "Type");
    assert_eq!(GroupingMode::Status.as_str(), "Status");
    assert_eq!(GroupingMode::User.as_str(), "User");
    assert_eq!(GroupingMode::Session.as_str(), "Session");
    assert_eq!(GroupingMode::Priority.as_str(), "Priority");
    assert_eq!(GroupingMode::GpuAdapter.as_str(), "GPU Adapter");
    assert_eq!(GroupingMode::Package.as_str(), "Package");
    assert_eq!(GroupingMode::None.as_str(), "None");
}

#[test]
fn grouping_mode_display() {
    assert_eq!(format!("{}", GroupingMode::GpuAdapter), "GPU Adapter");
}

#[test]
fn grouping_mode_serde_roundtrip() {
    let gm = GroupingMode::GpuAdapter;
    let json = serde_json::to_string(&gm).unwrap();
    assert_eq!(json, "\"gpu_adapter\"");
    let back: GroupingMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gm);
}

// ===========================================================================
// SchedulingPriority (6 variants)
// ===========================================================================

#[test]
fn scheduling_priority_default_is_normal() {
    let p = SchedulingPriority::default();
    assert_eq!(p, SchedulingPriority::Normal);
}

#[test]
fn scheduling_priority_as_str_all() {
    assert_eq!(SchedulingPriority::Realtime.as_str(), "Realtime");
    assert_eq!(SchedulingPriority::High.as_str(), "High");
    assert_eq!(SchedulingPriority::AboveNormal.as_str(), "Above Normal");
    assert_eq!(SchedulingPriority::Normal.as_str(), "Normal");
    assert_eq!(SchedulingPriority::BelowNormal.as_str(), "Below Normal");
    assert_eq!(SchedulingPriority::Idle.as_str(), "Idle");
}

#[test]
fn scheduling_priority_display() {
    assert_eq!(format!("{}", SchedulingPriority::AboveNormal), "Above Normal");
}

#[test]
fn scheduling_priority_serde_roundtrip() {
    let sp = SchedulingPriority::BelowNormal;
    let json = serde_json::to_string(&sp).unwrap();
    assert_eq!(json, "\"below_normal\"");
    let back: SchedulingPriority = serde_json::from_str(&json).unwrap();
    assert_eq!(back, sp);
}

// ===========================================================================
// IoPriority (5 variants)
// ===========================================================================

#[test]
fn io_priority_default_is_normal() {
    assert_eq!(IoPriority::default(), IoPriority::Normal);
}

#[test]
fn io_priority_as_str_all() {
    assert_eq!(IoPriority::Critical.as_str(), "Critical");
    assert_eq!(IoPriority::High.as_str(), "High");
    assert_eq!(IoPriority::Normal.as_str(), "Normal");
    assert_eq!(IoPriority::Low.as_str(), "Low");
    assert_eq!(IoPriority::VeryLow.as_str(), "Very Low");
}

#[test]
fn io_priority_display() {
    assert_eq!(format!("{}", IoPriority::VeryLow), "Very Low");
}

#[test]
fn io_priority_serde_roundtrip() {
    let iop = IoPriority::Critical;
    let json = serde_json::to_string(&iop).unwrap();
    assert_eq!(json, "\"critical\"");
    let back: IoPriority = serde_json::from_str(&json).unwrap();
    assert_eq!(back, iop);
}

// ===========================================================================
// ProcessAction
// ===========================================================================

#[test]
fn process_action_as_str_simple_variants() {
    assert_eq!(ProcessAction::EndTask.as_str(), "End Task");
    assert_eq!(ProcessAction::Restart.as_str(), "Restart");
    assert_eq!(ProcessAction::RunAsAdmin.as_str(), "Run as Administrator");
    assert_eq!(ProcessAction::CopyPid.as_str(), "Copy PID");
    assert_eq!(ProcessAction::CopyCmdLine.as_str(), "Copy Command Line");
}

#[test]
fn process_action_as_str_with_data() {
    assert_eq!(
        ProcessAction::SetPriority(SchedulingPriority::High).as_str(),
        "Set Priority"
    );
    assert_eq!(ProcessAction::SetAffinity(0xFF).as_str(), "Set Affinity");
    assert_eq!(
        ProcessAction::SetIoPriority(IoPriority::Low).as_str(),
        "Set I/O Priority"
    );
}

#[test]
fn process_action_display() {
    assert_eq!(
        format!("{}", ProcessAction::GenerateStackTrace),
        "Generate Stack Trace"
    );
}

#[test]
fn process_action_serde_roundtrip_simple() {
    let action = ProcessAction::EndTask;
    let json = serde_json::to_string(&action).unwrap();
    let back: ProcessAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, action);
}

#[test]
fn process_action_serde_roundtrip_with_priority() {
    let action = ProcessAction::SetPriority(SchedulingPriority::Realtime);
    let json = serde_json::to_string(&action).unwrap();
    let back: ProcessAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, action);
}

// ===========================================================================
// ProcessInfo (Default + serde)
// ===========================================================================

#[test]
fn process_info_default_identity_fields() {
    let info = ProcessInfo::default();
    assert_eq!(info.name, "");
    assert_eq!(info.pid, 0);
    assert!(info.ppid.is_none());
    assert_eq!(info.status, ProcessStatus::Running);
    assert_eq!(info.proc_type, ProcessType::App);
    assert_eq!(info.user, "");
}

#[test]
fn process_info_default_cpu_fields() {
    let info = ProcessInfo::default();
    assert!((info.cpu_percent - 0.0).abs() < f64::EPSILON);
    assert_eq!(info.cpu_time_ms, 0);
    assert_eq!(info.threads, 0);
    assert_eq!(info.priority, SchedulingPriority::Normal);
    assert_eq!(info.affinity, 0);
}

#[test]
fn process_info_default_misc_booleans() {
    let info = ProcessInfo::default();
    assert!(!info.dep_enabled);
    assert!(!info.aslr_enabled);
    assert!(!info.cfg_enabled);
    assert!(!info.elevated);
    assert!(!info.sandboxed);
}

#[test]
fn process_info_serde_roundtrip_key_fields() {
    let mut info = ProcessInfo::default();
    info.name = "firefox".to_string();
    info.pid = 1234;
    info.status = ProcessStatus::Sleeping;
    info.cpu_percent = 12.5;
    info.mem_working_bytes = 1_048_576;
    info.elevated = true;

    let json = serde_json::to_string(&info).unwrap();
    let back: ProcessInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(back.name, "firefox");
    assert_eq!(back.pid, 1234);
    assert_eq!(back.status, ProcessStatus::Sleeping);
    assert!((back.cpu_percent - 12.5).abs() < f64::EPSILON);
    assert_eq!(back.mem_working_bytes, 1_048_576);
    assert!(back.elevated);
}

// ===========================================================================
// ThreadInfo construction + serde
// ===========================================================================

#[test]
fn thread_info_construction_and_serde() {
    let ti = ThreadInfo {
        tid: 42,
        state: "Running".to_string(),
        cpu_percent: 5.5,
        cpu_time_ms: 10_000,
        priority: 8,
        start_address: Some("main".to_string()),
        wait_reason: None,
        ideal_processor: Some(3),
        stack_size_bytes: Some(65536),
    };
    let json = serde_json::to_string(&ti).unwrap();
    let back: ThreadInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.tid, 42);
    assert_eq!(back.state, "Running");
    assert!((back.cpu_percent - 5.5).abs() < f64::EPSILON);
    assert_eq!(back.priority, 8);
    assert_eq!(back.start_address.as_deref(), Some("main"));
    assert_eq!(back.ideal_processor, Some(3));
    assert_eq!(back.stack_size_bytes, Some(65536));
}

// ===========================================================================
// HandleInfo construction + serde
// ===========================================================================

#[test]
fn handle_info_construction_and_serde() {
    let hi = HandleInfo {
        handle: 0x1234,
        handle_type: "File".to_string(),
        name: Some("/dev/null".to_string()),
        access: 0x1F,
    };
    let json = serde_json::to_string(&hi).unwrap();
    let back: HandleInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.handle, 0x1234);
    assert_eq!(back.handle_type, "File");
    assert_eq!(back.name.as_deref(), Some("/dev/null"));
    assert_eq!(back.access, 0x1F);
}

// ===========================================================================
// ModuleInfo construction + serde
// ===========================================================================

#[test]
fn module_info_construction_and_serde() {
    let mi = ModuleInfo {
        name: "libc.so.6".to_string(),
        path: "/usr/lib/libc.so.6".to_string(),
        base_address: 0x7FFF_0000_0000,
        size_bytes: 2_000_000,
        version: Some("2.31".to_string()),
        publisher: Some("GNU".to_string()),
        description: Some("C standard library".to_string()),
    };
    let json = serde_json::to_string(&mi).unwrap();
    let back: ModuleInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "libc.so.6");
    assert_eq!(back.path, "/usr/lib/libc.so.6");
    assert_eq!(back.base_address, 0x7FFF_0000_0000);
    assert_eq!(back.size_bytes, 2_000_000);
    assert_eq!(back.version.as_deref(), Some("2.31"));
    assert_eq!(back.publisher.as_deref(), Some("GNU"));
}

// ===========================================================================
// ConnectionSummary construction + serde
// ===========================================================================

#[test]
fn connection_summary_construction_and_serde() {
    let cs = ConnectionSummary {
        protocol: "TCP".to_string(),
        local_address: "127.0.0.1:8080".to_string(),
        remote_address: "93.184.216.34:443".to_string(),
        state: "ESTABLISHED".to_string(),
        bytes_sent: 4096,
        bytes_received: 65536,
    };
    let json = serde_json::to_string(&cs).unwrap();
    let back: ConnectionSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(back.protocol, "TCP");
    assert_eq!(back.local_address, "127.0.0.1:8080");
    assert_eq!(back.remote_address, "93.184.216.34:443");
    assert_eq!(back.state, "ESTABLISHED");
    assert_eq!(back.bytes_sent, 4096);
    assert_eq!(back.bytes_received, 65536);
}
