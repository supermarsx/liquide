//! Kinetic scrolling / flick physics with deceleration and overscroll rubber-band.
//!
//! Based on the momentum scrolling model used by GTK and Mutter: velocity is
//! computed from recent touch samples, then exponential deceleration is applied
//! each tick until the velocity drops below a threshold.

/// Compute flick velocity from a series of recent touch points.
///
/// Each entry is `(x, y, timestamp_us)` where timestamp is monotonic
/// microseconds. Returns `(vx, vy)` in pixels-per-second.
///
/// Uses a weighted least-squares fit over the most recent samples (last 100ms).
pub fn flick_velocity(points: &[(f64, f64, u64)]) -> (f64, f64) {
    if points.len() < 2 {
        return (0.0, 0.0);
    }

    let last_t = points.last().unwrap().2;
    let window_us: u64 = 100_000; // 100ms

    // Filter to recent window
    let recent: Vec<&(f64, f64, u64)> = points
        .iter()
        .filter(|p| last_t - p.2 <= window_us)
        .collect();

    if recent.len() < 2 {
        // Fallback: just use last two points
        let a = &points[points.len() - 2];
        let b = &points[points.len() - 1];
        let dt = (b.2 as f64 - a.2 as f64) / 1_000_000.0;
        if dt <= 0.0 {
            return (0.0, 0.0);
        }
        return ((b.0 - a.0) / dt, (b.1 - a.1) / dt);
    }

    // Weighted average of per-segment velocities (more recent = higher weight)
    let mut vx_sum = 0.0;
    let mut vy_sum = 0.0;
    let mut weight_sum = 0.0;

    for i in 1..recent.len() {
        let prev = recent[i - 1];
        let cur = recent[i];
        let dt = (cur.2 as f64 - prev.2 as f64) / 1_000_000.0;
        if dt <= 0.0 {
            continue;
        }
        let seg_vx = (cur.0 - prev.0) / dt;
        let seg_vy = (cur.1 - prev.1) / dt;
        // Weight: linear ramp (1 for oldest, N for newest)
        let w = i as f64;
        vx_sum += seg_vx * w;
        vy_sum += seg_vy * w;
        weight_sum += w;
    }

    if weight_sum <= 0.0 {
        return (0.0, 0.0);
    }

    (vx_sum / weight_sum, vy_sum / weight_sum)
}

/// Kinetic deceleration configuration.
#[derive(Debug, Clone)]
pub struct KineticConfig {
    /// Friction coefficient per second (velocity multiplied by `1 - friction * dt`).
    /// Values in range [0, 1]. Higher = more friction = faster stop.
    pub friction: f64,
    /// Velocity magnitude (px/s) below which kinetic scrolling stops.
    pub min_velocity: f64,
    /// Maximum velocity (px/s) to clamp at.
    pub max_velocity: f64,
}

impl Default for KineticConfig {
    fn default() -> Self {
        Self {
            friction: 5.0,
            min_velocity: 1.0,
            max_velocity: 8000.0,
        }
    }
}

/// State for kinetic (momentum) scrolling.
pub struct KineticState {
    vx: f64,
    vy: f64,
    active: bool,
    config: KineticConfig,
}

impl KineticState {
    pub fn new(config: KineticConfig) -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            active: false,
            config,
        }
    }

    /// Start kinetic scrolling with the given initial velocity (px/s).
    pub fn start(&mut self, vx: f64, vy: f64) {
        let mag = (vx * vx + vy * vy).sqrt();
        if mag < self.config.min_velocity {
            self.active = false;
            return;
        }
        // Clamp to max velocity
        if mag > self.config.max_velocity {
            let ratio = self.config.max_velocity / mag;
            self.vx = vx * ratio;
            self.vy = vy * ratio;
        } else {
            self.vx = vx;
            self.vy = vy;
        }
        self.active = true;
    }

    /// Advance by `dt` seconds. Returns the scroll delta `(dx, dy)` for this tick.
    pub fn tick(&mut self, dt: f64) -> (f64, f64) {
        if !self.active || dt <= 0.0 {
            return (0.0, 0.0);
        }

        // Exponential decay: v *= e^(-friction * dt)
        let decay = (-self.config.friction * dt).exp();
        let new_vx = self.vx * decay;
        let new_vy = self.vy * decay;

        // Scroll delta is integral of velocity over [0, dt]:
        // integral v0 * e^(-f*t) dt from 0 to dt = v0 / f * (1 - e^(-f*dt))
        let f = self.config.friction;
        let (dx, dy) = if f.abs() > f64::EPSILON {
            let factor = (1.0 - decay) / f;
            (self.vx * factor, self.vy * factor)
        } else {
            (self.vx * dt, self.vy * dt)
        };

        self.vx = new_vx;
        self.vy = new_vy;

        let mag = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if mag < self.config.min_velocity {
            self.active = false;
            self.vx = 0.0;
            self.vy = 0.0;
        }

        (dx, dy)
    }

    /// Whether kinetic scrolling is still active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Immediately stop kinetic scrolling.
    pub fn stop(&mut self) {
        self.active = false;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    /// Current velocity (px/s).
    pub fn velocity(&self) -> (f64, f64) {
        (self.vx, self.vy)
    }
}

/// Rubber-band overscroll effect.
///
/// When content is scrolled past its bounds by `offset`, this function returns
/// a dampened visual offset. The result asymptotically approaches `limit` but
/// never exceeds it.
///
/// Formula based on Apple's rubber-banding: `d = (1 - 1/(offset/limit + 1)) * limit`
pub fn rubber_band(offset: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        return 0.0;
    }
    let abs_offset = offset.abs();
    let dampened = (1.0 - 1.0 / (abs_offset / limit + 1.0)) * limit;
    dampened.copysign(offset)
}

