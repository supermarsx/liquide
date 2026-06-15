//! Tests for the liquide-supervisor crate.

use crate::admission::{AdmissionController, AdmissionDecision, HostResources};
use crate::audit::AuditLevel;
use crate::config::{AdmissionConfig, DowngradeThresholds, ResourceDefaults, SupervisorConfig};
use crate::crash::{CrashCategory, CrashHandler};
use crate::downgrade::{DowngradeLevel, DowngradeManager};
use crate::heartbeat::{HeartbeatConfig, HeartbeatState, HeartbeatTracker};
use crate::ipc::{ControlCommand, ControlResponse};
use crate::resource::{HostMetrics, ResourceMonitor, ResourceSeverity, ResourceSnapshot};
use crate::restart::{RestartDecision, RestartPolicy};
use crate::runtime::SupervisorRuntime;
use crate::session::{ResourceBudget, SessionRecord, SessionRegistry, SessionState};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_runtime() -> SupervisorRuntime {
    let mut rt = SupervisorRuntime::new(
        SupervisorConfig::default(),
        ResourceDefaults::default(),
        AdmissionConfig::default(),
        DowngradeThresholds::default(),
        RestartPolicy::default(),
        16.0,  // 16-core host
        32768, // 32 GB
    );
    // Use a harmless, long-lived stand-in process so spawn_session launches a
    // real OS child (proving the spawn path) without depending on the
    // liquid-session binary being installed in the test environment.
    rt.set_spawn_command(sleeper_command());
    rt
}

/// A cross-platform command that starts a process which stays alive for a
/// while and accepts no per-session arguments.
fn sleeper_command() -> crate::spawn::SpawnCommand {
    #[cfg(windows)]
    {
        // `ping` with a count keeps the process alive for several seconds and
        // does not exit immediately.
        crate::spawn::SpawnCommand {
            program: "cmd".to_string(),
            base_args: vec![
                "/c".to_string(),
                "ping".to_string(),
                "-n".to_string(),
                "30".to_string(),
                "127.0.0.1".to_string(),
            ],
            append_session_args: false,
        }
    }
    #[cfg(not(windows))]
    {
        crate::spawn::SpawnCommand {
            program: "sleep".to_string(),
            base_args: vec!["30".to_string()],
            append_session_args: false,
        }
    }
}

fn make_budget() -> ResourceBudget {
    ResourceBudget::default()
}

fn make_admission(total_cpu: f64, total_mem: u64) -> AdmissionController {
    AdmissionController::new(
        AdmissionConfig::default(),
        HostResources::new(total_cpu, total_mem),
    )
}

// ===========================================================================
// Session registry
// ===========================================================================

#[test]
fn test_registry_new_is_empty() {
    let reg = SessionRegistry::new();
    assert_eq!(reg.total_count(), 0);
    assert_eq!(reg.active_count(), 0);
}

#[test]
fn test_registry_default_is_empty() {
    let reg = SessionRegistry::default();
    assert_eq!(reg.total_count(), 0);
}

#[test]
fn test_registry_register_and_get() {
    let mut reg = SessionRegistry::new();
    let record = SessionRecord::new(
        "sess-1".to_string(),
        "alice".to_string(),
        1000,
        make_budget(),
    );
    reg.register_session(record);

    assert_eq!(reg.total_count(), 1);
    let s = reg.get_session("sess-1").unwrap();
    assert_eq!(s.user, "alice");
    assert_eq!(s.pid, 1000);
    assert_eq!(s.state, SessionState::Spawning);
}

#[test]
fn test_registry_remove_session() {
    let mut reg = SessionRegistry::new();
    reg.register_session(SessionRecord::new(
        "sess-1".to_string(),
        "alice".to_string(),
        1000,
        make_budget(),
    ));
    assert_eq!(reg.total_count(), 1);

    let removed = reg.remove_session("sess-1");
    assert!(removed.is_some());
    assert_eq!(reg.total_count(), 0);
    assert!(reg.get_session("sess-1").is_none());
}

#[test]
fn test_registry_remove_nonexistent() {
    let mut reg = SessionRegistry::new();
    assert!(reg.remove_session("nope").is_none());
}

#[test]
fn test_registry_active_count_excludes_terminated() {
    let mut reg = SessionRegistry::new();

    let mut r1 = SessionRecord::new("s1".into(), "alice".into(), 1, make_budget());
    r1.state = SessionState::Running;
    reg.register_session(r1);

    let mut r2 = SessionRecord::new("s2".into(), "bob".into(), 2, make_budget());
    r2.state = SessionState::Terminated;
    reg.register_session(r2);

    let mut r3 = SessionRecord::new("s3".into(), "carol".into(), 3, make_budget());
    r3.state = SessionState::Failed;
    reg.register_session(r3);

    assert_eq!(reg.total_count(), 3);
    assert_eq!(reg.active_count(), 1);
}

#[test]
fn test_registry_sessions_for_user() {
    let mut reg = SessionRegistry::new();
    reg.register_session(SessionRecord::new(
        "s1".into(),
        "alice".into(),
        1,
        make_budget(),
    ));
    reg.register_session(SessionRecord::new(
        "s2".into(),
        "bob".into(),
        2,
        make_budget(),
    ));
    reg.register_session(SessionRecord::new(
        "s3".into(),
        "alice".into(),
        3,
        make_budget(),
    ));

    let alice = reg.sessions_for_user("alice");
    assert_eq!(alice.len(), 2);
    assert!(alice.contains(&"s1".to_string()));
    assert!(alice.contains(&"s3".to_string()));

    let bob = reg.sessions_for_user("bob");
    assert_eq!(bob.len(), 1);

    let charlie = reg.sessions_for_user("charlie");
    assert!(charlie.is_empty());
}

