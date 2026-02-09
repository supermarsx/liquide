//! Compositing effect trait and stub implementations.
//!
//! Complex effects (backdrop blur, box shadow, inner glow) are defined
//! here as trait implementations with stub rendering. The trait interface
//! and cost estimation are the important parts; actual rendering algorithms
//! will be filled in later.

use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;

/// Trait for compositing effects.
///
/// Implementations should respect per-effect budgets from [`EffectParams`].
pub trait Effect {
    /// Render the effect into the frame buffer within the given region.
    fn render(&self, fb: &mut FrameBuffer, region: Rect, params: &EffectParams);

    /// Estimated cost in milliseconds for the given region size.
    fn estimated_cost_ms(&self, region: Rect) -> f64;
}

/// Backdrop blur effect (dual-pass separable Gaussian).
pub struct BackdropBlur;

impl Effect for BackdropBlur {
    fn render(&self, _fb: &mut FrameBuffer, _region: Rect, _params: &EffectParams) {
        // TODO: Implement dual-pass separable Gaussian blur (spec section 5.1)
        // 1. Extract backdrop region
        // 2. Downsample to 1/DS resolution
        // 3. Horizontal Gaussian blur pass (SIMD AVX2)
        // 4. Vertical Gaussian blur pass (8x8 block transpose)
        // 5. Upsample (bilinear) to original resolution
        // 6. Composite blurred backdrop
        tracing::debug!("backdrop blur: stub (not yet implemented)");
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        // Rough estimate: ~4ms for a 1080p region
        let area = (region.width * region.height) as f64;
        (area / (1920.0 * 1080.0)) * 4.0
    }
}

/// Box shadow effect.
pub struct BoxShadow;

impl Effect for BoxShadow {
    fn render(&self, _fb: &mut FrameBuffer, _region: Rect, _params: &EffectParams) {
        // TODO: Implement box shadow (spec section 5.2)
        // 1. Expand bounds by shadow spread
        // 2. Generate shadow shape with corner radius SDF
        // 3. Downsample + Gaussian blur + upsample
        // 4. Multiply by shadow color and alpha
        // 5. Composite behind the surface
        // Shadows should be cached by (geometry hash, blur_radius, spread)
        tracing::debug!("box shadow: stub (not yet implemented)");
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        let area = (region.width * region.height) as f64;
        (area / (1920.0 * 1080.0)) * 1.0
    }
}

/// Inner glow effect.
pub struct InnerGlow;

impl Effect for InnerGlow {
    fn render(&self, _fb: &mut FrameBuffer, _region: Rect, _params: &EffectParams) {
        // TODO: Implement inner glow (spec section 5.3)
        // 1-2px inset stroke with gradient opacity
        // Use screen blend mode
        tracing::debug!("inner glow: stub (not yet implemented)");
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        let perimeter = 2.0 * (region.width + region.height) as f64;
        perimeter * 0.001 // ~0.2ms for typical surface
    }
}
