use crate::health::*;
use crate::inhibitor::*;
use crate::lifecycle::*;
use crate::registry::*;
use crate::service::*;
use crate::shutdown::*;
use crate::state::*;

// ── Registry tests ──────────────────────────────────────────────────

#[test]
fn register_and_lookup() {
    let mut reg = ServiceRegistry::new();
    let desc = ServiceDescriptor {
        id: ServiceId("test-svc".into()),
        name: "Test Service".into(),
        ..Default::default()
    };
    reg.register(desc);
    assert_eq!(reg.service_count(), 1);
    let entry = reg.get(&ServiceId("test-svc".into())).unwrap();
    assert_eq!(entry.state, ServiceState::Stopped);
    assert_eq!(entry.descriptor.name, "Test Service");
}

#[test]
fn topological_sort_basic() {
    let mut reg = ServiceRegistry::new();
    // A depends on B, B depends on C
    reg.register(ServiceDescriptor {
        id: ServiceId("A".into()),
        depends_on: vec![ServiceId("B".into())],
        priority: 30,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("B".into()),
        depends_on: vec![ServiceId("C".into())],
        priority: 20,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("C".into()),
        priority: 10,
        ..Default::default()
    });

    let order = reg.startup_order().unwrap();
    let ids: Vec<&str> = order.iter().map(|id| id.0.as_str()).collect();

    // C must come before B, B before A
    let pos_c = ids.iter().position(|&x| x == "C").unwrap();
    let pos_b = ids.iter().position(|&x| x == "B").unwrap();
    let pos_a = ids.iter().position(|&x| x == "A").unwrap();
    assert!(pos_c < pos_b);
    assert!(pos_b < pos_a);
}

#[test]
fn cycle_detection() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("X".into()),
        depends_on: vec![ServiceId("Y".into())],
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("Y".into()),
        depends_on: vec![ServiceId("X".into())],
        ..Default::default()
    });

    let result = reg.startup_order();
    assert!(result.is_err());
    let cycle = result.unwrap_err();
    assert_eq!(cycle.services.len(), 2);
}

#[test]
fn shutdown_order_reverses_startup() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("base".into()),
        priority: 10,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("mid".into()),
        depends_on: vec![ServiceId("base".into())],
        priority: 20,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("top".into()),
        depends_on: vec![ServiceId("mid".into())],
        priority: 30,
        ..Default::default()
    });

    let startup = reg.startup_order().unwrap();
    let shutdown = reg.shutdown_order().unwrap();

    let startup_ids: Vec<&str> = startup.iter().map(|id| id.0.as_str()).collect();
    let shutdown_ids: Vec<&str> = shutdown.iter().map(|id| id.0.as_str()).collect();

    // Shutdown should be exact reverse of startup
    let mut reversed = startup_ids.clone();
    reversed.reverse();
    assert_eq!(shutdown_ids, reversed);
}

#[test]
fn dependents_lookup() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("parent".into()),
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("child1".into()),
        depends_on: vec![ServiceId("parent".into())],
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("child2".into()),
        depends_on: vec![ServiceId("parent".into())],
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("unrelated".into()),
        ..Default::default()
    });

    let deps = reg.dependents(&ServiceId("parent".into()));
    assert_eq!(deps.len(), 2);
    assert!(deps.contains(&ServiceId("child1".into())));
    assert!(deps.contains(&ServiceId("child2".into())));
}

#[test]
fn auto_start_services_filter() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("auto".into()),
        auto_start: true,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("manual".into()),
        auto_start: false,
        ..Default::default()
    });

    let auto = reg.auto_start_services();
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0], ServiceId("auto".into()));
}

// ── Lifecycle tests ─────────────────────────────────────────────────

#[test]
fn start_builtin_service() {
    let mut reg = ServiceRegistry::new();
    // Built-in service (empty exec path) — should start immediately
    reg.register(ServiceDescriptor {
        id: ServiceId("builtin".into()),
        ..Default::default()
    });

    let mut lm = LifecycleManager::new();
    let result = lm.start_service(&ServiceId("builtin".into()), &mut reg);
    assert!(result.is_ok());

    let entry = reg.get(&ServiceId("builtin".into())).unwrap();
    assert_eq!(entry.state, ServiceState::Running);
    assert!(entry.last_start.is_some());
}

