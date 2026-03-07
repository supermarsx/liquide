//! Layout tree — the output of the layout engine.

use std::collections::HashMap;
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
    /// List marker (bullet, number, etc.) for display: list-item.
    ListMarker,
    /// Pseudo-element box (::before, ::after) with its generated content.
    PseudoElement {
        /// Which pseudo-element this is (before/after).
        kind: PseudoElementKind,
        /// The generated text content.
        content: String,
    },
}

/// Which pseudo-element a PseudoElement box represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoElementKind {
    Before,
    After,
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
    /// Scrollable content size (if overflow creates a scroll container).
    pub scroll_size: Option<Size>,
    /// Current scroll offset (how far the user has scrolled).
    pub scroll_offset: (f32, f32),
    /// Resolved grid column track sizes (for subgrid inheritance by children).
    pub grid_col_tracks: Vec<f32>,
    /// Resolved grid row track sizes (for subgrid inheritance by children).
    pub grid_row_tracks: Vec<f32>,
    /// When true, the text content was truncated and an ellipsis ("…") should
    /// be rendered at the end of the last visible line.  Set by inline layout
    /// when `overflow: hidden` + `text-overflow: ellipsis` applies.
    pub text_overflow_ellipsis: bool,
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
            scroll_offset: (0.0, 0.0),
            grid_col_tracks: Vec::new(),
            grid_row_tracks: Vec::new(),
            text_overflow_ellipsis: false,
        }
    }
}

/// The laid-out tree — result of running the layout engine.
#[derive(Clone)]
pub struct LayoutTree {
    /// All boxes, indexed by LayoutBoxId.
    pub boxes: Vec<LayoutBox>,
    /// The root box.
    pub root: LayoutBoxId,
    /// Node → box index for O(1) lookup.
    node_index: HashMap<NodeId, LayoutBoxId>,
}

impl LayoutTree {
    pub fn new() -> Self {
        Self {
            boxes: Vec::new(),
            root: 0,
            node_index: HashMap::new(),
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
        self.node_index.insert(node, id);
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

    /// Find the layout box for a given DOM node (O(1) via index).
    pub fn find_by_node(&self, node_id: NodeId) -> Option<&LayoutBox> {
        self.node_index.get(&node_id).and_then(|&id| self.boxes.get(id))
    }

    /// Find the layout box ID for a given DOM node (O(1) via index).
    pub fn find_box_id_by_node(&self, node_id: NodeId) -> Option<LayoutBoxId> {
        self.node_index.get(&node_id).copied()
    }

    /// Update the node → box mapping.
    ///
    /// This is needed when a positioned element (position: absolute/fixed) has
    /// `display: flex/grid`. The flex/grid layout creates a temporary box that
    /// gets registered in node_index, but the positioned box should be canonical.
    pub fn set_node_box(&mut self, node_id: NodeId, box_id: LayoutBoxId) {
        self.node_index.insert(node_id, box_id);
    }

    /// Remove the node → box mapping only when it still points to `box_id`.
    ///
    /// This is useful for incremental relayout where stale subtree boxes are
    /// detached while unrelated mappings must remain intact.
    pub fn clear_node_box_if(&mut self, node_id: NodeId, box_id: LayoutBoxId) {
        if self.node_index.get(&node_id).copied() == Some(box_id) {
            self.node_index.remove(&node_id);
        }
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
                ox += p.content_rect.x - p.scroll_offset.0;
                oy += p.content_rect.y - p.scroll_offset.1;
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

impl LayoutTree {
    /// Set the scroll offset for a layout box (scroll container).
    /// Clamps to the scrollable range based on `scroll_size`.
    pub fn set_scroll_offset(&mut self, box_id: LayoutBoxId, dx: f32, dy: f32) {
        if let Some(b) = self.get_mut(box_id) {
            if let Some(ref ss) = b.scroll_size {
                let max_x = (ss.width - b.content_rect.width).max(0.0);
                let max_y = (ss.height - b.content_rect.height).max(0.0);
                b.scroll_offset.0 = (b.scroll_offset.0 + dx).clamp(0.0, max_x);
                b.scroll_offset.1 = (b.scroll_offset.1 + dy).clamp(0.0, max_y);
            }
        }
    }

    /// Get the current scroll offset for a layout box.
    pub fn scroll_offset(&self, box_id: LayoutBoxId) -> (f32, f32) {
        self.get(box_id).map(|b| b.scroll_offset).unwrap_or((0.0, 0.0))
    }

    /// Find the nearest scroll container ancestor for a given box.
    pub fn find_scroll_container(&self, box_id: LayoutBoxId) -> Option<LayoutBoxId> {
        let mut current = self.get(box_id).and_then(|b| b.parent);
        while let Some(pid) = current {
            if let Some(p) = self.get(pid) {
                if p.scroll_size.is_some() {
                    return Some(pid);
                }
                current = p.parent;
            } else {
                break;
            }
        }
        None
    }
}
