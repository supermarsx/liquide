//! Spring physics animations based on damped harmonic oscillator model.
//!
//! Implements UIKit-style spring dynamics for natural-feeling animations.
//! The spring simulates a mass-spring-damper system where the equilibrium
//! position is the target value.
//!
//! # Physics Model
//!
//! The equation of motion is: `m * x'' + c * x' + k * x = 0`
//! where `m` = mass, `c` = damping, `k` = stiffness.
//!
//! Three regimes exist based on the damping ratio (zeta = c / (2 * sqrt(k*m))):
//! - **Underdamped** (zeta < 1): oscillates around equilibrium
//! - **Critically damped** (zeta = 1): fastest non-oscillating settle
//! - **Overdamped** (zeta > 1): slow return without oscillation

/// Configuration parameters for a spring animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Spring constant (k). Higher = stiffer/faster. Typical range: 100-1000.
    pub stiffness: f64,
    /// Damping coefficient (c). Higher = less oscillation. Typical range: 10-100.
    pub damping: f64,
    /// Mass of the object. Higher = more inertia. Typical range: 0.5-5.0.
    pub mass: f64,
    /// Initial velocity (displacement units per second).
    pub initial_velocity: f64,
}

impl SpringConfig {
    /// Create a new spring configuration.
    pub fn new(stiffness: f64, damping: f64, mass: f64) -> Self {
        Self {
            stiffness,
            damping,
            mass,
            initial_velocity: 0.0,
        }
    }

    /// A bouncy spring with visible overshoot. Suitable for playful UI
    /// interactions like "pop-in" or toggle animations.
    ///
    /// Parameters: stiffness=300, damping=10, mass=1.0
    pub fn bouncy() -> Self {
        Self::new(300.0, 10.0, 1.0)
    }

    /// A stiff, responsive spring with minimal overshoot. Suitable for
    /// snapping to positions, tab transitions, or toolbar animations.
    ///
    /// Parameters: stiffness=500, damping=30, mass=0.8
    pub fn stiff() -> Self {
        Self::new(500.0, 30.0, 0.8)
    }

    /// A gentle, slow spring with smooth settling. Suitable for page
    /// transitions, scale animations, or ambient effects.
    ///
    /// Parameters: stiffness=120, damping=14, mass=1.0
    pub fn gentle() -> Self {
        Self::new(120.0, 14.0, 1.0)
    }

    /// Create a spring that is critically damped for the given stiffness
    /// and mass. This is the fastest configuration that does not overshoot.
    pub fn critical(stiffness: f64, mass: f64) -> Self {
        let damping = critically_damped(stiffness, mass);
        Self::new(stiffness, damping, mass)
    }

    /// Set the initial velocity and return `self` (builder pattern).
    pub fn with_velocity(mut self, velocity: f64) -> Self {
        self.initial_velocity = velocity;
        self
    }

    /// Compute the damping ratio (zeta).
    ///
    /// - zeta < 1.0 = underdamped (oscillates)
    /// - zeta = 1.0 = critically damped (fastest without oscillation)
    /// - zeta > 1.0 = overdamped (slow, no oscillation)
    pub fn damping_ratio(&self) -> f64 {
        if self.stiffness <= 0.0 || self.mass <= 0.0 {
            return 0.0;
        }
        self.damping / (2.0 * (self.stiffness * self.mass).sqrt())
    }

    /// Natural frequency (omega_0 = sqrt(k/m)).
    pub fn natural_frequency(&self) -> f64 {
        if self.mass <= 0.0 || self.stiffness <= 0.0 {
            return 0.0;
        }
        (self.stiffness / self.mass).sqrt()
    }
}

/// Compute the critical damping coefficient for given stiffness and mass.
///
/// Critical damping is the threshold between oscillatory (underdamped) and
/// non-oscillatory (overdamped) behavior: `c_crit = 2 * sqrt(k * m)`.
pub fn critically_damped(stiffness: f64, mass: f64) -> f64 {
    2.0 * (stiffness * mass).sqrt()
}

