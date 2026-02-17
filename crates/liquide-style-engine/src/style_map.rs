//! Style map — computed styles for all nodes, with sharing.

use std::collections::HashMap;
use std::sync::Arc;

use liquide_dom::NodeId;

use crate::computed::ComputedStyle;

/// Which pseudo-element we're referring to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoKind {
    Before,
    After,
}

/// Computed styles for every node in the document.
pub struct StyleMap {
    styles: HashMap<NodeId, Arc<ComputedStyle>>,
    /// Pseudo-element styles: (host_node, kind) → computed style.
    pseudo_styles: HashMap<(NodeId, PseudoKind), Arc<ComputedStyle>>,
    /// Resolved container sizes for container query evaluation.
    /// Populated by the layout engine after layout for nodes with
    /// `container-type` != `normal`.  Used by the style engine to
    /// evaluate `@container` rules with real dimensions instead of
    /// falling back to the viewport.
    container_sizes: HashMap<NodeId, (f32, f32)>,
}

impl StyleMap {
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
            pseudo_styles: HashMap::new(),
            container_sizes: HashMap::new(),
        }
    }

    /// Get the computed style for a node.
    pub fn get(&self, node_id: NodeId) -> Option<&Arc<ComputedStyle>> {
        self.styles.get(&node_id)
    }

    /// Insert or replace a computed style.
    pub fn insert(&mut self, node_id: NodeId, style: ComputedStyle) {
        self.styles.insert(node_id, Arc::new(style));
    }

    /// Insert a shared (Arc'd) style.
    pub fn insert_shared(&mut self, node_id: NodeId, style: Arc<ComputedStyle>) {
        self.styles.insert(node_id, style);
    }

    /// Remove a node's style.
    pub fn remove(&mut self, node_id: NodeId) {
        self.styles.remove(&node_id);
        self.pseudo_styles.remove(&(node_id, PseudoKind::Before));
        self.pseudo_styles.remove(&(node_id, PseudoKind::After));
        self.container_sizes.remove(&node_id);
    }

    /// Clear all styles.
    pub fn clear(&mut self) {
        self.styles.clear();
        self.pseudo_styles.clear();
        self.container_sizes.clear();
    }

    /// Insert a pseudo-element style for a host node.
    pub fn insert_pseudo(&mut self, node_id: NodeId, kind: PseudoKind, style: Arc<ComputedStyle>) {
        self.pseudo_styles.insert((node_id, kind), style);
    }

    /// Get the pseudo-element style for a host node.
    pub fn get_pseudo(&self, node_id: NodeId, kind: PseudoKind) -> Option<&Arc<ComputedStyle>> {
        self.pseudo_styles.get(&(node_id, kind))
    }

    /// Iterate all pseudo-element styles.
    pub fn pseudo_iter(
        &self,
    ) -> impl Iterator<Item = (&(NodeId, PseudoKind), &Arc<ComputedStyle>)> {
        self.pseudo_styles.iter()
    }

    /// Set the resolved container dimensions for a container query host node.
    /// Called by the layout engine after computing box sizes.
    pub fn set_container_size(&mut self, node_id: NodeId, width: f32, height: f32) {
        self.container_sizes.insert(node_id, (width, height));
    }

    /// Get the resolved container dimensions for a container query host node.
    pub fn container_size(&self, node_id: NodeId) -> Option<(f32, f32)> {
        self.container_sizes.get(&node_id).copied()
    }

    /// Number of styled nodes.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Is the map empty?
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Iterate all (node, style) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &Arc<ComputedStyle>)> {
        self.styles.iter()
    }
}

impl Default for StyleMap {
    fn default() -> Self {
        Self::new()
    }
}
