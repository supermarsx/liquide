//! Service manager for the LiquiDE desktop environment.
//!
//! Provides a generic service lifecycle manager covering:
//! - Service registration and configuration (`service`)
//! - State machine transitions with event logging (`registry`)
//! - Dependency graph resolution with cycle detection (`dependency`)
//! - Health monitoring with thresholds and auto-recovery (`health`)
//! - Process watchdog with exponential backoff restarts (`watchdog`)

pub mod dependency;
pub mod health;
pub mod registry;
pub mod service;
pub mod watchdog;

pub use dependency::DependencyGraph;
pub use health::{
    HealthCheck, HealthConfig, HealthEvent, HealthMonitor, HealthReport, HealthStatus,
};
pub use registry::{RegistryError, ServiceEvent, ServiceRegistry};
pub use service::{RestartPolicy, ServiceConfig, ServiceId, ServiceInfo, ServiceState};
pub use watchdog::{Watchdog, WatchdogConfig, WatchdogEvent};

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test: full lifecycle with dependencies, health, and watchdog.
    #[test]
    fn integration_full_lifecycle() {
        // 1. Set up services with dependencies
        let dbus_cfg = ServiceConfig::new("dbus", "/usr/bin/dbus-daemon")
            .with_display_name("D-Bus Message Bus")
            .with_auto_start(true)
            .with_restart_policy(RestartPolicy::Always);

        let audio_cfg = ServiceConfig::new("audio", "/usr/bin/pipewire")
            .with_display_name("Audio Server")
            .with_auto_start(true)
            .with_restart_policy(RestartPolicy::OnFailureWithBackoff)
            .with_dependency("dbus");

        let compositor_cfg = ServiceConfig::new("compositor", "/usr/bin/compositor")
            .with_display_name("Compositor")
            .with_auto_start(true)
            .with_restart_policy(RestartPolicy::OnFailure)
            .with_dependency("dbus");

        // 2. Build dependency graph
        let mut graph = DependencyGraph::new();
        for cfg in [&dbus_cfg, &audio_cfg, &compositor_cfg] {
            graph.add_service(cfg.id.clone());
            for dep in &cfg.dependencies {
                graph.add_dependency(cfg.id.clone(), dep.clone());
            }
        }

        assert!(graph.has_cycle().is_none());

        // 3. Get start order for compositor (needs dbus)
        let order = graph.start_order(&ServiceId::new("compositor")).unwrap();
        assert_eq!(order[0], ServiceId::new("dbus"));
        assert_eq!(order[1], ServiceId::new("compositor"));

        // 4. Register services
        let mut registry = ServiceRegistry::new();
        registry.register(dbus_cfg).unwrap();
        registry.register(audio_cfg).unwrap();
        registry.register(compositor_cfg).unwrap();

        // 5. Start in dependency order
        registry
            .start_service(&ServiceId::new("dbus"), 1000)
            .unwrap();
        registry
            .start_service(&ServiceId::new("audio"), 1001)
            .unwrap();
        registry
            .start_service(&ServiceId::new("compositor"), 1002)
            .unwrap();

        assert_eq!(registry.all_services().len(), 3);

        // 6. Set up health monitoring
        let mut health = HealthMonitor::new();
        health.register(ServiceId::new("dbus"), HealthConfig::default());
        health.register(ServiceId::new("audio"), HealthConfig::default());
        health.register(ServiceId::new("compositor"), HealthConfig::default());

        health.record_check(&ServiceId::new("dbus"), HealthStatus::Healthy);
        health.record_check(&ServiceId::new("audio"), HealthStatus::Healthy);
        health.record_check(&ServiceId::new("compositor"), HealthStatus::Healthy);

        let report = health.report();
        assert!(report.all_healthy());

        // 7. Set up watchdog
        let mut watchdog = Watchdog::new();
        watchdog.register_pid(ServiceId::new("dbus"), 1000);
        watchdog.register_pid(ServiceId::new("audio"), 1001);
        watchdog.register_pid(ServiceId::new("compositor"), 1002);

        // All alive
        let tick_events = watchdog.tick(1000);
        assert_eq!(tick_events.len(), 3);

        // 8. Simulate audio crash
        watchdog.report_exit(&ServiceId::new("audio"), 1);
        watchdog.tick(2000);

        // Audio should have pending restart
        let pending = watchdog.pending_restarts();
        assert!(pending.contains(&ServiceId::new("audio")));

        // 9. Registry: mark audio as failed, then restart
        registry
            .mark_failed(&ServiceId::new("audio"), "unexpected exit")
            .unwrap();
        assert!(matches!(
            registry.service_state(&ServiceId::new("audio")).unwrap(),
            ServiceState::Failed(_)
        ));

        registry
            .restart_service(&ServiceId::new("audio"), 2001)
            .unwrap();
        assert!(
            registry
                .service_state(&ServiceId::new("audio"))
                .unwrap()
                .is_running()
        );

        watchdog.mark_restarted(&ServiceId::new("audio"), 2001);
        assert!(watchdog.is_alive(&ServiceId::new("audio")));

        // 10. Stop order for dbus (audio and compositor depend on it)
        let stop_order = graph.stop_order(&ServiceId::new("dbus"));
        // Must stop dependents before dbus
        let dbus_pos = stop_order
            .iter()
            .position(|id| id == &ServiceId::new("dbus"))
            .unwrap();
        assert_eq!(dbus_pos, stop_order.len() - 1); // dbus is last to stop
    }

    #[test]
    fn integration_cycle_detection_prevents_start() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency(ServiceId::new("a"), ServiceId::new("b"));
        graph.add_dependency(ServiceId::new("b"), ServiceId::new("c"));
        graph.add_dependency(ServiceId::new("c"), ServiceId::new("a"));

        // Cycle detected
        assert!(graph.has_cycle().is_some());

        // Cannot compute start order
        assert!(graph.start_order(&ServiceId::new("a")).is_none());
    }

    #[test]
    fn integration_auto_start_with_dependencies() {
        let mut registry = ServiceRegistry::new();
        let mut graph = DependencyGraph::new();

        let cfgs = vec![
            ServiceConfig::new("base", "/bin/base").with_auto_start(true),
            ServiceConfig::new("mid", "/bin/mid")
                .with_auto_start(true)
                .with_dependency("base"),
            ServiceConfig::new("top", "/bin/top")
                .with_auto_start(true)
                .with_dependency("mid"),
            ServiceConfig::new("optional", "/bin/opt").with_auto_start(false),
        ];

        for cfg in cfgs {
            graph.add_service(cfg.id.clone());
            for dep in &cfg.dependencies {
                graph.add_dependency(cfg.id.clone(), dep.clone());
            }
            registry.register(cfg).unwrap();
        }

        // Only auto-start services
        let auto = registry.auto_start_services();
        assert_eq!(auto.len(), 3);

        // Start in dependency order
        let order = graph.start_order(&ServiceId::new("top")).unwrap();
        assert_eq!(
            order,
            vec![
                ServiceId::new("base"),
                ServiceId::new("mid"),
                ServiceId::new("top"),
            ]
        );
    }

    #[test]
    fn integration_health_driven_restart() {
        let id = ServiceId::new("fragile");
        let cfg = ServiceConfig::new("fragile", "/bin/fragile")
            .with_restart_policy(RestartPolicy::OnFailure);

        let mut registry = ServiceRegistry::new();
        registry.register(cfg).unwrap();
        registry.start_service(&id, 500).unwrap();

        let mut health = HealthMonitor::new();
        health.register(
            id.clone(),
            HealthConfig {
                failure_threshold: 2,
                ..Default::default()
            },
        );

        let mut watchdog = Watchdog::new();
        watchdog.register_pid(id.clone(), 500);

        // Two unhealthy checks -> triggers unhealthy
        health.record_check(&id, HealthStatus::Unhealthy("timeout".into()));
        health.record_check(&id, HealthStatus::Unhealthy("timeout".into()));

        assert!(health.should_restart(&id));

        // Check restart policy allows it
        let policy = registry.restart_policy(&id).unwrap();
        assert!(matches!(policy, RestartPolicy::OnFailure));

        // Perform restart
        registry.mark_failed(&id, "health check failed").unwrap();
        watchdog.report_exit(&id, 1);
        watchdog.tick(0);

        registry.restart_service(&id, 501).unwrap();
        watchdog.mark_restarted(&id, 501);

        assert!(registry.service_state(&id).unwrap().is_running());
        assert!(watchdog.is_alive(&id));
    }

    #[test]
    fn integration_enable_disable_auto_start() {
        let mut registry = ServiceRegistry::new();
        let cfg = ServiceConfig::new("svc", "/bin/svc");
        registry.register(cfg).unwrap();

        let id = ServiceId::new("svc");
        assert!(!registry.service_info(&id).unwrap().enabled);

        registry.enable_service(&id).unwrap();
        assert!(registry.service_info(&id).unwrap().enabled);
        assert_eq!(registry.auto_start_services().len(), 1);

        registry.disable_service(&id).unwrap();
        assert!(!registry.service_info(&id).unwrap().enabled);
        assert_eq!(registry.auto_start_services().len(), 0);
    }
}
