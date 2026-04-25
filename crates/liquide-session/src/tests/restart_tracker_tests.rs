use crate::crash::{DisabledFeature, ResourceSnapshot, RestartAction, RestartTracker, SafeMode};

fn make_restart_tracker(max: u32, safe_threshold: u32) -> RestartTracker {
    RestartTracker::new(max, 600, 100, safe_threshold)
}

// ---------------------------------------------------------------------------
// Restart tracker, backoff, and safe mode escalation
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Safe mode features
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Resource snapshot defaults
// ---------------------------------------------------------------------------

#[test]
fn test_resource_snapshot_default() {
    let snap = ResourceSnapshot::default();
    assert_eq!(snap.cpu_percent, 0.0);
    assert_eq!(snap.memory_mb, 0);
    assert_eq!(snap.io_bytes, 0);
}
