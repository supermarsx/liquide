//! Easing / timing functions (CSS `animation-timing-function`).
//!
//! Implements all standard CSS easing keywords plus arbitrary cubic-bezier.

use serde::{Deserialize, Serialize};

/// A cubic-bezier curve defined by two control points (P1, P2).
///
/// P0 = (0, 0), P3 = (1, 1) are implicit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Evaluate the bezier at parameter `t` on the X axis.
    fn sample_x(&self, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        3.0 * (1.0 - t) * (1.0 - t) * t * self.x1 + 3.0 * (1.0 - t) * t2 * self.x2 + t3
    }

    /// Evaluate the bezier at parameter `t` on the Y axis.
    fn sample_y(&self, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        3.0 * (1.0 - t) * (1.0 - t) * t * self.y1 + 3.0 * (1.0 - t) * t2 * self.y2 + t3
    }

    /// Solve for the parameter `t` that produces a given `x` value
    /// using Newton's method with bisection fallback.
    fn solve_t_for_x(&self, x: f32) -> f32 {
        // Newton's method
        let mut t = x;
        for _ in 0..8 {
            let x_est = self.sample_x(t) - x;
            let dx = 3.0 * (1.0 - t) * (1.0 - t) * self.x1
                + 6.0 * (1.0 - t) * t * (self.x2 - self.x1)
                + 3.0 * t * t * (1.0 - self.x2);
            if dx.abs() < 1e-7 {
                break;
            }
            t -= x_est / dx;
            t = t.clamp(0.0, 1.0);
        }

        // Bisection fallback if Newton diverged
        if (self.sample_x(t) - x).abs() > 1e-4 {
            let mut lo = 0.0_f32;
            let mut hi = 1.0_f32;
            t = x;
            for _ in 0..20 {
                let x_est = self.sample_x(t);
                if (x_est - x).abs() < 1e-6 {
                    break;
                }
                if x_est < x {
                    lo = t;
                } else {
                    hi = t;
                }
                t = (lo + hi) / 2.0;
            }
        }

        t
    }

    /// Evaluate: given a progress `x` ∈ [0, 1], return eased output `y` ∈ [0, 1].
    pub fn evaluate(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let t = self.solve_t_for_x(x);
        self.sample_y(t)
    }
}

/// A CSS easing / timing function.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingFunction {
    /// `linear`
    Linear,
    /// `ease`
    Ease,
    /// `ease-in`
    EaseIn,
    /// `ease-out`
    EaseOut,
    /// `ease-in-out`
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`
    CubicBezier(CubicBezier),
    /// `steps(n, <position>)`
    Steps { count: u32, jump_start: bool },
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Ease
    }
}

impl EasingFunction {
    /// Evaluate the easing at progress `t` ∈ [0, 1].
    #[must_use]
    pub fn evaluate(&self, t: f32) -> f32 {
        match self {
            EasingFunction::Linear => t,
            EasingFunction::Ease => CubicBezier::new(0.25, 0.1, 0.25, 1.0).evaluate(t),
            EasingFunction::EaseIn => CubicBezier::new(0.42, 0.0, 1.0, 1.0).evaluate(t),
            EasingFunction::EaseOut => CubicBezier::new(0.0, 0.0, 0.58, 1.0).evaluate(t),
            EasingFunction::EaseInOut => CubicBezier::new(0.42, 0.0, 0.58, 1.0).evaluate(t),
            EasingFunction::CubicBezier(cb) => cb.evaluate(t),
            EasingFunction::Steps { count, jump_start } => {
                let n = *count as f32;
                if *jump_start {
                    ((t * n).ceil() / n).clamp(0.0, 1.0)
                } else {
                    ((t * n).floor() / n).clamp(0.0, 1.0)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_identity() {
        let e = EasingFunction::Linear;
        assert_eq!(e.evaluate(0.0), 0.0);
        assert_eq!(e.evaluate(0.5), 0.5);
        assert_eq!(e.evaluate(1.0), 1.0);
    }

    #[test]
    fn ease_endpoints() {
        let e = EasingFunction::Ease;
        assert!((e.evaluate(0.0) - 0.0).abs() < 1e-3);
        assert!((e.evaluate(1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn steps_jump_end() {
        let e = EasingFunction::Steps {
            count: 4,
            jump_start: false,
        };
        assert_eq!(e.evaluate(0.0), 0.0);
        assert!((e.evaluate(0.3) - 0.25).abs() < 1e-5);
        assert_eq!(e.evaluate(1.0), 1.0);
    }
}