#[test]
fn start_already_running_is_noop() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("svc".into()),
        ..Default::default()
    });

    let mut lm = LifecycleManager::new();
    lm.start_service(&ServiceId("svc".into()), &mut reg)
        .unwrap();
    // Second start should be a no-op
    let result = lm.start_service(&ServiceId("svc".into()), &mut reg);
    assert!(result.is_ok());
}

#[test]
fn start_disabled_service_errors() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("disabled".into()),
        ..Default::default()
    });
    reg.set_state(&ServiceId("disabled".into()), ServiceState::Disabled);

    let mut lm = LifecycleManager::new();
    let result = lm.start_service(&ServiceId("disabled".into()), &mut reg);
    assert!(matches!(result, Err(LifecycleError::Disabled(_))));
}

#[test]
fn start_not_found_errors() {
    let mut reg = ServiceRegistry::new();
    let mut lm = LifecycleManager::new();
    let result = lm.start_service(&ServiceId("ghost".into()), &mut reg);
    assert!(matches!(result, Err(LifecycleError::NotFound(_))));
}

#[test]
fn stop_builtin_service() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("svc".into()),
        ..Default::default()
    });

    let mut lm = LifecycleManager::new();
    lm.start_service(&ServiceId("svc".into()), &mut reg)
        .unwrap();
    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Running
    );

    lm.stop_service(&ServiceId("svc".into()), &mut reg).unwrap();
    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Stopped
    );
}

#[test]
fn start_all_starts_auto_services() {
    let mut reg = ServiceRegistry::new();
    for svc in builtin_services() {
        reg.register(svc);
    }

    let mut lm = LifecycleManager::new();
    let errors = lm.start_all(&mut reg);
    assert!(errors.is_empty(), "start_all errors: {:?}", errors);

    // All auto-start services should be running
    for entry in reg.all_services() {
        if entry.descriptor.auto_start {
            assert_eq!(
                entry.state,
                ServiceState::Running,
                "service {} should be running",
                entry.descriptor.id
            );
        }
    }
}

#[test]
fn stop_all_stops_everything() {
    let mut reg = ServiceRegistry::new();
    for svc in builtin_services() {
        reg.register(svc);
    }

    let mut lm = LifecycleManager::new();
    lm.start_all(&mut reg);
    lm.stop_all(&mut reg);

    for entry in reg.all_services() {
        assert_eq!(
            entry.state,
            ServiceState::Stopped,
            "service {} should be stopped",
            entry.descriptor.id
        );
    }
}

#[test]
fn dependency_starts_before_dependent() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("dep".into()),
        priority: 10,
        ..Default::default()
    });
    reg.register(ServiceDescriptor {
        id: ServiceId("main".into()),
        depends_on: vec![ServiceId("dep".into())],
        priority: 20,
        ..Default::default()
    });

    let mut lm = LifecycleManager::new();
    // Starting "main" should auto-start "dep" first
    lm.start_service(&ServiceId("main".into()), &mut reg)
        .unwrap();

    assert_eq!(
        reg.get(&ServiceId("dep".into())).unwrap().state,
        ServiceState::Running
    );
    assert_eq!(
        reg.get(&ServiceId("main".into())).unwrap().state,
        ServiceState::Running
    );
}

// ── Health tests ────────────────────────────────────────────────────

#[test]
fn health_all_healthy() {
    let mut hc = HealthCheck::new();
    hc.add_check(Box::new(|| HealthStatus::Healthy));
    hc.add_check(Box::new(|| HealthStatus::Healthy));
    assert_eq!(hc.run_all(), HealthStatus::Healthy);
}

#[test]
fn health_degraded_propagates() {
    let mut hc = HealthCheck::new();
    hc.add_check(Box::new(|| HealthStatus::Healthy));
    hc.add_check(Box::new(|| HealthStatus::Degraded));
    assert_eq!(hc.run_all(), HealthStatus::Degraded);
}

