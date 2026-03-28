//! Particle effects for visual embellishment.
//!
//! Provides a lightweight particle system suitable for window close effects,
//! celebration animations, ambient desktop effects (snow), or notification
//! sparkles. Particles are tick-driven and stateless beyond their own
//! position/velocity/lifetime.

/// A single particle with position, velocity, and visual properties.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    /// X position in pixels.
    pub x: f64,
    /// Y position in pixels.
    pub y: f64,
    /// X velocity in pixels per second.
    pub vx: f64,
    /// Y velocity in pixels per second.
    pub vy: f64,
    /// X acceleration in pixels per second squared.
    pub ax: f64,
    /// Y acceleration in pixels per second squared.
    pub ay: f64,
    /// Remaining lifetime in seconds.
    pub lifetime: f64,
    /// Initial lifetime (for computing normalized age).
    pub initial_lifetime: f64,
    /// Current alpha (0.0 = invisible, 1.0 = fully opaque).
    pub alpha: f64,
    /// Current size in pixels.
    pub size: f64,
    /// Color as RGBA packed u32 (0xRRGGBBAA).
    pub color: u32,
}

impl Particle {
    /// Compute the normalized age (0.0 = just born, 1.0 = about to expire).
    pub fn age(&self) -> f64 {
        if self.initial_lifetime <= 0.0 {
            return 1.0;
        }
        1.0 - (self.lifetime / self.initial_lifetime).clamp(0.0, 1.0)
    }

    /// Whether this particle has expired.
    pub fn is_expired(&self) -> bool {
        self.lifetime <= 0.0
    }

    /// Advance the particle by `dt` seconds.
    pub fn tick(&mut self, dt: f64) {
        self.vx += self.ax * dt;
        self.vy += self.ay * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.lifetime -= dt;

        // Fade out based on age.
        let age = self.age();
        self.alpha = (1.0 - age).max(0.0);
    }
}

/// Preset particle effect configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticlePreset {
    /// Colorful confetti pieces falling with random spin. Use case: celebrations.
    Confetti,
    /// Small bright sparkles that fade quickly. Use case: window close, magic effects.
    Sparkle,
    /// Soft gray particles rising slowly. Use case: ambient effect, transitions.
    Smoke,
    /// White particles drifting downward. Use case: ambient desktop, seasonal effect.
    Snow,
}

/// Configuration for a particle emitter.
#[derive(Debug, Clone)]
pub struct EmitterConfig {
    /// Spawn rate in particles per second.
    pub spawn_rate: f64,
    /// Minimum initial X velocity.
    pub vel_x_min: f64,
    /// Maximum initial X velocity.
    pub vel_x_max: f64,
    /// Minimum initial Y velocity.
    pub vel_y_min: f64,
    /// Maximum initial Y velocity.
    pub vel_y_max: f64,
    /// Minimum particle lifetime in seconds.
    pub lifetime_min: f64,
    /// Maximum particle lifetime in seconds.
    pub lifetime_max: f64,
    /// Gravity (Y acceleration applied to all particles). Positive = downward.
    pub gravity: f64,
    /// Minimum initial particle size in pixels.
    pub size_min: f64,
    /// Maximum initial particle size in pixels.
    pub size_max: f64,
    /// Particle color as RGBA packed u32.
    pub color: u32,
    /// Maximum number of particles alive at once.
    pub max_particles: usize,
}

