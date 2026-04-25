//! Menu components — context menu, session menu, and app menu.

use liquide_dom::PseudoStateFlags;

use crate::template::{Component, TemplateNode};
use crate::types::{element_ids, ContextMenuItemInfo, MenuItemInfo};

// ── Context Menu ─────────────────────────────────────────────────

/// Context menu component.
///
/// Produces:
/// ```text
/// <context-menu id="ctx-shell" style="left: 100; top: 200">
///   <menu-item data-key="0" data-index="0">Copy</menu-item>
///   <menu-separator />
///   <menu-item data-key="2" class="disabled" :disabled>Paste</menu-item>
/// </context-menu>
/// ```
pub struct ContextMenuComponent<'a> {
    pub menu_id: &'a str,
    pub items: &'a [ContextMenuItemInfo],
    pub hover_index: Option<usize>,
    /// Position (left, top) in logical pixels. If None, uses CSS defaults.
    pub position: Option<(f32, f32)>,
}

impl Component for ContextMenuComponent<'_> {
    fn render(&self) -> TemplateNode {
        let mut item_idx = 0usize;

        let mut menu = TemplateNode::el("context-menu").id(self.menu_id);

        // Apply inline position styles if provided
        if let Some((x, y)) = self.position {
            menu = menu
                .style("left", &format!("{x}"))
                .style("top", &format!("{y}"));
        }

        menu.children(self.items.iter().enumerate().map(|(i, item)| match item {
            ContextMenuItemInfo::Item(item) => {
                let current_idx = item_idx;
                item_idx += 1;
                TemplateNode::el("menu-item")
                    .key(&i.to_string())
                    .attr("data-index", &i.to_string())
                    .class_if("disabled", item.disabled)
                    .pseudo_if(PseudoStateFlags::DISABLED, item.disabled)
                    .pseudo_if(
                        PseudoStateFlags::HOVER,
                        self.hover_index == Some(current_idx),
                    )
                    .child(TemplateNode::text(&item.label))
            }
            ContextMenuItemInfo::Separator => {
                TemplateNode::el("menu-separator").key(&format!("sep-{i}"))
            }
        }))
    }

    fn mount_point(&self) -> &str {
        self.menu_id
    }
}

// ── Session Menu ─────────────────────────────────────────────────

/// Session menu component (lock, logout, restart, shutdown).
///
/// Produces:
/// ```text
/// <session-menu id="session-menu">
///   <menu-item data-key="lock" data-action="lock" data-icon="lock">
///     <menu-item-icon data-icon="lock" />
///     <menu-item-label>Lock</menu-item-label>
///   </menu-item>
///   …
/// </session-menu>
/// ```
pub struct SessionMenuComponent<'a> {
    pub items: &'a [MenuItemInfo],
    pub hover_index: Option<usize>,
}

impl Component for SessionMenuComponent<'_> {
    fn render(&self) -> TemplateNode {
        TemplateNode::el("session-menu")
            .id(element_ids::SESSION_MENU)
            .children(self.items.iter().enumerate().map(|(i, item)| {
                let mut node = TemplateNode::el("menu-item")
                    .key(&item.action)
                    .attr("data-action", &item.action)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::HOVER, self.hover_index == Some(i));

                // Icon sub-element
                if !item.icon.as_ref().map_or(true, |s| s.is_empty()) {
                    node = node
                        .attr("data-icon", item.icon.as_deref().unwrap_or(""))
                        .child(
                            TemplateNode::el("menu-item-icon")
                                .attr("data-icon", item.icon.as_deref().unwrap_or("")),
                        );
                }

                // Label
                node.child(
                    TemplateNode::el("menu-item-label").child(TemplateNode::text(&item.label)),
                )
            }))
    }

    fn mount_point(&self) -> &str {
        element_ids::SESSION_MENU
    }
}

// ── App Menu ─────────────────────────────────────────────────────

