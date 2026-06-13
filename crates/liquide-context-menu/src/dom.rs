//! DOM-based rendering helpers for context menus.
//!
//! Builds a DOM subtree representing a context menu so the CSS pipeline
//! (Style → Layout → Paint) can render it instead of the legacy immediate-mode
//! scene builder.

use liquide_dom::document::Document;
use liquide_dom::node::NodeId;
use liquide_dom::pseudo::PseudoStateFlags;

use crate::{ContextMenu, MenuItemKind};

// ---------------------------------------------------------------------------
// DOM tree builder
// ---------------------------------------------------------------------------

/// Build a `<context-menu>` DOM subtree under `parent`.
///
/// Returns the root `NodeId` of the context menu element.
///
/// Structure:
/// ```text
/// <context-menu id="css-context-menu">
///   <menu-item class="action">Label</menu-item>
///   <menu-item class="separator" />
///   <menu-item class="submenu">Submenu Label</menu-item>
///   <menu-item class="toggle checked">Toggle Label</menu-item>
///   ...
/// </context-menu>
/// ```
pub fn build_context_menu_dom(doc: &mut Document, parent: NodeId, menu: &ContextMenu) -> NodeId {
    let root = doc.create_element("context-menu");
    doc.set_id(root, "css-context-menu");
    doc.append_child(parent, root);

    append_menu_items(doc, root, menu);
    root
}

/// Re-synchronise the DOM subtree to match the current `ContextMenu` state.
///
/// Removes all existing children and rebuilds them from `menu.items()`.
pub fn sync_context_menu_dom(doc: &mut Document, menu_node: NodeId, menu: &ContextMenu) {
    // Remove existing children
    let children: Vec<NodeId> = doc.children(menu_node).to_vec();
    for child in children {
        doc.remove_child(menu_node, child);
    }

    append_menu_items(doc, menu_node, menu);
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn append_menu_items(doc: &mut Document, parent: NodeId, menu: &ContextMenu) {
    for (i, item) in menu.items().iter().enumerate() {
        let el = doc.create_element("menu-item");

        // Separators are non-interactive divider rows: emit the documented
        // `separator` class and skip kind/state classes and label text so the
        // CSS pipeline can style them as divider lines rather than blank
        // disabled action rows.
        if item.separator {
            doc.add_class(el, "separator");
            doc.append_child(parent, el);
            continue;
        }

        // Kind-specific classes
        match &item.kind {
            MenuItemKind::Action(_) => {
                doc.add_class(el, "action");
            }
            MenuItemKind::Submenu(_) => {
                doc.add_class(el, "submenu");
            }
            MenuItemKind::Toggle { checked, .. } => {
                doc.add_class(el, "toggle");
                if *checked {
                    doc.add_class(el, "checked");
                }
            }
        }

        // Disabled state
        if item.disabled {
            doc.add_class(el, "disabled");
        }

        // Hover pseudo-state
        if menu.hover_index() == Some(i) {
            doc.set_pseudo_state(el, PseudoStateFlags::HOVER, true);
        }

        // Data attributes
        if let Some(ref icon) = item.icon {
            doc.set_attribute(el, "data-icon", icon);
        }
        if let Some(ref shortcut) = item.shortcut_hint {
            doc.set_attribute(el, "data-shortcut", shortcut);
        }

        // Label text child
        let txt = doc.create_text(&item.label);
        doc.append_child(el, txt);

        doc.append_child(parent, el);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MenuAction, MenuItem};

    #[test]
    fn build_context_menu_dom_creates_items() {
        let mut doc = Document::new();
        let root = doc.root();

        let items = vec![
            MenuItem::action("Cut", MenuAction(1)).with_shortcut("Ctrl+X"),
            MenuItem::action("Copy", MenuAction(2)).with_shortcut("Ctrl+C"),
            MenuItem::action("Paste", MenuAction(3))
                .with_shortcut("Ctrl+V")
                .with_disabled(true),
            MenuItem::submenu("More…", vec![MenuItem::action("Select All", MenuAction(4))]),
        ];
        let menu = ContextMenu::new(items);

        let menu_node = build_context_menu_dom(&mut doc, root, &menu);
        let children = doc.children(menu_node);
        assert_eq!(children.len(), 4, "should have 4 menu items");
    }

    #[test]
    fn sync_context_menu_replaces_items() {
        let mut doc = Document::new();
        let root = doc.root();

        let menu = ContextMenu::new(vec![MenuItem::action("A", MenuAction(1))]);
        let menu_node = build_context_menu_dom(&mut doc, root, &menu);
        assert_eq!(doc.children(menu_node).len(), 1);

        // Replace with 3 items
        let mut menu2 = ContextMenu::new(vec![
            MenuItem::action("X", MenuAction(10)),
            MenuItem::action("Y", MenuAction(11)),
            MenuItem::action("Z", MenuAction(12)),
        ]);
        // Open to make it visible (not required for DOM, but good practice)
        menu2.open(liquide_compositor::geometry::Point::new(0.0, 0.0));

        sync_context_menu_dom(&mut doc, menu_node, &menu2);
        assert_eq!(doc.children(menu_node).len(), 3);
    }

    #[test]
    fn separator_item_emits_separator_class() {
        // F26: the DOM renderer must emit the documented `separator` class for
        // separator items (not `action disabled`).
        let mut doc = Document::new();
        let root = doc.root();

        let items = vec![
            MenuItem::action("Cut", MenuAction(1)),
            MenuItem::separator(),
            MenuItem::action("Paste", MenuAction(2)),
        ];
        let menu = ContextMenu::new(items);
        let menu_node = build_context_menu_dom(&mut doc, root, &menu);

        let children = doc.children(menu_node).to_vec();
        assert_eq!(children.len(), 3, "should have 3 menu items");

        // Exactly one node carries the `separator` class, and it is the middle
        // item.
        let sep_nodes = doc.get_elements_by_class("separator").to_vec();
        assert_eq!(sep_nodes.len(), 1, "exactly one separator node expected");
        assert_eq!(sep_nodes[0], children[1], "middle item is the separator");

        let sep = doc.get(children[1]).expect("separator node");
        assert!(sep.has_class("separator"), "separator must carry the class");
        // Separators must not masquerade as activatable action rows.
        assert!(
            !sep.has_class("action"),
            "separator must not have the `action` class"
        );

        // Sibling action items must not be tagged as separators.
        assert!(!doc.get(children[0]).unwrap().has_class("separator"));
        assert!(!doc.get(children[2]).unwrap().has_class("separator"));
    }
}
