//! Unit tests for the DevTools panel.

use super::*;

#[test]
fn test_toggle() {
    let mut panel = DevToolsPanel::with_defaults();
    assert!(!panel.is_visible());
    panel.toggle();
    assert!(panel.is_visible());
    panel.toggle();
    assert!(!panel.is_visible());
}

#[test]
fn test_tab_cycling() {
    let mut panel = DevToolsPanel::with_defaults();
    assert_eq!(panel.active_tab(), DevToolsTab::Elements);
    panel.next_tab();
    assert_eq!(panel.active_tab(), DevToolsTab::Console);
    panel.prev_tab();
    assert_eq!(panel.active_tab(), DevToolsTab::Elements);
}

#[test]
fn test_keyboard_f12() {
    let mut panel = DevToolsPanel::with_defaults();
    assert!(!panel.is_visible());
    assert!(panel.handle_key("F12", false, false, false));
    assert!(panel.is_visible());
}

#[test]
fn test_keyboard_ctrl_shift_i() {
    let mut panel = DevToolsPanel::with_defaults();
    assert!(panel.handle_key("I", true, true, false));
    assert!(panel.is_visible());
}

#[test]
fn test_panel_bounds_bottom() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    let bounds = panel.panel_bounds();
    assert_eq!(bounds.y, 1080.0 - 320.0);
    assert_eq!(bounds.width, 1920.0);
}

#[test]
fn test_dock_position_change() {
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_screen_size(1920.0, 1080.0);
    panel.set_dock_position(DockPosition::Right);
    let bounds = panel.panel_bounds();
    // Side-dock: the panel is pinned to the right edge, fills the full height,
    // and its width is the single-source `panel_size` (default 320). Asserting
    // against `panel_size()` rather than a bare literal keeps this honest if the
    // default size is ever retuned — but still proves the dock-position geometry
    // (right edge = screen_width - size; full height).
    let size = panel.panel_size();
    assert_eq!(size, 320.0); // default; the single source for the docked width
    assert_eq!(bounds.x, 1920.0 - size);
    assert_eq!(bounds.width, size);
    assert_eq!(bounds.height, 1080.0);
}

/// Mount the full panel `render_template` under a fresh parent and return the
/// reconciled (doc, devtools-panel root NodeId). Mirrors how the shell / the
/// separate devtools window reconcile the panel into a live document.
fn mount_panel_root(panel: &DevToolsPanel) -> (liquide_dom::Document, liquide_dom::NodeId) {
    use liquide_components::TemplateRenderer;
    let layout = liquide_layout::tree::LayoutTree::new();
    let styles = liquide_style_engine::StyleMap::new();
    let mut doc = liquide_dom::Document::new();
    let root = doc.root();
    let host = doc.create_element("devtools-host");
    doc.append_child(root, host);
    let tmpl = panel.render_template(&doc, &layout, &styles);
    // `apply_to_node` reconciles the template root IN PLACE onto `host`, so after
    // this `host` *is* the `devtools-panel` root (carrying its class + styles).
    TemplateRenderer::apply_to_node(&mut doc, host, &tmpl);
    (doc, host)
}

/// The painted/laid-out width of the docked panel (driven by the inline style
/// on the panel root, sourced from `config.panel_size`) MUST equal the width of
/// the hit/bounds region returned by `panel_bounds()`. Single source of truth:
/// they read the same `panel_size`, so paint == hit. RED if the rendered width
/// ever diverges from the bounds gate (the original 480px-CSS vs 320-config bug
/// escalated by t174).
#[test]
fn docked_panel_paint_width_matches_bounds_width() {
    for pos in [DockPosition::Left, DockPosition::Right] {
        let mut panel = DevToolsPanel::with_defaults();
        panel.set_screen_size(1920.0, 1080.0);
        panel.set_dock_position(pos);

        let bounds_w = panel.panel_bounds().width;

        let (doc, panel_root) = mount_panel_root(&panel);
        let painted = doc
            .get_inline_style(panel_root, "width")
            .expect("docked panel root must carry an inline width (single source)");
        let painted_px: f32 = painted
            .trim_end_matches("px")
            .parse()
            .expect("inline width must be a px length");

        assert_eq!(
            painted_px, bounds_w,
            "{:?}: painted width {} must equal bounds width {} (single source)",
            pos, painted_px, bounds_w
        );
    }
}

