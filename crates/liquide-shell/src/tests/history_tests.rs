use crate::history::*;
use crate::shell::Shell;
use crate::window::*;
use liquide_compositor::geometry::Rect;

// ========== WindowHistory unit tests ==========

#[test]
fn history_new_empty() {
    let h = WindowHistory::new(1000);
    assert_eq!(h.len(), 0);
    assert!(h.is_empty());
    assert_eq!(h.capacity(), 1000);
}

#[test]
fn history_record_single_event() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    assert_eq!(h.len(), 1);
    assert!(!h.is_empty());
}

#[test]
fn history_record_returns_incrementing_timestamps() {
    let mut h = WindowHistory::new(100);
    let ts1 = h.record(WindowId(1), WindowEventKind::Opened);
    let ts2 = h.record(WindowId(2), WindowEventKind::Opened);
    let ts3 = h.record(WindowId(3), WindowEventKind::Opened);
    assert!(ts1 < ts2);
    assert!(ts2 < ts3);
}

#[test]
fn history_record_at_explicit_timestamp() {
    let mut h = WindowHistory::new(100);
    h.record_at(WindowId(1), WindowEventKind::Opened, 42);
    h.record_at(WindowId(2), WindowEventKind::Opened, 100);
    let events = h.recent(2);
    assert_eq!(events[0].timestamp_us, 42);
    assert_eq!(events[1].timestamp_us, 100);
}

#[test]
fn history_events_for_window() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(WindowId(2), WindowEventKind::Opened);
    h.record(WindowId(1), WindowEventKind::Focused);
    h.record(WindowId(3), WindowEventKind::Opened);
    h.record(WindowId(1), WindowEventKind::Closed);

    let events = h.events_for_window(WindowId(1));
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0].kind, WindowEventKind::Opened));
    assert!(matches!(events[1].kind, WindowEventKind::Focused));
    assert!(matches!(events[2].kind, WindowEventKind::Closed));
}

#[test]
fn history_events_for_window_nonexistent() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    let events = h.events_for_window(WindowId(999));
    assert!(events.is_empty());
}

#[test]
fn history_recent_n() {
    let mut h = WindowHistory::new(100);
    for i in 0..10 {
        h.record(WindowId(i), WindowEventKind::Opened);
    }
    let recent = h.recent(3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].window_id, WindowId(7));
    assert_eq!(recent[1].window_id, WindowId(8));
    assert_eq!(recent[2].window_id, WindowId(9));
}

#[test]
fn history_recent_more_than_stored() {
    let mut h = WindowHistory::new(100);
    for i in 0..5 {
        h.record(WindowId(i), WindowEventKind::Opened);
    }
    let recent = h.recent(100);
    assert_eq!(recent.len(), 5);
}

#[test]
fn history_events_by_kind_opened() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(WindowId(1), WindowEventKind::Focused);
    h.record(WindowId(2), WindowEventKind::Opened);
    h.record(WindowId(1), WindowEventKind::Closed);

    let opened = h.events_by_kind(&|k| matches!(k, WindowEventKind::Opened));
    assert_eq!(opened.len(), 2);
}

#[test]
fn history_events_by_kind_state_changed() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
    );
    h.record(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Maximized,
            to: WindowState::Normal,
        },
    );

    let state_changes = h.events_by_kind(&|k| matches!(k, WindowEventKind::StateChanged { .. }));
    assert_eq!(state_changes.len(), 2);
}

#[test]
fn history_events_in_range() {
    let mut h = WindowHistory::new(100);
    h.record_at(WindowId(1), WindowEventKind::Opened, 10);
    h.record_at(WindowId(2), WindowEventKind::Opened, 20);
    h.record_at(WindowId(3), WindowEventKind::Opened, 30);
    h.record_at(WindowId(4), WindowEventKind::Opened, 40);
    h.record_at(WindowId(5), WindowEventKind::Opened, 50);

    let in_range = h.events_in_range(20, 40);
    assert_eq!(in_range.len(), 3);
    assert_eq!(in_range[0].timestamp_us, 20);
    assert_eq!(in_range[2].timestamp_us, 40);
}

#[test]
fn history_events_in_range_empty() {
    let mut h = WindowHistory::new(100);
    h.record_at(WindowId(1), WindowEventKind::Opened, 10);
    h.record_at(WindowId(2), WindowEventKind::Opened, 50);

    let in_range = h.events_in_range(20, 40);
    assert!(in_range.is_empty());
}

#[test]
fn history_ring_buffer_overflow() {
    let mut h = WindowHistory::new(5);
    for i in 0..8 {
        h.record(WindowId(i), WindowEventKind::Opened);
    }
    assert_eq!(h.len(), 5);
    // Oldest 3 should be gone (WindowId 0, 1, 2)
    let events = h.events_for_window(WindowId(0));
    assert!(events.is_empty());
    let events = h.events_for_window(WindowId(3));
    assert_eq!(events.len(), 1);
}

