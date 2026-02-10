//! High-level clipboard manager with policy enforcement.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::format::ClipboardFormat;
use crate::offer::{ClipboardOffer, ClipboardRequest};
use crate::store::ClipboardStore;
use crate::transfer::ClipboardTransfer;
use crate::{ClipboardError, Result};

/// Policy configuration for clipboard operations.
#[derive(Debug, Clone)]
pub struct ClipboardPolicy {
    pub max_payload_bytes: usize,
    pub allowed_formats: Option<Vec<ClipboardFormat>>,
    pub bidirectional: bool,
}

impl ClipboardPolicy {
    /// Create a permissive default policy.
    #[must_use]
    pub fn default_policy() -> Self {
        Self {
            max_payload_bytes: 16 * 1024 * 1024, // 16 MB
            allowed_formats: None,                // all allowed
            bidirectional: true,
        }
    }
}

impl Default for ClipboardPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// Manages local and remote clipboard stores with policy enforcement.
pub struct ClipboardManager {
    local_store: ClipboardStore,
    remote_store: ClipboardStore,
    policy: ClipboardPolicy,
    transfer: ClipboardTransfer,
    serial: u64,
}

impl ClipboardManager {
    /// Create a new clipboard manager.
    #[must_use]
    pub fn new(policy: ClipboardPolicy) -> Self {
        let max = policy.max_payload_bytes;
        Self {
            local_store: ClipboardStore::new(max),
            remote_store: ClipboardStore::new(max),
            policy,
            transfer: ClipboardTransfer::new(max),
            serial: 0,
        }
    }

    /// Handle a local clipboard offer (local app copies data).
    pub fn handle_local_offer(
        &mut self,
        formats: Vec<ClipboardFormat>,
        data_map: HashMap<ClipboardFormat, Vec<u8>>,
    ) -> Result<ClipboardOffer> {
        // Filter formats by policy
        let allowed_formats: Vec<ClipboardFormat> = formats
            .into_iter()
            .filter(|f| self.is_format_allowed(f))
            .collect();

        if allowed_formats.is_empty() {
            return Err(ClipboardError::FormatNotAvailable);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        // Store data locally
        for (fmt, data) in data_map {
            if self.is_format_allowed(&fmt) {
                if data.len() > self.policy.max_payload_bytes {
                    return Err(ClipboardError::PayloadTooLarge {
                        size: data.len(),
                        max: self.policy.max_payload_bytes,
                    });
                }
                self.local_store.set(fmt, data, 0, now)?;
            }
        }

        self.serial += 1;
        Ok(ClipboardOffer::new(0, allowed_formats, now, self.serial))
    }

    /// Handle a remote clipboard offer.
    pub fn handle_remote_offer(&mut self, offer: ClipboardOffer) -> Result<()> {
        if !self.policy.bidirectional {
            return Err(ClipboardError::Internal(
                "bidirectional clipboard disabled".to_string(),
            ));
        }
        self.transfer.begin_offer(offer);
        Ok(())
    }

    /// Request data in a specific format from the remote side.
    pub fn request_remote(&mut self, format: ClipboardFormat) -> Result<ClipboardRequest> {
        if !self.is_format_allowed(&format) {
            return Err(ClipboardError::FormatNotAvailable);
        }
        self.serial += 1;
        Ok(ClipboardRequest::new(format, self.serial))
    }

    /// Receive clipboard data from the remote side.
    pub fn receive_remote_data(&mut self, format: ClipboardFormat, data: Vec<u8>) -> Result<()> {
        if data.len() > self.policy.max_payload_bytes {
            return Err(ClipboardError::PayloadTooLarge {
                size: data.len(),
                max: self.policy.max_payload_bytes,
            });
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.remote_store.set(format, data, 0, now)?;
        Ok(())
    }

    /// Get local clipboard data for a format.
    #[must_use]
    pub fn get_local(&self, format: &ClipboardFormat) -> Option<&[u8]> {
        self.local_store.get(format)
    }

    /// Get the clipboard policy.
    #[must_use]
    pub fn policy(&self) -> &ClipboardPolicy {
        &self.policy
    }

    /// Check if a format is allowed by the policy.
    #[must_use]
    pub fn is_format_allowed(&self, format: &ClipboardFormat) -> bool {
        match &self.policy.allowed_formats {
            Some(allowed) => allowed.contains(format),
            None => true,
        }
    }

    /// Get remote clipboard data for a format.
    #[must_use]
    pub fn get_remote(&self, format: &ClipboardFormat) -> Option<&[u8]> {
        self.remote_store.get(format)
    }

    /// Clear the local clipboard store.
    pub fn clear_local(&mut self) {
        self.local_store.clear();
    }

    /// Clear the remote clipboard store.
    pub fn clear_remote(&mut self) {
        self.remote_store.clear();
    }

    /// Get the current serial number.
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Get the transfer state.
    #[must_use]
    pub fn transfer_state(&self) -> &crate::transfer::TransferState {
        self.transfer.state()
    }
}