#[test]
fn test_hidden_scene_minimal() {
    let panel = DevToolsPanel::with_defaults();
    let layout = liquide_layout::tree::LayoutTree::new();
    let styles = liquide_style_engine::StyleMap::new();
    let doc = liquide_dom::Document::new();
    let scene = panel.build_scene(&doc, &layout, &styles);
    // When hidden, only overlay/picker nodes (both inactive → 0).
    assert!(scene.is_empty());
}

// ─── t131 jank fix: has_active_overlays mirrors build_scene emission ───

#[test]
fn idle_visible_panel_has_no_active_overlays() {
    // REGRESSION (t130/t131 jank): a merely-visible devtools panel with no
    // picker / no target / no selection emits NO direct overlay scene nodes, so
    // it must NOT force the conservative full-frame path. `has_active_overlays`
    // must agree with `build_scene` returning empty here, otherwise the render
    // loop keeps discarding the precomputed-damage fast path every frame (the
    // bug). This is RED if the predicate ever reports overlays for an idle panel
    // (e.g. naively keying off `layout_overlay.is_enabled()`, which is true by
    // default but emits nothing without a target).
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();
    assert!(
        panel.is_visible(),
        "panel must be visible for this scenario"
    );
    assert!(
        !panel.has_active_overlays(),
        "an idle visible panel must report NO active overlays"
    );

    // And `build_scene` must indeed emit nothing — the contract the predicate
    // tracks.
    let layout = liquide_layout::tree::LayoutTree::new();
    let styles = liquide_style_engine::StyleMap::new();
    let doc = liquide_dom::Document::new();
    assert!(
        panel.build_scene(&doc, &layout, &styles).is_empty(),
        "an idle visible panel must emit no overlay scene nodes"
    );
}

#[test]
fn active_picker_or_selection_reports_overlays() {
    // The predicate must report overlays exactly when one is live, so the loop
    // falls back to the full diff (the overlay nodes added after build_scene
    // escape the precomputed-damage hint).
    let mut panel = DevToolsPanel::with_defaults();
    panel.show();
    assert!(!panel.has_active_overlays());

    // Picker active → overlay present.
    panel.toggle_picker();
    assert!(panel.element_picker.is_active());
    assert!(
        panel.has_active_overlays(),
        "an active element picker must report an active overlay"
    );
    panel.toggle_picker();
    assert!(!panel.has_active_overlays());

    // A selected node → persistent selection highlight overlay.
    let styles = liquide_style_engine::StyleMap::new();
    panel.select_node(liquide_dom::NodeId::from(7u32), &styles);
    assert!(
        panel.has_active_overlays(),
        "a selected node must report an active selection-highlight overlay"
    );
}

// ─── t131 separate window: detach model wiring ───

#[test]
fn toggle_detach_raises_then_clears_window_requests() {
    // The host drives window create/teardown off these flags. Detaching raises a
    // create request; re-docking raises a teardown request. This is the model
    // `devtools_panel/mod.rs:340-356` left unwired before t131.
    let mut panel = DevToolsPanel::with_defaults();
    assert!(!panel.is_detached());
    assert!(!panel.detach_requested());
    assert!(!panel.close_window_requested());

    // Detach → spawn-window request.
    panel.toggle_detach();
    assert!(panel.is_detached(), "panel must be in the Detached dock");
    assert!(
        panel.detach_requested(),
        "detaching must request a separate window be spawned"
    );
    assert!(!panel.close_window_requested());
    panel.clear_detach_request();
    assert!(!panel.detach_requested());

    // Re-dock → teardown request.
    panel.toggle_detach();
    assert!(!panel.is_detached(), "panel must return to a docked position");
    assert!(
        panel.close_window_requested(),
        "re-docking must request the separate window be torn down"
    );
    panel.clear_close_window_request();
    assert!(!panel.close_window_requested());
}

