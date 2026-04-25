#![doc = "Scene graph, damage tracking, frame buffer management, and compositor"]
#![doc = "contract for the Liquide desktop compositor."]
#![doc = ""]
#![doc = "This crate defines the core data structures that represent the visual"]
#![doc = "state of the desktop: scene graph nodes, damage regions, frame buffers,"]
#![doc = "and the effect budget / degradation system.  The crate exposes no"]
#![doc = "rendering logic itself — rendering is delegated to `liquide-renderer-cpu`"]
#![doc = "or `liquide-renderer-gpu`."]

pub mod compositor;
pub mod cursor;
pub mod damage;
pub mod effects;
pub mod framebuffer;
pub mod geometry;
pub mod pixel;
pub mod property_tree;
pub mod scene;

pub use compositor::{Compositor, CompositorContract, FrameLifecycle};
pub use cursor::{CursorBitmap, CursorUpdate};
pub use damage::{DamageClass, DamageSet, DamageTile, DamageTracker};
pub use effects::{
    DegradationController, DegradationLevel, EffectBudget, EffectParams, QualityProfile,
};
pub use framebuffer::{DoubleBuffer, FrameBuffer, FrameMemory, FrameMemoryPool};
pub use geometry::{Affine2D, Point, Rect, Size};
pub use pixel::{BlendMode, Color, PixelFormat};
pub use scene::{
    BackdropFilterSpec, BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
    BorderImageRepeat, BorderImageSpec, BorderSide, BorderSideStyle, BorderSides, BoxShadowSpec,
    ClipPathKind, DecorationButtons, DecorationColors, DecorationLayout, FilterSpec, FlatNode,
    GradientSpec, ImageFit, MaskMode, MaskSpec, NodeId, NodeProperties, OutlineSpec, OutlineStyle,
    Overflow, SceneNode, SceneNodeKind, TextDecoration, TextDecorationLine, TextDecorationStyle,
    TextShadow,
};

use thiserror::Error;

// ---------------------------------------------------------------------------
// Renderer trait — implemented by liquide-renderer-cpu (and future GPU backend)
// ---------------------------------------------------------------------------

/// Error returned by renderer implementations.
pub type RenderError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for renderer operations.
pub type RenderResult<T> = std::result::Result<T, RenderError>;

/// Quality / performance trade-off hint for renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQuality {
    /// Prefer visual quality over performance.
    Quality,
    /// Balanced quality and performance (default).
    Balanced,
    /// Prefer performance over visual quality.
    Performance,
}

/// The renderer trait: processes a flattened scene into a frame buffer.
///
/// Implementors convert a list of [`FlatNode`]s into pixel data inside a
/// [`FrameBuffer`].  The trait includes optional capability methods with
/// default no-op implementations so that render threads can drive any
/// backend without down-casting.
pub trait Renderer: Send {
    /// Render the visible scene nodes into the frame buffer.
    ///
    /// Only tiles listed in `damage` need re-rendering.  Returns per-tile
    /// damage classifications for the encoder.
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> RenderResult<Vec<DamageTile>>;

    // -- optional capability methods (no-op defaults) -----------------------

    /// Whether real blur is enabled (Glass nodes, etc.).
    fn blur_enabled(&self) -> bool {
        false
    }

    /// Enable or disable blur.
    fn set_blur_enabled(&mut self, _enabled: bool) {}

    /// Whether the last render had text glyphs still being rasterised.
    fn has_pending_glyphs(&self) -> bool {
        false
    }

    /// Report the last render time (ms) for adaptive quality decisions.
    fn report_render_time(&mut self, _ms: f64) {}

    /// Set a window to render in skeleton mode (outline-only during drag).
    fn set_skeleton_window(&mut self, _window_id: Option<u64>) {}

    /// Get the current quality / performance mode.
    fn get_quality_mode(&self) -> RenderQuality {
        RenderQuality::Balanced
    }

    /// Set the quality / performance mode.
    fn set_quality_mode(&mut self, _mode: RenderQuality) {}
}

// ---------------------------------------------------------------------------
// Compositor errors
// ---------------------------------------------------------------------------

/// Errors produced by the compositor crate.
#[derive(Debug, Error)]
pub enum CompositorError {
    /// An invalid scene graph was submitted.
    #[error("invalid scene graph: {0}")]
    InvalidScene(String),

    /// A node ID was not found in the scene graph.
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    /// Frame buffer allocation failed.
    #[error("frame buffer allocation failed: {width}x{height}")]
    AllocationFailed { width: u32, height: u32 },

    /// An effect exceeded its budget.
    #[error("effect budget exceeded: {effect} took {actual_ms:.2}ms (budget: {budget_ms:.2}ms)")]
    BudgetExceeded {
        effect: String,
        actual_ms: f64,
        budget_ms: f64,
    },

    /// Generic internal error.
    #[error("internal compositor error: {0}")]
    Internal(String),
}

/// Convenience result type for compositor operations.
pub type Result<T> = std::result::Result<T, CompositorError>;

#[cfg(test)]
mod tests;