#[test]
fn test_registry_get_session_mut() {
    let mut reg = SessionRegistry::new();
    reg.register_session(SessionRecord::new(
        "s1".into(),
        "alice".into(),
        1,
        make_budget(),
    ));

    {
        let s = reg.get_session_mut("s1").unwrap();
        s.state = SessionState::Running;
    }

    assert_eq!(reg.get_session("s1").unwrap().state, SessionState::Running);
}

#[test]
fn test_registry_all_sessions() {
    let mut reg = SessionRegistry::new();
    reg.register_session(SessionRecord::new(
        "s1".into(),
        "a".into(),
        1,
        make_budget(),
    ));
    reg.register_session(SessionRecord::new(
        "s2".into(),
        "b".into(),
        2,
        make_budget(),
    ));

    let all = reg.all_sessions();
    assert_eq!(all.len(), 2);
    assert!(all.contains_key("s1"));
    assert!(all.contains_key("s2"));
}

// ===========================================================================
// Admission controller
// ===========================================================================

#[test]
fn test_admission_accepts_when_resources_available() {
    let ctrl = make_admission(16.0, 32768);
    let budget = make_budget();
    assert_eq!(ctrl.check_admission(&budget), AdmissionDecision::Accepted);
}

#[test]
fn test_admission_rejects_insufficient_cpu() {
    // 4 CPU cores, 2 reserved = 2 available. Budget wants 2, but after accounting for
    // the reserved cores there should be exactly 2 available (the default reserved is 2).
    // So we need a host that after reservation has < budget.
    let host = HostResources {
        total_cpu_cores: 3.0,
        total_memory_mb: 32768,
        available_cpu: 0.5, // Only 0.5 available after other sessions.
        available_memory: 32768,
    };
    let ctrl = AdmissionController::new(AdmissionConfig::default(), host);
    let budget = make_budget(); // needs 2.0 cores

    match ctrl.check_admission(&budget) {
        AdmissionDecision::Rejected { reason } => {
            assert!(reason.contains("insufficient CPU"));
        }
        other => panic!("expected Rejected, got {:?}", other),
    }
}

#[test]
fn test_admission_rejects_insufficient_memory() {
    let host = HostResources {
        total_cpu_cores: 16.0,
        total_memory_mb: 32768,
        available_cpu: 16.0,
        available_memory: 100, // Only 100 MB available.
    };
    let ctrl = AdmissionController::new(AdmissionConfig::default(), host);
    let budget = make_budget(); // needs 512 MB

    match ctrl.check_admission(&budget) {
        AdmissionDecision::Rejected { reason } => {
            assert!(reason.contains("insufficient memory"));
        }
        other => panic!("expected Rejected, got {:?}", other),
    }
}

#[test]
fn test_admission_queues_when_enabled() {
    let config = AdmissionConfig {
        queue_enabled: true,
        ..AdmissionConfig::default()
    };
    let host = HostResources {
        total_cpu_cores: 16.0,
        total_memory_mb: 32768,
        available_cpu: 0.5,
        available_memory: 32768,
    };
    let ctrl = AdmissionController::new(config, host);
    let budget = make_budget();

    match ctrl.check_admission(&budget) {
        AdmissionDecision::Queued { position } => {
            assert_eq!(position, 1);
        }
        other => panic!("expected Queued, got {:?}", other),
    }
}

#[test]
fn test_admission_disabled_always_accepts() {
    let config = AdmissionConfig {
        enabled: false,
        ..AdmissionConfig::default()
    };
    let host = HostResources {
        total_cpu_cores: 1.0,
        total_memory_mb: 256,
        available_cpu: 0.0,
        available_memory: 0,
    };
    let ctrl = AdmissionController::new(config, host);
    let budget = make_budget();

    assert_eq!(ctrl.check_admission(&budget), AdmissionDecision::Accepted);
}

#[test]
fn test_admission_can_accept_4k() {
    let ctrl = make_admission(16.0, 32768);
    assert!(ctrl.can_accept_4k()); // 16 >= 8

    let small = make_admission(4.0, 32768);
    assert!(!small.can_accept_4k()); // 4 < 8
}

#[test]
fn test_admission_can_accept_60fps() {
    let ctrl = make_admission(16.0, 32768);
    assert!(ctrl.can_accept_60fps()); // 16 >= 4

    let small = make_admission(2.0, 32768);
    assert!(!small.can_accept_60fps()); // 2 < 4
}

#[test]
fn test_admission_compute_available_resources() {
    let mut ctrl = make_admission(16.0, 32768);

    let mut r1 = SessionRecord::new("s1".into(), "a".into(), 1, make_budget());
    r1.state = SessionState::Running;
    let mut r2 = SessionRecord::new("s2".into(), "b".into(), 2, make_budget());
    r2.state = SessionState::Running;
    let mut r3 = SessionRecord::new("s3".into(), "c".into(), 3, make_budget());
    r3.state = SessionState::Terminated; // Should not count.

    let sessions: Vec<&SessionRecord> = vec![&r1, &r2, &r3];
    ctrl.compute_available_resources(&sessions);

    // 16 - 2 reserved - 2*2 running = 10.0
    let h = ctrl.host_resources();
    assert!((h.available_cpu - 10.0).abs() < 0.001);
    // 32768 - 1024 reserved - 2*512 running = 30720
    assert_eq!(h.available_memory, 30720);
}

// ===========================================================================
// Heartbeat tracking
// ===========================================================================

