//! Screen-edge gestures and hot corner detection.
//!
//! Modeled after GNOME Shell / Mutter edge triggers: swipes that originate
//! from the screen perimeter, and pressure-based hot corners that fire after
//! the pointer dwells at a corner.

/// Which screen edge was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeGesture {
    TopEdgePull,
    BottomEdgePull,
    LeftEdgePull,
    RightEdgePull,
}

/// Configuration for screen-edge gesture detection.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// Master enable.
    pub enabled: bool,
    /// Maximum distance (px) from the edge to count as an edge-start.
    pub trigger_distance: f64,
    /// Size of the activation zone along the edge (px).
    pub activation_zone_size: f64,
    /// Minimum drag distance (px) before the gesture fires.
    pub min_drag_distance: f64,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_distance: 12.0,
            activation_zone_size: 40.0,
            min_drag_distance: 30.0,
        }
    }
}

/// Result of an edge detection check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeDetection {
    pub gesture: EdgeGesture,
    /// 0.0 = just started, 1.0 = fully dragged across screen.
    pub progress: f64,
}

/// Tracks pointer/touch to detect edge-originating swipes.
pub struct EdgeDetector {
    config: EdgeConfig,
    screen_width: f64,
    screen_height: f64,
    /// Start position if the gesture started from an edge.
    active: Option<(EdgeGesture, f64, f64)>,
}

impl EdgeDetector {
    pub fn new(config: EdgeConfig, screen_width: f64, screen_height: f64) -> Self {
        Self { config, screen_width, screen_height, active: None }
    }

    pub fn set_screen_size(&mut self, width: f64, height: f64) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Call when a touch/pointer press begins.
    pub fn begin(&mut self, x: f64, y: f64) {
        if !self.config.enabled {
            return;
        }
        self.active = self.classify_edge(x, y).map(|e| (e, x, y));
    }

    /// Call on pointer/touch motion. Returns detection result if an edge gesture is active.
    pub fn update(&self, x: f64, y: f64) -> Option<EdgeDetection> {
        let (gesture, sx, sy) = self.active?;
        let progress = self.compute_progress(gesture, sx, sy, x, y);
        if progress * self.screen_span(gesture) < self.config.min_drag_distance {
            return None;
        }
        Some(EdgeDetection { gesture, progress: progress.clamp(0.0, 1.0) })
    }

    /// Call on pointer/touch release. Returns final detection if threshold met.
    pub fn end(&mut self, x: f64, y: f64) -> Option<EdgeDetection> {
        let result = self.update(x, y);
        self.active = None;
        result
    }

    /// Cancel any active edge tracking.
    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Whether an edge gesture is currently being tracked.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    fn classify_edge(&self, x: f64, y: f64) -> Option<EdgeGesture> {
        let d = self.config.trigger_distance;
        if x <= d { Some(EdgeGesture::LeftEdgePull) }
        else if x >= self.screen_width - d { Some(EdgeGesture::RightEdgePull) }
        else if y <= d { Some(EdgeGesture::TopEdgePull) }
        else if y >= self.screen_height - d { Some(EdgeGesture::BottomEdgePull) }
        else { None }
    }

    fn compute_progress(&self, gesture: EdgeGesture, sx: f64, sy: f64, x: f64, y: f64) -> f64 {
        match gesture {
            EdgeGesture::LeftEdgePull => (x - sx) / self.screen_width,
            EdgeGesture::RightEdgePull => (sx - x) / self.screen_width,
            EdgeGesture::TopEdgePull => (y - sy) / self.screen_height,
            EdgeGesture::BottomEdgePull => (sy - y) / self.screen_height,
        }
    }

    fn screen_span(&self, gesture: EdgeGesture) -> f64 {
        match gesture {
            EdgeGesture::LeftEdgePull | EdgeGesture::RightEdgePull => self.screen_width,
            EdgeGesture::TopEdgePull | EdgeGesture::BottomEdgePull => self.screen_height,
        }
    }
}

// ---------------------------------------------------------------------------
// Hot corners
// ---------------------------------------------------------------------------

/// Corner of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Action to perform when a hot corner triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotCornerAction {
    Overview,
    ShowDesktop,
    LaunchApp(String),
    None,
}