#[test]
fn health_unhealthy_wins() {
    let mut hc = HealthCheck::new();
    hc.add_check(Box::new(|| HealthStatus::Healthy));
    hc.add_check(Box::new(|| HealthStatus::Degraded));
    hc.add_check(Box::new(|| HealthStatus::Unhealthy));
    assert_eq!(hc.run_all(), HealthStatus::Unhealthy);
}

#[test]
fn health_empty_is_healthy() {
    let hc = HealthCheck::new();
    assert_eq!(hc.run_all(), HealthStatus::Healthy);
}

#[test]
fn health_unknown_between_healthy_and_degraded() {
    let mut hc = HealthCheck::new();
    hc.add_check(Box::new(|| HealthStatus::Healthy));
    hc.add_check(Box::new(|| HealthStatus::Unknown));
    assert_eq!(hc.run_all(), HealthStatus::Unknown);
}

// ── Builtin services tests ─────────────────────────────────────────

#[test]
fn builtin_services_non_empty() {
    let services = builtin_services();
    assert!(!services.is_empty());
    assert!(services.len() >= 10);
}

#[test]
fn builtin_services_compositor_has_no_deps() {
    let services = builtin_services();
    let compositor = services.iter().find(|s| s.id.0 == "compositor").unwrap();
    assert!(compositor.depends_on.is_empty());
}

#[test]
fn builtin_startup_order_compositor_before_input() {
    let mut reg = ServiceRegistry::new();
    for svc in builtin_services() {
        reg.register(svc);
    }

    let order = reg.startup_order().unwrap();
    let ids: Vec<&str> = order.iter().map(|id| id.0.as_str()).collect();

    let pos_comp = ids.iter().position(|&x| x == "compositor").unwrap();
    let pos_input = ids.iter().position(|&x| x == "input-manager").unwrap();
    assert!(
        pos_comp < pos_input,
        "compositor (pos {}) should start before input-manager (pos {})",
        pos_comp,
        pos_input
    );
}

#[test]
fn builtin_services_no_cycles() {
    let mut reg = ServiceRegistry::new();
    for svc in builtin_services() {
        reg.register(svc);
    }
    assert!(reg.startup_order().is_ok());
}

#[test]
fn service_state_transitions() {
    let mut reg = ServiceRegistry::new();
    reg.register(ServiceDescriptor {
        id: ServiceId("svc".into()),
        ..Default::default()
    });

    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Stopped
    );

    reg.set_state(&ServiceId("svc".into()), ServiceState::Starting);
    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Starting
    );

    reg.set_state(&ServiceId("svc".into()), ServiceState::Running);
    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Running
    );

    reg.set_state(&ServiceId("svc".into()), ServiceState::Failed);
    assert_eq!(
        reg.get(&ServiceId("svc".into())).unwrap().state,
        ServiceState::Failed
    );
}

#[test]
fn lifecycle_error_display() {
    let err = LifecycleError::NotFound(ServiceId("foo".into()));
    assert!(err.to_string().contains("foo"));

    let err = LifecycleError::DependencyCycle(vec![ServiceId("a".into()), ServiceId("b".into())]);
    let msg = err.to_string();
    assert!(msg.contains("a"));
    assert!(msg.contains("b"));
}

// ── Session State tests ────────────────────────────────────────────

#[test]
fn session_state_display() {
    assert_eq!(SessionState::Starting.to_string(), "starting");
    assert_eq!(SessionState::Running.to_string(), "running");
    assert_eq!(SessionState::Locking.to_string(), "locking");
    assert_eq!(SessionState::Locked.to_string(), "locked");
    assert_eq!(SessionState::ShuttingDown.to_string(), "shutting-down");
    assert_eq!(SessionState::LoggingOut.to_string(), "logging-out");
}