#[test]
fn test_heartbeat_tracker_register_and_unregister() {
    let mut tracker = HeartbeatTracker::new(HeartbeatConfig::default());
    tracker.register("sess-1".to_string());
    assert_eq!(tracker.tracked_count(), 1);

    tracker.unregister("sess-1");
    assert_eq!(tracker.tracked_count(), 0);
}

#[test]
fn test_heartbeat_tracker_record_heartbeat() {
    let mut tracker = HeartbeatTracker::new(HeartbeatConfig::default());
    tracker.register("sess-1".to_string());

    // Simulate a tick (increments missed count).
    let alerts = tracker.check_all();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].state, HeartbeatState::Warning);

    // Record heartbeat resets missed count.
    tracker.record_heartbeat("sess-1");
    let entry = tracker.get_entry("sess-1").unwrap();
    assert_eq!(entry.missed_count, 0);
    assert_eq!(entry.state, HeartbeatState::Healthy);
}

#[test]
fn test_heartbeat_tracker_timeout() {
    let config = HeartbeatConfig {
        interval_sec: 5,
        timeout_count: 3,
    };
    let mut tracker = HeartbeatTracker::new(config);
    tracker.register("sess-1".to_string());

    // Three ticks without heartbeat should trigger timeout.
    tracker.check_all(); // missed = 1 -> Warning
    tracker.check_all(); // missed = 2 -> Warning
    let alerts = tracker.check_all(); // missed = 3 -> TimedOut

    let timed_out: Vec<_> = alerts
        .iter()
        .filter(|a| a.state == HeartbeatState::TimedOut)
        .collect();
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out[0].session_id, "sess-1");
    assert_eq!(timed_out[0].missed_count, 3);
}

#[test]
fn test_heartbeat_tracker_recovery() {
    let config = HeartbeatConfig {
        interval_sec: 5,
        timeout_count: 3,
    };
    let mut tracker = HeartbeatTracker::new(config);
    tracker.register("sess-1".to_string());

    tracker.check_all(); // missed = 1
    tracker.check_all(); // missed = 2
    tracker.record_heartbeat("sess-1");

    let entry = tracker.get_entry("sess-1").unwrap();
    assert_eq!(entry.missed_count, 0);
    assert_eq!(entry.state, HeartbeatState::Healthy);
}

#[test]
fn test_heartbeat_tracker_multiple_sessions() {
    let mut tracker = HeartbeatTracker::new(HeartbeatConfig {
        interval_sec: 5,
        timeout_count: 2,
    });
    tracker.register("s1".to_string());
    tracker.register("s2".to_string());

    tracker.check_all(); // Both get missed = 1
    tracker.record_heartbeat("s1"); // s1 resets, s2 stays at 1

    let alerts = tracker.check_all(); // s1 missed = 1, s2 missed = 2 (timeout)

    let timed_out: Vec<_> = alerts
        .iter()
        .filter(|a| a.state == HeartbeatState::TimedOut)
        .collect();
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out[0].session_id, "s2");
}

#[test]
fn test_heartbeat_timeout_count() {
    let tracker = HeartbeatTracker::new(HeartbeatConfig {
        interval_sec: 5,
        timeout_count: 7,
    });
    assert_eq!(tracker.timeout_count(), 7);
}

// ===========================================================================
// Restart policy
// ===========================================================================

#[test]
fn test_restart_policy_first_restart_is_immediate() {
    let policy = RestartPolicy::default();
    let decision = policy.evaluate(0);
    assert_eq!(decision, RestartDecision::RestartNow);
}

#[test]
fn test_restart_policy_second_restart_has_delay() {
    let policy = RestartPolicy::default();
    let decision = policy.evaluate(1);
    match decision {
        RestartDecision::RestartAfterDelay { delay_ms } => {
            assert_eq!(delay_ms, 1000); // base * 2^0
        }
        other => panic!("expected RestartAfterDelay, got {:?}", other),
    }
}

#[test]
fn test_restart_policy_backoff_doubles() {
    let policy = RestartPolicy::default();

    // restart_count=1 -> delay = 1000 * 2^0 = 1000
    assert_eq!(policy.compute_delay(1), 1000);
    // restart_count=2 -> delay = 1000 * 2^1 = 2000
    assert_eq!(policy.compute_delay(2), 2000);
    // restart_count=3 -> delay = 1000 * 2^2 = 4000
    assert_eq!(policy.compute_delay(3), 4000);
    // restart_count=4 -> delay = 1000 * 2^3 = 8000
    assert_eq!(policy.compute_delay(4), 8000);
}

#[test]
fn test_restart_policy_zero_delay_for_first() {
    let policy = RestartPolicy::default();
    assert_eq!(policy.compute_delay(0), 0);
}

#[test]
fn test_restart_policy_exceeds_max_enters_failed() {
    let policy = RestartPolicy::default(); // max_restarts = 5
    let decision = policy.evaluate(5);
    match decision {
        RestartDecision::EnterFailed { reason } => {
            assert!(reason.contains("exceeded maximum restarts"));
        }
        other => panic!("expected EnterFailed, got {:?}", other),
    }
}

#[test]
fn test_restart_policy_safe_mode_threshold() {
    let policy = RestartPolicy::default(); // safe_mode_threshold = 3
    assert!(!policy.should_enter_safe_mode(2));
    assert!(policy.should_enter_safe_mode(3));
    assert!(policy.should_enter_safe_mode(4));
}

