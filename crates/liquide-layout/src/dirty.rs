//! Layout invalidation metadata for staged dirty-bit adoption.
//!
//! This module defines a fine-grained vocabulary for future layout and cache
//! consumers. It intentionally does not change the active layout engine's broad
//! invalidation behavior.

use bitflags::bitflags;
use liquide_common::PipelineImpact;
use liquide_style_engine::{StyleChangeImpact, StyleDiffSummary};

bitflags! {
    /// Fine-grained categories describing which layout-owned metadata changed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LayoutDirty: u16 {
        /// Child insertion/removal or box-tree participation changed.
        const SUBTREE_STRUCTURE = 1 << 0;
        /// Parent-provided constraints changed, requiring descendants to reflow.
        const PARENT_CONSTRAINTS = 1 << 1;
        /// The node's own border/content box geometry may change.
        const OWN_BOX_GEOMETRY = 1 << 2;
        /// Intrinsic min/max-content or replaced-content size may change.
        const INTRINSIC_SIZE = 1 << 3;
        /// Inline line breaking, text runs, or line boxes may change.
        const LINE_LAYOUT = 1 << 4;
        /// Fragmentation, multicol, table, flex, or grid constraints may change.
        const FRAGMENTATION_CONSTRAINTS = 1 << 5;
        /// Scrollable overflow or scroll-snap bounds may change.
        const SCROLL_BOUNDS = 1 << 6;
        /// Paint overflow bounds derived from layout output may change.
        const PAINT_BOUNDS = 1 << 7;
        /// Accessibility bounds derived from layout output may change.
        const ACCESSIBILITY_BOUNDS = 1 << 8;
    }
}

impl Default for LayoutDirty {
    fn default() -> Self {
        Self::empty()
    }
}

impl LayoutDirty {
    /// Broad invalidation used when a source impact cannot be narrowed safely.
    #[must_use]
    pub fn conservative() -> Self {
        Self::all()
    }

    /// Convert shared pipeline impact metadata into layout dirty categories.
    #[must_use]
    pub fn from_pipeline_impact(impact: PipelineImpact) -> Self {
        let mut dirty = Self::empty();

        if impact.is_empty() {
            return dirty;
        }

        if impact.intersects(PipelineImpact::LAYOUT_GEOMETRY) {
            dirty |= Self::OWN_BOX_GEOMETRY
                | Self::SCROLL_BOUNDS
                | Self::PAINT_BOUNDS
                | Self::ACCESSIBILITY_BOUNDS;
        }

        if impact.intersects(PipelineImpact::INTRINSIC_MEASURE) {
            dirty |= Self::INTRINSIC_SIZE
                | Self::LINE_LAYOUT
                | Self::OWN_BOX_GEOMETRY
                | Self::SCROLL_BOUNDS
                | Self::PAINT_BOUNDS
                | Self::ACCESSIBILITY_BOUNDS;
        }

        if impact.intersects(
            PipelineImpact::PAINT_ONLY | PipelineImpact::CLIP_ONLY | PipelineImpact::LAYER_ONLY,
        ) {
            dirty |= Self::PAINT_BOUNDS;
        }

        if impact.intersects(PipelineImpact::TRANSFORM_ONLY) {
            dirty |= Self::PAINT_BOUNDS | Self::ACCESSIBILITY_BOUNDS;
        }

        if impact.intersects(PipelineImpact::ACCESSIBILITY_ONLY) {
            dirty |= Self::ACCESSIBILITY_BOUNDS;
        }

        if impact.intersects(PipelineImpact::RESOURCE_ONLY | PipelineImpact::OUTPUT_ONLY) {
            dirty |= Self::conservative();
        }

        dirty
    }

    /// Convert style-engine impact metadata into layout dirty categories.
    #[must_use]
    pub fn from_style_change_impact(impact: StyleChangeImpact) -> Self {
        Self::from_pipeline_impact(impact.pipeline_impact())
    }

