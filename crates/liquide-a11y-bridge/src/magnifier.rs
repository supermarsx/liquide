//! Screen magnification for the bridge layer.
//!
//! Provides configuration, state, and viewport-tracking logic for a screen
//! magnifier.  The magnifier can follow focus, caret, or mouse pointer, and
//! supports multiple lens shapes (full-screen, half-screen, custom rect).

// ---------------------------------------------------------------------------
// Rect (local, minimal)
// ---------------------------------------------------------------------------

/// A simple axis-aligned rectangle used for custom lens regions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Check if a point lies inside this rectangle.
    #[must_use]
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }

    /// Centre point.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

// ---------------------------------------------------------------------------
// Lens
// ---------------------------------------------------------------------------

/// The shape / region of the magnifier lens.
#[derive(Debug, Clone, PartialEq)]
pub enum MagnifierLens {
    /// Full-screen magnification.
    FullScreen,
    /// Top half of the screen.
    TopHalf,
    /// Bottom half of the screen.
    BottomHalf,
    /// Left half of the screen.
    LeftHalf,
    /// Right half of the screen.
    RightHalf,
    /// A custom rectangular region.
    Custom(Rect),
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the magnifier.
#[derive(Debug, Clone, PartialEq)]
pub struct MagnifierConfig {
    /// Zoom level (1.0 = no zoom, 2.0 = 2x, etc.).  Clamped to
    /// \[`MIN_ZOOM`, `MAX_ZOOM`\].
    pub zoom_level: f64,
    /// Whether the viewport should follow keyboard focus changes.
    pub follow_focus: bool,
    /// Whether the viewport should follow the text caret.
    pub follow_caret: bool,
    /// Whether the viewport should follow the mouse pointer.
    pub follow_mouse: bool,
    /// The lens shape.
    pub lens: MagnifierLens,
    /// Smooth scrolling factor (0.0 = instant, 1.0 = very slow).
    pub smooth_factor: f64,
}

/// Minimum zoom level.
pub const MIN_ZOOM: f64 = 1.0;
/// Maximum zoom level.
pub const MAX_ZOOM: f64 = 20.0;

impl MagnifierConfig {
    /// Create a default configuration (2x zoom, follow focus + caret +
    /// mouse, full-screen lens).
    #[must_use]
    pub fn new() -> Self {
        Self {
            zoom_level: 2.0,
            follow_focus: true,
            follow_caret: true,
            follow_mouse: true,
            lens: MagnifierLens::FullScreen,
            smooth_factor: 0.15,
        }
    }