#[test]
fn snapshot_serialize_deserialize_roundtrip() {
    let snap = SessionSnapshot {
        name: "default".to_string(),
        timestamp_ms: 1700000000000,
        windows: vec![
            SessionWindow {
                app_id: "firefox".to_string(),
                title: "Mozilla Firefox".to_string(),
                geometry: (100, 200, 1280, 720),
                workspace: 0,
                is_maximized: false,
                is_minimized: false,
            },
            SessionWindow {
                app_id: "terminal".to_string(),
                title: "Terminal".to_string(),
                geometry: (0, 0, 800, 600),
                workspace: 1,
                is_maximized: true,
                is_minimized: false,
            },
        ],
        active_workspace: 0,
        focused_window: Some("firefox".to_string()),
    };

    let serialized = serialize_snapshot(&snap);
    let deserialized = deserialize_snapshot(&serialized).unwrap();
    assert_eq!(deserialized.name, snap.name);
    assert_eq!(deserialized.timestamp_ms, snap.timestamp_ms);
    assert_eq!(deserialized.active_workspace, snap.active_workspace);
    assert_eq!(deserialized.focused_window, snap.focused_window);
    assert_eq!(deserialized.windows.len(), 2);
    assert_eq!(deserialized.windows[0].app_id, "firefox");
    assert_eq!(deserialized.windows[0].geometry, (100, 200, 1280, 720));
    assert_eq!(deserialized.windows[1].is_maximized, true);
    assert_eq!(deserialized.windows[1].workspace, 1);
}

#[test]
fn snapshot_roundtrip_empty_windows() {
    let snap = SessionSnapshot::new("empty", 12345);
    let serialized = serialize_snapshot(&snap);
    let deserialized = deserialize_snapshot(&serialized).unwrap();
    assert_eq!(deserialized.name, "empty");
    assert_eq!(deserialized.timestamp_ms, 12345);
    assert!(deserialized.windows.is_empty());
    assert_eq!(deserialized.active_workspace, 0);
    assert_eq!(deserialized.focused_window, None);
}

#[test]
fn snapshot_roundtrip_special_characters() {
    let snap = SessionSnapshot {
        name: "work\tsession".to_string(),
        timestamp_ms: 999,
        windows: vec![SessionWindow {
            app_id: "app".to_string(),
            title: "Title with\ttab and\nnewline".to_string(),
            geometry: (-10, -20, 100, 200),
            workspace: 0,
            is_maximized: false,
            is_minimized: true,
        }],
        active_workspace: 0,
        focused_window: None,
    };

    let serialized = serialize_snapshot(&snap);
    let deserialized = deserialize_snapshot(&serialized).unwrap();
    assert_eq!(deserialized.name, "work\tsession");
    assert_eq!(
        deserialized.windows[0].title,
        "Title with\ttab and\nnewline"
    );
    assert_eq!(deserialized.windows[0].geometry, (-10, -20, 100, 200));
    assert!(deserialized.windows[0].is_minimized);
}

#[test]
fn snapshot_deserialize_missing_header() {
    let result = deserialize_snapshot("TIMESTAMP:123\nWORKSPACE:0\n");
    assert!(matches!(
        result,
        Err(SessionError::DeserializationFailed(_))
    ));
}

#[test]
fn snapshot_deserialize_invalid_geometry() {
    let bad =
        "SESSION:test\nTIMESTAMP:0\nWORKSPACE:0\nFOCUSED:\nWINDOW:app\ttitle\t1,2,3\t0\t0\t0\n";
    let result = deserialize_snapshot(bad);
    assert!(matches!(
        result,
        Err(SessionError::DeserializationFailed(_))
    ));
}

#[test]
fn snapshot_roundtrip_multiple_windows() {
    let mut snap = SessionSnapshot::new("multi", 5000);
    for i in 0..10 {
        snap.windows.push(SessionWindow {
            app_id: format!("app-{}", i),
            title: format!("Window {}", i),
            geometry: (i * 100, i * 50, 640, 480),
            workspace: (i as u32) % 3,
            is_maximized: i % 2 == 0,
            is_minimized: i % 3 == 0,
        });
    }
    snap.active_workspace = 2;
    snap.focused_window = Some("app-5".to_string());

    let serialized = serialize_snapshot(&snap);
    let deserialized = deserialize_snapshot(&serialized).unwrap();
    assert_eq!(deserialized.windows.len(), 10);
    assert_eq!(deserialized.focused_window, Some("app-5".to_string()));
    assert_eq!(deserialized.active_workspace, 2);
    for i in 0..10 {
        assert_eq!(deserialized.windows[i].app_id, format!("app-{}", i));
    }
}

