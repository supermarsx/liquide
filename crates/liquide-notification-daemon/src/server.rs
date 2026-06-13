//! The notification daemon core — [`NotificationServer`].
//!
//! The server receives notifications, passes them through rate limiting and
//! priority queuing, then dispatches to the registered [`NotificationHandler`].

use crate::handler::NotificationHandler;
use crate::history::NotificationHistory;
use crate::queue::NotificationQueue;
use crate::spec::{CloseReason, Notification, Urgency};

/// Server identity information returned by `get_server_info()`.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Human-readable server name.
    pub name: String,
    /// Vendor name.
    pub vendor: String,
    /// Server version string.
    pub version: String,
    /// Notification protocol version supported.
    pub spec_version: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "LiquiDE Notification Daemon".to_string(),
            vendor: "LiquiDE".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            spec_version: "1.2".to_string(),
        }
    }
}

/// Default timeout in milliseconds for notifications that use the server default (-1).
const DEFAULT_TIMEOUT_MS: i32 = 5000;

/// The notification daemon. Orchestrates the queue, history, handler dispatch,
/// and active notification tracking.
pub struct NotificationServer {
    /// The registered handler (shell callback).
    handler: Option<Box<dyn NotificationHandler>>,
    /// Priority queue for incoming notifications.
    queue: NotificationQueue,
    /// Notification history log.
    history: NotificationHistory,
    /// Server identity.
    info: ServerInfo,
    /// Currently active (displayed) notifications: (id, notification, displayed_at_ms).
    active: Vec<(u32, Notification, u64)>,
    /// Default timeout applied when `expire_timeout == -1`.
    default_timeout_ms: i32,
}

