//! Quick peek — temporarily raise and highlight a single window while dimming
//! all others. Supports three-finger trackpad peek, Alt-Tab hover preview, and
//! taskbar hover preview modes.

/// How the peek was triggered, which determines behaviour and timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekMode {
    /// Three-finger trackpad gesture (peek as long as fingers are down).
    ThreeFingerPeek,
    /// Hovering over a window in the Alt-Tab switcher.
    AltTabHover,
    /// Hovering over a dock/taskbar item.
    TaskbarHover,
}

/// Internal phase of the peek animation.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PeekPhase {
    /// No peek is active.
    Idle,
    /// Dimming other windows and raising the target (progress 0..1).
    Entering(f32),
    /// Peek is fully active.
    Active,
    /// Restoring original stacking/opacity (progress 0..1).
    Exiting(f32),
}

/// State for the quick-peek feature.
pub struct PeekState {
    phase: PeekPhase,
    /// The window being peeked.
    target_window_id: Option<u64>,
    /// The mode that triggered this peek.
    mode: Option<PeekMode>,
    /// Opacity multiplier applied to all non-target windows (0.0 = fully dim).
    pub dim_opacity: f32,
    /// Auto-cancel timeout in milliseconds. 0 = no timeout.
    pub peek_timeout_ms: u64,
    /// How long (ms) the peek has been in the Active state.
    active_elapsed_ms: f64,
    /// Duration of the enter animation (ms).
    pub enter_duration_ms: f32,
    /// Duration of the exit animation (ms).
    pub exit_duration_ms: f32,
    /// Windows that were originally above the target (for restoring z-order).
    saved_z_order: Vec<u64>,
}

impl PeekState {
    pub fn new() -> Self {
        Self {
            phase: PeekPhase::Idle,
            target_window_id: None,
            mode: None,
            dim_opacity: 0.3,
            peek_timeout_ms: 5000,
            active_elapsed_ms: 0.0,
            enter_duration_ms: 150.0,
            exit_duration_ms: 100.0,
            saved_z_order: Vec::new(),
        }
    }

    /// Start peeking at a specific window.
    ///
    /// `windows_above` is the list of window IDs that are currently stacked
    /// above the target — these will be saved for restoration on `end_peek()`.
    pub fn start_peek(
        &mut self,
        window_id: u64,
        mode: PeekMode,
        windows_above: &[u64],
    ) {
        // If already peeking the same window, ignore.
        if self.target_window_id == Some(window_id) && !matches!(self.phase, PeekPhase::Idle | PeekPhase::Exiting(_)) {
            return;
        }

        self.target_window_id = Some(window_id);
        self.mode = Some(mode);
        self.saved_z_order = windows_above.to_vec();
        self.active_elapsed_ms = 0.0;
        self.phase = PeekPhase::Entering(0.0);
    }

    /// End the current peek (restore original stacking).
    pub fn end_peek(&mut self) {
        match self.phase {
            PeekPhase::Idle | PeekPhase::Exiting(_) => {}
            _ => {
                self.phase = PeekPhase::Exiting(0.0);
            }
        }
    }