#[test]
fn session_store_save_and_load() {
    let mut store = SessionStore::new();
    let snap = SessionSnapshot::new("work", 1000);
    store.save_session(snap);
    let loaded = store.load_session("work").unwrap();
    assert_eq!(loaded.name, "work");
}

#[test]
fn session_store_load_not_found() {
    let store = SessionStore::new();
    let result = store.load_session("nonexistent");
    assert!(matches!(result, Err(SessionError::SessionNotFound(_))));
}

#[test]
fn session_store_delete() {
    let mut store = SessionStore::new();
    store.save_session(SessionSnapshot::new("old", 100));
    assert_eq!(store.session_count(), 1);
    let deleted = store.delete_session("old").unwrap();
    assert_eq!(deleted.name, "old");
    assert_eq!(store.session_count(), 0);
}

#[test]
fn session_store_delete_not_found() {
    let mut store = SessionStore::new();
    assert!(matches!(
        store.delete_session("x"),
        Err(SessionError::SessionNotFound(_))
    ));
}

#[test]
fn session_store_multiple_sessions() {
    let mut store = SessionStore::new();
    store.save_session(SessionSnapshot::new("default", 1000));
    store.save_session(SessionSnapshot::new("work", 2000));
    store.save_session(SessionSnapshot::new("gaming", 3000));
    assert_eq!(store.session_count(), 3);
    let names = store.session_names();
    assert_eq!(names, vec!["default", "gaming", "work"]); // sorted
}

#[test]
fn session_store_overwrite_session() {
    let mut store = SessionStore::new();
    store.save_session(SessionSnapshot::new("default", 1000));
    store.save_session(SessionSnapshot::new("default", 2000));
    assert_eq!(store.session_count(), 1);
    let snap = store.load_session("default").unwrap();
    assert_eq!(snap.timestamp_ms, 2000);
}

#[test]
fn session_store_valid_transitions() {
    let mut store = SessionStore::new();
    assert_eq!(store.state, SessionState::Starting);
    store.transition(SessionState::Running).unwrap();
    assert_eq!(store.state, SessionState::Running);
    store.transition(SessionState::Locking).unwrap();
    store.transition(SessionState::Locked).unwrap();
    store.transition(SessionState::Running).unwrap(); // unlock
    store.transition(SessionState::LoggingOut).unwrap();
    store.transition(SessionState::ShuttingDown).unwrap();
}

#[test]
fn session_store_invalid_transition() {
    let mut store = SessionStore::new();
    let result = store.transition(SessionState::Locked);
    assert!(matches!(
        result,
        Err(SessionError::InvalidStateTransition { .. })
    ));
}

#[test]
fn session_store_same_state_transition() {
    let mut store = SessionStore::new();
    store.transition(SessionState::Starting).unwrap(); // no-op
    assert_eq!(store.state, SessionState::Starting);
}

#[test]
fn session_error_display() {
    let e = SessionError::DeserializationFailed("bad data".into());
    assert!(e.to_string().contains("bad data"));
    let e2 = SessionError::SessionNotFound("missing".into());
    assert!(e2.to_string().contains("missing"));
    let e3 = SessionError::InvalidStateTransition {
        from: SessionState::Starting,
        to: SessionState::Locked,
    };
    assert!(e3.to_string().contains("starting"));
    assert!(e3.to_string().contains("locked"));
}

// ── Inhibitor tests ────────────────────────────────────────────────

#[test]
fn inhibit_flag_basics() {
    assert!(InhibitFlag::LOGOUT.contains(InhibitFlag::LOGOUT));
    assert!(!InhibitFlag::LOGOUT.contains(InhibitFlag::SUSPEND));
    let combined = InhibitFlag::LOGOUT.union(InhibitFlag::SUSPEND);
    assert!(combined.contains(InhibitFlag::LOGOUT));
    assert!(combined.contains(InhibitFlag::SUSPEND));
    assert!(!combined.contains(InhibitFlag::IDLE));
}

#[test]
fn inhibit_flag_intersects() {
    let a = InhibitFlag::LOGOUT.union(InhibitFlag::IDLE);
    assert!(a.intersects(InhibitFlag::LOGOUT));
    assert!(a.intersects(InhibitFlag::IDLE));
    assert!(!a.intersects(InhibitFlag::SUSPEND));
}