/// Compute the oscillation period for an underdamped spring.
///
/// Returns the period `T = 2 * pi / omega_d` where `omega_d` is the damped
/// natural frequency. Returns `f64::INFINITY` if the spring is critically
/// damped or overdamped (no oscillation).
pub fn underdamped_period(config: &SpringConfig) -> f64 {
    let zeta = config.damping_ratio();
    if zeta >= 1.0 {
        return f64::INFINITY;
    }
    let omega0 = config.natural_frequency();
    if omega0 <= 0.0 {
        return f64::INFINITY;
    }
    let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
    if omega_d <= 0.0 {
        return f64::INFINITY;
    }
    2.0 * std::f64::consts::PI / omega_d
}

/// A spring-driven animation that simulates a damped harmonic oscillator.
///
/// The spring animates from a start value toward a target value, with the
/// motion governed by the configured stiffness, damping, and mass. Unlike
/// duration-based easing, springs feel physically natural and respond well
/// to interruption (retargeting mid-flight preserves momentum).
#[derive(Debug, Clone)]
pub struct SpringAnimation {
    /// Spring configuration.
    pub config: SpringConfig,
    /// Start value (equilibrium offset origin).
    pub from: f64,
    /// Target value (equilibrium position).
    pub target: f64,
    /// Current displacement from target.
    position: f64,
    /// Current velocity (displacement units per second).
    velocity: f64,
    /// Total elapsed time in seconds.
    elapsed: f64,
}

impl SpringAnimation {
    /// Create a new spring animation from `from` to `target`.
    pub fn new(config: SpringConfig, from: f64, target: f64) -> Self {
        Self {
            position: from - target, // displacement from equilibrium
            velocity: config.initial_velocity,
            from,
            target,
            config,
            elapsed: 0.0,
        }
    }

    /// Advance the simulation by `dt` seconds and return the current value.
    ///
    /// Uses semi-implicit Euler integration for stability. For typical frame
    /// rates (60-120 fps), `dt` is ~0.008-0.016 seconds.
    pub fn tick(&mut self, dt: f64) -> f64 {
        if dt <= 0.0 {
            return self.current_value();
        }

        self.elapsed += dt;

        let k = self.config.stiffness;
        let c = self.config.damping;
        let m = self.config.mass;

        if m <= 0.0 {
            // Degenerate: no mass means instant snap.
            self.position = 0.0;
            self.velocity = 0.0;
            return self.target;
        }

        // Semi-implicit Euler: update velocity first, then position.
        // F = -k*x - c*v
        let spring_force = -k * self.position;
        let damping_force = -c * self.velocity;
        let acceleration = (spring_force + damping_force) / m;

        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;

        self.current_value()
    }

    /// Get the current animated value without advancing time.
    pub fn current_value(&self) -> f64 {
        self.target + self.position
    }

    /// Get the current velocity.
    pub fn current_velocity(&self) -> f64 {
        self.velocity
    }

    /// Get the total elapsed time in seconds.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Check if the spring has settled within the given threshold.
    ///
    /// A spring is "at rest" when both the displacement from target and
    /// the velocity are below the threshold. A typical threshold for pixel
    /// animations is 0.5 (half a pixel).
    pub fn is_at_rest(&self, threshold: f64) -> bool {
        self.position.abs() < threshold && self.velocity.abs() < threshold
    }

    /// Retarget the spring to a new destination, preserving current momentum.
    ///
    /// This is the key advantage of spring animations over duration-based
    /// animations: mid-flight retargeting feels physically natural because
    /// velocity is preserved.
    pub fn retarget(&mut self, new_target: f64) {
        // Convert current absolute position to displacement from new target.
        let current = self.current_value();
        self.target = new_target;
        self.position = current - new_target;
        // Velocity is preserved — the spring will naturally redirect.
    }

    /// Reset the animation to animate from a new start value.
    pub fn reset(&mut self, from: f64, target: f64) {
        self.from = from;
        self.target = target;
        self.position = from - target;
        self.velocity = self.config.initial_velocity;
        self.elapsed = 0.0;
    }

