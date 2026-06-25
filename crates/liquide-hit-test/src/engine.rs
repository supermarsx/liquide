//! Hit test engine — finds which DOM node is under a point.
//!
//! Traverses the layout tree in CSS painting order (matching the painter's
//! stacking context algorithm) so the visually-topmost element is hit first.
//! Supports:
//! - CSS transforms (inverse-transform of the pointer into local space)
//! - Overflow clipping (overflow: hidden/scroll/auto/clip)
//! - Visibility checks (visibility: hidden, content-visibility: hidden)
//! - pointer-events: none
//! - Z-index stacking order (CSS 2.1 §E)

use std::sync::Arc;

use liquide_compositor::geometry::Affine2D;
use liquide_dom::NodeId;
use liquide_layout::geometry::{ClipComplexity, Point, Rect};
use liquide_layout::tree::{LayoutBoxId, LayoutTree};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{
    ContentVisibility, Display, Float, Overflow, PointerEvents, Position, Transform, Visibility,
};
use liquide_style_engine::dimension::{Corners, Dimension, EllipticalRadius};

/// Result of a hit test.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// The target DOM node.
    pub node: NodeId,
    /// The point relative to the node's content box.
    pub point_in_node: Point,
    /// Absolute bounds of the hit element (border rect).
    pub bounds: Rect,
    /// Ancestor chain from target up to root (bubble path).
    pub ancestors: Vec<NodeId>,
}

impl HitTestResult {
    /// Returns an iterator over the hit node and all ancestors.
    ///
    /// Yields the target node first, then ancestors in order (parent, grandparent, etc.)
    pub fn node_and_ancestors(&self) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::once(self.node).chain(self.ancestors.iter().copied())
    }

    /// Find the first node (including self) that satisfies a predicate.
    ///
    /// Useful for walking up the DOM to find specific element types.
    pub fn find_ancestor<F>(&self, predicate: F) -> Option<NodeId>
    where
        F: Fn(NodeId) -> bool,
    {
        self.node_and_ancestors().find(|&n| predicate(n))
    }
}

/// The hit test engine.
pub struct HitTestEngine {
    /// Cached layout tree (shared via Arc to avoid deep clones from pipeline).
    layout: Arc<LayoutTree>,
    /// Cached style map (shared via Arc to avoid deep clones from pipeline).
    styles: Arc<StyleMap>,
}

impl HitTestEngine {
    /// Create a new hit test engine from Arc-wrapped pipeline output.
    pub fn new(layout: Arc<LayoutTree>, styles: Arc<StyleMap>) -> Self {
        Self { layout, styles }
    }

    /// Convenience constructor that wraps owned values in Arc.
    pub fn from_owned(layout: LayoutTree, styles: StyleMap) -> Self {
        Self {
            layout: Arc::new(layout),
            styles: Arc::new(styles),
        }
    }

    /// Update with new layout/styles (after relayout).
    pub fn update(&mut self, layout: Arc<LayoutTree>, styles: Arc<StyleMap>) {
        self.layout = layout;
        self.styles = styles;
    }

    /// Get a reference to the layout tree.
    pub fn layout(&self) -> &LayoutTree {
        &self.layout
    }

    /// Get a mutable reference to the layout tree (for scroll offset updates).
    ///
    /// Uses `Arc::make_mut` which clones only if there are other Arc holders.
    pub fn layout_mut(&mut self) -> &mut LayoutTree {
        Arc::make_mut(&mut self.layout)
    }

    /// Get a reference to the style map.
    pub fn styles(&self) -> &StyleMap {
        &self.styles
    }

    /// Hit test a single point. Returns the topmost matching node.
    pub fn hit_test(&self, point: Point) -> Option<HitTestResult> {
        self.hit_test_box(
            self.layout.root,
            point,
            (0.0, 0.0),
            &ClipComplexity::Trivial,
        )
    }

    /// Hit test all overlapping nodes at a point (front to back).
    pub fn hit_test_all(&self, point: Point) -> Vec<HitTestResult> {
        let mut results = Vec::new();
        self.hit_test_box_all(
            self.layout.root,
            point,
            (0.0, 0.0),
            &ClipComplexity::Trivial,
            &mut results,
        );
        results
    }

    // ─── Programmatic bounds queries ──────────────────────────────────

    /// Get the absolute bounds (border rect) for a DOM node.
    ///
    /// Returns `None` if the node has no layout box (e.g., `display: none`).
    pub fn bounds_for_node(&self, node_id: NodeId) -> Option<Rect> {
        let box_id = self.layout.find_box_id_by_node(node_id)?;
        Some(self.layout.absolute_border_rect(box_id))
    }

