use crate::node::NodeId;
use crate::tree::AccessibilityTree;

/// Manages focus tracking, tab order, and focus ring visibility.
#[derive(Debug, Clone)]
pub struct FocusManager {
    focused: Option<NodeId>,
    focus_ring_visible: bool,
    tab_order: Vec<NodeId>,
    tab_index: usize,
}

impl FocusManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            focused: None,
            focus_ring_visible: false,
            tab_order: Vec::new(),
            tab_index: 0,
        }
    }

    /// Set focus to a specific node.
    pub fn set_focus(&mut self, id: NodeId) {
        self.focused = Some(id);
        // Sync tab_index
        if let Some(pos) = self.tab_order.iter().position(|&n| n == id) {
            self.tab_index = pos;
        }
    }

    /// Get the currently focused node.
    #[must_use]
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Build the tab order by walking the tree and collecting focusable nodes.
    pub fn build_tab_order(&mut self, tree: &AccessibilityTree) {
        self.tab_order.clear();
        tree.walk(|node| {
            if node.is_focusable() {
                self.tab_order.push(node.id);
            }
        });
        self.tab_index = 0;
    }

    /// Move focus to the next item in the tab order.
    #[must_use]
    pub fn focus_next(&mut self) -> Option<NodeId> {
        if self.tab_order.is_empty() {
            return None;
        }
        self.tab_index = (self.tab_index + 1) % self.tab_order.len();
        let id = self.tab_order[self.tab_index];
        self.focused = Some(id);
        Some(id)
    }

    /// Move focus to the previous item in the tab order.
    #[must_use]
    pub fn focus_previous(&mut self) -> Option<NodeId> {
        if self.tab_order.is_empty() {
            return None;
        }
        if self.tab_index == 0 {
            self.tab_index = self.tab_order.len() - 1;
        } else {
            self.tab_index -= 1;
        }
        let id = self.tab_order[self.tab_index];
        self.focused = Some(id);
        Some(id)
    }

    /// Show the focus ring indicator.
    pub fn show_focus_ring(&mut self) {
        self.focus_ring_visible = true;
    }

    /// Hide the focus ring indicator.
    pub fn hide_focus_ring(&mut self) {
        self.focus_ring_visible = false;
    }

    /// Check if focus ring is visible.
    #[must_use]
    pub fn is_focus_ring_visible(&self) -> bool {
        self.focus_ring_visible
    }

    /// Get the current tab order.
    #[must_use]
    pub fn tab_order(&self) -> &[NodeId] {
        &self.tab_order
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}
