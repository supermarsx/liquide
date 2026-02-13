use crate::heartbeat::{HeartbeatConfig, HeartbeatMonitor, HeartbeatStatus};

fn make_heartbeat_monitor(timeout_count: u32) -> HeartbeatMonitor {
    HeartbeatMonitor::new(HeartbeatConfig {
        interval_sec: 5,
        timeout_count,
    })
}

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
