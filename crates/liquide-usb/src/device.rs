//! USB device types, identification, and security key database.

use std::fmt;
use serde::{Serialize, Deserialize};

/// USB device class categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DeviceClass {
    Filesystem = 0,
    Printer = 1,
    SerialPort = 2,
    SmartCard = 3,
    RawUsb = 4,
}

impl fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem => write!(f, "Filesystem"),
            Self::Printer => write!(f, "Printer"),
            Self::SerialPort => write!(f, "SerialPort"),
            Self::SmartCard => write!(f, "SmartCard"),
            Self::RawUsb => write!(f, "RawUsb"),
        }
    }
}

/// USB Vendor ID and Product ID pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VidPid {
    pub vendor: u16,
    pub product: u16,
}

impl VidPid {
    /// Check if this VID:PID matches a pattern string.
    ///
    /// Supports wildcard `*` for either vendor or product, e.g. `"1050:*"`.
    #[must_use]
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        let Some((vendor_str, product_str)) = pattern.split_once(':') else {
            return false;
        };

        let vendor_match = if vendor_str == "*" {
            true
        } else {
            u16::from_str_radix(vendor_str, 16)
                .is_ok_and(|v| v == self.vendor)
        };

        let product_match = if product_str == "*" {
            true
        } else {
            u16::from_str_radix(product_str, 16)
                .is_ok_and(|p| p == self.product)
        };

        vendor_match && product_match
    }
}

impl fmt::Display for VidPid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}:{:04X}", self.vendor, self.product)
    }
}

/// Information about a USB device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub vid_pid: VidPid,
    pub device_class: DeviceClass,
    pub name: String,
    pub serial: Option<String>,
    pub interfaces: u8,
}

/// Current state of a USB device in the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Available,
    Forwarding,
    Connected,
    Disconnected,
    Blocked,
}

/// A USB device tracked by the manager.
pub struct UsbDevice {
    info: DeviceInfo,
    state: DeviceState,
    session_id: Option<String>,
    attached_at: Option<u64>,
}

impl UsbDevice {
    /// Create a new USB device in the Available state.
    #[must_use]
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            info,
            state: DeviceState::Available,
            session_id: None,
            attached_at: None,
        }
    }

    /// Get the device info.
    #[must_use]
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Get the current device state.
    #[must_use]
    pub fn state(&self) -> DeviceState {
        self.state
    }

    /// Get the session ID if attached.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Get the attachment timestamp.
    #[must_use]
    pub fn attached_at(&self) -> Option<u64> {
        self.attached_at
    }

    /// Set the device state.
    pub fn set_state(&mut self, state: DeviceState) {
        self.state = state;
    }

    /// Set the session ID and attachment time.
    pub fn attach(&mut self, session_id: String, timestamp: u64) {
        self.session_id = Some(session_id);
        self.attached_at = Some(timestamp);
        self.state = DeviceState::Forwarding;
    }

    /// Clear session attachment.
    pub fn detach(&mut self) {
        self.session_id = None;
        self.attached_at = None;
        self.state = DeviceState::Disconnected;
    }
}

/// Database of known security key VID:PID patterns.
#[derive(Debug, Clone)]
pub struct SecurityKeyDb {
    known_patterns: Vec<String>,
}

impl SecurityKeyDb {
    /// Create a new security key database with known vendor patterns.
    #[must_use]
    pub fn new() -> Self {
        Self {
            known_patterns: vec![
                // Yubico
                "1050:*".to_string(),
                // SoloKeys
                "1209:5070".to_string(),
                "1209:5071".to_string(),
                // Feitian
                "096E:*".to_string(),
                // Google Titan
                "18D1:5026".to_string(),
                "18D1:5028".to_string(),
                // Nitrokey
                "20A0:4287".to_string(),
                "20A0:42B1".to_string(),
                "20A0:42B2".to_string(),
            ],
        }
    }

    /// Create a security key database with custom overrides.
    #[must_use]
    pub fn with_overrides(additional: &[String], exceptions: &[String]) -> Self {
        let mut db = Self::new();
        for pattern in additional {
            db.known_patterns.push(pattern.clone());
        }
        db.known_patterns.retain(|p| !exceptions.contains(p));
        db
    }

    /// Check whether a given VID:PID is a known security key.
    #[must_use]
    pub fn is_security_key(&self, vid_pid: &VidPid) -> bool {
        self.known_patterns.iter().any(|p| vid_pid.matches_pattern(p))
    }
}

impl Default for SecurityKeyDb {
    fn default() -> Self {
        Self::new()
    }
}