#[test]
fn test_restart_policy_custom() {
    let policy = RestartPolicy::new(3, 300, 500, 2);
    assert_eq!(policy.max_restarts, 3);
    assert_eq!(policy.window_sec, 300);
    assert_eq!(policy.backoff_base_ms, 500);
    assert_eq!(policy.safe_mode_threshold, 2);

    assert!(!policy.should_enter_safe_mode(1));
    assert!(policy.should_enter_safe_mode(2));
}

// ===========================================================================
// Downgrade levels
// ===========================================================================

#[test]
fn test_downgrade_starts_at_none() {
    let mgr = DowngradeManager::new(DowngradeThresholds::default());
    assert_eq!(mgr.current_level(), DowngradeLevel::None);
}

#[test]
fn test_downgrade_escalation_reduce_fps() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string(), "s2".to_string()];

    let action = mgr.evaluate_host_load(87.0, &sessions);
    assert!(action.is_some());
    let action = action.unwrap();
    assert_eq!(action.level, DowngradeLevel::ReduceFps);
    assert_eq!(action.affected_sessions.len(), 2);
    assert_eq!(mgr.current_level(), DowngradeLevel::ReduceFps);
}

#[test]
fn test_downgrade_escalation_tile_only() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string()];

    // First escalate to ReduceFps.
    mgr.evaluate_host_load(87.0, &sessions);

    // Then escalate to TileOnly.
    let action = mgr.evaluate_host_load(92.0, &sessions);
    assert!(action.is_some());
    assert_eq!(action.unwrap().level, DowngradeLevel::TileOnly);
    assert_eq!(mgr.current_level(), DowngradeLevel::TileOnly);
}

#[test]
fn test_downgrade_escalation_reduce_quality() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string()];

    mgr.evaluate_host_load(87.0, &sessions); // ReduceFps
    mgr.evaluate_host_load(92.0, &sessions); // TileOnly

    let action = mgr.evaluate_host_load(96.0, &sessions);
    assert!(action.is_some());
    // SuspendLeastActive because 96 >= suspend_cpu_pct (95)
    assert_eq!(action.unwrap().level, DowngradeLevel::SuspendLeastActive);
}

#[test]
fn test_downgrade_does_not_downescalate_via_evaluate() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string()];

    mgr.evaluate_host_load(92.0, &sessions); // TileOnly
    assert_eq!(mgr.current_level(), DowngradeLevel::TileOnly);

    // Lower CPU should not de-escalate via evaluate_host_load.
    let action = mgr.evaluate_host_load(50.0, &sessions);
    assert!(action.is_none());
    assert_eq!(mgr.current_level(), DowngradeLevel::TileOnly);
}

#[test]
fn test_downgrade_no_action_below_thresholds() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string()];

    let action = mgr.evaluate_host_load(50.0, &sessions);
    assert!(action.is_none());
    assert_eq!(mgr.current_level(), DowngradeLevel::None);
}

#[test]
fn test_downgrade_reset() {
    let mut mgr = DowngradeManager::new(DowngradeThresholds::default());
    let sessions = vec!["s1".to_string()];

    mgr.evaluate_host_load(92.0, &sessions);
    assert_eq!(mgr.current_level(), DowngradeLevel::TileOnly);

    mgr.reset();
    assert_eq!(mgr.current_level(), DowngradeLevel::None);
}

#[test]
fn test_downgrade_level_ordering() {
    assert!(DowngradeLevel::None < DowngradeLevel::ReduceFps);
    assert!(DowngradeLevel::ReduceFps < DowngradeLevel::TileOnly);
    assert!(DowngradeLevel::TileOnly < DowngradeLevel::ReduceQuality);
    assert!(DowngradeLevel::ReduceQuality < DowngradeLevel::SuspendLeastActive);
}

// ===========================================================================
// Crash classification
// ===========================================================================

#[test]
fn test_crash_classify_segfault() {
    assert_eq!(
        CrashHandler::classify_crash(Some(11), None),
        CrashCategory::Segfault
    );
}

#[test]
fn test_crash_classify_abort() {
    assert_eq!(
        CrashHandler::classify_crash(Some(6), None),
        CrashCategory::Abort
    );
}

#[test]
fn test_crash_classify_bus_error() {
    assert_eq!(
        CrashHandler::classify_crash(Some(7), None),
        CrashCategory::BusError
    );
}

#[test]
fn test_crash_classify_fpe() {
    assert_eq!(
        CrashHandler::classify_crash(Some(8), None),
        CrashCategory::FloatingPoint
    );
}

#[test]
fn test_crash_classify_illegal_instruction() {
    assert_eq!(
        CrashHandler::classify_crash(Some(4), None),
        CrashCategory::IllegalInstruction
    );
}

#[test]
fn test_crash_classify_oom_signal() {
    assert_eq!(
        CrashHandler::classify_crash(Some(9), None),
        CrashCategory::OomKill
    );
}

#[test]
fn test_crash_classify_panic_exit_code() {
    assert_eq!(
        CrashHandler::classify_crash(None, Some(101)),
        CrashCategory::Panic
    );
}

#[test]
fn test_crash_classify_oom_exit_code() {
    assert_eq!(
        CrashHandler::classify_crash(None, Some(137)),
        CrashCategory::OomKill
    );
}

#[test]
fn test_crash_classify_unknown_signal() {
    assert_eq!(
        CrashHandler::classify_crash(Some(99), None),
        CrashCategory::Unknown
    );
}

#[test]
fn test_crash_classify_unknown_exit_code() {
    assert_eq!(
        CrashHandler::classify_crash(None, Some(42)),
        CrashCategory::Unknown
    );
}

#[test]
fn test_crash_classify_no_info() {
    assert_eq!(
        CrashHandler::classify_crash(None, None),
        CrashCategory::Unknown
    );
}

