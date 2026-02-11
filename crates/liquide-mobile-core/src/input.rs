//! Touch and gesture input handling for mobile clients.

use serde::{Deserialize, Serialize};

use crate::gesture::GestureKind;

/// How touch input is mapped to remote pointer events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchMode {
    /// Touch position maps directly to remote cursor position.
    Direct,
    /// Touch delta moves the remote cursor like a trackpad.
    Trackpad,
    /// Direct mode with trackpad gestures for scrolling and right-click.
    Hybrid,
}

impl std::fmt::Display for TouchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::Trackpad => write!(f, "trackpad"),
            Self::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Phase of a single touch contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchPhase {
    /// Finger touched the screen.
    Began,
    /// Finger moved on the screen.
    Moved,
    /// Finger lifted from the screen.
    Ended,
    /// Touch was cancelled by the system.
    Cancelled,
}

impl std::fmt::Display for TouchPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Began => write!(f, "began"),
            Self::Moved => write!(f, "moved"),
            Self::Ended => write!(f, "ended"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A single touch contact event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchEvent {
    /// Unique identifier for this touch contact.
    pub id: u32,
    /// X coordinate in display points.
    pub x: f32,
    /// Y coordinate in display points.
    pub y: f32,
    /// Phase of the touch.
    pub phase: TouchPhase,
    /// Pressure (0.0 to 1.0), if available.
    pub pressure: f32,
    /// Timestamp in milliseconds since some reference point.
    pub timestamp: u64,
}

/// A recognized gesture event with associated data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestureEvent {
    /// The kind of gesture recognized.
    pub kind: GestureKind,
    /// X coordinate of the gesture focal point.
    pub position_x: f32,
    /// Y coordinate of the gesture focal point.
    pub position_y: f32,
    /// Scale factor for pinch gestures.
    pub scale: Option<f32>,
    /// Horizontal delta for pan/swipe gestures.
    pub delta_x: Option<f32>,
    /// Vertical delta for pan/swipe gestures.
    pub delta_y: Option<f32>,
    /// Timestamp in milliseconds.
    pub timestamp: u64,
}

/// A mouse action to send to the remote desktop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MouseAction {
    /// Move the pointer to (x, y).
    Move { x: f32, y: f32 },
    /// Left-click at (x, y).
    LeftClick { x: f32, y: f32 },
    /// Right-click at (x, y).
    RightClick { x: f32, y: f32 },
    /// Middle-click at (x, y).
    MiddleClick { x: f32, y: f32 },
    /// Scroll by (dx, dy) at (x, y).
    Scroll { x: f32, y: f32, dx: f32, dy: f32 },
    /// Begin a drag at (x, y).
    DragStart { x: f32, y: f32 },
    /// Continue a drag to (x, y).
    DragMove { x: f32, y: f32 },
    /// End a drag at (x, y).
    DragEnd { x: f32, y: f32 },
}

/// Translates mobile touch and gesture events into remote mouse actions.
pub struct InputTranslator {
    mode: TouchMode,
    /// Last known cursor position for trackpad mode.
    cursor_x: f32,
    cursor_y: f32,
    /// Whether a drag is in progress.
    dragging: bool,
}

impl InputTranslator {
    /// Create a new translator with the given touch mode.
    #[must_use]
    pub fn new(mode: TouchMode) -> Self {
        Self {
            mode,
            cursor_x: 0.0,
            cursor_y: 0.0,
            dragging: false,
        }
    }

    /// Current touch mode.
    #[must_use]
    pub fn mode(&self) -> TouchMode {
        self.mode
    }

    /// Set the touch mode.
    pub fn set_mode(&mut self, mode: TouchMode) {
        self.mode = mode;
    }

    /// Whether a drag gesture is currently active.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Translate a raw touch event into a mouse action.
    #[must_use]
    pub fn translate_touch(&mut self, event: &TouchEvent) -> Option<MouseAction> {
        match self.mode {
            TouchMode::Direct => self.translate_direct(event),
            TouchMode::Trackpad => self.translate_trackpad(event),
            TouchMode::Hybrid => self.translate_direct(event),
        }
    }

