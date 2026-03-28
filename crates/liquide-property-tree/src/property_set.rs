//! Unified property tree set — provides combined access to all four property
//! trees (transform, clip, effect, scroll) and element-to-tree mappings.

use crate::clip_tree::ClipTree;
use crate::effect_tree::EffectTree;
use crate::scroll_tree::ScrollTree;
use crate::transform::Transform2D;
use crate::transform_tree::{NodeId, TransformTree, ROOT_ID};
use crate::Rect;

/// Identifies an element in the layout / DOM tree.
pub type ElementId = u32;

/// Maps an element to its property tree node IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeMapping {
    /// Index into the transform tree.
    pub transform_id: NodeId,
    /// Index into the clip tree.
    pub clip_id: NodeId,
    /// Index into the effect tree.
    pub effect_id: NodeId,
    /// Index into the scroll tree.
    pub scroll_id: NodeId,
}

impl Default for NodeMapping {
    fn default() -> Self {
        Self {
            transform_id: ROOT_ID,
            clip_id: ROOT_ID,
            effect_id: ROOT_ID,
            scroll_id: ROOT_ID,
        }
    }
}

/// Unified access to all four property trees plus element-to-tree mappings.
pub struct PropertyTreeSet {
    /// The transform property tree.
    pub transform_tree: TransformTree,
    /// The clip property tree.
    pub clip_tree: ClipTree,
    /// The effect property tree.
    pub effect_tree: EffectTree,
    /// The scroll property tree.
    pub scroll_tree: ScrollTree,
    /// Per-element mapping into the four trees.
    /// Indexed by `ElementId`.
    mappings: Vec<NodeMapping>,
    /// Local bounds per element (in element-local space).
    bounds: Vec<Rect>,
}

impl PropertyTreeSet {
    /// Create a new empty property tree set.
    pub fn new() -> Self {
        Self {
            transform_tree: TransformTree::new(),
            clip_tree: ClipTree::new(),
            effect_tree: EffectTree::new(),
            scroll_tree: ScrollTree::new(),
            mappings: Vec::new(),
            bounds: Vec::new(),
        }
    }

    /// Register an element and its property-tree node mappings.
    /// Returns the `ElementId`.
    pub fn add_element(&mut self, mapping: NodeMapping, local_bounds: Rect) -> ElementId {
        let id = self.mappings.len() as ElementId;
        self.mappings.push(mapping);
        self.bounds.push(local_bounds);
        id
    }

    /// Get the mapping for an element.
    pub fn mapping(&self, element_id: ElementId) -> Option<&NodeMapping> {
        self.mappings.get(element_id as usize)
    }

    /// Set the mapping for an element.
    pub fn set_mapping(&mut self, element_id: ElementId, mapping: NodeMapping) {
        if let Some(m) = self.mappings.get_mut(element_id as usize) {
            *m = mapping;
        }
    }

    /// Get the local bounds for an element.
    pub fn local_bounds(&self, element_id: ElementId) -> Option<Rect> {
        self.bounds.get(element_id as usize).copied()
    }

    /// Set the local bounds for an element.
    pub fn set_local_bounds(&mut self, element_id: ElementId, rect: Rect) {
        if let Some(b) = self.bounds.get_mut(element_id as usize) {
            *b = rect;
        }
    }

    /// Number of registered elements.
    pub fn element_count(&self) -> usize {
        self.mappings.len()
    }

    /// Recompute all dirty cached values across all trees.
    pub fn update(&mut self) {
        self.transform_tree.update();
        self.clip_tree.update();
        self.effect_tree.update();
        self.scroll_tree.update();
    }

    /// Clear all trees and element mappings for a full rebuild.
    pub fn clear(&mut self) {
        self.transform_tree.clear();
        self.clip_tree.clear();
        self.effect_tree.clear();
        self.scroll_tree.clear();
        self.mappings.clear();
        self.bounds.clear();
    }

    /// Map a point from an element's local space to screen (root) space.
    ///
    /// Applies the element's accumulated transform and scroll offset.
    pub fn map_point_to_screen(&self, element_id: ElementId, local_point: (f32, f32)) -> (f32, f32) {
        let mapping = match self.mappings.get(element_id as usize) {
            Some(m) => m,
            None => return local_point,
        };

        // Apply scroll offset first (scroll shifts the content)
        let (sx, sy) = self.scroll_tree.accumulated_scroll(mapping.scroll_id);
        let scrolled_x = local_point.0 - sx;
        let scrolled_y = local_point.1 - sy;

        // Then apply the world transform
        self.transform_tree.to_screen(mapping.transform_id, scrolled_x, scrolled_y)
    }

