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
    assert_eq!(bounds.x, 1920.0 - 320.0);
    assert_eq!(bounds.height, 1080.0);
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
