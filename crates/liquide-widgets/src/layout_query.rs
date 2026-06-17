//! `LayoutQuery` — read a widget's hit geometry from the laid-out CSS box.
//!
//! ## Why this exists (the menu-hit-test-mismatch guard)
//!
//! The recurring failure class in this codebase is interaction code that hits
//! against a *hardcoded constant* (a fixed menu-item height, a magic thumb
//! width) instead of the box the layout engine actually produced. The two drift
//! apart the moment CSS changes, and clicks land off-target — the symptom the
//! user reported as "dead menus".
//!
//! `LayoutQuery` is the single, narrow seam through which all widget interaction
//! reads geometry. It wraps the [`HitTestEngine`]'s laid-out [`LayoutTree`] (via
//! `hit_test.layout()`) and exposes screen-space rectangles keyed by the DOM:
//!
//! - [`LayoutQuery::box_of`] — the absolute border rect of a node,
//! - [`LayoutQuery::content_of`] — the absolute content rect of a node,
//! - [`LayoutQuery::box_of_part`] — the rect of a sub-part, located by the
//!   `data-part` attribute under a widget root (NOT by index/constant),
//! - [`LayoutQuery::fraction_along`] — where a point falls along a box's axis as
//!   a 0..=1 fraction (the slider/scrollbar value math, derived from the laid-out
//!   track — never a constant).
//!
//! Because every behavior reads through this, a widget that computes a hit zone
//! from a constant instead of the layout box is *structurally* unable to pass a
//! test that drives the real pipeline (the box moves when CSS moves; the constant
//! does not). That is the no-fake-green tooth the S0 harness enforces.

use liquide_dom::{Document, NodeId};
use liquide_hit_test::HitTestEngine;
use liquide_layout::geometry::{Point, Rect};

/// A read-only adapter over a laid-out tree that answers geometry queries in
/// **screen space** for widget interaction code.
///
/// Holds a borrow of the [`HitTestEngine`] (whose `layout()` is the real,
/// post-layout [`liquide_layout::LayoutTree`]) plus the [`Document`] so it can
/// resolve `data-part` sub-elements under a widget root.
pub struct LayoutQuery<'a> {
    hit_test: &'a HitTestEngine,
    doc: &'a Document,
}

impl<'a> LayoutQuery<'a> {
    /// Wrap a hit-test engine + document. The engine MUST carry the layout tree
    /// produced by the same pipeline pass that styled `doc`.
    pub fn new(hit_test: &'a HitTestEngine, doc: &'a Document) -> Self {
        Self { hit_test, doc }
    }

    /// The absolute **border** rect of `node`, in screen space, as produced by
    /// layout. `None` when the node has no box (e.g. `display: none`).
    ///
    /// This is the canonical hit rectangle for a widget root — read it, never
    /// hardcode the widget's size.
    pub fn box_of(&self, node: NodeId) -> Option<Rect> {
        self.hit_test.bounds_for_node(node)
    }

    /// The absolute **content** rect of `node` (inside padding/border).
    pub fn content_of(&self, node: NodeId) -> Option<Rect> {
        self.hit_test.content_rect_for_node(node)
    }

    /// Find the box of a named sub-part under `widget_root`, located by its
    /// `data-part="<part>"` attribute.
    ///
    /// Sub-parts (a slider `thumb`/`fill`/`track`, a scrollbar `vthumb`, a
    /// checkbox `indicator`) are addressed by a stable semantic name, NOT by
    /// child index or a constant offset — so reflow / reorder / theming cannot
    /// silently mis-target them. Returns the absolute border rect of the first
    /// descendant (depth-first, document order) whose `data-part` matches.
    pub fn box_of_part(&self, widget_root: NodeId, part: &str) -> Option<Rect> {
        let node = self.find_part(widget_root, part)?;
        self.box_of(node)
    }

    /// Resolve the [`NodeId`] of a named sub-part under `widget_root`
    /// (`data-part="<part>"`), depth-first in document order. Includes the root
    /// itself if it carries the attribute.
    pub fn find_part(&self, widget_root: NodeId, part: &str) -> Option<NodeId> {
        self.find_part_rec(widget_root, part)
    }

    fn find_part_rec(&self, node: NodeId, part: &str) -> Option<NodeId> {
        if self
            .doc
            .get_attribute(node, "data-part")
            .as_deref()
            == Some(part)
        {
            return Some(node);
        }
        for &child in self.doc.children(node) {
            if let Some(found) = self.find_part_rec(child, part) {
                return Some(found);
            }
        }
        None
    }

