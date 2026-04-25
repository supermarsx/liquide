//! Layer promotion heuristics — decide which elements get their own
//! compositor layer and when to demote them to save memory.

use crate::layer::PromotionReason;

/// Default number of frames after which an idle layer is eligible for
/// demotion (merged back into its parent). At 60 fps this is ~1 second.
pub const DEFAULT_DEMOTION_THRESHOLD: u64 = 60;

/// Information about a DOM/shell element used to decide whether it should
/// be promoted to its own compositor layer.
#[derive(Debug, Clone, Default)]
pub struct ElementInfo {
    /// The element has a CSS transform (2D or 3D).
    pub has_transform: bool,
    /// The element has non-1.0 opacity.
    pub has_opacity: bool,
    /// The element has CSS filter or backdrop-filter.
    pub has_filter: bool,
    /// The element is `position: fixed`.
    pub is_fixed: bool,
    /// The element is a scrollable container.
    pub is_scrollable: bool,
    /// The element has `will-change` for a compositor property.
    pub has_will_change: bool,
    /// Number of times this element has been repainted in recent frames.
    /// Used to detect frequently-changing content that benefits from its
    /// own layer.
    pub paint_count: u32,
    /// Whether the element currently has an active animation or transition
    /// on a compositor property (transform, opacity).
    pub animation_active: bool,
    /// Total scrollable content height (only relevant when `is_scrollable`).
    pub scroll_content_height: f32,
    /// Visible viewport height of the scroll container.
    pub scroll_viewport_height: f32,
}

/// Heuristic engine that decides layer promotion and demotion.
#[derive(Debug, Clone)]
pub struct LayerPromotionHeuristics {
    /// Frames of inactivity before a layer is eligible for demotion.
    pub demotion_threshold: u64,
    /// Minimum ratio of scroll-content-height / viewport-height before
    /// a scroll container gets promoted (avoids promoting tiny lists).
    pub scroll_promotion_ratio: f32,
    /// Paint count threshold — if an element has been repainted more than
    /// this many times recently, promote it to avoid re-compositing siblings.
    pub paint_count_promotion: u32,
}

impl Default for LayerPromotionHeuristics {
    fn default() -> Self {
        Self {
            demotion_threshold: DEFAULT_DEMOTION_THRESHOLD,
            scroll_promotion_ratio: 2.0,
            paint_count_promotion: 3,
        }
    }
}

impl LayerPromotionHeuristics {
    /// Create heuristics with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decide whether an element should be promoted to its own layer.
    ///
    /// Returns `Some(reason)` if the element should be promoted, or `None`
    /// if it should remain in its parent's layer.
    #[must_use]
    pub fn should_promote(&self, info: &ElementInfo) -> Option<PromotionReason> {
        // will-change is the strongest signal — the developer explicitly
        // asked for promotion.
        if info.has_will_change {
            return Some(PromotionReason::WillChange);
        }

        // Active animation on a compositor property — promote so the
        // compositor can interpolate without re-rasterizing.
        if info.animation_active && info.has_transform {
            return Some(PromotionReason::HasTransform);
        }
        if info.animation_active && info.has_opacity {
            return Some(PromotionReason::HasOpacity);
        }

        // Static (non-animated) transform still warrants its own layer
        // since the compositor can apply the transform to cached pixels.
        if info.has_transform {
            return Some(PromotionReason::HasTransform);
        }

        // Non-1.0 opacity — compositor can multiply cached pixels by
        // opacity without re-rasterizing.
        if info.has_opacity {
            return Some(PromotionReason::HasOpacity);
        }

        // Filters (blur, drop-shadow, etc.) — expensive to repaint;
        // caching the result avoids per-frame filter computation.
        if info.has_filter {
            return Some(PromotionReason::HasFilter);
        }

        // Fixed position — stays in place during scroll; separate layer
        // lets the compositor handle scroll without repainting the fixed
        // element.
        if info.is_fixed {
            return Some(PromotionReason::FixedPosition);
        }

        // Scrollable container with large content: the compositor can
        // translate the cached layer contents without repainting.
        if info.is_scrollable && info.scroll_viewport_height > 0.0 {
            let ratio = info.scroll_content_height / info.scroll_viewport_height;
            if ratio >= self.scroll_promotion_ratio {
                return Some(PromotionReason::ScrollingContent);
            }
        }

        // Frequently-repainted element — promote so repaints don't
        // invalidate sibling layers.
        if info.paint_count >= self.paint_count_promotion {
            return Some(PromotionReason::Explicit);
        }

        None
    }

    /// Check whether a layer that has been idle for `frames_since_dirty`
    /// frames should be demoted (merged back into its parent layer) to
    /// reclaim memory.
    ///
    /// Root layers and layers with active reasons (WillChange, Video,
    /// ScrollingContent, FixedPosition) are never demoted.
    #[must_use]
    pub fn demotion_check(&self, reason: PromotionReason, frames_since_dirty: u64) -> bool {
        // Never demote layers with persistent reasons.
        match reason {
            PromotionReason::Root
            | PromotionReason::WillChange
            | PromotionReason::Video
            | PromotionReason::FixedPosition
            | PromotionReason::ScrollingContent => return false,
            _ => {}
        }

        frames_since_dirty >= self.demotion_threshold
    }

    /// Scan a layer tree and return IDs of layers eligible for demotion.
    #[must_use]
    pub fn find_demotable_layers(
        &self,
        tree: &crate::tree::LayerTree,
    ) -> Vec<crate::layer::LayerId> {
        let mut result = Vec::new();
        for layer in tree.layers.values() {
            if layer.id == tree.root {
                continue;
            }
            if self.demotion_check(layer.promotion_reason, layer.frames_since_dirty) {
                result.push(layer.id);
            }
        }
        result
    }
}
