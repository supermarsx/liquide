//! Shared vocabulary for staged rendering-pipeline invalidation and metrics.
//!
//! # Consumer status (staged, t49-e10-F7)
//!
//! Only [`PipelineImpact`] is currently consumed at runtime — the style engine
//! (`liquide-style-engine`) classifies CSS properties into these categories and
//! the layout dirty bridge (`liquide-layout::dirty`) converts them into layout
//! dirty flags. The remaining two surfaces in this module are **staged
//! vocabulary with no runtime consumers yet**:
//!
//! * [`PipelineFeatureFlags`] / [`feature_flags`] — the roadmap's optimization
//!   toggles. They default to all-disabled and nothing reads them to gate
//!   behavior yet; they exist so future wiring uses one canonical flag name set.
//! * [`metric_names`] / [`metric_labels`] — telemetry identifiers that no
//!   metrics emitter publishes yet; they pin stable names ahead of adoption.
//!
//! These halves are intentionally inert. They are kept (rather than deleted) so
//! the eventual producers/consumers share one spelling, but callers must not
//! assume any flag is honored or any metric is emitted until those surfaces are
//! wired. Do not add a flag/metric name here without a matching plan to consume
//! it, to avoid re-growing dead, lying vocabulary.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Canonical categories describing which pipeline work a change can affect.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PipelineImpact: u32 {
        /// Local style recomputation without inherited propagation.
        const STYLE_ONLY = 1 << 0;
        /// Inherited style propagation into descendants.
        const INHERITED_STYLE = 1 << 1;
        /// Layout geometry such as position, size, or box participation.
        const LAYOUT_GEOMETRY = 1 << 2;
        /// Intrinsic measurement such as text or replaced-content sizing.
        const INTRINSIC_MEASURE = 1 << 3;
        /// Paint output without layout geometry changes.
        const PAINT_ONLY = 1 << 4;
        /// Transform-only compositor updates.
        const TRANSFORM_ONLY = 1 << 5;
        /// Opacity-only compositor updates.
        const OPACITY_ONLY = 1 << 6;
        /// Clip or mask state that may affect visible regions.
        const CLIP_ONLY = 1 << 7;
        /// External resource identity or content changes.
        const RESOURCE_ONLY = 1 << 8;
        /// Layer assignment, promotion, or demotion changes.
        const LAYER_ONLY = 1 << 9;
        /// Output-specific state such as scale, color space, or tile grid.
        const OUTPUT_ONLY = 1 << 10;
        /// Accessibility tree, bounds, or visibility updates.
        const ACCESSIBILITY_ONLY = 1 << 11;
    }
}

impl Default for PipelineImpact {
    fn default() -> Self {
        Self::empty()
    }
}

/// Runtime switches for planned pipeline optimizations.
///
/// Defaults intentionally keep every roadmap feature disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineFeatureFlags {
    /// Enable property-level style dirty tracking.
    pub property_dirty_tracking: bool,
    /// Enable layout dirty bits as the layout invalidation source.
    pub layout_dirty_bits: bool,
    /// Enable the second-generation text measurement cache.
    pub text_measure_cache_v2: bool,
    /// Enable stable display-list diffing.
    pub display_list_diffing: bool,
    /// Enable display command merging.
    pub display_command_merging: bool,
    /// Enable per-layer damage tracking.
    pub layer_damage: bool,
    /// Enable compositor occlusion culling.
    pub occlusion_culling: bool,
    /// Enable output-aware damage partitioning.
    pub output_aware_damage: bool,
    /// Enable bandwidth-driven output quality adaptation.
    pub adaptive_bandwidth_quality: bool,
    /// Enable vblank-aware presentation pacing.
    pub vblank_pacing: bool,
    /// Build accessibility deltas from layout and paint output.
    pub a11y_from_layout_paint: bool,
}

impl PipelineFeatureFlags {
    /// Construct a flag set with all roadmap behavior disabled.
    #[must_use]
    pub const fn all_disabled() -> Self {
        Self {
            property_dirty_tracking: false,
            layout_dirty_bits: false,
            text_measure_cache_v2: false,
            display_list_diffing: false,
            display_command_merging: false,
            layer_damage: false,
            occlusion_culling: false,
            output_aware_damage: false,
            adaptive_bandwidth_quality: false,
            vblank_pacing: false,
            a11y_from_layout_paint: false,
        }
    }

