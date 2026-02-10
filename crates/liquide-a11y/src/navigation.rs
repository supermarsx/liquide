use serde::{Deserialize, Serialize};

use crate::focus::FocusManager;
use crate::node::NodeId;
use crate::tree::AccessibilityTree;

/// Navigation action triggered by keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationAction {
    TabForward,
    TabBackward,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Activate,
    Escape,
    RegionNext,
    RegionPrevious,
}

/// Result of a navigation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationResult {
    FocusMoved(NodeId),
    Activated(NodeId),
    Escaped,
    NoChange,
}

/// Keyboard navigation engine — handles tab, arrow, and region cycling.
#[derive(Debug, Clone)]
pub struct KeyboardNavigation {
    regions: Vec<NodeId>,
    current_region: usize,
}

impl KeyboardNavigation {
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            current_region: 0,
        }
    }

    /// Set the navigation regions (e.g. landmark nodes for F6 cycling).
    pub fn set_regions(&mut self, ids: Vec<NodeId>) {
        self.regions = ids;
        self.current_region = 0;
    }

    /// Move to the next region, returning its ID.
    #[must_use]
    pub fn next_region(&mut self) -> Option<NodeId> {
        if self.regions.is_empty() {
            return None;
        }
        self.current_region = (self.current_region + 1) % self.regions.len();
        Some(self.regions[self.current_region])
    }

    /// Move to the previous region, returning its ID.
    #[must_use]
    pub fn previous_region(&mut self) -> Option<NodeId> {
        if self.regions.is_empty() {
            return None;
        }
        if self.current_region == 0 {
            self.current_region = self.regions.len() - 1;
        } else {
            self.current_region -= 1;
        }
        Some(self.regions[self.current_region])
    }

    /// Handle a navigation action, returning the result.
    pub fn handle_action(
        &mut self,
        action: NavigationAction,
        _tree: &AccessibilityTree,
        focus: &mut FocusManager,
    ) -> NavigationResult {
        match action {
            NavigationAction::TabForward => {
                if let Some(id) = focus.focus_next() {
                    focus.show_focus_ring();
                    NavigationResult::FocusMoved(id)
                } else {
                    NavigationResult::NoChange
                }
            }
            NavigationAction::TabBackward => {
                if let Some(id) = focus.focus_previous() {
                    focus.show_focus_ring();
                    NavigationResult::FocusMoved(id)
                } else {
                    NavigationResult::NoChange
                }
            }
            NavigationAction::Activate => {
                if let Some(id) = focus.focused() {
                    NavigationResult::Activated(id)
                } else {
                    NavigationResult::NoChange
                }
            }
            NavigationAction::Escape => {
                focus.clear_focus();
                focus.hide_focus_ring();
                NavigationResult::Escaped
            }
            NavigationAction::RegionNext => {
                if let Some(id) = self.next_region() {
                    focus.set_focus(id);
                    NavigationResult::FocusMoved(id)
                } else {
                    NavigationResult::NoChange
                }
            }
            NavigationAction::RegionPrevious => {
                if let Some(id) = self.previous_region() {
                    focus.set_focus(id);
                    NavigationResult::FocusMoved(id)
                } else {
                    NavigationResult::NoChange
                }
            }
            NavigationAction::ArrowUp
            | NavigationAction::ArrowDown
            | NavigationAction::ArrowLeft
            | NavigationAction::ArrowRight => {
                // Arrow navigation within widgets is context-dependent.
                // For now, treat like tab forward/backward for up/down.
                match action {
                    NavigationAction::ArrowDown | NavigationAction::ArrowRight => {
                        if let Some(id) = focus.focus_next() {
                            NavigationResult::FocusMoved(id)
                        } else {
                            NavigationResult::NoChange
                        }
                    }
                    _ => {
                        if let Some(id) = focus.focus_previous() {
                            NavigationResult::FocusMoved(id)
                        } else {
                            NavigationResult::NoChange
                        }
                    }
                }
            }
        }
    }
}

impl Default for KeyboardNavigation {
    fn default() -> Self {
        Self::new()
    }
}
