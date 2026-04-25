//! Multi-touch point tracking and geometric analysis.
//!
//! Tracks up to [`MAX_TOUCHES`] simultaneous contacts and provides derived
//! geometric quantities: centroid, spread (for pinch detection), and
//! rotation angle.

/// Maximum number of simultaneous touch points tracked.
pub const MAX_TOUCHES: usize = 10;

/// A tracked touch contact.
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub timestamp: u64,
}

/// Internal per-finger record.
#[derive(Debug, Clone, Copy)]
struct Finger {
    id: u64,
    start_x: f64,
    start_y: f64,
    cur_x: f64,
    cur_y: f64,
    timestamp: u64,
}

/// Multi-touch state tracker.
pub struct MultiTouchState {
    fingers: Vec<Finger>,
    /// Cached angle at the start of a two-or-more finger gesture (radians).
    initial_angle: f64,
}

impl MultiTouchState {
    pub fn new() -> Self {
        Self {
            fingers: Vec::with_capacity(MAX_TOUCHES),
            initial_angle: 0.0,
        }
    }

    /// Register a new touch contact. Returns `false` if MAX_TOUCHES reached.
    pub fn touch_down(&mut self, id: u64, x: f64, y: f64, timestamp: u64) -> bool {
        if self.fingers.len() >= MAX_TOUCHES {
            return false;
        }
        // Remove duplicate id if any (defensive)
        self.fingers.retain(|f| f.id != id);
        self.fingers.push(Finger {
            id,
            start_x: x,
            start_y: y,
            cur_x: x,
            cur_y: y,
            timestamp,
        });

        // Recompute initial angle whenever finger count changes
        if self.fingers.len() >= 2 {
            self.initial_angle = self.raw_angle();
        }
        true
    }

    /// Update position of an existing touch contact.
    pub fn touch_move(&mut self, id: u64, x: f64, y: f64, timestamp: u64) {
        if let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) {
            f.cur_x = x;
            f.cur_y = y;
            f.timestamp = timestamp;
        }
    }

    /// Remove a touch contact.
    pub fn touch_up(&mut self, id: u64) {
        self.fingers.retain(|f| f.id != id);
        if self.fingers.len() >= 2 {
            self.initial_angle = self.raw_angle();
        }
    }

    /// Number of currently active touches.
    pub fn touch_count(&self) -> usize {
        self.fingers.len()
    }

    /// Center of all active touches.
    pub fn centroid(&self) -> (f64, f64) {
        if self.fingers.is_empty() {
            return (0.0, 0.0);
        }
        let n = self.fingers.len() as f64;
        let (sx, sy) = self
            .fingers
            .iter()
            .fold((0.0, 0.0), |(ax, ay), f| (ax + f.cur_x, ay + f.cur_y));
        (sx / n, sy / n)
    }

    /// Average distance from the centroid (useful for pinch/spread detection).
    pub fn spread(&self) -> f64 {
        if self.fingers.len() < 2 {
            return 0.0;
        }
        let (cx, cy) = self.centroid();
        let n = self.fingers.len() as f64;
        let sum: f64 = self
            .fingers
            .iter()
            .map(|f| {
                let dx = f.cur_x - cx;
                let dy = f.cur_y - cy;
                (dx * dx + dy * dy).sqrt()
            })
            .sum();
        sum / n
    }

    /// Rotation angle (radians) since the gesture start.
    ///
    /// Positive = counter-clockwise. Computed from the angle of the first
    /// finger relative to the centroid.
    pub fn rotation_angle(&self) -> f64 {
        if self.fingers.len() < 2 {
            return 0.0;
        }
        let current = self.raw_angle();
        normalize_angle(current - self.initial_angle)
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.fingers.clear();
        self.initial_angle = 0.0;
    }

    /// Get a touch point by id.
    pub fn get(&self, id: u64) -> Option<TouchPoint> {
        self.fingers
            .iter()
            .find(|f| f.id == id)
            .map(|f| TouchPoint {
                id: f.id,
                x: f.cur_x,
                y: f.cur_y,
                timestamp: f.timestamp,
            })
    }

    /// Iterate active touch points.
    pub fn active_touches(&self) -> Vec<TouchPoint> {
        self.fingers
            .iter()
            .map(|f| TouchPoint {
                id: f.id,
                x: f.cur_x,
                y: f.cur_y,
                timestamp: f.timestamp,
            })
            .collect()
    }

    /// Raw angle of first finger relative to centroid.
    fn raw_angle(&self) -> f64 {
        if self.fingers.len() < 2 {
            return 0.0;
        }
        let (cx, cy) = self.centroid();
        let f0 = &self.fingers[0];
        (f0.cur_y - cy).atan2(f0.cur_x - cx)
    }
}