    /// The fraction (0.0..=1.0) of `point` along `rect`'s horizontal axis,
    /// clamped to the box. Used by the slider / horizontal-scrollbar value math:
    /// the value derives from the laid-out **track box**, so a CSS change to the
    /// track width changes the value mapping automatically.
    ///
    /// A zero-width box returns `0.0` (degenerate; avoids NaN).
    pub fn fraction_along_x(rect: Rect, point: Point) -> f32 {
        if rect.width <= 0.0 {
            return 0.0;
        }
        ((point.x - rect.x) / rect.width).clamp(0.0, 1.0)
    }

    /// The fraction (0.0..=1.0) of `point` along `rect`'s vertical axis, clamped
    /// to the box. Vertical-scrollbar / vertical-slider analog of
    /// [`fraction_along_x`](Self::fraction_along_x).
    pub fn fraction_along_y(rect: Rect, point: Point) -> f32 {
        if rect.height <= 0.0 {
            return 0.0;
        }
        ((point.y - rect.y) / rect.height).clamp(0.0, 1.0)
    }

    /// Borrow the underlying hit-test engine (e.g. to run a raw point hit-test).
    pub fn hit_test(&self) -> &HitTestEngine {
        self.hit_test
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::StyleEngine;
    use std::sync::Arc;

    fn build(css: &str, body: impl FnOnce(&mut Document) -> NodeId) -> (Document, HitTestEngine) {
        let mut doc = Document::new();
        let _ = body(&mut doc);
        let mut se = StyleEngine::default();
        se.add_stylesheet(css);
        let styles = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
        let engine = HitTestEngine::new(Arc::new(layout), Arc::new(styles));
        (doc, engine)
    }

    #[test]
    fn box_of_reads_laid_out_rect_not_a_constant() {
        // The CSS — not the test — decides the size. If box_of returned a
        // constant it could not track this 123x45 box.
        let mut root_id = None;
        let (doc, engine) = build(
            "lq-box { display: block; width: 123px; height: 45px; }",
            |doc| {
                let root = doc.root();
                let el = doc.create_element("lq-box");
                doc.append_child(root, el);
                root_id = Some(el);
                el
            },
        );
        let q = LayoutQuery::new(&engine, &doc);
        let rect = q.box_of(root_id.unwrap()).expect("box must exist");
        assert!(
            (rect.width - 123.0).abs() < 1.0 && (rect.height - 45.0).abs() < 1.0,
            "box_of must read the laid-out rect (got {}x{})",
            rect.width,
            rect.height
        );
    }

    #[test]
    fn box_of_part_locates_by_data_part_attr() {
        let mut thumb_id = None;
        let (doc, engine) = build(
            "lq-slider { display: block; width: 200px; height: 20px; }
             [data-part=\"thumb\"] { display: block; width: 16px; height: 16px; }",
            |doc| {
                let root = doc.root();
                let slider = doc.create_element("lq-slider");
                let thumb = doc.create_element("lq-thumb");
                doc.set_attribute(thumb, "data-part", "thumb");
                doc.append_child(slider, thumb);
                doc.append_child(root, slider);
                thumb_id = Some((slider, thumb));
                slider
            },
        );
        let (slider, thumb) = thumb_id.unwrap();
        let q = LayoutQuery::new(&engine, &doc);
        assert_eq!(q.find_part(slider, "thumb"), Some(thumb));
        let rect = q.box_of_part(slider, "thumb").expect("thumb box");
        assert!(
            (rect.width - 16.0).abs() < 1.0,
            "thumb part must be located by data-part and read its laid-out width (got {})",
            rect.width
        );
    }

    #[test]
    fn fraction_along_x_derives_from_box_geometry() {
        let track = Rect::new(100.0, 0.0, 200.0, 10.0);
        // 50% point.
        assert!((LayoutQuery::fraction_along_x(track, Point::new(200.0, 5.0)) - 0.5).abs() < 1e-4);
        // Clamps below/above.
        assert_eq!(LayoutQuery::fraction_along_x(track, Point::new(0.0, 5.0)), 0.0);
        assert_eq!(LayoutQuery::fraction_along_x(track, Point::new(999.0, 5.0)), 1.0);
        // Degenerate.
        assert_eq!(
            LayoutQuery::fraction_along_x(Rect::new(0.0, 0.0, 0.0, 0.0), Point::new(5.0, 5.0)),
            0.0
        );
    }
}