#[test]
fn inhibit_flag_display() {
    assert_eq!(InhibitFlag::NONE.to_string(), "none");
    assert_eq!(InhibitFlag::LOGOUT.to_string(), "logout");
    assert_eq!(
        InhibitFlag::LOGOUT.union(InhibitFlag::SUSPEND).to_string(),
        "logout|suspend"
    );
    assert_eq!(
        InhibitFlag::ALL.to_string(),
        "logout|switch-user|suspend|idle"
    );
}

#[test]
fn inhibit_flag_bits_roundtrip() {
    let flags = InhibitFlag::LOGOUT.union(InhibitFlag::IDLE);
    let bits = flags.bits();
    let restored = InhibitFlag::from_bits(bits);
    assert_eq!(restored, flags);
}

#[test]
fn inhibitor_add_remove() {
    let mut reg = InhibitorRegistry::new();
    let id = reg.add("firefox", "downloading file", InhibitFlag::LOGOUT);
    assert_eq!(reg.count(), 1);
    assert!(reg.remove(id));
    assert_eq!(reg.count(), 0);
}

#[test]
fn inhibitor_remove_nonexistent() {
    let mut reg = InhibitorRegistry::new();
    assert!(!reg.remove(999));
}

#[test]
fn inhibitor_is_inhibited() {
    let mut reg = InhibitorRegistry::new();
    assert!(!reg.is_inhibited(InhibitFlag::LOGOUT));

    reg.add("app", "reason", InhibitFlag::LOGOUT);
    assert!(reg.is_inhibited(InhibitFlag::LOGOUT));
    assert!(!reg.is_inhibited(InhibitFlag::SUSPEND));
}

#[test]
fn inhibitor_multiple_flags() {
    let mut reg = InhibitorRegistry::new();
    reg.add(
        "player",
        "playing video",
        InhibitFlag::SUSPEND.union(InhibitFlag::IDLE),
    );
    assert!(reg.is_inhibited(InhibitFlag::SUSPEND));
    assert!(reg.is_inhibited(InhibitFlag::IDLE));
    assert!(!reg.is_inhibited(InhibitFlag::LOGOUT));
}

#[test]
fn inhibitor_multiple_apps() {
    let mut reg = InhibitorRegistry::new();
    let id1 = reg.add("firefox", "downloading", InhibitFlag::LOGOUT);
    let _id2 = reg.add("vlc", "playing", InhibitFlag::SUSPEND);

    // Removing firefox's inhibitor should not affect vlc's
    reg.remove(id1);
    assert!(!reg.is_inhibited(InhibitFlag::LOGOUT));
    assert!(reg.is_inhibited(InhibitFlag::SUSPEND));
}

#[test]
fn inhibitor_active_inhibitors() {
    let mut reg = InhibitorRegistry::new();
    reg.add("a", "reason a", InhibitFlag::LOGOUT);
    reg.add("b", "reason b", InhibitFlag::SUSPEND);

    let all = reg.active_inhibitors();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].app_id, "a");
    assert_eq!(all[1].app_id, "b");
}

#[test]
fn inhibitor_inhibitors_for_flag() {
    let mut reg = InhibitorRegistry::new();
    reg.add("a", "r1", InhibitFlag::LOGOUT);
    reg.add("b", "r2", InhibitFlag::SUSPEND);
    reg.add("c", "r3", InhibitFlag::LOGOUT.union(InhibitFlag::SUSPEND));

    let logout_inhibitors = reg.inhibitors_for(InhibitFlag::LOGOUT);
    assert_eq!(logout_inhibitors.len(), 2); // a and c
    let suspend_inhibitors = reg.inhibitors_for(InhibitFlag::SUSPEND);
    assert_eq!(suspend_inhibitors.len(), 2); // b and c
}

#[test]
fn inhibitor_clear_expired() {
    let mut reg = InhibitorRegistry::new();
    reg.add_with_time("old", "stale", InhibitFlag::IDLE, 1000);
    reg.add_with_time("new", "fresh", InhibitFlag::IDLE, 9000);

    // At time 10000 with max_age 5000: old (age 9000) expires, new (age 1000) stays
    reg.clear_expired(10000, 5000);
    assert_eq!(reg.count(), 1);
    assert_eq!(reg.active_inhibitors()[0].app_id, "new");
}

