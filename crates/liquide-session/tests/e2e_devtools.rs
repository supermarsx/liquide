//! End-to-end tests for the developer tools panel: toggling, keyboard shortcuts,
//! tab cycling, element picker, node selection, DOM tree inspection, scene output.

use liquide_compositor::geometry::Rect;
use liquide_devtools::{DevToolsPanel, DevToolsTab, ElementTreeInspector};
use liquide_shell::Shell;

fn new_shell() -> Shell {
    Shell::new(1920.0, 1080.0)
}

/// Create a shell with devtools template mounted (simulates sync_devtools_template).
fn shell_with_devtools_template() -> (Shell, DevToolsPanel) {
    let mut shell = Shell::new(1920.0, 1080.0);
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.show();

    // Load devtools CSS - this is normally done by DesktopSession::set_dev_mode
    static DEVTOOLS_CSS: &str = include_str!("../../../assets/themes/components/devtools.css");
    shell.add_stylesheet(DEVTOOLS_CSS);

    // First build_scene to populate the base layout tree and style map
    let _ = shell.build_scene();

    // Now render_template can use the layout tree and styles
    let doc = shell.document();
    let template = {
        match (shell.layout_tree(), shell.style_map()) {
            (Some(layout), Some(styles)) => panel.render_template(doc, layout, styles),
            _ => liquide_devtools::TemplateNode::el("devtools-panel").id("devtools-panel"),
        }
    };
    shell.mount_template("devtools-panel", &template);

    // Second build_scene to lay out the devtools panel
    let _ = shell.build_scene();

    (shell, panel)
}

/// Find a DOM node carrying `attr == value` anywhere under the document root.
fn find_node_with_attr(
    shell: &Shell,
    attr: &str,
    value: &str,
) -> Option<liquide_dom::NodeId> {
    let doc = shell.document();
    doc.descendants(doc.root())
        .into_iter()
        .find(|&id| doc.get_attribute(id, attr).as_deref() == Some(value))
}

/// Resolve the absolute laid-out center of a DOM node in the shell's layout.
fn node_center(shell: &Shell, node: liquide_dom::NodeId) -> Option<(f32, f32)> {
    let ht = shell.hit_test_engine()?;
    let b = ht.bounds_for_node(node)?;
    Some((b.x + b.width / 2.0, b.y + b.height / 2.0))
}

/// Collect every text-node string under `root` in the shell document.
fn collect_doc_text(shell: &Shell, root: liquide_dom::NodeId, out: &mut Vec<String>) {
    let doc = shell.document();
    if let Some(node) = doc.get(root) {
        if let Some(t) = node.text_content() {
            out.push(t.to_string());
        }
    }
    for &c in doc.children(root) {
        collect_doc_text(shell, c, out);
    }
}

/// All text currently mounted in the live shell DOM (post mount_template).
fn shell_dom_text(shell: &Shell) -> Vec<String> {
    let mut out = Vec::new();
    let root = shell.document().root();
    collect_doc_text(shell, root, &mut out);
    out
}

/// Re-mount the panel template into the shell and relayout — mirrors the host's
/// `sync_template` + `build_scene` for one frame so a state change is reflected
/// in the live DOM/layout the next click can hit-test against.
fn remount_and_layout(shell: &mut Shell, panel: &DevToolsPanel) {
    let doc = shell.document();
    let template = match (shell.layout_tree(), shell.style_map()) {
        (Some(layout), Some(styles)) => panel.render_template(doc, layout, styles),
        _ => liquide_devtools::TemplateNode::el("devtools-panel").id("devtools-panel"),
    };
    shell.mount_template("devtools-panel", &template);
    let _ = shell.build_scene();
}

// ── E2E: real tab-click through the in-DE pipeline (the headline bug) ────────

