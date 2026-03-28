//! The [`NotificationHandler`] trait — implemented by the shell to display notifications.

use crate::spec::{CloseReason, Notification};

/// Trait for the shell (or any consumer) to implement in order to receive
/// notification events from the daemon.
pub trait NotificationHandler: Send {
    /// Called when a new notification arrives. The handler should display
    /// it and return the server-assigned notification ID.
    fn on_notify(&mut self, notification: &Notification) -> u32;

    /// Called when a notification is closed for the given reason.
    fn on_close(&mut self, id: u32, reason: CloseReason);

    /// Called when the user invokes an action on a notification.
    /// `action_key` is the key string from the notification's actions list.
    fn on_action_invoked(&mut self, id: u32, action_key: &str);
}