#[test]
fn history_ring_buffer_preserves_newest() {
    let mut h = WindowHistory::new(5);
    for i in 0..8 {
        h.record(WindowId(i), WindowEventKind::Opened);
    }
    let recent = h.recent(5);
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0].window_id, WindowId(3));
    assert_eq!(recent[4].window_id, WindowId(7));
}

#[test]
fn history_capacity_zero() {
    let mut h = WindowHistory::new(0);
    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(WindowId(2), WindowEventKind::Closed);
    assert_eq!(h.len(), 0);
    assert!(h.is_empty());
    assert_eq!(h.capacity(), 0);
}

#[test]
fn history_clear() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(WindowId(2), WindowEventKind::Opened);
    assert_eq!(h.len(), 2);
    h.clear();
    assert_eq!(h.len(), 0);
    assert!(h.is_empty());
}

#[test]
fn history_all_event_kinds_recorded() {
    let mut h = WindowHistory::new(100);
    let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
    let r2 = Rect::new(10.0, 20.0, 200.0, 150.0);
    let f1 = WindowFlags::default();
    let f2 = WindowFlags::from_bits(0);

    h.record(WindowId(1), WindowEventKind::Opened);
    h.record(WindowId(1), WindowEventKind::Closed);
    h.record(WindowId(1), WindowEventKind::Moved { from: r1, to: r2 });
    h.record(WindowId(1), WindowEventKind::Resized { from: r1, to: r2 });
    h.record(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
    );
    h.record(WindowId(1), WindowEventKind::Focused);
    h.record(WindowId(1), WindowEventKind::Unfocused);
    h.record(
        WindowId(1),
        WindowEventKind::TitleChanged {
            from: "Old".to_string(),
            to: "New".to_string(),
        },
    );
    h.record(
        WindowId(1),
        WindowEventKind::ZOrderChanged { from: 0, to: 5 },
    );
    h.record(
        WindowId(1),
        WindowEventKind::VisibilityChanged {
            from: true,
            to: false,
        },
    );
    h.record(
        WindowId(1),
        WindowEventKind::FlagsChanged { from: f1, to: f2 },
    );

    assert_eq!(h.len(), 11);
}

#[test]
fn history_moved_event_captures_bounds() {
    let mut h = WindowHistory::new(100);
    let from = Rect::new(10.0, 20.0, 300.0, 200.0);
    let to = Rect::new(50.0, 60.0, 300.0, 200.0);
    h.record(WindowId(1), WindowEventKind::Moved { from, to });

    let events = h.events_for_window(WindowId(1));
    assert_eq!(events.len(), 1);
    if let WindowEventKind::Moved { from: f, to: t } = &events[0].kind {
        assert_eq!(f.x, 10.0);
        assert_eq!(t.x, 50.0);
    } else {
        panic!("expected Moved event");
    }
}

#[test]
fn history_state_changed_captures_states() {
    let mut h = WindowHistory::new(100);
    h.record(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Fullscreen,
        },
    );

    let events = h.events_for_window(WindowId(1));
    if let WindowEventKind::StateChanged { from, to } = &events[0].kind {
        assert_eq!(*from, WindowState::Normal);
        assert_eq!(*to, WindowState::Fullscreen);
    } else {
        panic!("expected StateChanged event");
    }
}

#[test]
fn history_serde_roundtrip_window_event() {
    let event = WindowEvent {
        window_id: WindowId(42),
        timestamp_us: 1000,
        kind: WindowEventKind::Opened,
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: WindowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, back);
}

#[test]
fn history_serde_roundtrip_all_kinds() {
    let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
    let r2 = Rect::new(50.0, 50.0, 200.0, 200.0);
    let kinds = vec![
        WindowEventKind::Opened,
        WindowEventKind::Closed,
        WindowEventKind::Moved { from: r1, to: r2 },
        WindowEventKind::Resized { from: r1, to: r2 },
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
        WindowEventKind::Focused,
        WindowEventKind::Unfocused,
        WindowEventKind::TitleChanged {
            from: "A".to_string(),
            to: "B".to_string(),
        },
        WindowEventKind::ZOrderChanged { from: 0, to: 10 },
        WindowEventKind::VisibilityChanged {
            from: true,
            to: false,
        },
        WindowEventKind::FlagsChanged {
            from: WindowFlags::default(),
            to: WindowFlags::from_bits(0),
        },
    ];

    for kind in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        let back: WindowEventKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, back);
    }
}