    /// Get the absolute content rect for a DOM node.
    pub fn content_rect_for_node(&self, node_id: NodeId) -> Option<Rect> {
        let box_id = self.layout.find_box_id_by_node(node_id)?;
        Some(self.layout.absolute_content_rect(box_id))
    }

    /// Check if a point is inside a specific node's bounds (border box).
    pub fn point_in_node(&self, point: Point, node_id: NodeId) -> bool {
        self.bounds_for_node(node_id)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }

    /// Find all nodes under a point that satisfy a predicate.
    ///
    /// The predicate receives (NodeId, bounds) and returns true if the node
    /// should be included. Useful for component-based hit testing where you
    /// want to find specific types of elements.
    pub fn hit_test_filter<F>(&self, point: Point, predicate: F) -> Vec<(NodeId, Rect)>
    where
        F: Fn(NodeId, &Rect) -> bool,
    {
        self.hit_test_all(point)
            .into_iter()
            .filter(|r| predicate(r.node, &r.bounds))
            .map(|r| (r.node, r.bounds))
            .collect()
    }

    /// Core hit-test for a single box, returning the topmost match.
    ///
    /// `clip` is the active overflow clip in absolute coordinates.
    /// When a parent has `overflow` != `visible`, it constrains the hit
    /// region for all descendants.
    fn hit_test_box(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
        clip: &ClipComplexity,
    ) -> Option<HitTestResult> {
        let layout_box = self.layout.get(box_id)?;
        let style = self
            .styles
            .get(layout_box.node)
            .cloned()
            .unwrap_or_default();
        let (ox, oy) = paint_offset;

        // ── Visibility checks ─────────────────────────────────────────
        // Skip display:none (shouldn't be in layout, but guard anyway)
        if style.display == Display::None {
            return None;
        }
        // Skip visibility:hidden — element doesn't receive pointer events.
        // (Note: children with visibility:visible CAN still be hit; CSS
        //  inheritance means we check per-element, not skip the subtree.)
        if style.visibility == Visibility::Hidden {
            // Still recurse children — they may override to visible
            // (handled below by checking children first)
        }
        // content-visibility: hidden skips the entire subtree
        if style.content_visibility == ContentVisibility::Hidden {
            return None;
        }
        // pointer-events: none — skip this element but still check children
        // Per CSS spec, children can override with pointer-events: auto
        let this_receives_events = style.pointer_events != PointerEvents::None;

        // ── Transform: inverse-map the point into local space ─────────
        let local_point = if !style.transform.is_empty() {
            let abs_border = layout_box.border_rect.offset(ox, oy);
            // Compute transform origin from style (matching painter behavior)
            let origin_x = abs_border.x
                + resolve_origin_dimension(
                    &style.transform_origin.x,
                    abs_border.width,
                    style.font_size,
                );
            let origin_y = abs_border.y
                + resolve_origin_dimension(
                    &style.transform_origin.y,
                    abs_border.height,
                    style.font_size,
                );
            match inverse_transform_point(
                point,
                &style.transform,
                origin_x,
                origin_y,
                abs_border.width,
                abs_border.height,
            ) {
                Some(p) => p,
                None => return None, // Singular transform — can't hit
            }
        } else {
            point
        };

        // Does this box clip its descendants? (overflow != visible on either
        // axis). Used both for the subtree-cull decision (t49-e4-02) and for
        // building the child clip below.
        let clips_children = matches!(
            style.overflow_x,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
        ) || matches!(
            style.overflow_y,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
        );

        // ── Inherited clip check (t49-e4-03) ──────────────────────────
        // The inherited `clip` was built in this box's *incoming* coordinate
        // space (the parent's local space) — the same space as `point`. It
        // must be tested against `point`, NOT `local_point` (which has been
        // inverse-transformed into this box's own local space). Testing the
        // ancestor-space clip against the post-transform `local_point` mixes
        // coordinate spaces and mis-clips transformed descendants.
        // The inherited clip gates the whole subtree: if `point` is outside
        // it, neither this box nor any descendant is visible.
        if !clip.contains(point) {
            return None;
        }

        // ── Bounds check (t49-e4-02) ──────────────────────────────────
        // Whether the point falls inside this box's own border (and rounded
        // corners). A miss here means this box itself is not a self-hit, but
        // descendants painted outside the border box (overflow: visible,
        // absolutely-positioned beyond bounds, negative margins) may still be
        // hittable — so we only cull the subtree here when this box CLIPS its
        // children. Non-clipping boxes still recurse.
        let abs_border = layout_box.border_rect.offset(ox, oy);
        let mut self_in_bounds = abs_border.contains(local_point);

        // Shape-aware corner cull (border-radius). Reject self-hits that fall
        // inside the border rect but outside the rounded-corner quadrants.
        // Leaves the vast majority of nodes (radius = 0) untouched.
        if self_in_bounds
            && (!style.border_radius.top_left.is_zero()
                || !style.border_radius.top_right.is_zero()
                || !style.border_radius.bottom_right.is_zero()
                || !style.border_radius.bottom_left.is_zero())
            && !point_inside_rounded_rect(local_point, &abs_border, &style.border_radius)
        {
            self_in_bounds = false;
        }

        // Subtree cull: a clipping box confines its descendants to its padding
        // box (⊆ border box), so a border-box miss means nothing inside is
        // hittable. A non-clipping box must still recurse.
        if !self_in_bounds && clips_children {
            return None;
        }

        // ── Compute child clip rect ───────────────────────────────────
        // If this box has overflow != visible, it establishes a new clip
        // region (the padding box) for descendants.
        //
        // Coordinate spaces (t49-e4-03): `abs_padding` is in this box's local
        // space, while the inherited `clip` is in the incoming (ancestor)
        // space. When a transform is present those spaces differ, so we cannot
        // intersect them as axis-aligned rects. The inherited clip has already
        // been enforced for this subtree via `clip.contains(point)` above, so
        // when this box is transformed we re-base the child clip to local
        // space starting from this box's own padding box. Without a transform
        // the two spaces coincide and we intersect as before.
        let child_clip = if clips_children {
            let abs_padding = layout_box.padding_rect.offset(ox, oy);
            if style.transform.is_empty() {
                clip.intersect_rect(abs_padding)
            } else {
                ClipComplexity::rect(abs_padding)
            }
        } else if style.transform.is_empty() {
            clip.clone()
        } else {
            // Transform without its own clip: the inherited clip lives in the
            // ancestor space and cannot be carried across the transform as an
            // axis-aligned rect. It was already enforced at this level; child
            // hit-testing proceeds unclipped in local space.
            ClipComplexity::Trivial
        };

        // ── Child offsets (content origin, minus scroll) ──────────────
        let (scroll_x, scroll_y) = layout_box.scroll_offset;
        let child_offset = (
            ox + layout_box.content_rect.x - scroll_x,
            oy + layout_box.content_rect.y - scroll_y,
        );

        // ── Traverse children in CSS 2.1 §E stacking order ───────────
        // The topmost visual layer is tested first, so we traverse in
        // reverse painting order: 6→5→4→3→2→1.
        let children = layout_box.children.clone();

        let mut negative_z: Vec<(LayoutBoxId, i32)> = Vec::new();
        let mut in_flow_block: Vec<LayoutBoxId> = Vec::new();
        let mut floats: Vec<LayoutBoxId> = Vec::new();
        let mut in_flow_inline: Vec<LayoutBoxId> = Vec::new();
        let mut z_auto_or_zero: Vec<LayoutBoxId> = Vec::new();
        let mut positive_z: Vec<(LayoutBoxId, i32)> = Vec::new();

        for &child_id in &children {
            let child_style = self
                .layout
                .get(child_id)
                .and_then(|cb| self.styles.get(cb.node))
                .cloned();
            let child_display = child_style
                .as_ref()
                .map(|s| s.display)
                .unwrap_or(Display::Block);
            let child_position = child_style
                .as_ref()
                .map(|s| s.position)
                .unwrap_or(Position::Static);
            let child_z = child_style.as_ref().and_then(|s| s.z_index);
            let is_positioned = matches!(
                child_position,
                Position::Relative | Position::Absolute | Position::Fixed | Position::Sticky
            );
            let is_float = child_style
                .as_ref()
                .map(|s| s.float != Float::None)
                .unwrap_or(false);

            if is_positioned {
                match child_z {
                    Some(z) if z < 0 => negative_z.push((child_id, z)),
                    Some(z) if z > 0 => positive_z.push((child_id, z)),
                    _ => z_auto_or_zero.push(child_id),
                }
            } else if is_float {
                floats.push(child_id);
            } else if matches!(
                child_display,
                Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
            ) {
                in_flow_inline.push(child_id);
            } else {
                in_flow_block.push(child_id);
            }
        }

        // Sort by z-index (ascending for painting, we'll reverse for hit-test)
        negative_z.sort_by_key(|&(_, z)| z);
        positive_z.sort_by_key(|&(_, z)| z);

        // Hit-test in reverse painting order (highest z-index first):
        // 6. Positive z-index (highest first)
        for &(child_id, _) in positive_z.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }
        // 5. Positioned with z-index auto or 0 (reverse DOM order)
        for &child_id in z_auto_or_zero.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }
        // 4. In-flow inline (reverse DOM order)
        for &child_id in in_flow_inline.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }
        // 3. Floats (reverse DOM order)
        for &child_id in floats.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }
        // 2. In-flow block (reverse DOM order)
        for &child_id in in_flow_block.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }
        // 1. Negative z-index (highest-first, so reverse the sorted list)
        for &(child_id, _) in negative_z.iter().rev() {
            if let Some(result) =
                self.hit_test_box(child_id, local_point, child_offset, &child_clip)
            {
                return Some(result);
            }
        }

        // ── No child matched — this box is the target ─────────────────
        // But only if the point is actually within this box's own bounds
        // (t49-e4-02: a non-clipping box we recursed into for the sake of its
        // out-of-bounds children is not itself a self-hit), visibility is not
        // hidden, and pointer-events allows it.
        if !self_in_bounds || style.visibility == Visibility::Hidden || !this_receives_events {
            return None;
        }

        // Generated-content boxes (::before / ::after) are non-interactive per
        // CSS: they must NOT register a hit on the host node at their own rect
        // (t88-p0a). They carry the host node id only as a style back-reference.
        if matches!(layout_box.box_type, liquide_layout::tree::BoxType::PseudoElement { .. }) {
            return None;
        }

        let abs_content = layout_box.content_rect.offset(ox, oy);
        let point_in_node =
            Point::new(local_point.x - abs_content.x, local_point.y - abs_content.y);

        // Build ancestor chain by walking up via parent pointers
        let mut ancestors = Vec::new();
        let mut current_id = layout_box.parent;
        while let Some(pid) = current_id {
            if let Some(parent_box) = self.layout.get(pid) {
                ancestors.push(parent_box.node);
                current_id = parent_box.parent;
            } else {
                break;
            }
        }

        Some(HitTestResult {
            node: layout_box.node,
            point_in_node,
            bounds: abs_border,
            ancestors,
        })
    }

    fn hit_test_box_all(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
        clip: &ClipComplexity,
        results: &mut Vec<HitTestResult>,
    ) {
        let layout_box = match self.layout.get(box_id) {
            Some(b) => b,
            None => return,
        };
        let style = self
            .styles
            .get(layout_box.node)
            .cloned()
            .unwrap_or_default();
        let (ox, oy) = paint_offset;

        // Visibility / pointer-events checks
        if style.display == Display::None {
            return;
        }
        if style.content_visibility == ContentVisibility::Hidden {
            return;
        }
        // pointer-events: none — skip self but still recurse children
        // (children may have pointer-events: auto)
        let this_receives_events = style.pointer_events != PointerEvents::None;

        // Transform
        let local_point = if !style.transform.is_empty() {
            let abs_border = layout_box.border_rect.offset(ox, oy);
            // Compute transform origin from style (matching painter behavior)
            let origin_x = abs_border.x
                + resolve_origin_dimension(
                    &style.transform_origin.x,
                    abs_border.width,
                    style.font_size,
                );
            let origin_y = abs_border.y
                + resolve_origin_dimension(
                    &style.transform_origin.y,
                    abs_border.height,
                    style.font_size,
                );
            match inverse_transform_point(
                point,
                &style.transform,
                origin_x,
                origin_y,
                abs_border.width,
                abs_border.height,
            ) {
                Some(p) => p,
                None => return,
            }
        } else {
            point
        };

        // Does this box clip its descendants? (overflow != visible). Mirrors
        // hit_test_box; drives both the subtree-cull decision (t49-e4-02) and
        // the child clip (t49-e4-03).
        let clips_children = matches!(
            style.overflow_x,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
        ) || matches!(
            style.overflow_y,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip
        );

        // Inherited clip check (t49-e4-03): the inherited `clip` is in the
        // incoming (ancestor) coordinate space — the same space as `point` —
        // so it must be tested against `point`, not the inverse-transformed
        // `local_point`. It gates the whole subtree.
        if !clip.contains(point) {
            return;
        }

        // Bounds check (t49-e4-02): a border-box miss means this box itself is
        // not a self-hit, but its out-of-bounds descendants may still be hit.
        // Only cull the subtree when this box clips its children.
        let abs_border = layout_box.border_rect.offset(ox, oy);
        let mut self_in_bounds = abs_border.contains(local_point);

        // Shape-aware corner cull (border-radius).
        if self_in_bounds
            && (!style.border_radius.top_left.is_zero()
                || !style.border_radius.top_right.is_zero()
                || !style.border_radius.bottom_right.is_zero()
                || !style.border_radius.bottom_left.is_zero())
            && !point_inside_rounded_rect(local_point, &abs_border, &style.border_radius)
        {
            self_in_bounds = false;
        }

        if !self_in_bounds && clips_children {
            return;
        }

        // Add this box (only if the point is within its own bounds, and unless
        // visibility:hidden or pointer-events:none). Generated-content boxes
        // (::before / ::after) are non-interactive and never register a hit on
        // the host node (t88-p0a).
        let is_generated =
            matches!(layout_box.box_type, liquide_layout::tree::BoxType::PseudoElement { .. });
        if self_in_bounds
            && !is_generated
            && style.visibility != Visibility::Hidden
            && this_receives_events
        {
            let abs_content = layout_box.content_rect.offset(ox, oy);
            let point_in_node =
                Point::new(local_point.x - abs_content.x, local_point.y - abs_content.y);
            results.push(HitTestResult {
                node: layout_box.node,
                point_in_node,
                bounds: abs_border,
                ancestors: Vec::new(),
            });
        }

        // Child clip (t49-e4-03): mirror hit_test_box — re-base to local space
        // across a transform rather than intersecting across coordinate spaces.
        let child_clip = if clips_children {
            let abs_padding = layout_box.padding_rect.offset(ox, oy);
            if style.transform.is_empty() {
                clip.intersect_rect(abs_padding)
            } else {
                ClipComplexity::rect(abs_padding)
            }
        } else if style.transform.is_empty() {
            clip.clone()
        } else {
            ClipComplexity::Trivial
        };

        let (scroll_x, scroll_y) = layout_box.scroll_offset;
        let child_offset = (
            ox + layout_box.content_rect.x - scroll_x,
            oy + layout_box.content_rect.y - scroll_y,
        );
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            self.hit_test_box_all(child_id, local_point, child_offset, &child_clip, results);
        }
    }
}

