//! Auto-scroll support for drag-and-drop.
//!
//! When the drag cursor is near the edge of a scrollable area, auto-scroll
//! kicks in to reveal more content. [`AutoScrollZone`] provides the detection
//! logic; the actual scrolling is performed by the widget/shell.

/// Direction to auto-scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Axis-aligned bounding box for a scrollable region.
#[derive(Debug, Clone, Copy)]
pub struct ScrollBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ScrollBounds {
    /// Create new scroll bounds.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether the point (px, py) is inside these bounds.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }
}

/// Auto-scroll zone detector.
///
/// Detects when the cursor is within a margin of a scrollable area's edges,
/// and returns the appropriate [`ScrollDirection`].
#[derive(Debug, Clone)]
pub struct AutoScrollZone {
    /// Width of the edge margin (in pixels) that triggers auto-scroll.
    pub margin: f32,
    /// Speed multiplier (pixels per tick at the edge).
    pub speed: f32,
}

impl AutoScrollZone {
    /// Create a new auto-scroll zone with the given margin.
    #[must_use]
    pub fn new(margin: f32) -> Self {
        Self {
            margin,
            speed: 8.0,
        }
    }

    /// Create a new auto-scroll zone with custom margin and speed.
    #[must_use]
    pub fn with_speed(margin: f32, speed: f32) -> Self {
        Self { margin, speed }
    }

    /// Check whether the cursor position triggers auto-scrolling within
    /// the given scrollable bounds.
    ///
    /// Returns the direction to scroll, or `None` if the cursor is not
    /// in any auto-scroll zone.
    #[must_use]
    pub fn check_auto_scroll(
        &self,
        x: f32,
        y: f32,
        bounds: ScrollBounds,
    ) -> Option<ScrollDirection> {
        if !bounds.contains(x, y) {
            return None;
        }

        let rel_x = x - bounds.x;
        let rel_y = y - bounds.y;

        // Check vertical edges first (more common scrolling direction)
        if rel_y < self.margin {
            return Some(ScrollDirection::Up);
        }
        if rel_y > bounds.height - self.margin {
            return Some(ScrollDirection::Down);
        }

        // Check horizontal edges
        if rel_x < self.margin {
            return Some(ScrollDirection::Left);
        }
        if rel_x > bounds.width - self.margin {
            return Some(ScrollDirection::Right);
        }

        None
    }

    /// Compute the scroll speed based on how close the cursor is to the edge.
    ///
    /// Returns a value between 0.0 (at the inner edge of the margin) and
    /// `self.speed` (at the outer edge). Returns 0.0 if not in a scroll zone.
    #[must_use]
    pub fn scroll_speed(
        &self,
        x: f32,
        y: f32,
        bounds: ScrollBounds,
    ) -> f32 {
        if self.margin <= 0.0 || !bounds.contains(x, y) {
            return 0.0;
        }

        let rel_x = x - bounds.x;
        let rel_y = y - bounds.y;

        // Distance from the nearest edge (0 = at edge, margin = at inner boundary)
        let edge_dist = rel_y
            .min(bounds.height - rel_y)
            .min(rel_x)
            .min(bounds.width - rel_x);

        if edge_dist >= self.margin {
            return 0.0;
        }

        // Linear ramp: speed increases as we get closer to the edge
        let t = 1.0 - (edge_dist / self.margin);
        self.speed * t
    }
}

impl Default for AutoScrollZone {
    fn default() -> Self {
        Self::new(30.0)
    }
}

/// Free function for quick auto-scroll checks without creating an `AutoScrollZone`.
///
/// Uses the given `margin` and returns the scroll direction if the cursor
/// is within that margin of any edge of `bounds`.
#[must_use]
pub fn check_auto_scroll(
    x: f32,
    y: f32,
    bounds: ScrollBounds,
    margin: f32,
) -> Option<ScrollDirection> {
    AutoScrollZone::new(margin).check_auto_scroll(x, y, bounds)
}

/// Configuration for auto-scroll behavior during drag operations.
///
/// Controls edge zone size, base scroll speed, and acceleration when
/// the cursor dwells near a container edge.
#[derive(Debug, Clone)]
pub struct AutoScrollConfig {
    /// Width of the edge zone in pixels that triggers auto-scrolling.
    pub edge_zone_size: f32,
    /// Base scroll speed in pixels per second.
    pub scroll_speed: f32,
    /// Acceleration factor: speed ramps up the longer the cursor stays
    /// in the edge zone. 1.0 = no acceleration, 2.0 = double after 1 second.
    pub acceleration: f32,
    /// Maximum speed multiplier (caps the acceleration).
    pub max_speed_multiplier: f32,
}

