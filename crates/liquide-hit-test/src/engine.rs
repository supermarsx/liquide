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

use liquide_dom::NodeId;
use liquide_layout::geometry::{Point, Rect};
use liquide_layout::tree::{LayoutBoxId, LayoutTree};
use liquide_style_engine::computed::{
    ContentVisibility, Display, Float, Overflow, PointerEvents, Position, Visibility,
};
use liquide_style_engine::StyleMap;

/// Result of a hit test.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// The target DOM node.
    pub node: NodeId,
    /// The point relative to the node's content box.
    pub point_in_node: Point,
    /// Ancestor chain from target up to root (bubble path).
    pub ancestors: Vec<NodeId>,
}

/// The hit test engine.
pub struct HitTestEngine {
    /// Cached layout tree reference.
    layout: LayoutTree,
    /// Cached style map.
    styles: StyleMap,
}

impl HitTestEngine {
    /// Create a new hit test engine.
    pub fn new(layout: LayoutTree, styles: StyleMap) -> Self {
        Self { layout, styles }
    }

    /// Update with new layout/styles (after relayout).
    pub fn update(&mut self, layout: LayoutTree, styles: StyleMap) {
        self.layout = layout;
        self.styles = styles;
    }

    /// Get a reference to the layout tree.
    pub fn layout(&self) -> &LayoutTree {
        &self.layout
    }

    /// Get a mutable reference to the layout tree (for scroll offset updates).
    pub fn layout_mut(&mut self) -> &mut LayoutTree {
        &mut self.layout
    }

    /// Get a reference to the style map.
    pub fn styles(&self) -> &StyleMap {
        &self.styles
    }

    /// Hit test a single point. Returns the topmost matching node.
    pub fn hit_test(&self, point: Point) -> Option<HitTestResult> {
        self.hit_test_box(self.layout.root, point, (0.0, 0.0), None)
    }

    /// Hit test all overlapping nodes at a point (front to back).
    pub fn hit_test_all(&self, point: Point) -> Vec<HitTestResult> {
        let mut results = Vec::new();
        self.hit_test_box_all(self.layout.root, point, (0.0, 0.0), None, &mut results);
        results
    }