#[test]
fn test_crash_handler_generate_report() {
    let mut handler = CrashHandler::new("/tmp/crashes".into());
    let report = handler.generate_report(
        "sess-1",
        "alice",
        Some(11),
        None,
        120,
        vec!["error line 1".into()],
    );

    assert_eq!(report.crash_id, "crash-1");
    assert_eq!(report.session_id, "sess-1");
    assert_eq!(report.user, "alice");
    assert_eq!(report.category, CrashCategory::Segfault);
    assert_eq!(report.signal, Some(11));
    assert_eq!(report.uptime_seconds, 120);
    assert_eq!(report.log_lines.len(), 1);
}

#[test]
fn test_crash_handler_increments_ids() {
    let mut handler = CrashHandler::new("/tmp/crashes".into());
    let r1 = handler.generate_report("s1", "a", None, None, 0, vec![]);
    let r2 = handler.generate_report("s2", "b", None, None, 0, vec![]);
    assert_eq!(r1.crash_id, "crash-1");
    assert_eq!(r2.crash_id, "crash-2");
}

#[test]
fn test_crash_handler_store_report() {
    let handler = CrashHandler::new("/tmp/crashes".into());
    let mut h = CrashHandler::new("/tmp/crashes".into());
    let report = h.generate_report("s1", "a", None, None, 0, vec![]);
    let path = handler.store_report(&report).unwrap();
    assert!(path.starts_with("/tmp/crashes/"));
    assert!(path.ends_with(".json"));
}

// ===========================================================================
// Resource monitoring
// ===========================================================================

#[test]
fn test_resource_monitor_snapshot_defaults() {
    let monitor = ResourceMonitor::new();
    let snap = monitor.snapshot_session("sess-1");
    assert_eq!(snap.cpu_usage_pct, 0.0);
    assert_eq!(snap.memory_used_mb, 0);
}

#[test]
fn test_resource_monitor_host_metrics() {
    let mut monitor = ResourceMonitor::new();
    monitor.update_host_metrics(HostMetrics {
        cpu_pct: 45.0,
        memory_pct: 60.0,
        load_avg_1m: 2.5,
        load_avg_5m: 2.0,
        uptime_sec: 86400,
    });

    let metrics = monitor.snapshot_host();
    assert_eq!(metrics.cpu_pct, 45.0);
    assert_eq!(metrics.memory_pct, 60.0);
    assert_eq!(metrics.uptime_sec, 86400);
}

#[test]
fn test_resource_monitor_no_warnings_below_thresholds() {
    let monitor = ResourceMonitor::new();
    let snap = ResourceSnapshot {
        cpu_usage_pct: 50.0,
        memory_used_mb: 200,
        memory_total_mb: 512,
        pids_current: 100,
        io_read_bytes: 0,
        io_write_bytes: 0,
    };

    let warnings = monitor.check_warnings("s1", &snap, 2.0, 512, 256);
    assert!(warnings.is_empty());
}

#[test]
fn test_resource_monitor_cpu_warning() {
    let monitor = ResourceMonitor::new();
    let snap = ResourceSnapshot {
        cpu_usage_pct: 168.0, // 84% of 200% (2 cores)
        memory_used_mb: 100,
        memory_total_mb: 512,
        pids_current: 50,
        io_read_bytes: 0,
        io_write_bytes: 0,
    };

    let warnings = monitor.check_warnings("s1", &snap, 2.0, 512, 256);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].severity, ResourceSeverity::Warning);
}

#[test]
fn test_resource_monitor_cpu_critical() {
    let monitor = ResourceMonitor::new();
    let snap = ResourceSnapshot {
        cpu_usage_pct: 195.0, // 97.5% of 200% (2 cores)
        memory_used_mb: 100,
        memory_total_mb: 512,
        pids_current: 50,
        io_read_bytes: 0,
        io_write_bytes: 0,
    };

    let warnings = monitor.check_warnings("s1", &snap, 2.0, 512, 256);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].severity, ResourceSeverity::Critical);
}

#[test]
fn test_resource_monitor_memory_warning() {
    let monitor = ResourceMonitor::new();
    let snap = ResourceSnapshot {
        cpu_usage_pct: 0.0,
        memory_used_mb: 440, // ~86% of 512
        memory_total_mb: 512,
        pids_current: 50,
        io_read_bytes: 0,
        io_write_bytes: 0,
    };

    let warnings = monitor.check_warnings("s1", &snap, 2.0, 512, 256);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].severity, ResourceSeverity::Warning);
}

// ===========================================================================
// Audit events
// ===========================================================================

#[test]
fn test_audit_event_levels() {
    use crate::audit::SupervisorAuditEvent;

    assert_eq!(
        SupervisorAuditEvent::SupervisorStarted.level(),
        AuditLevel::Info
    );
    assert_eq!(
        SupervisorAuditEvent::SupervisorStopped.level(),
        AuditLevel::Info
    );
    assert_eq!(
        SupervisorAuditEvent::SessionSpawned {
            session_id: "s1".into(),
            user: "a".into()
        }
        .level(),
        AuditLevel::Info
    );
    assert_eq!(
        SupervisorAuditEvent::SessionTerminated {
            session_id: "s1".into(),
            reason: "test".into()
        }
        .level(),
        AuditLevel::Error
    );
    assert_eq!(
        SupervisorAuditEvent::SessionCrashed {
            session_id: "s1".into(),
            category: CrashCategory::Segfault
        }
        .level(),
        AuditLevel::Error
    );
    assert_eq!(
        SupervisorAuditEvent::RestartAttempted {
            session_id: "s1".into(),
            attempt: 1
        }
        .level(),
        AuditLevel::Warn
    );
    assert_eq!(
        SupervisorAuditEvent::AdmissionRejected {
            user: "a".into(),
            reason: "full".into()
        }
        .level(),
        AuditLevel::Warn
    );
    assert_eq!(
        SupervisorAuditEvent::PolicyUpdated.level(),
        AuditLevel::Info
    );
    assert_eq!(
        SupervisorAuditEvent::AuthenticationFailed {
            user: "a".into(),
            reason: "bad password".into()
        }
        .level(),
        AuditLevel::Error
    );
}

