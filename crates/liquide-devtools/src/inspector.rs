//! DOM tree inspector — browse the live element tree with expand/collapse,
//! search, attribute display, and hover-to-highlight.
//!
//! The inspector builds a lightweight snapshot of the DOM tree and exposes
//! it as a serializable structure for rendering in the dev-tools panel.

use liquide_dom::{Document, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A snapshot of a single DOM node for display in the inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorNode {
    /// DOM node ID.
    pub id: NodeId,
    /// Tag name (e.g., "div", "dock-item").
    pub tag: String,
    /// Element ID attribute (e.g., `id="statusbar"`).
    pub element_id: Option<String>,
    /// CSS classes on this element.
    pub classes: Vec<String>,
    /// Key attributes (filtered to most useful ones).
    pub attributes: Vec<(String, String)>,
    /// Text content (for text nodes, truncated).
    pub text: Option<String>,
    /// Whether this is a text node.
    pub is_text: bool,
    /// Child count (for expand/collapse UI).
    pub child_count: usize,
    /// Children (populated when expanded).
    pub children: Vec<InspectorNode>,
    /// Nesting depth.
    pub depth: u32,
    /// Pseudo-state flags as human-readable strings.
    pub pseudo_states: Vec<String>,
}

/// Manages the inspector state: which nodes are expanded, selected, etc.
pub struct ElementTreeInspector {
    /// Set of expanded node IDs.
    expanded: HashSet<NodeId>,
    /// Currently selected node (for style panel).
    selected: Option<NodeId>,
    /// Currently hovered node (for highlight overlay).
    hovered: Option<NodeId>,
    /// Search filter string.
    search_query: String,
    /// Cached tree snapshot.
    snapshot: Option<InspectorNode>,
    /// Maximum depth to auto-expand on initial load.
    auto_expand_depth: u32,
}

impl ElementTreeInspector {
    /// Create a new inspector.
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            selected: None,
            hovered: None,
            search_query: String::new(),
            snapshot: None,
            auto_expand_depth: 2,
        }
    }

    /// Build a snapshot of the DOM tree from the document.
    pub fn build_snapshot(&mut self, doc: &Document) -> &InspectorNode {
        let root_id = doc.root();

        let snapshot = self.build_node_snapshot(doc, root_id, 0);
        self.snapshot = Some(snapshot);
        self.snapshot.as_ref().unwrap()
    }

    /// Recursively build a node snapshot.
    fn build_node_snapshot(&self, doc: &Document, node_id: NodeId, depth: u32) -> InspectorNode {
        let node = match doc.get(node_id) {
            Some(n) => n,
            None => {
                return InspectorNode {
                    id: node_id,
                    tag: "<missing>".into(),
                    element_id: None,
                    classes: vec![],
                    attributes: vec![],
                    text: None,
                    is_text: false,
                    child_count: 0,
                    children: vec![],
                    depth,
                    pseudo_states: vec![],
                };
            }
        };

        let is_text = node.is_text();
        let text = node.text_content().map(|t| {
            if t.len() > 80 {
                format!("{}...", &t[..77])
            } else {
                t.to_string()
            }
        });

        // Collect key attributes (skip internal ones).
        let attributes: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| !k.starts_with("__"))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Decode pseudo-states to readable strings.
        let mut pseudo_states = Vec::new();
        let ps = node.pseudo_states;
        if ps.contains(liquide_dom::PseudoStateFlags::HOVER) {
            pseudo_states.push(":hover".into());
        }
        if ps.contains(liquide_dom::PseudoStateFlags::ACTIVE) {
            pseudo_states.push(":active".into());
        }
        if ps.contains(liquide_dom::PseudoStateFlags::FOCUS) {
            pseudo_states.push(":focus".into());
        }
        if ps.contains(liquide_dom::PseudoStateFlags::FOCUS_VISIBLE) {
            pseudo_states.push(":focus-visible".into());
        }
        if ps.contains(liquide_dom::PseudoStateFlags::FOCUS_WITHIN) {
            pseudo_states.push(":focus-within".into());
        }

        let child_ids: Vec<NodeId> = node.children.clone();
        let child_count = child_ids.len();

        // Expand children if this node is expanded or within auto-expand depth.
        let show_children = self.expanded.contains(&node_id) || depth < self.auto_expand_depth;

        let children = if show_children {
            child_ids
                .iter()
                .map(|&cid| self.build_node_snapshot(doc, cid, depth + 1))
                .collect()
        } else {
            vec![]
        };

        InspectorNode {
            id: node_id,
            tag: node.tag.as_str().to_string(),
            element_id: node.element_id.clone(),
            classes: node.classes.iter().map(|s| s.to_string()).collect(),
            attributes,
            text,
            is_text,
            child_count,
            children,
            depth,
            pseudo_states,
        }
    }

    /// Toggle expand/collapse for a node.
    pub fn toggle_expand(&mut self, node_id: NodeId) {
        if self.expanded.contains(&node_id) {
            self.expanded.remove(&node_id);
        } else {
            self.expanded.insert(node_id);
        }
    }

    /// Expand a node (ensures children are visible).
    pub fn expand(&mut self, node_id: NodeId) {
        self.expanded.insert(node_id);
    }

    /// Collapse a node.
    pub fn collapse(&mut self, node_id: NodeId) {
        self.expanded.remove(&node_id);
    }

    /// Expand all ancestors of a node to make it visible.
    pub fn reveal(&mut self, doc: &Document, node_id: NodeId) {
        let mut current = node_id;
        loop {
            match doc.parent(current) {
                Some(parent) => {
                    self.expanded.insert(parent);
                    current = parent;
                }
                None => break,
            }
        }
    }

    /// Select a node (for style panel display).
    pub fn select(&mut self, node_id: NodeId) {
        self.selected = Some(node_id);
    }

    /// Get the currently selected node.
    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// Set the hovered node (for highlight overlay).
    pub fn set_hovered(&mut self, node_id: Option<NodeId>) {
        self.hovered = node_id;
    }

    /// Get the currently hovered node.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Set the search query and filter the tree.
    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    /// Get the current search query.
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Find all nodes matching the search query.
    pub fn search(&self, doc: &Document) -> Vec<NodeId> {
        if self.search_query.is_empty() {
            return vec![];
        }

        let query = self.search_query.to_lowercase();
        let mut results = Vec::new();
        self.search_recursive(doc, 1, &query, &mut results);
        results
    }

    fn search_recursive(
        &self,
        doc: &Document,
        node_id: NodeId,
        query: &str,
        results: &mut Vec<NodeId>,
    ) {
        if let Some(node) = doc.get(node_id) {
            // Match against tag, id, classes, text content.
            let tag = node.tag.as_str().to_lowercase();
            let id = node.element_id.as_deref().unwrap_or("").to_lowercase();
            let classes_str = node.classes.to_class_string().to_lowercase();
            let text = node.text_content().unwrap_or("").to_lowercase();

            if tag.contains(query)
                || id.contains(query)
                || classes_str.contains(query)
                || text.contains(query)
            {
                results.push(node_id);
            }

            for &child in doc.children(node_id) {
                self.search_recursive(doc, child, query, results);
            }
        }
    }

    /// Get a flat list of visible nodes for rendering in a list view.
    pub fn visible_nodes(&self) -> Vec<&InspectorNode> {
        let mut result = Vec::new();
        if let Some(ref snapshot) = self.snapshot {
            Self::collect_visible(snapshot, &mut result);
        }
        result
    }

    fn collect_visible<'a>(node: &'a InspectorNode, out: &mut Vec<&'a InspectorNode>) {
        out.push(node);
        for child in &node.children {
            Self::collect_visible(child, out);
        }
    }

    /// Get the cached snapshot.
    pub fn snapshot(&self) -> Option<&InspectorNode> {
        self.snapshot.as_ref()
    }

    /// Export the tree as JSON.
    pub fn to_json(&self) -> String {
        match &self.snapshot {
            Some(snap) => serde_json::to_string_pretty(snap).unwrap_or_default(),
            None => "null".into(),
        }
    }

    /// Set the auto-expand depth for initial tree display.
    pub fn set_auto_expand_depth(&mut self, depth: u32) {
        self.auto_expand_depth = depth;
    }
}

impl Default for ElementTreeInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;

    #[test]
    fn test_inspector_builds_snapshot() {
        let mut doc = Document::new();
        let root_id = doc.root();
        let child = doc.create_element("div");
        doc.append_child(root_id, child);

        let mut inspector = ElementTreeInspector::new();
        let snap = inspector.build_snapshot(&doc);
        // The document root tag is created by Tag::root()
        assert_eq!(snap.child_count, 1);
    }

    #[test]
    fn test_expand_collapse() {
        let mut inspector = ElementTreeInspector::new();
        inspector.toggle_expand(42);
        assert!(inspector.expanded.contains(&42));
        inspector.toggle_expand(42);
        assert!(!inspector.expanded.contains(&42));
    }

    #[test]
    fn test_select() {
        let mut inspector = ElementTreeInspector::new();
        assert_eq!(inspector.selected(), None);
        inspector.select(10);
        assert_eq!(inspector.selected(), Some(10));
    }
}