impl AutoScrollConfig {
    /// Create a new auto-scroll config.
    #[must_use]
    pub fn new(edge_zone_size: f32, scroll_speed: f32, acceleration: f32) -> Self {
        Self {
            edge_zone_size,
            scroll_speed,
            acceleration,
            max_speed_multiplier: 5.0,
        }
    }

    /// Set the maximum speed multiplier.
    #[must_use]
    pub fn with_max_multiplier(mut self, max: f32) -> Self {
        self.max_speed_multiplier = max;
        self
    }
}

impl Default for AutoScrollConfig {
    fn default() -> Self {
        Self {
            edge_zone_size: 40.0,
            scroll_speed: 300.0,
            acceleration: 1.5,
            max_speed_multiplier: 5.0,
        }
    }
}

/// Scroll delta produced by an auto-scroll tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollDelta {
    /// Horizontal scroll delta (positive = scroll right / content moves left).
    pub dx: f32,
    /// Vertical scroll delta (positive = scroll down / content moves up).
    pub dy: f32,
}

impl ScrollDelta {
    /// Create a new scroll delta.
    #[must_use]
    pub fn new(dx: f32, dy: f32) -> Self {
        Self { dx, dy }
    }

    /// Whether both deltas are zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.dx.abs() < f32::EPSILON && self.dy.abs() < f32::EPSILON
    }
}

/// Tracks auto-scroll state during a drag operation.
///
/// Call [`tick`](AutoScrollState::tick) each frame with the cursor position
/// and container rect. When the cursor is near an edge, the state produces
/// a [`ScrollDelta`] whose magnitude increases over time (acceleration).
pub struct AutoScrollState {
    config: AutoScrollConfig,
    /// Accumulated dwell time (in seconds) while in an edge zone.
    dwell_time: f32,
    /// The direction(s) detected on the last tick.
    active_directions: ActiveEdges,
}

/// Bit flags for which edges the cursor is near.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ActiveEdges {
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
}

impl ActiveEdges {
    fn any_active(&self) -> bool {
        self.top || self.bottom || self.left || self.right
    }
}

