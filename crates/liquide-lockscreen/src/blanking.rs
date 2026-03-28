/// Screen blanking control.
///
/// Manages display brightness transitions (dim, blank, DPMS off)
/// with smooth interpolation between states.

/// Display blanking state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlankState {
    /// Normal brightness (1.0).
    Normal,
    /// Dimmed to the given brightness (0.0..=1.0).
    Dimmed(f32),
    /// Fully blanked (black screen, brightness 0.0).
    Blanked,
    /// Display powered off via DPMS.
    DPMSOff,
}

/// Controls display blanking with smooth transitions.
pub struct BlankController {
    state: BlankState,
    current_brightness: f32,
    target_brightness: f32,
    /// Transition speed in brightness units per millisecond.
    transition_speed: f32,
}

impl BlankController {
    /// Create a new controller at full brightness.
    pub fn new() -> Self {
        Self {
            state: BlankState::Normal,
            current_brightness: 1.0,
            target_brightness: 1.0,
            transition_speed: 0.002, // ~500ms for full 0->1 transition
        }
    }

    /// Dim the display to the given brightness (0.0..=1.0).
    pub fn dim(&mut self, brightness: f32) {
        let b = brightness.clamp(0.0, 1.0);
        self.state = BlankState::Dimmed(b);
        self.target_brightness = b;
    }

    /// Blank the screen (brightness to 0).
    pub fn blank(&mut self) {
        self.state = BlankState::Blanked;
        self.target_brightness = 0.0;
    }

    /// Tell the display to power off (DPMS standby/off).
    pub fn dpms_off(&mut self) {
        self.state = BlankState::DPMSOff;
        self.target_brightness = 0.0;
        self.current_brightness = 0.0; // immediate
    }

    /// Wake the display: restore to full brightness.
    pub fn wake(&mut self) {
        self.state = BlankState::Normal;
        self.target_brightness = 1.0;
    }

    /// Advance smooth brightness transitions.
    pub fn tick(&mut self, dt_ms: f32) {
        if (self.current_brightness - self.target_brightness).abs() < 0.001 {
            self.current_brightness = self.target_brightness;
            return;
        }

        let step = self.transition_speed * dt_ms;
        if self.current_brightness < self.target_brightness {
            self.current_brightness = (self.current_brightness + step).min(self.target_brightness);
        } else {
            self.current_brightness = (self.current_brightness - step).max(self.target_brightness);
        }
    }

    /// Current interpolated brightness (0.0..=1.0).
    pub fn current_brightness(&self) -> f32 {
        self.current_brightness
    }

    /// Current blanking state.
    pub fn state(&self) -> &BlankState {
        &self.state
    }

    /// Set the transition speed (brightness units per millisecond).
    pub fn set_transition_speed(&mut self, speed: f32) {
        self.transition_speed = speed.max(0.0001);
    }

