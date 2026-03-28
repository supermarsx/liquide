//! Modal window tracking.

use crate::types::WindowId;

/// A single modal entry: the modal dialog and its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModalEntry {
    modal: WindowId,
    owner: WindowId,
}

/// Tracks modal dialog stacking.
///
/// When a modal dialog is active, its owner (and all windows that are not
/// the modal itself or its descendants) are blocked from receiving activation.
/// Modal windows stack: a modal can spawn another modal on top of it.
#[derive(Debug, Clone, Default)]
pub struct ModalState {
    /// Stack of modal entries, outermost first.
    stack: Vec<ModalEntry>,
}

impl ModalState {
    /// Create a new empty modal state.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Enter modal state: `modal_window` is a modal dialog owned by `owner`.
    ///
    /// If `modal_window` is already in the stack, this is a no-op.
    pub fn push_modal(&mut self, modal_window: WindowId, owner: WindowId) {
        if self.stack.iter().any(|e| e.modal == modal_window) {
            return;
        }
        self.stack.push(ModalEntry {
            modal: modal_window,
            owner,
        });
    }

    /// Leave modal state for `modal_window`.
    ///
    /// Removes the entry from the stack (anywhere, not just top, to handle
    /// out-of-order destruction).
    pub fn pop_modal(&mut self, modal_window: WindowId) {
        self.stack.retain(|e| e.modal != modal_window);
    }

    /// Is `window_id` currently a modal dialog?
    #[must_use]
    pub fn is_modal(&self, window_id: WindowId) -> bool {
        self.stack.iter().any(|e| e.modal == window_id)
    }

    /// Return the owner of `window_id` if it is a modal dialog.
    #[must_use]
    pub fn modal_owner(&self, window_id: WindowId) -> Option<WindowId> {
        self.stack
            .iter()
            .find(|e| e.modal == window_id)
            .map(|e| e.owner)
    }

    /// Should activation of `target` be blocked?
    ///
    /// Returns `true` if there is at least one modal window active and
    /// `target` is NOT the topmost modal (only the topmost modal and its
    /// descendants can be activated).
    #[must_use]
    pub fn should_block_activation(&self, target: WindowId) -> bool {
        if let Some(top) = self.stack.last() {
            // The topmost modal itself is never blocked.
            if top.modal == target {
                return false;
            }
            // Everything else is blocked while a modal is active.
            return true;
        }
        false
    }

    /// Return the topmost modal window, if any.
    #[must_use]
    pub fn topmost_modal(&self) -> Option<WindowId> {
        self.stack.last().map(|e| e.modal)
    }

    /// Number of active modals.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Whether any modal is active.
    #[must_use]
    pub fn has_modal(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Remove a window from modal tracking regardless of whether it is a
    /// modal or an owner (used when a window is destroyed).
    pub fn remove_window(&mut self, window_id: WindowId) {
        self.stack
            .retain(|e| e.modal != window_id && e.owner != window_id);
    }
}
