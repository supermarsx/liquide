//! Tests for the connection state machine.

use crate::connection::{ConnectionManager, ConnectionState};

// ===========================================================================
// Initial state
// ===========================================================================

#[test]
fn test_initial_state_is_disconnected() {
    let mgr = ConnectionManager::new(5);
    assert_eq!(*mgr.state(), ConnectionState::Disconnected);
    assert!(!mgr.is_connected());
    assert!(mgr.info().is_none());
}

// ===========================================================================
// Connect flow
// ===========================================================================

#[test]
fn test_connect_transitions_to_connecting() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("example.com:3389").unwrap();
    assert_eq!(*mgr.state(), ConnectionState::Connecting);
    assert!(mgr.info().is_some());
    assert_eq!(mgr.info().unwrap().server_address, "example.com:3389");
}

#[test]
fn test_begin_auth_transitions_to_authenticating() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    assert_eq!(*mgr.state(), ConnectionState::Authenticating);
}

#[test]
fn test_connected_transitions_to_connected() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "quic", 1000).unwrap();
    assert!(mgr.is_connected());
    let info = mgr.info().unwrap();
    assert_eq!(info.protocol_version, "1.0");
    assert_eq!(info.transport, "quic");
    assert_eq!(info.connected_at, 1000);
}

#[test]
fn test_connect_while_connected_fails() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    let result = mgr.connect("other");
    assert!(result.is_err());
}

// ===========================================================================
// Disconnect
// ===========================================================================

#[test]
fn test_disconnect_resets_state() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    mgr.disconnect();
    assert_eq!(*mgr.state(), ConnectionState::Disconnected);
    assert!(mgr.info().is_none());
    assert!(!mgr.is_connected());
}

// ===========================================================================
// Suspend / resume
// ===========================================================================

#[test]
fn test_suspend_from_connected() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    mgr.suspend().unwrap();
    assert_eq!(*mgr.state(), ConnectionState::Suspended);
}

#[test]
fn test_suspend_from_disconnected_fails() {
    let mut mgr = ConnectionManager::new(5);
    let result = mgr.suspend();
    assert!(result.is_err());
}

#[test]
fn test_resume_from_suspended() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    mgr.suspend().unwrap();
    mgr.resume().unwrap();
    assert!(mgr.is_connected());
}

#[test]
fn test_resume_from_connected_fails() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    let result = mgr.resume();
    assert!(result.is_err());
}

// ===========================================================================
// Reconnect
// ===========================================================================

#[test]
fn test_reconnect_increments_attempt() {
    let mut mgr = ConnectionManager::new(3);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();

    let attempt = mgr.reconnect_attempt().unwrap();
    assert_eq!(attempt, 1);
    assert_eq!(
        *mgr.state(),
        ConnectionState::Reconnecting { attempt: 1 }
    );

    let attempt = mgr.reconnect_attempt().unwrap();
    assert_eq!(attempt, 2);
}

#[test]
fn test_reconnect_exceeds_max_fails() {
    let mut mgr = ConnectionManager::new(2);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();

    mgr.reconnect_attempt().unwrap(); // 1
    mgr.reconnect_attempt().unwrap(); // 2
    let result = mgr.reconnect_attempt(); // 3 > max(2)
    assert!(result.is_err());
    assert!(matches!(mgr.state(), ConnectionState::Failed { .. }));
}

// ===========================================================================
// Fail
// ===========================================================================

#[test]
fn test_fail_sets_failed_state() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.fail("network unreachable");
    assert!(matches!(mgr.state(), ConnectionState::Failed { .. }));
}

// ===========================================================================
// Display
// ===========================================================================

#[test]
fn test_connection_state_display() {
    assert_eq!(ConnectionState::Disconnected.to_string(), "disconnected");
    assert_eq!(ConnectionState::Connecting.to_string(), "connecting");
    assert_eq!(ConnectionState::Authenticating.to_string(), "authenticating");
    assert_eq!(ConnectionState::Connected.to_string(), "connected");
    assert_eq!(
        ConnectionState::Reconnecting { attempt: 3 }.to_string(),
        "reconnecting (attempt 3)"
    );
    assert_eq!(ConnectionState::Suspended.to_string(), "suspended");
    assert_eq!(
        ConnectionState::Failed {
            reason: "timeout".to_string()
        }
        .to_string(),
        "failed: timeout"
    );
}

// ===========================================================================
// Metrics update
// ===========================================================================

#[test]
fn test_update_metrics() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("host").unwrap();
    mgr.begin_auth().unwrap();
    mgr.connected("1.0", "tcp", 1000).unwrap();
    mgr.update_metrics(12.5, 1_000_000);
    let info = mgr.info().unwrap();
    assert!((info.latency_ms - 12.5).abs() < f32::EPSILON);
    assert_eq!(info.bandwidth_bps, 1_000_000);
}

// ===========================================================================
// Connect from failed state
// ===========================================================================

#[test]
fn test_connect_after_failure() {
    let mut mgr = ConnectionManager::new(5);
    mgr.fail("initial error");
    mgr.connect("new-host").unwrap();
    assert_eq!(*mgr.state(), ConnectionState::Connecting);
}
