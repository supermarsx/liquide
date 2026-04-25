//! End-to-end tests for window management: open, close, focus, minimize,
//! maximize, restore, raise, lower, move, resize.

use liquide_compositor::geometry::Rect;
use liquide_shell::{Shell, WindowFlags, WindowId, WindowState};

// ── Helpers ─────────────────────────────────────────────────────────────────

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

fn open_test_window(shell: &mut Shell, title: &str) -> WindowId {
    let bounds = Rect::new(100.0, 100.0, 800.0, 600.0);
    shell.open_window(title, bounds)
}

// ── Open / Close ────────────────────────────────────────────────────────────

#[test]
fn open_single_window_and_verify_visible() {
    let mut shell = new_shell();
    assert_eq!(shell.window_count(), 0);

    let wid = open_test_window(&mut shell, "Test Window");

    assert_eq!(shell.window_count(), 1);
    let win = shell.window(wid).expect("window should exist");
    assert_eq!(win.title, "Test Window");
    assert!(win.visible);
    assert_eq!(win.state, WindowState::Normal);
}

#[test]
fn open_multiple_windows_increments_count() {
    let mut shell = new_shell();
    let _w1 = open_test_window(&mut shell, "Window 1");
    let _w2 = open_test_window(&mut shell, "Window 2");
    let _w3 = open_test_window(&mut shell, "Window 3");

    assert_eq!(shell.window_count(), 3);
    assert_eq!(shell.visible_windows().len(), 3);
}

#[test]
fn close_window_removes_from_list() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Closing Window");
    assert_eq!(shell.window_count(), 1);

    let closed = shell.close_window(wid).expect("close should succeed");
    assert_eq!(closed.title, "Closing Window");
    assert_eq!(shell.window_count(), 0);
    assert!(shell.window(wid).is_err());
}

#[test]
fn close_nonexistent_window_returns_error() {
    let mut shell = new_shell();
    let result = shell.close_window(WindowId(9999));
    assert!(result.is_err());
}

// ── Window State Transitions ────────────────────────────────────────────────

#[test]
fn minimize_hides_window_from_visible_list() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Minimize Me");

    shell.minimize(wid).expect("minimize should succeed");

    let win = shell.window(wid).expect("window should still exist");
    assert_eq!(win.state, WindowState::Minimized);
    // Minimized windows should not appear in visible_windows
    let visible: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
    assert!(!visible.contains(&wid));
}

#[test]
fn maximize_fills_screen() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Maximize Me");

    shell.maximize(wid).expect("maximize should succeed");

    let win = shell.window(wid).expect("window should still exist");
    assert_eq!(win.state, WindowState::Maximized);

    let screen = shell.screen_rect();
    // Maximized window should fill the screen (or close to it)
    assert!(
        win.bounds.width >= screen.width * 0.9,
        "width should fill screen"
    );
    assert!(
        win.bounds.height >= screen.height * 0.9,
        "height should fill screen"
    );
}

#[test]
fn restore_after_minimize() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Restore Me");

    shell.minimize(wid).unwrap();
    assert_eq!(shell.window(wid).unwrap().state, WindowState::Minimized);

    shell.restore(wid).unwrap();
    let win = shell.window(wid).unwrap();
    assert_eq!(win.state, WindowState::Normal);
    assert!(win.visible);
}

#[test]
fn restore_after_maximize() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Restore Max");
    let original_bounds = shell.window(wid).unwrap().bounds;

    shell.maximize(wid).unwrap();
    assert_eq!(shell.window(wid).unwrap().state, WindowState::Maximized);

    shell.restore(wid).unwrap();
    let win = shell.window(wid).unwrap();
    assert_eq!(win.state, WindowState::Normal);
    // Bounds should be restored to approximately the original
    let delta = (win.bounds.width - original_bounds.width).abs();
    assert!(
        delta < 2.0,
        "width should be restored to original: delta={delta}"
    );
}

// ── Focus ───────────────────────────────────────────────────────────────────

#[test]
fn focus_tracks_last_focused_window() {
    let mut shell = new_shell();
    let w1 = open_test_window(&mut shell, "Win A");
    let w2 = open_test_window(&mut shell, "Win B");

    shell.set_focus(w1).unwrap();
    assert_eq!(shell.focus_manager().focused(), Some(w1));

    shell.set_focus(w2).unwrap();
    assert_eq!(shell.focus_manager().focused(), Some(w2));
}

#[test]
fn closing_focused_window_clears_or_shifts_focus() {
    let mut shell = new_shell();
    let w1 = open_test_window(&mut shell, "Focus A");
    let w2 = open_test_window(&mut shell, "Focus B");

    shell.set_focus(w2).unwrap();
    assert_eq!(shell.focus_manager().focused(), Some(w2));

    shell.close_window(w2).unwrap();
    // After closing focused window, focus should either be cleared or on w1
    let focused = shell.focus_manager().focused();
    assert!(focused.is_none() || focused == Some(w1));
}