    /// Return the value for a stable feature flag name, or `None` if unknown.
    #[must_use]
    pub fn is_enabled(&self, name: &str) -> Option<bool> {
        match name {
            feature_flags::PROPERTY_DIRTY_TRACKING => Some(self.property_dirty_tracking),
            feature_flags::LAYOUT_DIRTY_BITS => Some(self.layout_dirty_bits),
            feature_flags::TEXT_MEASURE_CACHE_V2 => Some(self.text_measure_cache_v2),
            feature_flags::DISPLAY_LIST_DIFFING => Some(self.display_list_diffing),
            feature_flags::DISPLAY_COMMAND_MERGING => Some(self.display_command_merging),
            feature_flags::LAYER_DAMAGE => Some(self.layer_damage),
            feature_flags::OCCLUSION_CULLING => Some(self.occlusion_culling),
            feature_flags::OUTPUT_AWARE_DAMAGE => Some(self.output_aware_damage),
            feature_flags::ADAPTIVE_BANDWIDTH_QUALITY => Some(self.adaptive_bandwidth_quality),
            feature_flags::VBLANK_PACING => Some(self.vblank_pacing),
            feature_flags::A11Y_FROM_LAYOUT_PAINT => Some(self.a11y_from_layout_paint),
            _ => None,
        }
    }
}

impl Default for PipelineFeatureFlags {
    fn default() -> Self {
        Self::all_disabled()
    }
}

/// Stable names for roadmap feature flags.
///
/// STAGED: these names are not yet consumed to gate any runtime behavior
/// (see the module-level "Consumer status" note). They pin a canonical
/// spelling for future wiring only.
pub mod feature_flags {
    /// Property-level style dirty tracking.
    pub const PROPERTY_DIRTY_TRACKING: &str = "pipeline.property_dirty_tracking";
    /// Layout dirty bits.
    pub const LAYOUT_DIRTY_BITS: &str = "pipeline.layout_dirty_bits";
    /// Text measurement cache v2.
    pub const TEXT_MEASURE_CACHE_V2: &str = "pipeline.text_measure_cache_v2";
    /// Display-list diffing.
    pub const DISPLAY_LIST_DIFFING: &str = "pipeline.display_list_diffing";
    /// Display command merging.
    pub const DISPLAY_COMMAND_MERGING: &str = "pipeline.display_command_merging";
    /// Layer damage tracking.
    pub const LAYER_DAMAGE: &str = "pipeline.layer_damage";
    /// Occlusion culling.
    pub const OCCLUSION_CULLING: &str = "pipeline.occlusion_culling";
    /// Output-aware damage tracking.
    pub const OUTPUT_AWARE_DAMAGE: &str = "pipeline.output_aware_damage";
    /// Adaptive bandwidth quality.
    pub const ADAPTIVE_BANDWIDTH_QUALITY: &str = "pipeline.adaptive_bandwidth_quality";
    /// Vblank pacing.
    pub const VBLANK_PACING: &str = "pipeline.vblank_pacing";
    /// Accessibility derived from layout and paint output.
    pub const A11Y_FROM_LAYOUT_PAINT: &str = "pipeline.a11y_from_layout_paint";

    /// All public feature flag names.
    pub const ALL: &[&str] = &[
        PROPERTY_DIRTY_TRACKING,
        LAYOUT_DIRTY_BITS,
        TEXT_MEASURE_CACHE_V2,
        DISPLAY_LIST_DIFFING,
        DISPLAY_COMMAND_MERGING,
        LAYER_DAMAGE,
        OCCLUSION_CULLING,
        OUTPUT_AWARE_DAMAGE,
        ADAPTIVE_BANDWIDTH_QUALITY,
        VBLANK_PACING,
        A11Y_FROM_LAYOUT_PAINT,
    ];
}

/// Stable telemetry metric names for baseline pipeline counters and timings.
///
/// STAGED: no metrics emitter publishes these yet (see the module-level
/// "Consumer status" note). They reserve stable identifiers for future
/// telemetry wiring.
pub mod metric_names {
    /// Count of DOM/style nodes that cascade recomputed.
    pub const CASCADE_NODES_RECOMPUTED: &str = "liquide_pipeline_cascade_nodes_recomputed_total";
    /// Count of style cache hits.
    pub const STYLE_CACHE_HITS: &str = "liquide_pipeline_style_cache_hits_total";
    /// Count of style cache misses.
    pub const STYLE_CACHE_MISSES: &str = "liquide_pipeline_style_cache_misses_total";
    /// Count of layout nodes recomputed.
    pub const LAYOUT_NODES_RECOMPUTED: &str = "liquide_pipeline_layout_nodes_recomputed_total";
    /// Count of text measurement requests.
    pub const TEXT_MEASURE_REQUESTS: &str = "liquide_pipeline_text_measure_requests_total";
    /// Count of text measurement cache hits.
    pub const TEXT_MEASURE_HITS: &str = "liquide_pipeline_text_measure_hits_total";
    /// Count of display items generated.
    pub const DISPLAY_ITEMS_GENERATED: &str = "liquide_pipeline_display_items_generated_total";
    /// Count of display-list diff hits.
    pub const DISPLAY_LIST_DIFF_HITS: &str = "liquide_pipeline_display_list_diff_hits_total";
    /// Count of display-list diff misses.
    pub const DISPLAY_LIST_DIFF_MISSES: &str = "liquide_pipeline_display_list_diff_misses_total";
    /// Count of layer promotions.
    pub const LAYER_PROMOTIONS: &str = "liquide_pipeline_layer_promotions_total";
    /// Count of layer demotions.
    pub const LAYER_DEMOTIONS: &str = "liquide_pipeline_layer_demotions_total";
    /// Count of damaged logical regions.
    pub const DAMAGED_REGIONS: &str = "liquide_pipeline_damaged_regions_total";
    /// Count of damaged output tiles.
    pub const DAMAGED_TILES: &str = "liquide_pipeline_damaged_tiles_total";
    /// Count of nodes skipped by occlusion.
    pub const OCCLUDED_NODES_SKIPPED: &str = "liquide_pipeline_occluded_nodes_skipped_total";
    /// Count of rasterized tiles.
    pub const RASTERIZED_TILES: &str = "liquide_pipeline_rasterized_tiles_total";
    /// Count of encoded tiles, labeled by strategy.
    pub const ENCODED_TILES: &str = "liquide_pipeline_encoded_tiles_total";
    /// Current transport queue depth.
    pub const TRANSPORT_QUEUE_DEPTH: &str = "liquide_pipeline_transport_queue_depth";
    /// Presentation latency histogram in seconds.
    pub const PRESENT_LATENCY: &str = "liquide_pipeline_present_latency_seconds";
    /// Count of missed vblanks.
    pub const MISSED_VBLANKS: &str = "liquide_pipeline_missed_vblanks_total";