#[test]
fn inhibitor_clear_expired_all_fresh() {
    let mut reg = InhibitorRegistry::new();
    reg.add_with_time("a", "r1", InhibitFlag::LOGOUT, 5000);
    reg.add_with_time("b", "r2", InhibitFlag::SUSPEND, 6000);
    reg.clear_expired(7000, 5000);
    assert_eq!(reg.count(), 2);
}

#[test]
fn inhibitor_clear_expired_all_stale() {
    let mut reg = InhibitorRegistry::new();
    reg.add_with_time("a", "r1", InhibitFlag::LOGOUT, 100);
    reg.add_with_time("b", "r2", InhibitFlag::SUSPEND, 200);
    reg.clear_expired(100_000, 1000);
    assert_eq!(reg.count(), 0);
}

#[test]
fn inhibitor_remove_all_for_app() {
    let mut reg = InhibitorRegistry::new();
    reg.add("firefox", "dl1", InhibitFlag::LOGOUT);
    reg.add("firefox", "dl2", InhibitFlag::SUSPEND);
    reg.add("vlc", "playing", InhibitFlag::IDLE);
    assert_eq!(reg.count(), 3);
    reg.remove_all_for_app("firefox");
    assert_eq!(reg.count(), 1);
    assert_eq!(reg.active_inhibitors()[0].app_id, "vlc");
}

#[test]
fn inhibitor_ids_are_unique() {
    let mut reg = InhibitorRegistry::new();
    let id1 = reg.add("a", "r1", InhibitFlag::LOGOUT);
    let id2 = reg.add("b", "r2", InhibitFlag::LOGOUT);
    let id3 = reg.add("c", "r3", InhibitFlag::LOGOUT);
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);
}

// ── Shutdown tests ─────────────────────────────────────────────────

#[test]
fn shutdown_initially_idle() {
    let sm = ShutdownManager::new(5000);
    assert!(sm.is_idle());
    assert!(!sm.is_complete());
    assert_eq!(sm.phase, ShutdownPhase::Idle);
}

#[test]
fn shutdown_begin_enters_confirmation() {
    let mut sm = ShutdownManager::new(5000);
    let phase = sm.begin_shutdown();
    assert_eq!(phase, ShutdownPhase::RequestingConfirmation);
    assert!(!sm.is_idle());
    assert!(!sm.is_complete());
}

#[test]
fn shutdown_cancel_during_confirmation() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.cancel();
    assert!(sm.is_idle());
    assert_eq!(sm.phase, ShutdownPhase::Idle);
}

#[test]
fn shutdown_cancel_after_confirmation_is_noop() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.confirm();
    sm.mark_session_saved();
    sm.tick(0.0);
    // Now in ClosingApps — cancel should be a no-op
    sm.cancel();
    assert_ne!(sm.phase, ShutdownPhase::Idle);
}

#[test]
fn shutdown_normal_flow() {
    let mut sm = ShutdownManager::new(10000);
    sm.begin_shutdown();
    sm.set_pending_apps(vec!["firefox".into(), "terminal".into()]);

    // Confirm
    sm.confirm();
    assert_eq!(sm.phase, ShutdownPhase::SavingSession);

    // Mark session saved
    sm.mark_session_saved();
    sm.tick(0.0);
    assert_eq!(sm.phase, ShutdownPhase::ClosingApps);

    // Apps close
    sm.app_closed("firefox");
    assert_eq!(sm.pending_count(), 1);
    sm.app_closed("terminal");
    sm.tick(0.0);
    assert!(sm.is_complete());
}

#[test]
fn shutdown_force_timeout() {
    let mut sm = ShutdownManager::new(1000); // 1s timeout
    sm.begin_shutdown();
    sm.set_pending_apps(vec!["stuck-app".into()]);
    sm.confirm();
    sm.mark_session_saved();
    sm.tick(0.0); // move to ClosingApps

    assert_eq!(sm.phase, ShutdownPhase::ClosingApps);

    // Tick past the timeout
    sm.tick(1001.0);
    assert_eq!(sm.phase, ShutdownPhase::ForceClosing);

    // One more tick to complete
    sm.tick(1.0);
    assert!(sm.is_complete());
    assert_eq!(sm.pending_count(), 0);
}

