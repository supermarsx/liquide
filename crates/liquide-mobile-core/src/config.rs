//! Mobile client configuration types.

use serde::{Deserialize, Serialize};

use crate::input::TouchMode;

/// Orientation lock preference for the mobile client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrientationLock {
    /// No lock -- follow device orientation.
    None,
    /// Lock to portrait.
    Portrait,
    /// Lock to landscape.
    Landscape,
}

impl std::fmt::Display for OrientationLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Portrait => write!(f, "portrait"),
            Self::Landscape => write!(f, "landscape"),
        }
    }
}

/// Top-level configuration for the mobile client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    /// Remote server address (host:port).
    pub server_address: String,
    /// Username for authentication.
    pub username: String,
    /// Whether to automatically reconnect on disconnection.
    pub auto_reconnect: bool,
    /// Delay between reconnection attempts in milliseconds.
    pub reconnect_delay_ms: u64,
    /// Maximum number of reconnection attempts before giving up.
    pub max_reconnect_attempts: u32,
    /// Preferred video codec name.
    pub preferred_codec: String,
    /// Whether to enable adaptive quality adjustment.
    pub adaptive_quality: bool,
    /// Touch input mode.
    pub touch_mode: TouchMode,
    /// Whether to show the extended virtual key bar.
    pub show_extended_keys: bool,
    /// Whether to provide haptic feedback on input.
    pub haptic_feedback: bool,
    /// Whether to keep the device screen on during a session.
    pub keep_screen_on: bool,
    /// Orientation lock preference.
    pub orientation_lock: OrientationLock,
}

impl Default for MobileConfig {
    fn default() -> Self {
        Self {
            server_address: String::new(),
            username: String::new(),
            auto_reconnect: true,
            reconnect_delay_ms: 2000,
            max_reconnect_attempts: 10,
            preferred_codec: "h264".to_string(),
            adaptive_quality: true,
            touch_mode: TouchMode::Direct,
            show_extended_keys: true,
            haptic_feedback: true,
            keep_screen_on: true,
            orientation_lock: OrientationLock::None,
        }
    }
}
