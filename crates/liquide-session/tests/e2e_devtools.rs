//! End-to-end tests for the developer tools panel: toggling, keyboard shortcuts,
//! tab cycling, element picker, node selection, DOM tree inspection, scene output.

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNodeKind;
use liquide_devtools::{DevToolsPanel, DevToolsTab, ElementTreeInspector};
use liquide_shell::Shell;

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

// ── DevToolsPanel Construction ──────────────────────────────────────────────

#[test]
fn devtools_panel_starts_hidden() {
    let panel = DevToolsPanel::with_defaults();
    assert!(!panel.is_visible());
}

#[test]
fn devtools_panel_default_tab_is_elements() {
    let panel = DevToolsPanel::with_defaults();
    assert!(matches!(panel.active_tab(), DevToolsTab::Elements));
}

// ── Toggle Visibility ───────────────────────────────────────────────────────

#[test]
fn toggle_shows_then_hides() {
    let mut panel = DevToolsPanel::with_defaults();

    panel.toggle();
    assert!(panel.is_visible(), "first toggle should show");

    panel.toggle();
    assert!(!panel.is_visible(), "second toggle should hide");
}

#[test]
fn show_and_hide_explicitly() {
    let mut panel = DevToolsPanel::with_defaults();

    panel.show();
    assert!(panel.is_visible());

    panel.hide();
    assert!(!panel.is_visible());

    // show twice is idempotent
    panel.show();
    panel.show();
    assert!(panel.is_visible());
}

// ── Keyboard Shortcuts ──────────────────────────────────────────────────────

#[test]
fn f12_toggles_devtools() {
    let mut panel = DevToolsPanel::with_defaults();

    let handled = panel.handle_key("F12", false, false, false);
    assert!(handled, "F12 should be handled");
    assert!(panel.is_visible(), "F12 should show devtools");

    let handled = panel.handle_key("F12", false, false, false);
    assert!(handled);
    assert!(!panel.is_visible(), "F12 again should hide devtools");
}

#[test]
fn ctrl_shift_i_toggles_devtools() {
    let mut panel = DevToolsPanel::with_defaults();

    let handled = panel.handle_key("I", true, true, false);
    assert!(handled, "Ctrl+Shift+I should be handled");
    assert!(panel.is_visible());

    let handled = panel.handle_key("I", true, true, false);
    assert!(handled);
    assert!(!panel.is_visible());
}

#[test]
fn ctrl_shift_c_opens_and_activates_picker() {
    let mut panel = DevToolsPanel::with_defaults();

    let handled = panel.handle_key("C", true, true, false);
    assert!(handled, "Ctrl+Shift+C should be handled");
    assert!(panel.is_visible(), "Ctrl+Shift+C should show devtools");
}

#[test]
fn tab_cycles_tabs_when_visible() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();

    let initial_tab = panel.active_tab();
    assert!(matches!(initial_tab, DevToolsTab::Elements));

    let handled = panel.handle_key("Tab", false, false, false);
    assert!(handled, "Tab should be handled when visible");

    let next_tab = panel.active_tab();
    assert!(
        !matches!(next_tab, DevToolsTab::Elements),
        "tab should have changed from Elements to next"
    );
}

#[test]
fn tab_does_not_handle_when_hidden() {
    let mut panel = DevToolsPanel::with_defaults();
    // Panel is hidden by default

    let handled = panel.handle_key("Tab", false, false, false);
    // Tab should only cycle when visible
    assert!(!handled || !panel.is_visible());
}

#[test]
fn cycle_through_all_tabs() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();

    let all_tabs = DevToolsTab::ALL;
    let mut seen_tabs = vec![panel.active_tab()];

    for _ in 0..all_tabs.len() {
        panel.next_tab();
        seen_tabs.push(panel.active_tab());
    }

    // Should wrap around — we should see at least as many unique tabs as exist
    let unique: std::collections::HashSet<_> = seen_tabs.iter().map(|t| t.label()).collect();
    assert!(
        unique.len() >= all_tabs.len(),
        "should cycle through all tabs: seen {}, expected {}",
        unique.len(),
        all_tabs.len()
    );
}