impl EmitterConfig {
    /// Create a configuration from a preset.
    pub fn from_preset(preset: ParticlePreset) -> Self {
        match preset {
            ParticlePreset::Confetti => Self {
                spawn_rate: 60.0,
                vel_x_min: -200.0,
                vel_x_max: 200.0,
                vel_y_min: -400.0,
                vel_y_max: -100.0,
                lifetime_min: 1.5,
                lifetime_max: 3.0,
                gravity: 300.0,
                size_min: 4.0,
                size_max: 10.0,
                color: 0xFF4488FF, // bright pink
                max_particles: 200,
            },
            ParticlePreset::Sparkle => Self {
                spawn_rate: 40.0,
                vel_x_min: -100.0,
                vel_x_max: 100.0,
                vel_y_min: -150.0,
                vel_y_max: -50.0,
                lifetime_min: 0.3,
                lifetime_max: 0.8,
                gravity: 0.0,
                size_min: 2.0,
                size_max: 5.0,
                color: 0xFFFFCCFF, // bright yellow
                max_particles: 100,
            },
            ParticlePreset::Smoke => Self {
                spawn_rate: 15.0,
                vel_x_min: -20.0,
                vel_x_max: 20.0,
                vel_y_min: -80.0,
                vel_y_max: -30.0,
                lifetime_min: 1.0,
                lifetime_max: 2.5,
                gravity: -10.0, // slight upward buoyancy
                size_min: 8.0,
                size_max: 20.0,
                color: 0x888888AA, // gray semi-transparent
                max_particles: 80,
            },
            ParticlePreset::Snow => Self {
                spawn_rate: 25.0,
                vel_x_min: -30.0,
                vel_x_max: 30.0,
                vel_y_min: 20.0,
                vel_y_max: 80.0,
                lifetime_min: 3.0,
                lifetime_max: 6.0,
                gravity: 5.0,
                size_min: 3.0,
                size_max: 7.0,
                color: 0xFFFFFFFF, // white
                max_particles: 150,
            },
        }
    }
}

/// A particle emitter that spawns and manages particles.
///
/// Call `tick(dt)` each frame to advance the simulation. Use
/// `active_particles()` to get the current set of live particles for rendering.
pub struct ParticleEmitter {
    /// Emitter configuration.
    config: EmitterConfig,
    /// Live particles.
    particles: Vec<Particle>,
    /// Emitter position (spawn origin).
    pub origin_x: f64,
    /// Emitter position (spawn origin).
    pub origin_y: f64,
    /// Whether the emitter is actively spawning new particles.
    pub emitting: bool,
    /// Accumulated time for spawn rate control.
    spawn_accumulator: f64,
    /// Simple PRNG state (xorshift64).
    rng_state: u64,
}

impl ParticleEmitter {
    /// Create a new emitter at the given origin with the specified config.
    pub fn new(config: EmitterConfig, origin_x: f64, origin_y: f64) -> Self {
        Self {
            particles: Vec::with_capacity(config.max_particles),
            config,
            origin_x,
            origin_y,
            emitting: true,
            spawn_accumulator: 0.0,
            rng_state: 0xDEAD_BEEF_CAFE_1234,
        }
    }

    /// Create an emitter from a preset.
    pub fn from_preset(preset: ParticlePreset, origin_x: f64, origin_y: f64) -> Self {
        Self::new(EmitterConfig::from_preset(preset), origin_x, origin_y)
    }

    /// Advance the particle system by `dt` seconds.
    ///
    /// Spawns new particles (if emitting), updates existing particles, and
    /// removes expired ones.
    pub fn tick(&mut self, dt: f64) {
        // Update existing particles.
        for p in &mut self.particles {
            p.ay = self.config.gravity;
            p.tick(dt);
        }

        // Remove expired particles.
        self.particles.retain(|p| !p.is_expired());

        // Spawn new particles.
        if self.emitting {
            self.spawn_accumulator += dt;
            let interval = if self.config.spawn_rate > 0.0 {
                1.0 / self.config.spawn_rate
            } else {
                f64::INFINITY
            };

            while self.spawn_accumulator >= interval
                && self.particles.len() < self.config.max_particles
            {
                self.spawn_accumulator -= interval;
                self.spawn_particle();
            }
        }
    }

    /// Spawn a single particle at the emitter origin.
    fn spawn_particle(&mut self) {
        let vx = self.rand_range(self.config.vel_x_min, self.config.vel_x_max);
        let vy = self.rand_range(self.config.vel_y_min, self.config.vel_y_max);
        let lifetime = self.rand_range(self.config.lifetime_min, self.config.lifetime_max);
        let size = self.rand_range(self.config.size_min, self.config.size_max);

        self.particles.push(Particle {
            x: self.origin_x,
            y: self.origin_y,
            vx,
            vy,
            ax: 0.0,
            ay: self.config.gravity,
            lifetime,
            initial_lifetime: lifetime,
            alpha: 1.0,
            size,
            color: self.config.color,
        });
    }

    /// Get all currently active particles.
    pub fn active_particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Get the number of active particles.
    pub fn active_count(&self) -> usize {
        self.particles.len()
    }

    /// Whether the emitter has no active particles and is not emitting.
    pub fn is_idle(&self) -> bool {
        !self.emitting && self.particles.is_empty()
    }