    /// Compute the normalized progress (0.0 to ~1.0).
    ///
    /// Note: for underdamped springs this can temporarily exceed 1.0 due to
    /// overshoot, and may oscillate around 1.0 before settling.
    pub fn progress(&self) -> f64 {
        let total = self.from - self.target;
        if total.abs() < 1e-12 {
            return 1.0;
        }
        1.0 - self.position / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REST_THRESHOLD: f64 = 0.5;
    const DT: f64 = 1.0 / 60.0; // 60 fps

    fn simulate_to_rest(anim: &mut SpringAnimation) -> u32 {
        let mut frames = 0u32;
        while !anim.is_at_rest(REST_THRESHOLD) && frames < 10000 {
            anim.tick(DT);
            frames += 1;
        }
        frames
    }

    #[test]
    fn spring_config_bouncy() {
        let cfg = SpringConfig::bouncy();
        assert!(cfg.damping_ratio() < 1.0, "bouncy should be underdamped");
    }

    #[test]
    fn spring_config_stiff() {
        let cfg = SpringConfig::stiff();
        assert!(cfg.stiffness > cfg.damping, "stiff should have high stiffness");
    }

    #[test]
    fn spring_config_gentle() {
        let cfg = SpringConfig::gentle();
        assert!(cfg.stiffness < 200.0, "gentle should have low stiffness");
    }

    #[test]
    fn spring_config_critical() {
        let cfg = SpringConfig::critical(400.0, 1.0);
        let zeta = cfg.damping_ratio();
        assert!((zeta - 1.0).abs() < 0.001, "critical config should have zeta~1.0, got {zeta}");
    }

    #[test]
    fn critically_damped_value() {
        let c = critically_damped(400.0, 1.0);
        // c_crit = 2 * sqrt(400 * 1) = 2 * 20 = 40
        assert!((c - 40.0).abs() < 0.001);
    }

    #[test]
    fn underdamped_period_value() {
        let cfg = SpringConfig::new(100.0, 2.0, 1.0); // very underdamped
        let period = underdamped_period(&cfg);
        assert!(period.is_finite());
        assert!(period > 0.0);
        // omega0 = 10, zeta = 2/(2*10) = 0.1, omega_d = 10*sqrt(1-0.01) ~ 9.95
        // T = 2*PI/9.95 ~ 0.631
        assert!((period - 0.631).abs() < 0.01, "period={period}");
    }

    #[test]
    fn overdamped_period_is_infinite() {
        let cfg = SpringConfig::new(100.0, 100.0, 1.0); // overdamped
        assert!(cfg.damping_ratio() > 1.0);
        let period = underdamped_period(&cfg);
        assert!(period.is_infinite());
    }

    #[test]
    fn spring_converges_to_target() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        let frames = simulate_to_rest(&mut anim);
        assert!(frames < 10000, "should converge");
        let val = anim.current_value();
        assert!((val - 100.0).abs() < REST_THRESHOLD, "should be near target: {val}");
    }

    #[test]
    fn bouncy_spring_overshoots() {
        let cfg = SpringConfig::bouncy();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        let mut max_val = 0.0f64;
        for _ in 0..600 {
            let v = anim.tick(DT);
            if v > max_val {
                max_val = v;
            }
        }
        assert!(max_val > 100.0, "bouncy spring should overshoot, max was {max_val}");
    }

    #[test]
    fn critical_spring_no_significant_overshoot() {
        let cfg = SpringConfig::critical(400.0, 1.0);
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        let mut max_val = 0.0f64;
        for _ in 0..600 {
            let v = anim.tick(DT);
            if v > max_val {
                max_val = v;
            }
        }
        // Critically damped may have tiny numerical overshoot due to Euler integration.
        assert!(max_val < 102.0, "critical spring should barely overshoot, max was {max_val}");
    }

    #[test]
    fn spring_initial_value() {
        let cfg = SpringConfig::stiff();
        let anim = SpringAnimation::new(cfg, 50.0, 200.0);
        assert!((anim.current_value() - 50.0).abs() < 0.001);
    }

