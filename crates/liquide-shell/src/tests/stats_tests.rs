use crate::app_history::AppHistory;
use crate::history::*;
use crate::shell::Shell;
use crate::stats::*;
use crate::window::*;
use liquide_compositor::geometry::Rect;

/// Helper: create a StatsCollector with a pre-populated window history.
fn make_collector<'a>(wh: &'a WindowHistory, ah: &'a AppHistory) -> StatsCollector<'a> {
    StatsCollector::new(wh, ah)
}

// ========== WindowStats unit tests ==========

#[test]
fn window_stats_empty_history() {
    let wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(999));
    assert_eq!(s.total_event_count, 0);
    assert_eq!(s.opened_at, None);
    assert_eq!(s.closed_at, None);
    assert_eq!(s.runtime_us, None);
    assert_eq!(s.focus_time_us, 0);
    assert_eq!(s.focus_count, 0);
    assert_eq!(s.move_count, 0);
    assert_eq!(s.resize_count, 0);
    assert_eq!(s.state_change_count, 0);
    assert_eq!(s.total_distance_moved, 0.0);
    assert!(s.last_state.is_none());
    assert!(s.time_in_state.is_empty());
}

#[test]
fn window_stats_opened_only() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 10);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.opened_at, Some(10));
    assert_eq!(s.closed_at, None);
    assert_eq!(s.runtime_us, None);
    assert_eq!(s.total_event_count, 1);
    assert_eq!(s.last_state, Some(WindowState::Normal));
}

#[test]
fn window_stats_full_lifecycle() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 350);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.opened_at, Some(100));
    assert_eq!(s.closed_at, Some(350));
    assert_eq!(s.runtime_us, Some(250));
}

#[test]
fn window_stats_focus_duration_single() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 30);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.focus_time_us, 20);
    assert_eq!(s.focus_count, 1);
}

#[test]
fn window_stats_focus_duration_multiple() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 30);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 50);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 80);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 100);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 105);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // 20 + 30 + 5 = 55
    assert_eq!(s.focus_time_us, 55);
    assert_eq!(s.focus_count, 3);
}

#[test]
fn window_stats_focus_closed_without_unfocus() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10);
    // No Unfocused — closed while focused
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // Focus truncated at close: 50 - 10 = 40
    assert_eq!(s.focus_time_us, 40);
    assert_eq!(s.focus_count, 1);
}

#[test]
fn window_stats_move_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    for i in 1..=5 {
        let to = Rect::new(i as f32 * 10.0, 0.0, 100.0, 100.0);
        wh.record_at(WindowId(1), WindowEventKind::Moved { from: r, to }, i);
    }
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.move_count, 5);
}

#[test]
fn window_stats_resize_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let r1 = Rect::new(0.0, 0.0, 100.0, 100.0);
    let r2 = Rect::new(0.0, 0.0, 200.0, 200.0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::Resized { from: r1, to: r2 },
        1,
    );
    wh.record_at(
        WindowId(1),
        WindowEventKind::Resized { from: r2, to: r1 },
        2,
    );
    wh.record_at(
        WindowId(1),
        WindowEventKind::Resized { from: r1, to: r2 },
        3,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.resize_count, 3);
}

#[test]
fn window_stats_state_change_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
        10,
    );
    wh.record_at(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Maximized,
            to: WindowState::Normal,
        },
        20,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.state_change_count, 2);
}

#[test]
fn window_stats_title_change_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::TitleChanged {
            from: "A".to_string(),
            to: "B".to_string(),
        },
        1,
    );
    wh.record_at(
        WindowId(1),
        WindowEventKind::TitleChanged {
            from: "B".to_string(),
            to: "C".to_string(),
        },
        2,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.title_change_count, 2);
}

#[test]
fn window_stats_z_order_change_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::ZOrderChanged { from: 0, to: 5 },
        1,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.z_order_change_count, 1);
}

#[test]
fn window_stats_visibility_change_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::VisibilityChanged {
            from: true,
            to: false,
        },
        1,
    );
    wh.record_at(
        WindowId(1),
        WindowEventKind::VisibilityChanged {
            from: false,
            to: true,
        },
        2,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.visibility_change_count, 2);
}

#[test]
fn window_stats_flags_change_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let f1 = WindowFlags::default();
    let f2 = WindowFlags::from_bits(0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::FlagsChanged { from: f1, to: f2 },
        1,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.flags_change_count, 1);
}

