use crate::config::RelayConfig;
use crate::relay::RelayManager;

fn make_manager() -> RelayManager {
    RelayManager::new(RelayConfig {
        enabled: true,
        max_relay_sessions: 3,
        max_bandwidth_mbps: 1000,
        splice_buffer_bytes: 65_536,
    })
}

#[test]
fn test_relay_create_and_terminate() {
    let mut manager = make_manager();

    let id = manager
        .create_relay("conn-1".to_string(), "conn-2".to_string(), 1000)
        .unwrap();
    assert_eq!(manager.active_count(), 1);

    let session = manager.get(&id).unwrap();
    assert!(session.is_active());
    assert_eq!(session.client_connection_id(), "conn-1");
    assert_eq!(session.server_connection_id(), "conn-2");

    manager.terminate_relay(&id).unwrap();
    assert_eq!(manager.active_count(), 0);
    let session = manager.get(&id).unwrap();
    assert!(!session.is_active());
}

#[test]
fn test_relay_capacity_exceeded() {
    let mut manager = make_manager();

    manager
        .create_relay("c1".to_string(), "s1".to_string(), 1000)
        .unwrap();
    manager
        .create_relay("c2".to_string(), "s2".to_string(), 1001)
        .unwrap();
    manager
        .create_relay("c3".to_string(), "s3".to_string(), 1002)
        .unwrap();

    // Fourth should fail.
    let result = manager.create_relay("c4".to_string(), "s4".to_string(), 1003);
    assert!(result.is_err());
}

#[test]
fn test_relay_data_tracking() {
    let mut manager = make_manager();
    let id = manager
        .create_relay("c1".to_string(), "s1".to_string(), 1000)
        .unwrap();

    manager.relay_data(&id, 1024, 2048).unwrap();
    manager.relay_data(&id, 512, 256).unwrap();

    let session = manager.get(&id).unwrap();
    assert_eq!(session.bytes_forwarded_in(), 1536);
    assert_eq!(session.bytes_forwarded_out(), 2304);
}

#[test]
fn test_relay_duration() {
    let mut manager = make_manager();
    let id = manager
        .create_relay("c1".to_string(), "s1".to_string(), 1000)
        .unwrap();

    let session = manager.get(&id).unwrap();
    assert_eq!(session.duration_seconds(1060), 60);
}

#[test]
fn test_relay_total_bandwidth() {
    let mut manager = make_manager();
    let id1 = manager
        .create_relay("c1".to_string(), "s1".to_string(), 1000)
        .unwrap();
    let id2 = manager
        .create_relay("c2".to_string(), "s2".to_string(), 1001)
        .unwrap();

    manager.relay_data(&id1, 100, 200).unwrap();
    manager.relay_data(&id2, 300, 400).unwrap();

    assert_eq!(manager.total_bandwidth(), 1000);
}

#[test]
fn test_relay_disabled() {
    let mut manager = RelayManager::new(RelayConfig {
        enabled: false,
        ..RelayConfig::default()
    });

    let result = manager.create_relay("c1".to_string(), "s1".to_string(), 1000);
    assert!(result.is_err());
}
