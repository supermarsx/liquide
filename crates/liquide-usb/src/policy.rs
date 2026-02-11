//! USB device policy enforcement.

use crate::config::UsbConfig;
use crate::device::{DeviceClass, DeviceInfo, SecurityKeyDb, VidPid};

/// Result of a policy check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyResult {
    /// Device is allowed.
    Allowed,
    /// Device is denied with a reason.
    Denied { reason: String },
}

/// Policy engine for USB device access control.
#[derive(Debug, Clone)]
pub struct UsbPolicy {
    blocked_vid_pid: Vec<String>,
    blocked_device_classes: Vec<DeviceClass>,
    allowed_device_classes: Vec<DeviceClass>,
    allowed_vid_pid: Vec<String>,
    security_key_db: SecurityKeyDb,
    max_devices: u32,
}

impl UsbPolicy {
    /// Create a policy from USB configuration.
    #[must_use]
    pub fn from_config(config: &UsbConfig) -> Self {
        let security_key_db = SecurityKeyDb::with_overrides(
            &config.security_key_overrides.additional,
            &config.security_key_overrides.exceptions,
        );
        Self {
            blocked_vid_pid: config.blocked_vid_pid.clone(),
            blocked_device_classes: config.blocked_device_classes.clone(),
            allowed_device_classes: config.allowed_device_classes.clone(),
            allowed_vid_pid: config.allowed_vid_pid.clone(),
            security_key_db,
            max_devices: config.max_devices_per_session,
        }
    }

    /// Check whether a device is allowed by all policy rules.
    #[must_use]
    pub fn is_device_allowed(&self, info: &DeviceInfo) -> PolicyResult {
        // Check if device class is blocked
        if let PolicyResult::Denied { reason } = self.is_class_allowed(&info.device_class) {
            return PolicyResult::Denied { reason };
        }

        // Check if VID:PID is blocked
        if let PolicyResult::Denied { reason } = self.is_vid_pid_allowed(&info.vid_pid) {
            return PolicyResult::Denied { reason };
        }

        // Check if it's a security key (blocked by default)
        if self.is_security_key(&info.vid_pid) {
            return PolicyResult::Denied {
                reason: format!("security key {} is blocked by default", info.vid_pid),
            };
        }

        PolicyResult::Allowed
    }

    /// Check whether a device class is permitted.
    #[must_use]
    pub fn is_class_allowed(&self, class: &DeviceClass) -> PolicyResult {
        if self.blocked_device_classes.contains(class) {
            return PolicyResult::Denied {
                reason: format!("device class {} is blocked", class),
            };
        }
        if !self.allowed_device_classes.is_empty() && !self.allowed_device_classes.contains(class) {
            return PolicyResult::Denied {
                reason: format!("device class {} is not in the allowed list", class),
            };
        }
        PolicyResult::Allowed
    }

    /// Check whether a VID:PID is permitted.
    #[must_use]
    pub fn is_vid_pid_allowed(&self, vid_pid: &VidPid) -> PolicyResult {
        for pattern in &self.blocked_vid_pid {
            if vid_pid.matches_pattern(pattern) {
                return PolicyResult::Denied {
                    reason: format!("VID:PID {} matches blocked pattern {}", vid_pid, pattern),
                };
            }
        }
        if !self.allowed_vid_pid.is_empty() {
            let allowed = self.allowed_vid_pid.iter().any(|p| vid_pid.matches_pattern(p));
            if !allowed {
                return PolicyResult::Denied {
                    reason: format!("VID:PID {} is not in the allowed list", vid_pid),
                };
            }
        }
        PolicyResult::Allowed
    }

    /// Check whether a VID:PID is a known security key.
    #[must_use]
    pub fn is_security_key(&self, vid_pid: &VidPid) -> bool {
        self.security_key_db.is_security_key(vid_pid)
    }

    /// Get the maximum number of devices allowed per session.
    #[must_use]
    pub fn max_devices(&self) -> u32 {
        self.max_devices
    }
}