#[test]
fn window_stats_total_distance_moved() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    // Move 1: (0,0) -> (3,4) = distance 5
    wh.record_at(
        WindowId(1),
        WindowEventKind::Moved {
            from: Rect::new(0.0, 0.0, 100.0, 100.0),
            to: Rect::new(3.0, 4.0, 100.0, 100.0),
        },
        1,
    );
    // Move 2: (3,4) -> (3,4+12)=(3,16), dx=0 dy=12, distance=12
    wh.record_at(
        WindowId(1),
        WindowEventKind::Moved {
            from: Rect::new(3.0, 4.0, 100.0, 100.0),
            to: Rect::new(3.0, 16.0, 100.0, 100.0),
        },
        2,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // 5.0 + 12.0 = 17.0
    assert!((s.total_distance_moved - 17.0).abs() < f64::EPSILON);
}

#[test]
fn window_stats_distance_moved_no_moves() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.total_distance_moved, 0.0);
}

#[test]
fn window_stats_time_in_state() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Opens at 0 in Normal
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    // State -> Maximized at 10 (Normal for 10us)
    wh.record_at(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Maximized,
        },
        10,
    );
    // State -> Normal at 15 (Maximized for 5us)
    wh.record_at(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Maximized,
            to: WindowState::Normal,
        },
        15,
    );
    // Close at 23 (Normal for 8us)
    wh.record_at(WindowId(1), WindowEventKind::Closed, 23);

    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // Normal: 10 (initial) + 8 (after restore) = 18
    assert_eq!(*s.time_in_state.get("Normal").unwrap_or(&0), 18);
    // Maximized: 5
    assert_eq!(*s.time_in_state.get("Maximized").unwrap_or(&0), 5);
}

#[test]
fn window_stats_last_state() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(
        WindowId(1),
        WindowEventKind::StateChanged {
            from: WindowState::Normal,
            to: WindowState::Fullscreen,
        },
        10,
    );
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.last_state, Some(WindowState::Fullscreen));
}

#[test]
fn window_stats_total_event_count() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 1);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 5);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 10);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.total_event_count, 4);
}

// ========== AppStats unit tests ==========

#[test]
fn app_stats_nonexistent() {
    let wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let c = make_collector(&wh, &ah);
    assert!(c.app_stats("nonexistent").is_none());
}

#[test]
fn app_stats_single_session_closed() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 400.0, 300.0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 10);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 110);
    ah.record_open("app", WindowId(1), b, 10);
    ah.record_close("app", WindowId(1), b, 110);
    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_runtime_us, 100);
    assert_eq!(s.avg_session_duration_us, 100);
    assert_eq!(s.min_session_duration_us, Some(100));
    assert_eq!(s.max_session_duration_us, Some(100));
    assert_eq!(s.total_sessions, 1);
    assert_eq!(s.closed_sessions, 1);
    assert_eq!(s.active_sessions, 0);
}

#[test]
fn app_stats_single_session_still_open() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 400.0, 300.0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 10);
    ah.record_open("app", WindowId(1), b, 10);
    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_runtime_us, 0);
    assert_eq!(s.closed_sessions, 0);
    assert_eq!(s.active_sessions, 1);
    assert_eq!(s.min_session_duration_us, None);
    assert_eq!(s.max_session_duration_us, None);
}

#[test]
fn app_stats_multiple_sessions_avg_runtime() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Session 1: 100us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    ah.record_open("app", WindowId(1), b, 0);
    ah.record_close("app", WindowId(1), b, 100);

    // Session 2: 200us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 200);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 400);
    ah.record_open("app", WindowId(2), b, 200);
    ah.record_close("app", WindowId(2), b, 400);

    // Session 3: 300us
    wh.record_at(WindowId(3), WindowEventKind::Opened, 500);
    wh.record_at(WindowId(3), WindowEventKind::Closed, 800);
    ah.record_open("app", WindowId(3), b, 500);
    ah.record_close("app", WindowId(3), b, 800);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_runtime_us, 600); // 100 + 200 + 300
    assert_eq!(s.avg_session_duration_us, 200); // 600 / 3
    assert_eq!(s.closed_sessions, 3);
}

