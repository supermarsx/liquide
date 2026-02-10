use crate::app_history::*;
use crate::window::WindowId;
use liquide_compositor::geometry::Rect;

// ========== AppHistory unit tests ==========

#[test]
fn app_history_new_empty() {
    let h = AppHistory::new(100);
    assert_eq!(h.tracked_count(), 0);
    assert_eq!(h.max_tracked(), 100);
}

#[test]
fn app_history_record_open() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
    h.record_open("com.example.app", WindowId(1), bounds, 1);
    assert_eq!(h.tracked_count(), 1);

    let info = h.app_info("com.example.app").unwrap();
    assert_eq!(info.app_id, "com.example.app");
    assert_eq!(info.total_windows_opened, 1);
    assert_eq!(info.active_window_count, 1);
    assert_eq!(info.sessions.len(), 1);
    assert!(info.sessions[0].closed_at.is_none());
}

#[test]
fn app_history_record_open_close_session() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(10.0, 20.0, 400.0, 300.0);
    h.record_open("com.example.app", WindowId(1), bounds, 10);
    h.record_close("com.example.app", WindowId(1), bounds, 50);

    let info = h.app_info("com.example.app").unwrap();
    assert_eq!(info.sessions.len(), 1);
    assert_eq!(info.sessions[0].opened_at, 10);
    assert_eq!(info.sessions[0].closed_at, Some(50));
}

#[test]
fn app_history_active_window_count() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
    h.record_open("app", WindowId(1), bounds, 1);
    h.record_open("app", WindowId(2), bounds, 2);
    assert_eq!(h.app_info("app").unwrap().active_window_count, 2);

    h.record_close("app", WindowId(1), bounds, 3);
    assert_eq!(h.app_info("app").unwrap().active_window_count, 1);

    h.record_close("app", WindowId(2), bounds, 4);
    assert_eq!(h.app_info("app").unwrap().active_window_count, 0);
}

#[test]
fn app_history_total_windows_opened() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
    h.record_open("app", WindowId(1), bounds, 1);
    h.record_close("app", WindowId(1), bounds, 2);
    h.record_open("app", WindowId(2), bounds, 3);
    h.record_close("app", WindowId(2), bounds, 4);
    h.record_open("app", WindowId(3), bounds, 5);

    assert_eq!(h.app_info("app").unwrap().total_windows_opened, 3);
}

#[test]
fn app_history_first_last_seen() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app", WindowId(1), bounds, 10);
    h.record_open("app", WindowId(2), bounds, 50);
    h.record_close("app", WindowId(1), bounds, 100);

    let info = h.app_info("app").unwrap();
    assert_eq!(info.first_seen, 10);
    assert_eq!(info.last_seen, 100);
}

#[test]
fn app_history_last_bounds_remembered() {
    let mut h = AppHistory::new(100);
    let b1 = Rect::new(0.0, 0.0, 400.0, 300.0);
    let b2 = Rect::new(50.0, 50.0, 600.0, 400.0);
    h.record_open("app", WindowId(1), b1, 1);
    h.record_close("app", WindowId(1), b2, 2);

    let info = h.app_info("app").unwrap();
    assert_eq!(info.last_bounds, Some(b2));
}

#[test]
fn app_history_last_bounds_for() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(100.0, 200.0, 800.0, 600.0);
    h.record_open("app", WindowId(1), bounds, 1);
    h.record_close("app", WindowId(1), bounds, 2);

    assert_eq!(h.last_bounds_for("app"), Some(bounds));
    assert_eq!(h.last_bounds_for("nonexistent"), None);
}

#[test]
fn app_history_most_recent() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app_a", WindowId(1), bounds, 10);
    h.record_open("app_b", WindowId(2), bounds, 20);
    h.record_open("app_c", WindowId(3), bounds, 30);

    let recent = h.most_recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].app_id, "app_c");
    assert_eq!(recent[1].app_id, "app_b");
}

#[test]
fn app_history_most_frequent() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

    // app_a: 3 windows
    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_open("app_a", WindowId(2), bounds, 2);
    h.record_open("app_a", WindowId(3), bounds, 3);
    // app_b: 1 window
    h.record_open("app_b", WindowId(4), bounds, 4);
    // app_c: 2 windows
    h.record_open("app_c", WindowId(5), bounds, 5);
    h.record_open("app_c", WindowId(6), bounds, 6);

    let frequent = h.most_frequent(2);
    assert_eq!(frequent.len(), 2);
    assert_eq!(frequent[0].app_id, "app_a");
    assert_eq!(frequent[1].app_id, "app_c");
}

#[test]
fn app_history_most_recent_empty() {
    let h = AppHistory::new(100);
    let recent = h.most_recent(10);
    assert!(recent.is_empty());
}

#[test]
fn app_history_most_frequent_more_than_tracked() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_open("app_b", WindowId(2), bounds, 2);

    let frequent = h.most_frequent(100);
    assert_eq!(frequent.len(), 2);
}

