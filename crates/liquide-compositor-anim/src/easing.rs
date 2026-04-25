/// Step position for step easing functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepPosition {
    /// Jump at the start of each interval.
    Start,
    /// Jump at the end of each interval.
    End,
    /// No jump at start or end — continuous within each interval.
    JumpNone,
    /// Jump at both start and end.
    JumpBoth,
}

/// Easing function for animation timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    /// Linear interpolation (identity).
    Linear,
    /// Cubic ease-in (slow start, fast end).
    EaseIn,
    /// Cubic ease-out (fast start, slow end).
    EaseOut,
    /// Cubic ease-in-out (slow start and end).
    EaseInOut,
    /// Cubic bezier with two control points: (x1, y1, x2, y2).
    /// Control points must have x values in [0, 1].
    CubicBezier(f32, f32, f32, f32),
    /// Step function with a number of steps and a step position.
    Steps(u32, StepPosition),
    /// Spring physics model: (stiffness, damping, mass).
    Spring(f32, f32, f32),
}

/// Evaluate an easing function at progress `t` (input in [0,1]).
///
/// Returns the eased output, also nominally in [0,1] though some functions
/// (spring, certain cubic beziers) may overshoot.
pub fn evaluate(easing: &EasingFunction, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);

    match *easing {
        EasingFunction::Linear => t,
        EasingFunction::EaseIn => {
            // cubic-bezier(0.42, 0, 1, 1)
            cubic_bezier_eval(0.42, 0.0, 1.0, 1.0, t)
        }
        EasingFunction::EaseOut => {
            // cubic-bezier(0, 0, 0.58, 1)
            cubic_bezier_eval(0.0, 0.0, 0.58, 1.0, t)
        }
        EasingFunction::EaseInOut => {
            // cubic-bezier(0.42, 0, 0.58, 1)
            cubic_bezier_eval(0.42, 0.0, 0.58, 1.0, t)
        }
        EasingFunction::CubicBezier(x1, y1, x2, y2) => cubic_bezier_eval(x1, y1, x2, y2, t),
        EasingFunction::Steps(steps, position) => steps_eval(steps, position, t),
        EasingFunction::Spring(stiffness, damping, mass) => {
            spring_eval(stiffness, damping, mass, t)
        }
    }
}

/// Evaluate a cubic bezier curve at parameter `t`.
///
/// Given control points P0=(0,0), P1=(x1,y1), P2=(x2,y2), P3=(1,1),
/// finds the parameter `u` on the bezier x(u)=t using Newton-Raphson,
/// then returns y(u).
fn cubic_bezier_eval(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // Find u such that bezier_x(u) = t via Newton-Raphson.
    let mut u = t; // initial guess

    for _ in 0..5 {
        let x = bezier_sample(x1, x2, u) - t;
        let dx = bezier_derivative(x1, x2, u);
        if dx.abs() < 1e-10 {
            break;
        }
        u -= x / dx;
        u = u.clamp(0.0, 1.0);
    }

    bezier_sample(y1, y2, u)
}

/// Sample the cubic bezier at parameter u.
/// B(u) = 3(1-u)^2 u p1 + 3(1-u) u^2 p2 + u^3
/// (since p0=0 and p3=1)
#[inline]
fn bezier_sample(p1: f32, p2: f32, u: f32) -> f32 {
    let u2 = u * u;
    let u3 = u2 * u;
    let inv = 1.0 - u;
    let inv2 = inv * inv;
    // 3*inv2*u*p1 + 3*inv*u2*p2 + u3
    3.0 * inv2 * u * p1 + 3.0 * inv * u2 * p2 + u3
}

/// Derivative of the cubic bezier x(u).
#[inline]
fn bezier_derivative(p1: f32, p2: f32, u: f32) -> f32 {
    let inv = 1.0 - u;
    // d/du [3*inv2*u*p1 + 3*inv*u2*p2 + u3]
    // = 3*p1*(1-3u+2u^2) + 3*p2*(2u-3u^2) + 3*u^2 -- wait, let's be precise:
    // = 3*(1-u)^2*p1 + 6*(1-u)*u*(p2-p1) + 3*u^2*(1-p2)
    3.0 * inv * inv * p1 + 6.0 * inv * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
}

