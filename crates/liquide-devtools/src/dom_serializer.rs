//! DOM tree serializer — JSON export of the live DOM for debugging.
//!
//! Provides a compact JSON representation of the DOM tree that can be
//! sent to external tools, saved to disk, or displayed in the devtools panel.

use liquide_dom::{Document, Node, NodeData, NodeId};
use serde::{Deserialize, Serialize};

/// A serializable snapshot of a single DOM node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    /// Node ID.
    pub id: NodeId,
    /// Tag name (e.g. "div", "dock-item", "#text").
    pub tag: String,
    /// Element `id` attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    /// CSS classes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// HTML-style attributes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<(String, String)>,
    /// Inline styles.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inline_styles: Vec<(String, String)>,
    /// Text content (for text nodes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Image source (for image nodes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_src: Option<String>,
    /// Surface ID (for sandbox surfaces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<u64>,
    /// Active pseudo-states (e.g. "hover", "focus").
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pseudo_states: Vec<String>,
    /// Number of children.
    pub child_count: usize,
    /// Children (recursive, controlled by depth limit).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SerializedNode>,
}

/// Configuration for DOM serialization.
#[derive(Debug, Clone)]
pub struct SerializerConfig {
    /// Maximum depth to serialize (-1 = unlimited).
    pub max_depth: i32,
    /// Whether to include attributes.
    pub include_attrs: bool,
    /// Whether to include inline styles.
    pub include_inline_styles: bool,
    /// Whether to include pseudo-states.
    pub include_pseudo_states: bool,
    /// Only serialize subtree rooted at this node (None = full tree from root).
    pub subtree_root: Option<NodeId>,
}

impl Default for SerializerConfig {
    fn default() -> Self {
        Self {
            max_depth: -1,
            include_attrs: true,
            include_inline_styles: true,
            include_pseudo_states: true,
            subtree_root: None,
        }
    }
}

/// Serializes a Document tree into JSON-friendly structures.
pub struct DomSerializer {
    config: SerializerConfig,
}

impl DomSerializer {
    pub fn new() -> Self {
        Self {
            config: SerializerConfig::default(),
        }
    }

    pub fn with_config(config: SerializerConfig) -> Self {
        Self { config }
    }

    /// Serialize the document tree into a `SerializedNode` hierarchy.
    pub fn serialize(&self, doc: &Document) -> SerializedNode {
        let root = self.config.subtree_root.unwrap_or(doc.root());
        self.serialize_node(doc, root, 0)
    }

    /// Serialize to a JSON string.
    pub fn to_json(&self, doc: &Document) -> String {
        let tree = self.serialize(doc);
        serde_json::to_string_pretty(&tree).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
    }

    /// Serialize to a compact JSON string (no pretty-printing).
    pub fn to_json_compact(&self, doc: &Document) -> String {
        let tree = self.serialize(doc);
        serde_json::to_string(&tree).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
    }

    fn serialize_node(&self, doc: &Document, node_id: NodeId, depth: i32) -> SerializedNode {
        let node = doc.get(node_id);

        let (
            tag,
            element_id,
            classes,
            attributes,
            inline_styles,
            text,
            image_src,
            surface_id,
            pseudo_states,
            child_count,
        ) = match node {
            Some(n) => {
                let tag = n.tag.as_str().to_string();
                let element_id = n.element_id.clone();
                let classes = n.classes.iter().map(|s| s.to_string()).collect();

                let attributes = if self.config.include_attrs {
                    n.attrs
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect()
                } else {
                    vec![]
                };

                let inline_styles = if self.config.include_inline_styles {
                    n.inline_styles
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect()
                } else {
                    vec![]
                };

                let (text, image_src, surface_id) = match &n.data {
                    NodeData::Text(t) => (Some(t.clone()), None, None),
                    NodeData::Image { src, .. } => (None, Some(src.clone()), None),
                    NodeData::Surface { surface_id } => (None, None, Some(*surface_id)),
                    _ => (None, None, None),
                };

                let pseudo_states = if self.config.include_pseudo_states {
                    extract_pseudo_state_names(n)
                } else {
                    vec![]
                };

                let child_count = n.children.len();

                (
                    tag,
                    element_id,
                    classes,
                    attributes,
                    inline_styles,
                    text,
                    image_src,
                    surface_id,
                    pseudo_states,
                    child_count,
                )
            }
            None => (
                "<missing>".to_string(),
                None,
                vec![],
                vec![],
                vec![],
                None,
                None,
                None,
                vec![],
                0,
            ),
        };

        // Build children if within depth limit.
        let children = if self.config.max_depth >= 0 && depth >= self.config.max_depth {
            vec![]
        } else if let Some(n) = node {
            n.children
                .iter()
                .map(|&child_id| self.serialize_node(doc, child_id, depth + 1))
                .collect()
        } else {
            vec![]
        };

        SerializedNode {
            id: node_id,
            tag,
            element_id,
            classes,
            attributes,
            inline_styles,
            text,
            image_src,
            surface_id,
            pseudo_states,
            child_count,
            children,
        }
    }
}

