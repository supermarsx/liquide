use std::collections::HashMap;

use crate::keyframe::{AnimValue, KeyframeTrack};

/// Unique identifier for an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub u64);

/// The current state of an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// Waiting for the delay to elapse.
    Pending,
    /// Actively running.
    Running,
    /// Paused (time not advancing).
    Paused,
    /// Completed all iterations.
    Finished,
}

/// How the animation's final value persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    /// No fill — value reverts after animation ends.
    None,
    /// Retains the final frame's value after finishing.
    Forwards,
    /// Applies the first frame's value during the delay.
    Backwards,
    /// Combines Forwards and Backwards.
    Both,
}

/// The direction an animation plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayDirection {
    /// Normal (0 → 1).
    Normal,
    /// Reversed (1 → 0).
    Reverse,
    /// Alternates: even iterations normal, odd iterations reversed.
    Alternate,
    /// Alternates starting reversed: even iterations reversed, odd normal.
    AlternateReverse,
}

/// A compositor-driven animation with one or more property tracks.
pub struct Animation {
    /// Unique identifier.
    pub id: AnimationId,
    /// Named property tracks (e.g., "opacity", "transform").
    pub tracks: HashMap<String, KeyframeTrack>,
    /// Total duration of one iteration in milliseconds.
    pub duration_ms: f32,
    /// Delay before the animation starts in milliseconds.
    pub delay_ms: f32,
    /// Number of iterations. Use `f32::INFINITY` for infinite looping.
    pub iteration_count: f32,
    /// Play direction.
    pub direction: PlayDirection,
    /// Fill mode.
    pub fill_mode: FillMode,
    /// Current state.
    pub state: AnimationState,
    /// Total elapsed time since the animation was started (includes delay).
    pub elapsed_ms: f32,
    /// Current iteration index (0-based).
    pub current_iteration: u32,
}

impl Animation {
    /// Create a new animation. It starts in the `Pending` state.
    pub fn new(id: AnimationId, tracks: HashMap<String, KeyframeTrack>, duration_ms: f32) -> Self {
        Self {
            id,
            tracks,
            duration_ms,
            delay_ms: 0.0,
            iteration_count: 1.0,
            direction: PlayDirection::Normal,
            fill_mode: FillMode::None,
            state: AnimationState::Pending,
            elapsed_ms: 0.0,
            current_iteration: 0,
        }
    }

