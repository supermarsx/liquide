//! The Document — the full DOM tree with index lookups and mutation dispatch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dirty::DirtySet;
use crate::node::{Node, NodeData, NodeId};
use crate::pseudo::PseudoStateFlags;
use crate::tag::Tag;
use crate::visitor::{MutationObserver, Visitor};

/// The DOM document. Owns all nodes and provides tree manipulation.
pub struct Document {
    /// All nodes by ID.
    nodes: HashMap<NodeId, Node>,
    /// The root node ID.
    root: NodeId,
    /// Fast lookup: element_id → NodeId.
    id_index: HashMap<String, NodeId>,
    /// Fast lookup: class → Vec<NodeId>.
    class_index: HashMap<String, Vec<NodeId>>,
    /// Monotonic node ID counter.
    next_id: AtomicU64,
    /// Dirty tracking for incremental processing.
    pub dirty: DirtySet,
    /// Mutation observers.
    observers: Vec<Box<dyn MutationObserver>>,
}

impl Document {
    /// Create a new document with an empty root element.
    pub fn new() -> Self {
        let root_id = 1u64;
        let mut root = Node::new_element(root_id, Tag::root());
        root.pseudo_states = PseudoStateFlags::ROOT;

        let mut nodes = HashMap::new();
        nodes.insert(root_id, root);

        Self {
            nodes,
            root: root_id,
            id_index: HashMap::new(),
            class_index: HashMap::new(),
            next_id: AtomicU64::new(2),
            dirty: DirtySet::new(),
            observers: Vec::new(),
        }
    }

    /// Get the root node ID.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Allocate a new unique node ID.
    fn alloc_id(&self) -> NodeId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    // -----------------------------------------------------------------------
    // Node creation
    // -----------------------------------------------------------------------

