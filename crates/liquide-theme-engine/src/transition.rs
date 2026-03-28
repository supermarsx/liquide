use crate::palette::ColorPalette;

/// Smooth animated transition between two color palettes.
///
/// Create one when switching themes, then call [`tick()`](Self::tick) every
/// frame with elapsed milliseconds.  The [`interpolate()`](Self::interpolate)
/// method returns the blended palette at the current progress.
#[derive(Debug, Clone)]
pub struct ThemeTransition {
    pub from: ColorPalette,
    pub to: ColorPalette,
    /// Current progress (0.0 = start, 1.0 = complete).
    pub progress: f32,
    /// Total transition duration in milliseconds.
    pub duration_ms: u32,
    /// Elapsed time so far (milliseconds), tracked internally.
    elapsed_ms: u32,
}

impl ThemeTransition {
    /// Create a new transition from one palette to another.
    pub fn new(from: ColorPalette, to: ColorPalette, duration_ms: u32) -> Self {
        Self {
            from,
            to,
            progress: 0.0,
            duration_ms: duration_ms.max(1),
            elapsed_ms: 0,
        }
    }

    /// Advance the transition by `delta_ms` milliseconds.
    ///
    /// Returns `true` when the transition is complete (progress >= 1.0).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        self.progress = (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0);
        self.is_complete()
    }

    /// Whether the transition has finished.
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Interpolate all palette colors at the current progress, using an
    /// ease-in-out cubic curve for smooth animation.
    pub fn interpolate(&self) -> ColorPalette {
        let t = ease_in_out_cubic(self.progress);
        self.from.lerp(&self.to, t)
    }

    /// Interpolate with a custom `t` override (ignoring internal progress).
    pub fn interpolate_at(&self, t: f32) -> ColorPalette {
        let t = ease_in_out_cubic(t.clamp(0.0, 1.0));
        self.from.lerp(&self.to, t)
    }

    /// Reset the transition (rewind to 0).
    pub fn reset(&mut self) {
        self.progress = 0.0;
        self.elapsed_ms = 0;
    }

    /// Replace the target palette mid-transition, keeping current progress.
    pub fn retarget(&mut self, new_to: ColorPalette) {
        // Snapshot current interpolated state as the new "from".
        self.from = self.interpolate();
        self.to = new_to;
        self.elapsed_ms = 0;
        self.progress = 0.0;
    }
}

/// Cubic ease-in-out: smooth acceleration then deceleration.
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = 2.0 * t - 2.0;
        0.5 * f * f * f + 1.0
    }
}