impl AutoScrollState {
    /// Create a new auto-scroll state with the given configuration.
    #[must_use]
    pub fn new(config: AutoScrollConfig) -> Self {
        Self {
            config,
            dwell_time: 0.0,
            active_directions: ActiveEdges::default(),
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(AutoScrollConfig::default())
    }

    /// Reset the dwell timer (e.g., when a drag starts or cursor leaves all zones).
    pub fn reset(&mut self) {
        self.dwell_time = 0.0;
        self.active_directions = ActiveEdges::default();
    }

    /// Returns the accumulated dwell time in seconds.
    #[must_use]
    pub fn dwell_time(&self) -> f32 {
        self.dwell_time
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &AutoScrollConfig {
        &self.config
    }

    /// Detect which edges the cursor is near and produce a scroll delta.
    ///
    /// `cursor_pos` is `(x, y)` in the same coordinate space as `container_rect`.
    /// `dt` is the elapsed time in seconds since the last tick.
    ///
    /// Returns `Some(ScrollDelta)` if the cursor is in an edge zone, or `None`
    /// if the cursor is in the safe interior of the container (or outside it).
    #[must_use]
    pub fn tick(
        &mut self,
        cursor_pos: (f32, f32),
        container_rect: ScrollBounds,
        dt: f32,
    ) -> Option<ScrollDelta> {
        let (cx, cy) = cursor_pos;
        let zone = self.config.edge_zone_size;

        if !container_rect.contains(cx, cy) || zone <= 0.0 {
            self.reset();
            return None;
        }

        let rel_x = cx - container_rect.x;
        let rel_y = cy - container_rect.y;

        let mut edges = ActiveEdges::default();
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;

        // Top edge proximity
        if rel_y < zone {
            edges.top = true;
            let proximity = 1.0 - (rel_y / zone); // 1.0 at edge, 0.0 at inner boundary
            dy = -proximity;
        }
        // Bottom edge proximity
        if rel_y > container_rect.height - zone {
            edges.bottom = true;
            let proximity = 1.0 - ((container_rect.height - rel_y) / zone);
            dy = proximity;
        }
        // Left edge proximity
        if rel_x < zone {
            edges.left = true;
            let proximity = 1.0 - (rel_x / zone);
            dx = -proximity;
        }
        // Right edge proximity
        if rel_x > container_rect.width - zone {
            edges.right = true;
            let proximity = 1.0 - ((container_rect.width - rel_x) / zone);
            dx = proximity;
        }

        if !edges.any_active() {
            self.reset();
            return None;
        }

        // Accumulate dwell time
        self.dwell_time += dt;
        self.active_directions = edges;

        // Compute speed multiplier from acceleration and dwell time.
        // multiplier = min(1 + (acceleration - 1) * dwell_time, max_multiplier)
        let accel_factor = 1.0
            + (self.config.acceleration - 1.0) * self.dwell_time;
        let multiplier = accel_factor.min(self.config.max_speed_multiplier);

        let speed = self.config.scroll_speed * multiplier * dt;

        Some(ScrollDelta {
            dx: dx * speed,
            dy: dy * speed,
        })
    }

    /// Whether the cursor is currently in any edge zone.
    #[must_use]
    pub fn is_scrolling(&self) -> bool {
        self.active_directions.any_active()
    }

    /// Returns the currently active scroll directions.
    #[must_use]
    pub fn active_scroll_directions(&self) -> Vec<ScrollDirection> {
        let mut dirs = Vec::new();
        if self.active_directions.top {
            dirs.push(ScrollDirection::Up);
        }
        if self.active_directions.bottom {
            dirs.push(ScrollDirection::Down);
        }
        if self.active_directions.left {
            dirs.push(ScrollDirection::Left);
        }
        if self.active_directions.right {
            dirs.push(ScrollDirection::Right);
        }
        dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bounds() -> ScrollBounds {
        ScrollBounds::new(100.0, 100.0, 400.0, 300.0)
    }

    #[test]
    fn test_no_scroll_center() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Center of bounds: no scroll
        assert_eq!(zone.check_auto_scroll(300.0, 250.0, bounds), None);
    }

    #[test]
    fn test_scroll_top() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Near top edge (y = 105, within 30px margin)
        assert_eq!(
            zone.check_auto_scroll(300.0, 105.0, bounds),
            Some(ScrollDirection::Up)
        );
    }

    #[test]
    fn test_scroll_bottom() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Near bottom edge (y = 395, within 30px of 400)
        assert_eq!(
            zone.check_auto_scroll(300.0, 395.0, bounds),
            Some(ScrollDirection::Down)
        );
    }

