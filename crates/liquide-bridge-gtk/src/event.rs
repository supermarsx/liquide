//! GTK event translation.
//!
//! Translates GTK4 event controller signals into Liquide's event model.
//! Handles keyboard, pointer (mouse + touch + pen), gesture, scroll,
//! and focus events.

use serde::{Deserialize, Serialize};

/// Modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
}

/// Keyboard event from GTK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtkKeyEvent {
    /// GDK keyval.
    pub keyval: u32,
    /// Hardware keycode.
    pub keycode: u16,
    /// Key name (e.g., "a", "Return", "BackSpace").
    pub key_name: String,
    /// Whether this is a key press (true) or release (false).
    pub pressed: bool,
    /// Whether this is an auto-repeat.
    pub is_repeat: bool,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Text produced by this key, if any.
    pub text: Option<String>,
}

/// Pointer event from GTK (mouse, touch, pen).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtkPointerEvent {
    /// X position in window coordinates.
    pub x: f64,
    /// Y position in window coordinates.
    pub y: f64,
    /// Button index (1=left, 2=middle, 3=right), 0 for motion.
    pub button: u32,
    /// Event kind.
    pub kind: PointerEventKind,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Pressure (0.0 – 1.0, from tablet pen).
    pub pressure: f32,
    /// Tilt angle X for pen input.
    pub tilt_x: f32,
    /// Tilt angle Y for pen input.
    pub tilt_y: f32,
    /// Input device type.
    pub device: InputDevice,
}

/// Pointer event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerEventKind {
    Enter,
    Leave,
    Motion,
    ButtonPress,
    ButtonRelease,
    Scroll,
}

/// Input device type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputDevice {
    Mouse,
    Touchscreen,
    Pen,
    Eraser,
    Unknown,
}

/// Scroll event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtkScrollEvent {
    pub dx: f64,
    pub dy: f64,
    pub x: f64,
    pub y: f64,
    pub modifiers: Modifiers,
    pub is_kinetic: bool,
}

/// Touch gesture event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtkGestureEvent {
    pub kind: GestureKind,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub angle: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GestureKind {
    PinchBegin,
    PinchUpdate,
    PinchEnd,
    RotateBegin,
    RotateUpdate,
    RotateEnd,
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
    LongPress,
}

/// Bridge that translates GTK events into Liquide events.
pub struct GtkEventBridge {
    /// Queued events waiting to be dispatched.
    event_queue: Vec<BridgedEvent>,
    /// Whether the event bridge is active.
    active: bool,
}

/// A Liquide-compatible event translated from GTK.
#[derive(Debug, Clone)]
pub enum BridgedEvent {
    Key(GtkKeyEvent),
    Pointer(GtkPointerEvent),
    Scroll(GtkScrollEvent),
    Gesture(GtkGestureEvent),
    FocusIn,
    FocusOut,
}

impl GtkEventBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_queue: Vec::new(),
            active: true,
        }
    }

    /// Push a key event from GTK.
    pub fn push_key_event(&mut self, event: GtkKeyEvent) {
        if self.active {
            self.event_queue.push(BridgedEvent::Key(event));
        }
    }

    /// Push a pointer event from GTK.
    pub fn push_pointer_event(&mut self, event: GtkPointerEvent) {
        if self.active {
            self.event_queue.push(BridgedEvent::Pointer(event));
        }
    }

    /// Push a scroll event from GTK.
    pub fn push_scroll_event(&mut self, event: GtkScrollEvent) {
        if self.active {
            self.event_queue.push(BridgedEvent::Scroll(event));
        }
    }

    /// Push a gesture event from GTK.
    pub fn push_gesture_event(&mut self, event: GtkGestureEvent) {
        if self.active {
            self.event_queue.push(BridgedEvent::Gesture(event));
        }
    }

    /// Push a focus event.
    pub fn push_focus(&mut self, focused: bool) {
        if self.active {
            self.event_queue.push(if focused {
                BridgedEvent::FocusIn
            } else {
                BridgedEvent::FocusOut
            });
        }
    }

    /// Drain all queued events.
    pub fn drain(&mut self) -> Vec<BridgedEvent> {
        std::mem::take(&mut self.event_queue)
    }

    /// Number of pending events.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.event_queue.len()
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

impl Default for GtkEventBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bridge() {
        let mut bridge = GtkEventBridge::new();
        bridge.push_key_event(GtkKeyEvent {
            keyval: 0x61,
            keycode: 38,
            key_name: "a".to_string(),
            pressed: true,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: Some("a".to_string()),
        });
        bridge.push_focus(true);
        assert_eq!(bridge.pending_count(), 2);

        let events = bridge.drain();
        assert_eq!(events.len(), 2);
        assert_eq!(bridge.pending_count(), 0);
    }

    #[test]
    fn test_inactive_drops_events() {
        let mut bridge = GtkEventBridge::new();
        bridge.set_active(false);
        bridge.push_focus(true);
        assert_eq!(bridge.pending_count(), 0);
    }
}
