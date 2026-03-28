//! Focus chain — bounded stack of previously focused windows.

use crate::types::WindowId;

/// Bounded stack tracking focus changes so that closing a dialog can
/// restore focus to the previous window.
///
/// Maximum depth is 32 entries.
#[derive(Debug, Clone)]
pub struct FocusChain {
    stack: Vec<WindowId>,
    max_depth: usize,
}

impl FocusChain {
    /// Default maximum depth.
    pub const DEFAULT_MAX_DEPTH: usize = 32;

    /// Create a new focus chain with the given maximum depth.
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        let max_depth = max_depth.max(1);
        Self {
            stack: Vec::with_capacity(max_depth.min(32)),
            max_depth,
        }
    }

    /// Push the old focus window onto the chain.
    ///
    /// If the stack is full, the oldest (bottom) entry is discarded.
    /// Duplicate entries are removed before pushing so a window only
    /// appears once in the chain.
    pub fn push_focus(&mut self, window_id: WindowId) {
        // Remove any existing occurrence so we don't get duplicates.
        self.stack.retain(|&w| w != window_id);
        if self.stack.len() >= self.max_depth {
            // Discard the oldest entry (bottom of stack).
            self.stack.remove(0);
        }
        self.stack.push(window_id);
    }

    /// Pop the most-recently-pushed window.
    ///
    /// Returns `None` if the chain is empty.
    pub fn pop_focus(&mut self) -> Option<WindowId> {
        self.stack.pop()
    }

    /// Peek at the top of the chain without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<WindowId> {
        self.stack.last().copied()
    }

    /// Remove a window from anywhere in the chain (e.g. when it is closed).
    pub fn remove(&mut self, window_id: WindowId) {
        self.stack.retain(|&w| w != window_id);
    }

    /// Number of entries on the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Iterate over the chain from most-recent to oldest.
    pub fn iter(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.stack.iter().rev().copied()
    }

    /// Clear the chain.
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

impl Default for FocusChain {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_DEPTH)
    }
}
