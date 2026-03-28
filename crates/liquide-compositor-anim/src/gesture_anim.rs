//! Gesture-driven animations for interactive UI transitions.
//!
//! Unlike time-based animations, gesture animations are directly controlled by
//! user input (touch drag, mouse swipe, scroll). The animation progress tracks
//! the gesture position, and when released, the animation completes to the
//! nearest snap target using momentum from the gesture velocity.
//!
//! # Use Cases
//!
//! - Swipe-to-dismiss (notifications, windows)
//! - Pull-to-overview (workspace overview)
//! - Workspace transition drag (horizontal workspace switching)
//! - Sheet/drawer drag (bottom sheet pull-up/pull-down)

use crate::spring::{SpringAnimation, SpringConfig};

/// The target state after a gesture is released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureTarget {
    /// Dismiss/close the element (progress snaps to 1.0).
    Dismiss,
    /// Complete the transition (progress snaps to 1.0).
    Complete,
    /// Cancel the gesture (progress returns to 0.0).
    Cancel,
}

/// The phase of a gesture animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    /// Awaiting the first gesture input.
    Idle,
    /// User is actively driving the progress.
    Tracking,
    /// User released; animation is completing to a target.
    Settling,
    /// Animation has finished settling.
    Finished,
}

/// Configuration for gesture animation behavior.
#[derive(Debug, Clone, Copy)]
pub struct GestureConfig {
    /// Velocity threshold (units/sec) above which the gesture commits
    /// to the direction of the swipe regardless of position.
    pub velocity_threshold: f64,
    /// Progress threshold (0.0-1.0) above which the gesture completes
    /// when released with low velocity.
    pub completion_threshold: f64,
    /// Spring configuration used for the settling animation.
    pub settle_spring: SpringConfig,
    /// Whether to allow rubber-banding beyond 0.0 and 1.0.
    pub rubber_band: bool,
    /// Rubber band stiffness factor (0.0-1.0). Lower = more resistance.
    pub rubber_band_factor: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            velocity_threshold: 500.0,
            completion_threshold: 0.5,
            settle_spring: SpringConfig::stiff(),
            rubber_band: true,
            rubber_band_factor: 0.3,
        }
    }
}

/// A gesture-driven animation.
///
/// Progress is controlled directly during tracking, then uses spring physics
/// to settle to the nearest target on release.
pub struct GestureAnimation {
    /// Configuration.
    config: GestureConfig,
    /// Current progress value (nominally 0.0 to 1.0, can exceed with rubber band).
    progress: f64,
    /// Current phase.
    phase: GesturePhase,
    /// The target decided at release time.
    resolved_target: Option<GestureTarget>,
    /// Spring animation used during settling phase.
    settle_spring: Option<SpringAnimation>,
    /// Velocity tracking: last two progress values for velocity estimation.
    prev_progress: f64,
    /// Time of previous set_progress call (for velocity estimation).
    tracking_velocity: f64,
}

