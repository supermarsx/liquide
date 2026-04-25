/// Action to take after a shake gesture is confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShakeAction {
    MinimizeOthers,
    RestoreOthers,
}

/// Detects rapid back-and-forth window movement (shake-to-minimize).
///
/// Internally tracks recent position samples and counts direction reversals
/// within a configurable time window.
pub struct ShakeDetector {
    /// Minimum total displacement (px) between reversals for a shake to count.
    threshold_px: f32,
    /// Maximum time span (ms) in which the reversals must occur.
    time_window_ms: f32,
    /// Ring buffer of recent positions: (x, y, timestamp_ms).
    samples: Vec<(f32, f32, f32)>,
    /// Whether the last shake action was MinimizeOthers (toggles on next shake).
    others_minimized: bool,
}

impl ShakeDetector {
    pub fn new(threshold_px: f32, time_window_ms: f32) -> Self {
        Self {
            threshold_px,
            time_window_ms,
            samples: Vec::with_capacity(64),
            others_minimized: false,
        }
    }

    /// Feed a window-move event.  Returns `true` if a shake gesture is detected.
    pub fn on_window_move(&mut self, x: f32, y: f32, now_ms: f32) -> bool {
        self.samples.push((x, y, now_ms));

        // Purge samples older than the time window.
        let cutoff = now_ms - self.time_window_ms;
        self.samples.retain(|&(_, _, t)| t >= cutoff);

        if self.samples.len() < 4 {
            return false;
        }

        detect_shake_gesture(&self.samples, self.threshold_px)
    }

    /// After a shake is confirmed, call this to get the appropriate action
    /// and reset internal state.
    pub fn consume_shake(&mut self) -> ShakeAction {
        self.samples.clear();
        if self.others_minimized {
            self.others_minimized = false;
            ShakeAction::RestoreOthers
        } else {
            self.others_minimized = true;
            ShakeAction::MinimizeOthers
        }
    }
}

/// Given a series of `(x, y, timestamp_ms)` samples, detect whether there are
/// 3 or more direction reversals (horizontal or vertical), each covering at
/// least `threshold_px` displacement.
pub fn detect_shake_gesture(positions: &[(f32, f32, f32)], threshold_px: f32) -> bool {
    if positions.len() < 4 {
        return false;
    }

    // Check both axes independently; a shake on either axis counts.
    detect_axis_shake(positions, threshold_px, true)
        || detect_axis_shake(positions, threshold_px, false)
}

/// Check for reversals along a single axis.
fn detect_axis_shake(positions: &[(f32, f32, f32)], threshold_px: f32, use_x: bool) -> bool {
    let coord = |i: usize| -> f32 {
        if use_x {
            positions[i].0
        } else {
            positions[i].1
        }
    };

    let mut reversals = 0u32;
    let mut seg_start = 0usize;
    let mut prev_dir: Option<bool> = None; // true = increasing

    for i in 1..positions.len() {
        let delta = coord(i) - coord(i - 1);
        if delta.abs() < 0.5 {
            continue; // noise
        }
        let increasing = delta > 0.0;

        match prev_dir {
            None => {
                prev_dir = Some(increasing);
                seg_start = i - 1;
            }
            Some(dir) if dir != increasing => {
                // Direction reversed.  Check if the segment covered enough distance.
                let seg_dist = (coord(i - 1) - coord(seg_start)).abs();
                if seg_dist >= threshold_px {
                    reversals += 1;
                    if reversals >= 3 {
                        return true;
                    }
                }
                seg_start = i - 1;
                prev_dir = Some(increasing);
            }
            _ => {}
        }
    }

    // Check the final segment for one more reversal (the last move before
    // the function was called).
    if let Some(_) = prev_dir {
        let last = positions.len() - 1;
        let seg_dist = (coord(last) - coord(seg_start)).abs();
        if seg_dist >= threshold_px {
            reversals += 1;
        }
    }

    reversals >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_drag_does_not_trigger() {
        let mut det = ShakeDetector::new(20.0, 500.0);
        // Smooth rightward drag
        assert!(!det.on_window_move(100.0, 200.0, 0.0));
        assert!(!det.on_window_move(130.0, 200.0, 50.0));
        assert!(!det.on_window_move(160.0, 200.0, 100.0));
        assert!(!det.on_window_move(190.0, 200.0, 150.0));
        assert!(!det.on_window_move(220.0, 200.0, 200.0));
    }

    #[test]
    fn rapid_shake_triggers() {
        let mut det = ShakeDetector::new(15.0, 600.0);
        // Right, left, right, left — 3 reversals + final segment = detected
        assert!(!det.on_window_move(100.0, 200.0, 0.0));
        assert!(!det.on_window_move(130.0, 200.0, 50.0)); // +30 right (seg 1)
        assert!(!det.on_window_move(100.0, 200.0, 100.0)); // -30 left  (rev 1)
        assert!(det.on_window_move(130.0, 200.0, 150.0)); // +30 right (rev 2 + final seg = rev 3) → shake!
    }

    #[test]
    fn slow_movement_does_not_trigger() {
        let mut det = ShakeDetector::new(20.0, 400.0);
        // Direction changes but spread over a long time (outside time window)
        assert!(!det.on_window_move(100.0, 200.0, 0.0));
        assert!(!det.on_window_move(130.0, 200.0, 200.0));
        assert!(!det.on_window_move(100.0, 200.0, 500.0)); // first sample expired
        assert!(!det.on_window_move(130.0, 200.0, 700.0));
        assert!(!det.on_window_move(100.0, 200.0, 900.0));
    }

    #[test]
    fn small_displacement_ignored() {
        let mut det = ShakeDetector::new(20.0, 500.0);
        // Reversals exist but displacement < threshold
        assert!(!det.on_window_move(100.0, 200.0, 0.0));
        assert!(!det.on_window_move(105.0, 200.0, 50.0));
        assert!(!det.on_window_move(100.0, 200.0, 100.0));
        assert!(!det.on_window_move(105.0, 200.0, 150.0));
        assert!(!det.on_window_move(100.0, 200.0, 200.0));
    }

    #[test]
    fn detect_shake_gesture_direct() {
        let positions = vec![
            (100.0, 200.0, 0.0),
            (130.0, 200.0, 50.0),
            (100.0, 200.0, 100.0),
            (130.0, 200.0, 150.0),
            (100.0, 200.0, 200.0),
        ];
        assert!(detect_shake_gesture(&positions, 15.0));
    }

    #[test]
    fn detect_shake_gesture_too_few_samples() {
        let positions = vec![(100.0, 200.0, 0.0), (130.0, 200.0, 50.0)];
        assert!(!detect_shake_gesture(&positions, 15.0));
    }

    #[test]
    fn consume_shake_toggles_action() {
        let mut det = ShakeDetector::new(15.0, 600.0);
        assert_eq!(det.consume_shake(), ShakeAction::MinimizeOthers);
        assert_eq!(det.consume_shake(), ShakeAction::RestoreOthers);
        assert_eq!(det.consume_shake(), ShakeAction::MinimizeOthers);
    }

    #[test]
    fn vertical_shake_triggers() {
        let mut det = ShakeDetector::new(15.0, 600.0);
        assert!(!det.on_window_move(200.0, 100.0, 0.0));
        assert!(!det.on_window_move(200.0, 130.0, 50.0));
        assert!(!det.on_window_move(200.0, 100.0, 100.0));
        assert!(det.on_window_move(200.0, 130.0, 150.0)); // 3 reversals (incl. final seg)
    }
}
