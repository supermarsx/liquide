//! Tests for the liquide-session crate.

use std::time::Duration;

use crate::config::{
    JailConfig, ResumeConfig, ResourceLimits, SessionConfig, SupervisorConfig,
};
use crate::crash::{
    DisabledFeature, ResourceSnapshot, RestartAction, RestartTracker, SafeMode,
};
use crate::heartbeat::{HeartbeatConfig, HeartbeatMonitor, HeartbeatStatus};
use crate::ipc::SupervisorCommand;
use crate::resume::{
    PersistenceState, ResumeManager, ResumeToken, SessionPersistence, TokenScope,
};
use crate::state::{SessionState, SessionStateMachine};
use crate::runtime::SessionRuntime;
use crate::SessionError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_state_machine(id: &str) -> SessionStateMachine {
    SessionStateMachine::new(id.to_string())
}

fn make_heartbeat_monitor(timeout_count: u32) -> HeartbeatMonitor {
    HeartbeatMonitor::new(HeartbeatConfig {
        interval_sec: 5,
        timeout_count,
    })
}

fn make_restart_tracker(max: u32, safe_threshold: u32) -> RestartTracker {
    RestartTracker::new(max, 600, 100, safe_threshold)
}

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

// ===========================================================================
// State machine transitions
// ===========================================================================

#[test]
fn test_state_machine_starts_in_created() {
    let sm = make_state_machine("sess-1");
    assert_eq!(sm.state(), SessionState::Created);
    assert_eq!(sm.session_id(), "sess-1");
    assert_eq!(sm.transition_count(), 0);
}

#[test]
fn test_state_display_variants() {
    assert_eq!(SessionState::Created.to_string(), "Created");
    assert_eq!(SessionState::Authenticating.to_string(), "Authenticating");
    assert_eq!(SessionState::Running.to_string(), "Running");
    assert_eq!(SessionState::Locked.to_string(), "Locked");
    assert_eq!(SessionState::Disconnected.to_string(), "Disconnected");
    assert_eq!(SessionState::Suspended.to_string(), "Suspended");
    assert_eq!(SessionState::Crashed.to_string(), "Crashed");
    assert_eq!(SessionState::Failed.to_string(), "Failed");
    assert_eq!(SessionState::Terminated.to_string(), "Terminated");
}

#[test]
fn test_valid_transitions_from_created() {
    let valid = SessionState::Created.valid_transitions();
    assert!(valid.contains(&SessionState::Authenticating));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Running));
}

#[test]
fn test_valid_transitions_from_authenticating() {
    let valid = SessionState::Authenticating.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Failed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Locked));
}

#[test]
fn test_valid_transitions_from_running() {
    let valid = SessionState::Running.valid_transitions();
    assert!(valid.contains(&SessionState::Locked));
    assert!(valid.contains(&SessionState::Disconnected));
    assert!(valid.contains(&SessionState::Suspended));
    assert!(valid.contains(&SessionState::Crashed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Authenticating));
}

#[test]
fn test_valid_transitions_from_locked() {
    let valid = SessionState::Locked.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Disconnected));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Suspended));
}

#[test]
fn test_valid_transitions_from_disconnected() {
    let valid = SessionState::Disconnected.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Suspended));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Locked));
}

#[test]
fn test_valid_transitions_from_suspended() {
    let valid = SessionState::Suspended.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Terminated));
    assert_eq!(valid.len(), 2);
}

#[test]
fn test_valid_transitions_from_crashed() {
    let valid = SessionState::Crashed.valid_transitions();
    assert!(valid.contains(&SessionState::Running));
    assert!(valid.contains(&SessionState::Failed));
    assert!(valid.contains(&SessionState::Terminated));
    assert!(!valid.contains(&SessionState::Suspended));
}

#[test]
fn test_valid_transitions_from_failed() {
    let valid = SessionState::Failed.valid_transitions();
    assert_eq!(valid, &[SessionState::Terminated]);
}

#[test]
fn test_terminated_has_no_transitions() {
    let valid = SessionState::Terminated.valid_transitions();
    assert!(valid.is_empty());
}

#[test]
fn test_transition_created_to_authenticating() {
    let mut sm = make_state_machine("s1");
    assert!(sm.transition_to(SessionState::Authenticating).is_ok());
    assert_eq!(sm.state(), SessionState::Authenticating);
    assert_eq!(sm.transition_count(), 1);
}

