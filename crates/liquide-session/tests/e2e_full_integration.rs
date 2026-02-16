//! Full integration tests exercising the complete flow: open app windows,
//! verify dock tracking, toggle devtools, inspect DOM tree, verify scene output,
//! and check window/devtools co-existence.

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNodeKind;
use liquide_devtools::{DevToolsPanel, DevToolsTab};
use liquide_shell::{Shell, ShellAction, WindowState};

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

// ── Full Workflow: Open App, Check Dock, Open DevTools, Inspect ─────────────

#[test]
fn full_workflow_app_window_dock_devtools() {
    let mut shell = new_shell();
    let mut devtools = DevToolsPanel::with_defaults();
    devtools.set_screen_size(1920.0, 1080.0);

    // --- Step 1: Open an app window ---
    let wid = shell.open_app_window("com.liquide.terminal");
    let win = shell.window(wid).expect("terminal window should exist");
    assert_eq!(win.app_id, "com.liquide.terminal");
    assert!(win.visible);

    // --- Step 2: Verify dock shows running indicator ---
    let dock_item = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == "com.liquide.terminal");
    assert!(dock_item.is_some(), "dock should have terminal item");
    assert!(
        dock_item.unwrap().running_window_count > 0,
        "dock should show running count for terminal"
    );

    // --- Step 3: Build scene and verify window is rendered ---
    let scene = shell.build_scene();
    let flat = scene.flatten();
    let w_base = 10_000 + wid.0 * 10;
    assert!(
        flat.iter().any(|n| n.id >= w_base && n.id < w_base + 10),
        "scene should contain nodes for the terminal window"
    );

    // --- Step 4: Open DevTools ---
    devtools.handle_key("F12", false, false, false);
    assert!(devtools.is_visible(), "devtools should be visible after F12");

    // --- Step 5: Inspect DOM tree ---
    let doc = shell.document();
    let root = devtools.inspector.build_snapshot(doc);
    assert!(!root.tag.is_empty(), "DOM root should have a tag");
    assert!(
        root.child_count > 0 || !root.children.is_empty(),
        "DOM tree should have children"
    );

    // --- Step 6: DevTools scene nodes should be generated ---
    let layout = shell.layout_tree().expect("layout after build_scene");
    let styles = shell.style_map().expect("styles after build_scene");
    let dt_nodes = devtools.build_scene(doc, layout, styles);
    assert!(
        !dt_nodes.is_empty(),
        "visible devtools should produce scene nodes"
    );

    // --- Step 7: Close window, verify dock updates ---
    shell.close_window(wid).unwrap();
    let dock_item_after = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == "com.liquide.terminal");
    if let Some(item) = dock_item_after {
        assert_eq!(
            item.running_window_count, 0,
            "dock running count should be 0 after closing window"
        );
    }
}

// ── Multiple Apps with Dock Tracking ────────────────────────────────────────

#[test]
fn multiple_apps_tracked_in_dock() {
    let mut shell = new_shell();

    // Open windows for different apps using open_app_window which updates dock
    let w_terminal = shell.open_app_window("com.liquide.terminal");
    let w_browser = shell.open_app_window("com.liquide.browser");
    let w_files = shell.open_app_window("com.liquide.files");

    assert_eq!(shell.window_count(), 3);

    // Each app's dock running count should be > 0
    for app_id in &["com.liquide.terminal", "com.liquide.browser", "com.liquide.files"] {
        let item = shell.dock().items().iter().find(|i| i.app_id == *app_id);
        assert!(
            item.is_some(),
            "dock should have an item for {app_id}"
        );
        assert!(
            item.unwrap().running_window_count > 0,
            "dock running count for {app_id} should be > 0"
        );
    }

    // Scene should have all windows
    let scene = shell.build_scene();
    let flat = scene.flatten();
    for wid in [w_terminal, w_browser, w_files] {
        let base = 10_000 + wid.0 * 10;
        let has = flat.iter().any(|n| n.id >= base && n.id < base + 10);
        assert!(has, "scene should have nodes for window {:?}", wid);
    }

    // Close one, verify dock updates
    shell.close_window(w_browser).unwrap();
    let browser_item = shell
        .dock()
        .items()
        .iter()
        .find(|i| i.app_id == "com.liquide.browser");
    if let Some(item) = browser_item {
        assert_eq!(item.running_window_count, 0);
    }

    // Other windows should still be alive
    assert_eq!(shell.window_count(), 2);
}

// ── DevTools Navigation Flow ────────────────────────────────────────────────

