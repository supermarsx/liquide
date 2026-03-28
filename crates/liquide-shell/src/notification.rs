//! Notification manager with system tray icons, do-not-disturb scheduling,
//! notification history, action buttons, and auto-demotion.
//!
//! Wraps [`liquide_interop::Notification`] with shell-level metadata such as
//! display timestamps, read/dismissed state, and a configurable ring buffer
//! of past notifications.
//!
//! Supports auto-demotion for inactive tray icons and popup-style
//! notification display.

use std::collections::{HashMap, VecDeque};
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
    /// Tray icon auto-demotion timeout in seconds.
    /// Icons with no interaction for this long are moved to the overflow area.
    pub tray_auto_demote_secs: u64,
    /// Optional DND schedule (time-based auto-enable/disable).
    pub dnd_schedule: Option<DndSchedule>,
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
            tray_auto_demote_secs: 300, // 5 minutes
            dnd_schedule: None,
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
// DND schedule
// ---------------------------------------------------------------------------

/// Time-of-day schedule for automatic do-not-disturb activation.
///
/// When configured, DND is automatically enabled during the specified window
/// (e.g. 22:00 - 07:00 for overnight quiet hours).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DndSchedule {
    /// Start hour (0-23).
    pub start_hour: u8,
    /// Start minute (0-59).
    pub start_minute: u8,
    /// End hour (0-23).
    pub end_hour: u8,
    /// End minute (0-59).
    pub end_minute: u8,
}

impl DndSchedule {
    /// Create a new DND schedule.
    #[must_use]
    pub fn new(start_hour: u8, start_minute: u8, end_hour: u8, end_minute: u8) -> Self {
        Self {
            start_hour: start_hour.min(23),
            start_minute: start_minute.min(59),
            end_hour: end_hour.min(23),
            end_minute: end_minute.min(59),
        }
    }

    /// Check whether the given time (hour, minute) falls within the schedule.
    ///
    /// Handles overnight spans (e.g. 22:00 - 07:00) correctly.
    #[must_use]
    pub fn is_active(&self, hour: u8, minute: u8) -> bool {
        let start = self.start_hour as u16 * 60 + self.start_minute as u16;
        let end = self.end_hour as u16 * 60 + self.end_minute as u16;
        let now = hour as u16 * 60 + minute as u16;

        if start <= end {
            // Same-day span: e.g. 09:00 - 17:00
            now >= start && now < end
        } else {
            // Overnight span: e.g. 22:00 - 07:00
            now >= start || now < end
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
    /// Optional progress value (0.0 - 1.0) for progress-style notifications.
    pub progress: Option<f32>,
    /// If true, the notification won't auto-expire.
    pub persistent: bool,
    /// If true, no audible alert is played.
    pub silent: bool,
    /// Optional category tag (e.g. "email", "im", "transfer").
    pub category: Option<String>,
    /// Optional grouping key; notifications with the same key replace each other.
    pub group_key: Option<String>,
}

// ---------------------------------------------------------------------------
// System tray icon
// ---------------------------------------------------------------------------

/// Unique identifier for a system tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrayIconId(pub u64);

impl fmt::Display for TrayIconId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TrayIcon({})", self.0)
    }
}

/// A system tray icon registered by an application.
///
/// A system tray icon with support for tooltip, badge text, context menu,
/// and auto-demotion of inactive icons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayIcon {
    /// Unique ID.
    pub id: TrayIconId,
    /// Application name that owns this icon.
    pub app_name: String,
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// Icon name or path.
    pub icon: String,
    /// Whether the icon is visible (vs. hidden by the app).
    pub visible: bool,
    /// Optional badge text (e.g. "3" for unread count).
    pub badge: Option<String>,
    /// Context menu items shown on right-click.
    pub menu_items: Vec<TrayMenuItem>,
    /// Monotonic timestamp (us) of last user or app interaction.
    pub last_interaction_us: u64,
    /// Whether this icon has been auto-demoted to the overflow area
    /// due to inactivity.
    pub auto_demoted: bool,
}

