//! Tests for session state save/restore.

use crate::recent::RecentSessions;
use crate::restore::{DisplayChange, SessionRestorer};
use crate::state::*;
use crate::store::SessionStore;

// ── Helpers ─────────────────────────────────────────────────────────

fn sample_display(connector: &str, w: u32, h: u32, x: i32, y: i32, primary: bool) -> DisplayState {
    DisplayState {
        connector: connector.to_string(),
        resolution: (w, h),
        position: (x, y),
        scale: 1.0,
        primary,
    }
}

fn sample_workspace(id: u32, name: &str, monitor: u32) -> WorkspaceState {
    WorkspaceState {
        id,
        name: name.to_string(),
        monitor_id: monitor,
    }
}

fn sample_window(
    id: u64,
    app: &str,
    title: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    ws: u32,
) -> WindowState {
    WindowState {
        window_id: id,
        app_id: app.to_string(),
        title: title.to_string(),
        bounds: (x, y, w, h),
        workspace_id: ws,
        state: WindowVisualState::Normal,
        z_order: id as u32,
        is_sticky: false,
    }
}

fn sample_session() -> SessionState {
    SessionState {
        windows: vec![
            sample_window(1, "terminal", "Terminal", 100.0, 100.0, 800.0, 600.0, 0),
            sample_window(2, "browser", "Browser", 200.0, 150.0, 1024.0, 768.0, 0),
            {
                let mut w =
                    sample_window(3, "editor", "Editor", 50.0, 50.0, 640.0, 480.0, 1);
                w.state = WindowVisualState::Maximized;
                w.is_sticky = true;
                w
            },
        ],
        workspaces: vec![
            sample_workspace(0, "Main", 0),
            sample_workspace(1, "Code", 0),
        ],
        active_workspace: 0,
        focused_window: Some(2),
        timestamp: 1700000000000,
        theme_id: "night".to_string(),
        display_config: vec![
            sample_display("eDP-1", 1920, 1080, 0, 0, true),
            sample_display("HDMI-1", 2560, 1440, 1920, 0, false),
        ],
    }
}

// ── WindowVisualState tests ─────────────────────────────────────────

#[test]
fn visual_state_roundtrip() {
    let states = [
        WindowVisualState::Normal,
        WindowVisualState::Maximized,
        WindowVisualState::Minimized,
        WindowVisualState::Fullscreen,
    ];
    for s in &states {
        let tag = s.as_str();
        let parsed = WindowVisualState::from_str(tag).unwrap();
        assert_eq!(*s, parsed);
    }
}

#[test]
fn visual_state_case_insensitive() {
    assert_eq!(
        WindowVisualState::from_str("MAXIMIZED"),
        Some(WindowVisualState::Maximized)
    );
    assert_eq!(
        WindowVisualState::from_str("  Fullscreen  "),
        Some(WindowVisualState::Fullscreen)
    );
}

#[test]
fn visual_state_unknown() {
    assert_eq!(WindowVisualState::from_str("floating"), None);
}

// ── SessionState::empty ─────────────────────────────────────────────

#[test]
fn empty_state_defaults() {
    let s = SessionState::empty();
    assert!(s.windows.is_empty());
    assert!(s.workspaces.is_empty());
    assert_eq!(s.active_workspace, 0);
    assert_eq!(s.focused_window, None);
    assert_eq!(s.timestamp, 0);
    assert!(s.theme_id.is_empty());
    assert!(s.display_config.is_empty());
}

// ── Store round-trip ────────────────────────────────────────────────

#[test]
fn store_roundtrip() {
    let original = sample_session();
    let serialized = SessionStore::save(&original).unwrap();
    let loaded = SessionStore::load(&serialized).unwrap();
    assert_eq!(original, loaded);
}

#[test]
fn store_empty_session() {
    let empty = SessionState::empty();
    let serialized = SessionStore::save(&empty).unwrap();
    let loaded = SessionStore::load(&serialized).unwrap();
    assert_eq!(empty, loaded);
}

#[test]
fn store_focused_window_none() {
    let mut state = SessionState::empty();
    state.theme_id = "sunset".to_string();
    state.focused_window = None;
    let s = SessionStore::save(&state).unwrap();
    assert!(s.contains("focused_window=none"));
    let loaded = SessionStore::load(&s).unwrap();
    assert_eq!(loaded.focused_window, None);
}

