//! USB subsystem configuration types.

use crate::device::DeviceClass;
use serde::{Deserialize, Serialize};

/// Tier negotiation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TierMode {
    Auto,
    Tier1,
    Tier2,
    Tier3,
}

/// Transport channel mode for USB data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportChannel {
    Dedicated,
    Shared,
}

/// PIN entry location for smart card operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinEntry {
    ClientSide,
    ServerSide,
}

/// Overrides for the security key database.
#[derive(Debug, Clone)]
pub struct SecurityKeyOverrides {
    pub additional: Vec<String>,
    pub exceptions: Vec<String>,
}

impl Default for SecurityKeyOverrides {
    fn default() -> Self {
        Self {
            additional: Vec::new(),
            exceptions: Vec::new(),
        }
    }
}

/// Configuration for the USB redirection subsystem.
#[derive(Debug, Clone)]
pub struct UsbConfig {
    pub enabled: bool,
    pub tier: TierMode,
    pub transport_channel: TransportChannel,
    pub allowed_device_classes: Vec<DeviceClass>,
    pub allowed_vid_pid: Vec<String>,
    pub blocked_vid_pid: Vec<String>,
    pub blocked_device_classes: Vec<DeviceClass>,
    pub max_devices_per_session: u32,
    pub max_bandwidth_mbps: u32,
    pub audit_log: bool,
    pub mass_storage_read_only: bool,
    pub security_key_overrides: SecurityKeyOverrides,
}

impl Default for UsbConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tier: TierMode::Auto,
            transport_channel: TransportChannel::Dedicated,
            allowed_device_classes: Vec::new(),
            allowed_vid_pid: Vec::new(),
            blocked_vid_pid: Vec::new(),
            blocked_device_classes: Vec::new(),
            max_devices_per_session: 5,
            max_bandwidth_mbps: 50,
            audit_log: true,
            mass_storage_read_only: false,
            security_key_overrides: SecurityKeyOverrides::default(),
        }
    }
}

/// Configuration for smart card redirection.
#[derive(Debug, Clone)]
pub struct SmartCardConfig {
    pub enabled: bool,
    pub pin_entry: PinEntry,
    pub apdu_timeout_ms: u64,
    pub max_readers: u32,
}

impl Default for SmartCardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pin_entry: PinEntry::ClientSide,
            apdu_timeout_ms: 5000,
            max_readers: 4,
        }
    }
}
