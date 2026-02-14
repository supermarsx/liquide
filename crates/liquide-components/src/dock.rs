//! Dock component — renders the application dock via the template engine.

use liquide_dom::PseudoStateFlags;

use crate::types::{element_ids, DockItemInfo};
use crate::template::{Component, TemplateNode};

/// Dock component that renders the application dock.
///
/// Produces a DOM tree like:
/// ```text
/// <dock id="shell-dock">
///   <dock-item data-key="files" class="active pinned" data-app-id="files" data-icon="folder">
///     <dock-item-icon data-icon="folder" />
///     <dock-item-label>Files</dock-item-label>
///     <dock-indicator class="running" />
///   </dock-item>
///   …
/// </dock>
/// ```
///
/// CSS can now target:
/// - `dock` — the container bar
/// - `dock-item` — each icon slot
/// - `dock-item.active` — running apps
/// - `dock-item.pinned` — pinned apps
/// - `dock-item:hover` — hovered item
/// - `dock-item-icon` — the icon element (uses `data-icon` attr)
/// - `dock-item-label` — the text label (for tooltips / accessibility)
/// - `dock-indicator` — the running-app dot
/// - `dock-indicator.running` — visible dot for running apps
pub struct DockComponent<'a> {
    pub items: &'a [DockItemInfo],
    pub hover_index: Option<usize>,
}

impl Component for DockComponent<'_> {
    fn render(&self) -> TemplateNode {
        TemplateNode::el("dock")
            .id(element_ids::DOCK)
            .children(self.items.iter().enumerate().map(|(i, item)| {
                TemplateNode::el("dock-item")
                    .key(&item.app_id)
                    .class_if("active", item.is_running)
                    .class_if("pinned", item.is_pinned)
                    .attr("data-app-id", &item.app_id)
                    .attr("data-icon", &item.icon)
                    .attr("data-label", &item.label)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::HOVER, self.hover_index == Some(i))
                    // Icon sub-element (CSS can style this + use data-icon for rendering)
                    .child(
                        TemplateNode::el("dock-item-icon")
                            .attr("data-icon", &item.icon),
                    )
                    // Label sub-element (for accessibility / tooltip display)
                    .child(
                        TemplateNode::el("dock-item-label")
                            .child(TemplateNode::text(&item.label)),
                    )
                    // Running indicator dot
                    .child(
                        TemplateNode::el("dock-indicator")
                            .class_if("running", item.is_running),
                    )
            }))
    }

    fn mount_point(&self) -> &str {
        element_ids::DOCK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items() -> Vec<DockItemInfo> {
        vec![
            DockItemInfo {
                app_id: "files".into(),
                label: "Files".into(),
                icon: "folder".into(),
                is_running: true,
                is_pinned: true,
            },
            DockItemInfo {
                app_id: "terminal".into(),
                label: "Terminal".into(),
                icon: "terminal".into(),
                is_running: false,
                is_pinned: true,
            },
            DockItemInfo {
                app_id: "browser".into(),
                label: "Browser".into(),
                icon: "globe".into(),
                is_running: true,
                is_pinned: false,
            },
        ]
    }

    #[test]
    fn dock_renders_all_items() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: None,
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "dock");
        assert_eq!(tree.element_id.as_deref(), Some(element_ids::DOCK));
        assert_eq!(tree.children.len(), 3);
    }

    #[test]
    fn dock_item_has_correct_structure() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: None,
        };
        let tree = comp.render();

        let first = &tree.children[0];
        assert_eq!(first.tag, "dock-item");
        assert_eq!(first.key.as_deref(), Some("files"));
        assert!(first.classes.contains(&"active".to_string()));
        assert!(first.classes.contains(&"pinned".to_string()));

        // Should have 3 sub-elements: icon, label, indicator
        assert_eq!(first.children.len(), 3);
        assert_eq!(first.children[0].tag, "dock-item-icon");
        assert_eq!(first.children[1].tag, "dock-item-label");
        assert_eq!(first.children[2].tag, "dock-indicator");
    }

    #[test]
    fn dock_hover_sets_pseudo_state() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: Some(1),
        };
        let tree = comp.render();

        assert!(!tree.children[0].pseudo_states.contains(PseudoStateFlags::HOVER));
        assert!(tree.children[1].pseudo_states.contains(PseudoStateFlags::HOVER));
        assert!(!tree.children[2].pseudo_states.contains(PseudoStateFlags::HOVER));
    }

    #[test]
    fn dock_inactive_item_no_active_class() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: None,
        };
        let tree = comp.render();

        let terminal = &tree.children[1];
        assert!(!terminal.classes.contains(&"active".to_string()));
        assert!(terminal.classes.contains(&"pinned".to_string()));
    }

    #[test]
    fn dock_running_indicator() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: None,
        };
        let tree = comp.render();

        // Files (running) → indicator has .running
        let files_indicator = &tree.children[0].children[2];
        assert!(files_indicator.classes.contains(&"running".to_string()));

        // Terminal (not running) → indicator has no .running
        let terminal_indicator = &tree.children[1].children[2];
        assert!(!terminal_indicator.classes.contains(&"running".to_string()));
    }

    #[test]
    fn dock_mount_point() {
        let items = make_items();
        let comp = DockComponent {
            items: &items,
            hover_index: None,
        };
        assert_eq!(comp.mount_point(), element_ids::DOCK);
    }
}
