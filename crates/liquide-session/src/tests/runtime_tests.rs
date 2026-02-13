use crate::config::{
    JailConfig, ResumeConfig, ResourceLimits, SessionConfig, SupervisorConfig,
};
use crate::crash::RestartAction;
use crate::ipc::SupervisorCommand;
use crate::resume::TokenScope;
use crate::runtime::SessionRuntime;
use crate::state::SessionState;

fn make_resume_config() -> ResumeConfig {
    ResumeConfig {
        enabled: true,
        token_lifetime_hours: 168,
        token_rotation: true,
        token_scope: TokenScope::SameServer,
        max_disconnected_minutes: 60,
        require_mfa_on_resume: false,
        require_mfa_after_hours: 24,
    }
}

fn make_runtime(session_id: &str) -> SessionRuntime {
    SessionRuntime::new(
        session_id.to_string(),
        SessionConfig::default(),
        SupervisorConfig::default(),
        ResourceLimits::default(),
        make_resume_config(),
        JailConfig::default(),
        false,
    )
}

fn make_runtime_safe_mode(session_id: &str) -> SessionRuntime {
    SessionRuntime::new(
        session_id.to_string(),
        SessionConfig::default(),
        SupervisorConfig::default(),
        ResourceLimits::default(),
        make_resume_config(),
        JailConfig::default(),
        true,
    )
}

#[test]
fn test_runtime_new_starts_in_created() {
    let rt = make_runtime("rt-1");
    assert_eq!(rt.state(), SessionState::Created);
    assert_eq!(rt.session_id(), "rt-1");
    assert!(!rt.is_safe_mode());
}

#[test]
fn test_runtime_initialize_reaches_running() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_initialize_starts_workers() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    // Without safe mode, all 9 workers should be running (7 essential + Plugin + Accessibility).
    assert_eq!(rt.worker_manager().running_count(), 9);
    assert!(rt.worker_manager().all_running());
}

#[test]
fn test_runtime_initialize_safe_mode_skips_optional_workers() {
    let mut rt = make_runtime_safe_mode("rt-safe");
    rt.initialize().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
    assert!(rt.is_safe_mode());
    // Safe mode: 7 essential workers only, Plugin and Accessibility are skipped.
    assert_eq!(rt.worker_manager().running_count(), 7);
}

#[test]
fn test_runtime_lock_and_unlock() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.lock().unwrap();
    assert_eq!(rt.state(), SessionState::Locked);

    rt.unlock().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_lock_from_wrong_state_fails() {
    let mut rt = make_runtime("rt-1");
    // Cannot lock from Created state (not in Running valid transitions).
    let result = rt.lock();
    assert!(result.is_err());
}

#[test]
fn test_runtime_suspend_and_resume() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.suspend().unwrap();
    assert_eq!(rt.state(), SessionState::Suspended);

    rt.resume_session().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_disconnect() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.disconnect().unwrap();
    assert_eq!(rt.state(), SessionState::Disconnected);
}

#[test]
fn test_runtime_disconnect_then_resume() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    rt.disconnect().unwrap();
    assert_eq!(rt.state(), SessionState::Disconnected);

    rt.resume_session().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_handle_crash_first_time_restarts_normally() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    let action = rt.handle_crash();
    assert_eq!(action, RestartAction::RestartNormal);
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_handle_crash_escalation() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    // First crash: RestartNormal
    let a1 = rt.handle_crash();
    assert_eq!(a1, RestartAction::RestartNormal);
    assert_eq!(rt.state(), SessionState::Running);

    // Second crash: RestartSafePlugins
    let a2 = rt.handle_crash();
    assert_eq!(a2, RestartAction::RestartSafePlugins);
    assert_eq!(rt.state(), SessionState::Running);

    // Third crash: RestartSafeMode (default safe_mode_after_restart = 3)
    let a3 = rt.handle_crash();
    assert_eq!(a3, RestartAction::RestartSafeMode);
    assert_eq!(rt.state(), SessionState::Running);
    assert!(rt.is_safe_mode());
}

