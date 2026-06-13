use crate::config::{SmartCardConfig, UsbConfig};
use crate::device::{DeviceClass, DeviceInfo, VidPid};
use crate::manager::UsbManager;

fn enabled_config() -> UsbConfig {
    let mut config = UsbConfig::default();
    config.enabled = true;
    config
}

fn make_info(vendor: u16, product: u16, class: DeviceClass, name: &str) -> DeviceInfo {
    DeviceInfo {
        vid_pid: VidPid { vendor, product },
        device_class: class,
        name: name.to_string(),
        serial: None,
        interfaces: 1,
    }
}

#[test]
fn test_manager_disabled() {
    let manager = UsbManager::new(UsbConfig::default(), SmartCardConfig::default());
    assert!(!manager.is_enabled());
    let info = make_info(0x046D, 0xC534, DeviceClass::Filesystem, "Drive");
    let mut manager = manager;
    let result = manager.attach_device(info);
    assert!(result.is_err());
}

#[test]
fn test_manager_attach_detach() {
    let mut manager = UsbManager::new(enabled_config(), SmartCardConfig::default());
    let info = make_info(0x046D, 0xC534, DeviceClass::Filesystem, "Drive");
    let id = manager.attach_device(info).unwrap();
    assert!(id >= 1);
    assert_eq!(manager.list_devices().len(), 1);

    // Detaching removes the device from the tracking set entirely, so the
    // map reflects only currently-attached devices.
    manager.detach_device(id).unwrap();
    assert!(manager.list_devices().get(&id).is_none());
    assert_eq!(manager.list_devices().len(), 0);
}

#[test]
fn test_manager_detach_removes_from_tracking_set() {
    // Regression for t49-e9-15: the manager must not retain detached devices,
    // otherwise the tracking map grows unbounded across plug/unplug cycles.
    let mut manager = UsbManager::new(enabled_config(), SmartCardConfig::default());

    for cycle in 0..1000u16 {
        let info = make_info(
            0x046D,
            0xC534,
            DeviceClass::Filesystem,
            &format!("Drive {cycle}"),
        );
        let id = manager.attach_device(info).unwrap();
        // Exactly one device is tracked while attached.
        assert_eq!(manager.list_devices().len(), 1);
        manager.detach_device(id).unwrap();
        // None remain tracked once detached, regardless of how many cycles run.
        assert_eq!(manager.list_devices().len(), 0);
    }

    // After 1000 plug/unplug cycles the tracking set is still empty.
    assert!(manager.list_devices().is_empty());
}

#[test]
fn test_manager_security_key_blocked() {
    let mut manager = UsbManager::new(enabled_config(), SmartCardConfig::default());
    let info = make_info(0x1050, 0x0407, DeviceClass::SmartCard, "YubiKey");
    let result = manager.attach_device(info);
    assert!(result.is_err());
}

#[test]
fn test_manager_device_limit() {
    let mut config = enabled_config();
    config.max_devices_per_session = 2;
    let mut manager = UsbManager::new(config, SmartCardConfig::default());

    let info1 = make_info(0x0001, 0x0001, DeviceClass::Filesystem, "Drive 1");
    let info2 = make_info(0x0001, 0x0002, DeviceClass::Filesystem, "Drive 2");
    let info3 = make_info(0x0001, 0x0003, DeviceClass::Filesystem, "Drive 3");

    manager.attach_device(info1).unwrap();
    manager.attach_device(info2).unwrap();
    let result = manager.attach_device(info3);
    assert!(result.is_err());
}

#[test]
fn test_manager_audit_events() {
    let mut manager = UsbManager::new(enabled_config(), SmartCardConfig::default());
    let info = make_info(0x046D, 0xC534, DeviceClass::Filesystem, "Drive");
    manager.attach_device(info).unwrap();

    let events = manager.drain_audit_events();
    assert!(!events.is_empty());
    assert_eq!(events[0].event_name(), "device_forwarded");

    // Second drain should be empty
    let events2 = manager.drain_audit_events();
    assert!(events2.is_empty());
}