/// A single item in a tray icon's context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    /// Action identifier returned when the item is clicked.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional icon name.
    pub icon: Option<String>,
    /// Whether the item is enabled (greyed out if false).
    pub enabled: bool,
    /// Optional check state (None = no checkbox, Some(true) = checked).
    pub checked: Option<bool>,
    /// If true, this item is rendered as a separator line.
    pub separator: bool,
}

impl TrayMenuItem {
    /// Create a normal menu item.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            enabled: true,
            checked: None,
            separator: false,
        }
    }

    /// Create a separator.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            icon: None,
            enabled: false,
            checked: None,
            separator: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Notification events
// ---------------------------------------------------------------------------

/// Events emitted by [`NotificationManager::tick`].
#[derive(Debug, Clone)]
pub enum NotificationEvent {
    /// A notification expired and was moved to history.
    Expired(u32),
    /// A tray icon was auto-demoted to the overflow area due to inactivity.
    TrayIconDemoted(TrayIconId),
    /// A notification action button was invoked by the user.
    ActionInvoked {
        /// The notification's shell ID.
        notification_id: u32,
        /// The action key (from `NotificationAction::key`).
        action_id: String,
    },
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Runtime state for the notification subsystem.
///
/// Tracks active (on-screen) notifications, a history ring buffer, system
/// tray icons with auto-demotion, and the current do-not-disturb state.
pub struct NotificationManager {
    config: NotificationConfig,
    active: Vec<ShellNotification>,
    history: VecDeque<ShellNotification>,
    dnd_active: bool,
    next_id: u32,
    // System tray
    tray_icons: HashMap<TrayIconId, TrayIcon>,
    next_tray_id: u64,
    // Pending action invocations (drained on tick)
    pending_actions: Vec<(u32, String)>,
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
            tray_icons: HashMap::new(),
            next_tray_id: 1,
            pending_actions: Vec::new(),
        }
    }

    // ===================================================================
    // Notification posting
    // ===================================================================

    /// Submit a new notification.
    ///
    /// Returns `Some(id)` if the notification was shown, or `None` if it was
    /// suppressed (e.g. by DND rules).
    pub fn notify(&mut self, notification: Notification, now_us: u64) -> Option<u32> {
        self.notify_ext(notification, now_us, None)
    }

    /// Submit a notification with extended options.
    ///
    /// `opts` provides additional shell-level metadata (progress, persistence,
    /// grouping, etc.).  Pass `None` for default behavior matching [`notify`].
    pub fn notify_ext(
        &mut self,
        notification: Notification,
        now_us: u64,
        opts: Option<NotifyOptions>,
    ) -> Option<u32> {
        if !self.should_show(notification.urgency) {
            return None;
        }

        let opts = opts.unwrap_or_default();

        let id = self.next_id;
        self.next_id += 1;

        let persistent = opts.persistent;
        let timeout_us = if persistent {
            // Persistent notifications don't auto-expire; set a far-future value.
            u64::MAX / 2
        } else if notification.timeout_ms > 0 {
            (notification.timeout_ms as u64) * 1000
        } else {
            self.config.default_timeout_ms * 1000
        };

        let shell_notif = ShellNotification {
            id,
            notification,
            shown_at_us: now_us,
            expires_at_us: now_us.saturating_add(timeout_us),
            read: false,
            dismissed: false,
            progress: opts.progress,
            persistent,
            silent: opts.silent,
            category: opts.category,
            group_key: opts.group_key.clone(),
        };

        // Group replacement: if a group_key is set, replace existing notification
        // with the same key.
        if let Some(ref key) = shell_notif.group_key {
            if let Some(pos) = self.active.iter().position(|n| {
                n.group_key.as_ref() == Some(key)
            }) {
                let replaced = self.active.remove(pos);
                self.push_history(replaced);
            }
        }

        // Enforce max_visible by pushing oldest non-persistent to history.
        while self.active.len() >= self.config.max_visible {
            if let Some(pos) = self.active.iter().position(|n| !n.persistent) {
                let oldest = self.active.remove(pos);
                self.push_history(oldest);
            } else {
                // All are persistent, push oldest anyway.
                if let Some(oldest) = self.active.first().cloned() {
                    self.push_history(oldest);
                    self.active.remove(0);
                }
                break;
            }
        }

        self.active.push(shell_notif);
        Some(id)
    }

    // ===================================================================
    // Dismiss / action
    // ===================================================================

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

    /// Invoke a notification action button.
    ///
    /// The action is queued and reported via [`NotificationEvent::ActionInvoked`]
    /// on the next [`tick`].  Non-persistent notifications are auto-dismissed.
    pub fn invoke_action(&mut self, id: u32, action_id: &str) {
        self.pending_actions.push((id, action_id.to_string()));
        // Auto-dismiss non-persistent notifications after action.
        let is_persistent = self
            .active
            .iter()
            .find(|n| n.id == id)
            .map_or(false, |n| n.persistent);
        if !is_persistent {
            self.dismiss(id);
        }
    }

    /// Update the progress value on an active notification.
    ///
    /// Does nothing if the notification is not found.
    pub fn update_progress(&mut self, id: u32, progress: f32) {
        if let Some(n) = self.active.iter_mut().find(|n| n.id == id) {
            n.progress = Some(progress.clamp(0.0, 1.0));
        }
    }

    // ===================================================================
    // Tick
    // ===================================================================

    /// Advance time: expire old notifications, auto-demote tray icons, and
    /// drain pending action events.
    ///
    /// Returns the IDs of notifications that were expired (for backward
    /// compatibility) and emits [`NotificationEvent`]s via `events_out`.
    pub fn tick(&mut self, now_us: u64) -> Vec<u32> {
        self.tick_with_events(now_us).0
    }

    /// Like [`tick`], but also returns the full list of events.
    pub fn tick_with_events(&mut self, now_us: u64) -> (Vec<u32>, Vec<NotificationEvent>) {
        let mut expired_ids = Vec::new();
        let mut events = Vec::new();

        // Expire notifications
        let mut i = 0;
        while i < self.active.len() {
            if now_us >= self.active[i].expires_at_us {
                let removed = self.active.remove(i);
                expired_ids.push(removed.id);
                events.push(NotificationEvent::Expired(removed.id));
                self.push_history(removed);
            } else {
                i += 1;
            }
        }

        // Auto-demote inactive tray icons
        let demote_threshold_us = self.config.tray_auto_demote_secs * 1_000_000;
        for icon in self.tray_icons.values_mut() {
            if !icon.auto_demoted
                && now_us.saturating_sub(icon.last_interaction_us) > demote_threshold_us
            {
                icon.auto_demoted = true;
                events.push(NotificationEvent::TrayIconDemoted(icon.id));
            }
        }

        // Drain pending action invocations
        for (nid, action_id) in self.pending_actions.drain(..) {
            events.push(NotificationEvent::ActionInvoked {
                notification_id: nid,
                action_id,
            });
        }

        (expired_ids, events)
    }

    // ===================================================================
    // Do-Not-Disturb
    // ===================================================================

    /// Enable or disable do-not-disturb mode.
    pub fn set_dnd(&mut self, active: bool) {
        self.dnd_active = active;
    }

    /// Toggle do-not-disturb mode. Returns the new state.
    pub fn toggle_dnd(&mut self) -> bool {
        self.dnd_active = !self.dnd_active;
        self.dnd_active
    }

    /// Whether DND is currently active.
    #[must_use]
    pub fn is_dnd(&self) -> bool {
        self.dnd_active
    }

    /// Set the DND schedule.
    pub fn set_dnd_schedule(&mut self, schedule: Option<DndSchedule>) {
        self.config.dnd_schedule = schedule;
    }

    /// Get the DND schedule.
    #[must_use]
    pub fn dnd_schedule(&self) -> Option<&DndSchedule> {
        self.config.dnd_schedule.as_ref()
    }

    /// Check the DND schedule against the given wall-clock time and
    /// auto-enable/disable DND accordingly.
    ///
    /// This should be called periodically (e.g. once per tick) with the
    /// current wall-clock hour and minute.
    pub fn check_dnd_schedule(&mut self, hour: u8, minute: u8) {
        if let Some(ref schedule) = self.config.dnd_schedule {
            self.dnd_active = schedule.is_active(hour, minute);
        }
    }

    // ===================================================================
    // System tray icon management
    // ===================================================================

    /// Register a new system tray icon. Returns its unique ID.
    pub fn add_tray_icon(
        &mut self,
        app_name: &str,
        tooltip: &str,
        icon: &str,
        now_us: u64,
    ) -> TrayIconId {
        let id = TrayIconId(self.next_tray_id);
        self.next_tray_id += 1;
        self.tray_icons.insert(
            id,
            TrayIcon {
                id,
                app_name: app_name.into(),
                tooltip: tooltip.into(),
                icon: icon.into(),
                visible: true,
                badge: None,
                menu_items: Vec::new(),
                last_interaction_us: now_us,
                auto_demoted: false,
            },
        );
        id
    }

    /// Remove a system tray icon.
    pub fn remove_tray_icon(&mut self, id: TrayIconId) {
        self.tray_icons.remove(&id);
    }

    /// Update a tray icon's properties.
    ///
    /// Pass `None` for any field to leave it unchanged.
    /// Updating an icon resets its inactivity timer and promotes it from
    /// the overflow area.
    pub fn update_tray_icon(
        &mut self,
        id: TrayIconId,
        tooltip: Option<&str>,
        icon: Option<&str>,
        badge: Option<Option<&str>>,
        now_us: u64,
    ) {
        if let Some(tray) = self.tray_icons.get_mut(&id) {
            if let Some(t) = tooltip {
                tray.tooltip = t.into();
            }
            if let Some(i) = icon {
                tray.icon = i.into();
            }
            if let Some(b) = badge {
                tray.badge = b.map(|s| s.into());
            }
            tray.last_interaction_us = now_us;
            tray.auto_demoted = false; // promote on update
        }
    }

    /// Set the context menu items for a tray icon.
    pub fn set_tray_menu(&mut self, id: TrayIconId, items: Vec<TrayMenuItem>) {
        if let Some(tray) = self.tray_icons.get_mut(&id) {
            tray.menu_items = items;
        }
    }

    /// Record a user interaction with a tray icon (click, hover, etc.).
    /// Resets the inactivity timer and promotes from overflow.
    pub fn touch_tray_icon(&mut self, id: TrayIconId, now_us: u64) {
        if let Some(tray) = self.tray_icons.get_mut(&id) {
            tray.last_interaction_us = now_us;
            tray.auto_demoted = false;
        }
    }

    /// Get all tray icons.
    #[must_use]
    pub fn tray_icons(&self) -> &HashMap<TrayIconId, TrayIcon> {
        &self.tray_icons
    }

    /// Get visible (promoted) tray icons — not auto-demoted.
    #[must_use]
    pub fn visible_tray_icons(&self) -> Vec<&TrayIcon> {
        self.tray_icons
            .values()
            .filter(|i| i.visible && !i.auto_demoted)
            .collect()
    }

    /// Get overflow (auto-demoted) tray icons.
    #[must_use]
    pub fn overflow_tray_icons(&self) -> Vec<&TrayIcon> {
        self.tray_icons
            .values()
            .filter(|i| i.visible && i.auto_demoted)
            .collect()
    }

    /// Number of registered tray icons.
    #[must_use]
    pub fn tray_icon_count(&self) -> usize {
        self.tray_icons.len()
    }

    // ===================================================================
    // Accessors
    // ===================================================================

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

    /// Mark all notifications (active and history) as read.
    pub fn mark_all_read(&mut self) {
        for n in &mut self.active {
            n.read = true;
        }
        for n in &mut self.history {
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
            "NotificationManager(active={}, history={}, dnd={}, tray={})",
            self.active.len(),
            self.history.len(),
            self.dnd_active,
            self.tray_icons.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Extended notify options
// ---------------------------------------------------------------------------

/// Extended options for [`NotificationManager::notify_ext`].
#[derive(Debug, Clone, Default)]
pub struct NotifyOptions {
    /// Optional progress value (0.0 - 1.0).
    pub progress: Option<f32>,
    /// Don't auto-expire this notification.
    pub persistent: bool,
    /// Suppress audible alert.
    pub silent: bool,
    /// Category tag.
    pub category: Option<String>,
    /// Group key for notification stacking/replacement.
    pub group_key: Option<String>,
}
