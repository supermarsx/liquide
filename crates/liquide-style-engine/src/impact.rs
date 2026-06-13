//! CSS property impact metadata for downstream pipeline invalidation.
//!
//! This module is intentionally metadata-only. It classifies property names and
//! summarizes changed property sets without changing cascade, layout, paint, or
//! compositor behavior.
//!
//! # Consumer status (staged, t49-e2-F7)
//!
//! **Not yet driven by the restyle pipeline.** Nothing in the cascade / restyle
//! path produces a [`StyleDiffSummary`] or [`StyleChangeImpact`] from a real
//! computed-style diff today; the only downstream use is the `From` conversions
//! into `liquide_layout::LayoutDirty`, which are themselves staged. These types
//! exist so that when property-level restyle invalidation is wired, producers
//! and consumers already agree on one classification table.
//!
//! The classification in [`classify_style_property`] is deliberately
//! conservative: unknown and custom (`--*`) properties fall back to
//! [`conservative_style_impact`] (every category set) so a future consumer can
//! never *under*-invalidate when it first adopts this metadata. The
//! `classification_table_*` tests below guard that conservatism against silent
//! drift if the match arms are edited.

use liquide_common::PipelineImpact;

/// Classify a CSS property name into conservative pipeline impact categories.
///
/// Unknown properties and custom properties are treated broadly so future
/// consumers cannot accidentally under-invalidate when they first adopt this
/// metadata.
#[must_use]
pub fn classify_style_property(property: &str) -> PipelineImpact {
    let normalized = property.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.starts_with("--") {
        return conservative_style_impact();
    }

    let mut impact = match normalized.as_str() {
        "all" => conservative_style_impact(),

        // Paint-only color and decoration changes.
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
        | "titlebar-background" => style_paint_impact(),

        // Resource-backed paint.
        "background-image" | "border-image-source" | "list-style-image" => {
            style_paint_resource_impact()
        }
        "cursor" => style_resource_impact(),
        "font" | "font-family" => style_text_resource_impact(),

        // Box participation and geometry.
        "display" => {
            style_layout_impact() | PipelineImpact::LAYER_ONLY | PipelineImpact::ACCESSIBILITY_ONLY
        }
        "content" => {
            style_layout_impact()
                | PipelineImpact::INTRINSIC_MEASURE
                | PipelineImpact::ACCESSIBILITY_ONLY
        }
        "visibility" => style_paint_impact() | PipelineImpact::ACCESSIBILITY_ONLY,
        "content-visibility" => style_layout_impact() | PipelineImpact::ACCESSIBILITY_ONLY,
        "width"
        | "height"
        | "min-width"
        | "max-width"
        | "min-height"
        | "max-height"
        | "inline-size"
        | "block-size"
        | "min-inline-size"
        | "min-block-size"
        | "max-inline-size"
        | "max-block-size"
        | "box-sizing"
        | "aspect-ratio"
        | "position"
        | "top"
        | "right"
        | "bottom"
        | "left"
        | "inset"
        | "inset-inline"
        | "inset-block"
        | "inset-inline-start"
        | "inset-inline-end"
        | "inset-block-start"
        | "inset-block-end"
        | "float"
        | "clear"
        | "z-index"
        | "object-fit"
        | "object-position"
        | "object-position-x"
        | "object-position-y"
        | "resize"
        | "contain"
        | "container"
        | "container-type"
        | "container-name"
        | "columns"
        | "column-count"
        | "column-width"
        | "column-gap"
        | "row-gap"
        | "column-span"
        | "column-fill"
        | "table-layout"
        | "border-collapse"
        | "border-spacing"
        | "caption-side"
        | "empty-cells"
        | "vertical-align"
        | "writing-mode"
        | "direction"
        | "unicode-bidi"
        | "overflow"
        | "overflow-x"
        | "overflow-y"
        | "overflow-clip-margin" => style_layout_impact(),

        // Text metrics and intrinsic measure.
        "font-size"
        | "line-height"
        | "font-weight"
        | "font-style"
        | "font-stretch"
        | "font-kerning"
        | "font-size-adjust"
        | "font-optical-sizing"
        | "font-feature-settings"
        | "font-variation-settings"
        | "font-variant"
        | "font-variant-caps"
        | "font-variant-numeric"
        | "font-variant-alternates"
        | "font-variant-east-asian"
        | "font-variant-ligatures"
        | "font-variant-position"
        | "font-variant-emoji"
        | "font-synthesis-weight"
        | "font-synthesis-style"
        | "font-synthesis-small-caps"
        | "font-language-override"
        | "font-palette"
        | "letter-spacing"
        | "word-spacing"
        | "text-transform"
        | "white-space"
        | "white-space-collapse"
        | "word-break"
        | "overflow-wrap"
        | "word-wrap"
        | "hyphens"
        | "line-break"
        | "text-wrap-mode"
        | "text-wrap-style"
        | "text-box-trim"
        | "text-box-edge"
        | "text-size-adjust"
        | "text-spacing-trim"
        | "text-autospace"
        | "hanging-punctuation"
        | "initial-letter"
        | "tab-size" => style_intrinsic_impact(),

        // Compositor and layer-affecting effects.
        "transform" | "rotate" | "scale" | "translate" | "offset-path" | "offset-distance"
        | "offset-rotate" | "offset-anchor" | "offset-position" => style_transform_impact(),
        "opacity" => style_opacity_impact(),
        "clip" | "clip-path" => style_clip_impact(),
        "filter"
        | "backdrop-filter"
        | "-webkit-backdrop-filter"
        | "blur-radius"
        | "backdrop-blur-radius" => style_filter_impact(),
        "transform-origin"
        | "transform-style"
        | "transform-box"
        | "perspective"
        | "perspective-origin"
        | "backface-visibility"
        | "will-change"
        | "isolation"
        | "mix-blend-mode"
        | "background-blend-mode"
        | "view-transition-name"
        | "view-transition-class" => style_layer_impact(),

        // Standard paint and image presentation.
        "background"
        | "background-attachment"
        | "background-clip"
        | "-webkit-background-clip"
        | "background-origin"
        | "background-position"
        | "background-position-x"
        | "background-position-y"
        | "background-size"
        | "background-repeat"
        | "box-shadow"
        | "box-shadow-color"
        | "outline"
        | "outline-width"
        | "outline-style"
        | "border-image"
        | "border-image-slice"
        | "border-image-width"
        | "border-image-outset"
        | "border-image-repeat"
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
        | "image-rendering"
        | "image-orientation"
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
        | "d"
        | "cx"
        | "cy"
        | "r"
        | "rx"
        | "ry"
        | "x"
        | "y" => style_paint_impact(),

        // Masking and clipping resources affect paint and layer visibility.
        "mask" | "mask-image" | "mask-mode" | "mask-position" | "mask-size" | "mask-repeat"
        | "mask-origin" | "mask-clip" | "mask-composite" | "mask-type" | "clip-rule" => {
            style_clip_resource_impact()
        }

        // Layout families with many longhands.
        property_name
            if is_margin_property(property_name) || is_padding_property(property_name) =>
        {
            style_layout_impact()
        }
        property_name if is_border_geometry_property(property_name) => style_layout_impact(),
        property_name if is_flex_property(property_name) || is_grid_property(property_name) => {
            style_layout_impact()
        }
        property_name if is_scroll_geometry_property(property_name) => style_layout_impact(),
        property_name if is_shape_property(property_name) => style_layout_impact(),

        // Interaction-only and animation metadata remain local style metadata for now.
        "pointer-events"
        | "user-select"
        | "appearance"
        | "scroll-behavior"
        | "overscroll-behavior"
        | "overscroll-behavior-x"
        | "overscroll-behavior-y"
        | "touch-action"
        | "color-scheme"
        | "forced-color-adjust"
        | "print-color-adjust"
        | "-webkit-print-color-adjust"
        | "transition"
        | "transition-property"
        | "transition-duration"
        | "transition-timing-function"
        | "transition-delay"
        | "transition-behavior"
        | "animation"
        | "animation-name"
        | "animation-duration"
        | "animation-timing-function"
        | "animation-delay"
        | "animation-iteration-count"
        | "animation-direction"
        | "animation-fill-mode"
        | "animation-play-state"
        | "animation-composition"
        | "animation-timeline"
        | "scroll-timeline-name"
        | "scroll-timeline-axis"
        | "view-timeline-name"
        | "view-timeline-axis"
        | "view-timeline-inset"
        | "timeline-scope"
        | "anchor-name"
        | "position-anchor"
        | "position-area"
        | "page"
        | "zoom"
        | "overlay"
        | "math-depth"
        | "math-style"
        | "reading-flow"
        | "field-sizing"
        | "counter-increment"
        | "counter-reset"
        | "counter-set"
        | "quotes" => PipelineImpact::STYLE_ONLY,

        _ => conservative_style_impact(),
    };

    if inherits_by_default(&normalized) {
        impact |= PipelineImpact::INHERITED_STYLE;
    }

    impact
}