// ── Helper functions ──────────────────────────────────────────────────────

/// Resolve a transform-origin dimension to pixels.
///
/// For transform-origin, percentages are relative to the box size.
/// This matches the painter's resolve_origin_dimension to ensure
/// paint and hit-test use identical transform origins.
fn resolve_origin_dimension(dim: &Dimension, box_size: f32, font_size: f32) -> f32 {
    match dim {
        Dimension::Px(v) => *v,
        Dimension::Percent(p) => box_size * p / 100.0,
        Dimension::Em(v) => v * font_size,
        Dimension::Rem(v) => v * font_size, // Use element font size as best approximation
        Dimension::Zero => 0.0,
        // For other dimensions, default to center (50%)
        _ => box_size * 0.5,
    }
}

/// Inverse-transform a screen-space point into the element's local coordinate
/// space, accounting for the CSS transform and its origin.
///
/// Returns `None` if the transform is singular (non-invertible).
fn inverse_transform_point(
    point: Point,
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
) -> Option<Point> {
    // Use the same matrix composition as the painter (Affine2D convention)
    let transform = compose_transform_matrix(transforms, origin_x, origin_y, box_width, box_height);

    // Invert the Affine2D matrix
    // Affine2D: x' = a*x + b*y + tx, y' = c*x + d*y + ty
    // Matrix: [a b tx; c d ty; 0 0 1]
    // Determinant of 2x2 part: a*d - b*c
    let det = transform.a * transform.d - transform.b * transform.c;
    if det.abs() < 1e-10 {
        return None; // Singular — can't hit-test
    }
    let inv_det = 1.0 / det;

    // Inverse of [a b; c d] is 1/det * [d -b; -c a]
    let inv_a = transform.d * inv_det;
    let inv_b = -transform.b * inv_det;
    let inv_c = -transform.c * inv_det;
    let inv_d = transform.a * inv_det;

    // Translation part of inverse: -(M^-1 * t)
    let inv_tx = -(inv_a * transform.tx + inv_b * transform.ty);
    let inv_ty = -(inv_c * transform.tx + inv_d * transform.ty);

    // Apply inverse transform: p_local = M^-1 * p_screen
    Some(Point::new(
        inv_a * point.x + inv_b * point.y + inv_tx,
        inv_c * point.x + inv_d * point.y + inv_ty,
    ))
}