impl GestureAnimation {
    /// Create a new gesture animation.
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            progress: 0.0,
            phase: GesturePhase::Idle,
            resolved_target: None,
            settle_spring: None,
            prev_progress: 0.0,
            tracking_velocity: 0.0,
        }
    }

    /// Create a gesture animation with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(GestureConfig::default())
    }

    /// Set the progress directly from gesture input (0.0 to 1.0).
    ///
    /// Typically called on each drag/scroll event. Values outside [0, 1] are
    /// rubber-banded if enabled, otherwise clamped.
    pub fn set_progress(&mut self, p: f64) {
        match self.phase {
            GesturePhase::Idle | GesturePhase::Finished => {
                self.phase = GesturePhase::Tracking;
                self.settle_spring = None;
                self.resolved_target = None;
            }
            GesturePhase::Settling => {
                // User interrupted the settling — go back to tracking.
                self.phase = GesturePhase::Tracking;
                self.settle_spring = None;
                self.resolved_target = None;
            }
            GesturePhase::Tracking => {}
        }

        self.prev_progress = self.progress;

        if self.config.rubber_band {
            if p < 0.0 {
                self.progress = p * self.config.rubber_band_factor;
            } else if p > 1.0 {
                self.progress = 1.0 + (p - 1.0) * self.config.rubber_band_factor;
            } else {
                self.progress = p;
            }
        } else {
            self.progress = p.clamp(0.0, 1.0);
        }
    }

    /// Release the gesture with the given velocity (units per second).
    ///
    /// The animation will settle to the nearest target based on current
    /// progress and velocity. Positive velocity = toward completion (1.0),
    /// negative velocity = toward cancellation (0.0).
    pub fn release(&mut self, velocity: f64) {
        if self.phase != GesturePhase::Tracking {
            return;
        }

        self.tracking_velocity = velocity;

        // Decide target based on velocity and position.
        let target = if velocity.abs() > self.config.velocity_threshold {
            // High velocity: commit to the swipe direction.
            if velocity > 0.0 {
                GestureTarget::Complete
            } else {
                GestureTarget::Cancel
            }
        } else if self.progress >= self.config.completion_threshold {
            GestureTarget::Complete
        } else {
            GestureTarget::Cancel
        };

        self.commit_to_target(target);
    }

    /// Release with a specific target, overriding the velocity/position
    /// heuristics.
    pub fn release_to(&mut self, target: GestureTarget) {
        if self.phase != GesturePhase::Tracking {
            return;
        }
        self.tracking_velocity = 0.0;
        self.commit_to_target(target);
    }

    /// Internal: begin settling toward a target.
    fn commit_to_target(&mut self, target: GestureTarget) {
        self.resolved_target = Some(target);
        self.phase = GesturePhase::Settling;

        let target_value = match target {
            GestureTarget::Dismiss | GestureTarget::Complete => 1.0,
            GestureTarget::Cancel => 0.0,
        };

        let spring_config = self.config.settle_spring
            .with_velocity(self.tracking_velocity);
        self.settle_spring = Some(SpringAnimation::new(
            spring_config,
            self.progress,
            target_value,
        ));
    }

    /// Advance the settling animation by `dt` seconds.
    ///
    /// Returns the current progress value. During tracking phase, this
    /// simply returns the current progress. During settling, it advances
    /// the spring simulation.
    pub fn tick(&mut self, dt: f64) -> f64 {
        match self.phase {
            GesturePhase::Settling => {
                if let Some(ref mut spring) = self.settle_spring {
                    self.progress = spring.tick(dt);
                    if spring.is_at_rest(0.001) {
                        // Snap to exact target value.
                        self.progress = spring.target;
                        self.phase = GesturePhase::Finished;
                    }
                }
            }
            _ => {
                // Idle, Tracking, or Finished — no time-based update needed.
            }
        }
        self.progress
    }

    /// Get the current progress value.
    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Get the current phase.
    pub fn phase(&self) -> GesturePhase {
        self.phase
    }

    /// Get the resolved target (set after release).
    pub fn resolved_target(&self) -> Option<GestureTarget> {
        self.resolved_target
    }

    /// Whether the gesture animation has finished settling.
    pub fn is_finished(&self) -> bool {
        self.phase == GesturePhase::Finished
    }

    /// Whether the animation is currently settling (needs tick calls).
    pub fn needs_animation(&self) -> bool {
        self.phase == GesturePhase::Settling
    }

    /// Reset the animation to idle state at progress 0.0.
    pub fn reset(&mut self) {
        self.progress = 0.0;
        self.phase = GesturePhase::Idle;
        self.resolved_target = None;
        self.settle_spring = None;
        self.prev_progress = 0.0;
        self.tracking_velocity = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn settle(anim: &mut GestureAnimation) -> u32 {
        let mut frames = 0u32;
        while anim.needs_animation() && frames < 5000 {
            anim.tick(DT);
            frames += 1;
        }
        frames
    }

    #[test]
    fn starts_idle() {
        let anim = GestureAnimation::with_defaults();
        assert_eq!(anim.phase(), GesturePhase::Idle);
        assert!((anim.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn set_progress_enters_tracking() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.3);
        assert_eq!(anim.phase(), GesturePhase::Tracking);
        assert!((anim.progress() - 0.3).abs() < 0.001);
    }

    #[test]
    fn set_progress_clamps_without_rubber_band() {
        let config = GestureConfig {
            rubber_band: false,
            ..Default::default()
        };
        let mut anim = GestureAnimation::new(config);
        anim.set_progress(1.5);
        assert!((anim.progress() - 1.0).abs() < 0.001);
        anim.set_progress(-0.5);
        assert!((anim.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn rubber_band_positive() {
        let config = GestureConfig {
            rubber_band: true,
            rubber_band_factor: 0.3,
            ..Default::default()
        };
        let mut anim = GestureAnimation::new(config);
        anim.set_progress(1.5);
        // Overshoot: 1.0 + (1.5 - 1.0) * 0.3 = 1.15
        assert!((anim.progress() - 1.15).abs() < 0.001, "got {}", anim.progress());
    }

    #[test]
    fn rubber_band_negative() {
        let config = GestureConfig {
            rubber_band: true,
            rubber_band_factor: 0.3,
            ..Default::default()
        };
        let mut anim = GestureAnimation::new(config);
        anim.set_progress(-1.0);
        // Undershoot: -1.0 * 0.3 = -0.3
        assert!((anim.progress() - (-0.3)).abs() < 0.001, "got {}", anim.progress());
    }

    #[test]
    fn release_high_positive_velocity_completes() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.1); // low progress
        anim.release(1000.0); // but high forward velocity
        assert_eq!(anim.resolved_target(), Some(GestureTarget::Complete));
        assert_eq!(anim.phase(), GesturePhase::Settling);
    }

    #[test]
    fn release_high_negative_velocity_cancels() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.9); // high progress
        anim.release(-1000.0); // but high reverse velocity
        assert_eq!(anim.resolved_target(), Some(GestureTarget::Cancel));
    }

    #[test]
    fn release_low_velocity_uses_position_complete() {
        let config = GestureConfig {
            completion_threshold: 0.5,
            velocity_threshold: 500.0,
            ..Default::default()
        };
        let mut anim = GestureAnimation::new(config);
        anim.set_progress(0.7);
        anim.release(100.0); // low velocity
        assert_eq!(anim.resolved_target(), Some(GestureTarget::Complete));
    }

    #[test]
    fn release_low_velocity_uses_position_cancel() {
        let config = GestureConfig {
            completion_threshold: 0.5,
            velocity_threshold: 500.0,
            ..Default::default()
        };
        let mut anim = GestureAnimation::new(config);
        anim.set_progress(0.3);
        anim.release(100.0); // low velocity
        assert_eq!(anim.resolved_target(), Some(GestureTarget::Cancel));
    }

    #[test]
    fn settling_converges_to_complete() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.8);
        anim.release(200.0);
        settle(&mut anim);
        assert!(anim.is_finished());
        assert!((anim.progress() - 1.0).abs() < 0.01, "should settle to 1.0, got {}", anim.progress());
    }

    #[test]
    fn settling_converges_to_cancel() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.2);
        anim.release(-200.0);
        settle(&mut anim);
        assert!(anim.is_finished());
        assert!((anim.progress() - 0.0).abs() < 0.01, "should settle to 0.0, got {}", anim.progress());
    }

    #[test]
    fn release_to_specific_target() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.1);
        anim.release_to(GestureTarget::Dismiss);
        assert_eq!(anim.resolved_target(), Some(GestureTarget::Dismiss));
        settle(&mut anim);
        assert!((anim.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn interrupt_settling_with_new_gesture() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.8);
        anim.release(200.0);
        // Start settling...
        for _ in 0..5 {
            anim.tick(DT);
        }
        assert_eq!(anim.phase(), GesturePhase::Settling);

        // Interrupt with new gesture input.
        anim.set_progress(0.3);
        assert_eq!(anim.phase(), GesturePhase::Tracking);
        assert!(anim.resolved_target().is_none());
    }

    #[test]
    fn tick_during_tracking_no_change() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.5);
        let before = anim.progress();
        anim.tick(DT);
        assert!((anim.progress() - before).abs() < 0.001);
    }

    #[test]
    fn tick_during_idle_no_change() {
        let mut anim = GestureAnimation::with_defaults();
        anim.tick(DT);
        assert!((anim.progress() - 0.0).abs() < 0.001);
        assert_eq!(anim.phase(), GesturePhase::Idle);
    }

    #[test]
    fn needs_animation_only_during_settling() {
        let mut anim = GestureAnimation::with_defaults();
        assert!(!anim.needs_animation());
        anim.set_progress(0.5);
        assert!(!anim.needs_animation());
        anim.release(0.0);
        assert!(anim.needs_animation());
    }

    #[test]
    fn reset_returns_to_idle() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.5);
        anim.release(200.0);
        anim.reset();
        assert_eq!(anim.phase(), GesturePhase::Idle);
        assert!((anim.progress() - 0.0).abs() < 0.001);
        assert!(anim.resolved_target().is_none());
    }

    #[test]
    fn release_while_idle_does_nothing() {
        let mut anim = GestureAnimation::with_defaults();
        anim.release(500.0);
        assert_eq!(anim.phase(), GesturePhase::Idle);
        assert!(anim.resolved_target().is_none());
    }

    #[test]
    fn release_while_finished_does_nothing() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.8);
        anim.release(200.0);
        settle(&mut anim);
        assert!(anim.is_finished());
        anim.release(500.0);
        assert!(anim.is_finished()); // still finished
    }

    #[test]
    fn set_progress_after_finished_restarts() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.8);
        anim.release(200.0);
        settle(&mut anim);
        assert!(anim.is_finished());
        anim.set_progress(0.2);
        assert_eq!(anim.phase(), GesturePhase::Tracking);
    }

    #[test]
    fn gesture_target_dismiss_settles_to_one() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.5);
        anim.release_to(GestureTarget::Dismiss);
        settle(&mut anim);
        assert!((anim.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn gesture_target_cancel_settles_to_zero() {
        let mut anim = GestureAnimation::with_defaults();
        anim.set_progress(0.5);
        anim.release_to(GestureTarget::Cancel);
        settle(&mut anim);
        assert!((anim.progress() - 0.0).abs() < 0.01);
    }
}
