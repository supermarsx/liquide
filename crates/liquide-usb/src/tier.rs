//! USB tier negotiation and capability discovery.

use crate::config::TierMode;
use serde::{Deserialize, Serialize};

/// USB capability tier levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbTier {
    /// Tier 1: File transfer only (drive redirection).
    FileTransfer = 1,
    /// Tier 2: Smart card redirection.
    SmartCard = 2,
    /// Tier 3: Full USB/IP passthrough.
    FullUsbIp = 3,
}

/// Capabilities advertised by a client or server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierCapabilities {
    pub file_transfer: bool,
    pub smartcard: bool,
    pub full_usb_ip: bool,
}

/// Negotiates the highest mutually supported USB tier.
pub struct TierNegotiator;

impl TierNegotiator {
    /// Negotiate the USB tier based on mode and mutual capabilities.
    #[must_use]
    pub fn negotiate(
        mode: TierMode,
        client: &TierCapabilities,
        server: &TierCapabilities,
    ) -> UsbTier {
        match mode {
            TierMode::Tier1 => UsbTier::FileTransfer,
            TierMode::Tier2 => UsbTier::SmartCard,
            TierMode::Tier3 => UsbTier::FullUsbIp,
            TierMode::Auto => {
                if client.full_usb_ip && server.full_usb_ip {
                    UsbTier::FullUsbIp
                } else if client.smartcard && server.smartcard {
                    UsbTier::SmartCard
                } else {
                    UsbTier::FileTransfer
                }
            }
        }
    }
}