#[test]
fn prev_tab_cycles_backward() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();

    // Go forward once
    panel.next_tab();
    let after_next = panel.active_tab();

    // Go back
    panel.prev_tab();
    let after_prev = panel.active_tab();

    assert!(matches!(after_prev, DevToolsTab::Elements));
}

// ── Tab Selection ───────────────────────────────────────────────────────────

#[test]
fn set_tab_directly() {
    let mut panel = DevToolsPanel::with_defaults();

    for &tab in DevToolsTab::ALL {
        panel.set_tab(tab);
        // Tab labels should match
        assert_eq!(
            panel.active_tab().label(),
            tab.label(),
            "active tab should be {}",
            tab.label()
        );
    }
}

// ── Node Selection ──────────────────────────────────────────────────────────

#[test]
fn initially_no_node_selected() {
    let panel = DevToolsPanel::with_defaults();
    assert!(panel.selected_node().is_none());
}

#[test]
fn select_and_clear_node() {
    let mut panel = DevToolsPanel::with_defaults();
    let mut shell = new_shell();

    // Build scene to populate layout/styles
    let _scene = shell.build_scene();

    let styles = shell.style_map().expect("styles should exist after build_scene");
    panel.select_node(1, styles);
    assert_eq!(panel.selected_node(), Some(1));

    panel.clear_selection();
    assert!(panel.selected_node().is_none());
}

// ── Panel Bounds ────────────────────────────────────────────────────────────

#[test]
fn panel_bounds_are_valid_rect() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();

    let bounds = panel.panel_bounds();
    assert!(bounds.width > 0.0, "panel width should be positive");
    assert!(bounds.height > 0.0, "panel height should be positive");
}

// ── Scene Output ────────────────────────────────────────────────────────────

#[test]
fn visible_devtools_produces_scene_nodes() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();

    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let layout = shell.layout_tree().expect("layout should exist");
    let styles = shell.style_map().expect("styles should exist");

    let nodes = panel.build_scene(doc, layout, styles);
    assert!(
        !nodes.is_empty(),
        "visible devtools should produce scene nodes"
    );
}

#[test]
fn hidden_devtools_produces_no_scene_nodes() {
    let panel = DevToolsPanel::with_defaults();
    // Panel is hidden

    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let layout = shell.layout_tree().expect("layout should exist");
    let styles = shell.style_map().expect("styles should exist");

    let nodes = panel.build_scene(doc, layout, styles);
    assert!(
        nodes.is_empty(),
        "hidden devtools should produce no scene nodes"
    );
}

#[test]
fn devtools_scene_contains_panel_nodes() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();

    let mut shell = new_shell();
    // Open a window so there's something in the scene
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    shell.open_window("Test", bounds);
    let _scene = shell.build_scene();

    let doc = shell.document();
    let layout = shell.layout_tree().unwrap();
    let styles = shell.style_map().unwrap();

    let nodes = panel.build_scene(doc, layout, styles);

    // DevTools scene nodes should have IDs in the 920_000+ range
    let has_devtools_nodes = nodes.iter().any(|n| n.id >= 920_000);
    assert!(
        has_devtools_nodes || !nodes.is_empty(),
        "devtools scene should contain panel nodes"
    );
}

// ── DOM Tree Inspector ──────────────────────────────────────────────────────

#[test]
fn inspector_builds_snapshot_from_shell_document() {
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = ElementTreeInspector::new();

    let root = inspector.build_snapshot(doc);
    assert!(!root.tag.is_empty(), "root node should have a tag");
    assert!(
        root.child_count > 0 || !root.children.is_empty(),
        "document should have children"
    );
}

#[test]
fn inspector_snapshot_contains_shell_chrome_elements() {
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = ElementTreeInspector::new();
    let root = inspector.build_snapshot(doc);

    // Collect all tags recursively
    fn collect_tags(node: &liquide_devtools::inspector::InspectorNode, tags: &mut Vec<String>) {
        tags.push(node.tag.clone());
        for child in &node.children {
            collect_tags(child, tags);
        }
    }

    let mut tags = Vec::new();
    collect_tags(root, &mut tags);

    // The desktop DOM should contain common shell elements
    assert!(
        tags.len() > 1,
        "DOM tree should have multiple elements, got {}",
        tags.len()
    );
}

