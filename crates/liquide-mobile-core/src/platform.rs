//! Platform abstraction for iOS and Android specifics.

use serde::{Deserialize, Serialize};

/// Target mobile platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// Apple iOS / iPadOS.
    Ios,
    /// Google Android.
    Android,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ios => write!(f, "ios"),
            Self::Android => write!(f, "android"),
        }
    }
}

/// Capabilities and metadata reported by the host platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// The platform.
    pub platform: Platform,
    /// Operating system version string.
    pub os_version: String,
    /// Device model string (e.g. "iPhone 15 Pro", "Pixel 8").
    pub device_model: String,
    /// Whether biometric authentication is available.
    pub supports_biometric: bool,
    /// Whether a hardware video decoder is available.
    pub has_hardware_decoder: bool,
    /// Whether the device has a haptic engine.
    pub has_haptic: bool,
    /// Whether the platform can keep the screen always on.
    pub screen_always_on_capable: bool,
    /// Whether push notifications are supported.
    pub push_notification_capable: bool,
}

/// Types of biometric authentication available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiometricType {
    /// No biometric capability.
    None,
    /// Fingerprint reader.
    Fingerprint,
    /// Face recognition (e.g. Face ID).
    FaceId,
    /// Iris scanner.
    Iris,
}

impl std::fmt::Display for BiometricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Fingerprint => write!(f, "fingerprint"),
            Self::FaceId => write!(f, "face-id"),
            Self::Iris => write!(f, "iris"),
        }
    }
}

/// An entry stored in the platform secure keystore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreEntry {
    /// Key identifier.
    pub key: String,
    /// Creation timestamp (epoch seconds).
    pub created_at: u64,
}

/// Abstraction over the platform secure keystore (iOS Keychain / Android Keystore).
pub struct PlatformKeystore {
    entries: Vec<(String, String, u64)>,
}

impl PlatformKeystore {
    /// Create a new empty keystore.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Store a value under the given key. Overwrites existing entries with
    /// the same key.
    pub fn store(&mut self, key: impl Into<String>, value: impl Into<String>, created_at: u64) {
        let key = key.into();
        self.entries.retain(|(k, _, _)| *k != key);
        self.entries.push((key, value.into(), created_at));
    }

    /// Load the value for the given key, if it exists.
    #[must_use]
    pub fn load(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, v, _)| v.as_str())
    }

    /// Delete an entry by key. Returns `true` if something was removed.
    pub fn delete(&mut self, key: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(k, _, _)| k != key);
        self.entries.len() < before
    }

    /// List all stored entry metadata.
    #[must_use]
    pub fn list(&self) -> Vec<KeystoreEntry> {
        self.entries
            .iter()
            .map(|(k, _, ts)| KeystoreEntry {
                key: k.clone(),
                created_at: *ts,
            })
            .collect()
    }
}

impl Default for PlatformKeystore {
    fn default() -> Self {
        Self::new()
    }
}

/// Application lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLifecycle {
    /// The app is in the foreground and active.
    Active,
    /// The app has moved to the background.
    Background,
    /// The app is suspended by the OS.
    Suspended,
    /// The app is being terminated.
    Terminating,
}

impl std::fmt::Display for AppLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Background => write!(f, "background"),
            Self::Suspended => write!(f, "suspended"),
            Self::Terminating => write!(f, "terminating"),
        }
    }
}