#[test]
fn store_window_all_states() {
    for vs in &[
        WindowVisualState::Normal,
        WindowVisualState::Maximized,
        WindowVisualState::Minimized,
        WindowVisualState::Fullscreen,
    ] {
        let mut state = SessionState::empty();
        state.windows.push(WindowState {
            window_id: 42,
            app_id: "test".to_string(),
            title: "Test".to_string(),
            bounds: (10.0, 20.0, 300.0, 400.0),
            workspace_id: 0,
            state: *vs,
            z_order: 1,
            is_sticky: false,
        });
        let s = SessionStore::save(&state).unwrap();
        let loaded = SessionStore::load(&s).unwrap();
        assert_eq!(loaded.windows[0].state, *vs);
    }
}

#[test]
fn store_sticky_window() {
    let mut state = SessionState::empty();
    state.windows.push(WindowState {
        window_id: 7,
        app_id: "chat".to_string(),
        title: "Chat".to_string(),
        bounds: (0.0, 0.0, 200.0, 200.0),
        workspace_id: 0,
        state: WindowVisualState::Normal,
        z_order: 0,
        is_sticky: true,
    });
    let s = SessionStore::save(&state).unwrap();
    let loaded = SessionStore::load(&s).unwrap();
    assert!(loaded.windows[0].is_sticky);
}

#[test]
fn store_multiple_displays() {
    let state = sample_session();
    let s = SessionStore::save(&state).unwrap();
    let loaded = SessionStore::load(&s).unwrap();
    assert_eq!(loaded.display_config.len(), 2);
    assert_eq!(loaded.display_config[0].connector, "eDP-1");
    assert_eq!(loaded.display_config[1].resolution, (2560, 1440));
    assert!(loaded.display_config[0].primary);
    assert!(!loaded.display_config[1].primary);
}

#[test]
fn store_display_negative_position() {
    let mut state = SessionState::empty();
    state.display_config.push(DisplayState {
        connector: "DP-2".to_string(),
        resolution: (3840, 2160),
        position: (-3840, -100),
        scale: 2.0,
        primary: false,
    });
    let s = SessionStore::save(&state).unwrap();
    let loaded = SessionStore::load(&s).unwrap();
    assert_eq!(loaded.display_config[0].position, (-3840, -100));
    assert_eq!(loaded.display_config[0].scale, 2.0);
}

#[test]
fn store_parse_error_bad_timestamp() {
    let bad = "[session]\ntimestamp=abc\n";
    assert!(SessionStore::load(bad).is_err());
}

#[test]
fn store_parse_error_bad_resolution() {
    let bad = "[session]\ntimestamp=0\nactive_workspace=0\nfocused_window=none\ntheme_id=x\n\n[display.0]\nconnector=DP-1\nresolution=bad\n";
    assert!(SessionStore::load(bad).is_err());
}

#[test]
fn store_ignores_unknown_sections() {
    let data = "[session]\ntimestamp=100\nactive_workspace=0\nfocused_window=none\ntheme_id=t\n\n[unknown_future_section]\nfoo=bar\n";
    let loaded = SessionStore::load(data).unwrap();
    assert_eq!(loaded.timestamp, 100);
}

#[test]
fn store_ignores_comments_and_blank_lines() {
    let data = "# comment\n\n[session]\n# another comment\ntimestamp=42\nactive_workspace=1\nfocused_window=none\ntheme_id=night\n";
    let loaded = SessionStore::load(data).unwrap();
    assert_eq!(loaded.timestamp, 42);
    assert_eq!(loaded.active_workspace, 1);
}

// ── File I/O ────────────────────────────────────────────────────────