    /// Set the zoom level, clamped to \[`MIN_ZOOM`, `MAX_ZOOM`\].
    pub fn set_zoom(&mut self, level: f64) {
        self.zoom_level = level.clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

impl Default for MagnifierConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Runtime state of the magnifier.
#[derive(Debug, Clone, PartialEq)]
pub struct MagnifierState {
    /// Current viewport centre in screen coordinates.
    pub viewport_x: f64,
    pub viewport_y: f64,
    /// Current zoom factor (may differ from `config.zoom_level` during
    /// animated transitions).
    pub zoom_factor: f64,
    /// Whether the magnifier is currently enabled.
    pub enabled: bool,
    /// Screen dimensions (needed for lens region computation).
    pub screen_width: f64,
    pub screen_height: f64,
}

impl MagnifierState {
    /// Create a new state with the given screen dimensions.
    #[must_use]
    pub fn new(screen_width: f64, screen_height: f64) -> Self {
        Self {
            viewport_x: screen_width / 2.0,
            viewport_y: screen_height / 2.0,
            zoom_factor: 1.0,
            enabled: false,
            screen_width,
            screen_height,
        }
    }

    /// Enable the magnifier, setting the zoom to the configured level.
    pub fn enable(&mut self, config: &MagnifierConfig) {
        self.enabled = true;
        self.zoom_factor = config.zoom_level;
    }

    /// Disable the magnifier, resetting the zoom to 1.0.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.zoom_factor = 1.0;
    }

    /// Toggle the magnifier on/off.
    pub fn toggle(&mut self, config: &MagnifierConfig) {
        if self.enabled {
            self.disable();
        } else {
            self.enable(config);
        }
    }

    /// Compute the visible rectangle (in screen coords) that the lens
    /// covers, given the current lens shape.
    #[must_use]
    pub fn lens_rect(&self, lens: &MagnifierLens) -> Rect {
        match lens {
            MagnifierLens::FullScreen => {
                Rect::new(0.0, 0.0, self.screen_width, self.screen_height)
            }
            MagnifierLens::TopHalf => {
                Rect::new(0.0, 0.0, self.screen_width, self.screen_height / 2.0)
            }
            MagnifierLens::BottomHalf => {
                Rect::new(
                    0.0,
                    self.screen_height / 2.0,
                    self.screen_width,
                    self.screen_height / 2.0,
                )
            }
            MagnifierLens::LeftHalf => {
                Rect::new(0.0, 0.0, self.screen_width / 2.0, self.screen_height)
            }
            MagnifierLens::RightHalf => {
                Rect::new(
                    self.screen_width / 2.0,
                    0.0,
                    self.screen_width / 2.0,
                    self.screen_height,
                )
            }
            MagnifierLens::Custom(r) => *r,
        }
    }

    /// Update the viewport to smoothly track a focus point, using
    /// `config.smooth_factor` as the interpolation weight.
    ///
    /// `focus_x` / `focus_y` are the screen coordinates of the point to
    /// track (e.g. focused widget centre, caret position, or mouse
    /// pointer).
    pub fn update_viewport(&mut self, focus_x: f64, focus_y: f64, config: &MagnifierConfig) {
        if !self.enabled {
            return;
        }

        let factor = config.smooth_factor.clamp(0.0, 1.0);

        // Linearly interpolate towards the focus point.
        self.viewport_x += (focus_x - self.viewport_x) * (1.0 - factor);
        self.viewport_y += (focus_y - self.viewport_y) * (1.0 - factor);

        // Clamp viewport so the magnified view doesn't scroll past the
        // screen edges.
        let half_w = self.screen_width / (2.0 * self.zoom_factor);
        let half_h = self.screen_height / (2.0 * self.zoom_factor);

        self.viewport_x = self.viewport_x.clamp(half_w, self.screen_width - half_w);
        self.viewport_y = self.viewport_y.clamp(half_h, self.screen_height - half_h);

        // Sync zoom to config in case it changed.
        self.zoom_factor = config.zoom_level;
    }

    /// Snap the viewport directly to a point (no smoothing).
    pub fn snap_to(&mut self, x: f64, y: f64) {
        self.viewport_x = x.clamp(0.0, self.screen_width);
        self.viewport_y = y.clamp(0.0, self.screen_height);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let c = MagnifierConfig::new();
        assert_eq!(c.zoom_level, 2.0);
        assert!(c.follow_focus);
        assert!(c.follow_caret);
        assert!(c.follow_mouse);
        assert_eq!(c.lens, MagnifierLens::FullScreen);
    }

    #[test]
    fn config_set_zoom_clamped() {
        let mut c = MagnifierConfig::new();
        c.set_zoom(0.5);
        assert_eq!(c.zoom_level, MIN_ZOOM);
        c.set_zoom(100.0);
        assert_eq!(c.zoom_level, MAX_ZOOM);
        c.set_zoom(3.5);
        assert_eq!(c.zoom_level, 3.5);
    }

    #[test]
    fn state_creation() {
        let s = MagnifierState::new(1920.0, 1080.0);
        assert_eq!(s.viewport_x, 960.0);
        assert_eq!(s.viewport_y, 540.0);
        assert_eq!(s.zoom_factor, 1.0);
        assert!(!s.enabled);
    }

    #[test]
    fn state_enable_disable() {
        let config = MagnifierConfig::new();
        let mut s = MagnifierState::new(1920.0, 1080.0);
        s.enable(&config);
        assert!(s.enabled);
        assert_eq!(s.zoom_factor, 2.0);
        s.disable();
        assert!(!s.enabled);
        assert_eq!(s.zoom_factor, 1.0);
    }

    #[test]
    fn state_toggle() {
        let config = MagnifierConfig::new();
        let mut s = MagnifierState::new(1920.0, 1080.0);
        s.toggle(&config);
        assert!(s.enabled);
        s.toggle(&config);
        assert!(!s.enabled);
    }

    #[test]
    fn lens_rect_full_screen() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let r = s.lens_rect(&MagnifierLens::FullScreen);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 1920.0);
        assert_eq!(r.height, 1080.0);
    }

