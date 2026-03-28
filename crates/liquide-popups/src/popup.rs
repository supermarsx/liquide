//! Core popup types: Popup, PopupType, PopupConfig, PopupId.

use crate::anchor::AnchorConfig;
use crate::Rect;

/// Unique popup identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PopupId(pub u64);

impl PopupId {
    /// Create a new popup id from a raw u64.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for PopupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Popup({})", self.0)
    }
}

/// Unique window identifier (matches `liquide-shell`'s `WindowId(u64)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Window({})", self.0)
    }
}

/// Classification of popup windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopupType {
    /// Informational hover popup (auto-dismiss).
    Tooltip,
    /// Right-click menu (dismiss on click outside or selection).
    ContextMenu,
    /// Combo box / select dropdown (dismiss on selection or click outside).
    Dropdown,
    /// Modal dialog (blocks interaction with parent).
    Dialog,
    /// Notification toast (auto-dismiss with timeout).
    Notification,
    /// Anchored popup (like a popover/callout).
    Popover,
    /// Splash screen (dismiss on click or timeout).
    Splash,
}

impl std::fmt::Display for PopupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tooltip => write!(f, "Tooltip"),
            Self::ContextMenu => write!(f, "ContextMenu"),
            Self::Dropdown => write!(f, "Dropdown"),
            Self::Dialog => write!(f, "Dialog"),
            Self::Notification => write!(f, "Notification"),
            Self::Popover => write!(f, "Popover"),
            Self::Splash => write!(f, "Splash"),
        }
    }
}

/// Configuration passed to `PopupManager::open()`.
#[derive(Debug, Clone)]
pub struct PopupConfig {
    /// The type of popup.
    pub popup_type: PopupType,
    /// Desired size of the popup.
    pub width: f32,
    pub height: f32,
    /// Optional anchor configuration for positioned popups.
    pub anchor: Option<AnchorConfig>,
    /// The window that owns this popup.
    pub owner: Option<WindowId>,
    /// Whether this popup is modal (blocks interaction with windows behind it).
    pub modal: bool,
    /// Auto-close after this many milliseconds (None = no auto-dismiss).
    pub auto_dismiss_ms: Option<u32>,
    /// Dismiss when the user clicks outside the popup.
    pub dismiss_on_click_outside: bool,
    /// Dismiss when the user presses Escape.
    pub dismiss_on_escape: bool,
    /// Preferred position (used when no anchor is set).
    pub preferred_x: f32,
    pub preferred_y: f32,
}