#[test]
fn test_runtime_handle_crash_exceeds_max_enters_failed() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    // The default max_restarts is 5. Crash 6 times to exceed.
    for _ in 0..5 {
        rt.handle_crash();
    }
    assert_eq!(rt.state(), SessionState::Running);

    let action = rt.handle_crash();
    assert_eq!(action, RestartAction::EnterFailed);
    assert_eq!(rt.state(), SessionState::Failed);
}

#[test]
fn test_runtime_audit_events_on_initialize() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    let events = rt.drain_audit_events();
    // Expect at least: SessionCreated, StateTransition(Created->Auth),
    // StateTransition(Auth->Running), and multiple WorkerStarted events.
    assert!(events.len() >= 4);

    let event_names: Vec<&str> = events.iter().map(|e| e.event_name()).collect();
    assert!(event_names.contains(&"session_created"));
    assert!(event_names.contains(&"state_transition"));
    assert!(event_names.contains(&"worker_started"));
}

#[test]
fn test_runtime_audit_events_drain_empties() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    let first = rt.drain_audit_events();
    assert!(!first.is_empty());

    let second = rt.drain_audit_events();
    assert!(second.is_empty());
}

#[test]
fn test_runtime_tick_sends_heartbeat() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    rt.drain_audit_events(); // clear creation events

    rt.tick();
    assert_eq!(rt.heartbeat_monitor().total_sent(), 1);
    assert_eq!(rt.heartbeat_monitor().missed_count(), 1);
}

#[test]
fn test_runtime_tick_multiple_triggers_warning_events() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    rt.drain_audit_events();

    rt.tick(); // missed = 1 -> Warning
    rt.tick(); // missed = 2 -> Warning

    let events = rt.drain_audit_events();
    let timeout_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_name() == "heartbeat_timeout")
        .collect();
    // Both ticks produce Warning (missed > 0), which logs HeartbeatTimeout.
    assert_eq!(timeout_events.len(), 2);
}

#[test]
fn test_runtime_record_heartbeat_received() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.tick();
    assert_eq!(rt.heartbeat_monitor().missed_count(), 1);

    rt.record_heartbeat_received();
    assert_eq!(rt.heartbeat_monitor().missed_count(), 0);
    assert_eq!(rt.heartbeat_monitor().total_received(), 1);
}

#[test]
fn test_runtime_supervisor_command_shutdown() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::Shutdown).unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_lock_unlock() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::Lock).unwrap();
    assert_eq!(rt.state(), SessionState::Locked);

    rt.handle_supervisor_command(SupervisorCommand::Unlock).unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_supervisor_command_suspend_resume() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::Suspend).unwrap();
    assert_eq!(rt.state(), SessionState::Suspended);

    rt.handle_supervisor_command(SupervisorCommand::Resume).unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_supervisor_command_force_terminate() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::ForceTerminate).unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_restart_session() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::RestartSession).unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_update_policy() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::UpdatePolicy).unwrap();
    // State should remain Running; update policy is a no-op stub.
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_resume_manager_access() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    assert!(rt.resume_manager().is_enabled());

    let tid = rt
        .resume_manager_mut()
        .issue_token("rt-1", "user-a", "fp-1")
        .unwrap();
    assert_eq!(rt.resume_manager().token_count(), 1);

    // Disconnect, then reconnect using the resume token.
    rt.disconnect().unwrap();
    rt.reconnect(&tid).unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_reconnect_with_invalid_token_fails() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();
    rt.disconnect().unwrap();

    let result = rt.reconnect("bogus-token");
    assert!(result.is_err());
    assert_eq!(rt.state(), SessionState::Disconnected);
}

#[test]
fn test_runtime_reconnect_with_wrong_session_fails() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    // Issue a token for a different session id.
    let tid = rt
        .resume_manager_mut()
        .issue_token("different-session", "user-a", "fp")
        .unwrap();

    rt.disconnect().unwrap();
    let result = rt.reconnect(&tid);
    assert!(result.is_err());
}

#[test]
fn test_runtime_config_access() {
    let rt = make_runtime("rt-1");
    let config = rt.config();
    assert!(config.auto_resume);
    assert_eq!(config.max_per_user, 3);
}

#[test]
fn test_runtime_resource_limits_access() {
    let rt = make_runtime("rt-1");
    let limits = rt.resource_limits();
    assert_eq!(limits.memory_mb, 512);
    assert_eq!(limits.max_pids, 256);
}