    /// Advance the animation by `dt_ms` milliseconds.
    ///
    /// Returns `true` if the animation is still active (not Finished), `false`
    /// if it has just completed.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        match self.state {
            AnimationState::Paused | AnimationState::Finished => {
                return self.state != AnimationState::Finished;
            }
            _ => {}
        }

        self.elapsed_ms += dt_ms;

        // Handle delay.
        if self.elapsed_ms < self.delay_ms {
            self.state = AnimationState::Pending;
            return true;
        }

        self.state = AnimationState::Running;

        let active_time = self.elapsed_ms - self.delay_ms;
        if self.duration_ms <= 0.0 {
            self.state = AnimationState::Finished;
            return false;
        }

        let raw_iteration = active_time / self.duration_ms;

        if raw_iteration >= self.iteration_count {
            // Animation complete.
            self.current_iteration = (self.iteration_count.ceil() as u32).saturating_sub(1);
            self.state = AnimationState::Finished;
            return false;
        }

        self.current_iteration = raw_iteration.floor() as u32;
        true
    }

    /// Compute the effective progress `t` in [0, 1] for the current frame,
    /// accounting for iteration count, direction, and fill mode.
    pub fn effective_t(&self) -> f32 {
        let active_time = self.elapsed_ms - self.delay_ms;

        if active_time < 0.0 {
            // During delay.
            return match self.fill_mode {
                FillMode::Backwards | FillMode::Both => self.directed_t(0.0, 0),
                _ => 0.0,
            };
        }

        if self.duration_ms <= 0.0 {
            return 1.0;
        }

        let raw_iteration = active_time / self.duration_ms;

        if raw_iteration >= self.iteration_count {
            // After completion.
            return match self.fill_mode {
                FillMode::Forwards | FillMode::Both => {
                    let final_iter = (self.iteration_count.ceil() as u32).saturating_sub(1);
                    // If iteration_count is whole, final t = 1.0 of last iteration;
                    // otherwise it's the fractional part.
                    let frac = self.iteration_count.fract();
                    let final_t = if frac < 1e-6 { 1.0 } else { frac };
                    self.directed_t(final_t, final_iter)
                }
                _ => 0.0,
            };
        }

        let iteration = raw_iteration.floor() as u32;
        let local_t = raw_iteration.fract();
        // Handle exact integer boundaries (e.g., t=1.0 of previous iteration).
        let local_t = if local_t < 1e-7 && iteration > 0 {
            1.0
        } else if local_t < 1e-7 {
            0.0
        } else {
            local_t
        };

        self.directed_t(local_t, iteration)
    }

    /// Apply direction to a local progress value.
    fn directed_t(&self, t: f32, iteration: u32) -> f32 {
        match self.direction {
            PlayDirection::Normal => t,
            PlayDirection::Reverse => 1.0 - t,
            PlayDirection::Alternate => {
                if iteration % 2 == 0 {
                    t
                } else {
                    1.0 - t
                }
            }
            PlayDirection::AlternateReverse => {
                if iteration % 2 == 0 {
                    1.0 - t
                } else {
                    t
                }
            }
        }
    }

    /// Get the current animated value for a named property.
    pub fn sample(&self, property: &str) -> Option<AnimValue> {
        let track = self.tracks.get(property)?;
        let t = self.effective_t();
        Some(track.sample(t))
    }

    /// Pause the animation. Time will not advance until `resume()` is called.
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running || self.state == AnimationState::Pending {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume a paused animation.
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            // Determine whether we should be pending or running.
            if self.elapsed_ms < self.delay_ms {
                self.state = AnimationState::Pending;
            } else {
                self.state = AnimationState::Running;
            }
        }
    }

    /// Cancel the animation, setting it to Finished immediately.
    pub fn cancel(&mut self) {
        self.state = AnimationState::Finished;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing::EasingFunction;
    use crate::keyframe::{Keyframe, KeyframeTrack};

    fn simple_opacity_anim(duration_ms: f32) -> Animation {
        let mut tracks = HashMap::new();
        tracks.insert(
            "opacity".to_string(),
            KeyframeTrack::new(vec![
                Keyframe {
                    offset: 0.0,
                    value: AnimValue::Float(0.0),
                    easing: EasingFunction::Linear,
                },
                Keyframe {
                    offset: 1.0,
                    value: AnimValue::Float(1.0),
                    easing: EasingFunction::Linear,
                },
            ]),
        );
        Animation::new(AnimationId(1), tracks, duration_ms)
    }

    #[test]
    fn new_animation_is_pending() {
        let anim = simple_opacity_anim(1000.0);
        assert_eq!(anim.state, AnimationState::Pending);
    }

    #[test]
    fn tick_starts_running() {
        let mut anim = simple_opacity_anim(1000.0);
        assert!(anim.tick(16.0));
        assert_eq!(anim.state, AnimationState::Running);
    }

    #[test]
    fn tick_completes() {
        let mut anim = simple_opacity_anim(100.0);
        assert!(anim.tick(50.0));
        assert_eq!(anim.state, AnimationState::Running);
        assert!(!anim.tick(60.0));
        assert_eq!(anim.state, AnimationState::Finished);
    }

    #[test]
    fn effective_t_progression() {
        let mut anim = simple_opacity_anim(100.0);
        anim.tick(50.0);
        let t = anim.effective_t();
        assert!((t - 0.5).abs() < 0.01, "expected ~0.5, got {t}");
    }

    #[test]
    fn sample_opacity() {
        let mut anim = simple_opacity_anim(100.0);
        anim.tick(50.0);
        match anim.sample("opacity") {
            Some(AnimValue::Float(v)) => assert!((v - 0.5).abs() < 0.02),
            other => panic!("expected Float(~0.5), got {other:?}"),
        }
    }

    #[test]
    fn sample_missing_property() {
        let anim = simple_opacity_anim(100.0);
        assert!(anim.sample("transform").is_none());
    }

    #[test]
    fn delay_keeps_pending() {
        let mut anim = simple_opacity_anim(100.0);
        anim.delay_ms = 50.0;
        assert!(anim.tick(30.0));
        assert_eq!(anim.state, AnimationState::Pending);
        assert!(anim.tick(30.0)); // now at 60ms, past 50ms delay
        assert_eq!(anim.state, AnimationState::Running);
    }

    #[test]
    fn iteration_count() {
        let mut anim = simple_opacity_anim(100.0);
        anim.iteration_count = 3.0;
        assert!(anim.tick(150.0));
        assert_eq!(anim.current_iteration, 1);
        assert!(anim.tick(100.0));
        assert_eq!(anim.current_iteration, 2);
        assert!(!anim.tick(100.0));
        assert_eq!(anim.state, AnimationState::Finished);
    }

    #[test]
    fn direction_reverse() {
        let mut anim = simple_opacity_anim(100.0);
        anim.direction = PlayDirection::Reverse;
        anim.tick(25.0);
        let t = anim.effective_t();
        assert!(
            (t - 0.75).abs() < 0.01,
            "reverse at 25%: expected ~0.75, got {t}"
        );
    }

    #[test]
    fn direction_alternate() {
        let mut anim = simple_opacity_anim(100.0);
        anim.iteration_count = 3.0;
        anim.direction = PlayDirection::Alternate;

        // First iteration (normal): 50ms = t=0.5
        anim.tick(50.0);
        let t = anim.effective_t();
        assert!((t - 0.5).abs() < 0.01, "alternate iter 0 at 50ms: {t}");

        // Second iteration (reverse): at 150ms, local=0.5 → directed = 0.5
        anim.tick(100.0);
        let t = anim.effective_t();
        assert!((t - 0.5).abs() < 0.01, "alternate iter 1 at 150ms: {t}");
    }

    #[test]
    fn fill_mode_forwards() {
        let mut anim = simple_opacity_anim(100.0);
        anim.fill_mode = FillMode::Forwards;
        anim.tick(200.0); // well past end
        let t = anim.effective_t();
        assert!(
            (t - 1.0).abs() < 0.01,
            "forwards fill should hold 1.0, got {t}"
        );
    }

    #[test]
    fn fill_mode_backwards() {
        let mut anim = simple_opacity_anim(100.0);
        anim.delay_ms = 50.0;
        anim.fill_mode = FillMode::Backwards;
        anim.tick(20.0); // during delay
        let t = anim.effective_t();
        assert!((t - 0.0).abs() < 0.01, "backwards fill during delay: {t}");
    }

    #[test]
    fn fill_mode_none_after_finish() {
        let mut anim = simple_opacity_anim(100.0);
        anim.fill_mode = FillMode::None;
        anim.tick(200.0);
        let t = anim.effective_t();
        assert!((t - 0.0).abs() < 0.01, "no fill should be 0.0, got {t}");
    }

    #[test]
    fn pause_and_resume() {
        let mut anim = simple_opacity_anim(100.0);
        anim.tick(30.0);
        assert_eq!(anim.state, AnimationState::Running);

        anim.pause();
        assert_eq!(anim.state, AnimationState::Paused);

        // Ticking while paused should not advance.
        anim.tick(50.0);
        assert_eq!(anim.state, AnimationState::Paused);
        let t = anim.effective_t();
        assert!(
            (t - 0.3).abs() < 0.01,
            "paused t should still be ~0.3, got {t}"
        );

        anim.resume();
        assert_eq!(anim.state, AnimationState::Running);
        anim.tick(20.0);
        let t = anim.effective_t();
        assert!((t - 0.5).abs() < 0.01, "resumed, should be ~0.5, got {t}");
    }

    #[test]
    fn cancel() {
        let mut anim = simple_opacity_anim(100.0);
        anim.tick(30.0);
        anim.cancel();
        assert_eq!(anim.state, AnimationState::Finished);
    }

    #[test]
    fn zero_duration_finishes_immediately() {
        let mut anim = simple_opacity_anim(0.0);
        assert!(!anim.tick(1.0));
        assert_eq!(anim.state, AnimationState::Finished);
    }

    #[test]
    fn infinite_iterations() {
        let mut anim = simple_opacity_anim(100.0);
        anim.iteration_count = f32::INFINITY;
        // Should never finish.
        for _ in 0..100 {
            assert!(anim.tick(100.0));
        }
        assert_ne!(anim.state, AnimationState::Finished);
    }
}