#[test]
fn test_full_lifecycle_created_to_terminated() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Locked).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Disconnected).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Suspended).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Terminated).unwrap();
    assert_eq!(sm.state(), SessionState::Terminated);
    assert_eq!(sm.transition_count(), 9);
}

#[test]
fn test_crash_recovery_cycle() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Crashed).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    assert_eq!(sm.state(), SessionState::Running);
    assert_eq!(sm.transition_count(), 4);
}

#[test]
fn test_crash_to_failed_to_terminated() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Crashed).unwrap();
    sm.transition_to(SessionState::Failed).unwrap();
    sm.transition_to(SessionState::Terminated).unwrap();
    assert_eq!(sm.state(), SessionState::Terminated);
}

#[test]
fn test_invalid_transition_created_to_running() {
    let mut sm = make_state_machine("s1");
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    match result.unwrap_err() {
        SessionError::InvalidStateTransition { from, to } => {
            assert_eq!(from, "Created");
            assert_eq!(to, "Running");
        }
        other => panic!("expected InvalidStateTransition, got {:?}", other),
    }
    assert_eq!(sm.state(), SessionState::Created);
    assert_eq!(sm.transition_count(), 0);
}

#[test]
fn test_invalid_transition_terminated_to_running() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Terminated).unwrap();
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Terminated);
}

#[test]
fn test_invalid_transition_failed_to_running() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Failed).unwrap();
    let result = sm.transition_to(SessionState::Running);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Failed);
}

#[test]
fn test_invalid_transition_locked_to_suspended() {
    let mut sm = make_state_machine("s1");
    sm.transition_to(SessionState::Authenticating).unwrap();
    sm.transition_to(SessionState::Running).unwrap();
    sm.transition_to(SessionState::Locked).unwrap();
    let result = sm.transition_to(SessionState::Suspended);
    assert!(result.is_err());
    assert_eq!(sm.state(), SessionState::Locked);
}

#[test]
fn test_invalid_transition_created_to_crashed() {
    let mut sm = make_state_machine("s1");
    let result = sm.transition_to(SessionState::Crashed);
    assert!(result.is_err());
}

#[test]
fn test_safe_mode_flag() {
    let mut sm = make_state_machine("s1");
    assert!(!sm.is_safe_mode());
    sm.set_safe_mode(true);
    assert!(sm.is_safe_mode());
    sm.set_safe_mode(false);
    assert!(!sm.is_safe_mode());
}

#[test]
fn test_valid_transitions_method_delegates() {
    let sm = make_state_machine("s1");
    assert_eq!(
        sm.valid_transitions(),
        SessionState::Created.valid_transitions()
    );
}

#[test]
fn test_uptime_and_last_transition_are_non_negative() {
    let sm = make_state_machine("s1");
    // These are just sanity checks; times will be near zero.
    assert!(sm.uptime_seconds() < 5);
    assert!(sm.seconds_since_last_transition() < 5);
}

// ===========================================================================
// Restart tracker, backoff, and safe mode escalation
// ===========================================================================

#[test]
fn test_restart_tracker_first_restart_is_normal() {
    let mut tracker = make_restart_tracker(5, 3);
    let action = tracker.record_restart();
    assert_eq!(action, RestartAction::RestartNormal);
    assert_eq!(tracker.restart_count(), 1);
}

#[test]
fn test_restart_tracker_second_restart_is_safe_plugins() {
    let mut tracker = make_restart_tracker(5, 3);
    tracker.record_restart(); // 1 -> Normal
    let action = tracker.record_restart(); // 2 -> SafePlugins
    assert_eq!(action, RestartAction::RestartSafePlugins);
    assert_eq!(tracker.restart_count(), 2);
}

#[test]
fn test_restart_tracker_threshold_triggers_safe_mode() {
    let mut tracker = make_restart_tracker(5, 3);
    tracker.record_restart(); // 1 -> Normal
    tracker.record_restart(); // 2 -> SafePlugins
    let action = tracker.record_restart(); // 3 -> SafeMode (threshold)
    assert_eq!(action, RestartAction::RestartSafeMode);
    assert!(tracker.should_enter_safe_mode());
}

#[test]
fn test_restart_tracker_above_threshold_still_safe_mode() {
    let mut tracker = make_restart_tracker(5, 3);
    tracker.record_restart(); // 1
    tracker.record_restart(); // 2
    tracker.record_restart(); // 3 -> SafeMode
    let action = tracker.record_restart(); // 4 -> still SafeMode
    assert_eq!(action, RestartAction::RestartSafeMode);
    assert_eq!(tracker.restart_count(), 4);
}

