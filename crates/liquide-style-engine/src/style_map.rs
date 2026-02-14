//! Style map — computed styles for all nodes, with sharing.

use std::collections::HashMap;
use std::sync::Arc;

use liquide_dom::NodeId;

use crate::computed::ComputedStyle;

/// Computed styles for every node in the document.
pub struct StyleMap {
    styles: HashMap<NodeId, Arc<ComputedStyle>>,
}

impl StyleMap {
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
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
    }

    /// Clear all styles.
    pub fn clear(&mut self) {
        self.styles.clear();
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