#[test]
fn app_stats_min_max_session_duration() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Short session: 50us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    ah.record_open("app", WindowId(1), b, 0);
    ah.record_close("app", WindowId(1), b, 50);

    // Long session: 500us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 600);
    ah.record_open("app", WindowId(2), b, 100);
    ah.record_close("app", WindowId(2), b, 600);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.min_session_duration_us, Some(50));
    assert_eq!(s.max_session_duration_us, Some(500));
}

#[test]
fn app_stats_total_focus_across_windows() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Window 1: 20us focus
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 5);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 25);
    ah.record_open("app", WindowId(1), b, 0);

    // Window 2: 30us focus
    wh.record_at(WindowId(2), WindowEventKind::Opened, 10);
    wh.record_at(WindowId(2), WindowEventKind::Focused, 30);
    wh.record_at(WindowId(2), WindowEventKind::Unfocused, 60);
    ah.record_open("app", WindowId(2), b, 10);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_focus_time_us, 50); // 20 + 30
}

#[test]
fn app_stats_total_moves_resizes() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b2 = Rect::new(10.0, 10.0, 200.0, 200.0);

    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Moved { from: b, to: b2 }, 1);
    wh.record_at(WindowId(1), WindowEventKind::Moved { from: b2, to: b }, 2);
    wh.record_at(WindowId(1), WindowEventKind::Resized { from: b, to: b2 }, 3);
    ah.record_open("app", WindowId(1), b, 0);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_move_count, 2);
    assert_eq!(s.total_resize_count, 1);
}

#[test]
fn app_stats_first_last_seen() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);
    ah.record_open("app", WindowId(1), b, 10);
    ah.record_close("app", WindowId(1), b, 50);
    ah.record_open("app", WindowId(2), b, 100);

    wh.record_at(WindowId(1), WindowEventKind::Opened, 10);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 100);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.first_seen, 10);
    assert_eq!(s.last_seen, 100);
}

#[test]
fn app_stats_active_vs_closed_counts() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // 2 closed
    ah.record_open("app", WindowId(1), b, 1);
    ah.record_close("app", WindowId(1), b, 2);
    ah.record_open("app", WindowId(2), b, 3);
    ah.record_close("app", WindowId(2), b, 4);
    // 1 still open
    ah.record_open("app", WindowId(3), b, 5);

    wh.record_at(WindowId(1), WindowEventKind::Opened, 1);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 2);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 3);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 4);
    wh.record_at(WindowId(3), WindowEventKind::Opened, 5);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_sessions, 3);
    assert_eq!(s.closed_sessions, 2);
    assert_eq!(s.active_sessions, 1);
}

#[test]
fn app_stats_total_event_count() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 1);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 2);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 3);
    ah.record_open("app", WindowId(1), b, 0);
    ah.record_close("app", WindowId(1), b, 3);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    assert_eq!(s.total_event_count, 4);
}

// ========== SystemStats unit tests ==========

#[test]
fn system_stats_empty() {
    let wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.total_windows_opened, 0);
    assert_eq!(s.total_windows_closed, 0);
    assert_eq!(s.currently_open, 0);
    assert_eq!(s.total_events, 0);
    assert_eq!(s.total_runtime_us, 0);
    assert_eq!(s.total_focus_switches, 0);
    assert_eq!(s.total_moves, 0);
    assert_eq!(s.total_resizes, 0);
    assert_eq!(s.unique_apps, 0);
    assert!(s.most_focused_window.is_none());
    assert!(s.most_active_app.is_none());
    assert!(s.longest_session.is_none());
    assert!(s.shortest_session.is_none());
    assert!(s.timestamp_range.is_none());
    assert_eq!(s.events_per_window_avg, 0.0);
}

#[test]
fn system_stats_basic_counts() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 1);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 2);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 10);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.total_windows_opened, 2);
    assert_eq!(s.total_windows_closed, 1);
    assert_eq!(s.currently_open, 1);
    assert_eq!(s.total_events, 3);
}

#[test]
fn system_stats_total_runtime() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 100us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    // Window 2: 200us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 50);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 250);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.total_runtime_us, 300); // 100 + 200
}

#[test]
fn system_stats_avg_runtime() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 100us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    // Window 2: 300us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 200);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 500);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.avg_window_runtime_us, 200); // (100 + 300) / 2
}

