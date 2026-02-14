//! Qt event translation.
//!
//! Translates Qt events (`QEvent` subclasses) into Liquide events.
//! Uses Qt's event filter mechanism to intercept events before they
//! reach the widget.

use serde::{Deserialize, Serialize};

/// Qt key modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QtModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
    pub keypad: bool,
}

/// Keyboard event (from `QKeyEvent`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QtKeyEvent {
    /// Qt key code (`Qt::Key`).
    pub key: u32,
    /// Scan code.
    pub scan_code: u32,
    /// Text produced by the key.
    pub text: String,
    /// Pressed or released.
    pub pressed: bool,
    /// Auto-repeat flag.
    pub auto_repeat: bool,
    /// Modifiers.
    pub modifiers: QtModifiers,
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtMouseButton {
    None,
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// Mouse event (from `QMouseEvent`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QtMouseEvent {
    pub x: f64,
    pub y: f64,
    pub global_x: f64,
    pub global_y: f64,
    pub button: QtMouseButton,
    pub kind: QtMouseEventKind,
    pub modifiers: QtModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtMouseEventKind {
    Press,
    Release,
    DoubleClick,
    Move,
}

/// Wheel event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QtWheelEvent {
    pub pixel_delta_x: f64,
    pub pixel_delta_y: f64,
    pub angle_delta_x: f64,
    pub angle_delta_y: f64,
    pub x: f64,
    pub y: f64,
    pub modifiers: QtModifiers,
    pub inverted: bool,
}

/// Tablet event (pen input).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QtTabletEvent {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub rotation: f64,
    pub device: QtTabletDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtTabletDevice {
    Stylus,
    Eraser,
    Cursor,
    Airbrush,
}

/// Bridged event union.
#[derive(Debug, Clone)]
pub enum QtBridgedEvent {
    Key(QtKeyEvent),
    Mouse(QtMouseEvent),
    Wheel(QtWheelEvent),
    Tablet(QtTabletEvent),
    FocusIn,
    FocusOut,
    Enter,
    Leave,
    Resize { width: u32, height: u32 },
    Close,
}

/// Bridge that translates Qt events into Liquide events.
pub struct QtEventBridge {
    event_queue: Vec<QtBridgedEvent>,
    active: bool,
}

impl QtEventBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_queue: Vec::new(),
            active: true,
        }
    }

    pub fn push(&mut self, event: QtBridgedEvent) {
        if self.active {
            self.event_queue.push(event);
        }
    }

    pub fn drain(&mut self) -> Vec<QtBridgedEvent> {
        std::mem::take(&mut self.event_queue)
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.event_queue.len()
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

impl Default for QtEventBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bridge() {
        let mut bridge = QtEventBridge::new();
        bridge.push(QtBridgedEvent::Key(QtKeyEvent {
            key: 0x41,
            scan_code: 30,
            text: "a".to_string(),
            pressed: true,
            auto_repeat: false,
            modifiers: QtModifiers::default(),
        }));
        bridge.push(QtBridgedEvent::FocusIn);
        assert_eq!(bridge.pending_count(), 2);
        let events = bridge.drain();
        assert_eq!(events.len(), 2);
    }
}