    /// Core hit-test for a single box, returning the topmost match.
    ///
    /// `clip_rect` is the active overflow clip in absolute coordinates.
    /// When a parent has `overflow` != `visible`, it constrains the hit
    /// region for all descendants.
    fn hit_test_box(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
        clip_rect: Option<Rect>,
    ) -> Option<HitTestResult> {
        let layout_box = self.layout.get(box_id)?;
        let style = self.styles.get(layout_box.node).cloned().unwrap_or_default();
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
        // pointer-events: none — skip this and children
        if style.pointer_events == PointerEvents::None {
            return None;
        }

        // ── Transform: inverse-map the point into local space ─────────
        let local_point = if !style.transform.is_empty() {
            let abs_border = layout_box.border_rect.offset(ox, oy);
            // Transform origin defaults to center of the border box
            let origin_x = abs_border.x + abs_border.width * 0.5;
            let origin_y = abs_border.y + abs_border.height * 0.5;
            match inverse_transform_point(point, &style.transform, origin_x, origin_y) {
                Some(p) => p,
                None => return None, // Singular transform — can't hit
            }
        } else {
            point
        };

        // ── Bounds check ──────────────────────────────────────────────
        let abs_border = layout_box.border_rect.offset(ox, oy);
        if !abs_border.contains(local_point) {
            return None;
        }

        // ── Clip check ────────────────────────────────────────────────
        // If there's an active clip from a parent, reject if outside it.
        if let Some(ref cr) = clip_rect {
            if !cr.contains(local_point) {
                return None;
            }
        }

        // ── Compute child clip rect ───────────────────────────────────
        // If this box has overflow != visible, it establishes a new clip
        // region (the padding box) for descendants.
        let child_clip = if matches!(style.overflow_x, Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip)
            || matches!(style.overflow_y, Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip)
        {
            let abs_padding = layout_box.padding_rect.offset(ox, oy);
            // Intersect with existing clip
            Some(match clip_rect {
                Some(existing) => intersect_rects(&existing, &abs_padding),
                None => abs_padding,
            })
        } else {
            clip_rect
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
            let child_style = self.layout.get(child_id)
                .and_then(|cb| self.styles.get(cb.node))
                .cloned();
            let child_display = child_style.as_ref().map(|s| s.display).unwrap_or(Display::Block);
            let child_position = child_style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
            let child_z = child_style.as_ref().and_then(|s| s.z_index);
            let is_positioned = matches!(
                child_position,
                Position::Relative | Position::Absolute | Position::Fixed | Position::Sticky
            );
            let is_float = child_style.as_ref().map(|s| s.float != Float::None).unwrap_or(false);

            if is_positioned {
                match child_z {
                    Some(z) if z < 0 => negative_z.push((child_id, z)),
                    Some(z) if z > 0 => positive_z.push((child_id, z)),
                    _ => z_auto_or_zero.push(child_id),
                }
            } else if is_float {
                floats.push(child_id);
            } else if matches!(child_display, Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid) {
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
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }
        // 5. Positioned with z-index auto or 0 (reverse DOM order)
        for &child_id in z_auto_or_zero.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }
        // 4. In-flow inline (reverse DOM order)
        for &child_id in in_flow_inline.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }
        // 3. Floats (reverse DOM order)
        for &child_id in floats.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }
        // 2. In-flow block (reverse DOM order)
        for &child_id in in_flow_block.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }
        // 1. Negative z-index (highest-first, so reverse the sorted list)
        for &(child_id, _) in negative_z.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, local_point, child_offset, child_clip) {
                return Some(result);
            }
        }

        // ── No child matched — this box is the target ─────────────────
        // But only if visibility is not hidden
        if style.visibility == Visibility::Hidden {
            return None;
        }

        let abs_content = layout_box.content_rect.offset(ox, oy);
        let point_in_node = Point::new(
            local_point.x - abs_content.x,
            local_point.y - abs_content.y,
        );

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
            ancestors,
        })
    }

    fn hit_test_box_all(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
        clip_rect: Option<Rect>,
        results: &mut Vec<HitTestResult>,
    ) {
        let layout_box = match self.layout.get(box_id) {
            Some(b) => b,
            None => return,
        };
        let style = self.styles.get(layout_box.node).cloned().unwrap_or_default();
        let (ox, oy) = paint_offset;

        // Visibility / pointer-events checks
        if style.display == Display::None {
            return;
        }
        if style.content_visibility == ContentVisibility::Hidden {
            return;
        }
        if style.pointer_events == PointerEvents::None {
            return;
        }

        // Transform
        let local_point = if !style.transform.is_empty() {
            let abs_border = layout_box.border_rect.offset(ox, oy);
            let origin_x = abs_border.x + abs_border.width * 0.5;
            let origin_y = abs_border.y + abs_border.height * 0.5;
            match inverse_transform_point(point, &style.transform, origin_x, origin_y) {
                Some(p) => p,
                None => return,
            }
        } else {
            point
        };

        let abs_border = layout_box.border_rect.offset(ox, oy);
        if !abs_border.contains(local_point) {
            return;
        }

        // Clip check
        if let Some(ref cr) = clip_rect {
            if !cr.contains(local_point) {
                return;
            }
        }

        // Add this box (unless visibility:hidden)
        if style.visibility != Visibility::Hidden {
            let abs_content = layout_box.content_rect.offset(ox, oy);
            let point_in_node = Point::new(
                local_point.x - abs_content.x,
                local_point.y - abs_content.y,
            );
            results.push(HitTestResult {
                node: layout_box.node,
                point_in_node,
                ancestors: Vec::new(),
            });
        }

        // Child clip
        let child_clip = if matches!(style.overflow_x, Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip)
            || matches!(style.overflow_y, Overflow::Hidden | Overflow::Scroll | Overflow::Auto | Overflow::Clip)
        {
            let abs_padding = layout_box.padding_rect.offset(ox, oy);
            Some(match clip_rect {
                Some(existing) => intersect_rects(&existing, &abs_padding),
                None => abs_padding,
            })
        } else {
            clip_rect
        };

        let (scroll_x, scroll_y) = layout_box.scroll_offset;
        let child_offset = (
            ox + layout_box.content_rect.x - scroll_x,
            oy + layout_box.content_rect.y - scroll_y,
        );
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            self.hit_test_box_all(child_id, local_point, child_offset, child_clip, results);
        }
    }
}

// ── Helper functions ──────────────────────────────────────────────────────