/// Inverse of `rubber_band`: given a dampened visual offset, recover the raw offset.
pub fn rubber_band_inverse(dampened: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        return 0.0;
    }
    let abs_d = dampened.abs();
    if abs_d >= limit {
        return f64::INFINITY.copysign(dampened);
    }
    let raw = limit * abs_d / (limit - abs_d);
    raw.copysign(dampened)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flick_velocity_two_points() {
        // 100px in 0.1s = 1000 px/s
        let points = vec![(0.0, 0.0, 0), (100.0, 0.0, 100_000)];
        let (vx, vy) = flick_velocity(&points);
        assert!((vx - 1000.0).abs() < 1.0);
        assert!(vy.abs() < 1.0);
    }

    #[test]
    fn flick_velocity_empty() {
        assert_eq!(flick_velocity(&[]), (0.0, 0.0));
    }

    #[test]
    fn flick_velocity_single_point() {
        let points = vec![(10.0, 20.0, 1000)];
        assert_eq!(flick_velocity(&points), (0.0, 0.0));
    }

    #[test]
    fn flick_velocity_vertical() {
        let points = vec![(0.0, 0.0, 0), (0.0, 50.0, 50_000)];
        let (vx, vy) = flick_velocity(&points);
        assert!(vx.abs() < 1.0);
        assert!((vy - 1000.0).abs() < 1.0);
    }

    #[test]
    fn flick_velocity_weighted_recent() {
        // Older segment: slow (10px in 100ms = 100 px/s)
        // Newer segment: fast (90px in 100ms = 900 px/s)
        // Weighted avg should be closer to 900 than to 100
        let points = vec![(0.0, 0.0, 0), (10.0, 0.0, 100_000), (100.0, 0.0, 200_000)];
        let (vx, _) = flick_velocity(&points);
        assert!(vx > 500.0, "vx={} should be weighted toward recent", vx);
    }

    #[test]
    fn kinetic_deceleration() {
        let mut ks = KineticState::new(KineticConfig {
            friction: 3.0,
            min_velocity: 1.0,
            max_velocity: 10000.0,
        });
        ks.start(1000.0, 0.0);
        assert!(ks.is_active());

        let (dx, _) = ks.tick(0.016); // ~16ms tick
        assert!(dx > 0.0);

        // After many ticks, should eventually stop
        for _ in 0..1000 {
            ks.tick(0.016);
        }
        assert!(!ks.is_active());
    }

    #[test]
    fn kinetic_stop() {
        let mut ks = KineticState::new(KineticConfig::default());
        ks.start(500.0, 500.0);
        assert!(ks.is_active());
        ks.stop();
        assert!(!ks.is_active());
        assert_eq!(ks.velocity(), (0.0, 0.0));
    }

    #[test]
    fn kinetic_no_start_below_min() {
        let mut ks = KineticState::new(KineticConfig {
            min_velocity: 10.0,
            ..KineticConfig::default()
        });
        ks.start(0.5, 0.5);
        assert!(!ks.is_active());
    }

    #[test]
    fn kinetic_max_velocity_clamp() {
        let mut ks = KineticState::new(KineticConfig {
            max_velocity: 100.0,
            ..KineticConfig::default()
        });
        ks.start(1000.0, 0.0);
        let (vx, _) = ks.velocity();
        assert!((vx - 100.0).abs() < 0.01);
    }

    #[test]
    fn kinetic_zero_dt_no_movement() {
        let mut ks = KineticState::new(KineticConfig::default());
        ks.start(1000.0, 0.0);
        let (dx, dy) = ks.tick(0.0);
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn kinetic_negative_dt_no_movement() {
        let mut ks = KineticState::new(KineticConfig::default());
        ks.start(1000.0, 0.0);
        let (dx, dy) = ks.tick(-1.0);
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn rubber_band_zero_offset() {
        assert!((rubber_band(0.0, 100.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn rubber_band_positive() {
        let d = rubber_band(50.0, 100.0);
        // Should be less than 50 (dampened) and less than 100 (limit)
        assert!(d > 0.0);
        assert!(d < 50.0);
        assert!(d < 100.0);
    }

    #[test]
    fn rubber_band_negative() {
        let d = rubber_band(-50.0, 100.0);
        assert!(d < 0.0);
        assert!(d > -50.0);
    }

    #[test]
    fn rubber_band_large_offset_approaches_limit() {
        let d = rubber_band(10_000.0, 100.0);
        // Should be close to 100 but not exceed
        assert!(d > 95.0);
        assert!(d < 100.0);
    }

    #[test]
    fn rubber_band_zero_limit() {
        assert_eq!(rubber_band(50.0, 0.0), 0.0);
    }

    #[test]
    fn rubber_band_inverse_roundtrip() {
        for offset in &[10.0, 50.0, 100.0, 500.0] {
            let dampened = rubber_band(*offset, 200.0);
            let recovered = rubber_band_inverse(dampened, 200.0);
            assert!(
                (recovered - offset).abs() < 0.01,
                "offset={} dampened={} recovered={}",
                offset,
                dampened,
                recovered
            );
        }
    }

    #[test]
    fn rubber_band_inverse_negative() {
        let dampened = rubber_band(-80.0, 200.0);
        let recovered = rubber_band_inverse(dampened, 200.0);
        assert!((recovered - (-80.0)).abs() < 0.01);
    }
}