#[test]
fn history_display_window_event_kind() {
    assert_eq!(format!("{}", WindowEventKind::Opened), "Opened");
    assert_eq!(format!("{}", WindowEventKind::Closed), "Closed");
    assert_eq!(format!("{}", WindowEventKind::Focused), "Focused");
    assert_eq!(format!("{}", WindowEventKind::Unfocused), "Unfocused");
    let s = format!(
        "{}",
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        }
    );
    assert!(s.contains("Normal"));
    assert!(s.contains("Maximized"));
}

#[test]
fn history_display_window_event() {
    let event = WindowEvent {
        window_id: WindowId(5),
        timestamp_us: 999,
        kind: WindowEventKind::Opened,
    };
    let s = format!("{event}");
    assert!(s.contains("999us"));
    assert!(s.contains("Window(5)"));
    assert!(s.contains("Opened"));
}

#[test]
fn history_display_window_history() {
    let mut h = WindowHistory::new(100);
    h.record(WindowId(1), WindowEventKind::Opened);
    let s = format!("{h}");
    assert!(s.contains("1/100"));
}

#[test]
fn history_reconstruct_window_lifecycle() {
    let mut h = WindowHistory::new(100);
    let id = WindowId(1);
    let r1 = Rect::new(0.0, 0.0, 400.0, 300.0);
    let r2 = Rect::new(50.0, 50.0, 400.0, 300.0);
    let r3 = Rect::new(50.0, 50.0, 600.0, 500.0);
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);

    h.record(id, WindowEventKind::Opened);
    h.record(id, WindowEventKind::Moved { from: r1, to: r2 });
    h.record(id, WindowEventKind::Resized { from: r2, to: r3 });
    h.record(
        id,
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
    );
    h.record(
        id,
        WindowEventKind::Resized {
            from: r3,
            to: screen,
        },
    );
    h.record(
        id,
        WindowEventKind::StateChanged {
            from: WindowState::Maximized,
            to: WindowState::Normal,
        },
    );
    h.record(id, WindowEventKind::Closed);

    let lifecycle = h.events_for_window(id);
    assert_eq!(lifecycle.len(), 7);
    assert!(matches!(lifecycle[0].kind, WindowEventKind::Opened));
    assert!(matches!(lifecycle[1].kind, WindowEventKind::Moved { .. }));
    assert!(matches!(lifecycle[2].kind, WindowEventKind::Resized { .. }));
    assert!(matches!(
        lifecycle[3].kind,
        WindowEventKind::StateChanged { .. }
    ));
    assert!(matches!(lifecycle[4].kind, WindowEventKind::Resized { .. }));
    assert!(matches!(
        lifecycle[5].kind,
        WindowEventKind::StateChanged { .. }
    ));
    assert!(matches!(lifecycle[6].kind, WindowEventKind::Closed));
}

// ========== Shell integration tests ==========

#[test]
fn shell_open_records_event() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 400.0, 300.0));
    let events = shell.window_history().events_for_window(id);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, WindowEventKind::Opened));
}

#[test]
fn shell_close_records_event() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    shell.close_window(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    assert_eq!(events.len(), 2); // Opened + Closed
    assert!(matches!(events[1].kind, WindowEventKind::Closed));
}

#[test]
fn shell_move_records_event() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));
    shell.move_window(id, 50.0, 75.0).unwrap();
    let events = shell.window_history().events_for_window(id);
    assert_eq!(events.len(), 2); // Opened + Moved
    if let WindowEventKind::Moved { from, to } = &events[1].kind {
        assert_eq!(from.x, 0.0);
        assert_eq!(to.x, 50.0);
        assert_eq!(to.y, 75.0);
    } else {
        panic!("expected Moved event");
    }
}

#[test]
fn shell_resize_records_event() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));
    shell.resize_window(id, 500.0, 400.0).unwrap();
    let events = shell.window_history().events_for_window(id);
    assert_eq!(events.len(), 2); // Opened + Resized
    if let WindowEventKind::Resized { from, to } = &events[1].kind {
        assert_eq!(from.width, 100.0);
        assert_eq!(to.width, 500.0);
        assert_eq!(to.height, 400.0);
    } else {
        panic!("expected Resized event");
    }
}

#[test]
fn shell_minimize_records_state_and_visibility() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 400.0, 300.0));
    shell.minimize(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    // Opened + StateChanged + VisibilityChanged
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[1].kind,
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Minimized,
        }
    ));
    assert!(matches!(
        events[2].kind,
        WindowEventKind::VisibilityChanged {
            from: true,
            to: false,
        }
    ));
}

#[test]
fn shell_maximize_records_state_and_resize() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    // Opened + StateChanged + Resized
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[1].kind,
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        }
    ));
    if let WindowEventKind::Resized { from, to } = &events[2].kind {
        assert_eq!(from.width, 400.0);
        assert_eq!(to.width, 1920.0);
    } else {
        panic!("expected Resized event");
    }
}