#[test]
fn test_restart_tracker_exceeds_max_enters_failed() {
    let mut tracker = make_restart_tracker(5, 3);
    for _ in 0..5 {
        tracker.record_restart();
    }
    // 5 restarts have occurred, next one exceeds the limit
    let action = tracker.record_restart(); // 6 > 5
    assert_eq!(action, RestartAction::EnterFailed);
    assert!(tracker.has_exceeded_limit());
}

#[test]
fn test_restart_tracker_exactly_at_max_is_not_exceeded() {
    let mut tracker = make_restart_tracker(5, 3);
    for _ in 0..5 {
        tracker.record_restart();
    }
    assert!(!tracker.has_exceeded_limit());
    assert_eq!(tracker.restart_count(), 5);
    assert_eq!(tracker.max_restarts(), 5);
}

#[test]
fn test_backoff_zero_when_no_restarts() {
    let tracker = make_restart_tracker(5, 3);
    assert_eq!(tracker.current_backoff_ms(), 0);
}

#[test]
fn test_backoff_base_after_first_restart() {
    let mut tracker = make_restart_tracker(5, 3);
    tracker.record_restart();
    // backoff = 100 * 2^(1-1) = 100
    assert_eq!(tracker.current_backoff_ms(), 100);
}

#[test]
fn test_backoff_doubles_each_restart() {
    let mut tracker = make_restart_tracker(10, 5);
    tracker.record_restart(); // count=1 -> 100 * 2^0 = 100
    assert_eq!(tracker.current_backoff_ms(), 100);
    tracker.record_restart(); // count=2 -> 100 * 2^1 = 200
    assert_eq!(tracker.current_backoff_ms(), 200);
    tracker.record_restart(); // count=3 -> 100 * 2^2 = 400
    assert_eq!(tracker.current_backoff_ms(), 400);
    tracker.record_restart(); // count=4 -> 100 * 2^3 = 800
    assert_eq!(tracker.current_backoff_ms(), 800);
}

#[test]
fn test_restart_tracker_safe_mode_query_before_threshold() {
    let mut tracker = make_restart_tracker(5, 3);
    tracker.record_restart();
    assert!(!tracker.should_enter_safe_mode());
    tracker.record_restart();
    assert!(!tracker.should_enter_safe_mode());
}

#[test]
fn test_restart_tracker_with_threshold_equals_one() {
    let mut tracker = make_restart_tracker(3, 1);
    // First restart immediately hits the safe mode threshold.
    let action = tracker.record_restart();
    assert_eq!(action, RestartAction::RestartSafeMode);
}

#[test]
fn test_restart_tracker_max_one() {
    let mut tracker = make_restart_tracker(1, 1);
    tracker.record_restart(); // 1 == max, SafeMode since threshold=1
    let action = tracker.record_restart(); // 2 > 1, EnterFailed
    assert_eq!(action, RestartAction::EnterFailed);
}

// ===========================================================================
// Safe mode features
// ===========================================================================

#[test]
fn test_safe_mode_inactive_no_disabled_features() {
    let sm = SafeMode::new(false);
    assert!(!sm.is_active());
    assert!(sm.features_disabled().is_empty());
}

#[test]
fn test_safe_mode_active_disables_all_features() {
    let sm = SafeMode::new(true);
    assert!(sm.is_active());
    let features = sm.features_disabled();
    assert!(features.contains(&DisabledFeature::WasmPlugins));
    assert!(features.contains(&DisabledFeature::UserCss));
    assert!(features.contains(&DisabledFeature::ShellAnimations));
    assert!(features.contains(&DisabledFeature::Wallpaper));
    assert!(features.contains(&DisabledFeature::NonEssentialShell));
    assert_eq!(features.len(), 5);
}

#[test]
fn test_safe_mode_toggle() {
    let mut sm = SafeMode::new(false);
    assert!(!sm.is_active());
    sm.set_active(true);
    assert!(sm.is_active());
    assert_eq!(sm.features_disabled().len(), 5);
    sm.set_active(false);
    assert!(!sm.is_active());
    assert!(sm.features_disabled().is_empty());
}

// ===========================================================================
// Resource snapshot defaults
// ===========================================================================

