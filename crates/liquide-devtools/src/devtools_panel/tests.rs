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