#[test]
fn app_history_max_tracked_eviction() {
    let mut h = AppHistory::new(3);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_close("app_a", WindowId(1), bounds, 2);
    h.record_open("app_b", WindowId(2), bounds, 3);
    h.record_close("app_b", WindowId(2), bounds, 4);
    h.record_open("app_c", WindowId(3), bounds, 5);
    h.record_close("app_c", WindowId(3), bounds, 6);

    // At capacity with 3 apps, all inactive. Adding a 4th should evict one.
    h.record_open("app_d", WindowId(4), bounds, 7);

    assert_eq!(h.tracked_count(), 3);
    // app_a was least recently seen, should be evicted
    assert!(h.app_info("app_a").is_none());
    assert!(h.app_info("app_d").is_some());
}

#[test]
fn app_history_eviction_prefers_least_recently_used() {
    let mut h = AppHistory::new(3);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_close("app_a", WindowId(1), bounds, 2);
    h.record_open("app_b", WindowId(2), bounds, 3);
    h.record_close("app_b", WindowId(2), bounds, 4);
    h.record_open("app_c", WindowId(3), bounds, 5);
    h.record_close("app_c", WindowId(3), bounds, 6);

    // Touch app_a to make it more recent than app_b
    h.touch("app_a", 7);

    // Now app_b is least recently seen among inactive apps
    h.record_open("app_d", WindowId(4), bounds, 8);

    assert!(h.app_info("app_b").is_none()); // evicted
    assert!(h.app_info("app_a").is_some()); // kept (more recent)
}

#[test]
fn app_history_no_eviction_when_all_active() {
    let mut h = AppHistory::new(2);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

    // Both apps are active (not closed)
    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_open("app_b", WindowId(2), bounds, 2);

    // Adding a third when all are active → soft-cap overflow
    h.record_open("app_c", WindowId(3), bounds, 3);

    assert_eq!(h.tracked_count(), 3);
    assert!(h.app_info("app_a").is_some());
    assert!(h.app_info("app_b").is_some());
    assert!(h.app_info("app_c").is_some());
}

#[test]
fn app_history_multiple_sessions_same_app() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 400.0, 300.0);

    h.record_open("app", WindowId(1), bounds, 1);
    h.record_close("app", WindowId(1), bounds, 2);
    h.record_open("app", WindowId(2), bounds, 3);
    h.record_close("app", WindowId(2), bounds, 4);
    h.record_open("app", WindowId(3), bounds, 5);

    let info = h.app_info("app").unwrap();
    assert_eq!(info.sessions.len(), 3);
    assert_eq!(info.sessions[0].closed_at, Some(2));
    assert_eq!(info.sessions[1].closed_at, Some(4));
    assert!(info.sessions[2].closed_at.is_none());
}

#[test]
fn app_history_empty_app_id_ignored() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("", WindowId(1), bounds, 1);
    assert_eq!(h.tracked_count(), 0);
}

#[test]
fn app_history_clear() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app_a", WindowId(1), bounds, 1);
    h.record_open("app_b", WindowId(2), bounds, 2);
    assert_eq!(h.tracked_count(), 2);

    h.clear();
    assert_eq!(h.tracked_count(), 0);
}

#[test]
fn app_history_serde_roundtrip_app_info() {
    let info = AppInfo {
        app_id: "com.example.app".to_string(),
        first_seen: 100,
        last_seen: 500,
        total_windows_opened: 3,
        active_window_count: 1,
        sessions: vec![AppSession {
            window_id: WindowId(42),
            opened_at: 100,
            closed_at: Some(200),
            last_bounds: Rect::new(10.0, 20.0, 300.0, 200.0),
        }],
        last_bounds: Some(Rect::new(10.0, 20.0, 300.0, 200.0)),
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: AppInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(info, back);
}

#[test]
fn app_history_serde_roundtrip_app_session() {
    let session = AppSession {
        window_id: WindowId(7),
        opened_at: 50,
        closed_at: None,
        last_bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
    };
    let json = serde_json::to_string(&session).unwrap();
    let back: AppSession = serde_json::from_str(&json).unwrap();
    assert_eq!(session, back);
}

#[test]
fn app_history_display_app_info() {
    let info = AppInfo {
        app_id: "com.example.app".to_string(),
        first_seen: 1,
        last_seen: 100,
        total_windows_opened: 5,
        active_window_count: 2,
        sessions: Vec::new(),
        last_bounds: None,
    };
    let s = format!("{info}");
    assert!(s.contains("com.example.app"));
    assert!(s.contains("5"));
    assert!(s.contains("2"));
}

#[test]
fn app_history_display_app_session() {
    let session = AppSession {
        window_id: WindowId(3),
        opened_at: 42,
        closed_at: Some(99),
        last_bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
    };
    let s = format!("{session}");
    assert!(s.contains("42"));
    assert!(s.contains("99"));
}

#[test]
fn app_history_display_app_history() {
    let mut h = AppHistory::new(50);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app", WindowId(1), bounds, 1);
    let s = format!("{h}");
    assert!(s.contains("1/50"));
}

#[test]
fn app_history_touch_updates_last_seen() {
    let mut h = AppHistory::new(100);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    h.record_open("app", WindowId(1), bounds, 10);
    assert_eq!(h.app_info("app").unwrap().last_seen, 10);

    h.touch("app", 500);
    assert_eq!(h.app_info("app").unwrap().last_seen, 500);
}