#[test]
fn test_resource_snapshot_default() {
    let snap = ResourceSnapshot::default();
    assert_eq!(snap.cpu_percent, 0.0);
    assert_eq!(snap.memory_mb, 0);
    assert_eq!(snap.io_bytes, 0);
}

// ===========================================================================
// Heartbeat monitoring
// ===========================================================================

#[test]
fn test_heartbeat_monitor_initial_state_is_healthy() {
    let monitor = make_heartbeat_monitor(3);
    assert!(monitor.is_healthy());
    assert_eq!(monitor.missed_count(), 0);
    assert_eq!(monitor.check(), HeartbeatStatus::Healthy);
    assert_eq!(monitor.total_sent(), 0);
    assert_eq!(monitor.total_received(), 0);
}

#[test]
fn test_heartbeat_default_config() {
    let config = HeartbeatConfig::default();
    assert_eq!(config.interval_sec, 5);
    assert_eq!(config.timeout_count, 3);
}

#[test]
fn test_heartbeat_send_increments_missed() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent();
    assert_eq!(monitor.missed_count(), 1);
    assert_eq!(monitor.total_sent(), 1);
}

#[test]
fn test_heartbeat_receive_resets_missed() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent();
    monitor.record_sent();
    assert_eq!(monitor.missed_count(), 2);
    monitor.record_received();
    assert_eq!(monitor.missed_count(), 0);
    assert_eq!(monitor.total_received(), 1);
}

#[test]
fn test_heartbeat_warning_status() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent(); // missed = 1
    assert_eq!(monitor.check(), HeartbeatStatus::Warning { missed: 1 });
    assert!(monitor.is_healthy());

    monitor.record_sent(); // missed = 2
    assert_eq!(monitor.check(), HeartbeatStatus::Warning { missed: 2 });
    assert!(monitor.is_healthy());
}

#[test]
fn test_heartbeat_timed_out_status() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent(); // 1
    monitor.record_sent(); // 2
    monitor.record_sent(); // 3 >= threshold
    assert_eq!(monitor.check(), HeartbeatStatus::TimedOut { missed: 3 });
    assert!(!monitor.is_healthy());
}

#[test]
fn test_heartbeat_timed_out_beyond_threshold() {
    let mut monitor = make_heartbeat_monitor(2);
    monitor.record_sent(); // 1
    monitor.record_sent(); // 2 >= threshold
    monitor.record_sent(); // 3
    assert_eq!(monitor.check(), HeartbeatStatus::TimedOut { missed: 3 });
    assert!(!monitor.is_healthy());
}

#[test]
fn test_heartbeat_recovery_from_warning() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent();
    monitor.record_sent();
    assert_eq!(monitor.check(), HeartbeatStatus::Warning { missed: 2 });
    monitor.record_received();
    assert_eq!(monitor.check(), HeartbeatStatus::Healthy);
    assert!(monitor.is_healthy());
}

#[test]
fn test_heartbeat_recovery_from_timeout() {
    let mut monitor = make_heartbeat_monitor(3);
    for _ in 0..5 {
        monitor.record_sent();
    }
    assert!(!monitor.is_healthy());
    monitor.record_received();
    assert!(monitor.is_healthy());
    assert_eq!(monitor.check(), HeartbeatStatus::Healthy);
}

#[test]
fn test_heartbeat_totals_accumulate() {
    let mut monitor = make_heartbeat_monitor(3);
    for _ in 0..10 {
        monitor.record_sent();
    }
    for _ in 0..4 {
        monitor.record_received();
    }
    assert_eq!(monitor.total_sent(), 10);
    assert_eq!(monitor.total_received(), 4);
}

#[test]
fn test_heartbeat_reset() {
    let mut monitor = make_heartbeat_monitor(3);
    monitor.record_sent();
    monitor.record_sent();
    monitor.record_received();
    assert_eq!(monitor.total_sent(), 2);
    assert_eq!(monitor.total_received(), 1);

    monitor.reset();
    assert_eq!(monitor.missed_count(), 0);
    assert_eq!(monitor.total_sent(), 0);
    assert_eq!(monitor.total_received(), 0);
    assert_eq!(monitor.check(), HeartbeatStatus::Healthy);
}

