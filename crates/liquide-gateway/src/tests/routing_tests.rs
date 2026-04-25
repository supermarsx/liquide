use crate::config::ListenTransport;
use crate::routing::{Router, RoutingStrategy};
use crate::server::{ServerCapabilities, ServerHealth, ServerLoad, ServerRegistry};
use std::collections::HashMap;

fn make_registry(count: usize) -> ServerRegistry {
    let mut registry = ServerRegistry::new();
    for i in 0..count {
        let mut caps = ServerCapabilities {
            max_sessions: 100,
            supported_transports: vec![ListenTransport::TlsTcp],
            supported_encoders: vec!["h264".to_string()],
            gpu_available: true,
            tags: HashMap::new(),
        };
        caps.tags
            .insert("region".to_string(), format!("region-{}", i % 2));

        let id = registry.register(format!("10.0.0.{}:3900", i + 1), caps, 1000);
        registry.update_health(&id, ServerHealth::Healthy);
    }
    registry
}

fn make_registry_with_load() -> ServerRegistry {
    let mut registry = ServerRegistry::new();

    let caps = ServerCapabilities::default();
    let id1 = registry.register("10.0.0.1:3900".to_string(), caps.clone(), 1000);
    let id2 = registry.register("10.0.0.2:3900".to_string(), caps, 1000);

    registry.update_health(&id1, ServerHealth::Healthy);
    registry.update_health(&id2, ServerHealth::Healthy);

    // Server 1 has high load.
    if let Some(s) = registry.get_mut(&id1) {
        s.update_load(ServerLoad {
            active_sessions: 40,
            cpu_percent: 80.0,
            memory_percent: 70.0,
            bandwidth_percent: 50.0,
        });
    }

    // Server 2 has low load.
    if let Some(s) = registry.get_mut(&id2) {
        s.update_load(ServerLoad {
            active_sessions: 5,
            cpu_percent: 10.0,
            memory_percent: 15.0,
            bandwidth_percent: 2.0,
        });
    }

    registry
}

#[test]
fn test_round_robin_routing() {
    let registry = make_registry(3);
    let mut router = Router::new(RoutingStrategy::RoundRobin);

    let d1 = router.route("192.168.1.1", &registry, None).unwrap();
    let d2 = router.route("192.168.1.2", &registry, None).unwrap();
    let d3 = router.route("192.168.1.3", &registry, None).unwrap();
    let d4 = router.route("192.168.1.4", &registry, None).unwrap();

    // After 3 routes we should cycle back, so d4 should match d1.
    assert_eq!(d4.target_server_id, d1.target_server_id);
    // Each consecutive call should pick a different server from the previous.
    assert_ne!(d1.target_server_id, d2.target_server_id);
}

#[test]
fn test_least_load_routing() {
    let registry = make_registry_with_load();
    let mut router = Router::new(RoutingStrategy::LeastLoad);

    let decision = router.route("192.168.1.1", &registry, None).unwrap();
    // Server 2 has much lower load, so it should be picked.
    assert_eq!(decision.target_server_id, "srv-2");
}

#[test]
fn test_direct_routing() {
    let registry = make_registry(2);
    let mut router = Router::new(RoutingStrategy::Direct);

    // Direct requires explicit server.
    let result = router.route("192.168.1.1", &registry, None);
    assert!(result.is_err());

    // Direct with explicit server should succeed.
    let decision = router
        .route("192.168.1.1", &registry, Some("srv-1"))
        .unwrap();
    assert_eq!(decision.target_server_id, "srv-1");
}

#[test]
fn test_routing_no_healthy_servers() {
    let registry = ServerRegistry::new();
    let mut router = Router::new(RoutingStrategy::RoundRobin);

    let result = router.route("192.168.1.1", &registry, None);
    assert!(result.is_err());
}

#[test]
fn test_sticky_routing() {
    let registry = make_registry(3);
    let mut router = Router::new(RoutingStrategy::Sticky);

    let d1 = router.route("192.168.1.1", &registry, None).unwrap();
    let d2 = router.route("192.168.1.1", &registry, None).unwrap();
    // Same client IP should get same server.
    assert_eq!(d1.target_server_id, d2.target_server_id);

    // Different client should get (potentially) different server.
    let d3 = router.route("192.168.1.2", &registry, None).unwrap();
    // We can't guarantee it's different with 3 servers and round-robin fallback,
    // but we can verify it still works and the binding is established.
    let d4 = router.route("192.168.1.2", &registry, None).unwrap();
    assert_eq!(d3.target_server_id, d4.target_server_id);
}

#[test]
fn test_sticky_clear() {
    let registry = make_registry(3);
    let mut router = Router::new(RoutingStrategy::Sticky);

    let d1 = router.route("192.168.1.1", &registry, None).unwrap();
    router.clear_sticky("192.168.1.1");
    let d2 = router.route("192.168.1.1", &registry, None).unwrap();

    // After clearing, it may or may not get the same server, but the operation should succeed.
    assert!(!d1.target_server_id.is_empty());
    assert!(!d2.target_server_id.is_empty());
}

#[test]
fn test_tag_based_routing() {
    let registry = make_registry(2);
    let mut router = Router::new(RoutingStrategy::TagBased);

    let mut filters = HashMap::new();
    filters.insert("region".to_string(), "region-0".to_string());
    router.set_tag_filters(filters);

    let decision = router.route("192.168.1.1", &registry, None).unwrap();
    // Server with index 0 has tag region-0 -> srv-1.
    assert_eq!(decision.target_server_id, "srv-1");
}