/// Compose a list of CSS transforms into a single 2D affine matrix.
///
/// This uses the EXACT same algorithm as the painter's compose_transform_matrix
/// to ensure paint and hit-test transforms match precisely.
///
/// The resulting Affine2D uses the convention:
///   x' = a * x + b * y + tx
///   y' = c * x + d * y + ty
fn compose_transform_matrix(
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
) -> Affine2D {
    // Start with identity matrix
    let mut a = 1.0f32;
    let mut b = 0.0f32;
    let mut c = 0.0f32;
    let mut d = 1.0f32;
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;

    // Matrix multiplication: current = current * new_matrix
    // [a  b  tx]   [na nb ne]   [a*na+b*nc  a*nb+b*nd  a*ne+b*nf+tx]
    // [c  d  ty] * [nc nd nf] = [c*na+d*nc  c*nb+d*nd  c*ne+d*nf+ty]
    // [0  0  1 ]   [0  0  1 ]   [0          0          1           ]
    let mut mul = |na: f32, nb: f32, nc: f32, nd: f32, ne: f32, nf: f32| {
        let new_a = a * na + b * nc;
        let new_b = a * nb + b * nd;
        let new_c = c * na + d * nc;
        let new_d = c * nb + d * nd;
        let new_tx = a * ne + b * nf + tx;
        let new_ty = c * ne + d * nf + ty;
        a = new_a;
        b = new_b;
        c = new_c;
        d = new_d;
        tx = new_tx;
        ty = new_ty;
    };

    // Pre-translate by +origin (move origin to coordinate system origin)
    mul(1.0, 0.0, 0.0, 1.0, origin_x, origin_y);

    // Apply transforms in order
    for t in transforms {
        match t {
            Transform::Translate(x, y) => {
                // translate(%) resolves against the element's own box: X% width, Y% height.
                mul(1.0, 0.0, 0.0, 1.0, x.resolve(box_width), y.resolve(box_height));
            }
            Transform::Scale(sx, sy) => {
                mul(*sx, 0.0, 0.0, *sy, 0.0, 0.0);
            }
            Transform::Rotate(deg) => {
                let r = deg.to_radians();
                let cos_r = r.cos();
                let sin_r = r.sin();
                // Rotation matrix for Affine2D: [cos, -sin; sin, cos]
                mul(cos_r, -sin_r, sin_r, cos_r, 0.0, 0.0);
            }
            Transform::Skew(ax, ay) => {
                let tan_ax = ax.to_radians().tan();
                let tan_ay = ay.to_radians().tan();
                // Skew matrix for Affine2D: [1, tan(ax); tan(ay), 1]
                mul(1.0, tan_ax, tan_ay, 1.0, 0.0, 0.0);
            }
            Transform::Matrix(ma, mb, mc, md, me, mf) => {
                // CSS matrix(a, b, c, d, e, f) = [a c e; b d f; 0 0 1]
                // Affine2D uses [a b tx; c d ty; 0 0 1]
                // So CSS (a,b,c,d,e,f) maps to Affine2D (a, c, b, d, e, f)
                mul(*ma, *mc, *mb, *md, *me, *mf);
            }
            // TODO(t9 Phase 2): real impl for 3D transforms / Matrix3d / Perspective
            // in hit-test affine projection. Ignored here to preserve 2D behaviour.
            _ => {}
        }
    }

    // Post-translate by -origin (restore origin shift)
    mul(1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y);

    Affine2D { a, b, c, d, tx, ty }
}

