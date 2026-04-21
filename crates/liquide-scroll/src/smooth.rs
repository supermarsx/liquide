/// Default animation duration for scroll snap transitions in milliseconds.
pub const SNAP_ANIMATION_DURATION_MS: u32 = 250;

/// Smooth scroll animator using ease-out cubic easing.
#[derive(Debug, Clone)]
pub struct SmoothScroller {
    /// Start offset when animation began.
    start: (f32, f32),
    /// Target offset to animate toward.
    target: (f32, f32),
    /// Total animation duration in milliseconds.
    duration_ms: u32,
    /// Elapsed time since animation started.
    elapsed_ms: u32,
    /// Whether an animation is currently running.
    active: bool,
}

impl SmoothScroller {
    pub fn new() -> Self {
        Self {
            start: (0.0, 0.0),
            target: (0.0, 0.0),
            duration_ms: 0,
            elapsed_ms: 0,
            active: false,
        }
    }

    /// Start a smooth scroll to an absolute target position.
    /// `current` is the current scroll offset used as the animation start.
    pub fn scroll_to(&mut self, current: (f32, f32), target: (f32, f32), duration_ms: u32) {
        if duration_ms == 0 {
            self.active = false;
            return;
        }
        self.start = current;
        self.target = target;
        self.duration_ms = duration_ms;
        self.elapsed_ms = 0;
        self.active = true;
    }

    /// Start a smooth scroll by a relative delta from the current position.
    pub fn scroll_by(&mut self, current: (f32, f32), delta: (f32, f32), duration_ms: u32) {
        let target = (current.0 + delta.0, current.1 + delta.1);
        self.scroll_to(current, target, duration_ms);
    }

    /// Advance the animation by `elapsed_ms` milliseconds.
    /// Returns the new scroll offset.
    pub fn tick(&mut self, elapsed_ms: u32) -> (f32, f32) {
        if !self.active {
            return self.target;
        }

        self.elapsed_ms += elapsed_ms;

        if self.elapsed_ms >= self.duration_ms {
            self.active = false;
            return self.target;
        }

        let t = self.elapsed_ms as f32 / self.duration_ms as f32;
        let eased = ease_out_cubic(t);

        let x = self.start.0 + (self.target.0 - self.start.0) * eased;
        let y = self.start.1 + (self.target.1 - self.start.1) * eased;
        (x, y)
    }

    /// Whether the scroller is currently animating.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Cancel the animation, freezing at the current interpolated position.
    pub fn cancel(&mut self) -> (f32, f32) {
        if !self.active {
            return self.target;
        }
        let t = if self.duration_ms > 0 {
            (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0)
        } else {
            1.0
        };
        let eased = ease_out_cubic(t);
        let pos = (
            self.start.0 + (self.target.0 - self.start.0) * eased,
            self.start.1 + (self.target.1 - self.start.1) * eased,
        );
        self.active = false;
        self.target = pos;
        pos
    }

    /// The current target position.
    pub fn target(&self) -> (f32, f32) {
        self.target
    }
}

impl Default for SmoothScroller {
    fn default() -> Self {
        Self::new()
    }
}

/// Ease-out cubic: f(t) = 1 - (1-t)^3
fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}