#[test]
fn test_audit_event_names() {
    use crate::audit::SupervisorAuditEvent;

    assert_eq!(
        SupervisorAuditEvent::SupervisorStarted.event_name(),
        "supervisor_started"
    );
    assert_eq!(
        SupervisorAuditEvent::SessionSpawned {
            session_id: "s1".into(),
            user: "a".into()
        }
        .event_name(),
        "session_spawned"
    );
    assert_eq!(
        SupervisorAuditEvent::SessionCrashed {
            session_id: "s1".into(),
            category: CrashCategory::Abort
        }
        .event_name(),
        "session_crashed"
    );
    assert_eq!(
        SupervisorAuditEvent::ControlCommandReceived {
            command: "GetStatus".into()
        }
        .event_name(),
        "control_command_received"
    );
}

// ===========================================================================
// Runtime: spawn / terminate / crash flow
// ===========================================================================

#[test]
fn test_runtime_spawn_session() {
    let mut rt = make_runtime();
    let session_id = rt.spawn_session("alice").unwrap();
    assert_eq!(session_id, "session-1");
    assert_eq!(rt.session_registry().active_count(), 1);

    let record = rt.session_registry().get_session(&session_id).unwrap();
    assert_eq!(record.user, "alice");
    assert_eq!(record.state, SessionState::Running);
}

#[test]
fn test_runtime_spawn_multiple_sessions() {
    let mut rt = make_runtime();
    let s1 = rt.spawn_session("alice").unwrap();
    let s2 = rt.spawn_session("bob").unwrap();
    let s3 = rt.spawn_session("alice").unwrap();

    assert_ne!(s1, s2);
    assert_ne!(s2, s3);
    assert_eq!(rt.session_registry().active_count(), 3);
    assert_eq!(rt.session_registry().sessions_for_user("alice").len(), 2);
}

#[test]
fn test_runtime_terminate_session() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();
    rt.terminate_session(&sid).unwrap();

    let record = rt.session_registry().get_session(&sid).unwrap();
    assert_eq!(record.state, SessionState::Terminated);
    assert_eq!(rt.session_registry().active_count(), 0);
}

#[test]
fn test_runtime_terminate_nonexistent_fails() {
    let mut rt = make_runtime();
    let result = rt.terminate_session("no-such-session");
    assert!(result.is_err());
}

#[test]
fn test_runtime_handle_crash_first_restart() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();

    let decision = rt.handle_crash(&sid, Some(11), None).unwrap();
    // restart_count is 1 after the first crash, so the policy applies a base-delay backoff.
    assert_eq!(
        decision,
        RestartDecision::RestartAfterDelay { delay_ms: 1000 }
    );

    let record = rt.session_registry().get_session(&sid).unwrap();
    assert_eq!(record.state, SessionState::Running);
    assert_eq!(record.restart_count, 1);
    assert_eq!(record.crash_history.len(), 1);
}

#[test]
fn test_runtime_handle_crash_escalation_to_failed() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();

    // Crash 5 times (max_restarts = 5, so the 5th restart_count hits the limit).
    for i in 0..5 {
        let decision = rt.handle_crash(&sid, Some(11), None).unwrap();
        if i < 4 {
            // First 4 crashes should result in restart (now or delayed).
            assert!(
                matches!(
                    decision,
                    RestartDecision::RestartNow | RestartDecision::RestartAfterDelay { .. }
                ),
                "crash {} should restart, got {:?}",
                i,
                decision
            );
        } else {
            // 5th crash (restart_count reaches 5 = max_restarts) -> EnterFailed
            assert!(
                matches!(decision, RestartDecision::EnterFailed { .. }),
                "crash {} should enter failed, got {:?}",
                i,
                decision
            );
        }
    }

    let record = rt.session_registry().get_session(&sid).unwrap();
    assert_eq!(record.state, SessionState::Failed);
}

#[test]
fn test_runtime_handle_crash_nonexistent_fails() {
    let mut rt = make_runtime();
    let result = rt.handle_crash("no-such-session", None, None);
    assert!(result.is_err());
}

#[test]
fn test_runtime_audit_events_on_spawn() {
    let mut rt = make_runtime();
    rt.spawn_session("alice").unwrap();

    let events = rt.drain_audit_events();
    let names: Vec<&str> = events.iter().map(|e| e.event_name()).collect();
    assert!(names.contains(&"session_spawned"));
}

#[test]
fn test_runtime_audit_events_drain_empties() {
    let mut rt = make_runtime();
    rt.spawn_session("alice").unwrap();

    let first = rt.drain_audit_events();
    assert!(!first.is_empty());

    let second = rt.drain_audit_events();
    assert!(second.is_empty());
}

#[test]
fn test_runtime_control_command_list_sessions() {
    let mut rt = make_runtime();
    rt.spawn_session("alice").unwrap();
    rt.spawn_session("bob").unwrap();
    rt.drain_audit_events(); // Clear spawn events.

    let resp = rt.handle_control_command(ControlCommand::ListSessions);
    match resp {
        ControlResponse::SessionList(list) => {
            assert_eq!(list.len(), 2);
        }
        other => panic!("expected SessionList, got {:?}", other),
    }
}