    /// Create an element node (detached — call `append_child` to attach).
    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let id = self.alloc_id();
        let node = Node::new_element(id, Tag::intern(tag));
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Create a text node (detached).
    pub fn create_text(&mut self, text: &str) -> NodeId {
        let id = self.alloc_id();
        let node = Node::new_text(id, text);
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Create a comment node (detached).
    pub fn create_comment(&mut self, data: &str) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            tag: Tag::intern("#comment"),
            parent: None,
            children: Vec::new(),
            attrs: crate::attrs::AttributeMap::new(),
            inline_styles: crate::attrs::AttributeMap::new(),
            classes: crate::class_list::ClassList::new(),
            element_id: None,
            pseudo_states: PseudoStateFlags::empty(),
            data: crate::node::NodeData::Comment(data.to_string()),
            dirty: crate::dirty::DirtyFlags::all_dirty(),
        };
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Create a document fragment (lightweight detached container).
    pub fn create_document_fragment(&mut self) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            tag: Tag::intern("#document-fragment"),
            parent: None,
            children: Vec::new(),
            attrs: crate::attrs::AttributeMap::new(),
            inline_styles: crate::attrs::AttributeMap::new(),
            classes: crate::class_list::ClassList::new(),
            element_id: None,
            pseudo_states: PseudoStateFlags::empty(),
            data: crate::node::NodeData::DocumentFragment,
            dirty: crate::dirty::DirtyFlags::all_dirty(),
        };
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Create a shadow root node (detached).
    ///
    /// Attach to a host element with `append_child` to form a shadow tree.
    pub fn create_shadow_root(&mut self) -> NodeId {
        let id = self.alloc_id();
        let node = Node {
            id,
            tag: Tag::intern("#shadow-root"),
            parent: None,
            children: Vec::new(),
            attrs: crate::attrs::AttributeMap::new(),
            inline_styles: crate::attrs::AttributeMap::new(),
            classes: crate::class_list::ClassList::new(),
            element_id: None,
            pseudo_states: PseudoStateFlags::empty(),
            data: crate::node::NodeData::ShadowRoot,
            dirty: crate::dirty::DirtyFlags::all_dirty(),
        };
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Create a pseudo-element node (detached).
    ///
    /// The node is inserted as a synthetic DOM child during style resolution.
    /// `::before` is prepended, `::after` is appended to the originating element.
    pub fn create_pseudo_element(
        &mut self,
        pseudo_type: crate::node::PseudoType,
        content: &str,
    ) -> NodeId {
        let id = self.alloc_id();
        let node = Node::new_pseudo_element(id, pseudo_type, content);
        self.nodes.insert(id, node);
        self.dirty.mark_style(id);
        id
    }

    /// Insert a child as the first child of a parent.
    pub fn prepend_child(&mut self, parent: NodeId, child: NodeId) {
        // Detach from previous parent if needed
        if let Some(old_parent) = self.nodes.get(&child).and_then(|n| n.parent) {
            if let Some(p) = self.nodes.get_mut(&old_parent) {
                p.children.retain(|&c| c != child);
            } else {
                eprintln!(
                    "prepend_child: old parent {:?} for child {:?} not found in nodes",
                    old_parent, child,
                );
            }
        }

        // Set new parent
        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = Some(parent);
        }
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.insert(0, child);
        }
    }

    /// Remove all pseudo-element children from a node.
    pub fn remove_pseudo_elements(&mut self, parent: NodeId) {
        let pseudo_ids: Vec<NodeId> = self
            .children(parent)
            .iter()
            .copied()
            .filter(|&cid| {
                self.get(cid)
                    .map(|n| n.is_pseudo_element())
                    .unwrap_or(false)
            })
            .collect();
        for pid in pseudo_ids {
            self.remove_child(parent, pid);
        }
    }

    // -----------------------------------------------------------------------
    // Tree mutation
    // -----------------------------------------------------------------------

    /// Append a child to a parent.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Detach from previous parent if needed
        if let Some(old_parent) = self.nodes.get(&child).and_then(|n| n.parent) {
            if let Some(p) = self.nodes.get_mut(&old_parent) {
                p.children.retain(|&c| c != child);
            } else {
                eprintln!(
                    "append_child: old parent {:?} for child {:?} not found in nodes",
                    old_parent, child,
                );
            }
        }

        // Set new parent
        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = Some(parent);
            node.dirty.mark_style_dirty();
        }
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.push(child);
            node.dirty.mark_layout_dirty();
        }

        // Update structural pseudo-states
        self.update_child_pseudo_states(parent);

        self.dirty.mark_layout(parent);

        // Notify observers
        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_child_added(parent, child);
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_child_added on node {:?}",
                    parent,
                );
            }
        }
    }

    /// Insert a child before a reference node.
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, before: NodeId) {
        // Detach from previous parent
        if let Some(old_parent) = self.nodes.get(&child).and_then(|n| n.parent) {
            if let Some(p) = self.nodes.get_mut(&old_parent) {
                p.children.retain(|&c| c != child);
            } else {
                eprintln!(
                    "insert_before: old parent {:?} for child {:?} not found in nodes",
                    old_parent, child,
                );
            }
        }

        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = Some(parent);
            node.dirty.mark_style_dirty();
        }
        if let Some(node) = self.nodes.get_mut(&parent) {
            if let Some(pos) = node.children.iter().position(|&c| c == before) {
                node.children.insert(pos, child);
            } else {
                node.children.push(child);
            }
            node.dirty.mark_layout_dirty();
        }

        self.update_child_pseudo_states(parent);
        self.dirty.mark_layout(parent);

        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_child_added(parent, child);
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_child_added on node {:?}",
                    parent,
                );
            }
        }
    }

    /// Remove a child from its parent. The node is NOT destroyed —
    /// call `destroy_node` to free it.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        if let Some(node) = self.nodes.get_mut(&parent) {
            node.children.retain(|&c| c != child);
            node.dirty.mark_layout_dirty();
        }
        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = None;
        }

        self.update_child_pseudo_states(parent);
        self.dirty.mark_layout(parent);

        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_child_removed(parent, child);
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_child_removed on node {:?}",
                    parent,
                );
            }
        }
    }

    /// Destroy a node and all its descendants, freeing memory.
    pub fn destroy_node(&mut self, node_id: NodeId) {
        // Collect descendants
        let mut to_remove = Vec::new();
        self.collect_descendants(node_id, &mut to_remove);
        to_remove.push(node_id);

        for id in &to_remove {
            if let Some(node) = self.nodes.remove(id) {
                // Remove from id index
                if let Some(ref eid) = node.element_id {
                    self.id_index.remove(eid);
                }
                // Remove from class index
                for class in node.classes.iter() {
                    if let Some(list) = self.class_index.get_mut(class) {
                        list.retain(|&nid| nid != *id);
                    }
                }
            }
            // Clear from document-level dirty tracking so destroyed nodes
            // do not leak into subsequent style/layout/paint passes.
            self.dirty.remove(*id);
        }
    }

    fn collect_descendants(&self, node_id: NodeId, out: &mut Vec<NodeId>) {
        if let Some(node) = self.nodes.get(&node_id) {
            for &child in &node.children {
                out.push(child);
                self.collect_descendants(child, out);
            }
        }
    }

    /// Mark every live node in the document as needing style recalculation.
    ///
    /// Used by theme hot-reload (`ThemeWatcher`) after the query cache is
    /// cleared so the next frame re-queries every element's style.
    pub fn mark_style_all(&mut self) {
        let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        for id in ids {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.dirty.mark_style_dirty();
            }
            self.dirty.mark_style(id);
        }
    }

    // -----------------------------------------------------------------------
    // Attribute manipulation
    // -----------------------------------------------------------------------

    /// Set an attribute on a node.
    pub fn set_attribute(&mut self, node_id: NodeId, key: &str, value: &str) {
        let old_value = if let Some(node) = self.nodes.get_mut(&node_id) {
            let old = node.attrs.get(key).map(String::from);
            node.attrs.set(key, value);
            node.dirty.mark_style_dirty();
            old
        } else {
            return;
        };

        self.dirty.mark_style(node_id);

        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_attribute_changed(node_id, key, old_value.as_deref(), Some(value));
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_attribute_changed on node {:?}",
                    node_id,
                );
            }
        }
    }

    /// Get an attribute value.
    pub fn get_attribute(&self, node_id: NodeId, key: &str) -> Option<String> {
        self.nodes.get(&node_id)?.attrs.get(key).map(String::from)
    }

    /// Remove an attribute from a node.
    pub fn remove_attribute(&mut self, node_id: NodeId, key: &str) {
        let old_value = if let Some(node) = self.nodes.get_mut(&node_id) {
            let old = node.attrs.remove(key);
            if old.is_some() {
                node.dirty.mark_style_dirty();
            }
            old
        } else {
            return;
        };

        if old_value.is_some() {
            self.dirty.mark_style(node_id);
            for obs in &mut self.observers {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    obs.on_attribute_changed(node_id, key, old_value.as_deref(), None);
                }));
                if result.is_err() {
                    eprintln!(
                        "mutation observer panicked during on_attribute_changed on node {:?}",
                        node_id,
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Inline style manipulation
    // -----------------------------------------------------------------------

    /// Set an inline CSS style property on a node.
    /// Inline styles have the highest specificity, overriding CSS rules.
    pub fn set_inline_style(&mut self, node_id: NodeId, property: &str, value: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.inline_styles.set(property, value);
            node.dirty.mark_style_dirty();
        }
        self.dirty.mark_style(node_id);
    }

    /// Remove an inline style property.
    pub fn remove_inline_style(&mut self, node_id: NodeId, property: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if node.inline_styles.remove(property).is_some() {
                node.dirty.mark_style_dirty();
            }
        }
        self.dirty.mark_style(node_id);
    }

    /// Clear all inline styles from a node.
    pub fn clear_inline_styles(&mut self, node_id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if !node.inline_styles.is_empty() {
                node.inline_styles.clear();
                node.dirty.mark_style_dirty();
            }
        }
        self.dirty.mark_style(node_id);
    }

    /// Get an inline style value.
    pub fn get_inline_style(&self, node_id: NodeId, property: &str) -> Option<String> {
        self.nodes
            .get(&node_id)?
            .inline_styles
            .get(property)
            .map(String::from)
    }

    // -----------------------------------------------------------------------
    // Class manipulation
    // -----------------------------------------------------------------------

    /// Add a CSS class to a node.
    pub fn add_class(&mut self, node_id: NodeId, class: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if node.classes.contains(class) {
                return; // Already has it
            }
            node.classes.add(class);
            node.dirty.mark_style_dirty();
            let classes = node.classes.clone();

            // Update class index
            self.class_index
                .entry(class.to_string())
                .or_default()
                .push(node_id);

            self.dirty.mark_style(node_id);

            for obs in &mut self.observers {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    obs.on_class_changed(node_id, &classes);
                }));
                if result.is_err() {
                    eprintln!(
                        "mutation observer panicked during on_class_changed on node {:?}",
                        node_id,
                    );
                }
            }
        }
    }

    /// Remove a CSS class from a node.
    pub fn remove_class(&mut self, node_id: NodeId, class: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if !node.classes.remove(class) {
                return; // Didn't have it
            }
            node.dirty.mark_style_dirty();
            let classes = node.classes.clone();

            // Update class index
            if let Some(list) = self.class_index.get_mut(class) {
                list.retain(|&nid| nid != node_id);
            }

            self.dirty.mark_style(node_id);

            for obs in &mut self.observers {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    obs.on_class_changed(node_id, &classes);
                }));
                if result.is_err() {
                    eprintln!(
                        "mutation observer panicked during on_class_changed on node {:?}",
                        node_id,
                    );
                }
            }
        }
    }

    /// Toggle a CSS class.
    pub fn toggle_class(&mut self, node_id: NodeId, class: &str) -> bool {
        let has_class = self
            .nodes
            .get(&node_id)
            .map_or(false, |n| n.classes.contains(class));
        if has_class {
            self.remove_class(node_id, class);
            false
        } else {
            self.add_class(node_id, class);
            true
        }
    }

    // -----------------------------------------------------------------------
    // ID manipulation
    // -----------------------------------------------------------------------

    /// Set the element ID (for `#id` selectors).
    ///
    /// Passing an empty string clears the ID without inserting an empty key
    /// into the index.
    pub fn set_id(&mut self, node_id: NodeId, id: &str) {
        let old_id = if let Some(node) = self.nodes.get_mut(&node_id) {
            let old = node.element_id.take();
            if id.is_empty() {
                // Clear only — do not store an empty element_id
            } else {
                node.element_id = Some(id.to_string());
            }
            node.dirty.mark_style_dirty();
            old
        } else {
            return;
        };

        // Update index — remove the old entry
        if let Some(ref old) = old_id {
            self.id_index.remove(old);
        }
        // Only insert a non-empty id into the index
        if !id.is_empty() {
            self.id_index.insert(id.to_string(), node_id);
        }

        self.dirty.mark_style(node_id);

        let new_id = if id.is_empty() { None } else { Some(id) };
        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_id_changed(node_id, old_id.as_deref(), new_id);
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_id_changed on node {:?}",
                    node_id,
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Text content
    // -----------------------------------------------------------------------

    /// Set text content of a text node.
    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.data = NodeData::Text(text.to_string());
            node.dirty.mark_layout_dirty();
        }

        self.dirty.mark_layout(node_id);

        for obs in &mut self.observers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                obs.on_text_changed(node_id, text);
            }));
            if result.is_err() {
                eprintln!(
                    "mutation observer panicked during on_text_changed on node {:?}",
                    node_id,
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pseudo-state
    // -----------------------------------------------------------------------

    /// Set or clear a pseudo-state flag on a node.
    pub fn set_pseudo_state(&mut self, node_id: NodeId, flag: PseudoStateFlags, active: bool) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            let old = node.pseudo_states;
            if active {
                node.pseudo_states |= flag;
            } else {
                node.pseudo_states &= !flag;
            }
            let new = node.pseudo_states;
            if old != new {
                node.dirty.mark_style_dirty();
                self.dirty.mark_style(node_id);

                for obs in &mut self.observers {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        obs.on_pseudo_state_changed(node_id, old, new);
                    }));
                    if result.is_err() {
                        eprintln!(
                            "mutation observer panicked during on_pseudo_state_changed on node {:?}",
                            node_id,
                        );
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    /// Get a node by ID.
    pub fn get(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.get(&node_id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, node_id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&node_id)
    }

    /// Get the tag name of an element node.
    ///
    /// Returns the lowercase tag name (e.g., "div", "devtools-tab").
    /// Returns `None` if the node doesn't exist.
    pub fn tag_name(&self, node_id: NodeId) -> Option<String> {
        self.nodes.get(&node_id).map(|n| n.tag_name())
    }

    /// Get an element by its `id` attribute.
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.id_index.get(id).copied()
    }

    /// Get all elements with a given class.
    pub fn get_elements_by_class(&self, class: &str) -> &[NodeId] {
        self.class_index
            .get(class)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get the parent of a node.
    pub fn parent(&self, node_id: NodeId) -> Option<NodeId> {
        self.nodes.get(&node_id)?.parent
    }

    /// Get children of a node.
    pub fn children(&self, node_id: NodeId) -> &[NodeId] {
        self.nodes
            .get(&node_id)
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Iterate ancestors (parent, grandparent, ...) up to but not including root.
    pub fn ancestors(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut current = self.nodes.get(&node_id).and_then(|n| n.parent);
        while let Some(pid) = current {
            result.push(pid);
            current = self.nodes.get(&pid).and_then(|n| n.parent);
        }
        result
    }

    /// Total number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    // -----------------------------------------------------------------------
    // Tree walking
    // -----------------------------------------------------------------------

    /// Walk the subtree rooted at `node_id` in depth-first pre-order.
    pub fn walk(&self, node_id: NodeId, visitor: &mut dyn Visitor) {
        if !visitor.enter(node_id) {
            visitor.leave(node_id);
            return;
        }
        if let Some(node) = self.nodes.get(&node_id) {
            let children = node.children.clone();
            for child in children {
                self.walk(child, visitor);
            }
        }
        visitor.leave(node_id);
    }

    /// Collect all descendant node IDs in pre-order.
    pub fn descendants(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.descendants_inner(node_id, &mut result);
        result
    }

    fn descendants_inner(&self, node_id: NodeId, out: &mut Vec<NodeId>) {
        if let Some(node) = self.nodes.get(&node_id) {
            for &child in &node.children {
                out.push(child);
                self.descendants_inner(child, out);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Observers
    // -----------------------------------------------------------------------

    /// Register a mutation observer.
    pub fn add_observer(&mut self, observer: Box<dyn MutationObserver>) {
        self.observers.push(observer);
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Update `:first-child`, `:last-child`, `:only-child`, and `:empty`
    /// pseudo-states for a parent's children.
    fn update_child_pseudo_states(&mut self, parent: NodeId) {
        let children = self
            .nodes
            .get(&parent)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        let total = children.len();
        for (i, &child_id) in children.iter().enumerate() {
            if let Some(child) = self.nodes.get_mut(&child_id) {
                let is_first = i == 0;
                let is_last = i == total - 1;
                let is_only = total == 1;

                if is_first {
                    child.pseudo_states |= PseudoStateFlags::FIRST_CHILD;
                } else {
                    child.pseudo_states &= !PseudoStateFlags::FIRST_CHILD;
                }

                if is_last {
                    child.pseudo_states |= PseudoStateFlags::LAST_CHILD;
                } else {
                    child.pseudo_states &= !PseudoStateFlags::LAST_CHILD;
                }

                if is_only {
                    child.pseudo_states |= PseudoStateFlags::ONLY_CHILD;
                } else {
                    child.pseudo_states &= !PseudoStateFlags::ONLY_CHILD;
                }

                // :empty
                let is_empty = child.children.is_empty()
                    && !matches!(child.data, NodeData::Text(ref s) if !s.is_empty());
                if is_empty {
                    child.pseudo_states |= PseudoStateFlags::EMPTY;
                } else {
                    child.pseudo_states &= !PseudoStateFlags::EMPTY;
                }
            }
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_append() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        assert_eq!(doc.children(root), &[div]);
        assert_eq!(doc.parent(div), Some(root));
    }

    #[test]
    fn text_content() {
        let mut doc = Document::new();
        let txt = doc.create_text("Hello");
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("Hello"));

        doc.set_text_content(txt, "World");
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("World"));
    }

    #[test]
    fn class_operations() {
        let mut doc = Document::new();
        let el = doc.create_element("button");
        doc.add_class(el, "primary");
        doc.add_class(el, "active");

        let node = doc.get(el).unwrap();
        assert!(node.has_class("primary"));
        assert!(node.has_class("active"));
        assert!(!node.has_class("disabled"));

        assert_eq!(doc.get_elements_by_class("primary"), &[el]);
    }

    #[test]
    fn id_operations() {
        let mut doc = Document::new();
        let el = doc.create_element("dock");
        doc.set_id(el, "main-dock");

        assert_eq!(doc.get_element_by_id("main-dock"), Some(el));
        assert_eq!(doc.get_element_by_id("nope"), None);
    }

    #[test]
    fn pseudo_state_management() {
        let mut doc = Document::new();
        let el = doc.create_element("button");
        doc.set_pseudo_state(el, PseudoStateFlags::HOVER, true);

        assert!(
            doc.get(el)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );

        doc.set_pseudo_state(el, PseudoStateFlags::HOVER, false);
        assert!(
            !doc.get(el)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::HOVER)
        );
    }

    #[test]
    fn child_pseudo_states_first_last() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let c = doc.create_element("c");
        doc.append_child(root, a);
        doc.append_child(root, b);
        doc.append_child(root, c);

        assert!(
            doc.get(a)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::FIRST_CHILD)
        );
        assert!(
            !doc.get(a)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::LAST_CHILD)
        );
        assert!(
            !doc.get(b)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::FIRST_CHILD)
        );
        assert!(
            !doc.get(b)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::LAST_CHILD)
        );
        assert!(
            !doc.get(c)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::FIRST_CHILD)
        );
        assert!(
            doc.get(c)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::LAST_CHILD)
        );
    }

    #[test]
    fn child_pseudo_state_only_child() {
        let mut doc = Document::new();
        let root = doc.root();
        let solo = doc.create_element("solo");
        doc.append_child(root, solo);
        // A single child is simultaneously first, last, and only.
        let node = doc.get(solo).unwrap();
        assert!(node.has_pseudo_state(PseudoStateFlags::FIRST_CHILD));
        assert!(node.has_pseudo_state(PseudoStateFlags::LAST_CHILD));
        assert!(node.has_pseudo_state(PseudoStateFlags::ONLY_CHILD));

        // Adding a sibling clears :only-child on both.
        let sibling = doc.create_element("sibling");
        doc.append_child(root, sibling);
        assert!(
            !doc.get(solo)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::ONLY_CHILD)
        );
        assert!(
            !doc.get(sibling)
                .unwrap()
                .has_pseudo_state(PseudoStateFlags::ONLY_CHILD)
        );
    }

    #[test]
    fn remove_child() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        doc.append_child(root, a);
        doc.append_child(root, b);
        doc.remove_child(root, a);

        assert_eq!(doc.children(root), &[b]);
        assert_eq!(doc.parent(a), None);
    }

    #[test]
    fn destroy_node_recursive() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("div");
        let child = doc.create_element("span");
        let grandchild = doc.create_element("em");
        doc.append_child(root, parent);
        doc.append_child(parent, child);
        doc.append_child(child, grandchild);

        let count_before = doc.node_count();
        doc.remove_child(root, parent);
        doc.destroy_node(parent);
        assert_eq!(doc.node_count(), count_before - 3);
    }

    #[test]
    fn ancestors() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        doc.append_child(root, a);
        doc.append_child(a, b);

        let anc = doc.ancestors(b);
        assert_eq!(anc, vec![a, root]);
    }

    #[test]
    fn descendants() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("a");
        let b = doc.create_element("b");
        let c = doc.create_element("c");
        doc.append_child(root, a);
        doc.append_child(a, b);
        doc.append_child(a, c);

        let desc = doc.descendants(root);
        assert_eq!(desc, vec![a, b, c]);
    }

    #[test]
    fn insert_before() {
        let mut doc = Document::new();
        let root = doc.root();
        let a = doc.create_element("a");
        let c = doc.create_element("c");
        let b = doc.create_element("b");
        doc.append_child(root, a);
        doc.append_child(root, c);
        doc.insert_before(root, b, c);

        assert_eq!(doc.children(root), &[a, b, c]);
    }

    #[test]
    fn dirty_tracking() {
        let mut doc = Document::new();
        let root = doc.root();
        let el = doc.create_element("div");
        doc.append_child(root, el);

        assert!(doc.dirty.has_work());
        doc.dirty.clear_all();
        assert!(!doc.dirty.has_work());

        doc.add_class(el, "foo");
        assert!(doc.dirty.style.contains(&el));
    }

    #[test]
    fn attribute_ops() {
        let mut doc = Document::new();
        let el = doc.create_element("img");
        doc.set_attribute(el, "src", "/wallpaper.png");
        assert_eq!(
            doc.get_attribute(el, "src"),
            Some("/wallpaper.png".to_string())
        );

        doc.remove_attribute(el, "src");
        assert_eq!(doc.get_attribute(el, "src"), None);
    }
}
