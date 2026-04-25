//! Notification daemon for the LiquiDE desktop environment.
//!
//! This crate implements a system-level notification service that receives
//! notifications from applications (via platform IPC on Linux/Windows/macOS)
//! and feeds them to the desktop shell for display.
//!
//! # Architecture
//!
//! - [`spec`] — Notification types ([`Notification`],
//!   [`Urgency`], [`CloseReason`], [`NotificationHints`]).
//! - [`server`] — The daemon core ([`NotificationServer`]) that coordinates
//!   queuing, rate limiting, handler dispatch, and timeout expiry.
//! - [`handler`] — The [`NotificationHandler`] trait that the shell implements
//!   to receive notification events.
//! - [`queue`] — Priority queue with urgency-based ordering.
//! - [`history`] — Persistent notification log for "missed notifications".
//! - [`rate_limiter`] — Per-application sliding-window rate limiting.
//! - [`platform`] — Platform-specific IPC bridges (D-Bus, PowerShell, osascript).
//! - [`grouping`] — Notification grouping by application.
//! - [`log`] — Notification event log (audit trail).
//! - [`dnd`] — Do-Not-Disturb scheduling.
//! - [`layout`] — Notification stacking layout computation.
//! - [`app_settings`] — Per-application notification settings.
//!
//! # Usage
//!
//! ```rust
//! use liquide_notification_daemon::*;
//!
//! // Create the server.
//! let mut server = NotificationServer::new();
//!
//! // Build and submit a notification.
//! let notif = Notification::new("Download complete")
//!     .with_app_name("file-manager")
//!     .with_body("report.pdf has finished downloading")
//!     .with_urgency(Urgency::Normal)
//!     .with_action("open", "Open File");
//!
//! let id = server.notify(notif);
//! ```

pub mod animation;
pub mod app_settings;
pub mod dnd;
pub mod grouping;
pub mod handler;
pub mod history;
pub mod layout;
pub mod log;
pub mod platform;
pub mod queue;
pub mod rate_limiter;
pub mod server;
pub mod spec;
pub mod theme;

pub use animation::{AnimationPhase, NotificationAnimationState};
pub use app_settings::{AppNotificationSettings, AppSettings};
pub use dnd::{DndSchedule, DndTimeRange};
pub use grouping::{NotificationGroup, NotificationId};
pub use handler::NotificationHandler;
pub use history::{HistoryEntry, NotificationHistory};
pub use layout::{
    LayoutAnchor, NotificationInfo, NotificationLayout, NotificationPosition, Priority, Rect,
};
pub use log::{LogAction, LogEntry, NotificationLog};
pub use platform::{PlatformError, PlatformResult};
pub use queue::NotificationQueue;
pub use rate_limiter::RateLimiter;
pub use server::{NotificationServer, ServerInfo};
pub use spec::{CloseReason, Notification, NotificationHints, Urgency};
pub use theme::{NotificationColor, NotificationTheme, UrgencyColors};

#[cfg(test)]
mod tests;