#[test]
fn test_runtime_control_command_get_session_info() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();
    rt.drain_audit_events();

    let resp = rt.handle_control_command(ControlCommand::GetSessionInfo {
        session_id: sid.clone(),
    });
    match resp {
        ControlResponse::SessionInfo(detail) => {
            assert_eq!(detail.session_id, sid);
            assert_eq!(detail.user, "alice");
            assert_eq!(detail.state, SessionState::Running);
        }
        other => panic!("expected SessionInfo, got {:?}", other),
    }
}

#[test]
fn test_runtime_control_command_get_nonexistent_session() {
    let mut rt = make_runtime();
    let resp = rt.handle_control_command(ControlCommand::GetSessionInfo {
        session_id: "nope".into(),
    });
    match resp {
        ControlResponse::Error(msg) => {
            assert!(msg.contains("session not found"));
        }
        other => panic!("expected Error, got {:?}", other),
    }
}

#[test]
fn test_runtime_control_command_lock_unlock() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();
    rt.drain_audit_events();

    let resp = rt.handle_control_command(ControlCommand::LockSession {
        session_id: sid.clone(),
    });
    assert!(matches!(resp, ControlResponse::Ok));
    assert_eq!(
        rt.session_registry().get_session(&sid).unwrap().state,
        SessionState::Locked
    );

    let resp = rt.handle_control_command(ControlCommand::UnlockSession {
        session_id: sid.clone(),
    });
    assert!(matches!(resp, ControlResponse::Ok));
    assert_eq!(
        rt.session_registry().get_session(&sid).unwrap().state,
        SessionState::Running
    );
}

#[test]
fn test_runtime_control_command_suspend_resume() {
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();
    rt.drain_audit_events();

    let resp = rt.handle_control_command(ControlCommand::SuspendSession {
        session_id: sid.clone(),
    });
    assert!(matches!(resp, ControlResponse::Ok));
    assert_eq!(
        rt.session_registry().get_session(&sid).unwrap().state,
        SessionState::Suspended
    );

    let resp = rt.handle_control_command(ControlCommand::ResumeSession {
        session_id: sid.clone(),
    });
    assert!(matches!(resp, ControlResponse::Ok));
    assert_eq!(
        rt.session_registry().get_session(&sid).unwrap().state,
        SessionState::Running
    );
}

#[test]
fn test_runtime_control_command_get_status() {
    let mut rt = make_runtime();
    rt.spawn_session("alice").unwrap();
    rt.drain_audit_events();

    let resp = rt.handle_control_command(ControlCommand::GetStatus);
    match resp {
        ControlResponse::Status(status) => {
            assert_eq!(status.active_sessions, 1);
        }
        other => panic!("expected Status, got {:?}", other),
    }
}

#[test]
fn test_runtime_control_command_shutdown() {
    let mut rt = make_runtime();
    let resp = rt.handle_control_command(ControlCommand::Shutdown);
    assert!(matches!(resp, ControlResponse::Ok));

    let events = rt.drain_audit_events();
    let names: Vec<&str> = events.iter().map(|e| e.event_name()).collect();
    assert!(names.contains(&"supervisor_stopped"));
}

#[test]
fn test_runtime_control_command_set_policy() {
    let mut rt = make_runtime();
    let resp = rt.handle_control_command(ControlCommand::SetPolicy {
        policy: "new-rules".into(),
    });
    assert!(matches!(resp, ControlResponse::Ok));

    let events = rt.drain_audit_events();
    let names: Vec<&str> = events.iter().map(|e| e.event_name()).collect();
    assert!(names.contains(&"policy_updated"));
}

#[test]
fn test_runtime_status() {
    let rt = make_runtime();
    let status = rt.status();
    assert_eq!(status.active_sessions, 0);
    assert!(status.uptime_sec < 5);
}

// ===========================================================================
// Config defaults
// ===========================================================================

#[test]
fn test_supervisor_config_defaults() {
    let cfg = SupervisorConfig::default();
    assert_eq!(cfg.listen_address, "0.0.0.0:3900");
    assert_eq!(cfg.control_socket_path, "/run/liquide/supervisor.sock");
    assert!(!cfg.dev_mode);
}

#[test]
fn test_resource_defaults() {
    let rd = ResourceDefaults::default();
    assert_eq!(rd.cpu_cores, 2.0);
    assert_eq!(rd.memory_mb, 512);
    assert_eq!(rd.max_pids, 256);
    assert_eq!(rd.io_bandwidth_mbps, 10);
    assert_eq!(rd.net_bandwidth_mbps, 20);
    assert_eq!(rd.encoder_threads, 2);
}

#[test]
fn test_admission_config_defaults() {
    let cfg = AdmissionConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.reserved_cpu_cores, 2.0);
    assert_eq!(cfg.reserved_memory_mb, 1024);
    assert_eq!(cfg.max_sessions, 0);
    assert!(!cfg.queue_enabled);
    assert_eq!(cfg.queue_timeout_sec, 30);
    assert_eq!(cfg.deny_4k_below_cores, 8);
    assert_eq!(cfg.deny_60fps_below_cores, 4);
}