impl PopupConfig {
    /// Create a tooltip config.
    #[must_use]
    pub fn tooltip(width: f32, height: f32) -> Self {
        Self {
            popup_type: PopupType::Tooltip,
            width,
            height,
            anchor: None,
            owner: None,
            modal: false,
            auto_dismiss_ms: Some(5000),
            dismiss_on_click_outside: true,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a context menu config.
    #[must_use]
    pub fn context_menu(width: f32, height: f32) -> Self {
        Self {
            popup_type: PopupType::ContextMenu,
            width,
            height,
            anchor: None,
            owner: None,
            modal: false,
            auto_dismiss_ms: None,
            dismiss_on_click_outside: true,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a dropdown config.
    #[must_use]
    pub fn dropdown(width: f32, height: f32) -> Self {
        Self {
            popup_type: PopupType::Dropdown,
            width,
            height,
            anchor: None,
            owner: None,
            modal: false,
            auto_dismiss_ms: None,
            dismiss_on_click_outside: true,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a dialog config.
    #[must_use]
    pub fn dialog(width: f32, height: f32, owner: WindowId) -> Self {
        Self {
            popup_type: PopupType::Dialog,
            width,
            height,
            anchor: None,
            owner: Some(owner),
            modal: true,
            auto_dismiss_ms: None,
            dismiss_on_click_outside: false,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a notification config.
    #[must_use]
    pub fn notification(width: f32, height: f32, timeout_ms: u32) -> Self {
        Self {
            popup_type: PopupType::Notification,
            width,
            height,
            anchor: None,
            owner: None,
            modal: false,
            auto_dismiss_ms: Some(timeout_ms),
            dismiss_on_click_outside: true,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a popover config.
    #[must_use]
    pub fn popover(width: f32, height: f32, anchor: AnchorConfig) -> Self {
        Self {
            popup_type: PopupType::Popover,
            width,
            height,
            anchor: Some(anchor),
            owner: None,
            modal: false,
            auto_dismiss_ms: None,
            dismiss_on_click_outside: true,
            dismiss_on_escape: true,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Create a splash config.
    #[must_use]
    pub fn splash(width: f32, height: f32, timeout_ms: u32) -> Self {
        Self {
            popup_type: PopupType::Splash,
            width,
            height,
            anchor: None,
            owner: None,
            modal: false,
            auto_dismiss_ms: Some(timeout_ms),
            dismiss_on_click_outside: true,
            dismiss_on_escape: false,
            preferred_x: 0.0,
            preferred_y: 0.0,
        }
    }

    /// Builder: set preferred position.
    #[must_use]
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.preferred_x = x;
        self.preferred_y = y;
        self
    }

    /// Builder: set owner window.
    #[must_use]
    pub fn owned_by(mut self, owner: WindowId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Builder: set anchor.
    #[must_use]
    pub fn with_anchor(mut self, anchor: AnchorConfig) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Builder: set modal.
    #[must_use]
    pub fn with_modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }
}

/// A managed popup instance.
#[derive(Debug, Clone)]
pub struct Popup {
    /// Unique identifier.
    pub id: PopupId,
    /// Classification.
    pub popup_type: PopupType,
    /// Current bounds (screen-space).
    pub bounds: Rect,
    /// Anchor configuration, if anchored to another element.
    pub anchor: Option<AnchorConfig>,
    /// The window that owns this popup.
    pub owner: Option<WindowId>,
    /// Whether this popup blocks interaction with windows behind it.
    pub modal: bool,
    /// Auto-close timeout in milliseconds.
    pub auto_dismiss_ms: Option<u32>,
    /// Dismiss when user clicks outside.
    pub dismiss_on_click_outside: bool,
    /// Dismiss when user presses Escape.
    pub dismiss_on_escape: bool,
    /// Z-order (always above regular windows).
    pub z_order: i32,
    /// Timestamp (microseconds) when the popup was created.
    pub created_at: u64,
}

impl Popup {
    /// Create a popup from a config, id, computed bounds, z_order, and timestamp.
    #[must_use]
    pub fn from_config(
        id: PopupId,
        config: &PopupConfig,
        bounds: Rect,
        z_order: i32,
        created_at: u64,
    ) -> Self {
        Self {
            id,
            popup_type: config.popup_type,
            bounds,
            anchor: config.anchor.clone(),
            owner: config.owner,
            modal: config.modal,
            auto_dismiss_ms: config.auto_dismiss_ms,
            dismiss_on_click_outside: config.dismiss_on_click_outside,
            dismiss_on_escape: config.dismiss_on_escape,
            z_order,
            created_at,
        }
    }

    /// Hit test: does the point lie within this popup?
    #[must_use]
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.bounds.contains_point(x, y)
    }

    /// Whether this popup should auto-dismiss at the given elapsed time (us).
    #[must_use]
    pub fn should_auto_dismiss(&self, now_us: u64) -> bool {
        if let Some(timeout_ms) = self.auto_dismiss_ms {
            let elapsed_us = now_us.saturating_sub(self.created_at);
            elapsed_us >= (timeout_ms as u64) * 1000
        } else {
            false
        }
    }
}

impl std::fmt::Display for Popup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({}, z={}, modal={})",
            self.popup_type, self.id, self.z_order, self.modal,
        )
    }
}