/// Test whether `point` lies inside the rounded-rectangle defined by `rect`
/// and the four corner radii in `radii`.
///
/// The caller must have already verified that `rect.contains(point)`; this
/// function only rejects points that lie in the four corner squares but
/// outside the quarter-ellipses that define the rounded corners.
///
/// Radii are clamped so that adjacent radii sum to at most the corresponding
/// side length (per CSS §5.1 border-radius spec).
pub fn point_inside_rounded_rect(
    point: Point,
    rect: &Rect,
    radii: &Corners<EllipticalRadius>,
) -> bool {
    // Clamp radii: per CSS, if the sum of adjacent radii exceeds the side
    // length, scale every radius by the same factor `f = 1/max(ratio)`.
    let (tl, tr, br, bl) = clamp_radii(rect, radii);

    let px = point.x;
    let py = point.y;
    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = x0 + rect.width;
    let y1 = y0 + rect.height;

    // Top-left quadrant
    if px < x0 + tl.x && py < y0 + tl.y {
        return point_inside_quarter_ellipse(px, py, x0 + tl.x, y0 + tl.y, tl.x, tl.y);
    }
    // Top-right quadrant
    if px > x1 - tr.x && py < y0 + tr.y {
        return point_inside_quarter_ellipse(px, py, x1 - tr.x, y0 + tr.y, tr.x, tr.y);
    }
    // Bottom-right quadrant
    if px > x1 - br.x && py > y1 - br.y {
        return point_inside_quarter_ellipse(px, py, x1 - br.x, y1 - br.y, br.x, br.y);
    }
    // Bottom-left quadrant
    if px < x0 + bl.x && py > y1 - bl.y {
        return point_inside_quarter_ellipse(px, py, x0 + bl.x, y1 - bl.y, bl.x, bl.y);
    }
    // Not in any corner region — the bounding-rect test already passed.
    true
}

