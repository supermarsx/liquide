use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Result;

/// Notification urgency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

/// An action that can be performed on a notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

impl NotificationAction {
    #[must_use]
    pub fn new(key: &str, label: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
        }
    }
}

/// A desktop notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: Option<String>,
    pub urgency: Urgency,
    pub timeout_ms: i32,
    pub actions: Vec<NotificationAction>,
}

impl Notification {
    #[must_use]
    pub fn new(app_name: &str, summary: &str) -> Self {
        Self {
            id: 0,
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: String::new(),
            icon: None,
            urgency: Urgency::Normal,
            timeout_ms: -1,
            actions: Vec::new(),
        }
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Notification(id={}, app={}, summary={})",
            self.id, self.app_name, self.summary
        )
    }
}

/// Trait for notification delivery.
pub trait NotificationService: Send {
    /// Send a notification, returning its assigned ID.
    fn notify(&mut self, n: Notification) -> Result<u32>;
    /// Close a notification by ID.
    fn close(&mut self, id: u32) -> Result<()>;
    /// List active notifications.
    fn list(&self) -> &[Notification];
}

/// Null notification service — discards all notifications.
pub struct NullNotificationService;

impl NotificationService for NullNotificationService {
    fn notify(&mut self, _n: Notification) -> Result<u32> {
        Ok(0)
    }

    fn close(&mut self, _id: u32) -> Result<()> {
        Ok(())
    }

    fn list(&self) -> &[Notification] {
        &[]
    }
}

/// In-memory notification service — stores notifications for testing.
pub struct MemoryNotificationService {
    notifications: Vec<Notification>,
    next_id: u32,
}

impl MemoryNotificationService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 1,
        }
    }
}

impl Default for MemoryNotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationService for MemoryNotificationService {
    fn notify(&mut self, mut n: Notification) -> Result<u32> {
        let id = self.next_id;
        self.next_id += 1;
        n.id = id;
        self.notifications.push(n);
        Ok(id)
    }

    fn close(&mut self, id: u32) -> Result<()> {
        self.notifications.retain(|n| n.id != id);
        Ok(())
    }

    fn list(&self) -> &[Notification] {
        &self.notifications
    }
}
