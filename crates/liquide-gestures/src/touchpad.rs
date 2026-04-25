//! Multi-finger touchpad gesture recognition.
//!
//! Models libinput's gesture events for touchpad hardware: two-finger scroll,
//! three/four-finger swipe, pinch-zoom, and pinch-rotate. The recognizer
//! consumes raw `TouchpadEvent`s (finger down/move/up) and emits high-level
//! `TouchpadGesture`s with phase tracking.

/// Direction of a multi-finger swipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Phase of a touchpad gesture lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Begin,
    Update,
    End,
    Cancel,
}

/// High-level touchpad gesture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchpadGesture {
    TwoFingerScroll {
        dx: f64,
        dy: f64,
        phase: GesturePhase,
    },
    ThreeFingerSwipe {
        direction: SwipeDirection,
        dx: f64,
        dy: f64,
        phase: GesturePhase,
    },
    FourFingerSwipe {
        direction: SwipeDirection,
        dx: f64,
        dy: f64,
        phase: GesturePhase,
    },
    PinchZoom {
        scale: f64,
        center_x: f64,
        center_y: f64,
        phase: GesturePhase,
    },
    PinchRotate {
        angle: f64,
        center_x: f64,
        center_y: f64,
        phase: GesturePhase,
    },
}

/// Semantic action for three-finger swipe directions (GNOME/Mutter style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeFingerAction {
    Overview,
    ShowDesktop,
    WorkspacePrev,
    WorkspaceNext,
}

impl ThreeFingerAction {
    /// Map a three-finger swipe direction to an action (GNOME defaults).
    pub fn from_direction(dir: SwipeDirection) -> Self {
        match dir {
            SwipeDirection::Up => ThreeFingerAction::Overview,
            SwipeDirection::Down => ThreeFingerAction::ShowDesktop,
            SwipeDirection::Left => ThreeFingerAction::WorkspaceNext,
            SwipeDirection::Right => ThreeFingerAction::WorkspacePrev,
        }
    }
}

/// Touchpad configuration.
#[derive(Debug, Clone)]
pub struct TouchpadConfig {
    /// When true, scroll direction matches finger movement (macOS-style).
    pub natural_scrolling: bool,
    /// Multiplier applied to scroll deltas.
    pub scroll_speed: f64,
    /// Minimum displacement (px) before a swipe is recognized.
    pub swipe_threshold: f64,
    /// Minimum scale deviation from 1.0 before pinch-zoom is recognized.
    pub pinch_threshold: f64,
    /// Minimum rotation (radians) before pinch-rotate is recognized.
    pub rotate_threshold: f64,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            natural_scrolling: true,
            scroll_speed: 1.0,
            swipe_threshold: 15.0,
            pinch_threshold: 0.08,
            rotate_threshold: 0.05,
        }
    }
}

/// Raw touch event from the touchpad.
#[derive(Debug, Clone, Copy)]
pub struct TouchpadEvent {
    pub finger_id: u32,
    pub x: f64,
    pub y: f64,
    pub kind: TouchpadEventKind,
}

/// Kind of touchpad event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchpadEventKind {
    Down,
    Motion,
    Up,
}

/// Internal per-finger state.
#[derive(Debug, Clone, Copy)]
struct FingerState {
    id: u32,
    start_x: f64,
    start_y: f64,
    cur_x: f64,
    cur_y: f64,
    prev_x: f64,
    prev_y: f64,
}

/// Internal recognizer state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecognizerMode {
    Idle,
    Pending,
    Scrolling,
    Swiping { fingers: u32 },
    Pinching,
    Rotating,
}

/// Touchpad gesture recognizer.
///
/// Feed `TouchpadEvent`s via [`feed`] and collect emitted gestures.
pub struct TouchpadRecognizer {
    config: TouchpadConfig,
    fingers: Vec<FingerState>,
    mode: RecognizerMode,
    initial_spread: f64,
    initial_angle: f64,
}