/// Broad style-originated impact used for unknown or custom properties.
#[must_use]
pub fn conservative_style_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY
        | PipelineImpact::INHERITED_STYLE
        | PipelineImpact::LAYOUT_GEOMETRY
        | PipelineImpact::INTRINSIC_MEASURE
        | PipelineImpact::PAINT_ONLY
        | PipelineImpact::TRANSFORM_ONLY
        | PipelineImpact::OPACITY_ONLY
        | PipelineImpact::CLIP_ONLY
        | PipelineImpact::RESOURCE_ONLY
        | PipelineImpact::LAYER_ONLY
        | PipelineImpact::ACCESSIBILITY_ONLY
}

/// Combined impact metadata for one or more style property changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StyleChangeImpact {
    impact: PipelineImpact,
}

impl StyleChangeImpact {
    /// Create an empty style impact.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            impact: PipelineImpact::empty(),
        }
    }

    /// Create a broad conservative style impact.
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            impact: conservative_style_impact(),
        }
    }

    /// Create an impact wrapper from raw pipeline impact bits.
    #[must_use]
    pub fn from_pipeline_impact(impact: PipelineImpact) -> Self {
        Self { impact }
    }

    /// Classify one property name.
    #[must_use]
    pub fn from_property(property: &str) -> Self {
        Self {
            impact: classify_style_property(property),
        }
    }

    /// Classify and combine a set of property names.
    #[must_use]
    pub fn from_properties<PropertyIter, PropertyName>(properties: PropertyIter) -> Self
    where
        PropertyIter: IntoIterator<Item = PropertyName>,
        PropertyName: AsRef<str>,
    {
        let mut combined = Self::empty();
        for property in properties {
            combined.add_property(property.as_ref());
        }
        combined
    }

    /// Return the raw shared pipeline impact bits.
    #[must_use]
    pub fn pipeline_impact(self) -> PipelineImpact {
        self.impact
    }

    /// Return true if no pipeline categories are set.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.impact.is_empty()
    }

    /// Return true if all requested categories are present.
    #[must_use]
    pub fn contains(self, impact: PipelineImpact) -> bool {
        self.impact.contains(impact)
    }

    /// Return true if any requested category is present.
    #[must_use]
    pub fn intersects(self, impact: PipelineImpact) -> bool {
        self.impact.intersects(impact)
    }

    /// Add impact for one property name.
    pub fn add_property(&mut self, property: &str) {
        self.impact |= classify_style_property(property);
    }

    /// Add another style impact.
    pub fn add_impact(&mut self, other: Self) {
        self.impact |= other.impact;
    }

    /// Return a combined copy of two impact wrappers.
    #[must_use]
    pub fn combined_with(mut self, other: Self) -> Self {
        self.add_impact(other);
        self
    }

    /// Whether any changed property requires inherited style propagation.
    #[must_use]
    pub fn affects_inherited_style(self) -> bool {
        self.intersects(PipelineImpact::INHERITED_STYLE)
    }

    /// Whether layout geometry or intrinsic measurement may be affected.
    #[must_use]
    pub fn affects_layout(self) -> bool {
        self.intersects(PipelineImpact::LAYOUT_GEOMETRY | PipelineImpact::INTRINSIC_MEASURE)
    }

    /// Whether intrinsic measurement may be affected.
    #[must_use]
    pub fn affects_intrinsic_measure(self) -> bool {
        self.intersects(PipelineImpact::INTRINSIC_MEASURE)
    }

    /// Whether paint output may be affected directly.
    #[must_use]
    pub fn affects_paint(self) -> bool {
        self.intersects(PipelineImpact::PAINT_ONLY)
    }

    /// Whether compositor properties, clips, or layers may be affected.
    #[must_use]
    pub fn affects_compositor(self) -> bool {
        self.intersects(
            PipelineImpact::TRANSFORM_ONLY
                | PipelineImpact::OPACITY_ONLY
                | PipelineImpact::CLIP_ONLY
                | PipelineImpact::LAYER_ONLY,
        )
    }

    /// Whether external resources may be affected.
    #[must_use]
    pub fn affects_resources(self) -> bool {
        self.intersects(PipelineImpact::RESOURCE_ONLY)
    }

    /// Whether accessibility state may be affected.
    #[must_use]
    pub fn affects_accessibility(self) -> bool {
        self.intersects(PipelineImpact::ACCESSIBILITY_ONLY)
    }
}