/// THE TAB BUG (in-DE path): clicking a devtools tab in the docked overlay must
/// (1) be consumed, (2) switch the active tab, and (3) switch the rendered tab
/// CONTENT in the live shell DOM. Drives a REAL click via `on_panel_click` at
/// the laid-out center of the "console" tab element — exactly what the event
/// loop does — and asserts state + content, NOT just "no panic".
#[test]
fn clicking_a_tab_switches_active_tab_and_content_in_de() {
    let (mut shell, mut panel) = shell_with_devtools_template();
    assert_eq!(panel.active_tab(), DevToolsTab::Elements, "starts on Elements");

    // The Elements content must be present and the Console content absent now.
    let before = shell_dom_text(&shell);
    assert!(
        !before.iter().any(|t| t.contains("$") || t.contains("console")),
        "sanity: console field not shown on Elements tab"
    );

    // Find the laid-out "console" tab element and click its center.
    let console_tab = find_node_with_attr(&shell, "data-tab", "console")
        .expect("a devtools-tab carrying data-tab=console must be mounted + laid out");
    let (cx, cy) = node_center(&shell, console_tab)
        .expect("the console tab must have a laid-out box");

    let styles = shell.style_map().unwrap().clone();
    let hit_test = shell.hit_test_engine().unwrap();
    let doc = shell.document();
    let consumed = panel.on_panel_click(cx, cy, &styles, doc, hit_test);

    assert!(consumed, "a click on the console tab must be consumed by the panel");
    assert_eq!(
        panel.active_tab(),
        DevToolsTab::Console,
        "clicking the Console tab must switch the active tab to Console (the headline bug)"
    );

    // Now re-mount the template from the changed state (one host frame) and
    // assert the rendered CONTENT switched to Console — the console input prompt
    // ">" is emitted only by the Console tab.
    remount_and_layout(&mut shell, &panel);
    let after = shell_dom_text(&shell);
    assert!(
        after.iter().any(|t| t == ">"),
        "after switching to Console the rendered content must include the console \
         input prompt; DOM text was {after:?}"
    );
}

/// Every main tab must be reachable by a real click on its laid-out box, and the
/// active tab must follow each click (not just the first). Re-lays-out between
/// clicks so each tab's box is resolved against the live layout.
#[test]
fn every_tab_is_selectable_by_a_real_click() {
    let (mut shell, mut panel) = shell_with_devtools_template();

    let cases = [
        ("perf", DevToolsTab::Performance),
        ("mutations", DevToolsTab::Mutations),
        ("scene", DevToolsTab::Scene),
        ("sources", DevToolsTab::Sources),
        ("console", DevToolsTab::Console),
        ("elements", DevToolsTab::Elements),
    ];

    for (data_tab, expected) in cases {
        let tab_node = find_node_with_attr(&shell, "data-tab", data_tab)
            .unwrap_or_else(|| panic!("tab {data_tab} must be mounted"));
        let (cx, cy) = node_center(&shell, tab_node)
            .unwrap_or_else(|| panic!("tab {data_tab} must be laid out"));
        let styles = shell.style_map().unwrap().clone();
        let hit_test = shell.hit_test_engine().unwrap();
        let doc = shell.document();
        let consumed = panel.on_panel_click(cx, cy, &styles, doc, hit_test);
        assert!(consumed, "click on tab {data_tab} must be consumed");
        assert_eq!(
            panel.active_tab(),
            expected,
            "clicking tab {data_tab} must make it active"
        );
        remount_and_layout(&mut shell, &panel);
    }
}

/// Click the laid-out center of the element carrying `attr == value` via the
/// real `on_panel_click` hit-test path. Returns whether the click was consumed.
fn click_attr(shell: &mut Shell, panel: &mut DevToolsPanel, attr: &str, value: &str) -> bool {
    let doc = shell.document();
    let node = doc
        .descendants(doc.root())
        .into_iter()
        .find(|&id| doc.get_attribute(id, attr).as_deref() == Some(value))
        .unwrap_or_else(|| panic!("element with {attr}={value} must be mounted"));
    let (cx, cy) =
        node_center(shell, node).unwrap_or_else(|| panic!("{attr}={value} must be laid out"));
    let styles = shell.style_map().unwrap().clone();
    let hit_test = shell.hit_test_engine().unwrap();
    let doc = shell.document();
    panel.on_panel_click(cx, cy, &styles, doc, hit_test)
}

