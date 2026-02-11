//! Input capture, keyboard layout management, and IME support.

use std::fmt;

/// Scope of keyboard/mouse capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureScope {
    All,
    Application,
    None,
}

impl fmt::Display for CaptureScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::All => "All",
            Self::Application => "Application",
            Self::None => "None",
        };
        f.write_str(label)
    }
}

/// Keyboard layout descriptor.
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    pub name: String,
    pub locale: String,
}

impl Default for KeyboardLayout {
    fn default() -> Self {
        Self {
            name: "US".to_string(),
            locale: "en-US".to_string(),
        }
    }
}

/// Input method editor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeMode {
    Auto,
    ClientSide,
    ServerSide,
}

/// Active IME pre-edit state.
#[derive(Debug, Clone)]
pub struct ImePreedit {
    pub text: String,
    pub cursor_pos: usize,
    pub selection_range: Option<(usize, usize)>,
}

/// Direction for swipe gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Touch gesture types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchGesture {
    PinchZoom { scale: f64 },
    LongPress,
    TwoFingerTap,
    ThreeFingerSwipe { direction: SwipeDirection },
    FourFingerSwipe { direction: SwipeDirection },
    EdgeSwipe { edge: SwipeDirection },
}

/// Manages input capture scope, keyboard layout, and IME state.
pub struct InputManager {
    capture_scope: CaptureScope,
    keyboard_layout: KeyboardLayout,
    ime_mode: ImeMode,
    current_preedit: Option<ImePreedit>,
    active_touch_points: u32,
}

impl InputManager {
    /// Create a new input manager with defaults.
    #[must_use]
    pub fn new(capture_scope: CaptureScope, ime_mode: ImeMode) -> Self {
        Self {
            capture_scope,
            keyboard_layout: KeyboardLayout::default(),
            ime_mode,
            current_preedit: None,
            active_touch_points: 0,
        }
    }

    /// Set the capture scope.
    pub fn set_capture_scope(&mut self, scope: CaptureScope) {
        self.capture_scope = scope;
    }

    /// Current capture scope.
    #[must_use]
    pub fn capture_scope(&self) -> CaptureScope {
        self.capture_scope
    }

    /// Set the keyboard layout.
    pub fn set_keyboard_layout(&mut self, layout: KeyboardLayout) {
        self.keyboard_layout = layout;
    }

    /// Current keyboard layout.
    #[must_use]
    pub fn keyboard_layout(&self) -> &KeyboardLayout {
        &self.keyboard_layout
    }

    /// Determine whether a key event should be captured (sent to server)
    /// based on the current capture scope.
    #[must_use]
    pub fn should_capture_key(&self, is_app_focused: bool) -> bool {
        match self.capture_scope {
            CaptureScope::All => true,
            CaptureScope::Application => is_app_focused,
            CaptureScope::None => false,
        }
    }

    /// Handle an IME pre-edit update from the OS.
    pub fn handle_ime_preedit(&mut self, preedit: ImePreedit) {
        self.current_preedit = Some(preedit);
    }

    /// Clear the current IME pre-edit.
    pub fn clear_preedit(&mut self) {
        self.current_preedit = None;
    }

    /// Record the start of a new touch point.
    pub fn record_touch_start(&mut self) {
        self.active_touch_points = self.active_touch_points.saturating_add(1);
    }

    /// Record the end of a touch point.
    pub fn record_touch_end(&mut self) {
        self.active_touch_points = self.active_touch_points.saturating_sub(1);
    }

    /// Number of currently active touch points.
    #[must_use]
    pub fn active_touch_count(&self) -> u32 {
        self.active_touch_points
    }
}