    /// Convert a style diff summary into layout dirty categories.
    #[must_use]
    pub fn from_style_diff_summary(summary: &StyleDiffSummary) -> Self {
        Self::from_style_change_impact(summary.impact())
    }

    /// Whether layout geometry, line placement, or structural constraints change.
    #[must_use]
    pub fn affects_geometry(self) -> bool {
        self.intersects(
            Self::SUBTREE_STRUCTURE
                | Self::PARENT_CONSTRAINTS
                | Self::OWN_BOX_GEOMETRY
                | Self::INTRINSIC_SIZE
                | Self::LINE_LAYOUT
                | Self::FRAGMENTATION_CONSTRAINTS,
        )
    }

    /// Whether intrinsic sizing caches are invalid.
    #[must_use]
    pub fn affects_intrinsic_size(self) -> bool {
        self.intersects(Self::INTRINSIC_SIZE)
    }

    /// Whether text shaping, wrapping, or line boxes are invalid.
    #[must_use]
    pub fn affects_text_or_lines(self) -> bool {
        self.intersects(Self::INTRINSIC_SIZE | Self::LINE_LAYOUT)
    }

    /// Whether fragmentation, multicol, table, flex, or grid constraints changed.
    #[must_use]
    pub fn affects_fragmentation(self) -> bool {
        self.intersects(Self::FRAGMENTATION_CONSTRAINTS)
    }

    /// Whether scrollable overflow or scroll-snap bounds changed.
    #[must_use]
    pub fn affects_scroll(self) -> bool {
        self.intersects(Self::SCROLL_BOUNDS)
    }

    /// Whether paint overflow bounds derived from layout changed.
    #[must_use]
    pub fn affects_paint_bounds(self) -> bool {
        self.intersects(Self::PAINT_BOUNDS)
    }

    /// Whether accessibility bounds derived from layout changed.
    #[must_use]
    pub fn affects_accessibility_bounds(self) -> bool {
        self.intersects(Self::ACCESSIBILITY_BOUNDS)
    }
}

impl From<PipelineImpact> for LayoutDirty {
    fn from(impact: PipelineImpact) -> Self {
        Self::from_pipeline_impact(impact)
    }
}

impl From<StyleChangeImpact> for LayoutDirty {
    fn from(impact: StyleChangeImpact) -> Self {
        Self::from_style_change_impact(impact)
    }
}

impl From<&StyleDiffSummary> for LayoutDirty {
    fn from(summary: &StyleDiffSummary) -> Self {
        Self::from_style_diff_summary(summary)
    }
}

/// Source category for a layout dirty summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutDirtyCause {
    /// Shared pipeline impact metadata.
    PipelineImpact,
    /// Computed-style diff metadata.
    StyleChange,
    /// DOM or box-tree structure changed.
    DomStructure,
    /// Text or replaced content changed.
    Content,
    /// External resource identity or content changed.
    Resource,
    /// Output scale, color, or constraint state changed.
    Output,
    /// Unknown source; callers should treat dirty bits conservatively.
    Unknown,
}

impl Default for LayoutDirtyCause {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Compact dirty-bit summary that can be attached to future invalidation logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LayoutInvalidationSummary {
    /// Dirty categories selected for this invalidation.
    pub dirty: LayoutDirty,
    /// High-level source of the invalidation.
    pub cause: LayoutDirtyCause,
}

impl LayoutInvalidationSummary {
    /// Create a summary from precomputed dirty bits and a cause.
    #[must_use]
    pub fn new(dirty: LayoutDirty, cause: LayoutDirtyCause) -> Self {
        Self { dirty, cause }
    }

    /// Build a summary from shared pipeline impact metadata.
    #[must_use]
    pub fn from_pipeline_impact(impact: PipelineImpact, cause: LayoutDirtyCause) -> Self {
        Self::new(LayoutDirty::from_pipeline_impact(impact), cause)
    }

