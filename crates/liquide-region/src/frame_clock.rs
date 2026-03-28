//! Frame timing and scheduling.
//!
//! Provides a `FrameClock` for tracking frame intervals and measuring
//! actual FPS, plus a `FrameThrottler` that adaptively adjusts the
//! target frame rate based on activity (animations vs idle).

/// Frame timing tracker.
///
/// Tracks when frames begin and end, measures actual frame durations,
/// and provides a `should_render()` predicate for frame pacing.
#[derive(Debug, Clone)]
pub struct FrameClock {
    /// Target frames per second.
    target_fps: u32,
    /// Precomputed frame interval in microseconds (1_000_000 / target_fps).
    frame_interval_us: u64,
    /// Timestamp (us) of the most recent `begin_frame()` call.
    last_frame_us: u64,
    /// Duration (us) of the most recently completed frame.
    last_frame_duration_us: u64,
    /// Total number of frames rendered (incremented on `end_frame()`).
    frame_count: u64,
    /// Timestamp of `begin_frame()` for the frame currently in progress.
    current_frame_start_us: u64,
    /// Running average of frame duration for FPS computation (EMA, us).
    avg_frame_duration_us: f64,
    /// True after the first `begin_frame()` call.
    started: bool,
}

impl FrameClock {
    /// Create a new frame clock targeting `target_fps` frames per second.
    ///
    /// # Panics
    /// Panics if `target_fps` is zero.
    pub fn new(target_fps: u32) -> Self {
        assert!(target_fps > 0, "target_fps must be > 0");
        Self {
            target_fps,
            frame_interval_us: 1_000_000 / target_fps as u64,
            last_frame_us: 0,
            last_frame_duration_us: 0,
            frame_count: 0,
            current_frame_start_us: 0,
            avg_frame_duration_us: 1_000_000.0 / target_fps as f64,
            started: false,
        }
    }

    /// True if enough time has elapsed since the last frame for another
    /// frame to begin at the target rate.
    ///
    /// `now_us` is the current monotonic time in microseconds.
    #[inline]
    pub fn should_render(&self, now_us: u64) -> bool {
        if !self.started {
            return true; // First frame always renders.
        }
        now_us.saturating_sub(self.last_frame_us) >= self.frame_interval_us
    }

    /// Mark the start of a new frame.
    ///
    /// Call this at the very beginning of your render loop iteration.
    pub fn begin_frame(&mut self, now_us: u64) {
        self.current_frame_start_us = now_us;
        self.last_frame_us = now_us;
        self.started = true;
    }

    /// Mark the end of the current frame and record its duration.
    ///
    /// Call this after all rendering and presentation for the frame
    /// is complete.
    pub fn end_frame(&mut self, now_us: u64) {
        self.last_frame_duration_us = now_us.saturating_sub(self.current_frame_start_us);
        self.frame_count += 1;

        // Update exponential moving average (alpha = 0.1 for smooth FPS).
        const ALPHA: f64 = 0.1;
        self.avg_frame_duration_us = ALPHA * self.last_frame_duration_us as f64
            + (1.0 - ALPHA) * self.avg_frame_duration_us;
    }

    /// The time budget for each frame in microseconds.
    ///
    /// For 60 fps this is 16666 us.
    #[inline]
    pub fn frame_budget_us(&self) -> u64 {
        self.frame_interval_us
    }

    /// Duration of the most recently completed frame in microseconds.
    #[inline]
    pub fn last_frame_duration_us(&self) -> u64 {
        self.last_frame_duration_us
    }

    /// Measured frames per second, computed from the exponential moving
    /// average of recent frame durations.
    ///
    /// Returns 0.0 if no frames have been rendered yet.
    pub fn fps(&self) -> f32 {
        if self.frame_count == 0 || self.avg_frame_duration_us <= 0.0 {
            return 0.0;
        }
        (1_000_000.0 / self.avg_frame_duration_us) as f32
    }

    /// Change the target frame rate.
    ///
    /// # Panics
    /// Panics if `fps` is zero.
    pub fn set_target_fps(&mut self, fps: u32) {
        assert!(fps > 0, "target_fps must be > 0");
        self.target_fps = fps;
        self.frame_interval_us = 1_000_000 / fps as u64;
    }

    /// Total number of frames rendered (completed via `end_frame()`).
    #[inline]
    pub fn frames_rendered(&self) -> u64 {
        self.frame_count
    }

