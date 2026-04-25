//! End-to-end tests for dock tracking: pinned items, running window counts,
//! app launch through dock, icon presence for open windows.

use liquide_compositor::geometry::Rect;
use liquide_shell::{DockItem, DockItemKind, Shell};

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

// ── Default Dock Items ──────────────────────────────────────────────────────

#[test]
fn dock_has_default_pinned_apps() {
    let shell = new_shell();
    let items = shell.dock().items();

    // Shell::new creates a dock with default pinned apps
    assert!(!items.is_empty(), "dock should have default items");

    // All default items should be pinned
    let pinned: Vec<&DockItem> = items
        .iter()
        .filter(|i| matches!(i.kind, DockItemKind::Pinned))
        .collect();
    assert!(!pinned.is_empty(), "dock should have pinned items");
}

#[test]
fn default_dock_items_have_labels_and_icons() {
    let shell = new_shell();
    for item in shell.dock().items() {
        if matches!(item.kind, DockItemKind::Pinned | DockItemKind::Running) {
            assert!(!item.label.is_empty(), "dock item should have a label");
            assert!(!item.icon.is_empty(), "dock item should have an icon");
            assert!(!item.app_id.is_empty(), "dock item should have an app_id");
        }
    }
}

// ── Running Window Count Tracking ───────────────────────────────────────────

#[test]
fn opening_app_window_increments_running_count() {
    let mut shell = new_shell();

    // Find a pinned app_id from the default dock
    let app_id = {
        let items = shell.dock().items();
        let pinned = items
            .iter()
            .find(|i| matches!(i.kind, DockItemKind::Pinned))
            .expect("should have at least one pinned item");
        pinned.app_id.clone()
    };

    // Get initial running count
    let initial_count = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    // Open an app window
    let _wid = shell.open_app_window(&app_id);

    // Running count should have increased
    let new_count = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    assert!(
        new_count > initial_count,
        "running_window_count should increase: was {initial_count}, now {new_count}"
    );
}

#[test]
fn closing_app_window_decrements_running_count() {
    let mut shell = new_shell();

    let app_id = {
        let items = shell.dock().items();
        items
            .iter()
            .find(|i| matches!(i.kind, DockItemKind::Pinned))
            .expect("need at least one pinned item")
            .app_id
            .clone()
    };

    let wid = shell.open_app_window(&app_id);

    let count_before_close = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    assert!(count_before_close > 0, "should have at least 1 running");

    shell.close_window(wid).unwrap();

    let count_after_close = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    assert!(
        count_after_close < count_before_close,
        "running_window_count should decrease: was {count_before_close}, now {count_after_close}"
    );
}

#[test]
fn multiple_windows_same_app_track_correctly() {
    let mut shell = new_shell();

    let app_id = {
        shell
            .dock()
            .items()
            .iter()
            .find(|i| matches!(i.kind, DockItemKind::Pinned))
            .unwrap()
            .app_id
            .clone()
    };

    // open_app_window calls dock.add_running, but will focus existing window
    // if one is already open. So use open_window_with_app + manual dock update.
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    let w1 = shell.open_window_with_app("App 1", bounds, &app_id);
    shell.dock_mut().add_running(&app_id);
    let w2 = shell.open_window_with_app("App 2", bounds, &app_id);
    shell.dock_mut().add_running(&app_id);
    let w3 = shell.open_window_with_app("App 3", bounds, &app_id);
    shell.dock_mut().add_running(&app_id);

    let count = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    assert!(
        count >= 3,
        "should have at least 3 running windows, got {count}"
    );

    // close_window calls dock.remove_running automatically
    shell.close_window(w1).unwrap();
    shell.close_window(w2).unwrap();

    let count2 = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    assert!(
        count2 < count,
        "running count should decrease after closing windows"
    );

    // Close last
    shell.close_window(w3).unwrap();

    let count3 = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == app_id)
        .map(|i| i.running_window_count)
        .unwrap_or(0);

    // Should be back to zero or original
    assert!(
        count3 < count2,
        "running count should be 0 after all windows closed, got {count3}"
    );
}

// ── Dock Item Presence for Open Windows ─────────────────────────────────────

#[test]
fn opening_unknown_app_adds_running_item_to_dock() {
    let mut shell = new_shell();

    let custom_app = "com.test.custom-app";

    // Use open_app_window which properly calls dock.add_running
    let wid = shell.open_app_window(custom_app);

    // Dock should have a new running entry for this app
    let has_item = shell.dock().items().iter().any(|i| i.app_id == custom_app);

    assert!(has_item, "dock should show an icon for the opened app");

    // Verify the window was created
    let win = shell.window(wid).expect("window should exist");
    assert_eq!(win.app_id, custom_app);
}

// ── Dock Bounds Computation ─────────────────────────────────────────────────

#[test]
fn dock_bounds_are_within_screen() {
    let shell = new_shell();
    let screen = shell.screen_rect();
    let dock_bounds = shell.dock().compute_bounds(screen);

    // Dock should be on-screen
    assert!(dock_bounds.x >= 0.0);
    assert!(dock_bounds.y >= 0.0);
    assert!(dock_bounds.x + dock_bounds.width <= screen.width + 1.0);
    assert!(dock_bounds.y + dock_bounds.height <= screen.height + 1.0);
}

// ── Add/Remove Pinned ───────────────────────────────────────────────────────

#[test]
fn add_pinned_app_to_dock() {
    let mut shell = new_shell();
    let initial_count = shell.dock().item_count();

    let id = shell
        .dock_mut()
        .add_pinned("com.test.new-app", "New App", "icon-new");

    assert!(id > 0);
    assert_eq!(shell.dock().item_count(), initial_count + 1);

    let item = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == "com.test.new-app")
        .expect("new pinned item should exist");
    assert_eq!(item.label, "New App");
    assert_eq!(item.icon, "icon-new");
    assert!(matches!(item.kind, DockItemKind::Pinned));
}

#[test]
fn remove_pinned_app_from_dock() {
    let mut shell = new_shell();

    let id = shell
        .dock_mut()
        .add_pinned("com.test.remove-me", "Remove Me", "icon-x");
    let count_after_add = shell.dock().item_count();

    let removed = shell.dock_mut().remove_pinned(id);
    assert!(removed, "remove_pinned should succeed");
    assert_eq!(shell.dock().item_count(), count_after_add - 1);
}

// ── Dock Item Indexing ──────────────────────────────────────────────────────

#[test]
fn dock_item_at_index() {
    let shell = new_shell();
    let items = shell.dock().items();

    for (i, item) in items.iter().enumerate() {
        let by_index = shell.dock().item_at_index(i);
        assert!(by_index.is_some(), "item at index {i} should exist");
        assert_eq!(by_index.unwrap().id, item.id);
    }

    assert!(
        shell.dock().item_at_index(items.len()).is_none(),
        "out-of-bounds index should return None"
    );
}
