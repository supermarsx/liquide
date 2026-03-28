/// Touch/trackpad momentum scroller.
///
/// Accumulates velocity from recent touch samples and applies
/// deceleration after the touch ends.
#[derive(Debug, Clone)]
pub struct MomentumScroller {
    /// Rolling buffer of recent touch samples for velocity computation.
    samples: [(f32, f32, u64); 3],
    /// Number of valid samples in the buffer (0..=3).
    sample_count: usize,
    /// Index of the next sample to write (circular).
    sample_index: usize,
    /// Current velocity in pixels per millisecond.
    velocity: (f32, f32),
    /// Whether momentum animation is running.
    active: bool,
    /// Whether a touch is currently being tracked.
    tracking: bool,
    /// Deceleration rate per millisecond (e.g. 0.998).
    pub deceleration_rate: f32,
    /// Minimum velocity below which momentum stops (px/ms).
    pub min_velocity: f32,
}

impl MomentumScroller {
    pub fn new() -> Self {
        Self {
            samples: [(0.0, 0.0, 0); 3],
            sample_count: 0,
            sample_index: 0,
            velocity: (0.0, 0.0),
            active: false,
            tracking: false,
            deceleration_rate: 0.998,
            min_velocity: 0.01,
        }
    }

    /// Begin tracking a touch at the given position.
    pub fn begin_touch(&mut self, pos: (f32, f32)) {
        self.samples = [(0.0, 0.0, 0); 3];
        self.sample_count = 0;
        self.sample_index = 0;
        self.velocity = (0.0, 0.0);
        self.active = false;
        self.tracking = true;
        // Store the initial position with timestamp 0 (will be set on move).
        // We don't add a sample here — first move_touch will provide delta.
        self.samples[0] = (pos.0, pos.1, 0);
    }

    /// Record a touch move with position and timestamp.
    /// Call this for each touch/trackpad move event.
    pub fn move_touch(&mut self, pos: (f32, f32), timestamp_ms: u64) {
        if !self.tracking {
            return;
        }
        let idx = self.sample_index % 3;
        self.samples[idx] = (pos.0, pos.1, timestamp_ms);
        self.sample_index += 1;
        if self.sample_count < 3 {
            self.sample_count += 1;
        }
    }

    /// End the touch gesture. Returns `true` if momentum animation was started
    /// (i.e., there was meaningful velocity).
    pub fn end_touch(&mut self) -> bool {
        self.tracking = false;

        if self.sample_count < 2 {
            self.velocity = (0.0, 0.0);
            self.active = false;
            return false;
        }

        // Compute velocity from available samples using rolling average.
        let mut vx = 0.0f32;
        let mut vy = 0.0f32;
        let mut weight = 0.0f32;

        // Walk samples in chronological order and compute per-pair velocity.
        let n = self.sample_count;
        let base = if self.sample_index >= n {
            self.sample_index - n
        } else {
            0
        };

        for i in 1..n {
            let prev_idx = (base + i - 1) % 3;
            let cur_idx = (base + i) % 3;
            let (px, py, pt) = self.samples[prev_idx];
            let (cx, cy, ct) = self.samples[cur_idx];
            let dt = ct as f64 - pt as f64;
            if dt > 0.0 {
                let w = i as f32; // More recent samples get higher weight.
                vx += ((cx - px) as f64 / dt) as f32 * w;
                vy += ((cy - py) as f64 / dt) as f32 * w;
                weight += w;
            }
        }

        if weight > 0.0 {
            vx /= weight;
            vy /= weight;
        }

        let speed = (vx * vx + vy * vy).sqrt();
        if speed < self.min_velocity {
            self.velocity = (0.0, 0.0);
            self.active = false;
            return false;
        }

        self.velocity = (vx, vy);
        self.active = true;
        true
    }

    /// Advance the momentum animation by `elapsed_ms` milliseconds.
    /// Returns the scroll delta to apply this frame.
    pub fn tick(&mut self, elapsed_ms: u32) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        // Apply deceleration for each millisecond elapsed.
        // For efficiency, use pow instead of looping.
        let decay = self.deceleration_rate.powi(elapsed_ms as i32);
        let avg_vx = self.velocity.0 * (1.0 + decay) * 0.5;
        let avg_vy = self.velocity.1 * (1.0 + decay) * 0.5;

        let dx = avg_vx * elapsed_ms as f32;
        let dy = avg_vy * elapsed_ms as f32;

        self.velocity.0 *= decay;
        self.velocity.1 *= decay;

        let speed = (self.velocity.0 * self.velocity.0 + self.velocity.1 * self.velocity.1).sqrt();
        if speed < self.min_velocity {
            self.velocity = (0.0, 0.0);
            self.active = false;
        }

        (dx, dy)
    }

    /// Whether momentum animation is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Cancel the momentum animation.
    pub fn cancel(&mut self) {
        self.active = false;
        self.tracking = false;
        self.velocity = (0.0, 0.0);
    }

    /// Current velocity in px/ms.
    pub fn velocity(&self) -> (f32, f32) {
        self.velocity
    }
}

impl Default for MomentumScroller {
    fn default() -> Self {
        Self::new()
    }
}