#[test]
fn system_stats_most_focused_window() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 0);
    // Window 1: 20us focus
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 30);
    // Window 2: 50us focus
    wh.record_at(WindowId(2), WindowEventKind::Focused, 30);
    wh.record_at(WindowId(2), WindowEventKind::Unfocused, 80);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    let (wid, ft) = s.most_focused_window.unwrap();
    assert_eq!(wid, WindowId(2));
    assert_eq!(ft, 50);
}

#[test]
fn system_stats_most_active_app() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // App A: 50us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    ah.record_open("app_a", WindowId(1), b, 0);
    ah.record_close("app_a", WindowId(1), b, 50);

    // App B: 200us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 300);
    ah.record_open("app_b", WindowId(2), b, 100);
    ah.record_close("app_b", WindowId(2), b, 300);

    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    let (app, rt) = s.most_active_app.unwrap();
    assert_eq!(app, "app_b");
    assert_eq!(rt, 200);
}

#[test]
fn system_stats_longest_shortest_session() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 50us (shortest)
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    // Window 2: 300us (longest)
    wh.record_at(WindowId(2), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 400);
    // Window 3: 100us
    wh.record_at(WindowId(3), WindowEventKind::Opened, 500);
    wh.record_at(WindowId(3), WindowEventKind::Closed, 600);

    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    let (lid, lrt) = s.longest_session.unwrap();
    assert_eq!(lid, WindowId(2));
    assert_eq!(lrt, 300);
    let (sid, srt) = s.shortest_session.unwrap();
    assert_eq!(sid, WindowId(1));
    assert_eq!(srt, 50);
}

#[test]
fn system_stats_unique_apps() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    ah.record_open("app_a", WindowId(1), b, 1);
    ah.record_open("app_b", WindowId(2), b, 2);
    ah.record_open("app_c", WindowId(3), b, 3);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 1);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 2);
    wh.record_at(WindowId(3), WindowEventKind::Opened, 3);

    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.unique_apps, 3);
}

#[test]
fn system_stats_timestamp_range() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 42);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 999);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    assert_eq!(s.timestamp_range, Some((42, 999)));
}

#[test]
fn system_stats_events_per_window_avg() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 3 events
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 5);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 10);
    // Window 2: 1 event
    wh.record_at(WindowId(2), WindowEventKind::Opened, 20);
    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    // 4 events / 2 windows = 2.0
    assert!((s.events_per_window_avg - 2.0).abs() < f64::EPSILON);
}

// ========== Query method tests ==========

#[test]
fn all_window_stats_returns_all() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Opened, 1);
    wh.record_at(WindowId(3), WindowEventKind::Opened, 2);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 3);
    let c = make_collector(&wh, &ah);
    let all = c.all_window_stats();
    assert_eq!(all.len(), 3);
}

#[test]
fn top_by_focus_time_ranking() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 10us focus
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 5);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 15);
    // Window 2: 50us focus
    wh.record_at(WindowId(2), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Focused, 20);
    wh.record_at(WindowId(2), WindowEventKind::Unfocused, 70);
    // Window 3: 30us focus
    wh.record_at(WindowId(3), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(3), WindowEventKind::Focused, 75);
    wh.record_at(WindowId(3), WindowEventKind::Unfocused, 105);

    let c = make_collector(&wh, &ah);
    let top = c.top_by_focus_time(2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].window_id, WindowId(2)); // 50
    assert_eq!(top[1].window_id, WindowId(3)); // 30
}

#[test]
fn top_by_runtime_ranking() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 100us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    // Window 2: 500us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 500);
    // Window 3: still open (excluded)
    wh.record_at(WindowId(3), WindowEventKind::Opened, 0);
    // Window 4: 200us
    wh.record_at(WindowId(4), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(4), WindowEventKind::Closed, 200);

    let c = make_collector(&wh, &ah);
    let top = c.top_by_runtime(2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].window_id, WindowId(2)); // 500
    assert_eq!(top[1].window_id, WindowId(4)); // 200
}

#[test]
fn top_apps_by_runtime_ranking() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    // App A: 100us
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    ah.record_open("a", WindowId(1), b, 0);
    ah.record_close("a", WindowId(1), b, 100);

    // App B: 300us
    wh.record_at(WindowId(2), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 300);
    ah.record_open("b", WindowId(2), b, 0);
    ah.record_close("b", WindowId(2), b, 300);

    // App C: 50us
    wh.record_at(WindowId(3), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(3), WindowEventKind::Closed, 50);
    ah.record_open("c", WindowId(3), b, 0);
    ah.record_close("c", WindowId(3), b, 50);

    let c = make_collector(&wh, &ah);
    let top = c.top_apps_by_runtime(2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].app_id, "b"); // 300
    assert_eq!(top[1].app_id, "a"); // 100
}