    /// Map a point from screen space back to element local spaces.
    ///
    /// Returns all elements whose screen-space bounds contain the point,
    /// along with the local-space coordinates. Results are ordered from
    /// front (highest z-order) to back.
    pub fn map_point_from_screen(&self, screen_point: (f32, f32)) -> Vec<(ElementId, (f32, f32))> {
        let mut hits = Vec::new();

        for (idx, mapping) in self.mappings.iter().enumerate() {
            let element_id = idx as ElementId;

            // Inverse-transform from screen to local space
            let world = self.transform_tree.world_transform(mapping.transform_id);
            let inv = match world.invert() {
                Some(inv) => inv,
                None => continue,
            };

            let (local_x, local_y) = inv.transform_point(screen_point.0, screen_point.1);

            // Apply inverse scroll
            let (sx, sy) = self.scroll_tree.accumulated_scroll(mapping.scroll_id);
            let unscrolled_x = local_x + sx;
            let unscrolled_y = local_y + sy;

            // Check if the point falls within the element's local bounds
            if let Some(bounds) = self.bounds.get(idx) {
                if unscrolled_x >= bounds.x
                    && unscrolled_x < bounds.x + bounds.width
                    && unscrolled_y >= bounds.y
                    && unscrolled_y < bounds.y + bounds.height
                {
                    hits.push((element_id, (unscrolled_x, unscrolled_y)));
                }
            }
        }

        hits
    }

    /// Compute the visible portion of an element after all clips are applied.
    ///
    /// Returns `None` if the element is fully clipped away.
    pub fn visible_rect(&self, element_id: ElementId) -> Option<Rect> {
        let mapping = match self.mappings.get(element_id as usize) {
            Some(m) => m,
            None => return None,
        };
        let local_bounds = match self.bounds.get(element_id as usize) {
            Some(b) => *b,
            None => return None,
        };

        // Transform local bounds to screen space
        let world = self.transform_tree.world_transform(mapping.transform_id);
        let (sx, sy) = self.scroll_tree.accumulated_scroll(mapping.scroll_id);
        let scroll_transform = Transform2D::translate(-sx, -sy);
        let full_transform = scroll_transform.multiply(&world);
        let screen_bounds = full_transform.transform_rect(local_bounds);

        // Intersect with accumulated clip
        let clip = self.clip_tree.accumulated_clip_rect(mapping.clip_id);
        match clip {
            Some(clip_rect) => {
                let visible = intersect_rects(screen_bounds, clip_rect);
                if visible.width > 0.0 && visible.height > 0.0 {
                    Some(visible)
                } else {
                    None
                }
            }
            None => {
                // No clip info — element is fully visible
                if screen_bounds.width > 0.0 && screen_bounds.height > 0.0 {
                    Some(screen_bounds)
                } else {
                    None
                }
            }
        }
    }

    /// Compute the screen-space damage rect for an element.
    ///
    /// This is the area of the screen that needs to be repainted when the
    /// element changes. It accounts for transforms, scroll offsets, and
    /// any filters that might expand the damage area (e.g., blur).
    pub fn damage_rect(&self, element_id: ElementId) -> Rect {
        let mapping = match self.mappings.get(element_id as usize) {
            Some(m) => m,
            None => return Rect::ZERO,
        };
        let local_bounds = match self.bounds.get(element_id as usize) {
            Some(b) => *b,
            None => return Rect::ZERO,
        };

        // Transform to screen space
        let world = self.transform_tree.world_transform(mapping.transform_id);
        let (sx, sy) = self.scroll_tree.accumulated_scroll(mapping.scroll_id);
        let scroll_transform = Transform2D::translate(-sx, -sy);
        let full_transform = scroll_transform.multiply(&world);
        let mut screen_rect = full_transform.transform_rect(local_bounds);

        // Expand for blur filters
        if let Some(effect_node) = self.effect_tree.get(mapping.effect_id) {
            for filter in &effect_node.filters {
                if let crate::effect_tree::FilterOp::Blur(radius) = filter {
                    // Blur expands the damage area by ~3x the radius in each direction
                    let expand = radius * 3.0;
                    screen_rect.x -= expand;
                    screen_rect.y -= expand;
                    screen_rect.width += expand * 2.0;
                    screen_rect.height += expand * 2.0;
                }
                if let crate::effect_tree::FilterOp::DropShadow { dx, dy, blur, .. } = filter {
                    let expand = blur * 3.0;
                    let shadow = Rect {
                        x: screen_rect.x + dx - expand,
                        y: screen_rect.y + dy - expand,
                        width: screen_rect.width + expand * 2.0,
                        height: screen_rect.height + expand * 2.0,
                    };
                    screen_rect = union_rects(screen_rect, shadow);
                }
            }
        }

        screen_rect
    }

    /// Total node count across all four trees.
    pub fn total_tree_nodes(&self) -> usize {
        self.transform_tree.len()
            + self.clip_tree.len()
            + self.effect_tree.len()
            + self.scroll_tree.len()
    }
}

impl Default for PropertyTreeSet {
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

/// Union of two rectangles.
fn union_rects(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    Rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}