#[test]
fn devtools_full_navigation_flow() {
    let mut devtools = DevToolsPanel::with_defaults();

    // Start hidden
    assert!(!devtools.is_visible());

    // F12 opens
    devtools.handle_key("F12", false, false, false);
    assert!(devtools.is_visible());
    assert!(matches!(devtools.active_tab(), DevToolsTab::Elements));

    // Tab cycles to next
    devtools.handle_key("Tab", false, false, false);
    let tab_after_1 = devtools.active_tab();
    assert!(
        !matches!(tab_after_1, DevToolsTab::Elements),
        "Tab should move to next tab"
    );

    // Continue cycling through all tabs
    for _ in 0..4 {
        devtools.handle_key("Tab", false, false, false);
    }
    // After cycling through all, should be back to Elements (or wrapping)
    // Just ensure no panics and tab changed

    // Ctrl+Shift+C opens picker
    devtools.handle_key("C", true, true, false);
    // Panel should still be visible
    assert!(devtools.is_visible());

    // Set a specific tab
    devtools.set_tab(DevToolsTab::DomTree);
    assert!(matches!(devtools.active_tab(), DevToolsTab::DomTree));

    devtools.set_tab(DevToolsTab::Styles);
    assert!(matches!(devtools.active_tab(), DevToolsTab::Styles));

    devtools.set_tab(DevToolsTab::Layout);
    assert!(matches!(devtools.active_tab(), DevToolsTab::Layout));

    devtools.set_tab(DevToolsTab::Mutations);
    assert!(matches!(devtools.active_tab(), DevToolsTab::Mutations));

    // F12 closes
    devtools.handle_key("F12", false, false, false);
    assert!(!devtools.is_visible());
}

// ── Window Tiling with DevTools Open ────────────────────────────────────────

#[test]
fn tiling_works_while_devtools_visible() {
    let mut shell = new_shell();
    let mut devtools = DevToolsPanel::with_defaults();
    devtools.set_screen_size(1920.0, 1080.0);

    // Open two windows
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let w1 = shell.open_window("Left Window", bounds);
    let w2 = shell.open_window("Right Window", bounds);

    // Open devtools
    devtools.show();
    assert!(devtools.is_visible());

    // Tile windows
    shell.set_focus(w1).unwrap();
    shell.execute_action(&ShellAction::TileLeft);
    shell.set_focus(w2).unwrap();
    shell.execute_action(&ShellAction::TileRight);

    // Windows should still be properly tiled
    let win1 = shell.window(w1).unwrap();
    let win2 = shell.window(w2).unwrap();
    let screen = shell.screen_rect();

    assert!(
        win1.bounds.x < screen.width / 2.0,
        "w1 should be on left half"
    );
    assert!(
        win2.bounds.x >= screen.width / 2.0 - 2.0,
        "w2 should be on right half"
    );

    // Scene should have both window and devtools nodes
    let scene = shell.build_scene();
    let doc = shell.document();
    let layout = shell.layout_tree().unwrap();
    let styles = shell.style_map().unwrap();

    let dt_nodes = devtools.build_scene(doc, layout, styles);
    assert!(
        !dt_nodes.is_empty(),
        "devtools should still produce scene nodes while windows are tiled"
    );
}

// ── Window Lifecycle with Scene Verification ────────────────────────────────

#[test]
fn window_lifecycle_reflected_in_scene() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);

    // Phase 1: Open
    let wid = shell.open_window("Lifecycle", bounds);
    let scene1 = shell.build_scene();
    let flat1 = scene1.flatten();
    let w_base = 10_000 + wid.0 * 10;
    assert!(flat1.iter().any(|n| n.id >= w_base && n.id < w_base + 10));

    // Phase 2: Maximize
    shell.maximize(wid).unwrap();
    let scene2 = shell.build_scene();
    let flat2 = scene2.flatten();
    // Window should still be in scene, perhaps with different bounds
    assert!(flat2.iter().any(|n| n.id >= w_base && n.id < w_base + 10));

    // Phase 3: Minimize (removes from visible)
    shell.minimize(wid).unwrap();
    let scene3 = shell.build_scene();
    let flat3 = scene3.flatten();
    assert!(!flat3.iter().any(|n| n.id >= w_base && n.id < w_base + 10));

    // Phase 4: Restore
    shell.restore(wid).unwrap();
    let scene4 = shell.build_scene();
    let flat4 = scene4.flatten();
    assert!(flat4.iter().any(|n| n.id >= w_base && n.id < w_base + 10));

    // Phase 5: Close
    shell.close_window(wid).unwrap();
    let scene5 = shell.build_scene();
    let flat5 = scene5.flatten();
    assert!(!flat5.iter().any(|n| n.id >= w_base && n.id < w_base + 10));
}

// ── Focus and Z-order in Scene ──────────────────────────────────────────────

