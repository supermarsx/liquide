//! Queue message types and message result.

/// Numeric window identifier (matches `liquide-shell`'s `WindowId(u64)`).
pub type WindowId = u64;

/// Return value from a message handler.
pub type MessageResult = i64;

/// Sentinel value for "no window" / broadcast messages.
pub const WINDOW_BROADCAST: WindowId = 0;

/// Desktop shell message types for the LiquiDE desktop environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    // ── Painting ────────────────────────────────────────────────────────
    /// Client-area repaint needed (synthesized from invalid region).
    Paint,
    /// Non-client (decoration) repaint needed.
    NcPaint,

    // ── Mouse input ─────────────────────────────────────────────────────
    MouseMove,
    MouseDown,
    MouseUp,
    MouseWheel,
    MouseEnter,
    MouseLeave,

    // ── Keyboard input ──────────────────────────────────────────────────
    KeyDown,
    KeyUp,
    /// Translated character input (after dead-key / IME composition).
    KeyChar,

    // ── Focus ───────────────────────────────────────────────────────────
    FocusGained,
    FocusLost,

    // ── Activation ──────────────────────────────────────────────────────
    /// Window activated within its own thread/queue.
    Activate,
    /// Window deactivated within its own thread/queue.
    Deactivate,
    /// Application-level activation change (different thread gained foreground).
    ActivateApp,

    // ── Window lifecycle ────────────────────────────────────────────────
    WindowCreated,
    WindowDestroyed,
    WindowMoved,
    WindowResized,

    // ── Visibility / state ──────────────────────────────────────────────
    Show,
    Hide,
    Minimize,
    Maximize,
    Restore,

    // ── Session / quit ──────────────────────────────────────────────────
    Close,
    Quit,

    // ── Timer ───────────────────────────────────────────────────────────
    Timer(u32),

    // ── Modal cancel ────────────────────────────────────────────────────
    /// Sent to cancel modal loops (move, size, menu tracking).
    CancelMode,

    // ── System notifications ────────────────────────────────────────────
    ThemeChanged,
    DpiChanged,
    DisplayChanged,

    // ── Global hotkey ───────────────────────────────────────────────────
    HotKey(u32),

    // ── User-defined ────────────────────────────────────────────────────
    Custom(u32),

    // ── Noop ────────────────────────────────────────────────────────────
    /// No operation.  Used as a placeholder / sentinel.
    Noop,
}

impl MessageType {
    /// Discriminant used for range filtering.  The ordering is chosen so that
    /// related messages cluster together, enabling efficient range checks in
    /// `MessageFilter`.
    #[must_use]
    pub fn discriminant(&self) -> u32 {
        match self {
            Self::Noop => 0,
            Self::Paint => 1,
            Self::NcPaint => 2,
            Self::MouseMove => 10,
            Self::MouseDown => 11,
            Self::MouseUp => 12,
            Self::MouseWheel => 13,
            Self::MouseEnter => 14,
            Self::MouseLeave => 15,
            Self::KeyDown => 20,
            Self::KeyUp => 21,
            Self::KeyChar => 22,
            Self::FocusGained => 30,
            Self::FocusLost => 31,
            Self::Activate => 40,
            Self::Deactivate => 41,
            Self::ActivateApp => 42,
            Self::WindowCreated => 50,
            Self::WindowDestroyed => 51,
            Self::WindowMoved => 52,
            Self::WindowResized => 53,
            Self::Show => 60,
            Self::Hide => 61,
            Self::Minimize => 62,
            Self::Maximize => 63,
            Self::Restore => 64,
            Self::Close => 70,
            Self::Quit => 71,
            Self::Timer(_) => 80,
            Self::CancelMode => 90,
            Self::ThemeChanged => 100,
            Self::DpiChanged => 101,
            Self::DisplayChanged => 102,
            Self::HotKey(_) => 110,
            Self::Custom(_) => 200,
        }
    }

    /// Returns `true` for mouse-related messages (including move).
    #[must_use]
    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Self::MouseMove
                | Self::MouseDown
                | Self::MouseUp
                | Self::MouseWheel
                | Self::MouseEnter
                | Self::MouseLeave
        )
    }

    /// Returns `true` for keyboard-related messages.
    #[must_use]
    pub fn is_key(&self) -> bool {
        matches!(self, Self::KeyDown | Self::KeyUp | Self::KeyChar)
    }

    /// Returns `true` for any input message (mouse or keyboard).
    #[must_use]
    pub fn is_input(&self) -> bool {
        self.is_mouse() || self.is_key()
    }
}

/// A message in the queue — the fundamental unit of communication.
#[derive(Debug, Clone)]
pub struct QueueMessage {
    /// Target window.  `WINDOW_BROADCAST` (0) means all windows.
    pub target: WindowId,
    /// Message type.
    pub msg: MessageType,
    /// First parameter (button flags, key code, etc.).
    pub wparam: u64,
    /// Second parameter (packed coordinates, delta, etc.).
    pub lparam: i64,
    /// Timestamp in microseconds (monotonic or epoch-based).
    pub time: u64,
    /// Cursor position at the time the message was generated.
    pub pt: (i32, i32),
    /// Extra information attached by the input provider.
    pub extra_info: u64,
}

impl QueueMessage {
    /// Create a minimal message with zeroed auxiliary fields.
    #[must_use]
    pub fn new(target: WindowId, msg: MessageType) -> Self {
        Self {
            target,
            msg,
            wparam: 0,
            lparam: 0,
            time: 0,
            pt: (0, 0),
            extra_info: 0,
        }
    }

    /// Builder: set wparam.
    #[must_use]
    pub fn with_wparam(mut self, wparam: u64) -> Self {
        self.wparam = wparam;
        self
    }

    /// Builder: set lparam.
    #[must_use]
    pub fn with_lparam(mut self, lparam: i64) -> Self {
        self.lparam = lparam;
        self
    }

    /// Builder: set timestamp.
    #[must_use]
    pub fn with_time(mut self, time: u64) -> Self {
        self.time = time;
        self
    }

    /// Builder: set cursor position.
    #[must_use]
    pub fn with_pt(mut self, x: i32, y: i32) -> Self {
        self.pt = (x, y);
        self
    }

    /// Builder: set extra info.
    #[must_use]
    pub fn with_extra_info(mut self, info: u64) -> Self {
        self.extra_info = info;
        self
    }
}
