use crate::SessionAuditEvent;
use crate::config::{JailConfig, ResourceLimits, ResumeConfig, SessionConfig, SupervisorConfig};
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
fn test_runtime_handle_crash_valid_transition_audits_and_restarts() {
    // A crash from a state that *accepts* the transition into Crashed must emit
    // the StateTransition audit event and restart workers (positive path for
    // t49-e6-08).
    let mut rt = make_runtime("rt-crash-ok");
    rt.initialize().unwrap();
    assert_eq!(rt.state(), SessionState::Running);
    rt.drain_audit_events();

    let action = rt.handle_crash();

    assert_eq!(action, RestartAction::RestartNormal);
    assert_eq!(rt.state(), SessionState::Running);
    let events = rt.drain_audit_events();
    assert!(
        events.iter().any(|event| matches!(
            event,
            SessionAuditEvent::StateTransition { to, .. }
                if to == &SessionState::Crashed.to_string()
        )),
        "a valid crash must audit the transition into Crashed"
    );
}

#[test]
fn test_runtime_handle_crash_refused_transition_does_not_audit_or_restart() {
    // Drive the runtime into Failed (which refuses any transition into
    // Crashed), then crash again: the state machine rejects the transition, so
    // handle_crash must NOT emit a StateTransition/RestartAttempt audit event
    // and must NOT restart workers (t49-e6-08 negative path).
    let mut rt = make_runtime("rt-crash-refused");
    rt.initialize().unwrap();
    for _ in 0..6 {
        rt.handle_crash();
    }
    assert_eq!(rt.state(), SessionState::Failed);
    rt.drain_audit_events();

    let action = rt.handle_crash();

    // State is unchanged — the refused transition did not move the machine.
    assert_eq!(rt.state(), SessionState::Failed);
    assert_eq!(action, RestartAction::EnterFailed);
    let events = rt.drain_audit_events();
    assert!(
        events.is_empty(),
        "a refused crash transition must not audit anything, got: {events:?}"
    );
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
fn test_runtime_owner_refs_emit_last_owner_cleanup_events() {
    let mut rt = make_runtime("rt-owners");
    rt.drain_audit_events();

    assert_eq!(rt.attach_owner("client:1"), 1);
    assert_eq!(rt.attach_owner("manager:1"), 2);
    assert_eq!(rt.owner_count(), 2);

    assert_eq!(rt.detach_owner("client:1"), 1);
    assert_eq!(rt.detach_owner("manager:1"), 0);

    let events = rt.drain_audit_events();
    assert!(events.iter().any(|event| matches!(
        event,
        SessionAuditEvent::OwnerAttached { owner_id, owner_count, .. }
            if owner_id == "client:1" && *owner_count == 1
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        SessionAuditEvent::OwnerDetached { owner_id, owner_count, .. }
            if owner_id == "manager:1" && *owner_count == 0
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SessionAuditEvent::LastOwnerDetached { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        SessionAuditEvent::CleanupStarted { reason, .. } if reason == "last_owner_detached"
    )));
}

#[test]
fn test_runtime_drains_audit_events_into_structured_sink() {
    // B6b (session side): the structured EventLogService sink is actually
    // driven from the session audit plane. Initialization produces audit
    // events; draining them into an InMemoryEventLog must populate the sink
    // with Session-category records, and leave the runtime buffer empty.
    use liquide_common::event_log::{EventCategory, InMemoryEventLog};

    let mut rt = make_runtime("rt-sink");
    rt.initialize().unwrap();

    let mut sink = InMemoryEventLog::new();
    let recorded = rt.drain_audit_events_to(&mut sink).unwrap();

    assert!(
        recorded >= 4,
        "expected several init audit events, got {recorded}"
    );
    assert_eq!(sink.len(), recorded);
    // Every forwarded record is a Session-category structured event.
    assert!(
        sink.records()
            .iter()
            .all(|r| r.category == EventCategory::Session)
    );
    // A session_created record made it through the structured path.
    assert!(
        sink.records()
            .iter()
            .any(|r| r.event_id == "session_created"),
        "session_created must reach the structured sink"
    );
    // The buffer is now empty — a second structured drain records nothing.
    assert_eq!(rt.drain_audit_events_to(&mut sink).unwrap(), 0);
}

#[test]
fn test_runtime_structured_sink_error_preserves_audit_events() {
    // Negative path: a failing sink must NOT silently drop audit history.
    // After a sink error, the events remain in the runtime buffer so they can
    // be retried or drained another way (fail-safe audit, no silent loss).
    use liquide_common::event_log::{EventLogService, EventRecord};

    struct FailingSink;
    impl EventLogService for FailingSink {
        fn record_event(&mut self, _record: EventRecord) -> liquide_common::Result<()> {
            Err(liquide_common::LiquideError::Internal(
                "sink down".to_string(),
            ))
        }
    }

    // A runtime with a populated, undrained audit buffer (initialization
    // produces several events).
    let mut rt = make_runtime("rt-sink-fail");
    rt.initialize().unwrap();
    let mut sink = FailingSink;
    let err = rt.drain_audit_events_to(&mut sink);
    assert!(err.is_err(), "failing sink must surface an error");

    // No audit event was lost: the buffer still holds them.
    let preserved = rt.drain_audit_events();
    assert!(
        !preserved.is_empty(),
        "a sink error must preserve audit events, not drop them"
    );
}

#[test]
fn test_runtime_complete_cleanup_emits_event_record_ready_audit() {
    let mut rt = make_runtime("rt-cleanup");
    rt.drain_audit_events();

    rt.complete_cleanup();

    let events = rt.drain_audit_events();
    let cleanup = events
        .iter()
        .find(|event| matches!(event, SessionAuditEvent::CleanupCompleted { .. }))
        .expect("cleanup completion event");
    let record = cleanup.to_event_record();
    assert_eq!(record.event_id, "cleanup_completed");
    assert_eq!(record.session_id.as_deref(), Some("rt-cleanup"));
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

    rt.handle_supervisor_command(SupervisorCommand::Shutdown)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_lock_unlock() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::Lock)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Locked);

    rt.handle_supervisor_command(SupervisorCommand::Unlock)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_supervisor_command_suspend_resume() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::Suspend)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Suspended);

    rt.handle_supervisor_command(SupervisorCommand::Resume)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Running);
}

#[test]
fn test_runtime_supervisor_command_force_terminate() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::ForceTerminate)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_restart_session() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::RestartSession)
        .unwrap();
    assert_eq!(rt.state(), SessionState::Terminated);
}

#[test]
fn test_runtime_supervisor_command_update_policy() {
    let mut rt = make_runtime("rt-1");
    rt.initialize().unwrap();

    rt.handle_supervisor_command(SupervisorCommand::UpdatePolicy)
        .unwrap();
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
