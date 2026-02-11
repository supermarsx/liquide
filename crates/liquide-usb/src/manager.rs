//! USB device manager — orchestrates policy, bandwidth, and audit.

use std::collections::HashMap;

use crate::audit::UsbAuditEvent;
use crate::bandwidth::BandwidthLimiter;
use crate::config::{SmartCardConfig, UsbConfig};
use crate::device::{DeviceInfo, DeviceState, UsbDevice};
use crate::policy::{PolicyResult, UsbPolicy};
use crate::{UsbError, Result};

/// Central manager for USB device redirection within a session.
pub struct UsbManager {
    config: UsbConfig,
    policy: UsbPolicy,
    devices: HashMap<u32, UsbDevice>,
    smartcard_config: SmartCardConfig,
    bandwidth_limiter: BandwidthLimiter,
    next_instance_id: u32,
    audit_events: Vec<UsbAuditEvent>,
}

impl UsbManager {
    /// Create a new USB manager.
    #[must_use]
    pub fn new(config: UsbConfig, smartcard_config: SmartCardConfig) -> Self {
        let policy = UsbPolicy::from_config(&config);
        let bandwidth_limiter = BandwidthLimiter::new(config.max_bandwidth_mbps);
        Self {
            config,
            policy,
            devices: HashMap::new(),
            smartcard_config,
            bandwidth_limiter,
            next_instance_id: 1,
            audit_events: Vec::new(),
        }
    }

    /// Attach a USB device to the session.
    ///
    /// Performs policy checks, security key checks, and device limit enforcement.
    /// Returns the assigned device instance ID on success.
    pub fn attach_device(&mut self, info: DeviceInfo) -> Result<u32> {
        if !self.config.enabled {
            return Err(UsbError::Disabled);
        }

        // Check security key
        if self.policy.is_security_key(&info.vid_pid) {
            self.audit_events.push(UsbAuditEvent::SecurityKeyForwardAttempt {
                user: String::new(),
                device_name: info.name.clone(),
                vid_pid: info.vid_pid.to_string(),
                allowed: false,
            });
            return Err(UsbError::SecurityKeyBlocked {
                vid_pid: info.vid_pid.to_string(),
            });
        }

        // Policy check
        match self.policy.is_device_allowed(&info) {
            PolicyResult::Denied { reason } => {
                self.audit_events.push(UsbAuditEvent::DeviceBlocked {
                    user: String::new(),
                    device_name: info.name.clone(),
                    vid_pid: info.vid_pid.to_string(),
                    class: info.device_class.to_string(),
                    block_reason: reason.clone(),
                });
                return Err(UsbError::PolicyDenied {
                    device: info.vid_pid.to_string(),
                    reason,
                });
            }
            PolicyResult::Allowed => {}
        }

        // Device limit check
        let active_count = self.devices.values()
            .filter(|d| d.state() != DeviceState::Disconnected && d.state() != DeviceState::Blocked)
            .count() as u32;
        if active_count >= self.policy.max_devices() {
            return Err(UsbError::DeviceLimitExceeded {
                max: self.policy.max_devices(),
            });
        }

        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;

        // Audit
        self.audit_events.push(UsbAuditEvent::DeviceForwarded {
            user: String::new(),
            device_name: info.name.clone(),
            vid_pid: info.vid_pid.to_string(),
            class: info.device_class.to_string(),
            session_id: String::new(),
        });

        let mut device = UsbDevice::new(info);
        device.set_state(DeviceState::Forwarding);
        self.devices.insert(instance_id, device);

        Ok(instance_id)
    }

    /// Detach a USB device from the session.
    pub fn detach_device(&mut self, instance_id: u32) -> Result<()> {
        let device = self.devices.get_mut(&instance_id).ok_or_else(|| {
            UsbError::InvalidDevice(format!("no device with instance ID {}", instance_id))
        })?;

        self.audit_events.push(UsbAuditEvent::DeviceDisconnected {
            user: String::new(),
            device_name: device.info().name.clone(),
            vid_pid: device.info().vid_pid.to_string(),
            reason: "detached".to_string(),
        });

        device.detach();
        Ok(())
    }

    /// Handle incoming data for a device.
    ///
    /// Checks bandwidth limits before forwarding.
    pub fn handle_data(&mut self, instance_id: u32, data: &[u8]) -> Result<Vec<u8>> {
        if !self.devices.contains_key(&instance_id) {
            return Err(UsbError::InvalidDevice(
                format!("no device with instance ID {}", instance_id),
            ));
        }

        if !self.bandwidth_limiter.try_consume(data.len() as u64) {
            return Err(UsbError::BandwidthExceeded {
                budget_mbps: self.config.max_bandwidth_mbps,
            });
        }

        // Stub: echo the data back
        Ok(data.to_vec())
    }

    /// List all tracked devices.
    #[must_use]
    pub fn list_devices(&self) -> &HashMap<u32, UsbDevice> {
        &self.devices
    }

    /// Drain pending audit events.
    pub fn drain_audit_events(&mut self) -> Vec<UsbAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// Check if USB redirection is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}