#[test]
fn test_downgrade_thresholds_defaults() {
    let dt = DowngradeThresholds::default();
    assert_eq!(dt.reduce_fps_cpu_pct, 85.0);
    assert_eq!(dt.tile_only_cpu_pct, 90.0);
    assert_eq!(dt.reduce_quality_cpu_pct, 95.0);
    assert_eq!(dt.suspend_cpu_pct, 95.0);
    assert_eq!(dt.recovery_hysteresis_pct, 5.0);
    assert_eq!(dt.recovery_hold_sec, 30);
}

// ===========================================================================
// Session state display
// ===========================================================================

#[test]
fn test_session_state_display() {
    assert_eq!(SessionState::Spawning.to_string(), "Spawning");
    assert_eq!(SessionState::Running.to_string(), "Running");
    assert_eq!(SessionState::Locked.to_string(), "Locked");
    assert_eq!(SessionState::Disconnected.to_string(), "Disconnected");
    assert_eq!(SessionState::Suspended.to_string(), "Suspended");
    assert_eq!(SessionState::Crashed.to_string(), "Crashed");
    assert_eq!(SessionState::Failed.to_string(), "Failed");
    assert_eq!(SessionState::Terminated.to_string(), "Terminated");
}

// ===========================================================================
// Spawn
// ===========================================================================

#[test]
fn test_spawner_launches_real_process_with_os_pid() {
    use crate::spawn::{SessionSpawner, SpawnRequest};

    let mut spawner = SessionSpawner::with_command(sleeper_command());
    let req1 = SpawnRequest {
        user: "alice".into(),
        session_id: "s1".into(),
        resource_budget: make_budget(),
        safe_mode: false,
    };
    let req2 = SpawnRequest {
        user: "bob".into(),
        session_id: "s2".into(),
        resource_budget: make_budget(),
        safe_mode: false,
    };

    let r1 = spawner.spawn_session(&req1).unwrap();
    let r2 = spawner.spawn_session(&req2).unwrap();

    // Real OS PIDs are assigned by the kernel, not a synthetic counter, so they
    // must be nonzero and distinct.
    assert_ne!(r1.pid, 0);
    assert_ne!(r2.pid, 0);
    assert_ne!(r1.pid, r2.pid);
    assert_eq!(r1.session_id, "s1");
    assert_eq!(r2.session_id, "s2");

    // Both children are tracked and reported alive.
    assert_eq!(spawner.tracked_count(), 2);
    assert!(spawner.is_alive(r1.pid));
    assert!(spawner.is_alive(r2.pid));

    // Killing one removes it from tracking and reports it no longer alive.
    spawner.kill_session(r1.pid).unwrap();
    assert!(!spawner.is_alive(r1.pid));
    spawner.kill_session(r2.pid).unwrap();
}

#[test]
fn test_spawn_fails_when_binary_missing() {
    use crate::spawn::{SessionSpawner, SpawnCommand, SpawnRequest};

    // Point at a program that does not exist; spawn must fail loudly rather
    // than report a phantom running session.
    let mut spawner = SessionSpawner::with_command(SpawnCommand {
        program: "liquide-no-such-binary-xyzzy".to_string(),
        base_args: Vec::new(),
        append_session_args: false,
    });
    let req = SpawnRequest {
        user: "alice".into(),
        session_id: "s1".into(),
        resource_budget: make_budget(),
        safe_mode: false,
    };
    let err = spawner.spawn_session(&req).unwrap_err();
    assert!(matches!(err, crate::SupervisorError::SpawnFailed { .. }));
    assert_eq!(spawner.tracked_count(), 0);
}

#[test]
fn test_runtime_session_process_is_actually_alive() {
    // The runtime must back a Running session with a live OS process.
    let mut rt = make_runtime();
    let sid = rt.spawn_session("alice").unwrap();
    assert_eq!(
        rt.session_registry().get_session(&sid).unwrap().state,
        SessionState::Running
    );
    assert!(
        rt.is_session_process_alive(&sid),
        "a Running session must have a live child process"
    );

    rt.terminate_session(&sid).unwrap();
    assert!(
        !rt.is_session_process_alive(&sid),
        "after termination the child process must be gone"
    );
}

#[test]
fn test_runtime_spawn_fails_with_missing_binary() {
    use crate::spawn::SpawnCommand;
    let mut rt = SupervisorRuntime::new(
        SupervisorConfig::default(),
        ResourceDefaults::default(),
        AdmissionConfig::default(),
        DowngradeThresholds::default(),
        RestartPolicy::default(),
        16.0,
        32768,
    );
    rt.set_spawn_command(SpawnCommand {
        program: "liquide-no-such-binary-xyzzy".to_string(),
        base_args: Vec::new(),
        append_session_args: false,
    });
    // Session creation must fail; nothing should be registered as Running.
    assert!(rt.spawn_session("alice").is_err());
    assert_eq!(rt.session_registry().active_count(), 0);
}

// ===========================================================================
// IPC types
// ===========================================================================

#[test]
fn test_control_command_display() {
    assert_eq!(ControlCommand::ListSessions.to_string(), "ListSessions");
    assert_eq!(ControlCommand::GetStatus.to_string(), "GetStatus");
    assert_eq!(ControlCommand::Shutdown.to_string(), "Shutdown");
    assert_eq!(
        ControlCommand::SpawnSession {
            user: "alice".into()
        }
        .to_string(),
        "SpawnSession(alice)"
    );
    assert_eq!(
        ControlCommand::TerminateSession {
            session_id: "s1".into()
        }
        .to_string(),
        "TerminateSession(s1)"
    );
}

#[test]
fn test_control_channel_socket_path() {
    use crate::ipc::ControlChannel;
    let ch = ControlChannel::new("/run/liquide/test.sock".into());
    assert_eq!(ch.socket_path(), "/run/liquide/test.sock");
}
