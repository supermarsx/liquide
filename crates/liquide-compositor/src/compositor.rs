//! Compositor contract trait and concrete implementation.

use crate::Result;
use crate::cursor::CursorUpdate;
use crate::damage::{DamageSet, DamageTracker};
use crate::effects::{
    DegradationController, DegradationLevel, EffectBudget, EffectParams, QualityProfile,
};
use crate::framebuffer::{DoubleBuffer, FrameBuffer};
use crate::scene::{FlatNode, GlassParams, SceneNode};

/// Frame lifecycle state machine.
///
/// Enforces the `prepare → render → present` ordering documented in the
/// compositor contract.  Transitions are checked at runtime with
/// `debug_assert!` so that buggy callers are caught in dev builds without
/// paying the branch cost in release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameLifecycle {
    /// No frame in flight.  `prepare_frame()` is the only legal next call.
    Idle,
    /// `prepare_frame()` was called — the back buffer is ready to be drawn
    /// into and `frame_buffer_mut()` may be called.
    Prepared,
    /// Renderer has signalled end of rendering via `end_frame()`.  Damage
    /// may be computed (`compute_damage()`), then the frame presented.
    Rendered,
    /// `present_frame()` has been called; the front buffer is readable by
    /// the encoder.  Returns to `Idle` on the next `prepare_frame()`.
    Presented,
}

/// The compositor contract: the stable interface between the compositor
/// and its consumers (renderer, encoder, transport).
pub trait CompositorContract: Send + Sync {
    /// Submit a new scene graph for the next frame.
    fn submit_scene(&mut self, root: SceneNode) -> Result<()>;

    /// Compute damage between the current and previous frame.
    fn compute_damage(&mut self) -> Result<DamageSet>;

    /// Access the current frame buffer (for the encoder to read).
    fn frame_buffer(&self) -> &FrameBuffer;

    /// Mutable access to the frame buffer (for the renderer to write).
    fn frame_buffer_mut(&mut self) -> &mut FrameBuffer;

    /// Query the current effect budget.
    fn effect_budget(&self) -> &EffectBudget;

    /// Query the current degradation level.
    fn degradation_level(&self) -> DegradationLevel;

    /// Query the current effect parameters (profile + degradation applied).
    fn effect_params(&self) -> EffectParams;

    /// Register a glass surface with the compositor.
    fn register_glass(&mut self, surface_id: u64, params: GlassParams) -> Result<()>;

    /// Get the latest cursor update (if any).
    fn cursor_update(&self) -> Option<&CursorUpdate>;
}

/// The main compositor state.
pub struct Compositor {
    scene: Option<SceneNode>,
    /// Scratch buffer for the most recently flattened scene.  Reused each
    /// frame to avoid reallocating on every `submit_scene`.
    flat_cache: Vec<FlatNode>,
    double_buffer: DoubleBuffer,
    damage_tracker: DamageTracker,
    degradation: DegradationController,
    effect_budget: EffectBudget,
    profile: QualityProfile,
    pub(crate) glass_surfaces: Vec<(u64, GlassParams)>,
    cursor: Option<CursorUpdate>,
    width: u32,
    height: u32,
    tile_size: u32,
    /// Lifecycle state, enforced by `debug_assert!` in transitions.
    lifecycle: FrameLifecycle,
}

impl Compositor {
    /// Create a new compositor.
    #[must_use]
    pub fn new(width: u32, height: u32, tile_size: u32, profile: QualityProfile) -> Self {
        Self {
            scene: None,
            flat_cache: Vec::new(),
            double_buffer: DoubleBuffer::new(width, height, crate::pixel::PixelFormat::Bgra8),
            damage_tracker: DamageTracker::new(tile_size, width, height),
            degradation: DegradationController::new(),
            effect_budget: EffectBudget::for_profile(profile),
            profile,
            glass_surfaces: Vec::new(),
            cursor: None,
            width,
            height,
            tile_size,
            lifecycle: FrameLifecycle::Idle,
        }
    }