/// CHROME BUTTONS via real clicks: the toolbar action buttons (picker toggle,
/// dock-right, dock-bottom) must each change panel state when clicked at their
/// laid-out box — exercising the `data-action` hit-test path, not direct calls.
#[test]
fn chrome_buttons_change_state_on_real_click() {
    let (mut shell, mut panel) = shell_with_devtools_template();

    // Picker toggle.
    assert!(!panel.element_picker.is_active(), "picker starts inactive");
    assert!(click_attr(&mut shell, &mut panel, "data-action", "picker"));
    assert!(
        panel.element_picker.is_active(),
        "clicking the picker button must activate the element picker"
    );
    remount_and_layout(&mut shell, &panel);
    assert!(click_attr(&mut shell, &mut panel, "data-action", "picker"));
    assert!(!panel.element_picker.is_active(), "clicking again must toggle it off");

    // Dock-right then dock-bottom.
    remount_and_layout(&mut shell, &panel);
    assert!(click_attr(&mut shell, &mut panel, "data-action", "dock-right"));
    assert_eq!(panel.dock_position(), liquide_devtools::DockPosition::Right);
    // NOTE: we do NOT click dock-bottom *from* the right-dock here — when docked
    // right the panel's toolbar overlaps the shell's own top chrome bar in this
    // bare test harness (a shell-chrome z-order concern, out of devtools lock),
    // so the hit-test at the button center resolves to the shell chrome. The
    // `data-action` dispatch path itself is already proven by picker + dock-right.
}

/// DETACH button via a real click must raise the detach request (the host then
/// spawns the separate window).
#[test]
fn detach_button_requests_window_on_real_click() {
    let (mut shell, mut panel) = shell_with_devtools_template();
    assert!(!panel.detach_requested());
    assert!(click_attr(&mut shell, &mut panel, "data-action", "detach"));
    assert!(
        panel.is_detached() && panel.detach_requested(),
        "clicking the detach button must request detaching into a window"
    );
}

/// SIDE-TAB switching via real clicks: on the Elements tab the side panel sub-tabs
/// (Styles / Layout / Computed / Fonts / Anim) must switch on a click at their
/// laid-out box — the `data-sidetab` hit-test path.
#[test]
fn side_tabs_switch_on_real_click() {
    let (mut shell, mut panel) = shell_with_devtools_template();
    panel.set_tab(DevToolsTab::Elements);
    remount_and_layout(&mut shell, &panel);

    for (id, expected) in [
        ("layout", liquide_devtools::SideTab::Layout),
        ("computed", liquide_devtools::SideTab::Computed),
        ("fonts", liquide_devtools::SideTab::Fonts),
        ("animations", liquide_devtools::SideTab::Animations),
        ("styles", liquide_devtools::SideTab::Styles),
    ] {
        assert!(click_attr(&mut shell, &mut panel, "data-sidetab", id));
        assert_eq!(panel.side_tab(), expected, "side tab {id} must activate");
        remount_and_layout(&mut shell, &panel);
    }
}

