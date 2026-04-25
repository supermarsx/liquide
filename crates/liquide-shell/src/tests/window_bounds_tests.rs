//! Tests for window bounding box behavior — work area, maximize, centering,
//! min_size, and interactions with statusbar and dock.

use liquide_compositor::geometry::Rect;

use crate::shell::Shell;
use crate::window::{WindowId, WindowState};

// ========== work_area ==========

#[test]
fn work_area_excludes_statusbar_and_dock() {
    let shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    assert!(work.y > 0.0, "work area should start below statusbar");
    assert!(
        work.height < 1080.0,
        "work area should be shorter than screen"
    );
    assert!(
        work.y + work.height < 1080.0,
        "work area bottom should be above dock"
    );
}

#[test]
fn work_area_y_equals_statusbar_height() {
    let shell = Shell::new(1920.0, 1080.0);
    let bar_h = shell.status_bar.config().height;
    let work = shell.work_area();
    assert_eq!(work.y, bar_h, "work area top should equal statusbar height");
}

#[test]
fn work_area_width_equals_screen_width() {
    let shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    assert_eq!(
        work.width, 1920.0,
        "work area width should equal screen width"
    );
}

#[test]
fn work_area_height_is_screen_minus_bar_and_dock() {
    let shell = Shell::new(1920.0, 1080.0);
    let bar_h = shell.status_bar.config().height;
    let dock_bounds = shell.dock.compute_bounds(shell.screen_rect);
    let dock_h = dock_bounds.height;
    let work = shell.work_area();
    assert_eq!(
        work.height,
        1080.0 - bar_h - dock_h,
        "work area height should be screen height minus bar and dock"
    );
}

#[test]
fn work_area_adjusts_with_screen_resize() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work1 = shell.work_area();
    shell.resize_screen(2560.0, 1440.0);
    let work2 = shell.work_area();
    assert!(work2.width > work1.width);
    assert!(work2.height > work1.height);
}

#[test]
fn work_area_never_negative_height() {
    // Even with a tiny screen, height should be clamped to 0.
    let shell = Shell::new(10.0, 10.0);
    let work = shell.work_area();
    assert!(
        work.height >= 0.0,
        "work area height should never be negative"
    );
}

// ========== maximize ==========

#[test]
fn maximize_respects_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds.x, work.x);
    assert_eq!(win.bounds.y, work.y);
    assert_eq!(win.bounds.width, work.width);
    assert_eq!(win.bounds.height, work.height);
    assert_eq!(win.state, WindowState::Maximized);
}

#[test]
fn maximize_does_not_overlap_statusbar() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let bar_h = shell.status_bar.config().height;
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    let win = shell.window(id).unwrap();
    assert!(
        win.bounds.y >= bar_h,
        "maximized window top ({}) should not overlap statusbar (height {})",
        win.bounds.y,
        bar_h
    );
}

#[test]
fn maximize_does_not_overlap_dock() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    let win = shell.window(id).unwrap();
    let dock_bounds = shell.dock.compute_bounds(shell.screen_rect);
    assert!(
        win.bounds.y + win.bounds.height <= dock_bounds.y,
        "maximized window bottom ({}) should not exceed dock top ({})",
        win.bounds.y + win.bounds.height,
        dock_bounds.y
    );
}

#[test]
fn maximize_then_restore_returns_original_bounds() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let orig = Rect::new(100.0, 200.0, 400.0, 300.0);
    let id = shell.open_window("Test", orig);
    shell.maximize(id).unwrap();
    shell.restore(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds, orig);
    assert_eq!(win.state, WindowState::Normal);
}

#[test]
fn maximize_sets_state_to_maximized() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(50.0, 50.0, 300.0, 200.0));
    shell.maximize(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Maximized);
}

#[test]
fn maximize_nonexistent_window_returns_error() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.maximize(WindowId(999)).is_err());
}

// ========== fullscreen ==========

#[test]
fn fullscreen_uses_full_screen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.toggle_fullscreen(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds.x, 0.0);
    assert_eq!(win.bounds.y, 0.0);
    assert_eq!(win.bounds.width, 1920.0);
    assert_eq!(win.bounds.height, 1080.0);
    assert_eq!(win.state, WindowState::Fullscreen);
}