    /// True if the last frame took longer than the frame budget,
    /// meaning we are falling behind the target frame rate.
    #[inline]
    pub fn is_behind(&self) -> bool {
        self.last_frame_duration_us > self.frame_interval_us
    }

    /// The configured target FPS.
    #[inline]
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

/// Adaptive frame rate throttler.
///
/// Reduces the target FPS when the system is idle (no animations,
/// no pending repaints) and increases it back to the maximum when
/// activity resumes. This saves CPU/GPU when nothing is moving.
#[derive(Debug, Clone)]
pub struct FrameThrottler {
    /// Minimum FPS when fully idle.
    min_fps: u32,
    /// Maximum FPS when animating.
    max_fps: u32,
    /// Current effective target FPS.
    current_fps: u32,
    /// True if animations are in progress.
    animating: bool,
    /// True if a repaint has been requested (e.g., damage was added).
    needs_repaint: bool,
    /// Timestamp of the last activity (animation or repaint request).
    last_activity_us: u64,
    /// Microseconds of idle time before ramping down FPS.
    idle_timeout_us: u64,
}

impl FrameThrottler {
    /// Create a new throttler that adapts between `min_fps` and `max_fps`.
    ///
    /// # Panics
    /// Panics if `min_fps` is zero or `min_fps > max_fps`.
    pub fn new(min_fps: u32, max_fps: u32) -> Self {
        assert!(min_fps > 0, "min_fps must be > 0");
        assert!(min_fps <= max_fps, "min_fps must be <= max_fps");
        Self {
            min_fps,
            max_fps,
            current_fps: max_fps,
            animating: false,
            needs_repaint: false,
            last_activity_us: 0,
            // Default: ramp down after 500ms of idle.
            idle_timeout_us: 500_000,
        }
    }

    /// Signal whether animations are currently running.
    pub fn set_animating(&mut self, animating: bool) {
        self.animating = animating;
    }

    /// Signal whether a repaint is needed (e.g., new damage arrived).
    pub fn set_needs_repaint(&mut self, needs: bool) {
        self.needs_repaint = needs;
    }

    /// The current effective target FPS.
    #[inline]
    pub fn current_target_fps(&self) -> u32 {
        self.current_fps
    }

    /// Update the throttler state. Call this once per frame.
    ///
    /// `now_us` is the current monotonic time in microseconds.
    pub fn tick(&mut self, now_us: u64) {
        if self.animating || self.needs_repaint {
            self.last_activity_us = now_us;
            self.current_fps = self.max_fps;
            return;
        }

        // No activity — check idle duration.
        let idle_us = now_us.saturating_sub(self.last_activity_us);
        if idle_us >= self.idle_timeout_us {
            self.current_fps = self.min_fps;
        }
        // else: keep current fps (ramp-down could be gradual, but
        // for simplicity we snap to min after the timeout).
    }

    /// Set the idle timeout in microseconds. After this duration with
    /// no activity, the frame rate drops to `min_fps`.
    pub fn set_idle_timeout_us(&mut self, timeout_us: u64) {
        self.idle_timeout_us = timeout_us;
    }

