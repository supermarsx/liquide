//! Gesture recognition from raw touch event sequences.

use serde::{Deserialize, Serialize};

use crate::input::{TouchEvent, TouchPhase};

/// The kind of recognized gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GestureKind {
    /// Single finger tap.
    SingleTap,
    /// Two taps in quick succession.
    DoubleTap,
    /// Long press (finger held down).
    LongPress,
    /// Long press followed by drag.
    LongPressDrag,
    /// Two-finger tap (used for right-click).
    TwoFingerTap,
    /// Single-finger pan / drag.
    Pan,
    /// Two-finger pinch (zoom).
    Pinch,
    /// Three-finger swipe.
    ThreeFingerSwipe,
    /// Swipe from the left edge.
    EdgeSwipeLeft,
    /// Swipe from the right edge.
    EdgeSwipeRight,
}

impl std::fmt::Display for GestureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SingleTap => write!(f, "single-tap"),
            Self::DoubleTap => write!(f, "double-tap"),
            Self::LongPress => write!(f, "long-press"),
            Self::LongPressDrag => write!(f, "long-press-drag"),
            Self::TwoFingerTap => write!(f, "two-finger-tap"),
            Self::Pan => write!(f, "pan"),
            Self::Pinch => write!(f, "pinch"),
            Self::ThreeFingerSwipe => write!(f, "three-finger-swipe"),
            Self::EdgeSwipeLeft => write!(f, "edge-swipe-left"),
            Self::EdgeSwipeRight => write!(f, "edge-swipe-right"),
        }
    }
}

/// Tracked state for a single active touch contact.
#[derive(Debug, Clone)]
struct ActiveTouch {
    id: u32,
    start_x: f32,
    start_y: f32,
    current_x: f32,
    current_y: f32,
    start_time: u64,
}

/// Maximum distance in points for a touch to be considered a tap.
const TAP_DISTANCE_THRESHOLD: f32 = 20.0;

/// Maximum duration in milliseconds for a touch to be considered a tap.
const TAP_DURATION_THRESHOLD: u64 = 300;

/// Minimum duration in milliseconds for a long press.
const LONG_PRESS_THRESHOLD: u64 = 500;

/// Maximum time between taps for a double-tap (ms).
const DOUBLE_TAP_INTERVAL: u64 = 400;

/// Minimum distance for a pan gesture.
const PAN_DISTANCE_THRESHOLD: f32 = 10.0;

/// Recognizes gestures from sequences of touch events.
pub struct GestureRecognizer {
    active_touches: Vec<ActiveTouch>,
    last_tap_time: u64,
    tap_count: u32,
}

impl GestureRecognizer {
    /// Create a new gesture recognizer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_touches: Vec::new(),
            last_tap_time: 0,
            tap_count: 0,
        }
    }

    /// Attempt to recognize a gesture from a slice of recent touch events.
    ///
    /// Returns `Some(kind)` if a gesture was recognized, `None` otherwise.
    pub fn recognize(&mut self, events: &[TouchEvent]) -> Option<GestureKind> {
        // Update internal tracking state.
        for event in events {
            match event.phase {
                TouchPhase::Began => {
                    self.active_touches.push(ActiveTouch {
                        id: event.id,
                        start_x: event.x,
                        start_y: event.y,
                        current_x: event.x,
                        current_y: event.y,
                        start_time: event.timestamp,
                    });
                }
                TouchPhase::Moved => {
                    if let Some(touch) = self
                        .active_touches
                        .iter_mut()
                        .find(|t| t.id == event.id)
                    {
                        touch.current_x = event.x;
                        touch.current_y = event.y;
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    // Process later, keep for now.
                    if let Some(touch) = self
                        .active_touches
                        .iter_mut()
                        .find(|t| t.id == event.id)
                    {
                        touch.current_x = event.x;
                        touch.current_y = event.y;
                    }
                }
            }
        }

        // Find the last ended event to decide on the gesture.
        let last_ended = events
            .iter()
            .rev()
            .find(|e| e.phase == TouchPhase::Ended);

        if let Some(ended) = last_ended {
            let result = self.evaluate_gesture(ended.timestamp);
            // Remove ended touches.
            for event in events {
                if event.phase == TouchPhase::Ended || event.phase == TouchPhase::Cancelled {
                    self.active_touches.retain(|t| t.id != event.id);
                }
            }
            return result;
        }

        // Check for ongoing gestures (long press, pan, pinch) from moved events.
        if let Some(last_moved) = events.iter().rev().find(|e| e.phase == TouchPhase::Moved) {
            return self.evaluate_ongoing(last_moved.timestamp);
        }

        None
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.active_touches.clear();
        self.last_tap_time = 0;
        self.tap_count = 0;
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn evaluate_gesture(&mut self, timestamp: u64) -> Option<GestureKind> {
        let touch_count = self.active_touches.len();

        if touch_count == 0 {
            return None;
        }

        // Three-finger swipe.
        if touch_count >= 3 {
            return Some(GestureKind::ThreeFingerSwipe);
        }

        // Two-finger tap.
        if touch_count == 2 {
            let all_taps = self.active_touches.iter().all(|t| {
                let dx = t.current_x - t.start_x;
                let dy = t.current_y - t.start_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let duration = timestamp.saturating_sub(t.start_time);
                dist < TAP_DISTANCE_THRESHOLD && duration < TAP_DURATION_THRESHOLD
            });
            if all_taps {
                return Some(GestureKind::TwoFingerTap);
            }
            // Two-finger movement is a pinch.
            return Some(GestureKind::Pinch);
        }

        // Single finger.
        if touch_count == 1 {
            let touch = &self.active_touches[0];
            let dx = touch.current_x - touch.start_x;
            let dy = touch.current_y - touch.start_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let duration = timestamp.saturating_sub(touch.start_time);

            if dist < TAP_DISTANCE_THRESHOLD && duration < TAP_DURATION_THRESHOLD {
                // It's a tap -- check for double-tap.
                if timestamp.saturating_sub(self.last_tap_time) < DOUBLE_TAP_INTERVAL
                    && self.tap_count >= 1
                {
                    self.tap_count = 0;
                    self.last_tap_time = 0;
                    return Some(GestureKind::DoubleTap);
                }
                self.tap_count = 1;
                self.last_tap_time = timestamp;
                return Some(GestureKind::SingleTap);
            }

            if duration >= LONG_PRESS_THRESHOLD && dist < TAP_DISTANCE_THRESHOLD {
                return Some(GestureKind::LongPress);
            }

            if dist >= PAN_DISTANCE_THRESHOLD {
                if duration >= LONG_PRESS_THRESHOLD {
                    return Some(GestureKind::LongPressDrag);
                }
                return Some(GestureKind::Pan);
            }
        }

        None
    }

    fn evaluate_ongoing(&self, timestamp: u64) -> Option<GestureKind> {
        let touch_count = self.active_touches.len();

        if touch_count >= 2 {
            return Some(GestureKind::Pinch);
        }

        if touch_count == 1 {
            let touch = &self.active_touches[0];
            let dx = touch.current_x - touch.start_x;
            let dy = touch.current_y - touch.start_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let duration = timestamp.saturating_sub(touch.start_time);

            if duration >= LONG_PRESS_THRESHOLD && dist < TAP_DISTANCE_THRESHOLD {
                return Some(GestureKind::LongPress);
            }

            if dist >= PAN_DISTANCE_THRESHOLD {
                if duration >= LONG_PRESS_THRESHOLD {
                    return Some(GestureKind::LongPressDrag);
                }
                return Some(GestureKind::Pan);
            }
        }

        None
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}