#[test]
fn fullscreen_covers_statusbar_and_dock() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let bar_h = shell.status_bar.config().height;
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.toggle_fullscreen(id).unwrap();
    let win = shell.window(id).unwrap();
    // Fullscreen should start at 0, covering the statusbar.
    assert_eq!(win.bounds.y, 0.0);
    assert!(
        win.bounds.y < bar_h,
        "fullscreen should cover the statusbar"
    );
    assert_eq!(win.bounds.height, 1080.0);
}

#[test]
fn fullscreen_then_restore_returns_original_bounds() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let orig = Rect::new(150.0, 200.0, 500.0, 400.0);
    let id = shell.open_window("Test", orig);
    shell.toggle_fullscreen(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Fullscreen);
    shell.toggle_fullscreen(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds, orig);
    assert_eq!(win.state, WindowState::Normal);
}

#[test]
fn maximize_is_different_from_fullscreen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id_max = shell.open_window("Max", Rect::new(50.0, 50.0, 300.0, 200.0));
    let id_fs = shell.open_window("FS", Rect::new(50.0, 50.0, 300.0, 200.0));
    shell.maximize(id_max).unwrap();
    shell.toggle_fullscreen(id_fs).unwrap();
    let max_bounds = shell.window(id_max).unwrap().bounds;
    let fs_bounds = shell.window(id_fs).unwrap().bounds;
    // Fullscreen should be larger than maximized (covers statusbar + dock).
    assert!(
        fs_bounds.height > max_bounds.height,
        "fullscreen height ({}) should exceed maximized height ({})",
        fs_bounds.height,
        max_bounds.height
    );
    assert!(
        fs_bounds.y < max_bounds.y,
        "fullscreen y ({}) should be less than maximized y ({})",
        fs_bounds.y,
        max_bounds.y
    );
}

// ========== open_app_window centering ==========

#[test]
fn app_window_centered_in_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    let id = shell.open_app_window("com.liquide.terminal");
    let win = shell.window(id).unwrap();
    // Window should be within work area.
    assert!(
        win.bounds.y >= work.y,
        "window top ({}) should be at or below work area top ({})",
        win.bounds.y,
        work.y
    );
    assert!(
        win.bounds.y + win.bounds.height <= work.y + work.height + 1.0,
        "window bottom ({}) should be within work area bottom ({})",
        win.bounds.y + win.bounds.height,
        work.y + work.height
    );
    // Should be approximately horizontally centered.
    let center_x = win.bounds.x + win.bounds.width / 2.0;
    let work_center_x = work.x + work.width / 2.0;
    assert!(
        (center_x - work_center_x).abs() < 1.0,
        "window should be horizontally centered in work area"
    );
}

#[test]
fn app_window_centered_vertically_in_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    let id = shell.open_app_window("com.liquide.files");
    let win = shell.window(id).unwrap();
    let center_y = win.bounds.y + win.bounds.height / 2.0;
    let work_center_y = work.y + work.height / 2.0;
    assert!(
        (center_y - work_center_y).abs() < 1.0,
        "window center Y ({}) should be close to work area center Y ({})",
        center_y,
        work_center_y
    );
}

#[test]
fn app_window_not_centered_on_full_screen() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let win = shell.window(id).unwrap();
    // The window Y should NOT be centered on the full screen (which would be
    // (1080 - 480) / 2 = 300). It should be offset by the statusbar.
    let full_center_y = (1080.0 - 480.0) / 2.0;
    assert!(
        (win.bounds.y - full_center_y).abs() > 1.0,
        "window should not be centered on full screen (y={}, full_center_y={})",
        win.bounds.y,
        full_center_y
    );
}

// ========== min_size ==========

#[test]
fn app_window_has_min_size() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.calculator");
    let win = shell.window(id).unwrap();
    assert!(
        win.min_size.is_some(),
        "app windows should have min_size set"
    );
}

#[test]
fn calculator_has_specific_min_size() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.calculator");
    let win = shell.window(id).unwrap();
    assert_eq!(win.min_size, Some((280.0, 320.0)));
}

#[test]
fn generic_app_has_default_min_size() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let win = shell.window(id).unwrap();
    assert_eq!(win.min_size, Some((200.0, 150.0)));
}

#[test]
fn manually_created_window_has_no_min_size() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Plain", Rect::new(0.0, 0.0, 300.0, 200.0));
    let win = shell.window(id).unwrap();
    assert_eq!(
        win.min_size, None,
        "manually created windows should not have min_size"
    );
}

