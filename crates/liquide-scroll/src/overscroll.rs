/// Rubber-band overscroll effect with spring-back animation.
#[derive(Debug, Clone)]
pub struct OverscrollEffect {
    /// Maximum visual overscroll displacement in pixels.
    pub max_overscroll: f32,
    /// Whether the overscroll effect is enabled.
    pub enabled: bool,
}

impl OverscrollEffect {
    pub fn new() -> Self {
        Self {
            max_overscroll: 100.0,
            enabled: true,
        }
    }

    /// Apply rubber-band effect to a scroll offset.
    ///
    /// Given a raw `offset`, the `max` scroll limit, and how far past the
    /// boundary the user has scrolled (`overscroll_amount`), returns the
    /// visual offset with rubber-banding.
    ///
    /// When `overscroll_amount` is 0 or overscroll is disabled, returns `offset` unchanged.
    /// When `overscroll_amount` is positive (scrolled past end) or negative (scrolled past start),
    /// the visual offset is dampened using an exponential rubber-band formula.
    pub fn apply(&self, offset: f32, max: f32, overscroll_amount: f32) -> f32 {
        if !self.enabled || overscroll_amount == 0.0 {
            return offset;
        }

        let abs_excess = overscroll_amount.abs();
        let sign = overscroll_amount.signum();
        let max_os = self.max_overscroll;

        // Rubber-band formula: dampened displacement
        // displacement = max_overscroll * (1 - exp(-abs(excess) / max_overscroll))
        let displacement = max_os * (1.0 - (-abs_excess / max_os).exp());

        // Apply displacement in the direction of overscroll.
        // If overscrolling past the start (negative), offset is negative.
        // If overscrolling past the end (positive), offset is beyond max.
        if sign < 0.0 {
            // Scrolled before start: visual offset goes negative
            -displacement
        } else {
            // Scrolled past end: visual offset goes beyond max
            max + displacement
        }
    }

    /// Create a spring animation to release from the current overscroll position
    /// back to the nearest boundary.
    ///
    /// `current_overscroll` is the current visual displacement beyond bounds
    /// (negative = before start, positive = past end).
    pub fn release(&self, current_overscroll: f32) -> SpringAnimation {
        SpringAnimation {
            value: current_overscroll,
            target: 0.0,
            velocity: 0.0,
            stiffness: 300.0,
            damping: 25.0,
        }
    }
}

impl Default for OverscrollEffect {
    fn default() -> Self {
        Self::new()
    }
}

/// Damped spring animation for snapping back from overscroll.
#[derive(Debug, Clone)]
pub struct SpringAnimation {
    /// Current value (displacement from rest).
    pub value: f32,
    /// Target value (rest position, typically 0).
    pub target: f32,
    /// Current velocity.
    pub velocity: f32,
    /// Spring stiffness constant.
    pub stiffness: f32,
    /// Damping coefficient.
    pub damping: f32,
}

impl SpringAnimation {
    /// Create a new spring animation.
    pub fn new(value: f32, target: f32, velocity: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value,
            target,
            velocity,
            stiffness,
            damping,
        }
    }

    /// Advance the spring simulation by `dt` seconds.
    /// Returns the new value.
    pub fn tick(&mut self, dt: f32) -> f32 {
        let displacement = self.value - self.target;

        // Spring force: F = -k * x
        let spring_force = -self.stiffness * displacement;
        // Damping force: F = -c * v
        let damping_force = -self.damping * self.velocity;

        let acceleration = spring_force + damping_force;

        // Semi-implicit Euler integration
        self.velocity += acceleration * dt;
        self.value += self.velocity * dt;

        self.value
    }

    /// Whether the spring has settled (close to target with near-zero velocity).
    pub fn is_settled(&self) -> bool {
        let displacement = (self.value - self.target).abs();
        let speed = self.velocity.abs();
        displacement < 0.5 && speed < 0.1
    }

    /// Snap to target immediately.
    pub fn settle(&mut self) {
        self.value = self.target;
        self.velocity = 0.0;
    }
}
