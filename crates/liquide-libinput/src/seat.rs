//! Multi-seat device grouping.
//!
//! A **seat** groups input devices that belong to the same logical user
//! session (keyboard + pointer + touch).  The default seat is `"seat0"`.

use crate::classify::{DeviceClass, DeviceInfo};

/// Opaque seat identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeatId(pub u32);

/// A logical input seat containing zero or more devices.
#[derive(Debug, Clone)]
pub struct InputSeat {
    pub id: SeatId,
    pub name: String,
    devices: Vec<DeviceInfo>,
    pub has_keyboard: bool,
    pub has_pointer: bool,
    pub has_touch: bool,
}

impl InputSeat {
    /// Create a new empty seat.
    pub fn new(id: SeatId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            devices: Vec::new(),
            has_keyboard: false,
            has_pointer: false,
            has_touch: false,
        }
    }

    /// Create the default `seat0`.
    pub fn default_seat() -> Self {
        Self::new(SeatId(0), "seat0")
    }

    /// Add a device and update capability summary flags.
    pub fn add_device(&mut self, info: DeviceInfo) {
        match info.device_class {
            DeviceClass::Keyboard => self.has_keyboard = true,
            DeviceClass::Mouse | DeviceClass::Touchpad => self.has_pointer = true,
            DeviceClass::Touchscreen => self.has_touch = true,
            _ => {}
        }
        self.devices.push(info);
    }

    /// Remove a device by path and recalculate summary flags.
    pub fn remove_device(&mut self, path: &str) {
        self.devices.retain(|d| d.path != path);
        self.recalculate_flags();
    }

    /// Immutable view of all devices on this seat.
    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    /// Human-readable summary of seat capabilities.
    pub fn capabilities_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.has_keyboard {
            parts.push("keyboard");
        }
        if self.has_pointer {
            parts.push("pointer");
        }
        if self.has_touch {
            parts.push("touch");
        }
        if parts.is_empty() {
            return "none".to_string();
        }
        parts.join(", ")
    }

    fn recalculate_flags(&mut self) {
        self.has_keyboard = false;
        self.has_pointer = false;
        self.has_touch = false;
        for d in &self.devices {
            match d.device_class {
                DeviceClass::Keyboard => self.has_keyboard = true,
                DeviceClass::Mouse | DeviceClass::Touchpad => self.has_pointer = true,
                DeviceClass::Touchscreen => self.has_touch = true,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::DeviceCapability;

    fn make_device(path: &str, class: DeviceClass) -> DeviceInfo {
        DeviceInfo {
            path: path.to_string(),
            name: format!("Test {class}"),
            device_class: class,
            capabilities: DeviceCapability::EMPTY,
            vendor_id: 0,
            product_id: 0,
            bus_type: 0,
        }
    }

    #[test]
    fn seat_add_remove() {
        let mut seat = InputSeat::default_seat();
        assert_eq!(seat.capabilities_summary(), "none");

        seat.add_device(make_device("/dev/input/event0", DeviceClass::Keyboard));
        seat.add_device(make_device("/dev/input/event1", DeviceClass::Mouse));
        assert!(seat.has_keyboard);
        assert!(seat.has_pointer);
        assert!(!seat.has_touch);
        assert_eq!(seat.devices().len(), 2);
        assert_eq!(seat.capabilities_summary(), "keyboard, pointer");

        seat.remove_device("/dev/input/event0");
        assert!(!seat.has_keyboard);
        assert!(seat.has_pointer);
        assert_eq!(seat.devices().len(), 1);
    }
}