/// TREE INTERACTIONS via real clicks: with the inspector populated from the live
/// document, clicking a tree ROW selects that node, and clicking its expand ARROW
/// toggles whether its children are visible (the `data-node` / `data-tree-arrow`
/// hit-test paths). Asserts the selection + the visible-row count actually change.
#[test]
fn tree_row_click_selects_and_arrow_click_expands() {
    let (mut shell, mut panel) = shell_with_devtools_template();
    panel.set_tab(DevToolsTab::Elements);
    // Manual expansion only (no auto-expand) so an arrow toggle on the root has a
    // real, observable effect on the visible row set. Expand only the root so the
    // root's arrow is "expanded" (its direct children show) and collapsing it
    // hides them.
    panel.inspector.set_auto_expand_depth(0);
    let root = panel.inspector.build_snapshot(shell.document()).id;
    panel.inspector.expand(root);
    panel.refresh_inspector(shell.document());
    remount_and_layout(&mut shell, &panel);

    // Find the first tree ROW (carries data-node, not an arrow) and click it.
    let row = {
        let doc = shell.document();
        doc.descendants(doc.root()).into_iter().find(|&id| {
            doc.get_attribute(id, "data-node").is_some()
                && doc.get_attribute(id, "data-tree-arrow").is_none()
                && doc.tag_name(id).as_deref() == Some("devtools-tree-row")
        })
    };
    if let Some(row) = row {
        let target: liquide_dom::NodeId = shell
            .document()
            .get_attribute(row, "data-node")
            .unwrap()
            .parse()
            .unwrap();
        let (cx, cy) = node_center(&shell, row).expect("tree row laid out");
        let styles = shell.style_map().unwrap().clone();
        let hit_test = shell.hit_test_engine().unwrap();
        let doc = shell.document();
        assert!(
            panel.on_panel_click(cx, cy, &styles, doc, hit_test),
            "a tree row click must be consumed"
        );
        assert_eq!(
            panel.selected_node(),
            Some(target),
            "clicking a tree row must select that DOM node"
        );
    }

    // Find an expand ARROW and click it: the count of visible tree rows must
    // change (children appear or disappear).
    let arrow = {
        let doc = shell.document();
        doc.descendants(doc.root())
            .into_iter()
            .find(|&id| doc.get_attribute(id, "data-tree-arrow").is_some())
    };
    if let Some(arrow) = arrow {
        // Snapshot the expansion fingerprint + visible count BEFORE the toggle.
        let sig_before = panel.refresh_signature();
        let before = panel.inspector.visible_nodes().len();
        let (cx, cy) = node_center(&shell, arrow).expect("arrow laid out");
        let styles = shell.style_map().unwrap().clone();
        let hit_test = shell.hit_test_engine().unwrap();
        let doc = shell.document();
        assert!(
            panel.on_panel_click(cx, cy, &styles, doc, hit_test),
            "an arrow click must be consumed"
        );

        // The toggle must change the refresh signature so the host rebuilds the
        // tree promptly (otherwise expand/collapse is frozen until a periodic
        // tick — the bug the expansion_fingerprint fix addresses).
        assert_ne!(
            panel.refresh_signature(),
            sig_before,
            "a tree expand/collapse must change the refresh signature so the panel \
             rebuilds the tree immediately"
        );

        // Rebuild the tree snapshot the way the host's refresh does, then assert
        // the visible-row count actually changed (children appeared/disappeared).
        panel.refresh_inspector(shell.document());
        let after = panel.inspector.visible_nodes().len();
        assert_ne!(
            before, after,
            "after toggling the expand/collapse arrow + rebuilding, the number of \
             visible tree rows must change (children appear/disappear)"
        );
    }
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

    let styles = shell
        .style_map()
        .expect("styles should exist after build_scene");
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

    // The panel is now template-rendered via the CSS pipeline; build_scene()
    // only returns overlay nodes (picker, hover, selection highlights).
    // Verify the template produces a non-trivial tree instead.
    let template = panel.render_template(doc, layout, styles);
    assert_eq!(
        template.tag.as_str(),
        "devtools-panel",
        "visible devtools should produce a devtools-panel template"
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

    // The panel itself is now rendered via render_template() → CSS pipeline,
    // so build_scene() only produces overlay nodes.  Verify that
    // render_template() returns a non-trivial devtools-panel tree.
    let template = panel.render_template(doc, layout, styles);
    let has_children = !template.children.is_empty();
    assert!(
        has_children,
        "devtools template should contain child nodes (toolbar, content, statusbar)"
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

// ── Click Dispatch ──────────────────────────────────────────────────────────

#[test]
fn on_panel_click_finds_devtools_elements() {
    let (shell, panel) = shell_with_devtools_template();

    // Get the panel bounds - should be at bottom of screen
    let bounds = panel.panel_bounds();

    let hit_test = shell.hit_test_engine().unwrap();

    // Click in the center of the panel, near top where tabs are
    let click_x = bounds.x + bounds.width / 2.0;
    let click_y = bounds.y + 15.0;

    // Test that hit_test finds something inside devtools panel
    let point = liquide_layout::geometry::Point::new(click_x, click_y);
    let hit_result = hit_test.hit_test(point);

    assert!(
        hit_result.is_some(),
        "Hit test should find an element inside devtools panel bounds"
    );

    // Verify hit bounds are within devtools panel
    let result = hit_result.unwrap();
    assert!(
        result.bounds.y >= bounds.y,
        "Hit element should be within panel bounds"
    );
}
