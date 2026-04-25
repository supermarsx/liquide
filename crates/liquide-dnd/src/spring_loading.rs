//! Spring-loaded folder support for drag-and-drop.
//!
//! When a drag hovers over a folder (or any expandable target) for a
//! configurable delay, the target is "spring-loaded" -- it opens
//! automatically, revealing its contents for the drop. If the cursor
//! leaves before the delay elapses, the pending open is cancelled.
//!
//! Inspired by macOS Finder spring-loaded folders and GNOME Files'
//! auto-open behavior.

/// Configuration for spring-loaded folder behavior.
#[derive(Debug, Clone)]
pub struct SpringLoadConfig {
    /// Time in milliseconds the cursor must hover before the folder opens.
    pub hover_delay_ms: u64,
    /// Whether spring-loading is enabled at all.
    pub enabled: bool,
}

impl SpringLoadConfig {
    /// Create a new configuration with the given hover delay.
    #[must_use]
    pub fn new(hover_delay_ms: u64) -> Self {
        Self {
            hover_delay_ms,
            enabled: true,
        }
    }

    /// Create a disabled configuration (spring-loading off).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            hover_delay_ms: 800,
            enabled: false,
        }
    }
}

impl Default for SpringLoadConfig {
    fn default() -> Self {
        Self {
            hover_delay_ms: 800,
            enabled: true,
        }
    }
}

/// Actions produced by the spring-load state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum SpringLoadAction {
    /// The hover delay elapsed -- open the folder at the given path.
    OpenFolder(String),
    /// The cursor left the target before the delay -- cancel the pending open.
    CancelOpen,
}

/// A bounding rectangle for a spring-load target.
#[derive(Debug, Clone, Copy)]
pub struct TargetRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl TargetRect {
    /// Create a new target rectangle.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether the point is inside this rectangle.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Tracks the state of a spring-loaded folder hover during a drag.
///
/// Call [`tick`](SpringLoadState::tick) each frame with the cursor position,
/// the target folder's rect and path, and the elapsed time. When the
/// accumulated hover time exceeds the configured delay, `OpenFolder` is
/// returned. If the cursor moves away, `CancelOpen` is returned once.
pub struct SpringLoadState {
    config: SpringLoadConfig,
    /// The path of the folder currently being hovered.
    pending_path: Option<String>,
    /// Accumulated hover time in milliseconds.
    hover_time_ms: f64,
    /// Whether the folder has already been opened (avoid re-firing).
    fired: bool,
}

impl SpringLoadState {
    /// Create a new spring-load state with the given configuration.
    #[must_use]
    pub fn new(config: SpringLoadConfig) -> Self {
        Self {
            config,
            pending_path: None,
            hover_time_ms: 0.0,
            fired: false,
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(SpringLoadConfig::default())
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &SpringLoadConfig {
        &self.config
    }

    /// Whether a folder open is pending (hovering but not yet fired).
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending_path.is_some() && !self.fired
    }

    /// The path currently being hovered, if any.
    #[must_use]
    pub fn pending_path(&self) -> Option<&str> {
        self.pending_path.as_deref()
    }

    /// The accumulated hover time in milliseconds.
    #[must_use]
    pub fn hover_time_ms(&self) -> f64 {
        self.hover_time_ms
    }

    /// Progress toward the open threshold, as a fraction 0.0..=1.0.
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.config.hover_delay_ms == 0 {
            return 1.0;
        }
        (self.hover_time_ms as f32 / self.config.hover_delay_ms as f32).min(1.0)
    }

    /// Reset the state machine (e.g., when a drag ends).
    pub fn reset(&mut self) {
        self.pending_path = None;
        self.hover_time_ms = 0.0;
        self.fired = false;
    }