impl NotificationServer {
    /// Creates a new notification server with default settings.
    pub fn new() -> Self {
        Self {
            handler: None,
            queue: NotificationQueue::new(),
            history: NotificationHistory::default(),
            info: ServerInfo::default(),
            active: Vec::new(),
            default_timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Registers a notification handler (typically the shell).
    /// Replaces any previously registered handler.
    pub fn register_handler(&mut self, handler: Box<dyn NotificationHandler>) {
        self.handler = Some(handler);
    }

    /// Returns whether a handler is registered.
    pub fn has_handler(&self) -> bool {
        self.handler.is_some()
    }

    /// Submits a notification to the server. Returns the assigned ID.
    ///
    /// The notification passes through rate limiting and is enqueued. If a
    /// handler is registered, notifications are immediately dispatched.
    /// Returns 0 if the notification was rate-limited.
    pub fn notify(&mut self, notification: Notification) -> u32 {
        self.notify_at(notification, Self::now_ms())
    }

    /// Submits a notification with an explicit timestamp (for testing).
    pub fn notify_at(&mut self, notification: Notification, now_ms: u64) -> u32 {
        if notification.replaces_id != 0 {
            if let Some(id) = self.replace_active(notification.clone(), now_ms) {
                return id;
            }
        }

        let id = match self.queue.enqueue_at(notification, now_ms) {
            Some(id) => id,
            None => return 0, // Rate-limited.
        };

        // Drain the queue into the handler.
        self.dispatch(now_ms);

        id
    }

    /// Closes an active notification.
    pub fn close_notification(&mut self, id: u32, reason: CloseReason) {
        self.close_notification_at(id, reason, Self::now_ms());
    }

    /// Closes an active notification with an explicit timestamp (for testing).
    pub fn close_notification_at(&mut self, id: u32, reason: CloseReason, now_ms: u64) {
        // Remove from active set.
        if let Some(pos) = self.active.iter().position(|(aid, _, _)| *aid == id) {
            let (_, notif, displayed_at) = self.active.remove(pos);
            self.history.record(&notif, reason, displayed_at, now_ms);

            if let Some(handler) = self.handler.as_mut() {
                handler.on_close(id, reason);
            }
        } else {
            // Maybe still in the queue — remove it without recording history.
            self.queue.remove(id);
        }
    }

    /// Returns the list of capabilities this server supports.
    pub fn get_capabilities(&self) -> Vec<String> {
        vec![
            "body".to_string(),
            "body-markup".to_string(),
            "actions".to_string(),
            "icon-static".to_string(),
            "persistence".to_string(),
            "action-icons".to_string(),
            "body-hyperlinks".to_string(),
        ]
    }

    /// Returns server identity information.
    pub fn get_server_info(&self) -> ServerInfo {
        self.info.clone()
    }

    /// Invokes an action on an active notification.
    pub fn invoke_action(&mut self, id: u32, action_key: &str) {
        // Check the notification exists and has this action.
        let has_action = self.active.iter().any(|(aid, notif, _)| {
            *aid == id && notif.actions.iter().any(|(k, _)| k == action_key)
        });

        if has_action {
            if let Some(handler) = self.handler.as_mut() {
                handler.on_action_invoked(id, action_key);
            }

            // If the notification is not "resident", close it after action.
            let is_resident = self
                .active
                .iter()
                .find(|(aid, _, _)| *aid == id)
                .map(|(_, n, _)| n.hints.resident)
                .unwrap_or(false);

            if !is_resident {
                self.close_notification(id, CloseReason::Dismissed);
            }
        }
    }

    /// Expires notifications whose timeout has elapsed.
    ///
    /// Call this periodically (e.g., once per second) from the event loop.
    pub fn tick(&mut self, now_ms: u64) {
        let mut expired = Vec::new();

        for (id, notif, displayed_at) in &self.active {
            // Critical notifications never auto-expire.
            if notif.urgency() == Urgency::Critical {
                continue;
            }

            let timeout = if notif.expire_timeout == -1 {
                self.default_timeout_ms
            } else if notif.expire_timeout == 0 {
                continue; // Never expire.
            } else {
                notif.expire_timeout
            };

            let elapsed = now_ms.saturating_sub(*displayed_at);
            if elapsed >= timeout as u64 {
                expired.push(*id);
            }
        }

        for id in expired {
            self.close_notification_at(id, CloseReason::Expired, now_ms);
        }

        // Also try to dispatch any queued notifications.
        self.dispatch(now_ms);
    }

    /// Sets the default timeout for notifications with `expire_timeout == -1`.
    pub fn set_default_timeout(&mut self, ms: i32) {
        self.default_timeout_ms = ms;
    }

    /// Returns the number of currently active (displayed) notifications.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Returns the number of queued (not yet displayed) notifications.
    pub fn pending_count(&self) -> usize {
        self.queue.pending_count()
    }

    /// Returns a reference to the notification history.
    pub fn history(&self) -> &NotificationHistory {
        &self.history
    }

    /// Returns a mutable reference to the notification history.
    pub fn history_mut(&mut self) -> &mut NotificationHistory {
        &mut self.history
    }

    /// Returns a mutable reference to the queue.
    pub fn queue_mut(&mut self) -> &mut NotificationQueue {
        &mut self.queue
    }

    /// Returns a reference to an active notification by ID.
    pub fn get_active(&self, id: u32) -> Option<&Notification> {
        self.active
            .iter()
            .find(|(aid, _, _)| *aid == id)
            .map(|(_, n, _)| n)
    }

    /// Returns the currently active (displayed) notifications in display order,
    /// as `(id, notification, displayed_at_ms)` triples.
    ///
    /// Read-only accessor (t52-e1): lets a consumer (the shell notification
    /// center) render directly off the daemon's canonical active set instead of
    /// keeping a duplicate cache. The daemon remains the single source of the
    /// active notification data.
    pub fn active_notifications(&self) -> &[(u32, Notification, u64)] {
        &self.active
    }

    /// Dispatches queued notifications to the handler.
    fn dispatch(&mut self, now_ms: u64) {
        if self.handler.is_none() {
            return;
        }

        while let Some(notif) = self.queue.dequeue() {
            let id = notif.id;
            let handler = self.handler.as_mut().unwrap();
            handler.on_notify(&notif);
            self.active.push((id, notif, now_ms));
        }
    }

    fn replace_active(&mut self, mut notification: Notification, now_ms: u64) -> Option<u32> {
        let id = notification.replaces_id;
        let pos = self.active.iter().position(|(aid, _, _)| *aid == id)?;

        notification.id = id;

        if let Some(handler) = self.handler.as_mut() {
            handler.on_notify(&notification);
        }

        self.active[pos] = (id, notification, now_ms);
        Some(id)
    }

    /// Platform-agnostic current time in ms.
    fn now_ms() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for NotificationServer {
    fn default() -> Self {
        Self::new()
    }
}
