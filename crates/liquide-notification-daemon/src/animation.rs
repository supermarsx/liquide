//! Notification in/out animation state.
//!
//! The notification daemon owns no rendering state directly; instead it
//! exposes a per-notification animation phase (Entering / Visible /
//! Exiting) plus eased progress in `[0.0, 1.0]` so the compositor can drive
//! fade/slide transitions from its frame callback.
//!
//! Typical flow, driven by a `tick(now_ms)` every frame:
//!
//! ```text
//!   new notification → phase = Entering, progress 0 → 1 over fade_in_ms
//!   progress hits 1  → phase = Visible  (progress stays at 1)
//!   close requested  → phase = Exiting, progress 1 → 0 over fade_out_ms
//!   progress hits 0  → phase = Done     (compositor may drop the node)
//! ```

use serde::{Deserialize, Serialize};

/// Animation lifecycle phase for a single notification card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationPhase {
    /// Fading / sliding in.
    Entering,
    /// Fully visible — no animation in progress.
    Visible,
    /// Fading / sliding out.
    Exiting,
    /// Exit complete — caller may drop the notification surface.
    Done,
}

/// Per-notification animation state.
#[derive(Debug, Clone)]
pub struct NotificationAnimationState {
    /// Current phase.
    phase: AnimationPhase,
    /// Milliseconds spent in the current phase.
    elapsed_ms: u32,
    /// Fade-in duration.
    fade_in_ms: u32,
    /// Fade-out duration.
    fade_out_ms: u32,
}

impl NotificationAnimationState {
    /// Create a new state in the `Entering` phase with the given durations.
    #[must_use]
    pub fn new(fade_in_ms: u32, fade_out_ms: u32) -> Self {
        Self {
            phase: AnimationPhase::Entering,
            elapsed_ms: 0,
            fade_in_ms,
            fade_out_ms,
        }
    }

    /// Create state with default durations (180 ms in, 220 ms out).
    #[must_use]
    pub fn default_durations() -> Self {
        Self::new(180, 220)
    }

    /// Current phase.
    #[must_use]
    pub fn phase(&self) -> AnimationPhase {
        self.phase
    }

    /// Linear progress in `[0.0, 1.0]` for the current animation phase.
    ///
    /// * `Entering`: 0 → 1 over `fade_in_ms`.
    /// * `Visible`: always `1.0`.
    /// * `Exiting`: 1 → 0 over `fade_out_ms`.
    /// * `Done`: always `0.0`.
    #[must_use]
    pub fn progress(&self) -> f32 {
        match self.phase {
            AnimationPhase::Entering => {
                if self.fade_in_ms == 0 {
                    1.0
                } else {
                    (self.elapsed_ms as f32 / self.fade_in_ms as f32).clamp(0.0, 1.0)
                }
            }
            AnimationPhase::Visible => 1.0,
            AnimationPhase::Exiting => {
                if self.fade_out_ms == 0 {
                    0.0
                } else {
                    let t = (self.elapsed_ms as f32 / self.fade_out_ms as f32).clamp(0.0, 1.0);
                    1.0 - t
                }
            }
            AnimationPhase::Done => 0.0,
        }
    }

    /// Eased opacity — cubic ease-in-out applied to [`Self::progress`].
    #[must_use]
    pub fn opacity(&self) -> f32 {
        let t = self.progress();
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            let f = 2.0 * t - 2.0;
            1.0 + f * f * f / 2.0
        }
    }

    /// Advance the animation by `delta_ms` milliseconds.
    ///
    /// Returns `true` when the phase transitioned this tick (so the
    /// compositor can schedule a redraw).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let before = self.phase;
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        match self.phase {
            AnimationPhase::Entering => {
                if self.elapsed_ms >= self.fade_in_ms {
                    self.phase = AnimationPhase::Visible;
                    self.elapsed_ms = 0;
                }
            }
            AnimationPhase::Exiting => {
                if self.elapsed_ms >= self.fade_out_ms {
                    self.phase = AnimationPhase::Done;
                    self.elapsed_ms = 0;
                }
            }
            AnimationPhase::Visible | AnimationPhase::Done => {}
        }
        before != self.phase
    }

    /// Begin the exit animation. No-op if already exiting or done.
    pub fn begin_exit(&mut self) {
        if matches!(
            self.phase,
            AnimationPhase::Entering | AnimationPhase::Visible
        ) {
            self.phase = AnimationPhase::Exiting;
            self.elapsed_ms = 0;
        }
    }

    /// Whether the state has reached `Done` and can be dropped.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.phase == AnimationPhase::Done
    }
}

impl Default for NotificationAnimationState {
    fn default() -> Self {
        Self::default_durations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entering_then_visible() {
        let mut s = NotificationAnimationState::new(100, 100);
        assert_eq!(s.phase(), AnimationPhase::Entering);
        assert!(s.progress() < 0.01);
        assert!(s.tick(50) == false);
        assert!((s.progress() - 0.5).abs() < 0.01);
        assert!(s.tick(50)); // phase transition on completion
        assert_eq!(s.phase(), AnimationPhase::Visible);
        assert_eq!(s.progress(), 1.0);
    }

    #[test]
    fn exit_animation_reaches_done() {
        let mut s = NotificationAnimationState::new(0, 100);
        s.tick(0);
        s.begin_exit();
        assert_eq!(s.phase(), AnimationPhase::Exiting);
        assert_eq!(s.progress(), 1.0);
        s.tick(100);
        assert_eq!(s.phase(), AnimationPhase::Done);
        assert!(s.is_done());
    }

    #[test]
    fn zero_duration_completes_immediately() {
        let s = NotificationAnimationState::new(0, 0);
        assert_eq!(s.progress(), 1.0);
    }

    #[test]
    fn opacity_is_clamped() {
        let s = NotificationAnimationState::new(100, 100);
        let o = s.opacity();
        assert!((0.0..=1.0).contains(&o));
    }
}