#[test]
fn on_window_closed_redocks_without_new_requests() {
    // When the OS / the window's own F12 closes the devtools window, the panel
    // must re-dock WITHOUT raising a fresh teardown request (the window is
    // already gone — a stale request would try to destroy a non-existent
    // window or respawn it).
    let mut panel = DevToolsPanel::with_defaults();
    panel.toggle_detach();
    panel.clear_detach_request();
    assert!(panel.is_detached());

    panel.on_window_closed();
    assert!(!panel.is_detached(), "closing the window must re-dock the panel");
    assert!(!panel.detach_requested());
    assert!(!panel.close_window_requested());
}

#[test]
fn detached_panel_bounds_fill_the_window() {
    // The detached panel fills its OWN window (bounds == window size mirrored
    // into screen_width/height), so its hit-testing / scrolling cover the whole
    // surface. RED if the Detached branch reverts to a centered float box.
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_dock_position(DockPosition::Detached);
    panel.set_screen_size(900.0, 600.0);
    let b = panel.panel_bounds();
    assert_eq!((b.x, b.y, b.width, b.height), (0.0, 0.0, 900.0, 600.0));
}

// ─── t142: per-frame FPS readout is paint-only, frame counter stays layout ───

/// Mount the active tab's content template under a fresh parent and return
/// (doc, parent). Mirrors how the shell reconciles the devtools template.
fn mount_perf_panel(panel: &DevToolsPanel) -> (liquide_dom::Document, liquide_dom::NodeId) {
    use liquide_components::{TemplateNode, TemplateRenderer};
    let layout = liquide_layout::tree::LayoutTree::new();
    let styles = liquide_style_engine::StyleMap::new();
    let mut doc = liquide_dom::Document::new();
    let root = doc.root();
    let parent = doc.create_element("perf");
    doc.append_child(root, parent);
    let tmpl = TemplateNode::el("perf").children(panel.template_performance(&doc, &layout, &styles));
    TemplateRenderer::apply_to_node(&mut doc, parent, &tmpl);
    (doc, parent)
}

/// Find the text node whose ancestor row carries `label`, returning its NodeId.
fn find_value_text(doc: &liquide_dom::Document, parent: liquide_dom::NodeId, label: &str)
    -> Option<liquide_dom::NodeId>
{
    for &row in doc.children(parent) {
        // Each row: <devtools-label>LABEL</> then <devtools-value><text></>.
        let kids: Vec<liquide_dom::NodeId> = doc.children(row).to_vec();
        let is_match = kids.first().is_some_and(|&lbl| {
            doc.children(lbl)
                .first()
                .and_then(|&t| doc.get(t).and_then(|n| n.text_content()))
                == Some(label)
        });
        if is_match {
            if let Some(&value_el) = kids.get(1) {
                return doc.children(value_el).first().copied();
            }
        }
    }
    None
}

