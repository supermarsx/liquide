//! Notification manager with do-not-disturb, history, and expiry.
//!
//! Wraps [`liquide_interop::Notification`] with shell-level metadata such as
//! display timestamps, read/dismissed state, and a configurable ring buffer
//! of past notifications.

use std::collections::VecDeque;
use std::fmt;

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::SceneNode;
use liquide_interop::notification::{Notification, Urgency};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Persistent configuration for the notification subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Maximum number of notifications visible at once.
    pub max_visible: usize,
    /// Default display timeout in milliseconds.
    pub default_timeout_ms: u64,
    /// Whether do-not-disturb is enabled at startup.
    pub dnd_enabled: bool,
    /// Allow critical-urgency notifications even during DND.
    pub dnd_allow_critical: bool,
    /// Maximum number of entries kept in the history ring.
    pub history_capacity: usize,
    /// Whether to stack (group) notifications from the same app.
    pub stacking: bool,
    /// Screen corner where notifications appear.
    pub position: NotificationPosition,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            max_visible: 5,
            default_timeout_ms: 5000,
            dnd_enabled: false,
            dnd_allow_critical: true,
            history_capacity: 100,
            stacking: true,
            position: NotificationPosition::TopRight,
        }
    }
}

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// Corner of the screen where notifications are anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl fmt::Display for NotificationPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopRight => write!(f, "TopRight"),
            Self::TopLeft => write!(f, "TopLeft"),
            Self::BottomRight => write!(f, "BottomRight"),
            Self::BottomLeft => write!(f, "BottomLeft"),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell notification wrapper
// ---------------------------------------------------------------------------

/// A notification augmented with shell-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellNotification {
    /// Shell-assigned unique ID.
    pub id: u32,
    /// The underlying protocol notification.
    pub notification: Notification,
    /// Monotonic timestamp when first shown (microseconds).
    pub shown_at_us: u64,
    /// Monotonic timestamp when the notification should expire.
    pub expires_at_us: u64,
    /// Whether the user has seen/acknowledged this notification.
    pub read: bool,
    /// Whether the user explicitly dismissed it.
    pub dismissed: bool,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Runtime state for the notification subsystem.
///
/// Tracks active (on-screen) notifications, a history ring buffer, and the
/// current do-not-disturb state.
pub struct NotificationManager {
    config: NotificationConfig,
    active: Vec<ShellNotification>,
    history: VecDeque<ShellNotification>,
    dnd_active: bool,
    next_id: u32,
}

impl NotificationManager {
    /// Create a new manager from the given configuration.
    #[must_use]
    pub fn new(config: NotificationConfig) -> Self {
        let dnd = config.dnd_enabled;
        Self {
            config,
            active: Vec::new(),
            history: VecDeque::new(),
            dnd_active: dnd,
            next_id: 1,
        }
    }

    /// Submit a new notification.
    ///
    /// Returns `Some(id)` if the notification was shown, or `None` if it was
    /// suppressed (e.g. by DND rules).
    pub fn notify(&mut self, notification: Notification, now_us: u64) -> Option<u32> {
        if !self.should_show(notification.urgency) {
            return None;
        }

        let id = self.next_id;
        self.next_id += 1;

        let timeout_us = if notification.timeout_ms > 0 {
            (notification.timeout_ms as u64) * 1000
        } else {
            self.config.default_timeout_ms * 1000
        };

        let shell_notif = ShellNotification {
            id,
            notification,
            shown_at_us: now_us,
            expires_at_us: now_us + timeout_us,
            read: false,
            dismissed: false,
        };

        // Enforce max_visible by pushing oldest to history.
        while self.active.len() >= self.config.max_visible {
            if let Some(oldest) = self.active.first().cloned() {
                self.push_history(oldest);
                self.active.remove(0);
            }
        }

        self.active.push(shell_notif);
        Some(id)
    }

    /// Dismiss a single notification by its shell ID.
    pub fn dismiss(&mut self, id: u32) {
        if let Some(pos) = self.active.iter().position(|n| n.id == id) {
            let mut removed = self.active.remove(pos);
            removed.dismissed = true;
            self.push_history(removed);
        }
    }

    /// Dismiss all active notifications.
    pub fn dismiss_all(&mut self) {
        let drained: Vec<_> = self.active.drain(..).collect();
        for mut n in drained {
            n.dismissed = true;
            self.push_history(n);
        }
    }