#[test]
fn store_file_roundtrip() {
    let state = sample_session();
    let dir = std::env::temp_dir().join("liquide_session_test");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_session.state");
    let path_str = path.to_str().unwrap();

    SessionStore::save_to_file(&state, path_str).unwrap();
    let loaded = SessionStore::load_from_file(path_str).unwrap();
    assert_eq!(state, loaded);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn store_load_missing_file() {
    let result = SessionStore::load_from_file("/nonexistent/path/session.state");
    assert!(result.is_err());
}

// ── Auto-save path ──────────────────────────────────────────────────

#[test]
fn auto_save_path_not_empty() {
    let p = SessionStore::auto_save_path();
    assert!(!p.is_empty());
    assert!(p.contains("liquide"));
}

// ── Restore planner ─────────────────────────────────────────────────

#[test]
fn restore_plan_basic() {
    let state = sample_session();
    let available = vec!["terminal".to_string(), "browser".to_string()];
    let plan = SessionRestorer::plan_restore(&state, &available);

    assert_eq!(plan.windows_to_restore.len(), 2);
    assert_eq!(plan.missing_apps, vec!["editor".to_string()]);
}

#[test]
fn restore_plan_all_available() {
    let state = sample_session();
    let available = vec![
        "terminal".to_string(),
        "browser".to_string(),
        "editor".to_string(),
    ];
    let plan = SessionRestorer::plan_restore(&state, &available);
    assert_eq!(plan.windows_to_restore.len(), 3);
    assert!(plan.missing_apps.is_empty());
}

#[test]
fn restore_plan_none_available() {
    let state = sample_session();
    let plan = SessionRestorer::plan_restore(&state, &[]);
    assert!(plan.windows_to_restore.is_empty());
    assert_eq!(plan.missing_apps.len(), 3);
}

#[test]
fn restore_plan_preserves_state() {
    let state = sample_session();
    let available = vec!["editor".to_string()];
    let plan = SessionRestorer::plan_restore(&state, &available);
    assert_eq!(plan.windows_to_restore.len(), 1);
    assert_eq!(plan.windows_to_restore[0].state, WindowVisualState::Maximized);
}

#[test]
fn restore_plan_duplicate_app_missing_once() {
    // Two windows from the same missing app should only list it once.
    let mut state = SessionState::empty();
    state.windows.push(sample_window(1, "gone_app", "W1", 0.0, 0.0, 100.0, 100.0, 0));
    state.windows.push(sample_window(2, "gone_app", "W2", 0.0, 0.0, 100.0, 100.0, 0));
    let plan = SessionRestorer::plan_restore(&state, &[]);
    assert_eq!(plan.missing_apps.len(), 1);
    assert_eq!(plan.missing_apps[0], "gone_app");
}

// ── Display change detection ────────────────────────────────────────

#[test]
fn display_change_moved() {
    let saved = vec![sample_display("eDP-1", 1920, 1080, 0, 0, true)];
    let current = vec![sample_display("eDP-1", 1920, 1080, 100, 50, true)];

    let state = SessionState {
        windows: vec![sample_window(1, "term", "T", 400.0, 300.0, 800.0, 600.0, 0)],
        display_config: saved.clone(),
        ..SessionState::empty()
    };

    let available = vec!["term".to_string()];
    let mut plan = SessionRestorer::plan_restore(&state, &available);
    SessionRestorer::adjust_for_display_changes(&mut plan, &saved, &current);

    assert_eq!(plan.display_changes.len(), 1);
    assert_eq!(
        plan.display_changes[0],
        DisplayChange::Moved {
            connector: "eDP-1".to_string(),
            from: (0, 0),
            to: (100, 50),
        }
    );

    // Window should be shifted by the delta.
    assert!((plan.windows_to_restore[0].bounds.0 - 500.0).abs() < 0.1);
    assert!((plan.windows_to_restore[0].bounds.1 - 350.0).abs() < 0.1);
}

#[test]
fn display_change_removed_moves_to_primary() {
    let saved = vec![
        sample_display("eDP-1", 1920, 1080, 0, 0, true),
        sample_display("HDMI-1", 2560, 1440, 1920, 0, false),
    ];
    // Only the laptop display remains.
    let current = vec![sample_display("eDP-1", 1920, 1080, 0, 0, true)];

    let state = SessionState {
        // Window was on HDMI-1.
        windows: vec![sample_window(1, "term", "T", 2000.0, 200.0, 800.0, 600.0, 0)],
        display_config: saved.clone(),
        ..SessionState::empty()
    };

    let available = vec!["term".to_string()];
    let mut plan = SessionRestorer::plan_restore(&state, &available);
    SessionRestorer::adjust_for_display_changes(&mut plan, &saved, &current);

    // Should have been moved onto eDP-1 (centered).
    let b = plan.windows_to_restore[0].bounds;
    assert!(b.0 >= 0.0 && b.0 + b.2 <= 1920.0);
    assert!(b.1 >= 0.0 && b.1 + b.3 <= 1080.0);

    // Display changes should report the removal.
    assert!(plan
        .display_changes
        .contains(&DisplayChange::Removed("HDMI-1".to_string())));
}

#[test]
fn display_change_added() {
    let saved = vec![sample_display("eDP-1", 1920, 1080, 0, 0, true)];
    let current = vec![
        sample_display("eDP-1", 1920, 1080, 0, 0, true),
        sample_display("DP-3", 3840, 2160, 1920, 0, false),
    ];

    let state = SessionState {
        windows: vec![],
        display_config: saved.clone(),
        ..SessionState::empty()
    };

    let mut plan = SessionRestorer::plan_restore(&state, &[]);
    SessionRestorer::adjust_for_display_changes(&mut plan, &saved, &current);

    assert!(plan
        .display_changes
        .contains(&DisplayChange::Added("DP-3".to_string())));
}

#[test]
fn display_clamp_oversized_window() {
    let saved = vec![sample_display("eDP-1", 1920, 1080, 0, 0, true)];
    let current = vec![sample_display("eDP-1", 1280, 720, 0, 0, true)];

    let state = SessionState {
        // Window larger than new resolution.
        windows: vec![sample_window(1, "app", "A", 0.0, 0.0, 1920.0, 1080.0, 0)],
        display_config: saved.clone(),
        ..SessionState::empty()
    };

    let available = vec!["app".to_string()];
    let mut plan = SessionRestorer::plan_restore(&state, &available);
    SessionRestorer::adjust_for_display_changes(&mut plan, &saved, &current);

    let b = plan.windows_to_restore[0].bounds;
    assert!(b.2 <= 1280.0);
    assert!(b.3 <= 720.0);
}

// ── Recent sessions ─────────────────────────────────────────────────

#[test]
fn recent_add_and_list() {
    let mut recent = RecentSessions::new(3);
    assert!(recent.is_empty());

    let mut s1 = SessionState::empty();
    s1.timestamp = 100;
    s1.theme_id = "night".to_string();

    let mut s2 = SessionState::empty();
    s2.timestamp = 200;
    s2.theme_id = "sunset".to_string();

    recent.add(s1);
    recent.add(s2);

    assert_eq!(recent.len(), 2);
    let list = recent.list();
    // Newest first.
    assert_eq!(list[0].timestamp, 200);
    assert_eq!(list[1].timestamp, 100);
}

#[test]
fn recent_evicts_oldest() {
    let mut recent = RecentSessions::new(2);

    for ts in [10, 20, 30] {
        let mut s = SessionState::empty();
        s.timestamp = ts;
        recent.add(s);
    }

    assert_eq!(recent.len(), 2);
    let list = recent.list();
    assert_eq!(list[0].timestamp, 30);
    assert_eq!(list[1].timestamp, 20);
}

#[test]
fn recent_get_by_index() {
    let mut recent = RecentSessions::new(5);

    for ts in [100, 200, 300] {
        let mut s = SessionState::empty();
        s.timestamp = ts;
        recent.add(s);
    }

    assert_eq!(recent.get(0).unwrap().timestamp, 300); // newest
    assert_eq!(recent.get(1).unwrap().timestamp, 200);
    assert_eq!(recent.get(2).unwrap().timestamp, 100); // oldest
    assert!(recent.get(3).is_none());
}

#[test]
fn recent_latest() {
    let mut recent = RecentSessions::new(5);
    assert!(recent.latest().is_none());

    let mut s = SessionState::empty();
    s.timestamp = 999;
    recent.add(s);

    assert_eq!(recent.latest().unwrap().timestamp, 999);
}

#[test]
fn recent_clear() {
    let mut recent = RecentSessions::new(5);
    let mut s = SessionState::empty();
    s.timestamp = 1;
    recent.add(s);
    assert!(!recent.is_empty());
    recent.clear();
    assert!(recent.is_empty());
    assert_eq!(recent.len(), 0);
}

#[test]
fn recent_default_capacity() {
    let recent = RecentSessions::default_capacity();
    assert_eq!(recent.capacity(), 5);
}

#[test]
fn recent_summary_fields() {
    let mut recent = RecentSessions::new(5);
    let mut s = sample_session();
    s.timestamp = 555;
    recent.add(s);

    let list = recent.list();
    assert_eq!(list[0].timestamp, 555);
    assert_eq!(list[0].window_count, 3);
    assert_eq!(list[0].workspace_count, 2);
    assert_eq!(list[0].theme_id, "night");
}

#[test]
#[should_panic(expected = "max_count must be at least 1")]
fn recent_zero_capacity_panics() {
    RecentSessions::new(0);
}

// ── Integration: save → load → restore ──────────────────────────────

#[test]
fn full_save_load_restore_cycle() {
    let original = sample_session();

    // Save and reload.
    let serialized = SessionStore::save(&original).unwrap();
    let loaded = SessionStore::load(&serialized).unwrap();
    assert_eq!(original, loaded);

    // Plan restore with one missing app.
    let available = vec!["terminal".to_string(), "browser".to_string()];
    let mut plan = SessionRestorer::plan_restore(&loaded, &available);

    // Same displays — no changes expected.
    SessionRestorer::adjust_for_display_changes(
        &mut plan,
        &loaded.display_config,
        &loaded.display_config,
    );

    assert_eq!(plan.windows_to_restore.len(), 2);
    assert_eq!(plan.missing_apps, vec!["editor".to_string()]);
    assert!(plan.display_changes.is_empty());

    // Store in recent sessions.
    let mut recent = RecentSessions::default_capacity();
    recent.add(loaded);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent.latest().unwrap().timestamp, 1700000000000);
}