#[test]
fn test_heartbeat_state_snapshot() {
    let mut monitor = make_heartbeat_monitor(3);
    let state = monitor.state();
    assert_eq!(state.consecutive_missed, 0);
    assert!(state.last_received.is_none());
    assert!(state.last_sent.is_none());

    monitor.record_sent();
    let state = monitor.state();
    assert_eq!(state.consecutive_missed, 1);
    assert!(state.last_sent.is_some());
    assert!(state.last_received.is_none());

    monitor.record_received();
    let state = monitor.state();
    assert_eq!(state.consecutive_missed, 0);
    assert!(state.last_received.is_some());
}

#[test]
fn test_heartbeat_timeout_count_accessor() {
    let monitor = make_heartbeat_monitor(7);
    assert_eq!(monitor.timeout_count(), 7);
}

// ===========================================================================
// Resume tokens
// ===========================================================================

#[test]
fn test_resume_token_creation() {
    let token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "user-a".into(),
        "fp-abc".into(),
        Duration::from_secs(3600),
        3,
        TokenScope::SameServer,
    );
    assert_eq!(token.token_id(), "tok-1");
    assert_eq!(token.session_id(), "sess-1");
    assert_eq!(token.user_id(), "user-a");
    assert_eq!(token.client_fingerprint(), "fp-abc");
    assert_eq!(token.scope(), TokenScope::SameServer);
    assert!(token.is_valid());
    assert!(!token.is_expired());
    assert_eq!(token.remaining_uses(), 3);
    assert_eq!(token.use_count(), 0);
}

#[test]
fn test_resume_token_scope_display() {
    assert_eq!(TokenScope::SameServer.to_string(), "SameServer");
    assert_eq!(TokenScope::AnyGateway.to_string(), "AnyGateway");
}

#[test]
fn test_resume_token_record_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "user-a".into(),
        "fp".into(),
        Duration::from_secs(3600),
        2,
        TokenScope::SameServer,
    );
    assert!(token.record_use());
    assert_eq!(token.use_count(), 1);
    assert_eq!(token.remaining_uses(), 1);
    assert!(token.is_valid());

    assert!(token.record_use());
    assert_eq!(token.use_count(), 2);
    assert_eq!(token.remaining_uses(), 0);
    assert!(!token.is_valid());
}

#[test]
fn test_resume_token_exhausted_denies_further_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::from_secs(3600),
        1,
        TokenScope::SameServer,
    );
    assert!(token.record_use());
    assert!(!token.record_use());
    assert_eq!(token.use_count(), 1);
}

#[test]
fn test_resume_token_zero_lifetime_is_expired() {
    let token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::ZERO,
        5,
        TokenScope::AnyGateway,
    );
    // A zero-duration token expires immediately.
    assert!(token.is_expired());
    assert!(!token.is_valid());
}

#[test]
fn test_resume_token_expired_denies_use() {
    let mut token = ResumeToken::new(
        "tok-1".into(),
        "sess-1".into(),
        "u".into(),
        "fp".into(),
        Duration::ZERO,
        5,
        TokenScope::SameServer,
    );
    assert!(!token.record_use());
    assert_eq!(token.use_count(), 0);
}

// ===========================================================================
// Resume manager
// ===========================================================================

#[test]
fn test_resume_manager_issue_and_validate() {
    let mut mgr = ResumeManager::new(make_resume_config());
    assert!(mgr.is_enabled());

    let token_id = mgr.issue_token("sess-1", "user-a", "fp-1").unwrap();
    assert!(token_id.starts_with("resume-"));
    assert_eq!(mgr.token_count(), 1);

    let session_id = mgr.validate_token(&token_id).unwrap();
    assert_eq!(session_id, "sess-1");
}