    /// Tick the spring-load state machine.
    ///
    /// - `cursor_pos`: current cursor `(x, y)`.
    /// - `target_rect`: the bounding box of the folder/target.
    /// - `target_path`: the path/identifier of the folder.
    /// - `dt_ms`: elapsed time in milliseconds since the last tick.
    ///
    /// Returns `Some(SpringLoadAction)` when state changes:
    /// - `OpenFolder` when the hover delay is exceeded.
    /// - `CancelOpen` when the cursor leaves a pending target.
    /// - `None` while hovering but delay not yet met, or when disabled.
    #[must_use]
    pub fn tick(
        &mut self,
        cursor_pos: (f32, f32),
        target_rect: TargetRect,
        target_path: &str,
        dt_ms: f64,
    ) -> Option<SpringLoadAction> {
        if !self.config.enabled {
            return None;
        }

        let (cx, cy) = cursor_pos;
        let inside = target_rect.contains(cx, cy);

        if inside {
            // Check if we switched to a different target
            let same_target = self
                .pending_path
                .as_deref()
                .is_some_and(|p| p == target_path);

            if !same_target {
                // New target — start fresh
                self.pending_path = Some(target_path.to_string());
                self.hover_time_ms = 0.0;
                self.fired = false;
            }

            if self.fired {
                // Already fired for this target — don't fire again
                return None;
            }

            self.hover_time_ms += dt_ms;

            if self.hover_time_ms >= self.config.hover_delay_ms as f64 {
                self.fired = true;
                return Some(SpringLoadAction::OpenFolder(target_path.to_string()));
            }

            None
        } else {
            // Cursor left the target
            if self.pending_path.is_some() && !self.fired {
                self.reset();
                return Some(SpringLoadAction::CancelOpen);
            }
            // If already fired or no pending, just reset silently
            if self.pending_path.is_some() {
                self.reset();
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rect() -> TargetRect {
        TargetRect::new(100.0, 100.0, 200.0, 50.0)
    }

    // ---- SpringLoadConfig tests ----

    #[test]
    fn test_config_default() {
        let cfg = SpringLoadConfig::default();
        assert_eq!(cfg.hover_delay_ms, 800);
        assert!(cfg.enabled);
    }

    #[test]
    fn test_config_custom() {
        let cfg = SpringLoadConfig::new(500);
        assert_eq!(cfg.hover_delay_ms, 500);
        assert!(cfg.enabled);
    }

    #[test]
    fn test_config_disabled() {
        let cfg = SpringLoadConfig::disabled();
        assert!(!cfg.enabled);
    }

    // ---- TargetRect tests ----

    #[test]
    fn test_target_rect_contains() {
        let r = test_rect();
        assert!(r.contains(100.0, 100.0)); // top-left
        assert!(r.contains(200.0, 125.0)); // center
        assert!(!r.contains(300.0, 100.0)); // right edge exclusive
        assert!(!r.contains(99.0, 100.0)); // outside left
        assert!(!r.contains(200.0, 150.0)); // below
    }

    // ---- SpringLoadState tests ----

    #[test]
    fn test_state_disabled_returns_none() {
        let mut state = SpringLoadState::new(SpringLoadConfig::disabled());
        let rect = test_rect();
        let result = state.tick((150.0, 125.0), rect, "/home/folder", 1000.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_cursor_outside_returns_none() {
        let mut state = SpringLoadState::with_defaults();
        let rect = test_rect();
        let result = state.tick((50.0, 50.0), rect, "/home/folder", 16.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_hover_accumulates_time() {
        let mut state = SpringLoadState::with_defaults();
        let rect = test_rect();
        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 100.0);
        assert!((state.hover_time_ms() - 100.0).abs() < 0.01);
        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 200.0);
        assert!((state.hover_time_ms() - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_state_fires_after_delay() {
        let cfg = SpringLoadConfig::new(500);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        // Hover for 400ms — not yet
        let r1 = state.tick((150.0, 125.0), rect, "/home/folder", 400.0);
        assert!(r1.is_none());
        assert!(state.is_pending());

        // Hover for another 200ms — total 600ms > 500ms delay
        let r2 = state.tick((150.0, 125.0), rect, "/home/folder", 200.0);
        assert_eq!(
            r2,
            Some(SpringLoadAction::OpenFolder("/home/folder".into()))
        );
        assert!(!state.is_pending()); // fired
    }

    #[test]
    fn test_state_does_not_refire() {
        let cfg = SpringLoadConfig::new(100);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 200.0); // fires
        let r = state.tick((150.0, 125.0), rect, "/home/folder", 100.0);
        assert!(r.is_none()); // should not re-fire
    }

    #[test]
    fn test_state_cancel_on_leave() {
        let cfg = SpringLoadConfig::new(500);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        // Start hovering
        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 200.0);
        assert!(state.is_pending());

        // Cursor leaves
        let r = state.tick((50.0, 50.0), rect, "/home/folder", 16.0);
        assert_eq!(r, Some(SpringLoadAction::CancelOpen));
        assert!(!state.is_pending());
    }

    #[test]
    fn test_state_switch_target_resets_timer() {
        let cfg = SpringLoadConfig::new(500);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        // Hover over folder A for 400ms
        let _ = state.tick((150.0, 125.0), rect, "/home/a", 400.0);
        assert!((state.hover_time_ms() - 400.0).abs() < 0.01);

        // Switch to folder B — timer resets
        let _ = state.tick((150.0, 125.0), rect, "/home/b", 100.0);
        assert!((state.hover_time_ms() - 100.0).abs() < 0.01);
        assert_eq!(state.pending_path(), Some("/home/b"));
    }

    #[test]
    fn test_state_progress() {
        let cfg = SpringLoadConfig::new(800);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        assert!((state.progress() - 0.0).abs() < f32::EPSILON);

        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 400.0);
        assert!((state.progress() - 0.5).abs() < 0.01);

        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 400.0);
        assert!((state.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_state_progress_zero_delay() {
        let cfg = SpringLoadConfig::new(0);
        let state = SpringLoadState::new(cfg);
        assert!((state.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_state_reset() {
        let mut state = SpringLoadState::with_defaults();
        let rect = test_rect();
        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 200.0);
        state.reset();
        assert!(state.pending_path().is_none());
        assert!((state.hover_time_ms() - 0.0).abs() < 0.01);
        assert!(!state.is_pending());
    }

    #[test]
    fn test_state_leave_after_fire_no_cancel() {
        let cfg = SpringLoadConfig::new(100);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        // Fire
        let _ = state.tick((150.0, 125.0), rect, "/home/folder", 200.0);
        // Leave — should not emit CancelOpen since it already fired
        let r = state.tick((50.0, 50.0), rect, "/home/folder", 16.0);
        assert!(r.is_none());
    }

    #[test]
    fn test_state_reuse_after_reset() {
        let cfg = SpringLoadConfig::new(200);
        let mut state = SpringLoadState::new(cfg);
        let rect = test_rect();

        // First cycle: fire
        let r = state.tick((150.0, 125.0), rect, "/home/a", 300.0);
        assert_eq!(r, Some(SpringLoadAction::OpenFolder("/home/a".into())));

        // Reset and start new cycle
        state.reset();
        let r = state.tick((150.0, 125.0), rect, "/home/b", 300.0);
        assert_eq!(r, Some(SpringLoadAction::OpenFolder("/home/b".into())));
    }
}