#[test]
fn idle_windows_detection() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    // Window 1: 200us runtime, no focus → idle
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 200);
    // Window 2: 300us runtime, has focus → not idle
    wh.record_at(WindowId(2), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(2), WindowEventKind::Focused, 10);
    wh.record_at(WindowId(2), WindowEventKind::Unfocused, 20);
    wh.record_at(WindowId(2), WindowEventKind::Closed, 300);
    // Window 3: 50us runtime, no focus → below threshold
    wh.record_at(WindowId(3), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(3), WindowEventKind::Closed, 50);

    let c = make_collector(&wh, &ah);
    let idle = c.idle_windows(100);
    assert_eq!(idle.len(), 1);
    assert_eq!(idle[0], WindowId(1));
}

// ========== Edge cases ==========

#[test]
fn stats_ring_buffer_partial_data() {
    // Ring buffer capacity 3, Opened event may be evicted
    let mut wh = WindowHistory::new(3);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0); // will be evicted
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10); // will be evicted
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 20); // will be evicted
    wh.record_at(
        WindowId(1),
        WindowEventKind::Moved {
            from: Rect::new(0.0, 0.0, 100.0, 100.0),
            to: Rect::new(50.0, 50.0, 100.0, 100.0),
        },
        30,
    );
    wh.record_at(WindowId(1), WindowEventKind::Focused, 40);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);

    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // Opened was evicted
    assert_eq!(s.opened_at, None);
    assert_eq!(s.closed_at, Some(100));
    assert_eq!(s.runtime_us, None); // can't compute without opened_at
    // Focus: 40 → 100 (closed without unfocus) = 60
    assert_eq!(s.focus_time_us, 60);
    assert_eq!(s.move_count, 1);
    assert_eq!(s.total_event_count, 3); // only 3 events in buffer
}

#[test]
fn stats_window_opened_twice_in_history() {
    // Simulate ring buffer wrapping with a second Opened
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 10);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 50);
    // Reused WindowId (hypothetical, e.g. from replay)
    wh.record_at(WindowId(1), WindowEventKind::Opened, 100);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 200);

    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // First Opened is used for opened_at, first Closed for closed_at
    assert_eq!(s.opened_at, Some(10));
    assert_eq!(s.closed_at, Some(50));
    assert_eq!(s.runtime_us, Some(40));
    assert_eq!(s.total_event_count, 4);
}

#[test]
fn stats_focus_without_unfocus_still_open() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 10);
    // No Unfocused, no Closed — window is still focused

    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    // Focus time is 0 (can't compute partial interval without close)
    assert_eq!(s.focus_time_us, 0);
    assert_eq!(s.focus_count, 1);
    assert_eq!(s.runtime_us, None);
}

#[test]
fn stats_zero_runtime_window() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 42);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 42);
    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    assert_eq!(s.opened_at, Some(42));
    assert_eq!(s.closed_at, Some(42));
    assert_eq!(s.runtime_us, Some(0));
}

#[test]
fn stats_serde_roundtrip_window_stats() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Focused, 5);
    wh.record_at(WindowId(1), WindowEventKind::Unfocused, 15);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);

    let c = make_collector(&wh, &ah);
    let s = c.window_stats(WindowId(1));
    let json = serde_json::to_string(&s).unwrap();
    let back: WindowStats = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn stats_serde_roundtrip_system_stats() {
    let mut wh = WindowHistory::new(100);
    let ah = AppHistory::new(100);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);

    let c = make_collector(&wh, &ah);
    let s = c.system_stats();
    let json = serde_json::to_string(&s).unwrap();
    let back: SystemStats = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn stats_serde_roundtrip_app_stats() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);
    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    ah.record_open("app", WindowId(1), b, 0);
    ah.record_close("app", WindowId(1), b, 100);

    let c = make_collector(&wh, &ah);
    let s = c.app_stats("app").unwrap();
    let json = serde_json::to_string(&s).unwrap();
    let back: AppStats = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn stats_display_impls() {
    let mut wh = WindowHistory::new(100);
    let mut ah = AppHistory::new(100);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);

    wh.record_at(WindowId(1), WindowEventKind::Opened, 0);
    wh.record_at(WindowId(1), WindowEventKind::Closed, 100);
    ah.record_open("app", WindowId(1), b, 0);
    ah.record_close("app", WindowId(1), b, 100);

    let c = make_collector(&wh, &ah);

    let ws = c.window_stats(WindowId(1));
    let ws_str = format!("{ws}");
    assert!(ws_str.contains("WindowStats"));
    assert!(ws_str.contains("Window(1)"));

    let as_ = c.app_stats("app").unwrap();
    let as_str = format!("{as_}");
    assert!(as_str.contains("AppStats"));
    assert!(as_str.contains("app"));

    let ss = c.system_stats();
    let ss_str = format!("{ss}");
    assert!(ss_str.contains("SystemStats"));

    let c_str = format!("{c}");
    assert!(c_str.contains("StatsCollector"));
}

