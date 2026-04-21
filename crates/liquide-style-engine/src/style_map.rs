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
    /// `::first-line` — applies to the first formatted line of a block container.
    FirstLine,
    /// `::first-letter` — applies to the first typographic letter of a block container.
    FirstLetter,
}

/// Computed styles for every node in the document.
#[derive(Clone)]
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
        self.pseudo_styles.remove(&(node_id, PseudoKind::FirstLine));
        self.pseudo_styles.remove(&(node_id, PseudoKind::FirstLetter));
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

    /// Get `::first-line` style overrides for a node.
    ///
    /// Per CSS spec, only a restricted set of properties apply to `::first-line`:
    /// font properties, color, background properties, text-decoration,
    /// letter-spacing, word-spacing, line-height, text-transform.
    pub fn get_first_line_overrides(
        &self,
        node_id: NodeId,
    ) -> Option<&Arc<ComputedStyle>> {
        self.pseudo_styles.get(&(node_id, PseudoKind::FirstLine))
    }

    /// Get `::first-letter` style overrides for a node.
    ///
    /// Per CSS spec, `::first-letter` accepts the same properties as
    /// `::first-line` plus: margin, padding, border, and float.
    pub fn get_first_letter_overrides(
        &self,
        node_id: NodeId,
    ) -> Option<&Arc<ComputedStyle>> {
        self.pseudo_styles.get(&(node_id, PseudoKind::FirstLetter))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64) -> NodeId {
        id
    }

    #[test]
    fn first_line_stored_and_retrieved() {
        let mut map = StyleMap::new();
        let n = node(1);
        let style = Arc::new(ComputedStyle::default());
        map.insert_pseudo(n, PseudoKind::FirstLine, style.clone());

        assert!(map.get_first_line_overrides(n).is_some());
        assert!(map.get_first_letter_overrides(n).is_none());
        assert!(map.get_pseudo(n, PseudoKind::FirstLine).is_some());
    }

    #[test]
    fn first_letter_stored_and_retrieved() {
        let mut map = StyleMap::new();
        let n = node(2);
        let style = Arc::new(ComputedStyle::default());
        map.insert_pseudo(n, PseudoKind::FirstLetter, style.clone());

        assert!(map.get_first_letter_overrides(n).is_some());
        assert!(map.get_first_line_overrides(n).is_none());
        assert!(map.get_pseudo(n, PseudoKind::FirstLetter).is_some());
    }

    #[test]
    fn remove_clears_first_line_and_first_letter() {
        let mut map = StyleMap::new();
        let n = node(3);
        map.insert(n, ComputedStyle::default());
        map.insert_pseudo(n, PseudoKind::Before, Arc::new(ComputedStyle::default()));
        map.insert_pseudo(n, PseudoKind::After, Arc::new(ComputedStyle::default()));
        map.insert_pseudo(n, PseudoKind::FirstLine, Arc::new(ComputedStyle::default()));
        map.insert_pseudo(n, PseudoKind::FirstLetter, Arc::new(ComputedStyle::default()));

        map.remove(n);

        assert!(map.get(n).is_none());
        assert!(map.get_pseudo(n, PseudoKind::Before).is_none());
        assert!(map.get_pseudo(n, PseudoKind::After).is_none());
        assert!(map.get_pseudo(n, PseudoKind::FirstLine).is_none());
        assert!(map.get_pseudo(n, PseudoKind::FirstLetter).is_none());
    }

    #[test]
    fn pseudo_iter_includes_first_line_and_letter() {
        let mut map = StyleMap::new();
        let n = node(4);
        map.insert_pseudo(n, PseudoKind::FirstLine, Arc::new(ComputedStyle::default()));
        map.insert_pseudo(n, PseudoKind::FirstLetter, Arc::new(ComputedStyle::default()));

        let kinds: Vec<PseudoKind> = map.pseudo_iter().map(|((_, k), _)| *k).collect();
        assert!(kinds.contains(&PseudoKind::FirstLine));
        assert!(kinds.contains(&PseudoKind::FirstLetter));
    }
}