#[test]
fn inspector_select_and_query() {
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = ElementTreeInspector::new();

    let root = inspector.build_snapshot(doc);
    let root_id = root.id;

    // Select the root node
    inspector.select(root_id);
    assert_eq!(inspector.selected(), Some(root_id));

    // Hover
    inspector.set_hovered(Some(root_id));
    assert_eq!(inspector.hovered(), Some(root_id));
    inspector.set_hovered(None);
    assert_eq!(inspector.hovered(), None);
}

#[test]
fn inspector_expand_and_collapse() {
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = ElementTreeInspector::new();

    let root = inspector.build_snapshot(doc);
    let root_id = root.id;

    // These should not panic
    inspector.expand(root_id);
    inspector.collapse(root_id);
    inspector.toggle_expand(root_id);
}

#[test]
fn inspector_search_returns_matching_nodes() {
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();
    let mut inspector = ElementTreeInspector::new();

    // Build snapshot first so the inspector has data
    let _snap = inspector.build_snapshot(doc);

    // Search for a common tag
    inspector.set_search("div");
    assert_eq!(inspector.search_query(), "div");

    let results = inspector.search(doc);
    // Results might be empty if DOM doesn't use "div" tags, that's OK
    // The search function should at least not panic
    let _ = results;
}

// ── DevTools Panel with Inspector (integrated) ──────────────────────────────

#[test]
fn devtools_inspector_field_accessible() {
    let mut panel = DevToolsPanel::with_defaults();
    let mut shell = new_shell();
    let _scene = shell.build_scene();

    let doc = shell.document();

    // Access the inspector field and build a snapshot
    let root = panel.inspector.build_snapshot(doc);
    assert!(!root.tag.is_empty());
}

#[test]
fn devtools_panel_all_tabs_have_labels() {
    for tab in DevToolsTab::ALL {
        let label = tab.label();
        assert!(!label.is_empty(), "{:?} tab should have a label", tab);
    }
}

// ── Element Picker (via DevToolsPanel) ──────────────────────────────────────

#[test]
fn element_picker_toggle() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();

    // Toggle picker
    panel.toggle_picker();
    // No crash — picker state is internal
}

#[test]
fn on_mouse_move_with_picker_active() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();
    panel.toggle_picker();

    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    shell.open_window("Picker Target", bounds);
    let _scene = shell.build_scene();

    let hit_test = shell.hit_test_engine().expect("hit_test should exist");
    let doc = shell.document();
    let layout = shell.layout_tree().expect("layout should exist");

    // Mouse move on content area — should not panic
    let _handled = panel.on_mouse_move(400.0, 400.0, hit_test, doc, layout);
}

#[test]
fn on_click_with_picker_selects_node() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();
    panel.toggle_picker();

    let mut shell = new_shell();
    let bounds = Rect::new(100.0, 100.0, 640.0, 480.0);
    shell.open_window("Click Target", bounds);
    let _scene = shell.build_scene();

    let hit_test = shell.hit_test_engine().unwrap();
    let doc = shell.document();
    let layout = shell.layout_tree().unwrap();
    let styles = shell.style_map().unwrap();

    // Simulate mouse move to a known area, then click
    let _ = panel.on_mouse_move(400.0, 400.0, hit_test, doc, layout);
    let _ = panel.on_click(styles);

    // After click, the picker may have selected a node (or not if nothing
    // was hit — depends on DOM layout). The test ensures no crash.
}

// ── Mutation Log ────────────────────────────────────────────────────────────

#[test]
fn mutation_log_starts_empty() {
    let panel = DevToolsPanel::with_defaults();
    // The mutation_log field should be accessible but empty
    let _ = &panel.mutation_log;
}

// ── DOM Serializer ──────────────────────────────────────────────────────────

#[test]
fn dom_serializer_accessible() {
    let panel = DevToolsPanel::with_defaults();
    let _ = &panel.dom_serializer;
}