    /// Advance the peek animation by `dt_ms` milliseconds.
    ///
    /// Returns `true` if the peek state changed and needs redraw.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        match self.phase {
            PeekPhase::Entering(p) => {
                let new_p = p + dt_ms / self.enter_duration_ms;
                if new_p >= 1.0 {
                    self.phase = PeekPhase::Active;
                    self.active_elapsed_ms = 0.0;
                } else {
                    self.phase = PeekPhase::Entering(new_p);
                }
                true
            }
            PeekPhase::Active => {
                self.active_elapsed_ms += dt_ms as f64;
                // Check for timeout.
                if self.peek_timeout_ms > 0
                    && self.active_elapsed_ms >= self.peek_timeout_ms as f64
                {
                    self.end_peek();
                    return true;
                }
                false
            }
            PeekPhase::Exiting(p) => {
                let new_p = p + dt_ms / self.exit_duration_ms;
                if new_p >= 1.0 {
                    self.phase = PeekPhase::Idle;
                    self.target_window_id = None;
                    self.mode = None;
                    self.saved_z_order.clear();
                }  else {
                    self.phase = PeekPhase::Exiting(new_p);
                }
                true
            }
            PeekPhase::Idle => false,
        }
    }

    /// Whether a peek is currently active (entering, active, or exiting).
    pub fn is_peeking(&self) -> bool {
        !matches!(self.phase, PeekPhase::Idle)
    }

    /// The window currently being peeked, if any.
    pub fn target(&self) -> Option<u64> {
        self.target_window_id
    }

    /// The mode that triggered the current peek.
    pub fn mode(&self) -> Option<PeekMode> {
        self.mode
    }

    /// Current dim opacity for non-target windows (0.0..1.0).
    ///
    /// During the enter animation this ramps from 1.0 to `dim_opacity`.
    /// During the exit animation it ramps from `dim_opacity` back to 1.0.
    pub fn current_dim_opacity(&self) -> f32 {
        match self.phase {
            PeekPhase::Idle => 1.0,
            PeekPhase::Entering(p) => lerp(1.0, self.dim_opacity, p.clamp(0.0, 1.0)),
            PeekPhase::Active => self.dim_opacity,
            PeekPhase::Exiting(p) => lerp(self.dim_opacity, 1.0, p.clamp(0.0, 1.0)),
        }
    }

    /// The saved z-order (windows that were above the target before peeking).
    pub fn saved_z_order(&self) -> &[u64] {
        &self.saved_z_order
    }

    /// Whether a given window should be dimmed (all windows except the target).
    pub fn should_dim(&self, window_id: u64) -> bool {
        if !self.is_peeking() {
            return false;
        }
        self.target_window_id != Some(window_id)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_peek() -> PeekState {
        PeekState::new()
    }

    // ── basic lifecycle ────────────────────────────────────────

    #[test]
    fn starts_idle() {
        let p = new_peek();
        assert!(!p.is_peeking());
        assert_eq!(p.target(), None);
        assert_eq!(p.mode(), None);
        assert_eq!(p.current_dim_opacity(), 1.0);
    }

    #[test]
    fn start_peek_enters() {
        let mut p = new_peek();
        p.start_peek(42, PeekMode::ThreeFingerPeek, &[10, 20]);
        assert!(p.is_peeking());
        assert_eq!(p.target(), Some(42));
        assert_eq!(p.mode(), Some(PeekMode::ThreeFingerPeek));
        assert_eq!(p.saved_z_order(), &[10, 20]);
    }

    #[test]
    fn start_same_window_is_noop() {
        let mut p = new_peek();
        p.start_peek(42, PeekMode::AltTabHover, &[]);
        p.tick(200.0); // fully active
        p.start_peek(42, PeekMode::AltTabHover, &[99]);
        // Should not have restarted — saved_z_order unchanged.
        assert_eq!(p.saved_z_order(), &[] as &[u64]);
    }

    #[test]
    fn end_peek_exits() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::TaskbarHover, &[]);
        p.tick(200.0);
        p.end_peek();
        assert!(p.is_peeking()); // still exiting
        p.tick(200.0);
        assert!(!p.is_peeking());
        assert_eq!(p.target(), None);
    }

    #[test]
    fn end_peek_from_idle_is_noop() {
        let mut p = new_peek();
        p.end_peek();
        assert!(!p.is_peeking());
    }

    // ── tick animation ─────────────────────────────────────────

    #[test]
    fn tick_enter_completes() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::ThreeFingerPeek, &[]);
        let changed = p.tick(200.0); // > 150ms default
        assert!(changed);
        assert!(matches!(p.phase, PeekPhase::Active));
    }

    #[test]
    fn tick_enter_partial() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::ThreeFingerPeek, &[]);
        p.tick(75.0); // half of 150ms
        assert!(matches!(p.phase, PeekPhase::Entering(_)));
    }

    #[test]
    fn tick_active_no_change() {
        let mut p = new_peek();
        p.peek_timeout_ms = 0; // no timeout
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        p.tick(200.0);
        let changed = p.tick(16.0);
        assert!(!changed);
    }

    #[test]
    fn tick_exit_completes() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        p.tick(200.0);
        p.end_peek();
        p.tick(200.0); // > 100ms default
        assert!(!p.is_peeking());
        assert!(p.saved_z_order().is_empty());
    }

    #[test]
    fn tick_idle_returns_false() {
        let mut p = new_peek();
        assert!(!p.tick(16.0));
    }

    // ── timeout ────────────────────────────────────────────────

    #[test]
    fn timeout_auto_cancels() {
        let mut p = new_peek();
        p.peek_timeout_ms = 100;
        p.start_peek(1, PeekMode::TaskbarHover, &[]);
        p.tick(200.0); // enter
        // Now active. Tick past the timeout.
        let changed = p.tick(150.0);
        assert!(changed);
        // Should now be exiting.
        assert!(p.is_peeking()); // still exiting
    }

    #[test]
    fn no_timeout_when_zero() {
        let mut p = new_peek();
        p.peek_timeout_ms = 0;
        p.start_peek(1, PeekMode::ThreeFingerPeek, &[]);
        p.tick(200.0); // enter
        p.tick(100_000.0); // long time, no timeout
        assert!(matches!(p.phase, PeekPhase::Active));
    }

    // ── dim opacity ────────────────────────────────────────────

    #[test]
    fn dim_opacity_idle_is_one() {
        let p = new_peek();
        assert_eq!(p.current_dim_opacity(), 1.0);
    }

    #[test]
    fn dim_opacity_active_is_config() {
        let mut p = new_peek();
        p.dim_opacity = 0.25;
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        p.tick(200.0);
        assert!((p.current_dim_opacity() - 0.25).abs() < 0.01);
    }

    #[test]
    fn dim_opacity_during_enter() {
        let mut p = new_peek();
        p.dim_opacity = 0.3;
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        p.tick(75.0); // half of 150ms
        let op = p.current_dim_opacity();
        // Should be between 1.0 and 0.3.
        assert!(op > 0.3);
        assert!(op < 1.0);
    }

    #[test]
    fn dim_opacity_during_exit() {
        let mut p = new_peek();
        p.dim_opacity = 0.3;
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        p.tick(200.0);
        p.end_peek();
        p.tick(50.0); // half of 100ms
        let op = p.current_dim_opacity();
        assert!(op > 0.3);
        assert!(op < 1.0);
    }

    // ── should_dim ─────────────────────────────────────────────

    #[test]
    fn should_dim_non_target() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        assert!(p.should_dim(2));
        assert!(p.should_dim(3));
    }

    #[test]
    fn should_not_dim_target() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        assert!(!p.should_dim(1));
    }

    #[test]
    fn should_dim_when_idle_is_false() {
        let p = new_peek();
        assert!(!p.should_dim(1));
    }

    // ── PeekMode variants ──────────────────────────────────────

    #[test]
    fn mode_three_finger() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::ThreeFingerPeek, &[]);
        assert_eq!(p.mode(), Some(PeekMode::ThreeFingerPeek));
    }

    #[test]
    fn mode_alt_tab() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::AltTabHover, &[]);
        assert_eq!(p.mode(), Some(PeekMode::AltTabHover));
    }

    #[test]
    fn mode_taskbar() {
        let mut p = new_peek();
        p.start_peek(1, PeekMode::TaskbarHover, &[]);
        assert_eq!(p.mode(), Some(PeekMode::TaskbarHover));
    }

    // ── default values ─────────────────────────────────────────

    #[test]
    fn default_dim_opacity() {
        let p = new_peek();
        assert!((p.dim_opacity - 0.3).abs() < 0.01);
    }

    #[test]
    fn default_peek_timeout() {
        let p = new_peek();
        assert_eq!(p.peek_timeout_ms, 5000);
    }

    #[test]
    fn default_animation_durations() {
        let p = new_peek();
        assert_eq!(p.enter_duration_ms, 150.0);
        assert_eq!(p.exit_duration_ms, 100.0);
    }
}
