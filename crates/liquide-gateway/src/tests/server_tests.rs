use crate::server::{ServerCapabilities, ServerHealth, ServerLoad, ServerRegistry};
use crate::config::ListenTransport;

fn sample_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        max_sessions: 100,
        supported_transports: vec![ListenTransport::TlsTcp],
        supported_encoders: vec!["h264".to_string()],
        gpu_available: true,
        ..ServerCapabilities::default()
    }
}

#[test]
fn test_server_register_and_deregister() {
    let mut registry = ServerRegistry::new();
    let id = registry.register("10.0.0.1:3900".to_string(), sample_capabilities(), 1000);
    assert_eq!(registry.server_count(), 1);
    assert!(registry.get(&id).is_some());

    let removed = registry.deregister(&id);
    assert!(removed.is_some());
    assert_eq!(registry.server_count(), 0);
}

#[test]
fn test_server_healthy_servers() {
    let mut registry = ServerRegistry::new();
    let id1 = registry.register("10.0.0.1:3900".to_string(), sample_capabilities(), 1000);
    let id2 = registry.register("10.0.0.2:3900".to_string(), sample_capabilities(), 1000);

    // Both should be Unknown (not in healthy list).
    assert!(registry.healthy_servers().is_empty());

    // Mark one healthy and one unhealthy.
    registry.update_health(&id1, ServerHealth::Healthy);
    registry.update_health(&id2, ServerHealth::Unhealthy);

    let healthy = registry.healthy_servers();
    assert_eq!(healthy.len(), 1);
    assert!(healthy.contains(&id1));
}

#[test]
fn test_server_heartbeat() {
    let mut registry = ServerRegistry::new();
    let id = registry.register("10.0.0.1:3900".to_string(), sample_capabilities(), 1000);

    {
        let server = registry.get_mut(&id).unwrap();
        server.record_heartbeat(2000);
    }

    let server = registry.get(&id).unwrap();
    assert_eq!(server.last_heartbeat(), 2000);
    assert!(server.keepalive_active());
}

#[test]
fn test_server_load_score() {
    let load = ServerLoad {
        active_sessions: 50,
        cpu_percent: 60.0,
        memory_percent: 40.0,
        bandwidth_percent: 20.0,
    };
    let score = load.score(100);
    // 0.4 * (50/100) + 0.3 * (60/100) + 0.2 * (40/100) + 0.1 * (20/100)
    // = 0.2 + 0.18 + 0.08 + 0.02 = 0.48
    assert!((score - 0.48).abs() < 0.01);
}

#[test]
fn test_server_update_load() {
    let mut registry = ServerRegistry::new();
    let id = registry.register("10.0.0.1:3900".to_string(), sample_capabilities(), 1000);

    let new_load = ServerLoad {
        active_sessions: 10,
        cpu_percent: 25.0,
        memory_percent: 30.0,
        bandwidth_percent: 5.0,
    };

    if let Some(server) = registry.get_mut(&id) {
        server.update_load(new_load);
    }

    let server = registry.get(&id).unwrap();
    assert_eq!(server.load().active_sessions, 10);
    assert!((server.load().cpu_percent - 25.0).abs() < 0.01);
}
