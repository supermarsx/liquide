//! Dirty tracking flags for incremental style/layout/paint.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::NodeId;

/// How far down the Style → Layout → Paint pipeline a single property change
/// must invalidate.
///
/// This is the *granularity* split that lets a provably paint-only property
/// change (a `:hover` recolour, an `opacity`/`box-shadow`/`border-color` tweak)
/// repaint without first re-running layout. The classification is deliberately
/// CONSERVATIVE: only properties that *cannot* affect box geometry, intrinsic
/// size, or fragmentation are classified [`DirtyKind::PaintOnly`]; everything
/// else — and every unknown / shorthand / custom (`--*`) property — falls back
/// to [`DirtyKind::Layout`]. Mis-classifying a geometry property as paint-only
/// would leave stale layout, so the only safe error direction is toward
/// `Layout`.
///
/// The set below mirrors the (separately-tested) `classify_style_property`
/// table in `liquide-style-engine`'s `impact.rs`, restricted to its strict
/// `PAINT_ONLY` (no `LAYOUT_GEOMETRY`, no `INTRINSIC_MEASURE`) members. It is
/// kept here — rather than depending on that crate — so `liquide-dom` stays
/// dependency-free and the layering boundary is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyKind {
    /// The change can only alter paint output (colours, shadows, decoration,
    /// visibility, compositor-only effects). Layout box is unchanged.
    PaintOnly,
    /// The change may alter geometry / intrinsic size / box participation, so
    /// layout (and therefore paint) must be recomputed. This is the safe
    /// default for anything not provably paint-only.
    Layout,
}

/// Classify a CSS property name by how far it must invalidate the pipeline.
///
/// Returns [`DirtyKind::PaintOnly`] ONLY for properties that are provably
/// incapable of changing layout geometry or intrinsic measurement; every other
/// property — including unknown names, shorthands whose longhands include
/// geometry, and custom properties — returns [`DirtyKind::Layout`].
///
/// Note `font-size`, `font-weight`, `line-height`, `letter/word-spacing`,
/// `width`/`height`, padding/margin/border-*width*, `display`, `content`,
/// `white-space`, `tab-size`, etc. are intrinsic/geometry and therefore
/// **Layout** — they are intentionally absent from the paint-only set.
#[must_use]
pub fn classify_property(property: &str) -> DirtyKind {
    let normalized = property.trim().to_ascii_lowercase();
    // Custom properties (`--foo`) can feed `var()` into ANY property, so they
    // must conservatively force layout.
    if normalized.is_empty() || normalized.starts_with("--") {
        return DirtyKind::Layout;
    }

    let paint_only = matches!(
        normalized.as_str(),
        // ── Colours (foreground / background / decoration / SVG) ──
        "color"
            | "background-color"
            | "border-color"
            | "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color"
            | "border-inline-color"
            | "border-inline-start-color"
            | "border-inline-end-color"
            | "border-block-color"
            | "border-block-start-color"
            | "border-block-end-color"
            | "column-rule-color"
            | "outline-color"
            | "text-decoration-color"
            | "text-emphasis-color"
            | "caret-color"
            | "accent-color"
            | "fill"
            | "stroke"
            | "flood-color"
            | "lighting-color"
            | "stop-color"
            | "glass-tint"
            | "titlebar-background"
            // ── visibility (does not reflow; intentionally NOT `display`) ──
            | "visibility"
            // ── Background painting (NOT size/position-driven layout) ──
            | "background"
            | "background-attachment"
            | "background-clip"
            | "-webkit-background-clip"
            | "background-origin"
            | "background-position"
            | "background-position-x"
            | "background-position-y"
            | "background-size"
            | "background-repeat"
            | "background-image"
            | "background-blend-mode"
            // ── Shadows / outline / borders that don't take box space ──
            | "box-shadow"
            | "box-shadow-color"
            | "outline"
            | "outline-width"
            | "outline-style"
            | "border-image"
            | "border-image-source"
            | "border-image-slice"
            | "border-image-width"
            | "border-image-outset"
            | "border-image-repeat"
            // ── Text decoration / emphasis / shadow (paint, not metrics) ──
            | "text-decoration"
            | "text-decoration-line"
            | "text-decoration-style"
            | "text-decoration-thickness"
            | "text-decoration-skip-ink"
            | "text-underline-offset"
            | "text-underline-position"
            | "text-emphasis-style"
            | "text-emphasis-position"
            | "text-shadow"
            | "paint-order"
            // ── Compositor-only effects (no reflow) ──
            | "opacity"
            | "transform"
            | "rotate"
            | "scale"
            | "translate"
            | "transform-origin"
            | "transform-style"
            | "transform-box"
            | "perspective"
            | "perspective-origin"
            | "backface-visibility"
            | "filter"
            | "backdrop-filter"
            | "-webkit-backdrop-filter"
            | "blur-radius"
            | "backdrop-blur-radius"
            | "mix-blend-mode"
            | "isolation"
            | "clip"
            | "clip-path"
            | "mask"
            | "mask-image"
            | "mask-mode"
            | "mask-position"
            | "mask-size"
            | "mask-repeat"
            | "mask-origin"
            | "mask-clip"
            | "mask-composite"
            | "mask-type"
            | "clip-rule"
            // ── Image presentation ──
            | "image-rendering"
            | "image-orientation"
            | "object-position"
            | "object-position-x"
            | "object-position-y"
            // ── SVG paint props that don't change layout box ──
            | "fill-opacity"
            | "fill-rule"
            | "stroke-width"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-opacity"
            | "color-interpolation"
            | "color-interpolation-filters"
            | "stop-opacity"
            | "shape-rendering"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            // ── Interaction / hint props (no geometry, no paint reflow) ──
            | "cursor"
            | "pointer-events"
            | "user-select"
    );

    if paint_only {
        DirtyKind::PaintOnly
    } else {
        DirtyKind::Layout
    }
}