    /// Build a summary from style-engine impact metadata.
    #[must_use]
    pub fn from_style_change_impact(impact: StyleChangeImpact) -> Self {
        Self::new(
            LayoutDirty::from_style_change_impact(impact),
            LayoutDirtyCause::StyleChange,
        )
    }

    /// Build a summary from a style diff summary.
    #[must_use]
    pub fn from_style_diff_summary(summary: &StyleDiffSummary) -> Self {
        Self::from_style_change_impact(summary.impact())
    }

    /// Whether this summary carries no dirty categories.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.dirty.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_style_engine::classify_style_property;

    #[test]
    fn dirty_paint_only_maps_to_paint_bounds_without_geometry() {
        let dirty = LayoutDirty::from(PipelineImpact::PAINT_ONLY);

        assert!(dirty.affects_paint_bounds());
        assert!(!dirty.affects_geometry());
        assert!(!dirty.affects_intrinsic_size());
        assert!(!dirty.affects_text_or_lines());
        assert!(!dirty.affects_accessibility_bounds());
    }

    #[test]
    fn dirty_layout_geometry_maps_to_box_and_bound_metadata() {
        let dirty = LayoutDirty::from(PipelineImpact::LAYOUT_GEOMETRY);

        assert!(dirty.contains(LayoutDirty::OWN_BOX_GEOMETRY));
        assert!(dirty.affects_geometry());
        assert!(dirty.affects_scroll());
        assert!(dirty.affects_paint_bounds());
        assert!(dirty.affects_accessibility_bounds());
        assert!(!dirty.affects_intrinsic_size());
    }

    #[test]
    fn dirty_intrinsic_measure_maps_to_measure_lines_and_geometry() {
        let dirty = LayoutDirty::from(PipelineImpact::INTRINSIC_MEASURE);

        assert!(dirty.affects_intrinsic_size());
        assert!(dirty.affects_text_or_lines());
        assert!(dirty.contains(LayoutDirty::OWN_BOX_GEOMETRY));
        assert!(dirty.affects_geometry());
        assert!(dirty.affects_paint_bounds());
        assert!(dirty.affects_accessibility_bounds());
    }

    #[test]
    fn dirty_transform_and_opacity_do_not_dirty_layout_geometry() {
        let dirty =
            LayoutDirty::from(PipelineImpact::TRANSFORM_ONLY | PipelineImpact::OPACITY_ONLY);

        assert!(!dirty.affects_geometry());
        assert!(!dirty.affects_intrinsic_size());
        assert!(!dirty.affects_text_or_lines());
        assert!(!dirty.affects_scroll());
        assert!(dirty.affects_paint_bounds());
        assert!(dirty.affects_accessibility_bounds());
    }

    #[test]
    fn dirty_resource_and_output_impacts_are_conservative() {
        for impact in [PipelineImpact::RESOURCE_ONLY, PipelineImpact::OUTPUT_ONLY] {
            let dirty = LayoutDirty::from(impact);

            assert_eq!(dirty, LayoutDirty::conservative());
            assert!(dirty.affects_geometry());
            assert!(dirty.affects_intrinsic_size());
            assert!(dirty.affects_text_or_lines());
            assert!(dirty.affects_fragmentation());
            assert!(dirty.affects_scroll());
            assert!(dirty.affects_paint_bounds());
            assert!(dirty.affects_accessibility_bounds());
        }
    }

    #[test]
    fn dirty_unknown_style_property_uses_broad_invalidation() {
        let impact = classify_style_property("--liquide-new-property");
        let dirty = LayoutDirty::from(impact);

        assert_eq!(dirty, LayoutDirty::conservative());
    }

    #[test]
    fn dirty_style_diff_summary_conversion_uses_combined_impact() {
        let summary = StyleDiffSummary::from_properties(["color", "font-size"]);
        let dirty = LayoutDirty::from(&summary);

        assert!(dirty.affects_paint_bounds());
        assert!(dirty.affects_intrinsic_size());
        assert!(dirty.affects_text_or_lines());
        assert!(dirty.affects_geometry());
    }
}
