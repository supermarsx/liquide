#[cfg(test)]
mod tests {
    use crate::classify::{DeviceCapability, DeviceClass, DeviceInfo, classify_device};
    use crate::enumerate::EvdevEnumerator;
    use crate::hotplug::HotplugMonitor;
    use crate::seat::{InputSeat, SeatId};

    #[test]
    fn test_device_capability_operations() {
        let caps = DeviceCapability::KEY.union(DeviceCapability::REL);
        assert!(caps.contains(DeviceCapability::KEY));
        assert!(caps.contains(DeviceCapability::REL));
        assert!(!caps.contains(DeviceCapability::ABS));
    }

    #[test]
    fn test_classify_keyboard() {
        let class = classify_device(DeviceCapability::KEY, false);
        assert_eq!(class, DeviceClass::Keyboard);
    }

    #[test]
    fn test_classify_mouse() {
        let caps = DeviceCapability::KEY.union(DeviceCapability::REL);
        let class = classify_device(caps, false);
        assert_eq!(class, DeviceClass::Mouse);
    }

    #[test]
    fn test_classify_touchpad() {
        let caps = DeviceCapability::KEY.union(DeviceCapability::ABS);
        let class = classify_device(caps, true);
        // Touchpad has ABS + multitouch
        assert!(matches!(class, DeviceClass::Touchpad | DeviceClass::Touchscreen));
    }

    #[test]
    fn test_device_info() {
        let info = DeviceInfo {
            path: "/dev/input/event0".to_string(),
            name: "Test Keyboard".to_string(),
            device_class: DeviceClass::Keyboard,
            capabilities: DeviceCapability::KEY,
            vendor_id: 0x1234,
            product_id: 0x5678,
            bus_type: 3,
        };
        assert_eq!(info.device_class, DeviceClass::Keyboard);
    }

    #[test]
    fn test_evdev_enumerator_non_linux() {
        let enumerator = EvdevEnumerator::new();
        #[cfg(not(target_os = "linux"))]
        {
            let devices = enumerator.scan().unwrap();
            assert!(devices.is_empty());
        }
    }

    #[test]
    fn test_hotplug_monitor() {
        let mut monitor = HotplugMonitor::new();
        assert!(monitor.poll().is_none());
    }

    #[test]
    fn test_input_seat() {
        let mut seat = InputSeat::new(SeatId(0), "seat0".to_string());
        assert!(!seat.has_keyboard);
        assert!(!seat.has_pointer);
        seat.add_device(DeviceInfo {
            path: "/dev/input/event0".to_string(),
            name: "Keyboard".to_string(),
            device_class: DeviceClass::Keyboard,
            capabilities: DeviceCapability::KEY,
            vendor_id: 0, product_id: 0, bus_type: 0,
        });
        assert!(seat.has_keyboard);
    }

    #[test]
    fn test_device_class_display() {
        assert_eq!(format!("{}", DeviceClass::Keyboard), "Keyboard");
        assert_eq!(format!("{}", DeviceClass::Mouse), "Mouse");
    }
}