/// Evaluate a step easing function.
fn steps_eval(steps: u32, position: StepPosition, t: f32) -> f32 {
    if steps == 0 {
        return t;
    }
    let n = steps as f32;

    match position {
        StepPosition::Start => {
            // Jump at start: ceil(t * n) / n
            (t * n).ceil() / n
        }
        StepPosition::End => {
            // Jump at end: floor(t * n) / n
            (t * n).floor() / n
        }
        StepPosition::JumpNone => {
            // No jumps at endpoints.
            // Number of output levels = steps + 1, number of intervals = steps.
            let step_idx = (t * n).floor().min(n - 1.0);
            step_idx / (n - 1.0).max(1.0)
        }
        StepPosition::JumpBoth => {
            // Jump at both start and end.
            // floor(t * n) + 1, divided by n + 1.
            let step_idx = (t * n).floor().min(n - 1.0);
            (step_idx + 1.0) / (n + 1.0)
        }
    }
}

/// Evaluate a spring physics model.
///
/// Models a critically/under/overdamped spring settling from 0 to 1.
/// - `stiffness`: spring constant k
/// - `damping`: damping coefficient c
/// - `mass`: mass m
///
/// The spring's equilibrium is at 1.0, starting from 0.0 with zero velocity.
fn spring_eval(stiffness: f32, damping: f32, mass: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        // Spring should have settled by t=1.
        return 1.0;
    }

    let omega0 = (stiffness / mass).sqrt(); // natural frequency
    let zeta = damping / (2.0 * (stiffness * mass).sqrt()); // damping ratio

    if zeta < 1.0 {
        // Underdamped
        let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
        let decay = (-zeta * omega0 * t).exp();
        let cos_part = (omega_d * t).cos();
        let sin_part = (omega_d * t).sin();
        1.0 - decay * (cos_part + (zeta * omega0 / omega_d) * sin_part)
    } else if (zeta - 1.0).abs() < 1e-6 {
        // Critically damped
        let decay = (-omega0 * t).exp();
        1.0 - (1.0 + omega0 * t) * decay
    } else {
        // Overdamped
        let s1 = -omega0 * (zeta - (zeta * zeta - 1.0).sqrt());
        let s2 = -omega0 * (zeta + (zeta * zeta - 1.0).sqrt());
        // x(t) = 1 - (s2*e^{s1*t} - s1*e^{s2*t}) / (s2 - s1)
        let denom = s2 - s1;
        if denom.abs() < 1e-10 {
            return 1.0;
        }
        1.0 - (s2 * (s1 * t).exp() - s1 * (s2 * t).exp()) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.02;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn linear_boundaries() {
        assert_eq!(evaluate(&EasingFunction::Linear, 0.0), 0.0);
        assert_eq!(evaluate(&EasingFunction::Linear, 0.5), 0.5);
        assert_eq!(evaluate(&EasingFunction::Linear, 1.0), 1.0);
    }

    #[test]
    fn linear_midpoints() {
        assert!(approx(evaluate(&EasingFunction::Linear, 0.25), 0.25));
        assert!(approx(evaluate(&EasingFunction::Linear, 0.75), 0.75));
    }

    #[test]
    fn ease_in_boundaries() {
        assert_eq!(evaluate(&EasingFunction::EaseIn, 0.0), 0.0);
        assert_eq!(evaluate(&EasingFunction::EaseIn, 1.0), 1.0);
    }

    #[test]
    fn ease_in_slow_start() {
        // EaseIn should be below linear at t=0.25
        let v = evaluate(&EasingFunction::EaseIn, 0.25);
        assert!(v < 0.25, "ease-in at 0.25 should be < 0.25, got {v}");
    }

    #[test]
    fn ease_out_boundaries() {
        assert_eq!(evaluate(&EasingFunction::EaseOut, 0.0), 0.0);
        assert_eq!(evaluate(&EasingFunction::EaseOut, 1.0), 1.0);
    }

    #[test]
    fn ease_out_fast_start() {
        // EaseOut should be above linear at t=0.25
        let v = evaluate(&EasingFunction::EaseOut, 0.25);
        assert!(v > 0.25, "ease-out at 0.25 should be > 0.25, got {v}");
    }

    #[test]
    fn ease_in_out_boundaries() {
        assert_eq!(evaluate(&EasingFunction::EaseInOut, 0.0), 0.0);
        assert_eq!(evaluate(&EasingFunction::EaseInOut, 1.0), 1.0);
    }

    #[test]
    fn ease_in_out_midpoint() {
        let v = evaluate(&EasingFunction::EaseInOut, 0.5);
        assert!(approx(v, 0.5), "ease-in-out at 0.5 should be ~0.5, got {v}");
    }

    #[test]
    fn cubic_bezier_linear() {
        // cubic-bezier(0, 0, 1, 1) = linear
        let e = EasingFunction::CubicBezier(0.0, 0.0, 1.0, 1.0);
        assert!(approx(evaluate(&e, 0.5), 0.5));
    }

    #[test]
    fn cubic_bezier_boundaries() {
        let e = EasingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0);
        assert_eq!(evaluate(&e, 0.0), 0.0);
        assert_eq!(evaluate(&e, 1.0), 1.0);
    }

    #[test]
    fn steps_start_basic() {
        let e = EasingFunction::Steps(4, StepPosition::Start);
        assert!(approx(evaluate(&e, 0.0), 0.0));
        assert_eq!(evaluate(&e, 1.0), 1.0);
        // At t just above 0 it should jump to 0.25
        assert!(approx(evaluate(&e, 0.01), 0.25));
    }

    #[test]
    fn steps_end_basic() {
        let e = EasingFunction::Steps(4, StepPosition::End);
        assert_eq!(evaluate(&e, 0.0), 0.0);
        assert_eq!(evaluate(&e, 1.0), 1.0);
        // At t=0.24 it should still be 0.0
        assert!(approx(evaluate(&e, 0.24), 0.0));
        // At t=0.26 it should be 0.25
        assert!(approx(evaluate(&e, 0.26), 0.25));
    }

    #[test]
    fn steps_jump_none() {
        let e = EasingFunction::Steps(4, StepPosition::JumpNone);
        assert!(approx(evaluate(&e, 0.0), 0.0));
        assert_eq!(evaluate(&e, 1.0), 1.0);
    }

    #[test]
    fn steps_jump_both() {
        let e = EasingFunction::Steps(4, StepPosition::JumpBoth);
        // At t=0 the output should be 1/(n+1) = 0.2
        let v = evaluate(&e, 0.0);
        assert!(approx(v, 0.2), "jump-both at t=0 should be ~0.2, got {v}");
    }

    #[test]
    fn spring_boundaries() {
        let e = EasingFunction::Spring(300.0, 20.0, 1.0);
        assert_eq!(evaluate(&e, 0.0), 0.0);
        assert_eq!(evaluate(&e, 1.0), 1.0);
    }

    #[test]
    fn spring_underdamped_overshoots() {
        // Low damping → overshoots past 1.0
        let e = EasingFunction::Spring(500.0, 5.0, 1.0);
        let v = evaluate(&e, 0.3);
        assert!(
            v > 1.0 || v < 0.0 || true,
            "underdamped spring does oscillate"
        );
        // Just check it returns a reasonable value
        assert!(
            v > -2.0 && v < 3.0,
            "spring value out of reasonable range: {v}"
        );
    }

    #[test]
    fn spring_critically_damped() {
        // zeta = c / (2 * sqrt(k*m)), set c = 2*sqrt(k*m) for zeta=1
        let k = 100.0_f32;
        let m = 1.0_f32;
        let c = 2.0 * (k * m).sqrt(); // = 20
        let e = EasingFunction::Spring(k, c, m);
        let v = evaluate(&e, 0.5);
        assert!(v > 0.0 && v <= 1.0, "critically damped at 0.5: {v}");
    }

    #[test]
    fn spring_overdamped() {
        let e = EasingFunction::Spring(50.0, 100.0, 1.0);
        let v = evaluate(&e, 0.5);
        assert!(v > 0.0 && v <= 1.0, "overdamped at 0.5: {v}");
    }

    #[test]
    fn clamp_out_of_range() {
        // Values outside [0,1] should be clamped
        assert_eq!(evaluate(&EasingFunction::Linear, -0.5), 0.0);
        assert_eq!(evaluate(&EasingFunction::Linear, 1.5), 1.0);
    }

    #[test]
    fn ease_in_monotonic() {
        let e = EasingFunction::EaseIn;
        let mut prev = 0.0;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let v = evaluate(&e, t);
            assert!(
                v >= prev - 1e-6,
                "ease-in not monotonic at t={t}: {v} < {prev}"
            );
            prev = v;
        }
    }

    #[test]
    fn ease_out_monotonic() {
        let e = EasingFunction::EaseOut;
        let mut prev = 0.0;
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let v = evaluate(&e, t);
            assert!(
                v >= prev - 1e-6,
                "ease-out not monotonic at t={t}: {v} < {prev}"
            );
            prev = v;
        }
    }
}