#[test]
fn resize_does_not_enforce_min_size() {
    // resize_window is a raw operation that doesn't clamp. This test documents that.
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.window_mut(id).unwrap().min_size = Some((200.0, 150.0));
    shell.resize_window(id, 50.0, 30.0).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds.width, 50.0);
    assert_eq!(win.bounds.height, 30.0);
}

// ========== open_app_window reuse ==========

#[test]
fn open_app_window_reuses_existing() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_app_window("com.liquide.terminal");
    let id2 = shell.open_app_window("com.liquide.terminal");
    assert_eq!(id1, id2, "should reuse existing visible window");
    assert_eq!(shell.window_count(), 1);
}

#[test]
fn open_different_app_windows() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_app_window("com.liquide.terminal");
    let id2 = shell.open_app_window("com.liquide.files");
    assert_ne!(id1, id2);
    assert_eq!(shell.window_count(), 2);
}

// ========== z-order ==========

#[test]
fn window_raise_changes_z_order() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.raise_window(id1).unwrap();
    assert!(
        shell.window(id1).unwrap().z_order > shell.window(id2).unwrap().z_order,
        "raised window should have higher z_order"
    );
}

#[test]
fn window_lower_changes_z_order() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    // Raise id1 first so it's above id2, then lower id2.
    shell.raise_window(id1).unwrap();
    shell.lower_window(id2).unwrap();
    assert!(
        shell.window(id2).unwrap().z_order < shell.window(id1).unwrap().z_order,
        "lowered window should have lower z_order"
    );
}

// ========== dock interaction ==========

#[test]
fn open_app_window_adds_to_dock_running() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let _id = shell.open_app_window("com.liquide.terminal");
    let term_item = shell
        .dock
        .items()
        .iter()
        .find(|i| i.app_id == "com.liquide.terminal");
    assert!(term_item.is_some(), "dock should have the terminal entry");
    assert!(
        term_item.unwrap().running_window_count > 0,
        "terminal dock item should have running_window_count > 0"
    );
}

#[test]
fn window_close_decrements_dock_running() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.terminal");
    shell.close_window(id).unwrap();
    // After closing, dock running count should be 0 (item stays because it's pinned).
    let term_item = shell
        .dock
        .items()
        .iter()
        .find(|i| i.app_id == "com.liquide.terminal");
    if let Some(item) = term_item {
        assert_eq!(
            item.running_window_count, 0,
            "after close, running count should be 0"
        );
    }
}

// ========== multiple maximize/restore cycles ==========

#[test]
fn double_maximize_is_idempotent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let orig = Rect::new(100.0, 200.0, 400.0, 300.0);
    let id = shell.open_window("Test", orig);
    shell.maximize(id).unwrap();
    let bounds_after_first = shell.window(id).unwrap().bounds;
    shell.maximize(id).unwrap();
    let bounds_after_second = shell.window(id).unwrap().bounds;
    assert_eq!(bounds_after_first, bounds_after_second);
}

#[test]
fn maximize_after_minimize_uses_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let work = shell.work_area();
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.minimize(id).unwrap();
    shell.maximize(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds, work);
    assert_eq!(win.state, WindowState::Maximized);
}

// ========== edge cases ==========

#[test]
fn work_area_with_zero_screen() {
    let shell = Shell::new(0.0, 0.0);
    let work = shell.work_area();
    assert_eq!(work.height, 0.0);
    assert_eq!(work.width, 0.0);
}

#[test]
fn maximize_on_small_screen() {
    let mut shell = Shell::new(200.0, 100.0);
    let work = shell.work_area();
    let id = shell.open_window("Test", Rect::new(10.0, 10.0, 50.0, 50.0));
    shell.maximize(id).unwrap();
    let win = shell.window(id).unwrap();
    assert_eq!(win.bounds.x, work.x);
    assert_eq!(win.bounds.y, work.y);
    assert_eq!(win.bounds.width, work.width);
    assert_eq!(win.bounds.height, work.height);
}

#[test]
fn all_known_apps_get_min_size() {
    let apps = [
        "com.liquide.settings",
        "com.liquide.terminal",
        "com.liquide.files",
        "com.liquide.browser",
        "com.liquide.calculator",
    ];
    for app in &apps {
        let mut s = Shell::new(1920.0, 1080.0);
        let id = s.open_app_window(app);
        let win = s.window(id).unwrap();
        assert!(
            win.min_size.is_some(),
            "app {} should have min_size set",
            app
        );
    }
}