    /// All public metric names.
    pub const ALL: &[&str] = &[
        CASCADE_NODES_RECOMPUTED,
        STYLE_CACHE_HITS,
        STYLE_CACHE_MISSES,
        LAYOUT_NODES_RECOMPUTED,
        TEXT_MEASURE_REQUESTS,
        TEXT_MEASURE_HITS,
        DISPLAY_ITEMS_GENERATED,
        DISPLAY_LIST_DIFF_HITS,
        DISPLAY_LIST_DIFF_MISSES,
        LAYER_PROMOTIONS,
        LAYER_DEMOTIONS,
        DAMAGED_REGIONS,
        DAMAGED_TILES,
        OCCLUDED_NODES_SKIPPED,
        RASTERIZED_TILES,
        ENCODED_TILES,
        TRANSPORT_QUEUE_DEPTH,
        PRESENT_LATENCY,
        MISSED_VBLANKS,
    ];
}

/// Stable label names shared by baseline pipeline telemetry.
pub mod metric_labels {
    /// Pipeline impact category label.
    pub const IMPACT: &str = "impact";
    /// Feature flag label.
    pub const FEATURE: &str = "feature";
    /// Cache name label.
    pub const CACHE: &str = "cache";
    /// Encoder or transport strategy label.
    pub const STRATEGY: &str = "strategy";
    /// Display output label.
    pub const OUTPUT: &str = "output";
    /// Damage or tile class label.
    pub const DAMAGE_CLASS: &str = "damage_class";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_impact_supports_bit_operations() {
        let impact = PipelineImpact::STYLE_ONLY | PipelineImpact::PAINT_ONLY;

        assert!(impact.contains(PipelineImpact::STYLE_ONLY));
        assert!(impact.intersects(PipelineImpact::PAINT_ONLY));
        assert!(!impact.contains(PipelineImpact::LAYOUT_GEOMETRY));
        assert_eq!(PipelineImpact::default(), PipelineImpact::empty());
        assert_eq!(PipelineImpact::all().bits().count_ones(), 12);
    }

    #[test]
    fn pipeline_feature_flags_default_disabled() {
        let flags = PipelineFeatureFlags::default();

        for name in feature_flags::ALL {
            assert_eq!(
                flags.is_enabled(name),
                Some(false),
                "{name} should default off"
            );
        }
        assert_eq!(flags.is_enabled("pipeline.unknown"), None);
    }

    #[test]
    fn feature_flag_names_are_stable_and_unique() {
        assert_eq!(
            feature_flags::PROPERTY_DIRTY_TRACKING,
            "pipeline.property_dirty_tracking"
        );
        assert_eq!(feature_flags::VBLANK_PACING, "pipeline.vblank_pacing");
        assert_unique(feature_flags::ALL);
    }

    #[test]
    fn metric_names_and_labels_are_stable() {
        assert_eq!(
            metric_names::CASCADE_NODES_RECOMPUTED,
            "liquide_pipeline_cascade_nodes_recomputed_total"
        );
        assert_eq!(
            metric_names::ENCODED_TILES,
            "liquide_pipeline_encoded_tiles_total"
        );
        assert_eq!(
            metric_names::PRESENT_LATENCY,
            "liquide_pipeline_present_latency_seconds"
        );
        assert_eq!(metric_labels::STRATEGY, "strategy");
        assert_eq!(metric_labels::OUTPUT, "output");
        assert_unique(metric_names::ALL);
    }

    fn assert_unique(values: &[&str]) {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), values.len());
    }
}
