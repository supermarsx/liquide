//! Animation primitives and management.

use std::collections::HashMap;

/// Easing function for animations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// Starts slow, accelerates.
    EaseIn,
    /// Starts fast, decelerates.
    EaseOut,
    /// Starts slow, speeds up, then slows down.
    EaseInOut,
    /// Custom cubic bezier curve.
    CubicBezier {
        /// First control point x.
        x1: f32,
        /// First control point y.
        y1: f32,
        /// Second control point x.
        x2: f32,
        /// Second control point y.
        y2: f32,
    },
}

impl Default for Easing {
    fn default() -> Self {
        Self::Linear
    }
}

impl Easing {
    /// Apply the easing function to a linear progress value (0.0 to 1.0).
    #[must_use]
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            Self::CubicBezier { x1, y1, x2, y2 } => {
                cubic_bezier_sample(t, *x1 as f64, *y1 as f64, *x2 as f64, *y2 as f64)
            }
        }
    }
}

/// Approximate a cubic bezier y value for a given t using the bezier formula.
fn cubic_bezier_sample(t: f64, _x1: f64, y1: f64, _x2: f64, y2: f64) -> f64 {
    // Simplified: compute the bezier y for a given parametric t.
    // B(t) = 3(1-t)^2*t*y1 + 3(1-t)*t^2*y2 + t^3
    let mt = 1.0 - t;
    3.0 * mt * mt * t * y1 + 3.0 * mt * t * t * y2 + t * t * t
}

/// State of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationState {
    /// The animation is idle (not yet started or reset).
    Idle,
    /// The animation is currently running.
    Running,
    /// The animation is paused.
    Paused,
    /// The animation has completed.
    Completed,
}

/// A single animation interpolating between two values over time.
#[derive(Debug, Clone)]
pub struct Animation {
    /// Unique identifier.
    pub id: u64,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Easing function.
    pub easing: Easing,
    /// Current state.
    pub state: AnimationState,
    /// Starting value.
    pub from_value: f64,
    /// Ending value.
    pub to_value: f64,
}

impl Animation {
    /// Create a new animation.
    #[must_use]
    pub fn new(from: f64, to: f64, duration_ms: u64, easing: Easing) -> Self {
        Self {
            id: 0,
            duration_ms,
            elapsed_ms: 0,
            easing,
            state: AnimationState::Running,
            from_value: from,
            to_value: to,
        }
    }

    /// Advance the animation by the given number of milliseconds.
    pub fn tick(&mut self, delta_ms: u64) {
        if self.state != AnimationState::Running {
            return;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms >= self.duration_ms {
            self.elapsed_ms = self.duration_ms;
            self.state = AnimationState::Completed;
        }
    }

    /// The current linear progress (0.0 to 1.0).
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).min(1.0)
    }

    /// The current interpolated value, with easing applied.
    #[must_use]
    pub fn current_value(&self) -> f64 {
        let eased = self.easing.apply(self.progress());
        self.from_value + (self.to_value - self.from_value) * eased
    }

    /// Whether the animation has completed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state == AnimationState::Completed
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume a paused animation.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Reset the animation to the beginning.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
        self.state = AnimationState::Idle;
    }
}

/// Manager for multiple concurrent animations.
#[derive(Debug, Clone, Default)]
pub struct AnimationManager {
    /// Active animations.
    animations: HashMap<u64, Animation>,
    /// Counter for generating unique animation ids.
    next_id: u64,
}

impl AnimationManager {
    /// Create a new animation manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new animation and return its id.
    pub fn start(&mut self, from: f64, to: f64, duration_ms: u64, easing: Easing) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let mut anim = Animation::new(from, to, duration_ms, easing);
        anim.id = id;
        self.animations.insert(id, anim);
        id
    }

    /// Tick all running animations by the given delta.
    ///
    /// Returns the ids of animations that completed during this tick.
    pub fn tick_all(&mut self, delta_ms: u64) -> Vec<u64> {
        let mut completed = Vec::new();
        for (id, anim) in &mut self.animations {
            let was_complete = anim.is_complete();
            anim.tick(delta_ms);
            if !was_complete && anim.is_complete() {
                completed.push(*id);
            }
        }
        completed
    }

    /// Cancel and remove an animation.
    pub fn cancel(&mut self, id: u64) {
        self.animations.remove(&id);
    }

    /// Get a reference to an animation by id.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<&Animation> {
        self.animations.get(&id)
    }

    /// Get a mutable reference to an animation by id.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Animation> {
        self.animations.get_mut(&id)
    }

    /// The number of active (non-completed) animations.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.animations
            .values()
            .filter(|a| a.state == AnimationState::Running || a.state == AnimationState::Paused)
            .count()
    }

    /// Total number of tracked animations (including completed).
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.animations.len()
    }
}