impl TouchpadRecognizer {
    pub fn new(config: TouchpadConfig) -> Self {
        Self {
            config,
            fingers: Vec::with_capacity(10),
            mode: RecognizerMode::Idle,
            initial_spread: 0.0,
            initial_angle: 0.0,
        }
    }

    /// Feed a touchpad event and return any recognized gestures.
    pub fn feed(&mut self, event: TouchpadEvent) -> Vec<TouchpadGesture> {
        let mut out = Vec::new();
        match event.kind {
            TouchpadEventKind::Down => {
                self.fingers.push(FingerState {
                    id: event.finger_id,
                    start_x: event.x,
                    start_y: event.y,
                    cur_x: event.x,
                    cur_y: event.y,
                    prev_x: event.x,
                    prev_y: event.y,
                });
                self.mode = RecognizerMode::Pending;
                // Capture initial geometry when we have 2+ fingers
                if self.fingers.len() >= 2 {
                    self.initial_spread = self.current_spread();
                    self.initial_angle = self.current_angle();
                }
            }
            TouchpadEventKind::Motion => {
                if let Some(f) = self.fingers.iter_mut().find(|f| f.id == event.finger_id) {
                    f.prev_x = f.cur_x;
                    f.prev_y = f.cur_y;
                    f.cur_x = event.x;
                    f.cur_y = event.y;
                }
                self.update_mode(&mut out);
            }
            TouchpadEventKind::Up => {
                self.emit_end(&mut out);
                self.fingers.retain(|f| f.id != event.finger_id);
                if self.fingers.is_empty() {
                    self.mode = RecognizerMode::Idle;
                }
            }
        }
        out
    }

    /// Cancel all active gestures.
    pub fn cancel(&mut self) -> Vec<TouchpadGesture> {
        let mut out = Vec::new();
        self.emit_cancel(&mut out);
        self.fingers.clear();
        self.mode = RecognizerMode::Idle;
        out
    }

    /// Current finger count.
    pub fn finger_count(&self) -> usize {
        self.fingers.len()
    }