/// Intersect two rectangles, returning the overlap region.
fn intersect_rects(a: &Rect, b: &Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

/// Inverse-transform a screen-space point into the element's local coordinate
/// space, accounting for the CSS transform and its origin.
///
/// Returns `None` if the transform is singular (non-invertible).
fn inverse_transform_point(
    point: Point,
    transforms: &[liquide_style_engine::computed::Transform],
    origin_x: f32,
    origin_y: f32,
) -> Option<Point> {
    let (tx, ty, sx, sy, rotate_deg, skx_deg, _sky_deg) = flatten_transforms(transforms);

    // Build the forward affine matrix and invert it
    let (a, b, c, d, e, f) = build_transform_matrix(tx, ty, sx, sy, rotate_deg, skx_deg, origin_x, origin_y);

    // Invert the 2x2 part: [a c; b d]
    let det = a * d - b * c;
    if det.abs() < 1e-10 {
        return None; // Singular — can't hit-test
    }
    let inv_det = 1.0 / det;
    let inv_a = d * inv_det;
    let inv_b = -b * inv_det;
    let inv_c = -c * inv_det;
    let inv_d = a * inv_det;
    let inv_e = -(inv_a * e + inv_c * f);
    let inv_f = -(inv_b * e + inv_d * f);

    Some(Point::new(
        inv_a * point.x + inv_c * point.y + inv_e,
        inv_b * point.x + inv_d * point.y + inv_f,
    ))
}

/// Build a 2D affine matrix (a, b, c, d, e, f) from flattened transform components.
///
/// The matrix maps local coordinates to screen coordinates:
///   screen_x = a * local_x + c * local_y + e
///   screen_y = b * local_x + d * local_y + f
///
/// Incorporates transform-origin by pre/post translating.
fn build_transform_matrix(
    tx: f32, ty: f32,
    sx: f32, sy: f32,
    rotate_deg: f32,
    skew_x_deg: f32,
    origin_x: f32, origin_y: f32,
) -> (f32, f32, f32, f32, f32, f32) {
    let r = rotate_deg.to_radians();
    let cos_r = r.cos();
    let sin_r = r.sin();
    let tan_skx = skew_x_deg.to_radians().tan();

    // Combined = Rotate * SkewX * Scale
    // R = [cos_r, -sin_r; sin_r, cos_r]
    // SkewX = [1, tan_skx; 0, 1]
    // Scale = [sx, 0; 0, sy]
    // R * SkewX = [cos_r, cos_r*tan_skx - sin_r; sin_r, sin_r*tan_skx + cos_r]
    // (R * SkewX) * Scale = [cos_r*sx, (cos_r*tan_skx - sin_r)*sy; sin_r*sx, (sin_r*tan_skx + cos_r)*sy]
    let a = cos_r * sx;
    let b = sin_r * sx;
    let c = (cos_r * tan_skx - sin_r) * sy;
    let d = (sin_r * tan_skx + cos_r) * sy;

    // Apply transform-origin: translate(origin) * T * translate(-origin)
    // Where T includes translation (tx, ty) and the rotation/scale/skew matrix
    // Final: e = origin_x + tx - a*origin_x - c*origin_y
    //        f = origin_y + ty - b*origin_x - d*origin_y
    let e = origin_x + tx - a * origin_x - c * origin_y;
    let f = origin_y + ty - b * origin_x - d * origin_y;

    (a, b, c, d, e, f)
}

/// Flatten a list of CSS transforms into accumulated components.
/// Returns (translate_x, translate_y, scale_x, scale_y, rotate_deg, skew_x_deg, skew_y_deg).
fn flatten_transforms(transforms: &[liquide_style_engine::computed::Transform]) -> (f32, f32, f32, f32, f32, f32, f32) {
    use liquide_style_engine::computed::Transform;
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;
    let mut sx = 1.0f32;
    let mut sy = 1.0f32;
    let mut r = 0.0f32;
    let mut skx = 0.0f32;
    let mut sky = 0.0f32;

    for t in transforms {
        match t {
            Transform::Translate(x, y) => { tx += x; ty += y; }
            Transform::Scale(x, y) => { sx *= x; sy *= y; }
            Transform::Rotate(deg) => { r += deg; }
            Transform::Skew(ax, ay) => { skx += ax; sky += ay; }
            Transform::Matrix(a, b, c, d, e, f) => {
                tx += e; ty += f;
                let sx_m = (a * a + b * b).sqrt();
                let sy_m = (c * c + d * d).sqrt();
                if sx_m > 1e-6 { sx *= sx_m; }
                if sy_m > 1e-6 { sy *= sy_m; }
                let rot = b.atan2(*a).to_degrees();
                r += rot;
                if sx_m > 1e-6 && sy_m > 1e-6 {
                    let dot = a * c + b * d;
                    let skew_rad = (dot / (sx_m * sy_m)).asin();
                    skx += skew_rad.to_degrees();
                }
            }
        }
    }
    (tx, ty, sx, sy, r, skx, sky)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

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
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let engine = HitTestEngine::new(layout_tree, style_map);
        let result = engine.hit_test(Point::new(100.0, 50.0));

        assert!(result.is_some(), "Should hit something within the viewport");
    }
}
