//! Desktop notification support.
//!
//! Provides the [`NativeNotifications`] trait for showing and dismissing
//! notifications, and a [`NullNativeNotifications`] for testing.

use serde::{Deserialize, Serialize};

use crate::PlatformResult;

/// Parameters for creating a desktop notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeNotificationParams {
    /// Notification title.
    pub title: String,
    /// Notification body text.
    pub body: String,
    /// Optional icon name or path.
    pub icon: Option<String>,
    /// Urgency level (e.g. "low", "normal", "critical").
    pub urgency: String,
    /// Auto-dismiss timeout in milliseconds (0 = never).
    pub timeout_ms: u64,
    /// Action button labels.
    pub actions: Vec<String>,
    /// Whether to play a notification sound.
    pub sound: bool,
}

/// Backend for desktop notifications.
pub trait NativeNotifications: Send {
    /// Show a notification and return its unique identifier.
    fn show(&mut self, params: NativeNotificationParams) -> PlatformResult<u32>;

    /// Dismiss / close an active notification.
    fn dismiss(&mut self, id: u32) -> PlatformResult<()>;
}

/// A [`NativeNotifications`] backend that tracks notification IDs
/// in memory without displaying anything.
#[derive(Debug, Default)]
pub struct NullNativeNotifications {
    next_id: u32,
}

impl NullNativeNotifications {
    /// Create a new null notifications backend.
    #[must_use]
    pub fn new() -> Self {
        Self { next_id: 1 }
    }
}

impl NativeNotifications for NullNativeNotifications {
    fn show(&mut self, _params: NativeNotificationParams) -> PlatformResult<u32> {
        let id = self.next_id;
        self.next_id += 1;
        Ok(id)
    }

    fn dismiss(&mut self, _id: u32) -> PlatformResult<()> {
        Ok(())
    }
}