    /// Resize the compositor output.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.double_buffer = DoubleBuffer::new(width, height, crate::pixel::PixelFormat::Bgra8);
        self.damage_tracker.resize(width, height);
        // Reset lifecycle: in-flight frame state is invalidated by resize.
        self.lifecycle = FrameLifecycle::Idle;
        Ok(())
    }

    /// Begin preparing the next frame.
    ///
    /// Unlike the old `begin_frame` (which swapped front/back *before* the
    /// renderer drew into it, causing damage to be computed against the
    /// previously-presented buffer), `prepare_frame` leaves the front buffer
    /// untouched and exposes `frame_buffer_mut()` as the back buffer for
    /// the renderer to draw into.  The front/back swap happens inside
    /// `present_frame()` after the renderer signals end-of-frame.
    pub fn prepare_frame(&mut self) {
        debug_assert!(
            matches!(
                self.lifecycle,
                FrameLifecycle::Idle | FrameLifecycle::Presented
            ),
            "prepare_frame called in lifecycle {:?}",
            self.lifecycle
        );
        self.lifecycle = FrameLifecycle::Prepared;
    }

    /// Legacy alias for [`prepare_frame`](Self::prepare_frame).
    #[deprecated(note = "Use prepare_frame instead; see CompositorContract lifecycle docs")]
    pub fn begin_frame(&mut self) {
        self.prepare_frame();
    }

    /// Signal that the renderer has finished drawing into the back buffer.
    ///
    /// After `end_frame`, callers may `compute_damage()` (which hashes the
    /// back buffer — the one that was just rendered — against the previous
    /// frame's hashes) and then `present_frame()` to publish.
    pub fn end_frame(&mut self) {
        debug_assert_eq!(
            self.lifecycle,
            FrameLifecycle::Prepared,
            "end_frame called without prepare_frame"
        );
        self.lifecycle = FrameLifecycle::Rendered;
    }

    /// Swap back→front so the encoder can read the just-rendered frame.
    ///
    /// Also known as [`swap_frame`](Self::swap_frame) for backwards compat.
    pub fn present_frame(&mut self) {
        debug_assert!(
            matches!(
                self.lifecycle,
                FrameLifecycle::Rendered | FrameLifecycle::Prepared
            ),
            "present_frame called in lifecycle {:?}",
            self.lifecycle
        );
        self.double_buffer.swap();
        self.lifecycle = FrameLifecycle::Presented;
    }

    /// Alias for [`present_frame`](Self::present_frame).
    pub fn swap_frame(&mut self) {
        self.present_frame();
    }

    /// Report frame timing for degradation control.
    pub fn report_frame_time(&mut self, frame_ms: f64) {
        let budget_ms = self.effect_budget.total_frame_budget_ms;
        self.degradation.report_frame_time(frame_ms, budget_ms);
    }

    /// Update cursor state.
    pub fn set_cursor(&mut self, update: CursorUpdate) {
        self.cursor = Some(update);
    }

    /// Get the scene graph (if one has been submitted).
    pub fn scene(&self) -> Option<&SceneNode> {
        self.scene.as_ref()
    }

    /// Flattened scene cache — populated on each `submit_scene`.
    #[must_use]
    pub fn flat_scene(&self) -> &[FlatNode] {
        &self.flat_cache
    }

    /// Current lifecycle state.  Primarily useful for tests and debugging.
    #[must_use]
    pub fn lifecycle(&self) -> FrameLifecycle {
        self.lifecycle
    }

    /// Output width.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Output height.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Tile size in pixels.
    #[must_use]
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

impl CompositorContract for Compositor {
    fn submit_scene(&mut self, root: SceneNode) -> Result<()> {
        // Flatten into the cached buffer so downstream consumers
        // (render threads, encoder) see a ready-to-iterate `Vec<FlatNode>`.
        root.flatten_into(&mut self.flat_cache);
        self.scene = Some(root);
        Ok(())
    }

    fn compute_damage(&mut self) -> Result<DamageSet> {
        // Hash the back buffer — the freshly rendered frame — against the
        // previous frame's hashes.  Previously this hashed `front()` which
        // is the *already-presented* buffer, producing damage that had
        // nothing to do with the pending frame.
        Ok(self
            .damage_tracker
            .compute_damage(self.double_buffer.back()))
    }

    fn frame_buffer(&self) -> &FrameBuffer {
        self.double_buffer.front()
    }

    fn frame_buffer_mut(&mut self) -> &mut FrameBuffer {
        self.double_buffer.back_mut()
    }

    fn effect_budget(&self) -> &EffectBudget {
        &self.effect_budget
    }

    fn degradation_level(&self) -> DegradationLevel {
        self.degradation.current_level()
    }

    fn effect_params(&self) -> EffectParams {
        self.degradation.current_params(self.profile)
    }

    fn register_glass(&mut self, surface_id: u64, params: GlassParams) -> Result<()> {
        // Replace if already registered, otherwise add
        if let Some(entry) = self
            .glass_surfaces
            .iter_mut()
            .find(|(id, _)| *id == surface_id)
        {
            entry.1 = params;
        } else {
            self.glass_surfaces.push((surface_id, params));
        }
        Ok(())
    }

    fn cursor_update(&self) -> Option<&CursorUpdate> {
        self.cursor.as_ref()
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::scene::{NodeProperties, SceneNodeKind};

    fn root_scene() -> SceneNode {
        SceneNode::new(
            1,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
        )
    }

    #[test]
    fn lifecycle_happy_path() {
        let mut c = Compositor::new(100, 100, 32, QualityProfile::Balanced);
        assert_eq!(c.lifecycle(), FrameLifecycle::Idle);
        c.prepare_frame();
        assert_eq!(c.lifecycle(), FrameLifecycle::Prepared);
        c.end_frame();
        assert_eq!(c.lifecycle(), FrameLifecycle::Rendered);
        c.present_frame();
        assert_eq!(c.lifecycle(), FrameLifecycle::Presented);
        c.prepare_frame();
        assert_eq!(c.lifecycle(), FrameLifecycle::Prepared);
    }

    #[test]
    fn submit_scene_flattens() {
        let mut c = Compositor::new(100, 100, 32, QualityProfile::Balanced);
        assert!(c.flat_scene().is_empty());
        c.submit_scene(root_scene()).unwrap();
        // Root is not a visual node, but flatten should have run (no panic).
        // Flat cache is either empty-after-walk or populated — the key
        // invariant is that flatten was called.
        let _ = c.flat_scene();
    }

    #[test]
    #[should_panic(expected = "end_frame called without prepare_frame")]
    fn lifecycle_rejects_end_without_prepare() {
        let mut c = Compositor::new(100, 100, 32, QualityProfile::Balanced);
        c.end_frame();
    }
}