impl Default for DomSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract active pseudo-state names from a node's pseudo-state flags.
fn extract_pseudo_state_names(node: &Node) -> Vec<String> {
    use liquide_dom::PseudoStateFlags;
    let flags = node.pseudo_states;
    let mut names = Vec::new();
    if flags.contains(PseudoStateFlags::HOVER) {
        names.push("hover".to_string());
    }
    if flags.contains(PseudoStateFlags::FOCUS) {
        names.push("focus".to_string());
    }
    if flags.contains(PseudoStateFlags::ACTIVE) {
        names.push("active".to_string());
    }
    if flags.contains(PseudoStateFlags::DISABLED) {
        names.push("disabled".to_string());
    }
    if flags.contains(PseudoStateFlags::CHECKED) {
        names.push("checked".to_string());
    }
    if flags.contains(PseudoStateFlags::FOCUS_VISIBLE) {
        names.push("focus-visible".to_string());
    }
    if flags.contains(PseudoStateFlags::FOCUS_WITHIN) {
        names.push("focus-within".to_string());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SerializerConfig::default();
        assert_eq!(config.max_depth, -1);
        assert!(config.include_attrs);
        assert!(config.include_inline_styles);
        assert!(config.include_pseudo_states);
        assert_eq!(config.subtree_root, None);
    }

    #[test]
    fn test_serializer_new() {
        let serializer = DomSerializer::new();
        assert_eq!(serializer.config.max_depth, -1);
    }

    #[test]
    fn test_depth_limited_config() {
        let config = SerializerConfig {
            max_depth: 2,
            include_attrs: false,
            include_inline_styles: false,
            include_pseudo_states: false,
            subtree_root: None,
        };
        let serializer = DomSerializer::with_config(config);
        assert_eq!(serializer.config.max_depth, 2);
        assert!(!serializer.config.include_attrs);
    }

    #[test]
    fn test_config_with_subtree_root() {
        let config = SerializerConfig {
            max_depth: 5,
            include_attrs: true,
            include_inline_styles: false,
            include_pseudo_states: true,
            subtree_root: Some(42),
        };
        assert_eq!(config.subtree_root, Some(42));
        assert_eq!(config.max_depth, 5);
        assert!(!config.include_inline_styles);
    }

    #[test]
    fn test_serialized_node_serde_roundtrip() {
        let node = SerializedNode {
            id: 1,
            tag: "div".to_string(),
            element_id: Some("main".to_string()),
            classes: vec!["container".to_string()],
            attributes: vec![("role".to_string(), "main".to_string())],
            inline_styles: vec![("color".to_string(), "red".to_string())],
            text: Some("hello".to_string()),
            image_src: None,
            surface_id: None,
            pseudo_states: vec!["hover".to_string()],
            child_count: 0,
            children: vec![],
        };
        let json = serde_json::to_string(&node).unwrap();
        let deser: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deser["id"], 1);
        assert_eq!(deser["tag"], "div");
        assert_eq!(deser["element_id"], "main");
        assert_eq!(deser["classes"][0], "container");
        assert_eq!(deser["pseudo_states"][0], "hover");
    }

    #[test]
    fn test_serialized_node_skip_empty_fields() {
        let node = SerializedNode {
            id: 2,
            tag: "span".to_string(),
            element_id: None,
            classes: vec![],
            attributes: vec![],
            inline_styles: vec![],
            text: Some("hello".to_string()),
            image_src: None,
            surface_id: None,
            pseudo_states: vec![],
            child_count: 0,
            children: vec![],
        };
        let json = serde_json::to_string(&node).unwrap();
        // Empty vecs and None optionals should be skipped.
        assert!(!json.contains("classes"));
        assert!(!json.contains("element_id"));
        assert!(json.contains("hello"));
    }
}