#[inline]
fn point_inside_quarter_ellipse(px: f32, py: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    if rx <= 0.0 || ry <= 0.0 {
        return true;
    }
    let dx = (px - cx) / rx;
    let dy = (py - cy) / ry;
    dx * dx + dy * dy <= 1.0
}

fn clamp_radii(
    rect: &Rect,
    radii: &Corners<EllipticalRadius>,
) -> (
    EllipticalRadius,
    EllipticalRadius,
    EllipticalRadius,
    EllipticalRadius,
) {
    let tl = radii.top_left;
    let tr = radii.top_right;
    let br = radii.bottom_right;
    let bl = radii.bottom_left;

    let w = rect.width.max(0.0);
    let h = rect.height.max(0.0);

    // Per CSS border-radius §5.1: compute f = min(side/sum) across all
    // sides where sum > side. If f < 1, scale all radii by f.
    let f = [
        side_factor(tl.x + tr.x, w),
        side_factor(bl.x + br.x, w),
        side_factor(tl.y + bl.y, h),
        side_factor(tr.y + br.y, h),
    ]
    .into_iter()
    .fold(1.0f32, f32::min);

    if f < 1.0 {
        (
            EllipticalRadius {
                x: tl.x * f,
                y: tl.y * f,
            },
            EllipticalRadius {
                x: tr.x * f,
                y: tr.y * f,
            },
            EllipticalRadius {
                x: br.x * f,
                y: br.y * f,
            },
            EllipticalRadius {
                x: bl.x * f,
                y: bl.y * f,
            },
        )
    } else {
        (tl, tr, br, bl)
    }
}

