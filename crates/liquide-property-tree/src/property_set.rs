//! Unified property tree set — provides combined access to all four property
//! trees (transform, clip, effect, scroll) and element-to-tree mappings.

use crate::Rect;
use crate::clip_tree::ClipTree;
use crate::effect_tree::EffectTree;
use crate::scroll_tree::ScrollTree;
use crate::transform::Transform2D;
use crate::transform_tree::{NodeId, ROOT_ID, TransformTree};

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
    fn local_to_screen_transform(&self, mapping: &NodeMapping) -> Transform2D {
        let world = self.transform_tree.world_transform(mapping.transform_id);
        let (sx, sy) = self.scroll_tree.accumulated_scroll(mapping.scroll_id);
        Transform2D::translate(-sx, -sy).multiply(&world)
    }

    fn screen_to_local_point(
        &self,
        mapping: &NodeMapping,
        screen_point: (f32, f32),
    ) -> Option<(f32, f32)> {
        let inv = self.local_to_screen_transform(mapping).invert()?;
        Some(inv.transform_point(screen_point.0, screen_point.1))
    }

    fn clipped_local_bounds(&self, mapping: &NodeMapping, local_bounds: Rect) -> Option<Rect> {
        match self.clip_tree.accumulated_clip_rect(mapping.clip_id) {
            Some(clip_rect) => local_bounds.intersection(&clip_rect),
            None => Some(local_bounds),
        }
    }

    fn effect_chain_is_visible(&self, effect_id: NodeId) -> bool {
        let mut current = Some(effect_id);
        while let Some(id) = current {
            let Some(node) = self.effect_tree.get(id) else {
                break;
            };
            if node.opacity <= 0.0 {
                return false;
            }
            if node.filters.iter().any(|filter| {
                matches!(filter, crate::effect_tree::FilterOp::Opacity(value) if *value <= 0.0)
            }) {
                return false;
            }
            current = node.parent;
        }
        true
    }

    fn inflate_damage_for_effect_chain(&self, effect_id: NodeId, rect: Rect) -> Rect {
        let mut current = Some(effect_id);
        let mut damage = rect;
        while let Some(id) = current {
            let Some(node) = self.effect_tree.get(id) else {
                break;
            };
            for filter in &node.filters {
                damage = expand_damage_for_filter(damage, filter);
            }
            current = node.parent;
        }
        damage
    }

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

    /// Detach an element from the property-tree set.
    ///
    /// Because `ElementId` is the raw index into internal storage, we cannot
    /// compact the vectors without invalidating outstanding `ElementId`s held
    /// by callers. Instead we tombstone the slot: reset the mapping back to
    /// the root transform/clip/effect/scroll nodes and zero the local bounds
    /// so the element contributes nothing to subsequent hit-tests or damage
    /// computations. The underlying property-tree nodes remain in place so
    /// that sibling elements that share those nodes keep working.
    ///
    /// Returns `true` if the element existed, `false` if `element_id` was
    /// out-of-range.
    pub fn remove_element(&mut self, element_id: ElementId) -> bool {
        let idx = element_id as usize;
        if idx >= self.mappings.len() {
            return false;
        }
        self.mappings[idx] = NodeMapping::default();
        self.bounds[idx] = Rect::ZERO;
        true
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
    pub fn map_point_to_screen(
        &self,
        element_id: ElementId,
        local_point: (f32, f32),
    ) -> (f32, f32) {
        let mapping = match self.mappings.get(element_id as usize) {
            Some(m) => m,
            None => return local_point,
        };

        self.local_to_screen_transform(mapping)
            .transform_point(local_point.0, local_point.1)
    }

    /// Map a point from screen space back to element local spaces.
    ///
    /// Returns all elements whose screen-space bounds contain the point,
    /// along with the local-space coordinates. Results are returned in reverse
    /// registration order, which is the best deterministic proxy available for
    /// front-to-back hit testing without explicit z-order metadata.
    pub fn map_point_from_screen(&self, screen_point: (f32, f32)) -> Vec<(ElementId, (f32, f32))> {
        let mut hits = Vec::new();

        for (idx, mapping) in self.mappings.iter().enumerate().rev() {
            let element_id = idx as ElementId;
            let Some(bounds) = self.bounds.get(idx).copied() else {
                continue;
            };
            if bounds.is_empty() || !self.effect_chain_is_visible(mapping.effect_id) {
                continue;
            }

            let (local_x, local_y) = match self.screen_to_local_point(mapping, screen_point) {
                Some(point) => point,
                None => continue,
            };

            if bounds.contains(local_x, local_y)
                && self
                    .clip_tree
                    .contains_point(mapping.clip_id, (local_x, local_y))
            {
                hits.push((element_id, (local_x, local_y)));
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
        if local_bounds.is_empty() || !self.effect_chain_is_visible(mapping.effect_id) {
            return None;
        }

        let local_visible = self.clipped_local_bounds(mapping, local_bounds)?;
        let screen_visible = self
            .local_to_screen_transform(mapping)
            .transform_rect(local_visible);

        if screen_visible.is_empty() {
            None
        } else {
            Some(screen_visible)
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
        if local_bounds.is_empty() {
            return Rect::ZERO;
        }

        let local_damage = match self.clipped_local_bounds(mapping, local_bounds) {
            Some(rect) => rect,
            None => return Rect::ZERO,
        };

        let screen_rect = self
            .local_to_screen_transform(mapping)
            .transform_rect(local_damage);
        if screen_rect.is_empty() {
            return Rect::ZERO;
        }

        self.inflate_damage_for_effect_chain(mapping.effect_id, screen_rect)
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

fn expand_damage_for_filter(rect: Rect, filter: &crate::effect_tree::FilterOp) -> Rect {
    match filter {
        crate::effect_tree::FilterOp::Blur(radius) => rect.expand(radius.max(0.0) * 3.0),
        crate::effect_tree::FilterOp::DropShadow { dx, dy, blur, .. } => {
            let expand = blur.max(0.0) * 3.0;
            let shadow = Rect {
                x: rect.x + dx - expand,
                y: rect.y + dy - expand,
                width: rect.width + expand * 2.0,
                height: rect.height + expand * 2.0,
            };
            union_rects(rect, shadow)
        }
        _ => rect,
    }
}
