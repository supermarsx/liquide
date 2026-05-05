//! Hierarchical clip tree.
//!
//! Each node defines a clip region. Accumulated clips are the intersection
//! of all ancestor clips down to the current node — used for visibility
//! culling and overdraw elimination.

use crate::Rect;
use crate::transform_tree::{NodeId, ROOT_ID};

/// The type of clip applied at a node.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipType {
    /// Axis-aligned rectangular clip.
    Rect,
    /// Rounded rectangle with per-corner radii (top-left, top-right, bottom-right, bottom-left).
    RoundedRect { radii: (f32, f32, f32, f32) },
    /// Circle or ellipse clip.
    CircleEllipse { cx: f32, cy: f32, rx: f32, ry: f32 },
    /// Arbitrary polygon clip (list of vertices).
    Path(Vec<(f32, f32)>),
}

impl Default for ClipType {
    fn default() -> Self {
        ClipType::Rect
    }
}

/// A node in the clip tree.
#[derive(Debug, Clone)]
pub struct ClipNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Parent node, or `None` for the root.
    pub parent: Option<NodeId>,
    /// The clip rectangle in the local space shared by this clip chain.
    ///
    /// The accumulated clip cached by this tree remains in that same local
    /// space; callers must apply transforms separately before comparing it to
    /// screen-space geometry.
    pub clip_rect: Rect,
    /// The clip shape type.
    pub clip_type: ClipType,
}

impl Default for ClipNode {
    fn default() -> Self {
        Self {
            id: ROOT_ID,
            parent: None,
            clip_rect: Rect {
                x: f32::MIN / 2.0,
                y: f32::MIN / 2.0,
                width: f32::MAX,
                height: f32::MAX,
            },
            clip_type: ClipType::Rect,
        }
    }
}

/// A chain of clips from root to a specific node — used for rendering.
#[derive(Debug, Clone)]
pub struct ClipChain {
    /// The clips in order from root to leaf.
    pub clips: Vec<ClipChainEntry>,
}

/// A single entry in a clip chain.
#[derive(Debug, Clone)]
pub struct ClipChainEntry {
    /// The node ID that produced this clip.
    pub node_id: NodeId,
    /// The clip rectangle.
    pub clip_rect: Rect,
    /// The clip type.
    pub clip_type: ClipType,
}

/// Hierarchical tree of clip nodes with cached accumulated clips.
pub struct ClipTree {
    nodes: Vec<ClipNode>,
    /// Cached accumulated local-space clip rect per node (AABB intersection of
    /// all ancestor clip bounds).
    accumulated: Vec<Option<Rect>>,
    /// Per-node dirty flag.
    dirty: Vec<bool>,
    /// Children list per node.
    children: Vec<Vec<NodeId>>,
}

impl ClipTree {
    /// Create a new clip tree with just the root node.
    pub fn new() -> Self {
        let root = ClipNode::default();
        Self {
            nodes: vec![root],
            accumulated: vec![None],
            dirty: vec![true],
            children: vec![Vec::new()],
        }
    }

    /// Add a new clip node. Returns its `NodeId`.
    pub fn add(&mut self, parent: Option<NodeId>, clip_rect: Rect, clip_type: ClipType) -> NodeId {
        let id = self.nodes.len() as NodeId;
        let parent_id = match parent.unwrap_or(ROOT_ID) {
            pid if (pid as usize) < self.nodes.len() => pid,
            _ => ROOT_ID,
        };
        self.nodes.push(ClipNode {
            id,
            parent: Some(parent_id),
            clip_rect,
            clip_type,
        });
        self.accumulated.push(None);
        self.dirty.push(true);
        while self.children.len() <= id as usize {
            self.children.push(Vec::new());
        }
        self.children[parent_id as usize].push(id);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&ClipNode> {
        self.nodes.get(id as usize)
    }

    /// Set the clip rect of a node and mark it dirty.
    pub fn set_clip_rect(&mut self, id: NodeId, rect: Rect) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.clip_rect = rect;
            self.mark_dirty(id);
        }
    }

    /// Set the clip type of a node and mark it dirty.
    pub fn set_clip_type(&mut self, id: NodeId, clip_type: ClipType) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.clip_type = clip_type;
            self.mark_dirty(id);
        }
    }

    /// Mark a node and all its descendants as needing recomputation.
    pub fn mark_dirty(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(nid) = stack.pop() {
            let idx = nid as usize;
            if idx < self.dirty.len() {
                self.dirty[idx] = true;
                if idx < self.children.len() {
                    for &child in &self.children[idx] {
                        stack.push(child);
                    }
                }
            }
        }
    }

    /// Recompute accumulated clips for all dirty nodes (top-down walk).
    pub fn update(&mut self) {
        let len = self.nodes.len();
        for i in 0..len {
            if !self.dirty[i] {
                continue;
            }
            let parent_acc = match self.nodes[i].parent {
                Some(pid) => {
                    self.accumulated[pid as usize].unwrap_or(self.nodes[pid as usize].clip_rect)
                }
                None => self.nodes[i].clip_rect,
            };
            self.accumulated[i] = Some(intersect_rects(parent_acc, self.nodes[i].clip_rect));
            self.dirty[i] = false;
        }
    }

    /// Get the accumulated clip rect for a node.
    ///
    /// Call `update()` first to ensure dirty nodes are recomputed.
    pub fn accumulated_clip_rect(&self, id: NodeId) -> Option<Rect> {
        self.accumulated.get(id as usize).copied().flatten()
    }

    /// Build the full clip chain from root to the given node.
    pub fn accumulated_clip(&self, id: NodeId) -> ClipChain {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(nid) = current {
            if let Some(node) = self.nodes.get(nid as usize) {
                chain.push(ClipChainEntry {
                    node_id: nid,
                    clip_rect: node.clip_rect,
                    clip_type: node.clip_type.clone(),
                });
                current = node.parent;
            } else {
                break;
            }
        }
        chain.reverse();
        ClipChain { clips: chain }
    }

    /// Test whether a point in the clip chain's local space is inside every
    /// clip from the given node back to the root.
    pub fn contains_point(&self, id: NodeId, point: (f32, f32)) -> bool {
        let mut current = Some(id);
        while let Some(nid) = current {
            let Some(node) = self.nodes.get(nid as usize) else {
                return false;
            };
            if !clip_contains_point(node, point.0, point.1) {
                return false;
            }
            current = node.parent;
        }
        true
    }

    /// Test whether a rectangle is visible after applying all accumulated clips
    /// for the given node. This is a conservative test using accumulated AABB clips.
    pub fn is_visible(&self, id: NodeId, test_rect: Rect) -> bool {
        let acc = match self.accumulated.get(id as usize) {
            Some(Some(r)) => *r,
            _ => return true, // No clip data — assume visible
        };
        // Empty accumulated clip means fully clipped away
        if acc.width <= 0.0 || acc.height <= 0.0 {
            return false;
        }
        rects_intersect(acc, test_rect)
    }

    /// Number of nodes (including root).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree has only the root node.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Whether any nodes are dirty.
    pub fn has_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }

    /// Clear all nodes except root.
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        self.accumulated.truncate(1);
        self.dirty.truncate(1);
        self.dirty[0] = true;
        self.accumulated[0] = None;
        self.children.truncate(1);
        self.children[0].clear();
    }

    /// Iterate all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &ClipNode> {
        self.nodes.iter()
    }
}