/// One named CSS property change and its precomputed impact metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StylePropertyChange {
    /// CSS property name as reported by the caller.
    pub property: String,
    /// Conservative pipeline impact for this property.
    pub impact: StyleChangeImpact,
}

impl StylePropertyChange {
    /// Create a property change by classifying the property name.
    #[must_use]
    pub fn new(property: impl Into<String>) -> Self {
        let property = property.into();
        let impact = StyleChangeImpact::from_property(&property);
        Self { property, impact }
    }

    /// Create a property change with caller-provided impact metadata.
    #[must_use]
    pub fn with_impact(property: impl Into<String>, impact: StyleChangeImpact) -> Self {
        Self {
            property: property.into(),
            impact,
        }
    }
}

/// Compact summary of a computed-style diff for later pipeline consumers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleDiffSummary {
    changes: Vec<StylePropertyChange>,
    impact: StyleChangeImpact,
}

impl StyleDiffSummary {
    /// Create an empty diff summary.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a summary from changed property names.
    #[must_use]
    pub fn from_properties<PropertyIter, PropertyName>(properties: PropertyIter) -> Self
    where
        PropertyIter: IntoIterator<Item = PropertyName>,
        PropertyName: AsRef<str>,
    {
        let mut summary = Self::empty();
        for property in properties {
            summary.add_property(property.as_ref());
        }
        summary
    }