/// Per-node dirty flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyFlags {
    bits: u8,
}

impl DirtyFlags {
    const STYLE: u8 = 0x01;
    const LAYOUT: u8 = 0x02;
    const PAINT: u8 = 0x04;
    const SUBTREE_STYLE: u8 = 0x08;
    const SUBTREE_LAYOUT: u8 = 0x10;

    /// No flags set.
    pub fn clean() -> Self {
        Self { bits: 0 }
    }

    /// All dirty.
    pub fn all_dirty() -> Self {
        Self { bits: 0x1F }
    }

    pub fn needs_style(&self) -> bool {
        self.bits & Self::STYLE != 0
    }
    pub fn needs_layout(&self) -> bool {
        self.bits & Self::LAYOUT != 0
    }
    pub fn needs_paint(&self) -> bool {
        self.bits & Self::PAINT != 0
    }
    pub fn subtree_needs_style(&self) -> bool {
        self.bits & Self::SUBTREE_STYLE != 0
    }
    pub fn subtree_needs_layout(&self) -> bool {
        self.bits & Self::SUBTREE_LAYOUT != 0
    }

    pub fn mark_style_dirty(&mut self) {
        self.bits |= Self::STYLE | Self::LAYOUT | Self::PAINT;
    }
    pub fn mark_layout_dirty(&mut self) {
        self.bits |= Self::LAYOUT | Self::PAINT;
    }
    pub fn mark_paint_dirty(&mut self) {
        self.bits |= Self::PAINT;
    }

    /// Mark this node dirty for a single known CSS property, classified by
    /// [`classify_property`]. A paint-only property re-runs the cascade (STYLE,
    /// to recompute the new value) and PAINT but **NOT** LAYOUT, so the cached
    /// layout box is reused. A layout-relevant (or unknown) property escalates
    /// to the full STYLE|LAYOUT|PAINT cascade exactly like [`Self::mark_style_dirty`].
    pub fn mark_style_dirty_for_property(&mut self, property: &str) {
        match classify_property(property) {
            DirtyKind::PaintOnly => self.bits |= Self::STYLE | Self::PAINT,
            DirtyKind::Layout => self.mark_style_dirty(),
        }
    }
    pub fn mark_subtree_style_dirty(&mut self) {
        self.bits |= Self::SUBTREE_STYLE;
    }
    pub fn mark_subtree_layout_dirty(&mut self) {
        self.bits |= Self::SUBTREE_LAYOUT;
    }

    pub fn clear_style(&mut self) {
        self.bits &= !Self::STYLE;
    }
    pub fn clear_layout(&mut self) {
        self.bits &= !Self::LAYOUT;
    }
    pub fn clear_paint(&mut self) {
        self.bits &= !Self::PAINT;
    }
    pub fn clear_subtree_style(&mut self) {
        self.bits &= !Self::SUBTREE_STYLE;
    }
    pub fn clear_subtree_layout(&mut self) {
        self.bits &= !Self::SUBTREE_LAYOUT;
    }
    pub fn clear_all(&mut self) {
        self.bits = 0;
    }

    pub fn is_clean(&self) -> bool {
        self.bits == 0
    }
    pub fn any_dirty(&self) -> bool {
        self.bits != 0
    }
}

/// Document-level set of dirty nodes for batch processing.
#[derive(Debug, Clone, Default)]
pub struct DirtySet {
    /// Nodes needing style recalculation.
    pub style: HashSet<NodeId>,
    /// Nodes needing layout.
    pub layout: HashSet<NodeId>,
    /// Nodes needing repaint.
    pub paint: HashSet<NodeId>,
}

