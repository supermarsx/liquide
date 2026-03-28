//! Notification grouping by application.
//!
//! Groups notifications from the same application together, allowing the shell
//! to display a collapsed summary (e.g. "3 messages from Chat") or expand to
//! show all individual notifications.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a notification within the daemon.
pub type NotificationId = u64;

/// A group of notifications from the same application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationGroup {
    /// The application identifier (matches `Notification::app_name`).
    pub app_id: String,
    /// Ordered list of notification IDs in this group (newest last).
    pub notifications: Vec<NotificationId>,
    /// Whether this group is collapsed (showing only summary + count badge).
    pub collapsed: bool,
    /// Number of notifications summarized in the collapsed view.
    pub summary_count: u32,
}

impl NotificationGroup {
    /// Creates a new empty group for the given application.
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            notifications: Vec::new(),
            collapsed: false,
            summary_count: 0,
        }
    }

    /// Returns the number of notifications in this group.
    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    /// Returns whether this group has no notifications.
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Returns the most recent notification ID, if any.
    pub fn latest(&self) -> Option<NotificationId> {
        self.notifications.last().copied()
    }

    /// Adds a notification ID to this group.
    pub fn add(&mut self, id: NotificationId) {
        self.notifications.push(id);
        self.summary_count = self.notifications.len() as u32;
    }

    /// Removes a notification ID from this group. Returns whether it was present.
    pub fn remove(&mut self, id: NotificationId) -> bool {
        if let Some(pos) = self.notifications.iter().position(|&nid| nid == id) {
            self.notifications.remove(pos);
            self.summary_count = self.notifications.len() as u32;
            true
        } else {
            false
        }
    }
}

/// Information about a notification used for grouping. The caller provides
/// this so the grouping module does not depend on [`crate::spec::Notification`]
/// directly (keeping it decoupled and testable).
#[derive(Debug, Clone)]
pub struct GroupableNotification {
    /// Unique notification ID.
    pub id: NotificationId,
    /// Application name (used as the grouping key).
    pub app_id: String,
}

/// Groups a slice of notifications by their application ID.
///
/// Returns groups in the order their first notification appears. Within each
/// group, notifications are ordered chronologically (same order as the input).
pub fn group_notifications(notifications: &[GroupableNotification]) -> Vec<NotificationGroup> {
    let mut groups: Vec<NotificationGroup> = Vec::new();
    let mut index_map: HashMap<&str, usize> = HashMap::new();

    for notif in notifications {
        if let Some(&idx) = index_map.get(notif.app_id.as_str()) {
            groups[idx].add(notif.id);
        } else {
            let idx = groups.len();
            let mut group = NotificationGroup::new(&notif.app_id);
            group.add(notif.id);
            groups.push(group);
            index_map.insert(&notif.app_id, idx);
        }
    }

    groups
}

/// Collapses a group so the shell displays only the latest notification
/// plus a count badge. Sets `collapsed = true` and updates `summary_count`.
pub fn collapse_group(group: &mut NotificationGroup) {
    group.collapsed = true;
    group.summary_count = group.notifications.len() as u32;
}

/// Expands a previously collapsed group so all notifications are visible.
pub fn expand_group(group: &mut NotificationGroup) {
    group.collapsed = false;
}
