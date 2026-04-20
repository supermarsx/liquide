//! Easing functions for **window management effects** (snap, minimize, maximize, etc.).
//!
//! These intentionally use simpler polynomial and bounce curves rather than full
//! cubic-bezier solving, giving a snappier, more responsive feel for window
//! management operations where sub-millisecond easing accuracy is not required.
//!
//! For CSS-standard easing, use `liquide_animation::easing::EasingFunction`.

/// Easing function type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInBack,
    EaseOutBack,
    EaseOutBounce,
    Spring,
}

impl EasingFunction {
    /// Evaluate the easing function. t is 0.0..=1.0, returns 0.0..=1.0
    pub fn eval(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 { 2.0 * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseInOutCubic => {
                if t < 0.5 { 4.0 * t * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            }
            Self::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Self::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Self::EaseOutBounce => ease_out_bounce(t),
            Self::Spring => {
                if t >= 1.0 {
                    return 1.0;
                }
                // Damped spring
                let freq = 4.5;
                let decay = 5.0;
                1.0 - ((-decay * t).exp() * (freq * std::f32::consts::TAU * t).cos())
            }
        }
    }
}

fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}