    /// Translate a gesture event into a mouse action.
    #[must_use]
    pub fn translate_gesture(&mut self, event: &GestureEvent) -> Option<MouseAction> {
        match event.kind {
            GestureKind::SingleTap => Some(MouseAction::LeftClick {
                x: event.position_x,
                y: event.position_y,
            }),
            GestureKind::DoubleTap => {
                // Double-tap sends two left clicks (handled at higher level);
                // we report one click here.
                Some(MouseAction::LeftClick {
                    x: event.position_x,
                    y: event.position_y,
                })
            }
            GestureKind::TwoFingerTap => Some(MouseAction::RightClick {
                x: event.position_x,
                y: event.position_y,
            }),
            GestureKind::LongPress => {
                self.dragging = true;
                Some(MouseAction::DragStart {
                    x: event.position_x,
                    y: event.position_y,
                })
            }
            GestureKind::LongPressDrag => {
                if self.dragging {
                    Some(MouseAction::DragMove {
                        x: event.position_x,
                        y: event.position_y,
                    })
                } else {
                    self.dragging = true;
                    Some(MouseAction::DragStart {
                        x: event.position_x,
                        y: event.position_y,
                    })
                }
            }
            GestureKind::Pan => {
                let dx = event.delta_x.unwrap_or(0.0);
                let dy = event.delta_y.unwrap_or(0.0);
                if self.mode == TouchMode::Trackpad {
                    self.cursor_x += dx;
                    self.cursor_y += dy;
                    Some(MouseAction::Move {
                        x: self.cursor_x,
                        y: self.cursor_y,
                    })
                } else {
                    Some(MouseAction::Scroll {
                        x: event.position_x,
                        y: event.position_y,
                        dx,
                        dy,
                    })
                }
            }
            GestureKind::Pinch => {
                // Pinch maps to scroll for zooming.
                let scale = event.scale.unwrap_or(1.0);
                let dy = (scale - 1.0) * 100.0;
                Some(MouseAction::Scroll {
                    x: event.position_x,
                    y: event.position_y,
                    dx: 0.0,
                    dy,
                })
            }
            GestureKind::ThreeFingerSwipe => Some(MouseAction::MiddleClick {
                x: event.position_x,
                y: event.position_y,
            }),
            GestureKind::EdgeSwipeLeft | GestureKind::EdgeSwipeRight => {
                // Edge swipes are consumed by the mobile UI, not forwarded.
                None
            }
        }
    }

    /// End any active drag.
    pub fn end_drag(&mut self, x: f32, y: f32) -> Option<MouseAction> {
        if self.dragging {
            self.dragging = false;
            Some(MouseAction::DragEnd { x, y })
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn translate_direct(&mut self, event: &TouchEvent) -> Option<MouseAction> {
        match event.phase {
            TouchPhase::Began | TouchPhase::Moved => Some(MouseAction::Move {
                x: event.x,
                y: event.y,
            }),
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.dragging {
                    self.dragging = false;
                    Some(MouseAction::DragEnd {
                        x: event.x,
                        y: event.y,
                    })
                } else {
                    None
                }
            }
        }
    }

    fn translate_trackpad(&mut self, event: &TouchEvent) -> Option<MouseAction> {
        match event.phase {
            TouchPhase::Moved => {
                // In trackpad mode the delta is computed at a higher level;
                // raw events move the cursor by the touch coordinates offset.
                self.cursor_x = event.x;
                self.cursor_y = event.y;
                Some(MouseAction::Move {
                    x: self.cursor_x,
                    y: self.cursor_y,
                })
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.dragging {
                    self.dragging = false;
                    Some(MouseAction::DragEnd {
                        x: self.cursor_x,
                        y: self.cursor_y,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
