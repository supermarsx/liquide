//! Device type classification based on evdev capabilities.

use std::fmt;

/// Broad device classification derived from evdev capability bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Keyboard,
    Mouse,
    Touchpad,
    Touchscreen,
    Tablet,
    Joystick,
    Switch,
    Unknown,
}

impl fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Keyboard => "Keyboard",
            Self::Mouse => "Mouse",
            Self::Touchpad => "Touchpad",
            Self::Touchscreen => "Touchscreen",
            Self::Tablet => "Tablet",
            Self::Joystick => "Joystick",
            Self::Switch => "Switch",
            Self::Unknown => "Unknown",
        };
        f.write_str(s)
    }
}

/// Bitfield of evdev event-type capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapability(pub u32);

impl DeviceCapability {
    pub const KEY: Self = Self(1 << 0);
    pub const REL: Self = Self(1 << 1);
    pub const ABS: Self = Self(1 << 2);
    pub const MSC: Self = Self(1 << 3);
    pub const LED: Self = Self(1 << 4);
    pub const REP: Self = Self(1 << 5);
    pub const FF: Self = Self(1 << 6);
    pub const EMPTY: Self = Self(0);

    /// Returns `true` if all bits in `other` are set in `self`.
    #[inline]
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the bitwise union of `self` and `other`.
    #[inline]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for DeviceCapability {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for DeviceCapability {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Information about a discovered evdev device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Filesystem path (e.g. `/dev/input/event3`).
    pub path: String,
    /// Human-readable device name from the kernel.
    pub name: String,
    /// Classified device type.
    pub device_class: DeviceClass,
    /// Raw capability bitfield.
    pub capabilities: DeviceCapability,
    /// USB / bus vendor ID.
    pub vendor_id: u16,
    /// USB / bus product ID.
    pub product_id: u16,
    /// Bus type (e.g. `0x03` for USB, `0x05` for Bluetooth).
    pub bus_type: u16,
}

/// Classify a device from its capability bits and whether it reports
/// multi-touch ABS axes (`ABS_MT_SLOT`, etc.).
///
/// The heuristic mirrors what libinput and systemd-logind use:
///
/// | Caps combination        | Class       |
/// |------------------------|-------------|
/// | KEY + REL              | Mouse       |
/// | KEY + ABS + MT         | Touchpad *  |
/// | ABS + MT (no KEY btns) | Touchscreen |
/// | KEY + ABS (no MT)      | Tablet      |
/// | KEY + FF               | Joystick    |
/// | KEY only (≥ 80 keys)   | Keyboard    |
///
/// * Touchpad vs. touchscreen when MT present is typically distinguished
///   by the presence of `BTN_TOOL_FINGER`; we accept `has_abs_mt` as a
///   proxy supplied by the caller.
pub fn classify_device(capabilities: DeviceCapability, has_abs_mt: bool) -> DeviceClass {
    let has_key = capabilities.contains(DeviceCapability::KEY);
    let has_rel = capabilities.contains(DeviceCapability::REL);
    let has_abs = capabilities.contains(DeviceCapability::ABS);
    let has_ff = capabilities.contains(DeviceCapability::FF);

    if has_key && has_rel {
        return DeviceClass::Mouse;
    }

    if has_abs && has_abs_mt {
        if has_key {
            // BTN_TOOL_FINGER-style → touchpad; otherwise touchscreen.
            // Caller should set `has_abs_mt` only when BTN_TOOL_FINGER
            // is present for touchpad discrimination.
            return DeviceClass::Touchpad;
        }
        return DeviceClass::Touchscreen;
    }

    if has_key && has_abs && !has_abs_mt {
        return DeviceClass::Tablet;
    }

    if has_key && has_ff {
        return DeviceClass::Joystick;
    }

    if has_key {
        return DeviceClass::Keyboard;
    }

    DeviceClass::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_keyboard() {
        let caps = DeviceCapability::KEY;
        assert_eq!(classify_device(caps, false), DeviceClass::Keyboard);
    }

    #[test]
    fn classify_mouse() {
        let caps = DeviceCapability::KEY | DeviceCapability::REL;
        assert_eq!(classify_device(caps, false), DeviceClass::Mouse);
    }

    #[test]
    fn classify_touchpad() {
        let caps = DeviceCapability::KEY | DeviceCapability::ABS;
        assert_eq!(classify_device(caps, true), DeviceClass::Touchpad);
    }

    #[test]
    fn classify_touchscreen() {
        let caps = DeviceCapability::ABS;
        assert_eq!(classify_device(caps, true), DeviceClass::Touchscreen);
    }

    #[test]
    fn classify_tablet() {
        let caps = DeviceCapability::KEY | DeviceCapability::ABS;
        assert_eq!(classify_device(caps, false), DeviceClass::Tablet);
    }

    #[test]
    fn classify_joystick() {
        let caps = DeviceCapability::KEY | DeviceCapability::FF;
        assert_eq!(classify_device(caps, false), DeviceClass::Joystick);
    }

    #[test]
    fn classify_unknown() {
        assert_eq!(
            classify_device(DeviceCapability::EMPTY, false),
            DeviceClass::Unknown
        );
    }

    #[test]
    fn capability_bitops() {
        let mut caps = DeviceCapability::EMPTY;
        caps |= DeviceCapability::KEY;
        caps |= DeviceCapability::REL;
        assert!(caps.contains(DeviceCapability::KEY));
        assert!(caps.contains(DeviceCapability::REL));
        assert!(!caps.contains(DeviceCapability::ABS));
    }
}