    /// Build a summary from already-classified property changes.
    #[must_use]
    pub fn from_changes<ChangeIter>(changes: ChangeIter) -> Self
    where
        ChangeIter: IntoIterator<Item = StylePropertyChange>,
    {
        let mut summary = Self::empty();
        for change in changes {
            summary.add_change(change);
        }
        summary
    }

    /// Add one property name.
    pub fn add_property(&mut self, property: impl Into<String>) {
        self.add_change(StylePropertyChange::new(property));
    }

    /// Add one property change.
    pub fn add_change(&mut self, change: StylePropertyChange) {
        self.impact.add_impact(change.impact);
        self.changes.push(change);
    }

    /// Changed properties in insertion order.
    #[must_use]
    pub fn changes(&self) -> &[StylePropertyChange] {
        &self.changes
    }

    /// Combined style impact for all changes.
    #[must_use]
    pub fn impact(&self) -> StyleChangeImpact {
        self.impact
    }

    /// Number of recorded property changes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Return true when no properties are recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Whether any changed property requires inherited style propagation.
    #[must_use]
    pub fn affects_inherited_style(&self) -> bool {
        self.impact.affects_inherited_style()
    }

    /// Whether layout geometry or intrinsic measurement may be affected.
    #[must_use]
    pub fn affects_layout(&self) -> bool {
        self.impact.affects_layout()
    }

    /// Whether paint output may be affected directly.
    #[must_use]
    pub fn affects_paint(&self) -> bool {
        self.impact.affects_paint()
    }

    /// Whether compositor properties, clips, or layers may be affected.
    #[must_use]
    pub fn affects_compositor(&self) -> bool {
        self.impact.affects_compositor()
    }

    /// Whether external resources may be affected.
    #[must_use]
    pub fn affects_resources(&self) -> bool {
        self.impact.affects_resources()
    }

    /// Whether accessibility state may be affected.
    #[must_use]
    pub fn affects_accessibility(&self) -> bool {
        self.impact.affects_accessibility()
    }
}

fn inherits_by_default(property: &str) -> bool {
    crate::inheritance::is_inherited(property) || matches!(property, "font" | "list-style")
}

fn style_layout_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::LAYOUT_GEOMETRY | PipelineImpact::PAINT_ONLY
}

fn style_intrinsic_impact() -> PipelineImpact {
    style_layout_impact() | PipelineImpact::INTRINSIC_MEASURE
}

fn style_paint_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::PAINT_ONLY
}

fn style_paint_resource_impact() -> PipelineImpact {
    style_paint_impact() | PipelineImpact::RESOURCE_ONLY
}

fn style_resource_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::RESOURCE_ONLY
}

fn style_text_resource_impact() -> PipelineImpact {
    style_intrinsic_impact() | PipelineImpact::RESOURCE_ONLY
}

fn style_transform_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::TRANSFORM_ONLY | PipelineImpact::LAYER_ONLY
}

fn style_opacity_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::OPACITY_ONLY | PipelineImpact::LAYER_ONLY
}

fn style_clip_impact() -> PipelineImpact {
    style_paint_impact() | PipelineImpact::CLIP_ONLY | PipelineImpact::LAYER_ONLY
}

fn style_filter_impact() -> PipelineImpact {
    style_paint_impact() | PipelineImpact::LAYER_ONLY
}

