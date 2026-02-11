use crate::connection::{ConnectionManager, ConnectionQuality, ConnectionState};

#[test]
fn test_initial_state_is_disconnected() {
    let mgr = ConnectionManager::new(5);
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
}

#[test]
fn test_connect_transitions_to_connected() {
    let mut mgr = ConnectionManager::new(5);
    let result = mgr.connect("example.com:3389");
    assert!(result.is_ok());
    assert_eq!(mgr.state(), ConnectionState::Connected);
}

#[test]
fn test_disconnect_returns_to_disconnected() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("example.com:3389").unwrap();
    mgr.disconnect();
    assert_eq!(mgr.state(), ConnectionState::Disconnected);
}

#[test]
fn test_reconnect_succeeds_within_limit() {
    let mut mgr = ConnectionManager::new(3);
    mgr.connect("example.com:3389").unwrap();
    mgr.disconnect();
    // Re-set server_addr so reconnect knows where to go.
    mgr.connect("example.com:3389").unwrap();
    // Simulate disconnect requiring reconnect.
    mgr.disconnect();
    mgr.connect("example.com:3389").unwrap();
    let result = mgr.reconnect();
    assert!(result.is_ok());
}

#[test]
fn test_reconnect_fails_when_no_server() {
    let mut mgr = ConnectionManager::new(3);
    let result = mgr.reconnect();
    assert!(result.is_err());
}

#[test]
fn test_quality_when_disconnected() {
    let mgr = ConnectionManager::new(5);
    assert_eq!(mgr.quality(), ConnectionQuality::Disconnected);
}

#[test]
fn test_quality_excellent() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("example.com:3389").unwrap();
    mgr.update_metrics(10.0, 0.0, 100.0);
    assert_eq!(mgr.quality(), ConnectionQuality::Excellent);
}

#[test]
fn test_quality_good() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("example.com:3389").unwrap();
    mgr.update_metrics(60.0, 0.1, 50.0);
    assert_eq!(mgr.quality(), ConnectionQuality::Good);
}

#[test]
fn test_quality_bad() {
    let mut mgr = ConnectionManager::new(5);
    mgr.connect("example.com:3389").unwrap();
    mgr.update_metrics(350.0, 12.0, 5.0);
    assert_eq!(mgr.quality(), ConnectionQuality::Bad);
}

#[test]
fn test_from_metrics_factory() {
    assert_eq!(
        ConnectionQuality::from_metrics(5.0, 0.0, false),
        ConnectionQuality::Excellent,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(150.0, 0.0, false),
        ConnectionQuality::Fair,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(250.0, 6.0, false),
        ConnectionQuality::Poor,
    );
    assert_eq!(
        ConnectionQuality::from_metrics(0.0, 0.0, false),
        ConnectionQuality::Disconnected,
    );
}

#[test]
fn test_connection_profiles() {
    use crate::connection::ConnectionProfile;

    let mut mgr = ConnectionManager::new(5);
    assert!(mgr.profiles().is_empty());

    mgr.add_profile(ConnectionProfile {
        name: "Work".to_string(),
        address: "work.example.com:3389".to_string(),
        username: Some("alice".to_string()),
        transport: "quic".to_string(),
        encoder: "h265".to_string(),
        encryption: "aes256".to_string(),
        monitors: 2,
        audio_playback: true,
        audio_microphone: false,
        clipboard: true,
        performance: "balanced".to_string(),
        cursor_mode: "local_predict".to_string(),
    });

    assert_eq!(mgr.profiles().len(), 1);
    assert!(mgr.remove_profile("Work"));
    assert!(mgr.profiles().is_empty());
}

#[test]
fn test_next_reconnect_delay_exponential() {
    let mgr = ConnectionManager::new(10);
    // First attempt: 1000 * 2^0 = 1000
    assert_eq!(mgr.next_reconnect_delay_ms(), 1000);
}

#[test]
fn test_quality_color_values() {
    assert_eq!(ConnectionQuality::Excellent.color(), "#00c853");
    assert_eq!(ConnectionQuality::Bad.color(), "#d50000");
    assert_eq!(ConnectionQuality::Disconnected.color(), "#9e9e9e");
}