#[test]
fn test_resume_manager_disabled() {
    let config = ResumeConfig {
        enabled: false,
        ..make_resume_config()
    };
    let mut mgr = ResumeManager::new(config);
    assert!(!mgr.is_enabled());

    let result = mgr.issue_token("sess-1", "user-a", "fp-1");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_validate_invalid_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let result = mgr.validate_token("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_token_ids_increment() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let t1 = mgr.issue_token("s1", "u1", "fp").unwrap();
    let t2 = mgr.issue_token("s2", "u2", "fp").unwrap();
    assert_ne!(t1, t2);
    assert_eq!(t1, "resume-1");
    assert_eq!(t2, "resume-2");
    assert_eq!(mgr.token_count(), 2);
}

#[test]
fn test_resume_manager_validate_consumes_use() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let tid = mgr.issue_token("s1", "u1", "fp").unwrap();

    // Tokens issued by ResumeManager have max_uses = 3.
    mgr.validate_token(&tid).unwrap(); // use 1
    mgr.validate_token(&tid).unwrap(); // use 2
    mgr.validate_token(&tid).unwrap(); // use 3

    // Fourth use should fail (token exhausted).
    let result = mgr.validate_token(&tid);
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_revoke_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let tid = mgr.issue_token("s1", "u1", "fp").unwrap();
    assert_eq!(mgr.token_count(), 1);

    mgr.revoke_token(&tid);
    assert_eq!(mgr.token_count(), 0);

    let result = mgr.validate_token(&tid);
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_revoke_nonexistent_is_noop() {
    let mut mgr = ResumeManager::new(make_resume_config());
    mgr.revoke_token("does-not-exist");
    assert_eq!(mgr.token_count(), 0);
}

#[test]
fn test_resume_manager_rotate_token() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let old_tid = mgr.issue_token("s1", "u1", "fp-1").unwrap();
    assert_eq!(mgr.token_count(), 1);

    let new_tid = mgr.rotate_token(&old_tid).unwrap();
    assert_ne!(old_tid, new_tid);
    assert_eq!(mgr.token_count(), 1);

    // Old token is revoked.
    let result = mgr.validate_token(&old_tid);
    assert!(result.is_err());

    // New token is valid.
    let session_id = mgr.validate_token(&new_tid).unwrap();
    assert_eq!(session_id, "s1");
}

#[test]
fn test_resume_manager_rotate_nonexistent_fails() {
    let mut mgr = ResumeManager::new(make_resume_config());
    let result = mgr.rotate_token("no-such-token");
    assert!(result.is_err());
}

#[test]
fn test_resume_manager_cleanup_expired() {
    let mut mgr = ResumeManager::new(ResumeConfig {
        token_lifetime_hours: 0, // tokens expire immediately
        ..make_resume_config()
    });
    mgr.issue_token("s1", "u1", "fp").unwrap();
    mgr.issue_token("s2", "u2", "fp").unwrap();
    assert_eq!(mgr.token_count(), 2);

    let removed = mgr.cleanup_expired();
    assert_eq!(removed, 2);
    assert_eq!(mgr.token_count(), 0);
}

#[test]
fn test_resume_manager_cleanup_preserves_valid_tokens() {
    let mut mgr = ResumeManager::new(make_resume_config());
    mgr.issue_token("s1", "u1", "fp").unwrap();
    let removed = mgr.cleanup_expired();
    assert_eq!(removed, 0);
    assert_eq!(mgr.token_count(), 1);
}

// ===========================================================================
// Session persistence
// ===========================================================================

#[test]
fn test_persistence_new_has_no_snapshot() {
    let p = SessionPersistence::new();
    assert!(!p.has_snapshot());
    assert!(p.restore().is_none());
}

#[test]
fn test_persistence_default_matches_new() {
    let p = SessionPersistence::default();
    assert!(!p.has_snapshot());
}

#[test]
fn test_persistence_snapshot_and_restore() {
    let mut p = SessionPersistence::new();
    let state = PersistenceState {
        window_positions: vec![(10, 20, 800, 600)],
        clipboard_available: true,
        cursor_position: (100, 200),
        audio_state: "playing".to_string(),
    };
    p.snapshot(state);
    assert!(p.has_snapshot());

    let restored = p.restore().unwrap();
    assert_eq!(restored.window_positions.len(), 1);
    assert_eq!(restored.window_positions[0], (10, 20, 800, 600));
    assert!(restored.clipboard_available);
    assert_eq!(restored.cursor_position, (100, 200));
    assert_eq!(restored.audio_state, "playing");
}

#[test]
fn test_persistence_clear() {
    let mut p = SessionPersistence::new();
    p.snapshot(PersistenceState {
        clipboard_available: true,
        ..PersistenceState::default()
    });
    assert!(p.has_snapshot());

    p.clear();
    assert!(!p.has_snapshot());
    assert!(p.restore().is_none());
}

#[test]
fn test_persistence_state_default() {
    let state = PersistenceState::default();
    assert!(state.window_positions.is_empty());
    assert!(!state.clipboard_available);
    assert_eq!(state.cursor_position, (0, 0));
    assert_eq!(state.audio_state, "muted");
}

// ===========================================================================
// Session runtime initialization and basic operations
// ===========================================================================

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