/// Per-corner configuration.
#[derive(Debug, Clone)]
pub struct HotCornerConfig {
    /// Milliseconds the pointer must dwell before triggering.
    pub delay_ms: u64,
    pub top_left: HotCornerAction,
    pub top_right: HotCornerAction,
    pub bottom_left: HotCornerAction,
    pub bottom_right: HotCornerAction,
    /// Size (px) of the corner hit region.
    pub corner_size: f64,
}

impl Default for HotCornerConfig {
    fn default() -> Self {
        Self {
            delay_ms: 400,
            top_left: HotCornerAction::Overview,
            top_right: HotCornerAction::None,
            bottom_left: HotCornerAction::ShowDesktop,
            bottom_right: HotCornerAction::None,
            corner_size: 3.0,
        }
    }
}

/// Hot-corner detector.
pub struct HotCornerDetector {
    config: HotCornerConfig,
    screen_width: f64,
    screen_height: f64,
    /// Which corner the pointer is dwelling in, and the timestamp (microseconds) it entered.
    dwelling: Option<(HotCorner, u64)>,
    /// Suppress re-trigger until the pointer leaves the corner.
    fired: bool,
}

impl HotCornerDetector {
    pub fn new(config: HotCornerConfig, screen_width: f64, screen_height: f64) -> Self {
        Self { config, screen_width, screen_height, dwelling: None, fired: false }
    }

    /// Update pointer position. `timestamp_us` is a monotonic microsecond clock.
    /// Returns the triggered action, if any.
    pub fn update(&mut self, x: f64, y: f64, timestamp_us: u64) -> Option<HotCornerAction> {
        let corner = self.hit_test(x, y);
        match (corner, &self.dwelling) {
            (Some(c), Some((dc, enter_us))) if c == *dc => {
                if self.fired {
                    return None;
                }
                let elapsed_ms = (timestamp_us - enter_us) / 1000;
                if elapsed_ms >= self.config.delay_ms {
                    self.fired = true;
                    let action = self.action_for(c);
                    if action != HotCornerAction::None { Some(action) } else { None }
                } else {
                    None
                }
            }
            (Some(c), _) => {
                self.dwelling = Some((c, timestamp_us));
                self.fired = false;
                None
            }
            (None, _) => {
                self.dwelling = None;
                self.fired = false;
                None
            }
        }
    }

    fn hit_test(&self, x: f64, y: f64) -> Option<HotCorner> {
        let s = self.config.corner_size;
        let is_left = x <= s;
        let is_right = x >= self.screen_width - s;
        let is_top = y <= s;
        let is_bottom = y >= self.screen_height - s;

        if is_left && is_top { Some(HotCorner::TopLeft) }
        else if is_right && is_top { Some(HotCorner::TopRight) }
        else if is_left && is_bottom { Some(HotCorner::BottomLeft) }
        else if is_right && is_bottom { Some(HotCorner::BottomRight) }
        else { None }
    }

    fn action_for(&self, corner: HotCorner) -> HotCornerAction {
        match corner {
            HotCorner::TopLeft => self.config.top_left.clone(),
            HotCorner::TopRight => self.config.top_right.clone(),
            HotCorner::BottomLeft => self.config.bottom_left.clone(),
            HotCorner::BottomRight => self.config.bottom_right.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_left_detected() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(5.0, 500.0);
        assert!(det.is_active());
        let r = det.end(100.0, 500.0);
        assert!(r.is_some());
        assert_eq!(r.unwrap().gesture, EdgeGesture::LeftEdgePull);
    }

    #[test]
    fn edge_right_detected() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(1915.0, 500.0);
        let r = det.end(1800.0, 500.0);
        assert!(r.is_some());
        assert_eq!(r.unwrap().gesture, EdgeGesture::RightEdgePull);
    }

