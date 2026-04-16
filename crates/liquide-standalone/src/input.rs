//! Input device integration for standalone compositor.
//!
//! Wraps liquide-libinput to provide input events to the compositor.

use liquide_libinput::{DeviceInfo, DeviceClass};

/// Summary of discovered input devices.
#[derive(Debug, Default)]
pub struct InputDeviceSummary {
    /// All discovered devices.
    pub devices: Vec<DeviceInfo>,
    /// Number of keyboards found.
    pub keyboard_count: usize,
    /// Number of pointer devices (mouse/touchpad) found.
    pub pointer_count: usize,
    /// Number of touch screens found.
    pub touch_count: usize,
}

impl InputDeviceSummary {
    /// Create a summary from a list of device info.
    pub fn from_devices(devices: Vec<DeviceInfo>) -> Self {
        let keyboard_count = devices.iter().filter(|d| d.device_class == DeviceClass::Keyboard).count();
        let pointer_count = devices.iter().filter(|d| matches!(d.device_class, DeviceClass::Mouse | DeviceClass::Touchpad)).count();
        let touch_count = devices.iter().filter(|d| d.device_class == DeviceClass::Touchscreen).count();
        Self { devices, keyboard_count, pointer_count, touch_count }
    }

    /// Whether basic input is available (at least keyboard + pointer).
    pub fn has_basic_input(&self) -> bool {
        self.keyboard_count > 0 && self.pointer_count > 0
    }
}
