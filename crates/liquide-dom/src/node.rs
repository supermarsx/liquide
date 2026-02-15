//! DOM node definition.

use serde::{Deserialize, Serialize};

use crate::attrs::AttributeMap;
use crate::class_list::ClassList;
use crate::dirty::DirtyFlags;
use crate::pseudo::PseudoStateFlags;
use crate::tag::Tag;

/// Unique identifier for a DOM node.
pub type NodeId = u64;

/// A node in the DOM tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier.
    pub id: NodeId,
    /// Element tag (interned string).
    pub tag: Tag,
    /// Parent node, if any.
    pub parent: Option<NodeId>,
    /// Child nodes in document order.
    pub children: Vec<NodeId>,
    /// HTML-style attributes.
    pub attrs: AttributeMap,
    /// Inline CSS styles (property: value pairs, highest specificity).
    pub inline_styles: AttributeMap,
    /// CSS class list.
    pub classes: ClassList,
    /// Element `id` attribute (for `#id` selectors).
    pub element_id: Option<String>,
    /// CSS pseudo-class states.
    pub pseudo_states: PseudoStateFlags,
    /// Node-type-specific data.
    pub data: NodeData,
    /// Dirty tracking flags.
    pub dirty: DirtyFlags,
}

/// Type-specific data stored in a DOM node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeData {
    /// Generic element container.
    Element,
    /// Text content node.
    Text(String),
    /// Image element.
    Image {
        src: String,
        alt: String,
        #[serde(skip)]
        natural_width: Option<u32>,
        #[serde(skip)]
        natural_height: Option<u32>,
    },
    /// Sandboxed application surface (rendered externally).
    Surface {
        surface_id: u64,
    },
    /// Shadow root (for component isolation within the desktop).
    ShadowRoot,
}

impl Node {
    /// Create a new element node.
    pub fn new_element(id: NodeId, tag: Tag) -> Self {
        Self {
            id,
            tag,
            parent: None,
            children: Vec::new(),
            attrs: AttributeMap::new(),
            inline_styles: AttributeMap::new(),
            classes: ClassList::new(),
            element_id: None,
            pseudo_states: PseudoStateFlags::empty(),
            data: NodeData::Element,
            dirty: DirtyFlags::all_dirty(),
        }
    }

    /// Create a new text node.
    pub fn new_text(id: NodeId, text: &str) -> Self {
        Self {
            id,
            tag: Tag::text(),
            parent: None,
            children: Vec::new(),
            attrs: AttributeMap::new(),
            inline_styles: AttributeMap::new(),
            classes: ClassList::new(),
            element_id: None,
            pseudo_states: PseudoStateFlags::empty(),
            data: NodeData::Text(text.to_string()),
            dirty: DirtyFlags::all_dirty(),
        }
    }

    /// Check if this is a text node.
    pub fn is_text(&self) -> bool {
        matches!(self.data, NodeData::Text(_))
    }

    /// Check if this is an element node.
    pub fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element | NodeData::ShadowRoot)
    }

    /// Get text content (for text nodes).
    pub fn text_content(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Get the tag name as a string.
    pub fn tag_name(&self) -> String {
        self.tag.as_str()
    }

    /// Check if a pseudo-state is active.
    pub fn has_pseudo_state(&self, state: PseudoStateFlags) -> bool {
        self.pseudo_states.contains(state)
    }

    /// Get child count.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Check if this node has the given class.
    pub fn has_class(&self, class: &str) -> bool {
        self.classes.contains(class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_basics() {
        let node = Node::new_element(1, Tag::intern("button"));
        assert!(node.is_element());
        assert!(!node.is_text());
        assert_eq!(node.tag_name(), "button");
        assert_eq!(node.child_count(), 0);
    }

    #[test]
    fn text_node() {
        let node = Node::new_text(2, "Hello, world!");
        assert!(node.is_text());
        assert_eq!(node.text_content(), Some("Hello, world!"));
    }

    #[test]
    fn pseudo_state_check() {
        let mut node = Node::new_element(3, Tag::intern("input"));
        node.pseudo_states = PseudoStateFlags::FOCUS | PseudoStateFlags::HOVER;
        assert!(node.has_pseudo_state(PseudoStateFlags::FOCUS));
        assert!(node.has_pseudo_state(PseudoStateFlags::HOVER));
        assert!(!node.has_pseudo_state(PseudoStateFlags::DISABLED));
    }
}