    #[test]
    fn test_scroll_left() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Near left edge (x = 105), but not near top/bottom
        assert_eq!(
            zone.check_auto_scroll(105.0, 250.0, bounds),
            Some(ScrollDirection::Left)
        );
    }

    #[test]
    fn test_scroll_right() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Near right edge (x = 495), but not near top/bottom
        assert_eq!(
            zone.check_auto_scroll(495.0, 250.0, bounds),
            Some(ScrollDirection::Right)
        );
    }

    #[test]
    fn test_outside_bounds() {
        let zone = AutoScrollZone::new(30.0);
        let bounds = test_bounds();
        // Outside bounds entirely
        assert_eq!(zone.check_auto_scroll(50.0, 50.0, bounds), None);
        assert_eq!(zone.check_auto_scroll(600.0, 500.0, bounds), None);
    }

    #[test]
    fn test_scroll_speed_center() {
        let zone = AutoScrollZone::with_speed(30.0, 10.0);
        let bounds = test_bounds();
        let speed = zone.scroll_speed(300.0, 250.0, bounds);
        assert!((speed - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_scroll_speed_at_edge() {
        let zone = AutoScrollZone::with_speed(30.0, 10.0);
        let bounds = test_bounds();
        // At the very top edge (y=100.0, 0 pixels from edge)
        let speed = zone.scroll_speed(300.0, 100.0, bounds);
        assert!((speed - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_scroll_speed_midway() {
        let zone = AutoScrollZone::with_speed(30.0, 10.0);
        let bounds = test_bounds();
        // 15 pixels from top edge (y=115.0, half the margin)
        let speed = zone.scroll_speed(300.0, 115.0, bounds);
        assert!((speed - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_free_function() {
        let bounds = test_bounds();
        assert_eq!(
            check_auto_scroll(300.0, 105.0, bounds, 30.0),
            Some(ScrollDirection::Up)
        );
        assert_eq!(check_auto_scroll(300.0, 250.0, bounds, 30.0), None);
    }

    #[test]
    fn test_bounds_contains() {
        let b = ScrollBounds::new(10.0, 20.0, 100.0, 50.0);
        assert!(b.contains(10.0, 20.0)); // top-left corner
        assert!(b.contains(50.0, 40.0)); // center
        assert!(!b.contains(110.0, 20.0)); // right edge (exclusive)
        assert!(!b.contains(9.9, 20.0)); // just outside left
    }

    #[test]
    fn test_zero_margin() {
        let zone = AutoScrollZone::new(0.0);
        let bounds = test_bounds();
        // Zero margin means no auto-scroll zones
        assert_eq!(zone.check_auto_scroll(100.0, 100.0, bounds), None);
        assert!((zone.scroll_speed(100.0, 100.0, bounds) - 0.0).abs() < f32::EPSILON);
    }

    // ---- AutoScrollConfig tests ----

    #[test]
    fn test_config_default() {
        let cfg = AutoScrollConfig::default();
        assert!((cfg.edge_zone_size - 40.0).abs() < f32::EPSILON);
        assert!((cfg.scroll_speed - 300.0).abs() < f32::EPSILON);
        assert!((cfg.acceleration - 1.5).abs() < f32::EPSILON);
        assert!((cfg.max_speed_multiplier - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_custom() {
        let cfg = AutoScrollConfig::new(20.0, 100.0, 2.0);
        assert!((cfg.edge_zone_size - 20.0).abs() < f32::EPSILON);
        assert!((cfg.scroll_speed - 100.0).abs() < f32::EPSILON);
        assert!((cfg.acceleration - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_with_max_multiplier() {
        let cfg = AutoScrollConfig::new(20.0, 100.0, 2.0).with_max_multiplier(3.0);
        assert!((cfg.max_speed_multiplier - 3.0).abs() < f32::EPSILON);
    }

    // ---- ScrollDelta tests ----

    #[test]
    fn test_scroll_delta_new() {
        let d = ScrollDelta::new(1.5, -2.5);
        assert!((d.dx - 1.5).abs() < f32::EPSILON);
        assert!((d.dy - (-2.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_scroll_delta_is_zero() {
        assert!(ScrollDelta::new(0.0, 0.0).is_zero());
        assert!(!ScrollDelta::new(1.0, 0.0).is_zero());
        assert!(!ScrollDelta::new(0.0, -0.5).is_zero());
    }

    // ---- AutoScrollState tests ----

    #[test]
    fn test_state_center_no_scroll() {
        let bounds = test_bounds();
        let mut state = AutoScrollState::with_defaults();
        // Cursor in center — no scroll
        let result = state.tick((300.0, 250.0), bounds, 0.016);
        assert!(result.is_none());
        assert!(!state.is_scrolling());
    }

    #[test]
    fn test_state_top_edge_scrolls_up() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0); // no acceleration
        let mut state = AutoScrollState::new(cfg);
        // Cursor near top edge: y=110 is 10px from top (within 40px zone)
        let result = state.tick((300.0, 110.0), bounds, 1.0);
        let delta = result.unwrap();
        assert!(delta.dy < 0.0, "should scroll up (negative dy)");
        assert!(delta.dx.abs() < f32::EPSILON, "no horizontal scroll");
        assert!(state.is_scrolling());
    }

    #[test]
    fn test_state_bottom_edge_scrolls_down() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0);
        let mut state = AutoScrollState::new(cfg);
        // Cursor near bottom edge: y=390 is 10px from bottom (within 40px zone)
        let result = state.tick((300.0, 390.0), bounds, 1.0);
        let delta = result.unwrap();
        assert!(delta.dy > 0.0, "should scroll down (positive dy)");
    }

    #[test]
    fn test_state_left_edge_scrolls_left() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0);
        let mut state = AutoScrollState::new(cfg);
        // x=110 is 10px from left edge
        let result = state.tick((110.0, 250.0), bounds, 1.0);
        let delta = result.unwrap();
        assert!(delta.dx < 0.0, "should scroll left (negative dx)");
    }

    #[test]
    fn test_state_right_edge_scrolls_right() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0);
        let mut state = AutoScrollState::new(cfg);
        // x=490 is 10px from right edge (right = 100+400=500)
        let result = state.tick((490.0, 250.0), bounds, 1.0);
        let delta = result.unwrap();
        assert!(delta.dx > 0.0, "should scroll right (positive dx)");
    }

    #[test]
    fn test_state_corner_scrolls_diagonal() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0);
        let mut state = AutoScrollState::new(cfg);
        // Top-left corner: near both top and left edge
        let result = state.tick((110.0, 110.0), bounds, 1.0);
        let delta = result.unwrap();
        assert!(delta.dx < 0.0, "should scroll left");
        assert!(delta.dy < 0.0, "should scroll up");
        let dirs = state.active_scroll_directions();
        assert!(dirs.contains(&ScrollDirection::Up));
        assert!(dirs.contains(&ScrollDirection::Left));
    }

    #[test]
    fn test_state_acceleration_increases_speed() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 2.0); // acceleration = 2.0
        let mut state = AutoScrollState::new(cfg);
        // First tick at top edge
        let d1 = state.tick((300.0, 105.0), bounds, 0.5).unwrap();
        // Second tick — dwell time accumulates
        let d2 = state.tick((300.0, 105.0), bounds, 0.5).unwrap();
        // Second tick should be faster due to acceleration
        assert!(
            d2.dy.abs() > d1.dy.abs(),
            "speed should increase with dwell time"
        );
    }

    #[test]
    fn test_state_acceleration_capped_at_max() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 100.0, 10.0).with_max_multiplier(2.0);
        let mut state = AutoScrollState::new(cfg);
        // Dwell for a long time
        let _ = state.tick((300.0, 100.0), bounds, 100.0);
        let d = state.tick((300.0, 100.0), bounds, 1.0).unwrap();
        // Max multiplier is 2.0, base speed 100.0, dt=1.0, proximity=1.0
        // Expected: 100 * 2.0 * 1.0 * 1.0 = 200.0
        assert!(d.dy.abs() <= 200.0 + 0.1, "should be capped at max multiplier");
    }

    #[test]
    fn test_state_outside_bounds_resets() {
        let bounds = test_bounds();
        let mut state = AutoScrollState::with_defaults();
        // First: dwell in edge zone
        let _ = state.tick((300.0, 105.0), bounds, 1.0);
        assert!(state.dwell_time() > 0.0);

        // Move outside bounds — resets
        let _ = state.tick((50.0, 50.0), bounds, 0.016);
        assert!((state.dwell_time() - 0.0).abs() < f32::EPSILON);
        assert!(!state.is_scrolling());
    }

    #[test]
    fn test_state_center_resets_dwell() {
        let bounds = test_bounds();
        let mut state = AutoScrollState::with_defaults();
        let _ = state.tick((300.0, 105.0), bounds, 1.0);
        assert!(state.dwell_time() > 0.0);

        // Move to center — resets
        let _ = state.tick((300.0, 250.0), bounds, 0.016);
        assert!((state.dwell_time() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_state_reset() {
        let bounds = test_bounds();
        let mut state = AutoScrollState::with_defaults();
        let _ = state.tick((300.0, 105.0), bounds, 1.0);
        state.reset();
        assert!((state.dwell_time() - 0.0).abs() < f32::EPSILON);
        assert!(!state.is_scrolling());
    }

    #[test]
    fn test_state_zero_edge_zone() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(0.0, 300.0, 1.5);
        let mut state = AutoScrollState::new(cfg);
        let result = state.tick((100.0, 100.0), bounds, 0.016);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_proximity_scales_speed() {
        let bounds = test_bounds();
        let cfg = AutoScrollConfig::new(40.0, 300.0, 1.0);
        // At the very edge: maximum proximity
        let mut state1 = AutoScrollState::new(cfg.clone());
        let d_edge = state1.tick((300.0, 100.0), bounds, 1.0).unwrap();

        // 20px from edge (50% proximity within 40px zone)
        let mut state2 = AutoScrollState::new(cfg);
        let d_mid = state2.tick((300.0, 120.0), bounds, 1.0).unwrap();

        assert!(
            d_edge.dy.abs() > d_mid.dy.abs(),
            "closer to edge should produce larger delta"
        );
    }
}
