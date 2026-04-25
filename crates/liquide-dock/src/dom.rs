//! DOM-based rendering for the dock.
//!
//! Provides helpers to construct a DOM subtree for the dock and its items,
//! and to compute styled properties from the new CSS pipeline instead of
//! the legacy `DockThemeColors`.

use liquide_dom::{Document, NodeId, PseudoStateFlags};
use liquide_style_engine::StyleEngine;

use crate::dock::{Dock, DockItem, DockThemeColors};

/// Build a DOM subtree for the dock and return the root `<dock>` node id.
///
/// The resulting tree looks like:
/// ```text
/// <dock id="css-dock">
///   <dock-item class="active pinned" data-app-id="files"> Files </dock-item>
///   <dock-item data-app-id="browser"> Browser </dock-item>
///   …
/// </dock>
/// ```
pub fn build_dock_dom(doc: &mut Document, parent: NodeId, dock: &Dock) -> NodeId {
    let dock_el = doc.create_element("dock");
    doc.set_id(dock_el, "css-dock");
    doc.append_child(parent, dock_el);

    for (i, item) in dock.items().iter().enumerate() {
        add_dock_item_node(doc, dock_el, item, i, dock.hover_index());
    }

    dock_el
}

/// Add a single `<dock-item>` node to a parent.
fn add_dock_item_node(
    doc: &mut Document,
    parent: NodeId,
    item: &DockItem,
    index: usize,
    hover_index: Option<usize>,
) -> NodeId {
    let el = doc.create_element("dock-item");

    if item.running_window_count > 0 {
        doc.add_class(el, "active");
    }
    if item.pinned_position.is_some() {
        doc.add_class(el, "pinned");
    }
    if hover_index == Some(index) {
        doc.set_pseudo_state(el, PseudoStateFlags::HOVER, true);
    }

    doc.set_attribute(el, "data-app-id", &item.app_id);
    doc.set_attribute(el, "data-label", &item.label);

    let txt = doc.create_text(&item.label);
    doc.append_child(el, txt);
    doc.append_child(parent, el);

    el
}

/// Sync the DOM subtree to match the current dock state.
///
/// Removes stale children and rebuilds from the current item list.
pub fn sync_dock_dom(doc: &mut Document, dock_node: NodeId, dock: &Dock) {
    // Remove old children
    let old: Vec<NodeId> = doc.children(dock_node).to_vec();
    for child in old {
        doc.remove_child(dock_node, child);
        doc.destroy_node(child);
    }

    // Rebuild
    for (i, item) in dock.items().iter().enumerate() {
        add_dock_item_node(doc, dock_node, item, i, dock.hover_index());
    }
}

/// Extract theme colors from a `StyleEngine` for the dock, with fallbacks
/// matching the default `DockThemeColors`.
///
/// This is the bridge between the new pipeline and the existing
/// `Dock::build_scene()` API: callers can call this to get a
/// `DockThemeColors` populated from CSS computed styles.
pub fn dock_theme_from_css(
    engine: &StyleEngine,
    doc: &Document,
    dock_node: NodeId,
) -> DockThemeColors {
    use liquide_compositor::pixel::Color;

    let dock_style = engine.compute_style(doc, dock_node);

    let glass_tint = dock_style.background_color;
    let border = Color::new(
        dock_style.color.r,
        dock_style.color.g,
        dock_style.color.b,
        (dock_style.opacity * 255.0) as u8,
    );

    DockThemeColors {
        glass_tint: if glass_tint.a > 0 {
            glass_tint
        } else {
            Color::new(30, 30, 50, 179)
        },
        border: if border.a > 0 {
            border
        } else {
            Color::new(255, 255, 255, 15)
        },
        item_active: Color::new(255, 255, 255, 255),
        item_inactive: Color::new(255, 255, 255, 179),
        hover_highlight: Color::new(255, 255, 255, 31),
        needs_attention: DockThemeColors::default_needs_attention(),
        focus_outline: DockThemeColors::default_focus_outline(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dock_dom_creates_items() {
        let mut doc = Document::new();
        let root = doc.root();

        let mut dock = Dock::new(Default::default());
        dock.add_running("terminal");
        dock.add_running("files");

        let dock_node = build_dock_dom(&mut doc, root, &dock);
        assert_eq!(doc.children(dock_node).len(), 2);
    }

    #[test]
    fn sync_replaces_children() {
        let mut doc = Document::new();
        let root = doc.root();

        let mut dock = Dock::new(Default::default());
        dock.add_running("a");

        let dock_node = build_dock_dom(&mut doc, root, &dock);
        assert_eq!(doc.children(dock_node).len(), 1);

        dock.add_running("b");
        sync_dock_dom(&mut doc, dock_node, &dock);
        assert_eq!(doc.children(dock_node).len(), 2);
    }
}