    /// Whether the brightness transition is complete.
    pub fn is_transition_complete(&self) -> bool {
        (self.current_brightness - self.target_brightness).abs() < 0.001
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_normal() {
        let ctrl = BlankController::new();
        assert_eq!(*ctrl.state(), BlankState::Normal);
        assert_eq!(ctrl.current_brightness(), 1.0);
    }

    #[test]
    fn dim_sets_target() {
        let mut ctrl = BlankController::new();
        ctrl.dim(0.5);
        assert!(matches!(*ctrl.state(), BlankState::Dimmed(b) if (b - 0.5).abs() < 0.001));
    }

    #[test]
    fn dim_clamps_brightness() {
        let mut ctrl = BlankController::new();
        ctrl.dim(1.5);
        assert!(matches!(*ctrl.state(), BlankState::Dimmed(b) if (b - 1.0).abs() < 0.001));
        ctrl.dim(-0.5);
        assert!(matches!(*ctrl.state(), BlankState::Dimmed(b) if b.abs() < 0.001));
    }

    #[test]
    fn blank_targets_zero() {
        let mut ctrl = BlankController::new();
        ctrl.blank();
        assert_eq!(*ctrl.state(), BlankState::Blanked);
        // After enough ticks, brightness reaches 0
        for _ in 0..1000 {
            ctrl.tick(1.0);
        }
        assert!(ctrl.current_brightness() < 0.01);
    }

    #[test]
    fn dpms_off_immediate() {
        let mut ctrl = BlankController::new();
        ctrl.dpms_off();
        assert_eq!(*ctrl.state(), BlankState::DPMSOff);
        assert_eq!(ctrl.current_brightness(), 0.0);
    }

    #[test]
    fn wake_restores_brightness() {
        let mut ctrl = BlankController::new();
        ctrl.blank();
        // Run transition to zero
        for _ in 0..1000 {
            ctrl.tick(1.0);
        }
        assert!(ctrl.current_brightness() < 0.01);

        ctrl.wake();
        assert_eq!(*ctrl.state(), BlankState::Normal);
        // Run transition back up
        for _ in 0..1000 {
            ctrl.tick(1.0);
        }
        assert!((ctrl.current_brightness() - 1.0).abs() < 0.01);
    }

    #[test]
    fn wake_from_dpms() {
        let mut ctrl = BlankController::new();
        ctrl.dpms_off();
        assert_eq!(ctrl.current_brightness(), 0.0);
        ctrl.wake();
        assert_eq!(*ctrl.state(), BlankState::Normal);
        // Should transition back to 1.0
        for _ in 0..1000 {
            ctrl.tick(1.0);
        }
        assert!((ctrl.current_brightness() - 1.0).abs() < 0.01);
    }

    #[test]
    fn smooth_dim_transition() {
        let mut ctrl = BlankController::new();
        ctrl.set_transition_speed(0.01); // 0.01 per ms
        ctrl.dim(0.5);

        // After 25ms: brightness should drop by 0.25 (from 1.0 to ~0.75)
        ctrl.tick(25.0);
        assert!(ctrl.current_brightness() < 1.0);
        assert!(ctrl.current_brightness() > 0.5);

        // After many more ms, should reach target
        for _ in 0..100 {
            ctrl.tick(10.0);
        }
        assert!((ctrl.current_brightness() - 0.5).abs() < 0.01);
    }

    #[test]
    fn transition_complete_flag() {
        let mut ctrl = BlankController::new();
        assert!(ctrl.is_transition_complete());

        ctrl.dim(0.5);
        assert!(!ctrl.is_transition_complete());

        for _ in 0..2000 {
            ctrl.tick(1.0);
        }
        assert!(ctrl.is_transition_complete());
    }

    #[test]
    fn tick_no_overshoot_downward() {
        let mut ctrl = BlankController::new();
        ctrl.set_transition_speed(1.0); // very fast
        ctrl.dim(0.5);
        ctrl.tick(1000.0); // way past target
        assert!((ctrl.current_brightness() - 0.5).abs() < 0.001);
    }

    #[test]
    fn tick_no_overshoot_upward() {
        let mut ctrl = BlankController::new();
        ctrl.dpms_off(); // 0.0
        ctrl.wake(); // target 1.0
        ctrl.set_transition_speed(1.0);
        ctrl.tick(1000.0);
        assert!((ctrl.current_brightness() - 1.0).abs() < 0.001);
    }

    #[test]
    fn multiple_state_changes() {
        let mut ctrl = BlankController::new();
        ctrl.dim(0.7);
        ctrl.tick(100.0);
        ctrl.blank();
        ctrl.tick(100.0);
        ctrl.wake();
        ctrl.tick(100.0);
        // Should be heading back toward 1.0
        assert_eq!(*ctrl.state(), BlankState::Normal);
    }

    #[test]
    fn set_transition_speed_clamps() {
        let mut ctrl = BlankController::new();
        ctrl.set_transition_speed(-1.0);
        // Should be clamped to minimum
        ctrl.dim(0.5);
        ctrl.tick(1.0);
        // Should still move (not be stuck)
        assert!(ctrl.current_brightness() < 1.0);
    }
}
