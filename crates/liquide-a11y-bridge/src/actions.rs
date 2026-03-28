//! Accessible actions for the bridge layer.
//!
//! Models the set of actions that can be performed on an accessible node
//! (WAI-ARIA `aria-pressed`, `aria-expanded`, etc.) and provides a dispatch
//! mechanism for executing them.

use std::collections::HashMap;

use crate::tree::NodeId;

// ---------------------------------------------------------------------------
// Action enum
// ---------------------------------------------------------------------------

/// An action that can be performed on an accessible node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleAction {
    /// Default activation (e.g. click a button).
    Click,
    /// Press (mouse-down / key-down).
    Press,
    /// Release (mouse-up / key-up).
    Release,
    /// Move keyboard focus to this node.
    Focus,
    /// Activate the node (equivalent to Enter / Space).
    Activate,
    /// Dismiss a transient container (dialog, tooltip, popup).
    Dismiss,
    /// Expand a collapsible section.
    Expand,
    /// Collapse an expanded section.
    Collapse,
    /// Scroll the viewport so this node is visible.
    ScrollTo,
    /// Set the node's value (e.g. slider, text input).
    SetValue,
    /// Increment a numeric value (slider, spin-button).
    Increment,
    /// Decrement a numeric value.
    Decrement,
}

impl std::fmt::Display for AccessibleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// ---------------------------------------------------------------------------
// ActionSet
// ---------------------------------------------------------------------------

/// The set of actions available on a particular node.
#[derive(Debug, Clone)]
pub struct ActionSet {
    actions: Vec<AccessibleAction>,
}

impl ActionSet {
    /// Create an empty action set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Create an action set from a list of actions.
    #[must_use]
    pub fn from_actions(actions: &[AccessibleAction]) -> Self {
        let mut set = Self::new();
        for &a in actions {
            set.add(a);
        }
        set
    }

    /// Add an action (duplicates ignored).
    pub fn add(&mut self, action: AccessibleAction) {
        if !self.actions.contains(&action) {
            self.actions.push(action);
        }
    }

    /// Remove an action.
    pub fn remove(&mut self, action: AccessibleAction) {
        self.actions.retain(|a| *a != action);
    }

    /// Check if the set contains an action.
    #[must_use]
    pub fn contains(&self, action: AccessibleAction) -> bool {
        self.actions.contains(&action)
    }

    /// Number of actions in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Iterate over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &AccessibleAction> {
        self.actions.iter()
    }

    /// Return the actions as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[AccessibleAction] {
        &self.actions
    }
}

impl Default for ActionSet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActionHandler (callback-based dispatch)
// ---------------------------------------------------------------------------

/// Callback type for action handlers.
pub type ActionCallback = Box<dyn Fn(NodeId, AccessibleAction) -> bool + Send>;

/// Registry of action handlers, keyed by node ID.
///
/// Each node may have an [`ActionSet`] describing its available actions, and
/// a callback that is invoked when `perform_action` is called.
pub struct ActionHandler {
    action_sets: HashMap<NodeId, ActionSet>,
    callbacks: HashMap<NodeId, ActionCallback>,
}

impl ActionHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            action_sets: HashMap::new(),
            callbacks: HashMap::new(),
        }
    }

    /// Register the available actions for a node.
    pub fn set_actions(&mut self, node_id: NodeId, set: ActionSet) {
        self.action_sets.insert(node_id, set);
    }

    /// Register a callback for a node.
    pub fn set_callback(&mut self, node_id: NodeId, cb: ActionCallback) {
        self.callbacks.insert(node_id, cb);
    }

    /// Get the action set for a node.
    #[must_use]
    pub fn get_actions(&self, node_id: NodeId) -> Option<&ActionSet> {
        self.action_sets.get(&node_id)
    }

    /// Remove a node's actions and callback.
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.action_sets.remove(&node_id);
        self.callbacks.remove(&node_id);
    }

    /// Perform an action on a node.  Returns `true` if the action was
    /// handled, `false` if the node has no handler or does not support the
    /// action.
    pub fn perform_action(&self, node_id: NodeId, action: AccessibleAction) -> bool {
        // Check the action is in the node's set.
        if let Some(set) = self.action_sets.get(&node_id) {
            if !set.contains(action) {
                return false;
            }
        } else {
            return false;
        }

        // Dispatch to the callback.
        if let Some(cb) = self.callbacks.get(&node_id) {
            cb(node_id, action)
        } else {
            false
        }
    }

    /// Number of nodes with registered action sets.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.action_sets.len()
    }
}