    #[test]
    fn spring_with_velocity() {
        let cfg = SpringConfig::stiff().with_velocity(500.0);
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        anim.tick(DT);
        // With high initial velocity toward target, should move faster initially.
        let val = anim.current_value();
        assert!(val > 5.0, "high velocity should give fast start: {val}");
    }

    #[test]
    fn spring_retarget_preserves_velocity() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        // Simulate a few frames to build up velocity.
        for _ in 0..10 {
            anim.tick(DT);
        }
        let vel_before = anim.current_velocity();
        let val_before = anim.current_value();

        anim.retarget(200.0);

        assert!((anim.current_value() - val_before).abs() < 0.001,
            "retarget should preserve position");
        assert!((anim.current_velocity() - vel_before).abs() < 0.001,
            "retarget should preserve velocity");
        assert!((anim.target - 200.0).abs() < 0.001);
    }

    #[test]
    fn spring_retarget_converges_to_new_target() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        for _ in 0..30 {
            anim.tick(DT);
        }
        anim.retarget(50.0);
        let frames = simulate_to_rest(&mut anim);
        assert!(frames < 10000);
        assert!((anim.current_value() - 50.0).abs() < REST_THRESHOLD);
    }

    #[test]
    fn spring_reset() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        for _ in 0..30 {
            anim.tick(DT);
        }
        anim.reset(0.0, 200.0);
        assert!((anim.current_value() - 0.0).abs() < 0.001);
        assert!((anim.target - 200.0).abs() < 0.001);
        assert!((anim.elapsed() - 0.0).abs() < 0.001);
    }

    #[test]
    fn spring_progress_starts_at_zero() {
        let cfg = SpringConfig::stiff();
        let anim = SpringAnimation::new(cfg, 0.0, 100.0);
        assert!((anim.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn spring_progress_reaches_one() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        simulate_to_rest(&mut anim);
        assert!((anim.progress() - 1.0).abs() < 0.01, "progress={}", anim.progress());
    }

    #[test]
    fn spring_zero_dt_no_change() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        let v1 = anim.tick(0.0);
        assert!((v1 - 0.0).abs() < 0.001);
    }

    #[test]
    fn spring_zero_mass_snaps() {
        let cfg = SpringConfig::new(300.0, 20.0, 0.0);
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        let v = anim.tick(DT);
        assert!((v - 100.0).abs() < 0.001, "zero mass should snap to target: {v}");
    }

    #[test]
    fn spring_same_from_target() {
        let cfg = SpringConfig::stiff();
        let anim = SpringAnimation::new(cfg, 50.0, 50.0);
        assert!(anim.is_at_rest(0.001));
        assert!((anim.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn damping_ratio_underdamped() {
        let cfg = SpringConfig::bouncy();
        assert!(cfg.damping_ratio() < 1.0);
    }

    #[test]
    fn damping_ratio_overdamped() {
        let cfg = SpringConfig::new(100.0, 100.0, 1.0);
        assert!(cfg.damping_ratio() > 1.0);
    }

    #[test]
    fn natural_frequency() {
        let cfg = SpringConfig::new(400.0, 20.0, 1.0);
        let omega = cfg.natural_frequency();
        assert!((omega - 20.0).abs() < 0.001); // sqrt(400/1) = 20
    }

    #[test]
    fn elapsed_tracks_time() {
        let cfg = SpringConfig::stiff();
        let mut anim = SpringAnimation::new(cfg, 0.0, 100.0);
        anim.tick(0.1);
        anim.tick(0.2);
        assert!((anim.elapsed() - 0.3).abs() < 0.0001);
    }

    #[test]
    fn gentle_spring_slower_than_stiff() {
        let mut gentle_anim = SpringAnimation::new(SpringConfig::gentle(), 0.0, 100.0);
        let mut stiff_anim = SpringAnimation::new(SpringConfig::stiff(), 0.0, 100.0);
        let gentle_frames = simulate_to_rest(&mut gentle_anim);
        let stiff_frames = simulate_to_rest(&mut stiff_anim);
        assert!(gentle_frames > stiff_frames,
            "gentle ({gentle_frames}) should take more frames than stiff ({stiff_frames})");
    }
}