    fn update_mode(&mut self, out: &mut Vec<TouchpadGesture>) {
        let n = self.fingers.len();
        if n == 0 {
            return;
        }

        let (avg_dx, avg_dy) = self.average_delta_from_start();
        let dist = (avg_dx * avg_dx + avg_dy * avg_dy).sqrt();

        match self.mode {
            RecognizerMode::Pending => {
                // Wait until ALL fingers have moved before committing to a
                // gesture type — prevents false pinch/rotate when only one
                // finger has moved (which shifts the centroid).
                let min_move = 1.0_f64;
                let all_moved = n >= 2
                    && self.fingers.iter().all(|f| {
                        let fdx = f.cur_x - f.start_x;
                        let fdy = f.cur_y - f.start_y;
                        (fdx * fdx + fdy * fdy).sqrt() > min_move
                    });

                if n == 2 && all_moved {
                    // All fingers moved — decide scroll vs pinch vs rotate.
                    let spread = self.current_spread();
                    let angle = self.current_angle();
                    let spread_delta = (spread - self.initial_spread).abs();

                    // Pinch: require meaningful initial spread AND absolute change
                    let min_abs_spread_change = self.config.swipe_threshold;
                    if self.initial_spread > 5.0 && spread_delta > min_abs_spread_change {
                        let scale = spread / self.initial_spread;
                        if (scale - 1.0).abs() > self.config.pinch_threshold {
                            self.mode = RecognizerMode::Pinching;
                            let (cx, cy) = self.centroid();
                            out.push(TouchpadGesture::PinchZoom {
                                scale,
                                center_x: cx,
                                center_y: cy,
                                phase: GesturePhase::Begin,
                            });
                            return;
                        }
                    }
                    // Rotate: require meaningful initial spread AND angle change
                    if self.initial_spread > 5.0 {
                        let angle_diff = angle - self.initial_angle;
                        if angle_diff.abs() > self.config.rotate_threshold {
                            self.mode = RecognizerMode::Rotating;
                            let (cx, cy) = self.centroid();
                            out.push(TouchpadGesture::PinchRotate {
                                angle: angle_diff,
                                center_x: cx,
                                center_y: cy,
                                phase: GesturePhase::Begin,
                            });
                            return;
                        }
                    }
                    // Default: scroll (when avg displacement exceeds threshold)
                    if dist > self.config.swipe_threshold {
                        self.mode = RecognizerMode::Scrolling;
                        let (dx, dy) = self.scroll_delta();
                        out.push(TouchpadGesture::TwoFingerScroll {
                            dx,
                            dy,
                            phase: GesturePhase::Begin,
                        });
                    }
                } else if (n == 3 || n == 4) && dist > self.config.swipe_threshold {
                    let fingers = n as u32;
                    self.mode = RecognizerMode::Swiping { fingers };
                    let direction = classify_direction(avg_dx, avg_dy);
                    let gesture = if fingers == 3 {
                        TouchpadGesture::ThreeFingerSwipe {
                            direction,
                            dx: avg_dx,
                            dy: avg_dy,
                            phase: GesturePhase::Begin,
                        }
                    } else {
                        TouchpadGesture::FourFingerSwipe {
                            direction,
                            dx: avg_dx,
                            dy: avg_dy,
                            phase: GesturePhase::Begin,
                        }
                    };
                    out.push(gesture);
                }
            }
            RecognizerMode::Scrolling => {
                let (dx, dy) = self.scroll_delta();
                out.push(TouchpadGesture::TwoFingerScroll {
                    dx,
                    dy,
                    phase: GesturePhase::Update,
                });
            }
            RecognizerMode::Pinching => {
                let spread = self.current_spread();
                let scale = if self.initial_spread > 0.01 {
                    spread / self.initial_spread
                } else {
                    1.0
                };
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchZoom {
                    scale,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::Update,
                });
            }
            RecognizerMode::Rotating => {
                let angle = self.current_angle();
                let angle_diff = angle - self.initial_angle;
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchRotate {
                    angle: angle_diff,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::Update,
                });
            }
            RecognizerMode::Swiping { fingers } => {
                let direction = classify_direction(avg_dx, avg_dy);
                let gesture = if fingers == 3 {
                    TouchpadGesture::ThreeFingerSwipe {
                        direction,
                        dx: avg_dx,
                        dy: avg_dy,
                        phase: GesturePhase::Update,
                    }
                } else {
                    TouchpadGesture::FourFingerSwipe {
                        direction,
                        dx: avg_dx,
                        dy: avg_dy,
                        phase: GesturePhase::Update,
                    }
                };
                out.push(gesture);
            }
            RecognizerMode::Idle => {}
        }
    }

    fn emit_end(&mut self, out: &mut Vec<TouchpadGesture>) {
        match self.mode {
            RecognizerMode::Scrolling => {
                out.push(TouchpadGesture::TwoFingerScroll {
                    dx: 0.0,
                    dy: 0.0,
                    phase: GesturePhase::End,
                });
            }
            RecognizerMode::Pinching => {
                let spread = self.current_spread();
                let scale = if self.initial_spread > 0.01 {
                    spread / self.initial_spread
                } else {
                    1.0
                };
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchZoom {
                    scale,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::End,
                });
            }
            RecognizerMode::Rotating => {
                let angle = self.current_angle() - self.initial_angle;
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchRotate {
                    angle,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::End,
                });
            }
            RecognizerMode::Swiping { fingers } => {
                let (avg_dx, avg_dy) = self.average_delta_from_start();
                let direction = classify_direction(avg_dx, avg_dy);
                if fingers == 3 {
                    out.push(TouchpadGesture::ThreeFingerSwipe {
                        direction,
                        dx: avg_dx,
                        dy: avg_dy,
                        phase: GesturePhase::End,
                    });
                } else {
                    out.push(TouchpadGesture::FourFingerSwipe {
                        direction,
                        dx: avg_dx,
                        dy: avg_dy,
                        phase: GesturePhase::End,
                    });
                }
            }
            _ => {}
        }
    }

    fn emit_cancel(&mut self, out: &mut Vec<TouchpadGesture>) {
        match self.mode {
            RecognizerMode::Scrolling => {
                out.push(TouchpadGesture::TwoFingerScroll {
                    dx: 0.0,
                    dy: 0.0,
                    phase: GesturePhase::Cancel,
                });
            }
            RecognizerMode::Pinching => {
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchZoom {
                    scale: 1.0,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::Cancel,
                });
            }
            RecognizerMode::Rotating => {
                let (cx, cy) = self.centroid();
                out.push(TouchpadGesture::PinchRotate {
                    angle: 0.0,
                    center_x: cx,
                    center_y: cy,
                    phase: GesturePhase::Cancel,
                });
            }
            RecognizerMode::Swiping { fingers } => {
                let direction = SwipeDirection::Up;
                if fingers == 3 {
                    out.push(TouchpadGesture::ThreeFingerSwipe {
                        direction,
                        dx: 0.0,
                        dy: 0.0,
                        phase: GesturePhase::Cancel,
                    });
                } else {
                    out.push(TouchpadGesture::FourFingerSwipe {
                        direction,
                        dx: 0.0,
                        dy: 0.0,
                        phase: GesturePhase::Cancel,
                    });
                }
            }
            _ => {}
        }
    }

    fn average_delta_from_start(&self) -> (f64, f64) {
        if self.fingers.is_empty() {
            return (0.0, 0.0);
        }
        let n = self.fingers.len() as f64;
        let (sx, sy) = self.fingers.iter().fold((0.0, 0.0), |(ax, ay), f| {
            (ax + (f.cur_x - f.start_x), ay + (f.cur_y - f.start_y))
        });
        (sx / n, sy / n)
    }

    fn scroll_delta(&self) -> (f64, f64) {
        if self.fingers.is_empty() {
            return (0.0, 0.0);
        }
        let n = self.fingers.len() as f64;
        let (sx, sy) = self.fingers.iter().fold((0.0, 0.0), |(ax, ay), f| {
            (ax + (f.cur_x - f.prev_x), ay + (f.cur_y - f.prev_y))
        });
        let mut dx = sx / n * self.config.scroll_speed;
        let mut dy = sy / n * self.config.scroll_speed;
        if self.config.natural_scrolling {
            dx = -dx;
            dy = -dy;
        }
        (dx, dy)
    }

    fn centroid(&self) -> (f64, f64) {
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

    fn current_spread(&self) -> f64 {
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

    fn current_angle(&self) -> f64 {
        if self.fingers.len() < 2 {
            return 0.0;
        }
        let (cx, cy) = self.centroid();
        let f0 = &self.fingers[0];
        (f0.cur_y - cy).atan2(f0.cur_x - cx)
    }
}

fn classify_direction(dx: f64, dy: f64) -> SwipeDirection {
    if dx.abs() > dy.abs() {
        if dx > 0.0 {
            SwipeDirection::Right
        } else {
            SwipeDirection::Left
        }
    } else {
        if dy > 0.0 {
            SwipeDirection::Down
        } else {
            SwipeDirection::Up
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: u32, x: f64, y: f64, kind: TouchpadEventKind) -> TouchpadEvent {
        TouchpadEvent {
            finger_id: id,
            x,
            y,
            kind,
        }
    }

    #[test]
    fn default_config() {
        let cfg = TouchpadConfig::default();
        assert!(cfg.natural_scrolling);
        assert!((cfg.scroll_speed - 1.0).abs() < f64::EPSILON);
        assert!(cfg.swipe_threshold > 0.0);
        assert!(cfg.pinch_threshold > 0.0);
    }

    #[test]
    fn two_finger_scroll_detected() {
        let mut rec = TouchpadRecognizer::new(TouchpadConfig::default());
        // Fingers 200px apart so that small parallel movement doesn't cause
        // detectable rotation or spread change relative to distance.
        rec.feed(ev(0, 300.0, 300.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 500.0, 300.0, TouchpadEventKind::Down));
        // Move both fingers down together past threshold
        rec.feed(ev(0, 300.0, 320.0, TouchpadEventKind::Motion));
        let gestures = rec.feed(ev(1, 500.0, 320.0, TouchpadEventKind::Motion));
        let has_scroll = gestures
            .iter()
            .any(|g| matches!(g, TouchpadGesture::TwoFingerScroll { .. }));
        assert!(has_scroll, "Expected TwoFingerScroll, got {:?}", gestures);
    }

    #[test]
    fn natural_scrolling_inverts() {
        let mut cfg = TouchpadConfig::default();
        cfg.natural_scrolling = true;
        cfg.swipe_threshold = 1.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        rec.feed(ev(0, 100.0, 100.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 120.0, 100.0, TouchpadEventKind::Down));
        rec.feed(ev(0, 100.0, 110.0, TouchpadEventKind::Motion));
        let gestures = rec.feed(ev(1, 120.0, 110.0, TouchpadEventKind::Motion));
        for g in &gestures {
            if let TouchpadGesture::TwoFingerScroll { dy, .. } = g {
                // Finger moved +10 (down), natural scrolling should invert
                assert!(*dy < 0.0, "Natural scroll should invert dy, got {}", dy);
            }
        }
    }

    #[test]
    fn three_finger_swipe_up_overview() {
        let mut cfg = TouchpadConfig::default();
        cfg.swipe_threshold = 5.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        rec.feed(ev(0, 300.0, 300.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 320.0, 300.0, TouchpadEventKind::Down));
        rec.feed(ev(2, 340.0, 300.0, TouchpadEventKind::Down));
        let gestures = rec.feed(ev(0, 300.0, 270.0, TouchpadEventKind::Motion));
        let has_swipe = gestures.iter().any(|g| {
            matches!(
                g,
                TouchpadGesture::ThreeFingerSwipe {
                    direction: SwipeDirection::Up,
                    ..
                }
            )
        });
        assert!(has_swipe, "Expected three-finger swipe up");
        assert_eq!(
            ThreeFingerAction::from_direction(SwipeDirection::Up),
            ThreeFingerAction::Overview
        );
    }

    #[test]
    fn three_finger_swipe_down_show_desktop() {
        assert_eq!(
            ThreeFingerAction::from_direction(SwipeDirection::Down),
            ThreeFingerAction::ShowDesktop
        );
    }

    #[test]
    fn three_finger_swipe_left_workspace() {
        assert_eq!(
            ThreeFingerAction::from_direction(SwipeDirection::Left),
            ThreeFingerAction::WorkspaceNext
        );
    }

    #[test]
    fn three_finger_swipe_right_workspace() {
        assert_eq!(
            ThreeFingerAction::from_direction(SwipeDirection::Right),
            ThreeFingerAction::WorkspacePrev
        );
    }

    #[test]
    fn four_finger_swipe_detected() {
        let mut cfg = TouchpadConfig::default();
        cfg.swipe_threshold = 5.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        for i in 0..4 {
            rec.feed(ev(
                i,
                300.0 + i as f64 * 20.0,
                400.0,
                TouchpadEventKind::Down,
            ));
        }
        let gestures = rec.feed(ev(0, 250.0, 400.0, TouchpadEventKind::Motion));
        let has_swipe = gestures.iter().any(|g| {
            matches!(
                g,
                TouchpadGesture::FourFingerSwipe {
                    direction: SwipeDirection::Left,
                    ..
                }
            )
        });
        assert!(has_swipe, "Expected four-finger swipe left");
    }

    #[test]
    fn scroll_end_phase() {
        let mut cfg = TouchpadConfig::default();
        cfg.swipe_threshold = 1.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        // Fingers 200px apart to avoid false pinch/rotate detection
        rec.feed(ev(0, 100.0, 100.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 300.0, 100.0, TouchpadEventKind::Down));
        // Move both fingers together
        rec.feed(ev(0, 100.0, 120.0, TouchpadEventKind::Motion));
        rec.feed(ev(1, 300.0, 120.0, TouchpadEventKind::Motion));
        let gestures = rec.feed(ev(0, 100.0, 120.0, TouchpadEventKind::Up));
        let has_end = gestures.iter().any(|g| {
            matches!(
                g,
                TouchpadGesture::TwoFingerScroll {
                    phase: GesturePhase::End,
                    ..
                }
            )
        });
        assert!(has_end, "Expected scroll End phase, got {:?}", gestures);
    }

    #[test]
    fn cancel_emits_cancel_phase() {
        let mut cfg = TouchpadConfig::default();
        cfg.swipe_threshold = 1.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        // Fingers 200px apart to avoid false pinch/rotate detection
        rec.feed(ev(0, 100.0, 100.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 300.0, 100.0, TouchpadEventKind::Down));
        // Move both fingers together to establish scrolling
        rec.feed(ev(0, 100.0, 120.0, TouchpadEventKind::Motion));
        rec.feed(ev(1, 300.0, 120.0, TouchpadEventKind::Motion));
        let gestures = rec.cancel();
        let has_cancel = gestures.iter().any(|g| {
            matches!(
                g,
                TouchpadGesture::TwoFingerScroll {
                    phase: GesturePhase::Cancel,
                    ..
                }
            )
        });
        assert!(has_cancel, "Expected cancel phase");
        assert_eq!(rec.finger_count(), 0);
    }

    #[test]
    fn pinch_zoom_detected() {
        let mut cfg = TouchpadConfig::default();
        cfg.pinch_threshold = 0.05;
        cfg.swipe_threshold = 5.0;
        let mut rec = TouchpadRecognizer::new(cfg);
        // Start with fingers 60px apart (initial_spread = 30, well above 5.0 threshold)
        rec.feed(ev(0, 270.0, 300.0, TouchpadEventKind::Down));
        rec.feed(ev(1, 330.0, 300.0, TouchpadEventKind::Down));
        // Move fingers apart: 170 and 430 → spread ~130, delta 100 > swipe_threshold
        let g1 = rec.feed(ev(0, 170.0, 300.0, TouchpadEventKind::Motion));
        let g2 = rec.feed(ev(1, 430.0, 300.0, TouchpadEventKind::Motion));
        let all: Vec<_> = g1.into_iter().chain(g2).collect();
        let has_pinch = all
            .iter()
            .any(|g| matches!(g, TouchpadGesture::PinchZoom { .. }));
        assert!(has_pinch, "Expected PinchZoom, got {:?}", all);
    }

    #[test]
    fn classify_direction_horizontal() {
        assert_eq!(classify_direction(50.0, 10.0), SwipeDirection::Right);
        assert_eq!(classify_direction(-50.0, 10.0), SwipeDirection::Left);
    }

    #[test]
    fn classify_direction_vertical() {
        assert_eq!(classify_direction(5.0, 50.0), SwipeDirection::Down);
        assert_eq!(classify_direction(5.0, -50.0), SwipeDirection::Up);
    }

    #[test]
    fn finger_count_tracks() {
        let mut rec = TouchpadRecognizer::new(TouchpadConfig::default());
        assert_eq!(rec.finger_count(), 0);
        rec.feed(ev(0, 0.0, 0.0, TouchpadEventKind::Down));
        assert_eq!(rec.finger_count(), 1);
        rec.feed(ev(1, 10.0, 0.0, TouchpadEventKind::Down));
        assert_eq!(rec.finger_count(), 2);
        rec.feed(ev(0, 0.0, 0.0, TouchpadEventKind::Up));
        assert_eq!(rec.finger_count(), 1);
    }
}