impl Default for ClipTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Intersect two rectangles, clamping to non-negative dimensions.
fn intersect_rects(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    Rect {
        x: x1,
        y: y1,
        width: (x2 - x1).max(0.0),
        height: (y2 - y1).max(0.0),
    }
}

/// AABB intersection test.
fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

fn clip_contains_point(node: &ClipNode, x: f32, y: f32) -> bool {
    match &node.clip_type {
        ClipType::Rect => node.clip_rect.contains(x, y),
        ClipType::RoundedRect { radii } => {
            rounded_rect_contains_point(node.clip_rect, *radii, x, y)
        }
        ClipType::CircleEllipse { cx, cy, rx, ry } => {
            ellipse_contains_point(*cx, *cy, *rx, *ry, x, y)
        }
        ClipType::Path(points) => path_contains_point(points, x, y),
    }
}

fn rounded_rect_contains_point(rect: Rect, radii: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    if !rect.contains(x, y) {
        return false;
    }

    let max_radius = (rect.width.min(rect.height) * 0.5).max(0.0);
    let top_left = radii.0.clamp(0.0, max_radius);
    let top_right = radii.1.clamp(0.0, max_radius);
    let bottom_right = radii.2.clamp(0.0, max_radius);
    let bottom_left = radii.3.clamp(0.0, max_radius);

    if x < rect.x + top_left && y < rect.y + top_left {
        return ellipse_contains_point(
            rect.x + top_left,
            rect.y + top_left,
            top_left,
            top_left,
            x,
            y,
        );
    }
    if x >= rect.right() - top_right && y < rect.y + top_right {
        return ellipse_contains_point(
            rect.right() - top_right,
            rect.y + top_right,
            top_right,
            top_right,
            x,
            y,
        );
    }
    if x >= rect.right() - bottom_right && y >= rect.bottom() - bottom_right {
        return ellipse_contains_point(
            rect.right() - bottom_right,
            rect.bottom() - bottom_right,
            bottom_right,
            bottom_right,
            x,
            y,
        );
    }
    if x < rect.x + bottom_left && y >= rect.bottom() - bottom_left {
        return ellipse_contains_point(
            rect.x + bottom_left,
            rect.bottom() - bottom_left,
            bottom_left,
            bottom_left,
            x,
            y,
        );
    }

    true
}

fn ellipse_contains_point(cx: f32, cy: f32, rx: f32, ry: f32, x: f32, y: f32) -> bool {
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }

    let norm_x = (x - cx) / rx;
    let norm_y = (y - cy) / ry;
    norm_x * norm_x + norm_y * norm_y <= 1.0
}

fn path_contains_point(points: &[(f32, f32)], x: f32, y: f32) -> bool {
    if points.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = points[points.len() - 1];

    for &current in points {
        let (x0, y0) = previous;
        let (x1, y1) = current;
        let crosses_scanline = (y0 > y) != (y1 > y);
        if crosses_scanline {
            let intersection_x = x0 + (x1 - x0) * (y - y0) / (y1 - y0);
            if x <= intersection_x {
                inside = !inside;
            }
        }
        previous = current;
    }

    inside
}
