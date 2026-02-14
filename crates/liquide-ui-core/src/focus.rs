//! Focus management — keyboard focus traversal chain.

use crate::id::WidgetId;

/// Direction of focus traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Forward,
    Backward,
}

/// A unique focus identifier (aliases WidgetId for clarity).
pub type FocusId = WidgetId;

/// A linked chain of focusable widgets for Tab/Shift+Tab navigation.
#[derive(Debug, Clone)]
pub struct FocusChain {
    chain: Vec<FocusId>,
    current: Option<usize>,
}

impl FocusChain {
    pub fn new() -> Self {
        Self {
            chain: Vec::new(),
            current: None,
        }
    }

    /// Add a widget to the focus chain.
    pub fn push(&mut self, id: FocusId) {
        if !self.chain.contains(&id) {
            self.chain.push(id);
        }
    }

    /// Remove a widget from the focus chain.
    pub fn remove(&mut self, id: &FocusId) {
        if let Some(pos) = self.chain.iter().position(|w| w == id) {
            self.chain.remove(pos);
            // Adjust current index if needed
            if let Some(cur) = self.current {
                if pos < cur {
                    self.current = Some(cur - 1);
                } else if pos == cur {
                    self.current = None;
                }
            }
        }
    }

    /// Get the currently focused widget.
    pub fn current(&self) -> Option<FocusId> {
        self.current.map(|i| self.chain[i])
    }

    /// Move focus in the given direction. Returns the newly focused widget.
    pub fn advance(&mut self, direction: FocusDirection) -> Option<FocusId> {
        if self.chain.is_empty() {
            return None;
        }
        let len = self.chain.len();
        let next = match (self.current, direction) {
            (None, FocusDirection::Forward) => 0,
            (None, FocusDirection::Backward) => len - 1,
            (Some(i), FocusDirection::Forward) => (i + 1) % len,
            (Some(i), FocusDirection::Backward) => (i + len - 1) % len,
        };
        self.current = Some(next);
        Some(self.chain[next])
    }

    /// Directly focus a specific widget.
    pub fn focus(&mut self, id: FocusId) -> bool {
        if let Some(pos) = self.chain.iter().position(|w| *w == id) {
            self.current = Some(pos);
            true
        } else {
            false
        }
    }

    /// Clear focus.
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Number of focusable widgets.
    pub fn len(&self) -> usize {
        self.chain.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }
}

impl Default for FocusChain {
    fn default() -> Self {
        Self::new()
    }
}
