//! Reduced-motion animation overrides.
//!
//! When the user has indicated a preference for reduced motion
//! (`prefers-reduced-motion: reduce`), these overrides tell the
//! animation and transition systems what to disable or cap.

/// Overrides for animation and transition systems when the user
/// prefers reduced motion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationOverrides {
    /// Disable CSS-style property transitions entirely.
    pub disable_transitions: bool,
    /// Disable window open/close/minimize animations.
    pub disable_window_animations: bool,
    /// Maximum duration (in ms) for any animation that is still
    /// allowed. Animations longer than this are clamped.
    /// A value of `0` means no cap (only relevant when other
    /// flags are `false`).
    pub max_duration_ms: u32,
    /// Disable parallax scrolling effects.
    pub disable_parallax: bool,
    /// Disable animated blur transitions (e.g. glass effect fade-in).
    pub disable_blur_animation: bool,
}

impl Default for AnimationOverrides {
    /// Default: all animations enabled, no caps.
    fn default() -> Self {
        Self {
            disable_transitions: false,
            disable_window_animations: false,
            max_duration_ms: 0,
            disable_parallax: false,
            disable_blur_animation: false,
        }
    }
}

impl AnimationOverrides {
    /// Returns `true` if any animation restrictions are active.
    #[must_use]
    pub fn has_restrictions(&self) -> bool {
        self.disable_transitions
            || self.disable_window_animations
            || self.max_duration_ms > 0
            || self.disable_parallax
            || self.disable_blur_animation
    }

    /// Clamp a proposed animation duration (in ms) to the maximum
    /// allowed by these overrides.
    ///
    /// Returns `0` if transitions are fully disabled.
    #[must_use]
    pub fn clamp_duration(&self, duration_ms: u32) -> u32 {
        if self.disable_transitions {
            return 0;
        }
        if self.max_duration_ms > 0 && duration_ms > self.max_duration_ms {
            return self.max_duration_ms;
        }
        duration_ms
    }

    /// Returns `true` if a window animation (open, close, minimize,
    /// maximize) should be skipped.
    #[must_use]
    pub fn should_skip_window_animation(&self) -> bool {
        self.disable_window_animations
    }
}

/// Conservative reduced-motion overrides that disable most motion.
///
/// This is the recommended set when `prefers-reduced-motion: reduce`
/// is active. Transitions are disabled entirely, window animations
/// are off, and any remaining motion is capped to 200ms.
#[must_use]
pub fn reduced_motion_overrides() -> AnimationOverrides {
    AnimationOverrides {
        disable_transitions: true,
        disable_window_animations: true,
        max_duration_ms: 200,
        disable_parallax: true,
        disable_blur_animation: true,
    }
}

/// Minimal-motion overrides that only keep essential feedback.
///
/// Focus indicators, button press feedback, and loading spinners
/// still animate (up to 150ms), but everything else is instant.
/// This is stricter than [`reduced_motion_overrides`] and suitable
/// for users who experience vestibular discomfort.
#[must_use]
pub fn essential_motion_only() -> AnimationOverrides {
    AnimationOverrides {
        disable_transitions: true,
        disable_window_animations: true,
        max_duration_ms: 150,
        disable_parallax: true,
        disable_blur_animation: true,
    }
}