    #[test]
    fn edge_top_detected() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(960.0, 3.0);
        let r = det.end(960.0, 100.0);
        assert!(r.is_some());
        assert_eq!(r.unwrap().gesture, EdgeGesture::TopEdgePull);
    }

    #[test]
    fn edge_bottom_detected() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(960.0, 1076.0);
        let r = det.end(960.0, 980.0);
        assert!(r.is_some());
        assert_eq!(r.unwrap().gesture, EdgeGesture::BottomEdgePull);
    }

    #[test]
    fn edge_not_from_center() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(960.0, 540.0);
        assert!(!det.is_active());
        let r = det.end(960.0, 100.0);
        assert!(r.is_none());
    }

    #[test]
    fn edge_disabled() {
        let cfg = EdgeConfig { enabled: false, ..EdgeConfig::default() };
        let mut det = EdgeDetector::new(cfg, 1920.0, 1080.0);
        det.begin(5.0, 500.0);
        assert!(!det.is_active());
    }

    #[test]
    fn edge_cancel() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(5.0, 500.0);
        assert!(det.is_active());
        det.cancel();
        assert!(!det.is_active());
    }

    #[test]
    fn edge_progress_clamped() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 1920.0, 1080.0);
        det.begin(5.0, 500.0);
        let r = det.update(2000.0, 500.0);
        assert!(r.is_some());
        assert!(r.unwrap().progress <= 1.0);
    }

    #[test]
    fn edge_insufficient_drag() {
        let cfg = EdgeConfig { min_drag_distance: 100.0, ..EdgeConfig::default() };
        let mut det = EdgeDetector::new(cfg, 1920.0, 1080.0);
        det.begin(5.0, 500.0);
        let r = det.update(15.0, 500.0); // only 10px drag
        assert!(r.is_none());
    }

    #[test]
    fn hot_corner_top_left_triggers() {
        let cfg = HotCornerConfig { delay_ms: 100, ..HotCornerConfig::default() };
        let mut hc = HotCornerDetector::new(cfg, 1920.0, 1080.0);
        // Enter corner
        assert!(hc.update(1.0, 1.0, 0).is_none());
        // After delay
        let action = hc.update(1.0, 1.0, 200_000);
        assert_eq!(action, Some(HotCornerAction::Overview));
    }

    #[test]
    fn hot_corner_no_retrigger_until_leave() {
        let cfg = HotCornerConfig { delay_ms: 10, ..HotCornerConfig::default() };
        let mut hc = HotCornerDetector::new(cfg, 1920.0, 1080.0);
        hc.update(1.0, 1.0, 0);
        hc.update(1.0, 1.0, 100_000); // triggers
        let second = hc.update(1.0, 1.0, 200_000);
        assert!(second.is_none(), "Should not re-trigger");
        // Leave and re-enter
        hc.update(500.0, 500.0, 300_000);
        hc.update(1.0, 1.0, 400_000);
        let re = hc.update(1.0, 1.0, 500_000);
        assert_eq!(re, Some(HotCornerAction::Overview));
    }

    #[test]
    fn hot_corner_none_action_not_emitted() {
        let cfg = HotCornerConfig {
            delay_ms: 0,
            top_right: HotCornerAction::None,
            corner_size: 5.0,
            ..HotCornerConfig::default()
        };
        let mut hc = HotCornerDetector::new(cfg, 1920.0, 1080.0);
        hc.update(1919.0, 1.0, 0);
        let r = hc.update(1919.0, 1.0, 100_000);
        assert!(r.is_none());
    }

    #[test]
    fn hot_corner_bottom_left() {
        let cfg = HotCornerConfig { delay_ms: 0, ..HotCornerConfig::default() };
        let mut hc = HotCornerDetector::new(cfg, 1920.0, 1080.0);
        hc.update(1.0, 1079.0, 0);
        let r = hc.update(1.0, 1079.0, 100_000);
        assert_eq!(r, Some(HotCornerAction::ShowDesktop));
    }

    #[test]
    fn hot_corner_launch_app() {
        let cfg = HotCornerConfig {
            delay_ms: 0,
            bottom_right: HotCornerAction::LaunchApp("terminal".into()),
            corner_size: 5.0,
            ..HotCornerConfig::default()
        };
        let mut hc = HotCornerDetector::new(cfg, 1920.0, 1080.0);
        hc.update(1919.0, 1079.0, 0);
        let r = hc.update(1919.0, 1079.0, 100_000);
        assert_eq!(r, Some(HotCornerAction::LaunchApp("terminal".into())));
    }

    #[test]
    fn set_screen_size() {
        let mut det = EdgeDetector::new(EdgeConfig::default(), 800.0, 600.0);
        det.set_screen_size(1920.0, 1080.0);
        det.begin(5.0, 500.0);
        assert!(det.is_active());
    }
}