impl Default for ActionHandler {
    fn default() -> Self {
        Self::new()
    }
}

// We can't derive Debug because ActionCallback is a trait object.
impl std::fmt::Debug for ActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionHandler")
            .field("action_sets", &self.action_sets)
            .field("callbacks", &format!("({} callbacks)", self.callbacks.len()))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

    #[test]
    fn action_set_add_contains() {
        let mut set = ActionSet::new();
        assert!(set.is_empty());
        set.add(AccessibleAction::Click);
        set.add(AccessibleAction::Focus);
        assert_eq!(set.len(), 2);
        assert!(set.contains(AccessibleAction::Click));
        assert!(set.contains(AccessibleAction::Focus));
        assert!(!set.contains(AccessibleAction::Dismiss));
    }

    #[test]
    fn action_set_no_duplicates() {
        let mut set = ActionSet::new();
        set.add(AccessibleAction::Click);
        set.add(AccessibleAction::Click);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn action_set_remove() {
        let mut set = ActionSet::new();
        set.add(AccessibleAction::Click);
        set.add(AccessibleAction::Focus);
        set.remove(AccessibleAction::Click);
        assert_eq!(set.len(), 1);
        assert!(!set.contains(AccessibleAction::Click));
    }

    #[test]
    fn action_set_from_actions() {
        let set = ActionSet::from_actions(&[
            AccessibleAction::Click,
            AccessibleAction::Expand,
            AccessibleAction::Collapse,
        ]);
        assert_eq!(set.len(), 3);
        assert!(set.contains(AccessibleAction::Expand));
    }

    #[test]
    fn action_set_iter() {
        let set = ActionSet::from_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);
        let collected: Vec<_> = set.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn action_display() {
        assert_eq!(format!("{}", AccessibleAction::Click), "Click");
        assert_eq!(format!("{}", AccessibleAction::ScrollTo), "ScrollTo");
    }

    #[test]
    fn perform_action_success() {
        let mut handler = ActionHandler::new();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);

        handler.set_actions(1, ActionSet::from_actions(&[AccessibleAction::Click]));
        handler.set_callback(1, Box::new(move |_id, _action| {
            fired_clone.store(true, Ordering::SeqCst);
            true
        }));

        assert!(handler.perform_action(1, AccessibleAction::Click));
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn perform_action_unsupported() {
        let mut handler = ActionHandler::new();
        handler.set_actions(1, ActionSet::from_actions(&[AccessibleAction::Click]));
        handler.set_callback(1, Box::new(|_id, _action| true));

        // Focus is not in the set.
        assert!(!handler.perform_action(1, AccessibleAction::Focus));
    }

    #[test]
    fn perform_action_no_node() {
        let handler = ActionHandler::new();
        assert!(!handler.perform_action(99, AccessibleAction::Click));
    }

    #[test]
    fn perform_action_no_callback() {
        let mut handler = ActionHandler::new();
        handler.set_actions(1, ActionSet::from_actions(&[AccessibleAction::Click]));
        // No callback registered.
        assert!(!handler.perform_action(1, AccessibleAction::Click));
    }

    #[test]
    fn remove_node_from_handler() {
        let mut handler = ActionHandler::new();
        handler.set_actions(1, ActionSet::from_actions(&[AccessibleAction::Click]));
        handler.set_callback(1, Box::new(|_id, _action| true));
        assert_eq!(handler.node_count(), 1);
        handler.remove_node(1);
        assert_eq!(handler.node_count(), 0);
        assert!(!handler.perform_action(1, AccessibleAction::Click));
    }

    #[test]
    fn action_handler_debug() {
        let handler = ActionHandler::new();
        let dbg = format!("{handler:?}");
        assert!(dbg.contains("ActionHandler"));
    }

    #[test]
    fn action_set_default() {
        let set = ActionSet::default();
        assert!(set.is_empty());
    }
}
