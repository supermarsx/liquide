//! USB device redirection for the LiquiDE remote desktop protocol.
//!
//! Provides USB device forwarding with tiered capabilities, policy enforcement,
//! smart card redirection, file transfer, bandwidth limiting, and audit logging.

pub mod device;
pub mod config;
pub mod policy;
pub mod message;
pub mod manager;
pub mod smartcard;
pub mod file_transfer;
pub mod audit;
pub mod bandwidth;
pub mod tier;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the USB subsystem.
#[derive(Debug, Error)]
pub enum UsbError {
    /// USB subsystem is disabled in configuration.
    #[error("USB redirection is disabled")]
    Disabled,

    /// Device was denied by policy.
    #[error("policy denied device {device}: {reason}")]
    PolicyDenied { device: String, reason: String },

    /// Session device limit exceeded.
    #[error("device limit exceeded (max {max})")]
    DeviceLimitExceeded { max: u32 },

    /// Bandwidth budget exceeded.
    #[error("bandwidth exceeded (budget {budget_mbps} Mbps)")]
    BandwidthExceeded { budget_mbps: u32 },

    /// Security key forwarding is blocked.
    #[error("security key blocked: {vid_pid}")]
    SecurityKeyBlocked { vid_pid: String },

    /// APDU command timed out.
    #[error("APDU timeout: {elapsed_ms}ms exceeded max {max_ms}ms")]
    ApduTimeout { elapsed_ms: u64, max_ms: u64 },

    /// Invalid device specification.
    #[error("invalid device: {0}")]
    InvalidDevice(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for USB operations.
pub type Result<T> = std::result::Result<T, UsbError>;

// Re-exports
pub use device::{DeviceClass, VidPid, DeviceInfo, DeviceState, UsbDevice, SecurityKeyDb};
pub use config::{TierMode, TransportChannel, PinEntry, UsbConfig, SmartCardConfig, SecurityKeyOverrides};
pub use policy::{PolicyResult, UsbPolicy};
pub use message::{
    DeviceRedirectionMsg, UsbAttachRequest, UsbAttachResponse, DetachReason,
    UsbDetachNotification, UsbDataTransfer, CapabilityAnnouncement,
};
pub use manager::UsbManager;
pub use smartcard::{ApduCommand, ApduResponse, SmartCardReaderState, SmartCardReader};
pub use file_transfer::{MountPoint, FileEntry, FileOperation, FileTransferRequest, FileTransferResponse};
pub use audit::{AuditLevel, UsbAuditEvent};
pub use bandwidth::BandwidthLimiter;
pub use tier::{UsbTier, TierCapabilities, TierNegotiator};