#[test]
fn shell_restore_records_events() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.minimize(id).unwrap();
    shell.restore(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    // Opened + minimize(StateChanged + VisibilityChanged) + restore(StateChanged + VisibilityChanged + Resized)
    // restore from minimized: state changes, visibility true, bounds restored
    let state_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, WindowEventKind::StateChanged { .. }))
        .collect();
    assert_eq!(state_events.len(), 2); // minimize + restore
}

#[test]
fn shell_toggle_fullscreen_records_events() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.toggle_fullscreen(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    // Opened + StateChanged + Resized
    let state_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, WindowEventKind::StateChanged { .. }))
        .collect();
    assert_eq!(state_events.len(), 1);
    assert!(matches!(
        state_events[0].kind,
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Fullscreen,
        }
    ));
}

#[test]
fn shell_set_focus_records_focused_unfocused() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.set_focus(id1).unwrap();
    shell.set_focus(id2).unwrap(); // id1 gets Unfocused, id2 gets Focused

    let events1 = shell.window_history().events_for_window(id1);
    let focus_events1: Vec<_> = events1
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                WindowEventKind::Focused | WindowEventKind::Unfocused
            )
        })
        .collect();
    assert_eq!(focus_events1.len(), 2); // Focused + Unfocused
    assert!(matches!(focus_events1[0].kind, WindowEventKind::Focused));
    assert!(matches!(focus_events1[1].kind, WindowEventKind::Unfocused));

    let events2 = shell.window_history().events_for_window(id2);
    let focus_events2: Vec<_> = events2
        .iter()
        .filter(|e| matches!(e.kind, WindowEventKind::Focused))
        .collect();
    assert_eq!(focus_events2.len(), 1);
}

#[test]
fn shell_raise_records_z_order_change() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    shell.raise_window(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    let z_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, WindowEventKind::ZOrderChanged { .. }))
        .collect();
    assert_eq!(z_events.len(), 1);
}

#[test]
fn shell_lower_records_z_order_change() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    shell.lower_window(id).unwrap();
    let events = shell.window_history().events_for_window(id);
    let z_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, WindowEventKind::ZOrderChanged { .. }))
        .collect();
    assert_eq!(z_events.len(), 1);
}

#[test]
fn shell_open_with_app_records_app_history() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let bounds = Rect::new(100.0, 100.0, 400.0, 300.0);
    let id = shell.open_window_with_app("Test", bounds, "com.example.app");
    assert_eq!(shell.window(id).unwrap().app_id, "com.example.app");

    let app = shell.app_history().app_info("com.example.app").unwrap();
    assert_eq!(app.total_windows_opened, 1);
    assert_eq!(app.active_window_count, 1);
}

#[test]
fn shell_close_updates_app_history() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let bounds = Rect::new(100.0, 100.0, 400.0, 300.0);
    let id = shell.open_window_with_app("Test", bounds, "com.example.app");
    shell.close_window(id).unwrap();

    let app = shell.app_history().app_info("com.example.app").unwrap();
    assert_eq!(app.active_window_count, 0);
    assert_eq!(app.sessions.len(), 1);
    assert!(app.sessions[0].closed_at.is_some());
    assert_eq!(app.last_bounds, Some(bounds));
}

#[test]
fn shell_window_history_accessor() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.window_history().capacity(), 1000);
    assert!(shell.window_history().is_empty());
}

#[test]
fn shell_app_history_accessor() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.app_history().max_tracked(), 100);
    assert_eq!(shell.app_history().tracked_count(), 0);
}

#[test]
fn shell_custom_history_capacity() {
    let shell = Shell::with_history_capacity(1920.0, 1080.0, 500, 50);
    assert_eq!(shell.window_history().capacity(), 500);
    assert_eq!(shell.app_history().max_tracked(), 50);
}

#[test]
fn shell_failed_operations_no_events() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // These should all fail without recording events
    assert!(shell.move_window(WindowId(999), 0.0, 0.0).is_err());
    assert!(shell.resize_window(WindowId(999), 0.0, 0.0).is_err());
    assert!(shell.minimize(WindowId(999)).is_err());
    assert!(shell.maximize(WindowId(999)).is_err());
    assert!(shell.restore(WindowId(999)).is_err());
    assert!(shell.toggle_fullscreen(WindowId(999)).is_err());
    assert!(shell.set_focus(WindowId(999)).is_err());
    assert!(shell.raise_window(WindowId(999)).is_err());
    assert!(shell.lower_window(WindowId(999)).is_err());
    assert!(shell.close_window(WindowId(999)).is_err());

    assert!(shell.window_history().is_empty());
}