    /// True if the throttler is currently at its minimum (idle) FPS.
    #[inline]
    pub fn is_idle(&self) -> bool {
        self.current_fps == self.min_fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- FrameClock tests ----

    #[test]
    fn clock_new() {
        let c = FrameClock::new(60);
        assert_eq!(c.target_fps(), 60);
        assert_eq!(c.frame_budget_us(), 16666);
        assert_eq!(c.frames_rendered(), 0);
        assert_eq!(c.fps(), 0.0);
    }

    #[test]
    fn clock_should_render_first_frame() {
        let c = FrameClock::new(60);
        assert!(c.should_render(0));
        assert!(c.should_render(1_000_000));
    }

    #[test]
    fn clock_should_render_timing() {
        let mut c = FrameClock::new(60);
        c.begin_frame(0);
        c.end_frame(5000);
        // 10ms later — not enough (need ~16.6ms)
        assert!(!c.should_render(10_000));
        // 17ms later — enough
        assert!(c.should_render(17_000));
    }

    #[test]
    fn clock_frame_duration() {
        let mut c = FrameClock::new(60);
        c.begin_frame(1_000_000);
        c.end_frame(1_010_000); // 10ms frame
        assert_eq!(c.last_frame_duration_us(), 10_000);
        assert_eq!(c.frames_rendered(), 1);
    }

    #[test]
    fn clock_fps_measurement() {
        let mut c = FrameClock::new(60);
        // Simulate 10 frames at exactly 16666us each.
        let mut t = 0u64;
        for _ in 0..10 {
            c.begin_frame(t);
            t += 16666;
            c.end_frame(t);
        }
        let fps = c.fps();
        // Should be close to 60.
        assert!(fps > 55.0 && fps < 65.0, "fps = {}", fps);
    }

    #[test]
    fn clock_is_behind() {
        let mut c = FrameClock::new(60);
        c.begin_frame(0);
        c.end_frame(20_000); // 20ms > 16.6ms budget
        assert!(c.is_behind());

        c.begin_frame(20_000);
        c.end_frame(30_000); // 10ms < 16.6ms budget
        assert!(!c.is_behind());
    }

    #[test]
    fn clock_set_target_fps() {
        let mut c = FrameClock::new(60);
        c.set_target_fps(30);
        assert_eq!(c.target_fps(), 30);
        assert_eq!(c.frame_budget_us(), 33333);
    }

    #[test]
    #[should_panic(expected = "target_fps must be > 0")]
    fn clock_zero_fps_panics() {
        FrameClock::new(0);
    }

    #[test]
    #[should_panic(expected = "target_fps must be > 0")]
    fn clock_set_zero_fps_panics() {
        let mut c = FrameClock::new(60);
        c.set_target_fps(0);
    }

    #[test]
    fn clock_120fps() {
        let c = FrameClock::new(120);
        assert_eq!(c.frame_budget_us(), 8333);
    }

    // ---- FrameThrottler tests ----

    #[test]
    fn throttler_new() {
        let t = FrameThrottler::new(1, 60);
        assert_eq!(t.current_target_fps(), 60);
        assert!(!t.is_idle());
    }

    #[test]
    fn throttler_stays_max_when_animating() {
        let mut t = FrameThrottler::new(1, 60);
        t.set_animating(true);
        t.tick(1_000_000); // 1 second
        assert_eq!(t.current_target_fps(), 60);
    }

    #[test]
    fn throttler_stays_max_when_needs_repaint() {
        let mut t = FrameThrottler::new(1, 60);
        t.set_needs_repaint(true);
        t.tick(1_000_000);
        assert_eq!(t.current_target_fps(), 60);
    }

    #[test]
    fn throttler_drops_to_min_when_idle() {
        let mut t = FrameThrottler::new(1, 60);
        // First tick with activity to set last_activity_us.
        t.set_animating(true);
        t.tick(0);
        assert_eq!(t.current_target_fps(), 60);

        // Stop animating.
        t.set_animating(false);
        // Tick within idle timeout — still at max.
        t.tick(400_000); // 400ms < 500ms default timeout
        assert_eq!(t.current_target_fps(), 60);

        // Tick past idle timeout.
        t.tick(600_000); // 600ms > 500ms
        assert_eq!(t.current_target_fps(), 1);
        assert!(t.is_idle());
    }

    #[test]
    fn throttler_recovers_from_idle() {
        let mut t = FrameThrottler::new(4, 60);
        // Go idle.
        t.set_animating(true);
        t.tick(0);
        t.set_animating(false);
        t.tick(1_000_000);
        assert_eq!(t.current_target_fps(), 4);

        // Resume activity.
        t.set_animating(true);
        t.tick(1_500_000);
        assert_eq!(t.current_target_fps(), 60);
    }

    #[test]
    fn throttler_custom_idle_timeout() {
        let mut t = FrameThrottler::new(1, 60);
        t.set_idle_timeout_us(100_000); // 100ms

        t.set_animating(true);
        t.tick(0);
        t.set_animating(false);

        t.tick(50_000); // 50ms — not idle yet
        assert_eq!(t.current_target_fps(), 60);

        t.tick(200_000); // 200ms > 100ms — idle
        assert_eq!(t.current_target_fps(), 1);
    }

    #[test]
    #[should_panic(expected = "min_fps must be > 0")]
    fn throttler_zero_min_panics() {
        FrameThrottler::new(0, 60);
    }

    #[test]
    #[should_panic(expected = "min_fps must be <= max_fps")]
    fn throttler_inverted_range_panics() {
        FrameThrottler::new(120, 60);
    }
}