#[test]
fn shutdown_force_close_remaining() {
    let mut sm = ShutdownManager::new(10000);
    sm.begin_shutdown();
    sm.set_pending_apps(vec!["app1".into(), "app2".into()]);
    sm.confirm();
    sm.mark_session_saved();
    sm.tick(0.0); // move to ClosingApps

    sm.force_close_remaining();
    assert!(sm.is_complete());
    assert_eq!(sm.pending_count(), 0);
}

#[test]
fn shutdown_logout_sets_kind() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_logout();
    assert_eq!(sm.kind, ShutdownKind::Logout);
    assert_eq!(sm.phase, ShutdownPhase::RequestingConfirmation);
}

#[test]
fn shutdown_reboot_sets_kind() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_reboot();
    assert_eq!(sm.kind, ShutdownKind::Reboot);
}

#[test]
fn shutdown_reason_display() {
    assert_eq!(ShutdownReason::UserRequested.to_string(), "user-requested");
    assert_eq!(ShutdownReason::SystemUpdate.to_string(), "system-update");
    assert_eq!(ShutdownReason::TimerExpired.to_string(), "timer-expired");
    assert_eq!(ShutdownReason::PowerFailure.to_string(), "power-failure");
}

#[test]
fn shutdown_phase_display() {
    assert_eq!(ShutdownPhase::Idle.to_string(), "idle");
    assert_eq!(
        ShutdownPhase::RequestingConfirmation.to_string(),
        "requesting-confirmation"
    );
    assert_eq!(ShutdownPhase::SavingSession.to_string(), "saving-session");
    assert_eq!(ShutdownPhase::ClosingApps.to_string(), "closing-apps");
    assert_eq!(ShutdownPhase::ForceClosing.to_string(), "force-closing");
    assert_eq!(ShutdownPhase::Complete.to_string(), "complete");
}

#[test]
fn shutdown_with_reason() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_with_reason(ShutdownKind::PowerOff, ShutdownReason::PowerFailure);
    assert_eq!(sm.reason, ShutdownReason::PowerFailure);
    assert_eq!(sm.kind, ShutdownKind::PowerOff);
}

#[test]
fn shutdown_skip_confirmation() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.skip_confirmation();
    assert_eq!(sm.phase, ShutdownPhase::SavingSession);
}

#[test]
fn shutdown_tick_idle_is_noop() {
    let mut sm = ShutdownManager::new(5000);
    let phase = sm.tick(100.0);
    assert_eq!(phase, ShutdownPhase::Idle);
}

#[test]
fn shutdown_tick_complete_is_noop() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.confirm();
    sm.mark_session_saved();
    sm.tick(0.0);
    sm.force_close_remaining();
    assert!(sm.is_complete());
    let phase = sm.tick(100.0);
    assert_eq!(phase, ShutdownPhase::Complete);
}

#[test]
fn shutdown_pending_apps_tracking() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.set_pending_apps(vec!["a".into(), "b".into(), "c".into()]);
    assert_eq!(sm.pending_count(), 3);
    assert_eq!(sm.pending_apps(), &["a", "b", "c"]);

    sm.app_closed("b");
    assert_eq!(sm.pending_count(), 2);
    assert_eq!(sm.pending_apps(), &["a", "c"]);
}

#[test]
fn shutdown_default_timeout() {
    let sm = ShutdownManager::default();
    assert!(sm.is_idle());
    // Default is 10 seconds
}

#[test]
fn shutdown_phase_elapsed() {
    let mut sm = ShutdownManager::new(5000);
    sm.begin_shutdown();
    sm.confirm();
    sm.mark_session_saved();
    sm.tick(0.0); // move to ClosingApps
    sm.set_pending_apps(vec!["app".into()]);
    sm.tick(250.0);
    assert!(sm.phase_elapsed_ms() >= 250.0);
}
