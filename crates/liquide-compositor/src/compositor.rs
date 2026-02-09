//! Compositor contract trait and concrete implementation.

use crate::cursor::CursorUpdate;
use crate::damage::{DamageSet, DamageTracker};
use crate::effects::{
    DegradationController, DegradationLevel, EffectBudget, EffectParams, QualityProfile,
};
use crate::framebuffer::{DoubleBuffer, FrameBuffer};
use crate::scene::{GlassParams, SceneNode};
use crate::Result;

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
}

impl Compositor {
    /// Create a new compositor.
    #[must_use]
    pub fn new(width: u32, height: u32, tile_size: u32, profile: QualityProfile) -> Self {
        Self {
            scene: None,
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
        }
    }

    /// Resize the compositor output.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.double_buffer =
            DoubleBuffer::new(width, height, crate::pixel::PixelFormat::Bgra8);
        self.damage_tracker.resize(width, height);
        Ok(())
    }

    /// Advance to the next frame: swap front/back buffers.
    pub fn begin_frame(&mut self) {
        self.double_buffer.swap();
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
        self.scene = Some(root);
        Ok(())
    }

    fn compute_damage(&mut self) -> Result<DamageSet> {
        Ok(self.damage_tracker.compute_damage(self.double_buffer.front()))
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
        if let Some(entry) = self.glass_surfaces.iter_mut().find(|(id, _)| *id == surface_id) {
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