#[inline]
fn side_factor(sum: f32, side: f32) -> f32 {
    if side <= 0.0 || sum <= 0.0 || sum <= side {
        1.0
    } else {
        side / sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
    use liquide_style_engine::computed::LengthPercent;
    use liquide_style_engine::engine::StyleEngine;

    /// t88-p0a: a generated-content `::before` box must NOT register a hit on
    /// the host node — generated content is non-interactive. The hit at the
    /// pseudo box's location must resolve to the host's REAL block box (full
    /// width), and `hit_test_all` must not contain a result whose bounds are the
    /// small pseudo box. Pre-fix the pseudo box (sharing the host node id) hit at
    /// its own small rect.
    #[test]
    fn pseudo_element_box_is_not_interactive() {
        let mut doc = Document::new();
        let root = doc.root();
        let host = doc.create_element("host");
        doc.append_child(root, host);

        let mut se = StyleEngine::default();
        se.add_stylesheet(
            r#"host { display: block; width: 200px; height: 40px; }
               host::before { content: ""; width: 20px; height: 20px; }"#,
        );
        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);
        let engine = HitTestEngine::from_owned(layout_tree, style_map);

        // Point inside the pseudo box (top-left 20x20 region).
        let hit = engine.hit_test(Point::new(5.0, 5.0)).expect("host should be hit");
        assert_eq!(hit.node, host, "hit must resolve to the host element");
        assert!(
            hit.bounds.width > 100.0,
            "hit must be the host's full block box, not the small pseudo box (got width {})",
            hit.bounds.width
        );

        // No result in hit_test_all should carry the small pseudo bounds.
        let all = engine.hit_test_all(Point::new(5.0, 5.0));
        assert!(
            all.iter().all(|r| r.bounds.width > 100.0),
            "no hit result may have the small (20px) pseudo-box bounds"
        );
    }

    #[test]
    fn shape_aware_hit_test_rounded_corner_cull() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let radii = Corners {
            top_left: EllipticalRadius { x: 20.0, y: 20.0 },
            top_right: EllipticalRadius { x: 20.0, y: 20.0 },
            bottom_right: EllipticalRadius { x: 20.0, y: 20.0 },
            bottom_left: EllipticalRadius { x: 20.0, y: 20.0 },
        };
        // Center of box is inside.
        assert!(point_inside_rounded_rect(
            Point::new(50.0, 50.0),
            &rect,
            &radii
        ));
        // Mid-edge is inside.
        assert!(point_inside_rounded_rect(
            Point::new(50.0, 0.5),
            &rect,
            &radii
        ));
        // Extreme top-left corner (0,0) is outside the rounded edge.
        assert!(!point_inside_rounded_rect(
            Point::new(0.5, 0.5),
            &rect,
            &radii
        ));
        // Extreme top-right corner.
        assert!(!point_inside_rounded_rect(
            Point::new(99.5, 0.5),
            &rect,
            &radii
        ));
        // Extreme bottom-right corner.
        assert!(!point_inside_rounded_rect(
            Point::new(99.5, 99.5),
            &rect,
            &radii
        ));
        // Inside the corner quadrant but inside the ellipse is OK.
        assert!(point_inside_rounded_rect(
            Point::new(15.0, 15.0),
            &rect,
            &radii
        ));
    }

    #[test]
    fn shape_aware_zero_radius_accepts_all_inside() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let radii = Corners {
            top_left: EllipticalRadius { x: 0.0, y: 0.0 },
            top_right: EllipticalRadius { x: 0.0, y: 0.0 },
            bottom_right: EllipticalRadius { x: 0.0, y: 0.0 },
            bottom_left: EllipticalRadius { x: 0.0, y: 0.0 },
        };
        assert!(point_inside_rounded_rect(
            Point::new(0.5, 0.5),
            &rect,
            &radii
        ));
        assert!(point_inside_rounded_rect(
            Point::new(99.5, 99.5),
            &rect,
            &radii
        ));
    }

    #[test]
    fn basic_hit_test() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 200px; height: 100px; }");

        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let engine = HitTestEngine::from_owned(layout_tree, style_map);
        let result = engine.hit_test(Point::new(100.0, 50.0));

        assert!(result.is_some(), "Should hit something within the viewport");
    }

    /// Test that transform + inverse_transform is identity (round-trip)
    #[test]
    fn transform_inverse_round_trip() {
        // Test rotation
        let transforms = vec![Transform::Rotate(45.0)];
        let origin_x = 50.0;
        let origin_y = 50.0;

        let matrix = compose_transform_matrix(&transforms, origin_x, origin_y, 0.0, 0.0);
        let original = Point::new(75.0, 50.0);

        // Forward transform using Affine2D
        let transformed = matrix.transform_point(liquide_compositor::geometry::Point::new(
            original.x, original.y,
        ));
        let transformed_point = Point::new(transformed.x, transformed.y);

        // Inverse transform
        let recovered =
            inverse_transform_point(transformed_point, &transforms, origin_x, origin_y, 0.0, 0.0);

        assert!(recovered.is_some(), "Should be able to inverse transform");
        let recovered = recovered.unwrap();

        assert!(
            (recovered.x - original.x).abs() < 0.001,
            "X should round-trip: {} vs {}",
            recovered.x,
            original.x
        );
        assert!(
            (recovered.y - original.y).abs() < 0.001,
            "Y should round-trip: {} vs {}",
            recovered.y,
            original.y
        );
    }

    /// Test that multiple transforms compose correctly and round-trip
    #[test]
    fn multiple_transforms_round_trip() {
        // Rotate then scale then translate
        let transforms = vec![
            Transform::Rotate(30.0),
            Transform::Scale(2.0, 1.5),
            Transform::Translate(LengthPercent::Px(100.0), LengthPercent::Px(50.0)),
        ];
        let origin_x = 100.0;
        let origin_y = 100.0;

        let matrix = compose_transform_matrix(&transforms, origin_x, origin_y, 0.0, 0.0);
        let original = Point::new(150.0, 75.0);

        // Forward transform
        let transformed = matrix.transform_point(liquide_compositor::geometry::Point::new(
            original.x, original.y,
        ));
        let transformed_point = Point::new(transformed.x, transformed.y);

        // Inverse transform
        let recovered =
            inverse_transform_point(transformed_point, &transforms, origin_x, origin_y, 0.0, 0.0);

        assert!(recovered.is_some(), "Should be able to inverse transform");
        let recovered = recovered.unwrap();

        assert!(
            (recovered.x - original.x).abs() < 0.001,
            "X should round-trip: {} vs {}",
            recovered.x,
            original.x
        );
        assert!(
            (recovered.y - original.y).abs() < 0.001,
            "Y should round-trip: {} vs {}",
            recovered.y,
            original.y
        );
    }

    /// Test skew transform round-trip
    #[test]
    fn skew_transform_round_trip() {
        let transforms = vec![Transform::Skew(15.0, 10.0)];
        let origin_x = 50.0;
        let origin_y = 50.0;

        let matrix = compose_transform_matrix(&transforms, origin_x, origin_y, 0.0, 0.0);
        let original = Point::new(80.0, 60.0);

        let transformed = matrix.transform_point(liquide_compositor::geometry::Point::new(
            original.x, original.y,
        ));
        let transformed_point = Point::new(transformed.x, transformed.y);

        let recovered =
            inverse_transform_point(transformed_point, &transforms, origin_x, origin_y, 0.0, 0.0);

        assert!(recovered.is_some());
        let recovered = recovered.unwrap();

        assert!((recovered.x - original.x).abs() < 0.001);
        assert!((recovered.y - original.y).abs() < 0.001);
    }
}