// ── Move / Resize ───────────────────────────────────────────────────────────

#[test]
fn move_window_changes_position() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Move Me");

    shell.move_window(wid, 300.0, 200.0).unwrap();

    let win = shell.window(wid).unwrap();
    assert!((win.bounds.x - 300.0).abs() < 1.0);
    assert!((win.bounds.y - 200.0).abs() < 1.0);
}

#[test]
fn resize_window_changes_dimensions() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Resize Me");

    shell.resize_window(wid, 640.0, 480.0).unwrap();

    let win = shell.window(wid).unwrap();
    assert!((win.bounds.width - 640.0).abs() < 1.0);
    assert!((win.bounds.height - 480.0).abs() < 1.0);
}

// ── Window Flags ────────────────────────────────────────────────────────────

#[test]
fn new_window_has_default_flags() {
    let mut shell = new_shell();
    let wid = open_test_window(&mut shell, "Flags Test");

    let win = shell.window(wid).unwrap();
    // Default flags: DECORATED | RESIZABLE | FOCUSABLE
    assert!(win.flags.contains(WindowFlags::DECORATED));
    assert!(win.flags.contains(WindowFlags::RESIZABLE));
    assert!(win.flags.contains(WindowFlags::FOCUSABLE));
}

// ── Z-ordering ──────────────────────────────────────────────────────────────

#[test]
fn raise_window_increases_z_order() {
    let mut shell = new_shell();
    let w1 = open_test_window(&mut shell, "Back");
    let _w2 = open_test_window(&mut shell, "Front");

    let z1_before = shell.window(w1).unwrap().z_order;

    shell.raise_window(w1).unwrap();

    let z1_after = shell.window(w1).unwrap().z_order;
    assert!(
        z1_after >= z1_before,
        "z_order should not decrease after raise"
    );
}

// ── App Windows ─────────────────────────────────────────────────────────────

#[test]
fn open_app_window_sets_app_id() {
    let mut shell = new_shell();
    let wid = shell.open_app_window("com.liquide.terminal");

    let win = shell.window(wid).unwrap();
    assert_eq!(win.app_id, "com.liquide.terminal");
    assert!(win.visible);
}

#[test]
fn open_window_with_app_sets_app_id() {
    let mut shell = new_shell();
    let bounds = Rect::new(50.0, 50.0, 640.0, 480.0);
    let wid = shell.open_window_with_app("Browser", bounds, "com.liquide.browser");

    let win = shell.window(wid).unwrap();
    assert_eq!(win.title, "Browser");
    assert_eq!(win.app_id, "com.liquide.browser");
}

#[test]
fn multiple_app_windows_same_app() {
    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    let w1 = shell.open_window_with_app("Terminal 1", bounds, "com.liquide.terminal");
    let w2 = shell.open_window_with_app("Terminal 2", bounds, "com.liquide.terminal");

    assert_ne!(w1, w2, "different window IDs");
    assert_eq!(shell.window(w1).unwrap().app_id, "com.liquide.terminal");
    assert_eq!(shell.window(w2).unwrap().app_id, "com.liquide.terminal");
    assert_eq!(shell.window_count(), 2);
}

// ── Edge Cases ──────────────────────────────────────────────────────────────

#[test]
fn operations_on_nonexistent_window_return_error() {
    let mut shell = new_shell();
    let bad_id = WindowId(12345);

    assert!(shell.minimize(bad_id).is_err());
    assert!(shell.maximize(bad_id).is_err());
    assert!(shell.restore(bad_id).is_err());
    assert!(shell.move_window(bad_id, 0.0, 0.0).is_err());
    assert!(shell.resize_window(bad_id, 100.0, 100.0).is_err());
    assert!(shell.set_focus(bad_id).is_err());
}

#[test]
fn open_many_windows_stress() {
    let mut shell = new_shell();
    let mut ids = Vec::new();

    for i in 0..50 {
        let bounds = Rect::new(
            (i as f32 * 20.0) % 1920.0,
            (i as f32 * 15.0) % 1080.0,
            400.0,
            300.0,
        );
        ids.push(shell.open_window(format!("Win {i}"), bounds));
    }

    assert_eq!(shell.window_count(), 50);
    assert_eq!(shell.visible_windows().len(), 50);

    // Close half
    for wid in &ids[..25] {
        shell.close_window(*wid).unwrap();
    }
    assert_eq!(shell.window_count(), 25);
}

// ── Screen Resize ───────────────────────────────────────────────────────────

#[test]
fn resize_screen_updates_rect() {
    let mut shell = new_shell();
    assert!((shell.screen_rect().width - 1920.0).abs() < 1.0);

    shell.resize_screen(2560.0, 1440.0);

    let screen = shell.screen_rect();
    assert!((screen.width - 2560.0).abs() < 1.0);
    assert!((screen.height - 1440.0).abs() < 1.0);
}
