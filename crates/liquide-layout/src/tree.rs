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
///
/// All rects use **parent-local coordinates**: the (x, y) position is
/// relative to the parent box's content area origin, matching Blink's
/// PhysicalOffset model.  The painter and hit-tester accumulate absolute
/// offsets during tree traversal.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// Unique ID within the tree.
    pub id: LayoutBoxId,
    /// Back-reference to the DOM node.
    pub node: NodeId,
    /// Kind of layout.
    pub box_type: BoxType,
    /// Content area (inside padding) — parent-local.
    pub content_rect: Rect,
    /// Content + padding — parent-local.
    pub padding_rect: Rect,
    /// Content + padding + border — parent-local.
    pub border_rect: Rect,
    /// Content + padding + border + margin — parent-local.
    pub margin_rect: Rect,
    /// Children.
    pub children: Vec<LayoutBoxId>,
    /// Parent box (None for root).
    pub parent: Option<LayoutBoxId>,
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
            parent: None,
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
        if let Some(c) = self.boxes.get_mut(child) {
            c.parent = Some(parent);
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

    /// Compute the **absolute** content rect for a box by accumulating
    /// parent content-area offsets up to the root.  This implements the
    /// screen-space mapping that Blink performs inside
    /// `MapLocalToAncestor`.
    pub fn absolute_content_rect(&self, box_id: LayoutBoxId) -> Rect {
        let b = match self.get(box_id) {
            Some(b) => b,
            None => return Rect::zero(),
        };
        let (ox, oy) = self.accumulated_offset(box_id);
        Rect::new(
            b.content_rect.x + ox,
            b.content_rect.y + oy,
            b.content_rect.width,
            b.content_rect.height,
        )
    }

    /// Compute the **absolute** border rect for a box.
    pub fn absolute_border_rect(&self, box_id: LayoutBoxId) -> Rect {
        let b = match self.get(box_id) {
            Some(b) => b,
            None => return Rect::zero(),
        };
        let (ox, oy) = self.accumulated_offset(box_id);
        Rect::new(
            b.border_rect.x + ox,
            b.border_rect.y + oy,
            b.border_rect.width,
            b.border_rect.height,
        )
    }

    /// Compute the **absolute** padding rect for a box.
    pub fn absolute_padding_rect(&self, box_id: LayoutBoxId) -> Rect {
        let b = match self.get(box_id) {
            Some(b) => b,
            None => return Rect::zero(),
        };
        let (ox, oy) = self.accumulated_offset(box_id);
        Rect::new(
            b.padding_rect.x + ox,
            b.padding_rect.y + oy,
            b.padding_rect.width,
            b.padding_rect.height,
        )
    }

    /// Compute the **absolute** margin rect for a box.
    pub fn absolute_margin_rect(&self, box_id: LayoutBoxId) -> Rect {
        let b = match self.get(box_id) {
            Some(b) => b,
            None => return Rect::zero(),
        };
        let (ox, oy) = self.accumulated_offset(box_id);
        Rect::new(
            b.margin_rect.x + ox,
            b.margin_rect.y + oy,
            b.margin_rect.width,
            b.margin_rect.height,
        )
    }

    /// Walk ancestors to accumulate the paint offset for a box.
    /// Each ancestor contributes its `content_rect.(x,y)` since children
    /// are positioned relative to the parent's content area.
    fn accumulated_offset(&self, box_id: LayoutBoxId) -> (f32, f32) {
        let mut ox = 0.0f32;
        let mut oy = 0.0f32;
        let mut current = self.get(box_id).and_then(|b| b.parent);
        while let Some(pid) = current {
            if let Some(p) = self.get(pid) {
                ox += p.content_rect.x;
                oy += p.content_rect.y;
                current = p.parent;
            } else {
                break;
            }
        }
        (ox, oy)
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