// ========== Shell integration tests ==========

#[test]
fn shell_stats_accessor_works() {
    let shell = Shell::new(1920.0, 1080.0);
    let c = shell.stats();
    let s = c.system_stats();
    assert_eq!(s.total_events, 0);
}

#[test]
fn shell_stats_empty_shell() {
    let shell = Shell::new(1920.0, 1080.0);
    let c = shell.stats();
    let all = c.all_window_stats();
    assert!(all.is_empty());
    let top = c.top_by_focus_time(10);
    assert!(top.is_empty());
    let top_rt = c.top_by_runtime(10);
    assert!(top_rt.is_empty());
    let top_apps = c.top_apps_by_runtime(10);
    assert!(top_apps.is_empty());
    let idle = c.idle_windows(0);
    assert!(idle.is_empty());
}

#[test]
fn shell_stats_after_complex_lifecycle() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window_with_app("Editor", Rect::new(0.0, 0.0, 800.0, 600.0), "com.editor");
    let id2 = shell.open_window_with_app(
        "Browser",
        Rect::new(100.0, 100.0, 1000.0, 700.0),
        "com.browser",
    );

    shell.set_focus(id1).unwrap();
    shell.move_window(id1, 50.0, 50.0).unwrap();
    shell.resize_window(id1, 900.0, 700.0).unwrap();
    shell.maximize(id1).unwrap();
    shell.restore(id1).unwrap();

    shell.set_focus(id2).unwrap();
    shell.move_window(id2, 200.0, 200.0).unwrap();

    shell.close_window(id1).unwrap();

    let c = shell.stats();

    // Window stats for id1
    let s1 = c.window_stats(id1);
    assert!(s1.opened_at.is_some());
    assert!(s1.closed_at.is_some());
    assert!(s1.runtime_us.is_some());
    assert!(s1.focus_time_us > 0);
    assert_eq!(s1.move_count, 1);
    assert_eq!(s1.resize_count, 3); // resize + maximize resize + restore resize
    assert!(s1.state_change_count >= 2); // maximize + restore

    // Window stats for id2 (still open)
    let s2 = c.window_stats(id2);
    assert!(s2.opened_at.is_some());
    assert_eq!(s2.closed_at, None);
    assert_eq!(s2.runtime_us, None);
    assert_eq!(s2.move_count, 1);

    // System stats
    let sys = c.system_stats();
    assert_eq!(sys.total_windows_opened, 2);
    assert_eq!(sys.total_windows_closed, 1);
    assert_eq!(sys.currently_open, 1);
    assert!(sys.total_runtime_us > 0);
    assert!(sys.total_focus_switches > 0);
    assert_eq!(sys.unique_apps, 2);

    // App stats
    let editor = c.app_stats("com.editor").unwrap();
    assert_eq!(editor.closed_sessions, 1);
    assert_eq!(editor.active_sessions, 0);
    assert!(editor.total_runtime_us > 0);

    let browser = c.app_stats("com.browser").unwrap();
    assert_eq!(browser.active_sessions, 1);
    assert_eq!(browser.closed_sessions, 0);

    // All window stats
    let all = c.all_window_stats();
    assert_eq!(all.len(), 2);

    // Top by runtime (only closed windows)
    let top_rt = c.top_by_runtime(10);
    assert_eq!(top_rt.len(), 1);
    assert_eq!(top_rt[0].window_id, id1);
}