fn style_layer_impact() -> PipelineImpact {
    PipelineImpact::STYLE_ONLY | PipelineImpact::LAYER_ONLY
}

fn style_clip_resource_impact() -> PipelineImpact {
    style_clip_impact() | PipelineImpact::RESOURCE_ONLY
}

fn is_margin_property(property: &str) -> bool {
    property == "margin" || property.starts_with("margin-")
}

fn is_padding_property(property: &str) -> bool {
    property == "padding" || property.starts_with("padding-")
}

fn is_border_geometry_property(property: &str) -> bool {
    (property.starts_with("border")
        && !property.ends_with("color")
        && property != "border-image"
        && !property.starts_with("border-image"))
        || matches!(
            property,
            "outline-offset"
                | "border-start-start-radius"
                | "border-start-end-radius"
                | "border-end-start-radius"
                | "border-end-end-radius"
        )
}

fn is_flex_property(property: &str) -> bool {
    matches!(
        property,
        "flex"
            | "flex-direction"
            | "flex-wrap"
            | "flex-grow"
            | "flex-shrink"
            | "flex-basis"
            | "justify-content"
            | "align-items"
            | "align-self"
            | "align-content"
            | "justify-items"
            | "justify-self"
            | "place-items"
            | "place-content"
            | "place-self"
            | "order"
            | "gap"
    )
}

fn is_grid_property(property: &str) -> bool {
    property == "grid"
        || property == "subgrid"
        || property.starts_with("grid-")
        || matches!(property, "row-gap" | "column-gap")
}

fn is_scroll_geometry_property(property: &str) -> bool {
    property.starts_with("scroll-padding")
        || property.starts_with("scroll-margin")
        || matches!(
            property,
            "scroll-snap-type" | "scroll-snap-align" | "scroll-snap-stop"
        )
}

fn is_shape_property(property: &str) -> bool {
    matches!(
        property,
        "shape-outside" | "shape-margin" | "shape-image-threshold"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_property_is_conservative() {
        let impact = StyleChangeImpact::from_property(" ");

        assert!(impact.affects_layout());
        assert!(impact.affects_paint());
        assert!(impact.affects_compositor());
        assert!(impact.affects_accessibility());
        assert!(impact.affects_resources());
    }

    #[test]
    fn summary_combines_changes() {
        let summary = StyleDiffSummary::from_properties(["color", "transform", "font-size"]);

        assert_eq!(summary.len(), 3);
        assert!(summary.affects_inherited_style());
        assert!(summary.affects_layout());
        assert!(summary.affects_paint());
        assert!(summary.affects_compositor());
        assert!(!summary.affects_accessibility());
    }

    /// Drift guard: unknown and custom properties must remain conservatively
    /// classified (every category set) so a future consumer cannot
    /// under-invalidate. If a wildcard arm is ever narrowed, this fails loudly.
    #[test]
    fn classification_table_unknown_and_custom_stay_conservative() {
        for property in ["--liquide-custom", "not-a-real-property", "totally-made-up"] {
            assert_eq!(
                classify_style_property(property),
                conservative_style_impact(),
                "{property} must classify conservatively",
            );
        }
    }

    /// Drift guard: representative properties keep their declared impact class.
    /// These assertions pin the table's intent; editing a match arm that breaks
    /// one of these is a deliberate behavioral change, not a silent one.
    #[test]
    fn classification_table_representative_arms_are_stable() {
        // Paint-only color change: no layout, no compositor layer move.
        let color = StyleChangeImpact::from_property("color");
        assert!(color.affects_paint());
        assert!(!color.affects_layout());
        assert!(color.affects_inherited_style());

        // Geometry: layout but not intrinsic measure.
        let width = StyleChangeImpact::from_property("width");
        assert!(width.affects_layout());
        assert!(!width.affects_intrinsic_measure());

        // Text metric: intrinsic measure implies layout.
        let font_size = StyleChangeImpact::from_property("font-size");
        assert!(font_size.affects_intrinsic_measure());
        assert!(font_size.affects_layout());

        // Transform: compositor only, never layout.
        let transform = StyleChangeImpact::from_property("transform");
        assert!(transform.affects_compositor());
        assert!(!transform.affects_layout());

        // Resource-backed paint.
        let bg_image = StyleChangeImpact::from_property("background-image");
        assert!(bg_image.affects_paint());
        assert!(bg_image.affects_resources());

        // Longhand family fallthrough (margin) is layout-affecting.
        let margin_left = StyleChangeImpact::from_property("margin-left");
        assert!(margin_left.affects_layout());
    }
}
