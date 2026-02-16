//! Layout tree — the output of the layout engine.

use std::ops::Range;

use liquide_dom::NodeId;

use crate::geometry::{Rect, Size};

/// ID for a layout box within the tree.
pub type LayoutBoxId = usize;

/// The type of layout box.
#[derive(Debug, Clone)]
pub enum BoxType {
    /// Block-level formatting context.
    Block,
    /// Inline element.
    Inline,
    /// Inline-block element.
    InlineBlock,
    /// Flex container.
    Flex,
    /// Flex item.
    FlexItem,
    /// Grid container.
    Grid,
    /// Grid item.
    GridItem,
    /// Text content with shaped line boxes.
    Text { line_boxes: Vec<LineBox> },
    /// Replaced element (image, surface).
    Replaced,
    /// Absolutely positioned.
    Absolute,
    /// Fixed positioned.
    Fixed,
    /// Sticky positioned.
    Sticky,
}

/// A line box within a text layout.
#[derive(Debug, Clone)]
pub struct LineBox {
    /// Glyph index range.
    pub range: Range<usize>,
    /// Position and size.
    pub rect: Rect,
    /// Baseline position within the line.
    pub baseline: f32,
}

/// A single layout box — represents one element's geometry.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// Unique ID within the tree.
    pub id: LayoutBoxId,
    /// Back-reference to the DOM node.
    pub node: NodeId,
    /// Kind of layout.
    pub box_type: BoxType,
    /// Content area (inside padding).
    pub content_rect: Rect,
    /// Content + padding.
    pub padding_rect: Rect,
    /// Content + padding + border.
    pub border_rect: Rect,
    /// Content + padding + border + margin.
    pub margin_rect: Rect,
    /// Children.
    pub children: Vec<LayoutBoxId>,
    /// First baseline for flex alignment.
    pub baseline: Option<f32>,
    /// Scrollable content size (if overflow).
    pub scroll_size: Option<Size>,
}

impl LayoutBox {
    /// Create a new empty layout box.
    pub fn new(id: LayoutBoxId, node: NodeId, box_type: BoxType) -> Self {
        Self {
            id,
            node,
            box_type,
            content_rect: Rect::zero(),
            padding_rect: Rect::zero(),
            border_rect: Rect::zero(),
            margin_rect: Rect::zero(),
            children: Vec::new(),
            baseline: None,
            scroll_size: None,
        }
    }
}

/// The laid-out tree — result of running the layout engine.
pub struct LayoutTree {
    /// All boxes, indexed by LayoutBoxId.
    pub boxes: Vec<LayoutBox>,
    /// The root box.
    pub root: LayoutBoxId,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self {
            boxes: Vec::new(),
            root: 0,
        }
    }

    /// Get a box by ID.
    pub fn get(&self, id: LayoutBoxId) -> Option<&LayoutBox> {
        self.boxes.get(id)
    }

    /// Get a mutable box by ID.
    pub fn get_mut(&mut self, id: LayoutBoxId) -> Option<&mut LayoutBox> {
        self.boxes.get_mut(id)
    }

    /// Allocate a new box.
    pub fn alloc(&mut self, node: NodeId, box_type: BoxType) -> LayoutBoxId {
        let id = self.boxes.len();
        self.boxes.push(LayoutBox::new(id, node, box_type));
        id
    }

    /// Add a child to a parent box.
    pub fn add_child(&mut self, parent: LayoutBoxId, child: LayoutBoxId) {
        if let Some(p) = self.boxes.get_mut(parent) {
            p.children.push(child);
        }
    }

    /// Remove a child from a parent box's children list.
    pub fn remove_child(&mut self, parent: LayoutBoxId, child: LayoutBoxId) {
        if let Some(p) = self.boxes.get_mut(parent) {
            p.children.retain(|&c| c != child);
        }
    }

    /// Find the layout box for a given DOM node.
    pub fn find_by_node(&self, node_id: NodeId) -> Option<&LayoutBox> {
        self.boxes.iter().find(|b| b.node == node_id)
    }

    /// Total number of boxes.
    pub fn box_count(&self) -> usize {
        self.boxes.len()
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
