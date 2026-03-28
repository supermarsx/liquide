use crate::effects::Rect;

/// Quadratic ease-in-out: slow start, fast middle, slow end.
pub fn ease_in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Tracks the animation state for minimize/restore transitions.
///
/// The window interpolates between its full-size rectangle and a small
/// rectangle (typically the dock/taskbar icon position), fading opacity along
/// the way.
pub struct MinimizeAnimation {
    active: bool,
    from: Rect,
    to: Rect,
    duration_ms: f32,
    elapsed_ms: f32,
    /// `true` when animating a minimize (opacity fades out).
    minimizing: bool,
}

impl MinimizeAnimation {
    pub fn new() -> Self {
        Self {
            active: false,
            from: Rect::new(0.0, 0.0, 0.0, 0.0),
            to: Rect::new(0.0, 0.0, 0.0, 0.0),
            duration_ms: 0.0,
            elapsed_ms: 0.0,
            minimizing: true,
        }
    }

    /// Begin a minimize animation.
    /// `from` is the window rectangle; `to` is the dock icon rectangle.
    pub fn begin_minimize(&mut self, from: Rect, to: Rect, duration_ms: f32) {
        self.active = true;
        self.from = from;
        self.to = to;
        self.duration_ms = duration_ms.max(0.0);
        self.elapsed_ms = 0.0;
        self.minimizing = true;
    }

    /// Begin a restore animation.
    /// `from` is the dock icon rectangle; `to` is the full window rectangle.
    pub fn begin_restore(&mut self, from: Rect, to: Rect, duration_ms: f32) {
        self.active = true;
        self.from = from;
        self.to = to;
        self.duration_ms = duration_ms.max(0.0);
        self.elapsed_ms = 0.0;
        self.minimizing = false;
    }

    /// Advance the animation by `dt_ms` milliseconds.
    /// Returns `true` while the animation is still running.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        if !self.active {
            return false;
        }
        self.elapsed_ms += dt_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.elapsed_ms = self.duration_ms;
            self.active = false;
        }
        true
    }

    /// The current interpolated rectangle.
    pub fn current_rect(&self) -> Rect {
        let t = self.progress();
        self.from.lerp(&self.to, t)
    }

    /// Current opacity: fades to 0 during minimize, fades from 0 during restore.
    pub fn current_opacity(&self) -> f32 {
        let t = self.progress();
        if self.minimizing {
            1.0 - t
        } else {
            t
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    fn progress(&self) -> f32 {
        if self.duration_ms <= 0.0 {
            return 1.0;
        }
        let raw = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
        ease_in_out_quad(raw)
    }
}

impl Default for MinimizeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_rect() -> Rect {
        Rect::new(100.0, 100.0, 800.0, 600.0)
    }

    fn icon_rect() -> Rect {
        Rect::new(450.0, 1050.0, 48.0, 48.0)
    }

    // ── ease_in_out_quad ────────────────────────────────────────────

    #[test]
    fn easing_at_zero() {
        assert!((ease_in_out_quad(0.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn easing_at_one() {
        assert!((ease_in_out_quad(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn easing_at_half() {
        assert!((ease_in_out_quad(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn easing_clamps_negative() {
        assert!((ease_in_out_quad(-1.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn easing_clamps_above_one() {
        assert!((ease_in_out_quad(2.0) - 1.0).abs() < 1e-5);
    }

    // ── MinimizeAnimation ───────────────────────────────────────────

    #[test]
    fn minimize_at_t0_matches_from() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_minimize(window_rect(), icon_rect(), 200.0);
        let r = anim.current_rect();
        assert!((r.x - 100.0).abs() < 1e-3);
        assert!((r.y - 100.0).abs() < 1e-3);
        assert!((r.width - 800.0).abs() < 1e-3);
        assert!((r.height - 600.0).abs() < 1e-3);
    }

    #[test]
    fn minimize_at_t1_matches_to() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_minimize(window_rect(), icon_rect(), 200.0);
        anim.tick(200.0);
        let r = anim.current_rect();
        assert!((r.x - icon_rect().x).abs() < 1e-3);
        assert!((r.y - icon_rect().y).abs() < 1e-3);
        assert!((r.width - icon_rect().width).abs() < 1e-3);
        assert!((r.height - icon_rect().height).abs() < 1e-3);
    }

    #[test]
    fn minimize_opacity_fades_out() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_minimize(window_rect(), icon_rect(), 200.0);
        assert!((anim.current_opacity() - 1.0).abs() < 1e-3);
        anim.tick(200.0);
        assert!((anim.current_opacity() - 0.0).abs() < 1e-3);
    }

    #[test]
    fn restore_opacity_fades_in() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_restore(icon_rect(), window_rect(), 200.0);
        assert!((anim.current_opacity() - 0.0).abs() < 1e-3);
        anim.tick(200.0);
        assert!((anim.current_opacity() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn restore_at_t1_matches_to() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_restore(icon_rect(), window_rect(), 200.0);
        anim.tick(200.0);
        let r = anim.current_rect();
        assert!((r.x - 100.0).abs() < 1e-3);
        assert!((r.width - 800.0).abs() < 1e-3);
    }

    #[test]
    fn tick_returns_false_when_done() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_minimize(window_rect(), icon_rect(), 100.0);
        assert!(anim.tick(50.0));
        assert!(anim.is_active());
        // This tick finishes the animation
        let still = anim.tick(60.0);
        // tick returns true for the finishing frame
        assert!(still);
        assert!(!anim.is_active());
        // Subsequent tick returns false
        assert!(!anim.tick(10.0));
    }

    #[test]
    fn zero_duration_finishes_immediately() {
        let mut anim = MinimizeAnimation::new();
        anim.begin_minimize(window_rect(), icon_rect(), 0.0);
        // zero-duration means progress() returns 1.0 immediately
        let r = anim.current_rect();
        assert!((r.x - icon_rect().x).abs() < 1e-3);
        assert!(!anim.is_active() || { anim.tick(0.0); !anim.is_active() });
    }

    #[test]
    fn minimize_animation_default() {
        let anim = MinimizeAnimation::default();
        assert!(!anim.is_active());
    }

    #[test]
    fn minimize_midpoint_interpolation() {
        let mut anim = MinimizeAnimation::new();
        let from = Rect::new(0.0, 0.0, 1000.0, 500.0);
        let to = Rect::new(0.0, 0.0, 0.0, 0.0);
        anim.begin_minimize(from, to, 100.0);
        anim.tick(50.0); // halfway
        let r = anim.current_rect();
        // At midpoint eased t=0.5, lerp should give 500.0 width
        assert!((r.width - 500.0).abs() < 1e-3);
    }
}
