//! Context menu — right-click context menus for DOM nodes in the
//! element tree and viewport.

use liquide_compositor::geometry::Rect;
use liquide_dom::NodeId;

/// A single context menu item.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Unique action ID.
    pub action: ContextAction,
    /// Display label.
    pub label: String,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Whether this is a separator.
    pub separator: bool,
}

/// Actions that can be triggered from a context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    /// Inspect this element (select + show in Elements tab).
    InspectElement,
    /// Copy the element's outer HTML to clipboard.
    CopyOuterHtml,
    /// Copy the element's selector path.
    CopySelectorPath,
    /// Copy the node ID.
    CopyNodeId,
    /// Expand all children recursively.
    ExpandAll,
    /// Collapse all children recursively.
    CollapseAll,
    /// Scroll element into viewport.
    ScrollIntoView,
    /// Force element state (:hover).
    ForceHover,
    /// Force element state (:active).
    ForceActive,
    /// Force element state (:focus).
    ForceFocus,
    /// Show layout overlay for this element.
    ShowLayout,
    /// Hide element (toggle visibility).
    HideElement,
    /// Delete element from DOM.
    DeleteElement,
    /// Edit element attributes.
    EditAttributes,
    /// Edit element text content.
    EditTextContent,
    /// Navigate to this node in the Scene Graph.
    ShowInSceneGraph,
    /// Log this element to console.
    LogToConsole,
    /// Close the menu (no action).
    Close,
}

/// The context menu state.
pub struct ContextMenu {
    /// Whether the menu is visible.
    visible: bool,
    /// Screen position of the top-left corner.
    position: (f32, f32),
    /// The node this menu was opened on.
    target_node: Option<NodeId>,
    /// Menu items.
    items: Vec<ContextMenuItem>,
    /// Currently highlighted item index.
    hovered_index: Option<usize>,
}

impl ContextMenu {
    /// Create a new hidden context menu.
    pub fn new() -> Self {
        Self {
            visible: false,
            position: (0.0, 0.0),
            target_node: None,
            items: Vec::new(),
            hovered_index: None,
        }
    }

    /// Show the context menu for a specific node at screen coords.
    pub fn show(&mut self, node_id: NodeId, x: f32, y: f32) {
        self.visible = true;
        self.position = (x, y);
        self.target_node = Some(node_id);
        self.hovered_index = None;
        self.items = Self::build_items(node_id);
    }

    /// Hide the context menu.
    pub fn hide(&mut self) {
        self.visible = false;
        self.target_node = None;
        self.items.clear();
        self.hovered_index = None;
    }

    /// Whether visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get target node.
    pub fn target_node(&self) -> Option<NodeId> {
        self.target_node
    }

    /// Get menu position.
    pub fn position(&self) -> (f32, f32) {
        self.position
    }

    /// Get menu items.
    pub fn items(&self) -> &[ContextMenuItem] {
        &self.items
    }

    /// Get hovered index.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Get menu bounds for hit testing.
    pub fn bounds(&self) -> Rect {
        let item_h = 24.0;
        let width = 220.0;
        let height = self.items.len() as f32 * item_h + 8.0;
        Rect::new(self.position.0, self.position.1, width, height)
    }

    /// Handle mouse move inside the menu. Returns true if hover changed.
    pub fn on_mouse_move(&mut self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        let bounds = self.bounds();
        if x < bounds.x
            || x > bounds.x + bounds.width
            || y < bounds.y
            || y > bounds.y + bounds.height
        {
            if self.hovered_index.is_some() {
                self.hovered_index = None;
                return true;
            }
            return false;
        }

        let item_h = 24.0;
        let idx = ((y - bounds.y - 4.0) / item_h).floor() as usize;
        let new_hover = if idx < self.items.len() && !self.items[idx].separator {
            Some(idx)
        } else {
            None
        };

        if new_hover != self.hovered_index {
            self.hovered_index = new_hover;
            true
        } else {
            false
        }
    }

    /// Handle click inside menu. Returns the action if a valid item was clicked.
    pub fn on_click(&mut self, x: f32, y: f32) -> Option<(ContextAction, NodeId)> {
        if !self.visible {
            return None;
        }
        let bounds = self.bounds();
        if x < bounds.x
            || x > bounds.x + bounds.width
            || y < bounds.y
            || y > bounds.y + bounds.height
        {
            self.hide();
            return None;
        }

        let item_h = 24.0;
        let idx = ((y - bounds.y - 4.0) / item_h).floor() as usize;
        if let Some(item) = self.items.get(idx) {
            if item.enabled && !item.separator {
                let action = item.action;
                let node = self.target_node;
                self.hide();
                return node.map(|n| (action, n));
            }
        }

        None
    }

    /// Build the standard context menu items for a node.
    fn build_items(_node_id: NodeId) -> Vec<ContextMenuItem> {
        vec![
            ContextMenuItem {
                action: ContextAction::InspectElement,
                label: "Inspect Element".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::ShowLayout,
                label: "Show Layout".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::ShowInSceneGraph,
                label: "Show in Scene Graph".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::LogToConsole,
                label: "Log to Console".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::Close,
                label: String::new(),
                enabled: false,
                separator: true,
            },
            ContextMenuItem {
                action: ContextAction::CopyOuterHtml,
                label: "Copy Outer HTML".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::CopySelectorPath,
                label: "Copy Selector Path".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::CopyNodeId,
                label: "Copy Node ID".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::Close,
                label: String::new(),
                enabled: false,
                separator: true,
            },
            ContextMenuItem {
                action: ContextAction::ForceHover,
                label: "Force :hover".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::ForceActive,
                label: "Force :active".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::ForceFocus,
                label: "Force :focus".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::Close,
                label: String::new(),
                enabled: false,
                separator: true,
            },
            ContextMenuItem {
                action: ContextAction::ExpandAll,
                label: "Expand All Children".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::CollapseAll,
                label: "Collapse All Children".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::ScrollIntoView,
                label: "Scroll into View".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::Close,
                label: String::new(),
                enabled: false,
                separator: true,
            },
            ContextMenuItem {
                action: ContextAction::HideElement,
                label: "Hide Element".into(),
                enabled: true,
                separator: false,
            },
            ContextMenuItem {
                action: ContextAction::DeleteElement,
                label: "Delete Element".into(),
                enabled: true,
                separator: false,
            },
        ]
    }
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_hidden_by_default() {
        let menu = ContextMenu::new();
        assert!(!menu.is_visible());
        assert!(menu.target_node().is_none());
    }

    #[test]
    fn test_show_and_hide() {
        let mut menu = ContextMenu::new();
        menu.show(5u64, 100.0, 200.0);
        assert!(menu.is_visible());
        assert_eq!(menu.target_node(), Some(5u64));
        assert_eq!(menu.position(), (100.0, 200.0));
        assert!(!menu.items().is_empty());

        menu.hide();
        assert!(!menu.is_visible());
    }

    #[test]
    fn test_hover() {
        let mut menu = ContextMenu::new();
        menu.show(1u64, 0.0, 0.0);
        let bounds = menu.bounds();
        // Move into the first item area.
        let changed = menu.on_mouse_move(bounds.x + 10.0, bounds.y + 10.0);
        assert!(changed);
        assert_eq!(menu.hovered_index(), Some(0));
    }
}
