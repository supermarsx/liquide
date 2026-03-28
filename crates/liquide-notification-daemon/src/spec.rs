//! Desktop notification types.
//!
//! Defines the core notification data types:
//! [`Notification`], [`NotificationHints`], [`Urgency`], and [`CloseReason`].

use serde::{Deserialize, Serialize};

/// Urgency level for a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Urgency {
    /// Low urgency — informational, may be silently logged.
    Low = 0,
    /// Normal urgency — default for most notifications.
    Normal = 1,
    /// Critical urgency — stays visible until explicitly dismissed.
    Critical = 2,
}

impl Urgency {
    /// Returns the numeric priority (higher = more urgent).
    pub fn priority(self) -> u8 {
        match self {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        }
    }
}

impl Default for Urgency {
    fn default() -> Self {
        Urgency::Normal
    }
}

/// Reason a notification was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CloseReason {
    /// The notification expired (timeout elapsed).
    Expired = 1,
    /// The user dismissed the notification.
    Dismissed = 2,
    /// The notification was closed programmatically via `CloseNotification`.
    Closed = 3,
    /// Undefined/reserved reason.
    Undefined = 4,
}

/// Optional hints attached to a notification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationHints {
    /// The urgency level.
    pub urgency: Option<Urgency>,
    /// A category string (e.g. "email", "im.received", "transfer.complete").
    pub category: Option<String>,
    /// The desktop-entry name of the sending application.
    pub desktop_entry: Option<String>,
    /// Path or URI to an image to display.
    pub image_path: Option<String>,
    /// Named sound from the sound theme to play.
    pub sound_name: Option<String>,
    /// If true, suppress the notification sound.
    pub suppress_sound: bool,
    /// If true, the notification is transient (bypasses history).
    pub transient: bool,
    /// If true, action key strings are icon names rather than display labels.
    pub action_icons: bool,
    /// If true, the notification stays after an action is invoked.
    pub resident: bool,
}

/// A desktop notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Server-assigned unique ID (0 before assignment).
    pub id: u32,
    /// Name of the application sending the notification.
    pub app_name: String,
    /// ID of an existing notification this one replaces (0 = no replacement).
    pub replaces_id: u32,
    /// Icon name or path.
    pub icon: String,
    /// Single-line summary text.
    pub summary: String,
    /// Multi-line body text (may contain markup if server supports "body-markup").
    pub body: String,
    /// Action pairs: (key, display_label). "default" key = click on notification body.
    pub actions: Vec<(String, String)>,
    /// Optional hints.
    pub hints: NotificationHints,
    /// Timeout in milliseconds. -1 = server default, 0 = never expire.
    pub expire_timeout: i32,
}

impl Notification {
    /// Creates a new notification with the given summary.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            id: 0,
            app_name: String::new(),
            replaces_id: 0,
            icon: String::new(),
            summary: summary.into(),
            body: String::new(),
            actions: Vec::new(),
            hints: NotificationHints::default(),
            expire_timeout: -1,
        }
    }

    /// Builder: set the application name.
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    /// Builder: set the body text.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// Builder: set the icon name or path.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Builder: set the urgency hint.
    pub fn with_urgency(mut self, urgency: Urgency) -> Self {
        self.hints.urgency = Some(urgency);
        self
    }

    /// Builder: add an action.
    pub fn with_action(mut self, key: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push((key.into(), label.into()));
        self
    }

    /// Builder: set expiration timeout in milliseconds.
    pub fn with_timeout(mut self, ms: i32) -> Self {
        self.expire_timeout = ms;
        self
    }

    /// Builder: set the replaces_id.
    pub fn with_replaces_id(mut self, id: u32) -> Self {
        self.replaces_id = id;
        self
    }

    /// Returns the effective urgency (defaults to Normal).
    pub fn urgency(&self) -> Urgency {
        self.hints.urgency.unwrap_or(Urgency::Normal)
    }
}