    #[test]
    fn lens_rect_top_half() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let r = s.lens_rect(&MagnifierLens::TopHalf);
        assert_eq!(r.height, 540.0);
        assert_eq!(r.y, 0.0);
    }

    #[test]
    fn lens_rect_bottom_half() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let r = s.lens_rect(&MagnifierLens::BottomHalf);
        assert_eq!(r.y, 540.0);
        assert_eq!(r.height, 540.0);
    }

    #[test]
    fn lens_rect_left_half() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let r = s.lens_rect(&MagnifierLens::LeftHalf);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.width, 960.0);
    }

    #[test]
    fn lens_rect_right_half() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let r = s.lens_rect(&MagnifierLens::RightHalf);
        assert_eq!(r.x, 960.0);
        assert_eq!(r.width, 960.0);
    }

    #[test]
    fn lens_rect_custom() {
        let s = MagnifierState::new(1920.0, 1080.0);
        let custom = Rect::new(100.0, 200.0, 300.0, 400.0);
        let r = s.lens_rect(&MagnifierLens::Custom(custom));
        assert_eq!(r, custom);
    }

    #[test]
    fn update_viewport_moves_towards_focus() {
        let mut config = MagnifierConfig::new();
        config.smooth_factor = 0.0; // instant tracking
        let mut s = MagnifierState::new(1920.0, 1080.0);
        s.enable(&config);
        s.update_viewport(100.0, 100.0, &config);
        // With smooth_factor=0.0 the viewport should snap close to the
        // focus point (clamped to stay within screen bounds).
        // At 2x zoom, half_w = 1920/(2*2) = 480, half_h = 1080/(2*2) = 270
        // so viewport_x is clamped to [480, 1440], viewport_y to [270, 810].
        assert_eq!(s.viewport_x, 480.0); // clamped lower bound
        assert_eq!(s.viewport_y, 270.0); // clamped lower bound
    }

    #[test]
    fn update_viewport_noop_when_disabled() {
        let config = MagnifierConfig::new();
        let mut s = MagnifierState::new(1920.0, 1080.0);
        let orig_x = s.viewport_x;
        let orig_y = s.viewport_y;
        s.update_viewport(0.0, 0.0, &config);
        assert_eq!(s.viewport_x, orig_x);
        assert_eq!(s.viewport_y, orig_y);
    }

    #[test]
    fn snap_to() {
        let mut s = MagnifierState::new(1920.0, 1080.0);
        s.snap_to(500.0, 300.0);
        assert_eq!(s.viewport_x, 500.0);
        assert_eq!(s.viewport_y, 300.0);
    }

    #[test]
    fn snap_to_clamped() {
        let mut s = MagnifierState::new(1920.0, 1080.0);
        s.snap_to(-100.0, 2000.0);
        assert_eq!(s.viewport_x, 0.0);
        assert_eq!(s.viewport_y, 1080.0);
    }

    #[test]
    fn rect_contains() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(110.0, 20.0));
        assert!(!r.contains(9.0, 20.0));
    }

    #[test]
    fn rect_center() {
        let r = Rect::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(r.center(), (50.0, 100.0));
    }

    #[test]
    fn config_default_trait() {
        let c = MagnifierConfig::default();
        assert_eq!(c.zoom_level, 2.0);
    }
}