    /// Stop emitting new particles (existing particles continue until expired).
    pub fn stop(&mut self) {
        self.emitting = false;
    }

    /// Clear all particles immediately.
    pub fn clear(&mut self) {
        self.particles.clear();
        self.spawn_accumulator = 0.0;
    }

    /// Seed the PRNG for deterministic testing.
    pub fn seed(&mut self, seed: u64) {
        self.rng_state = seed;
        if self.rng_state == 0 {
            self.rng_state = 1;
        }
    }

    /// Simple xorshift64 PRNG returning a value in [0, 1).
    fn rand_01(&mut self) -> f64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state as f64) / (u64::MAX as f64)
    }

    /// Random value in [min, max].
    fn rand_range(&mut self, min: f64, max: f64) -> f64 {
        min + self.rand_01() * (max - min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    #[test]
    fn particle_age_at_birth() {
        let p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 1.0, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        assert!((p.age() - 0.0).abs() < 0.001);
    }

    #[test]
    fn particle_age_at_half() {
        let p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 0.5, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        assert!((p.age() - 0.5).abs() < 0.001);
    }

    #[test]
    fn particle_is_expired() {
        let p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 0.0, initial_lifetime: 1.0, alpha: 0.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        assert!(p.is_expired());
    }

    #[test]
    fn particle_tick_moves() {
        let mut p = Particle {
            x: 0.0, y: 0.0, vx: 100.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 1.0, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        p.tick(0.1);
        assert!((p.x - 10.0).abs() < 0.01);
    }

    #[test]
    fn particle_tick_applies_acceleration() {
        let mut p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 100.0,
            lifetime: 1.0, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        p.tick(0.1);
        assert!(p.vy > 0.0, "gravity should increase vy");
        assert!(p.y > 0.0, "particle should move down");
    }

    #[test]
    fn particle_tick_reduces_lifetime() {
        let mut p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 1.0, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        p.tick(0.3);
        assert!((p.lifetime - 0.7).abs() < 0.001);
    }

    #[test]
    fn particle_alpha_fades_with_age() {
        let mut p = Particle {
            x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, ax: 0.0, ay: 0.0,
            lifetime: 1.0, initial_lifetime: 1.0, alpha: 1.0, size: 5.0,
            color: 0xFFFFFFFF,
        };
        p.tick(0.5);
        assert!(p.alpha < 1.0, "alpha should decrease: {}", p.alpha);
        assert!(p.alpha > 0.0, "alpha should still be positive: {}", p.alpha);
    }

    // --- EmitterConfig preset tests ---

    #[test]
    fn confetti_preset() {
        let cfg = EmitterConfig::from_preset(ParticlePreset::Confetti);
        assert!(cfg.spawn_rate > 0.0);
        assert!(cfg.gravity > 0.0); // confetti falls
    }

    #[test]
    fn sparkle_preset() {
        let cfg = EmitterConfig::from_preset(ParticlePreset::Sparkle);
        assert!(cfg.lifetime_max < 1.0); // sparkles are short-lived
    }

    #[test]
    fn smoke_preset() {
        let cfg = EmitterConfig::from_preset(ParticlePreset::Smoke);
        assert!(cfg.gravity < 0.0); // smoke rises
    }

    #[test]
    fn snow_preset() {
        let cfg = EmitterConfig::from_preset(ParticlePreset::Snow);
        assert!(cfg.vel_y_min > 0.0); // snow falls
    }

    // --- ParticleEmitter tests ---

    #[test]
    fn emitter_starts_empty() {
        let emitter = ParticleEmitter::from_preset(ParticlePreset::Sparkle, 100.0, 100.0);
        assert_eq!(emitter.active_count(), 0);
    }

    #[test]
    fn emitter_spawns_particles() {
        let mut emitter = ParticleEmitter::from_preset(ParticlePreset::Confetti, 100.0, 100.0);
        emitter.seed(42);
        // Tick for enough time that at least one particle spawns.
        emitter.tick(0.1);
        assert!(emitter.active_count() > 0, "should have spawned particles");
    }

    #[test]
    fn emitter_respects_max_particles() {
        let config = EmitterConfig {
            spawn_rate: 10000.0, // very fast
            max_particles: 5,
            vel_x_min: 0.0, vel_x_max: 0.0,
            vel_y_min: 0.0, vel_y_max: 0.0,
            lifetime_min: 10.0, lifetime_max: 10.0,
            gravity: 0.0,
            size_min: 1.0, size_max: 1.0,
            color: 0xFFFFFFFF,
        };
        let mut emitter = ParticleEmitter::new(config, 0.0, 0.0);
        emitter.seed(42);
        emitter.tick(1.0);
        assert!(emitter.active_count() <= 5);
    }

    #[test]
    fn emitter_removes_expired() {
        let config = EmitterConfig {
            spawn_rate: 100.0,
            max_particles: 50,
            vel_x_min: 0.0, vel_x_max: 0.0,
            vel_y_min: 0.0, vel_y_max: 0.0,
            lifetime_min: 0.05, lifetime_max: 0.05, // very short
            gravity: 0.0,
            size_min: 1.0, size_max: 1.0,
            color: 0xFFFFFFFF,
        };
        let mut emitter = ParticleEmitter::new(config, 0.0, 0.0);
        emitter.seed(42);
        emitter.tick(0.02); // spawn some
        let count_after_spawn = emitter.active_count();
        assert!(count_after_spawn > 0);
        emitter.stop();
        // Tick past their lifetime.
        for _ in 0..10 {
            emitter.tick(0.1);
        }
        assert_eq!(emitter.active_count(), 0, "all should have expired");
    }

    #[test]
    fn emitter_stop_stops_spawning() {
        let mut emitter = ParticleEmitter::from_preset(ParticlePreset::Confetti, 0.0, 0.0);
        emitter.seed(42);
        emitter.stop();
        emitter.tick(1.0);
        assert_eq!(emitter.active_count(), 0, "should not spawn when stopped");
    }

    #[test]
    fn emitter_clear() {
        let mut emitter = ParticleEmitter::from_preset(ParticlePreset::Confetti, 0.0, 0.0);
        emitter.seed(42);
        emitter.tick(0.1);
        assert!(emitter.active_count() > 0);
        emitter.clear();
        assert_eq!(emitter.active_count(), 0);
    }

    #[test]
    fn emitter_is_idle() {
        let mut emitter = ParticleEmitter::from_preset(ParticlePreset::Confetti, 0.0, 0.0);
        assert!(!emitter.is_idle()); // emitting = true
        emitter.stop();
        assert!(emitter.is_idle()); // no particles, not emitting
    }

    #[test]
    fn active_particles_returns_slice() {
        let mut emitter = ParticleEmitter::from_preset(ParticlePreset::Sparkle, 50.0, 50.0);
        emitter.seed(42);
        emitter.tick(0.1);
        let particles = emitter.active_particles();
        assert_eq!(particles.len(), emitter.active_count());
    }

    #[test]
    fn particles_move_over_time() {
        let config = EmitterConfig {
            spawn_rate: 1000.0,
            max_particles: 10,
            vel_x_min: 100.0, vel_x_max: 100.0,
            vel_y_min: 0.0, vel_y_max: 0.0,
            lifetime_min: 5.0, lifetime_max: 5.0,
            gravity: 0.0,
            size_min: 1.0, size_max: 1.0,
            color: 0xFFFFFFFF,
        };
        let mut emitter = ParticleEmitter::new(config, 0.0, 0.0);
        emitter.seed(42);
        emitter.tick(DT); // spawn
        emitter.stop();
        let initial_x = emitter.active_particles()[0].x;
        emitter.tick(0.1);
        let after_x = emitter.active_particles()[0].x;
        assert!(after_x > initial_x, "particle should move right: {initial_x} -> {after_x}");
    }

    #[test]
    fn deterministic_with_same_seed() {
        let config = EmitterConfig::from_preset(ParticlePreset::Sparkle);
        let mut e1 = ParticleEmitter::new(config.clone(), 0.0, 0.0);
        let mut e2 = ParticleEmitter::new(config, 0.0, 0.0);
        e1.seed(123);
        e2.seed(123);
        e1.tick(0.1);
        e2.tick(0.1);
        assert_eq!(e1.active_count(), e2.active_count());
        for (p1, p2) in e1.active_particles().iter().zip(e2.active_particles().iter()) {
            assert!((p1.x - p2.x).abs() < 0.001);
            assert!((p1.y - p2.y).abs() < 0.001);
        }
    }
}