impl Default for MultiTouchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize angle to [-PI, PI].
fn normalize_angle(a: f64) -> f64 {
    let mut r = a % (2.0 * std::f64::consts::PI);
    if r > std::f64::consts::PI {
        r -= 2.0 * std::f64::consts::PI;
    } else if r < -std::f64::consts::PI {
        r += 2.0 * std::f64::consts::PI;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state() {
        let mt = MultiTouchState::new();
        assert_eq!(mt.touch_count(), 0);
        assert_eq!(mt.centroid(), (0.0, 0.0));
        assert!((mt.spread()).abs() < f64::EPSILON);
        assert!((mt.rotation_angle()).abs() < f64::EPSILON);
    }

    #[test]
    fn touch_down_up() {
        let mut mt = MultiTouchState::new();
        assert!(mt.touch_down(1, 100.0, 200.0, 0));
        assert_eq!(mt.touch_count(), 1);
        mt.touch_up(1);
        assert_eq!(mt.touch_count(), 0);
    }

    #[test]
    fn max_touches_limit() {
        let mut mt = MultiTouchState::new();
        for i in 0..MAX_TOUCHES as u64 {
            assert!(mt.touch_down(i, i as f64, 0.0, 0));
        }
        assert!(!mt.touch_down(99, 99.0, 0.0, 0));
        assert_eq!(mt.touch_count(), MAX_TOUCHES);
    }

    #[test]
    fn centroid_single() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 100.0, 200.0, 0);
        assert_eq!(mt.centroid(), (100.0, 200.0));
    }

    #[test]
    fn centroid_two() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 100.0, 200.0, 0);
        let (cx, cy) = mt.centroid();
        assert!((cx - 50.0).abs() < f64::EPSILON);
        assert!((cy - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn touch_move_updates_centroid() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 100.0, 0.0, 0);
        mt.touch_move(1, 50.0, 0.0, 1);
        let (cx, _) = mt.centroid();
        assert!((cx - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn spread_single_finger_zero() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 100.0, 100.0, 0);
        assert!((mt.spread()).abs() < f64::EPSILON);
    }

    #[test]
    fn spread_two_fingers() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 100.0, 0.0, 0);
        let s = mt.spread();
        // Centroid at (50, 0), each finger 50px away => average spread = 50
        assert!((s - 50.0).abs() < 0.01);
    }

    #[test]
    fn spread_increases_with_pinch_out() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 40.0, 0.0, 0);
        mt.touch_down(2, 60.0, 0.0, 0);
        let s1 = mt.spread();
        mt.touch_move(1, 0.0, 0.0, 1);
        mt.touch_move(2, 100.0, 0.0, 1);
        let s2 = mt.spread();
        assert!(s2 > s1, "Spread should increase: {} vs {}", s1, s2);
    }

    #[test]
    fn rotation_no_movement() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 100.0, 0.0, 0);
        assert!((mt.rotation_angle()).abs() < 0.001);
    }

    #[test]
    fn rotation_90_degrees() {
        let mut mt = MultiTouchState::new();
        // Two fingers horizontally: (0,0) and (100,0), centroid at (50,0)
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 100.0, 0.0, 0);
        // Rotate finger 1 to (50, -50) — 90 degrees CCW from original (-50, 0) vector
        mt.touch_move(1, 50.0, -50.0, 1);
        mt.touch_move(2, 50.0, 50.0, 1);
        let angle = mt.rotation_angle();
        let expected = std::f64::consts::FRAC_PI_2; // 90 degrees
        assert!(
            (angle - expected).abs() < 0.1,
            "Expected ~PI/2 ({}) got {}",
            expected,
            angle
        );
    }

    #[test]
    fn get_touch() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(42, 10.0, 20.0, 1000);
        let tp = mt.get(42);
        assert!(tp.is_some());
        let tp = tp.unwrap();
        assert_eq!(tp.id, 42);
        assert!((tp.x - 10.0).abs() < f64::EPSILON);
        assert_eq!(tp.timestamp, 1000);
    }

    #[test]
    fn get_nonexistent() {
        let mt = MultiTouchState::new();
        assert!(mt.get(99).is_none());
    }

    #[test]
    fn active_touches_list() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 10.0, 10.0, 0);
        let touches = mt.active_touches();
        assert_eq!(touches.len(), 2);
    }

    #[test]
    fn reset_clears() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(2, 10.0, 10.0, 0);
        mt.reset();
        assert_eq!(mt.touch_count(), 0);
    }

    #[test]
    fn duplicate_id_replaced() {
        let mut mt = MultiTouchState::new();
        mt.touch_down(1, 0.0, 0.0, 0);
        mt.touch_down(1, 50.0, 50.0, 1);
        assert_eq!(mt.touch_count(), 1);
        let tp = mt.get(1).unwrap();
        assert!((tp.x - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normalize_angle_wrap() {
        let a = normalize_angle(4.0 * std::f64::consts::PI + 0.1);
        assert!((a - 0.1).abs() < 0.01);
        let b = normalize_angle(-4.0 * std::f64::consts::PI - 0.1);
        assert!((b + 0.1).abs() < 0.01);
    }
}
