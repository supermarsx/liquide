/// Gesture types that can trigger the overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewGesture {
    HotCorner(Corner),
    ThreeFingerSwipeUp,
    SuperKey,
    CustomShortcut,
}

/// Screen corners for hot-corner activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Detects when the cursor dwells in a screen corner long enough to trigger
/// the overview.
pub struct HotCornerDetector {
    screen_width: f32,
    screen_height: f32,
    /// Pixel distance from the corner edge that counts as "in the corner".
    threshold: f32,
    /// How long (ms) the cursor must stay in the corner to trigger.
    dwell_ms: f32,
    /// Which corner the cursor is currently dwelling in, if any.
    current_corner: Option<Corner>,
    /// Accumulated dwell time in ms.
    dwell_time: f32,
    /// Whether a trigger has already been emitted for the current dwell
    /// (to avoid retriggering until the cursor leaves).
    triggered: bool,
}

impl HotCornerDetector {
    pub fn new(
        screen_width: f32,
        screen_height: f32,
        threshold: f32,
        dwell_ms: f32,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            threshold,
            dwell_ms,
            current_corner: None,
            dwell_time: 0.0,
            triggered: false,
        }
    }

    /// Call on every mouse-move event. Returns `Some(Corner)` once when the
    /// dwell threshold is reached.
    pub fn on_mouse_move(&mut self, x: f32, y: f32, dt_ms: f32) -> Option<Corner> {
        let corner = self.detect_corner(x, y);
        match (corner, self.current_corner) {
            (Some(c), Some(prev)) if c == prev => {
                // Still in the same corner — accumulate dwell time.
                if self.triggered {
                    return None;
                }
                self.dwell_time += dt_ms;
                if self.dwell_time >= self.dwell_ms {
                    self.triggered = true;
                    return Some(c);
                }
                None
            }
            (Some(c), _) => {
                // Entered a (different) corner — start fresh.
                self.current_corner = Some(c);
                self.dwell_time = dt_ms;
                self.triggered = false;
                if self.dwell_time >= self.dwell_ms {
                    self.triggered = true;
                    return Some(c);
                }
                None
            }
            (None, _) => {
                // Left the corner area — reset.
                self.current_corner = None;
                self.dwell_time = 0.0;
                self.triggered = false;
                None
            }
        }
    }

    /// Returns the corner the cursor is currently dwelling in, if any.
    pub fn current_corner(&self) -> Option<Corner> {
        self.current_corner
    }

    fn detect_corner(&self, x: f32, y: f32) -> Option<Corner> {
        let near_left = x <= self.threshold;
        let near_right = x >= self.screen_width - self.threshold;
        let near_top = y <= self.threshold;
        let near_bottom = y >= self.screen_height - self.threshold;

        match (near_left || near_right, near_top || near_bottom) {
            (true, true) => {
                let corner = match (near_left, near_top) {
                    (true, true) => Corner::TopLeft,
                    (true, false) => Corner::BottomLeft,
                    (false, true) => Corner::TopRight,
                    (false, false) => Corner::BottomRight,
                };
                Some(corner)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> HotCornerDetector {
        HotCornerDetector::new(1920.0, 1080.0, 5.0, 200.0)
    }

    #[test]
    fn no_trigger_in_center() {
        let mut d = detector();
        let result = d.on_mouse_move(960.0, 540.0, 300.0);
        assert_eq!(result, None);
    }

    #[test]
    fn top_left_after_dwell() {
        let mut d = detector();
        // Move to top-left corner.
        assert_eq!(d.on_mouse_move(1.0, 1.0, 100.0), None);
        // Still there — not enough time.
        assert_eq!(d.on_mouse_move(1.0, 1.0, 50.0), None);
        // Now enough time has passed (100 + 50 + 60 = 210 >= 200).
        assert_eq!(d.on_mouse_move(1.0, 1.0, 60.0), Some(Corner::TopLeft));
    }

    #[test]
    fn leaving_corner_resets() {
        let mut d = detector();
        d.on_mouse_move(1.0, 1.0, 150.0);
        // Move away.
        d.on_mouse_move(500.0, 500.0, 16.0);
        // Move back — should need fresh dwell.
        assert_eq!(d.on_mouse_move(1.0, 1.0, 150.0), None);
        assert_eq!(d.on_mouse_move(1.0, 1.0, 60.0), Some(Corner::TopLeft));
    }

    #[test]
    fn no_retrigger_without_leave() {
        let mut d = detector();
        d.on_mouse_move(1.0, 1.0, 100.0);
        d.on_mouse_move(1.0, 1.0, 110.0); // triggers
        // Should not trigger again.
        assert_eq!(d.on_mouse_move(1.0, 1.0, 500.0), None);
    }

    #[test]
    fn bottom_right_corner() {
        let mut d = detector();
        assert_eq!(d.on_mouse_move(1919.0, 1079.0, 250.0), Some(Corner::BottomRight));
    }

    #[test]
    fn top_right_corner() {
        let mut d = detector();
        assert_eq!(d.on_mouse_move(1919.0, 1.0, 250.0), Some(Corner::TopRight));
    }

    #[test]
    fn bottom_left_corner() {
        let mut d = detector();
        assert_eq!(d.on_mouse_move(1.0, 1079.0, 250.0), Some(Corner::BottomLeft));
    }

    #[test]
    fn switching_corners_resets_dwell() {
        let mut d = detector();
        d.on_mouse_move(1.0, 1.0, 150.0); // top-left, 150ms
        d.on_mouse_move(1919.0, 1.0, 150.0); // switch to top-right, reset
        // Only 150ms in top-right, not enough.
        assert_eq!(d.on_mouse_move(1919.0, 1.0, 40.0), None);
        // Now 150+40 = 190, still not enough.
        assert_eq!(d.on_mouse_move(1919.0, 1.0, 20.0), Some(Corner::TopRight));
    }

    #[test]
    fn current_corner_tracks_state() {
        let mut d = detector();
        assert_eq!(d.current_corner(), None);
        d.on_mouse_move(1.0, 1.0, 10.0);
        assert_eq!(d.current_corner(), Some(Corner::TopLeft));
        d.on_mouse_move(500.0, 500.0, 10.0);
        assert_eq!(d.current_corner(), None);
    }

    #[test]
    fn zero_dwell_triggers_immediately() {
        let mut d = HotCornerDetector::new(1920.0, 1080.0, 5.0, 0.0);
        assert_eq!(d.on_mouse_move(1.0, 1.0, 0.0), Some(Corner::TopLeft));
    }
}