impl DirtySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_style(&mut self, node: NodeId) {
        self.style.insert(node);
        self.layout.insert(node);
        self.paint.insert(node);
    }

    /// Mark a node dirty for a single known CSS property, classified by
    /// [`classify_property`]. A provably paint-only property records the node in
    /// the STYLE set (so the cascade recomputes the value) and the PAINT set,
    /// but NOT the LAYOUT set — letting the pipeline reuse the cached layout box
    /// and re-run paint only. A layout-relevant or unknown property falls back
    /// to the full [`Self::mark_style`] (style + layout + paint).
    pub fn mark_style_for_property(&mut self, node: NodeId, property: &str) {
        match classify_property(property) {
            DirtyKind::PaintOnly => {
                self.style.insert(node);
                self.paint.insert(node);
            }
            DirtyKind::Layout => self.mark_style(node),
        }
    }

    pub fn mark_layout(&mut self, node: NodeId) {
        self.layout.insert(node);
        self.paint.insert(node);
    }

    pub fn mark_paint(&mut self, node: NodeId) {
        self.paint.insert(node);
    }

    pub fn clear_style(&mut self) {
        self.style.clear();
    }

    pub fn clear_layout(&mut self) {
        self.layout.clear();
    }

    pub fn clear_paint(&mut self) {
        self.paint.clear();
    }

    pub fn clear_all(&mut self) {
        self.style.clear();
        self.layout.clear();
        self.paint.clear();
    }

    /// Drop a node from every dirty set. Called by `Document::destroy_node`
    /// so destroyed nodes do not leak into subsequent style/layout/paint passes.
    pub fn remove(&mut self, node: NodeId) {
        self.style.remove(&node);
        self.layout.remove(&node);
        self.paint.remove(&node);
    }

    /// Mark every node currently tracked (or newly added via `ids`) as needing
    /// style recalculation. Used by `ThemeWatcher` after a cache clear so that
    /// the next frame re-queries styles for every live element.
    pub fn mark_style_for<I: IntoIterator<Item = NodeId>>(&mut self, ids: I) {
        for id in ids {
            self.mark_style(id);
        }
    }

    pub fn has_work(&self) -> bool {
        !self.style.is_empty() || !self.layout.is_empty() || !self.paint.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_cascade() {
        let mut f = DirtyFlags::clean();
        assert!(f.is_clean());
        f.mark_style_dirty();
        assert!(f.needs_style());
        assert!(f.needs_layout());
        assert!(f.needs_paint());
    }

    #[test]
    fn set_tracking() {
        let mut set = DirtySet::new();
        set.mark_style(1);
        assert!(set.has_work());
        set.clear_all();
        assert!(!set.has_work());
    }

    // ── Property → DirtyKind classification (lever t91 paint-only split) ──

    #[test]
    fn paint_only_properties_classify_as_paint() {
        for p in [
            "background-color",
            "color",
            "border-color",
            "box-shadow",
            "opacity",
            "border-top-color",
            "outline-color",
            "background",
            "filter",
            "transform",
            "visibility",
            "text-shadow",
            "background-image",
        ] {
            assert_eq!(
                classify_property(p),
                DirtyKind::PaintOnly,
                "{p} must be paint-only (it cannot change layout geometry)"
            );
        }
    }

    #[test]
    fn geometry_properties_classify_as_layout() {
        // These MUST force layout — a wrong paint-only classification here would
        // leave stale layout boxes.
        for p in [
            "width",
            "height",
            "padding",
            "padding-left",
            "margin",
            "margin-top",
            "font-size",
            "font-weight",
            "line-height",
            "letter-spacing",
            "display",
            "content",
            "border-width",
            "border-left-width",
            "white-space",
            "flex-grow",
            "gap",
            "position",
            "top",
            "left",
            "writing-mode",
            "tab-size",
            "min-width",
            "box-sizing",
        ] {
            assert_eq!(
                classify_property(p),
                DirtyKind::Layout,
                "{p} affects geometry/intrinsic size and MUST classify as Layout"
            );
        }
    }

    #[test]
    fn unknown_and_custom_properties_are_conservative_layout() {
        assert_eq!(classify_property("--brand-accent"), DirtyKind::Layout);
        assert_eq!(classify_property("totally-made-up"), DirtyKind::Layout);
        assert_eq!(classify_property(""), DirtyKind::Layout);
        // Case / whitespace insensitive.
        assert_eq!(classify_property("  Background-Color "), DirtyKind::PaintOnly);
    }

    #[test]
    fn mark_style_for_property_paint_only_skips_layout_set() {
        let mut set = DirtySet::new();
        set.mark_style_for_property(7, "background-color");
        assert!(set.style.contains(&7), "style recompute still needed");
        assert!(set.paint.contains(&7), "paint needed");
        assert!(
            !set.layout.contains(&7),
            "a paint-only property must NOT mark the layout set (else layout re-runs)"
        );
    }

    #[test]
    fn mark_style_for_property_geometry_marks_layout_set() {
        let mut set = DirtySet::new();
        set.mark_style_for_property(7, "width");
        assert!(set.style.contains(&7));
        assert!(set.layout.contains(&7), "geometry property must mark layout");
        assert!(set.paint.contains(&7));
    }

    #[test]
    fn flags_paint_only_property_does_not_set_layout_flag() {
        let mut f = DirtyFlags::clean();
        f.mark_style_dirty_for_property("color");
        assert!(f.needs_style());
        assert!(f.needs_paint());
        assert!(
            !f.needs_layout(),
            "paint-only property must not set the per-node LAYOUT flag"
        );

        let mut g = DirtyFlags::clean();
        g.mark_style_dirty_for_property("width");
        assert!(g.needs_layout(), "geometry property must set the LAYOUT flag");
    }
}