    /// Advance time and expire old notifications.
    ///
    /// Returns the IDs of notifications that were expired.
    pub fn tick(&mut self, now_us: u64) -> Vec<u32> {
        let mut expired_ids = Vec::new();
        let mut i = 0;
        while i < self.active.len() {
            if now_us >= self.active[i].expires_at_us {
                let removed = self.active.remove(i);
                expired_ids.push(removed.id);
                self.push_history(removed);
            } else {
                i += 1;
            }
        }
        expired_ids
    }

    /// Enable or disable do-not-disturb mode.
    pub fn set_dnd(&mut self, active: bool) {
        self.dnd_active = active;
    }

    /// Whether DND is currently active.
    #[must_use]
    pub fn is_dnd(&self) -> bool {
        self.dnd_active
    }

    /// Currently visible notifications.
    #[must_use]
    pub fn active_notifications(&self) -> &[ShellNotification] {
        &self.active
    }

    /// Historical notifications (most recent first).
    #[must_use]
    pub fn history(&self) -> &VecDeque<ShellNotification> {
        &self.history
    }

    /// Count of unread notifications across active and history.
    #[must_use]
    pub fn unread_count(&self) -> usize {
        self.active.iter().filter(|n| !n.read).count()
            + self.history.iter().filter(|n| !n.read).count()
    }

    /// Mark a notification as read.
    pub fn mark_read(&mut self, id: u32) {
        if let Some(n) = self.active.iter_mut().find(|n| n.id == id) {
            n.read = true;
        }
        if let Some(n) = self.history.iter_mut().find(|n| n.id == id) {
            n.read = true;
        }
    }

    /// Clear all history entries.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Active notification count.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Total history length.
    #[must_use]
    pub fn history_count(&self) -> usize {
        self.history.len()
    }

    /// The active configuration.
    #[must_use]
    pub fn config(&self) -> &NotificationConfig {
        &self.config
    }

    /// Build the scene graph for active notifications.
    pub fn build_scene(
        &self,
        screen: Rect,
        theme: &crate::theme::ShellTheme,
        layout: Option<&crate::css_integration::NotificationLayout>,
    ) -> SceneNode {
        use crate::scene_builder::*;
        use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

        let defaults = crate::css_integration::NotificationLayout::default();
        let layout = layout.unwrap_or(&defaults);

        let mut container = SceneNode::new(
            NODE_NOTIFICATION_BASE,
            SceneNodeKind::Overlay,
            NodeProperties::new(screen).with_z_order(960),
        );

        if self.active.is_empty() {
            return container;
        }

        let notif_width = layout.width;
        let notif_height = layout.height;
        let gap = layout.gap;
        let margin = layout.margin;
        let top_offset = layout.top_offset;

        for (i, _notif) in self.active.iter().enumerate() {
            let (nx, ny) = match self.config.position {
                NotificationPosition::TopRight => (
                    screen.x + screen.width - notif_width - margin,
                    screen.y + top_offset + margin + i as f32 * (notif_height + gap),
                ),
                NotificationPosition::TopLeft => (
                    screen.x + margin,
                    screen.y + top_offset + margin + i as f32 * (notif_height + gap),
                ),
                NotificationPosition::BottomRight => (
                    screen.x + screen.width - notif_width - margin,
                    screen.y + screen.height - margin - (i as f32 + 1.0) * (notif_height + gap),
                ),
                NotificationPosition::BottomLeft => (
                    screen.x + margin,
                    screen.y + screen.height - margin - (i as f32 + 1.0) * (notif_height + gap),
                ),
            };

            let notif_bounds = Rect::new(nx, ny, notif_width, notif_height);
            container.add_child(SceneNode::new(
                NODE_NOTIFICATION_BASE + 1 + i as u64,
                SceneNodeKind::Glass(GlassParams {
                    blur_radius: layout.blur_radius,
                    tint_color: theme.notification_glass_tint,
                    inner_glow: true,
                    parallax: false,
                }),
                NodeProperties::new(notif_bounds).with_z_order(961),
            ));
        }

        container
    }

    /// Determine whether a notification with the given urgency should be shown
    /// under the current DND state.
    #[must_use]
    pub fn should_show(&self, urgency: Urgency) -> bool {
        if !self.dnd_active {
            return true;
        }
        // In DND, only critical notifications pass if allowed.
        self.config.dnd_allow_critical && urgency == Urgency::Critical
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn push_history(&mut self, notif: ShellNotification) {
        if self.history.len() >= self.config.history_capacity {
            self.history.pop_back();
        }
        self.history.push_front(notif);
    }
}

impl fmt::Display for NotificationManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotificationManager(active={}, history={}, dnd={})",
            self.active.len(),
            self.history.len(),
            self.dnd_active,
        )
    }
}