/// Application menu component (minimize, maximize, close, settings, about).
pub struct AppMenuComponent<'a> {
    pub items: &'a [MenuItemInfo],
    pub hover_index: Option<usize>,
}

impl Component for AppMenuComponent<'_> {
    fn render(&self) -> TemplateNode {
        TemplateNode::el("app-menu")
            .id(element_ids::APP_MENU)
            .children(self.items.iter().enumerate().map(|(i, item)| {
                let mut node = TemplateNode::el("menu-item")
                    .key(&item.action)
                    .attr("data-action", &item.action)
                    .attr("data-index", &i.to_string())
                    .pseudo_if(PseudoStateFlags::HOVER, self.hover_index == Some(i));

                if !item.icon.as_ref().map_or(true, |s| s.is_empty()) {
                    node = node
                        .attr("data-icon", item.icon.as_deref().unwrap_or(""))
                        .child(
                            TemplateNode::el("menu-item-icon")
                                .attr("data-icon", item.icon.as_deref().unwrap_or("")),
                        );
                }

                node.child(
                    TemplateNode::el("menu-item-label").child(TemplateNode::text(&item.label)),
                )
            }))
    }

    fn mount_point(&self) -> &str {
        element_ids::APP_MENU
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_menu_basic() {
        let items = vec![
            ContextMenuItemInfo::Item(MenuItemInfo {
                label: "Copy".into(),
                action: "copy".into(),
                icon: None,
                disabled: false,
            }),
            ContextMenuItemInfo::Separator,
            ContextMenuItemInfo::Item(MenuItemInfo {
                label: "Paste".into(),
                action: "paste".into(),
                icon: None,
                disabled: true,
            }),
        ];
        let comp = ContextMenuComponent {
            menu_id: "ctx-1",
            items: &items,
            hover_index: Some(0),
            position: None,
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "context-menu");
        assert_eq!(tree.children.len(), 3);
        assert_eq!(tree.children[0].tag, "menu-item");
        assert_eq!(tree.children[1].tag, "menu-separator");
        assert_eq!(tree.children[2].tag, "menu-item");

        // First item hovered
        assert!(tree.children[0]
            .pseudo_states
            .contains(PseudoStateFlags::HOVER));

        // Third item disabled
        assert!(tree.children[2].classes.contains(&"disabled".to_string()));
        assert!(tree.children[2]
            .pseudo_states
            .contains(PseudoStateFlags::DISABLED));
    }

    #[test]
    fn session_menu_with_icons() {
        let items = vec![
            MenuItemInfo {
                label: "Lock".into(),
                action: "lock".into(),
                icon: Some("lock".into()),
                disabled: false,
            },
            MenuItemInfo {
                label: "Shutdown".into(),
                action: "shutdown".into(),
                icon: Some("power".into()),
                disabled: false,
            },
        ];
        let comp = SessionMenuComponent {
            items: &items,
            hover_index: Some(1),
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "session-menu");
        assert_eq!(tree.children.len(), 2);

        // Second item hovered
        assert!(tree.children[1]
            .pseudo_states
            .contains(PseudoStateFlags::HOVER));

        // Items have icon + label sub-elements
        let lock = &tree.children[0];
        assert_eq!(lock.children.len(), 2); // icon + label
        assert_eq!(lock.children[0].tag, "menu-item-icon");
        assert_eq!(lock.children[1].tag, "menu-item-label");
    }

    #[test]
    fn app_menu_renders() {
        let items = vec![
            MenuItemInfo {
                label: "Minimize".into(),
                action: "minimize".into(),
                icon: None,
                disabled: false,
            },
            MenuItemInfo {
                label: "Close".into(),
                action: "close".into(),
                icon: None,
                disabled: false,
            },
        ];
        let comp = AppMenuComponent {
            items: &items,
            hover_index: None,
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "app-menu");
        assert_eq!(tree.children.len(), 2);

        // No icon → only label sub-element
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].tag, "menu-item-label");
    }
}