#[test]
fn fps_readout_text_update_is_paint_only_not_layout() {
    use liquide_components::{TemplateNode, TemplateRenderer};

    let mut panel = DevToolsPanel::with_defaults();
    panel.set_tab(DevToolsTab::Performance);
    panel.push_frame_snapshot(FrameSnapshot {
        frame_number: 10,
        fps: 59.9,
        avg_frame_ms: 16.0,
        css_rule_count: 1,
        css_variable_count: 1,
        stylesheet_count: 1,
        viewport_w: 1920.0,
        viewport_h: 1080.0,
    });

    // First mount.
    let layout = liquide_layout::tree::LayoutTree::new();
    let styles = liquide_style_engine::StyleMap::new();
    let mut doc = liquide_dom::Document::new();
    let root = doc.root();
    let parent = doc.create_element("perf");
    doc.append_child(root, parent);
    let tmpl1 =
        TemplateNode::el("perf").children(panel.template_performance(&doc, &layout, &styles));
    TemplateRenderer::apply_to_node(&mut doc, parent, &tmpl1);

    let fps_txt =
        find_value_text(&doc, parent, "FPS").expect("FPS value text node must be present");
    let frame_txt =
        find_value_text(&doc, parent, "Frame").expect("Frame value text node must be present");

    // Clear all dirty from the initial construction.
    doc.dirty.clear_all();
    for n in [fps_txt, frame_txt] {
        if let Some(node) = doc.get_mut(n) {
            node.dirty.clear_all();
        }
    }

    // Next frame: FPS bumps (same row, new content) AND the frame number bumps.
    panel.push_frame_snapshot(FrameSnapshot {
        frame_number: 11,
        fps: 60.0,
        avg_frame_ms: 16.0,
        css_rule_count: 1,
        css_variable_count: 1,
        stylesheet_count: 1,
        viewport_w: 1920.0,
        viewport_h: 1080.0,
    });
    let tmpl2 =
        TemplateNode::el("perf").children(panel.template_performance(&doc, &layout, &styles));
    TemplateRenderer::apply_to_node(&mut doc, parent, &tmpl2);

    // Rendered output still updates (the text genuinely changed).
    assert_eq!(doc.get(fps_txt).and_then(|n| n.text_content()), Some("60.0"));

    // The FPS text update must be PAINT, NOT LAYOUT (the size-stable fast path).
    assert!(
        !doc.dirty.layout.contains(&fps_txt),
        "FPS readout text update must not mark LAYOUT-dirty"
    );
    assert!(
        !doc.get(fps_txt).unwrap().dirty.needs_layout(),
        "FPS readout text node must not carry the LAYOUT flag"
    );
    assert!(
        doc.dirty.paint.contains(&fps_txt),
        "FPS readout text update must still mark PAINT-dirty so it repaints"
    );
    assert!(
        doc.get(fps_txt).unwrap().dirty.needs_paint(),
        "FPS readout text node must carry the PAINT flag"
    );

    // SELECTIVITY / teeth: the unbounded Frame counter is deliberately left on the
    // conservative LAYOUT path (it can grow wide and reflow). If someone naively
    // marks ALL perf rows paint-only this assertion turns RED.
    assert_eq!(doc.get(frame_txt).and_then(|n| n.text_content()), Some("11"));
    assert!(
        doc.dirty.layout.contains(&frame_txt),
        "the unbounded Frame counter must stay on the LAYOUT path"
    );
}

#[test]
fn fps_value_cell_is_fixed_width_so_swap_cannot_reflow() {
    // The paint-only demotion is only sound because the FPS value box has a fixed
    // pixel width (content-independent geometry). Assert the inline width style is
    // present on the value cell — if it's dropped, the box could reflow and the
    // paint-only path would risk a stale layout.
    let mut panel = DevToolsPanel::with_defaults();
    panel.set_tab(DevToolsTab::Performance);
    panel.push_frame_snapshot(FrameSnapshot {
        frame_number: 1,
        fps: 60.0,
        avg_frame_ms: 16.0,
        css_rule_count: 0,
        css_variable_count: 0,
        stylesheet_count: 0,
        viewport_w: 800.0,
        viewport_h: 600.0,
    });
    let (doc, parent) = mount_perf_panel(&panel);

    // Locate the FPS row's value element (sibling-after-label) and check width.
    let mut found = false;
    for &row in doc.children(parent) {
        let kids: Vec<liquide_dom::NodeId> = doc.children(row).to_vec();
        let is_fps = kids.first().is_some_and(|&lbl| {
            doc.children(lbl)
                .first()
                .and_then(|&t| doc.get(t).and_then(|n| n.text_content()))
                == Some("FPS")
        });
        if is_fps {
            let value_el = kids[1];
            assert_eq!(
                doc.get_inline_style(value_el, "width").as_deref(),
                Some("44px"),
                "FPS value cell must be pinned to a fixed width"
            );
            found = true;
        }
    }
    assert!(found, "FPS row must exist");
}
