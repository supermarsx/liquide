//! DOM tree visitor and mutation observer traits.

use crate::NodeId;
use crate::class_list::ClassList;

/// Depth-first tree visitor.
pub trait Visitor {
    /// Called when entering a node (pre-order). Return `false` to skip children.
    fn enter(&mut self, node_id: NodeId) -> bool;
    /// Called when leaving a node (post-order).
    fn leave(&mut self, node_id: NodeId);
}

/// Observer for DOM mutations. Used by the style engine, layout engine,
/// and accessibility tree to react to changes.
pub trait MutationObserver: Send {
    /// A child was appended to a parent.
    fn on_child_added(&mut self, parent: NodeId, child: NodeId);
    /// A child was removed from a parent.
    fn on_child_removed(&mut self, parent: NodeId, child: NodeId);
    /// An attribute was changed on a node.
    fn on_attribute_changed(
        &mut self,
        node: NodeId,
        attr: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
    );
    /// The class list was changed on a node.
    fn on_class_changed(&mut self, node: NodeId, classes: &ClassList);
    /// The text content was changed on a node.
    fn on_text_changed(&mut self, node: NodeId, text: &str);
    /// A pseudo-state was changed on a node.
    fn on_pseudo_state_changed(
        &mut self,
        node: NodeId,
        old_state: crate::pseudo::PseudoStateFlags,
        new_state: crate::pseudo::PseudoStateFlags,
    );
    /// The element ID was changed.
    fn on_id_changed(&mut self, node: NodeId, old_id: Option<&str>, new_id: Option<&str>);
}

/// A no-op mutation observer (useful as default).
pub struct NullObserver;

impl MutationObserver for NullObserver {
    fn on_child_added(&mut self, _parent: NodeId, _child: NodeId) {}
    fn on_child_removed(&mut self, _parent: NodeId, _child: NodeId) {}
    fn on_attribute_changed(&mut self, _: NodeId, _: &str, _: Option<&str>, _: Option<&str>) {}
    fn on_class_changed(&mut self, _: NodeId, _: &ClassList) {}
    fn on_text_changed(&mut self, _: NodeId, _: &str) {}
    fn on_pseudo_state_changed(
        &mut self,
        _: NodeId,
        _: crate::pseudo::PseudoStateFlags,
        _: crate::pseudo::PseudoStateFlags,
    ) {
    }
    fn on_id_changed(&mut self, _: NodeId, _: Option<&str>, _: Option<&str>) {}
}