#[test]
fn focused_window_has_higher_z_in_scene() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let w1 = shell.open_window("Background", bounds);
    let w2 = shell.open_window("Foreground", bounds);

    shell.set_focus(w2).unwrap();
    let scene = shell.build_scene();
    let flat = scene.flatten();

    // Find decoration nodes for both windows
    let w1_base = 10_000 + w1.0 * 10;
    let w2_base = 10_000 + w2.0 * 10;

    let w1_z = flat
        .iter()
        .filter(|n| n.id >= w1_base && n.id < w1_base + 10)
        .map(|n| n.z_order)
        .max();

    let w2_z = flat
        .iter()
        .filter(|n| n.id >= w2_base && n.id < w2_base + 10)
        .map(|n| n.z_order)
        .max();

    if let (Some(z1), Some(z2)) = (w1_z, w2_z) {
        assert!(
            z2 >= z1,
            "focused window should have >= z_order than background: z1={z1}, z2={z2}"
        );
    }
}

// ── DOM Inspector Reflects Window State ─────────────────────────────────────

#[test]
fn dom_tree_has_content_after_app_window_open() {
    let mut shell = new_shell();
    let _wid = shell.open_app_window("com.liquide.terminal");
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = liquide_devtools::ElementTreeInspector::new();
    let root = inspector.build_snapshot(doc);

    // Count all nodes recursively
    fn count_nodes(node: &liquide_devtools::inspector::InspectorNode) -> usize {
        1 + node.children.iter().map(|c| count_nodes(c)).sum::<usize>()
    }

    let total = count_nodes(root);
    assert!(
        total > 1,
        "DOM tree should have multiple nodes after opening a window, got {total}"
    );
}

// ── DevTools Node Selection from Inspector ──────────────────────────────────

#[test]
fn select_inspector_node_updates_devtools() {
    let mut shell = new_shell();
    let _wid = shell.open_app_window("com.liquide.terminal");
    let _scene = shell.build_scene();

    let mut devtools = DevToolsPanel::with_defaults();
    devtools.set_screen_size(1920.0, 1080.0);
    devtools.show();

    let doc = shell.document();
    let root = devtools.inspector.build_snapshot(doc);

    // Select the root node via inspector
    let root_id = root.id;
    devtools.inspector.select(root_id);
    assert_eq!(devtools.inspector.selected(), Some(root_id));

    // Also select via panel
    let styles = shell.style_map().unwrap();
    devtools.select_node(root_id, styles);
    assert_eq!(devtools.selected_node(), Some(root_id));
}

// ── Scene Output Consistency ────────────────────────────────────────────────

#[test]
fn scene_flattening_includes_all_visible_window_types() {
    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);

    // Open different types of windows
    let w_normal = shell.open_window("Normal", bounds);
    let w_app = shell.open_app_window("com.liquide.terminal");

    let scene = shell.build_scene();
    let flat = scene.flatten();

    // Count different node kinds
    let shadows = flat
        .iter()
        .filter(|n| matches!(n.kind, SceneNodeKind::Shadow { .. }))
        .count();
    let decorations = flat
        .iter()
        .filter(|n| matches!(n.kind, SceneNodeKind::Decoration { .. }))
        .count();
    let backgrounds = flat
        .iter()
        .filter(|n| matches!(n.kind, SceneNodeKind::Background { .. }))
        .count();

    assert!(shadows >= 2, "should have shadows for both windows, got {shadows}");
    assert!(
        decorations >= 2,
        "should have decorations for both windows, got {decorations}"
    );
    assert!(
        backgrounds > 0,
        "should have background nodes, got {backgrounds}"
    );
}

// ── Screen Resize with Windows and DevTools ─────────────────────────────────

#[test]
fn screen_resize_updates_all_components() {
    let mut shell = new_shell();
    let bounds = Rect::new(200.0, 200.0, 640.0, 480.0);
    let _wid = shell.open_window("Resize Test", bounds);

    let mut devtools = DevToolsPanel::with_defaults();
    devtools.set_screen_size(1920.0, 1080.0);
    devtools.show();

    // Resize screen
    shell.resize_screen(2560.0, 1440.0);
    devtools.set_screen_size(2560.0, 1440.0);

    let screen = shell.screen_rect();
    assert!((screen.width - 2560.0).abs() < 1.0);
    assert!((screen.height - 1440.0).abs() < 1.0);

    // DevTools panel bounds should reflect new size
    let panel_bounds = devtools.panel_bounds();
    assert!(
        panel_bounds.width > 0.0 && panel_bounds.height > 0.0,
        "devtools panel should have valid bounds after resize"
    );

    // Scene should still build correctly
    let scene = shell.build_scene();
    assert!(
        matches!(scene.kind, SceneNodeKind::Root),
        "scene should still have Root node after resize"
    );
}

// ── Open Window, Verify Scene Text Nodes ────────────────────────────────────

#[test]
fn window_title_appears_in_decoration_nodes() {
    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    let _wid = shell.open_window("My Custom Title", bounds);

    let scene = shell.build_scene();
    let flat = scene.flatten();

    let decoration = flat.iter().find(|n| {
        if let SceneNodeKind::Decoration { title, .. } = &n.kind {
            title.as_deref() == Some("My Custom Title")
        } else {
            false
        }
    });

    assert!(
        decoration.is_some(),
        "scene should contain a Decoration node with the window title"
    );
}
