use crate::config::UsbConfig;
use crate::device::{DeviceClass, DeviceInfo, VidPid};
use crate::policy::{PolicyResult, UsbPolicy};

fn make_policy() -> UsbPolicy {
    let config = UsbConfig::default();
    UsbPolicy::from_config(&config)
}

fn make_info(vendor: u16, product: u16, class: DeviceClass) -> DeviceInfo {
    DeviceInfo {
        vid_pid: VidPid { vendor, product },
        device_class: class,
        name: "Test Device".to_string(),
        serial: None,
        interfaces: 1,
    }
}

#[test]
fn test_policy_allows_normal_device() {
    let policy = make_policy();
    let info = make_info(0x046D, 0xC534, DeviceClass::Filesystem);
    assert_eq!(policy.is_device_allowed(&info), PolicyResult::Allowed);
}

#[test]
fn test_policy_blocks_security_key() {
    let policy = make_policy();
    let info = make_info(0x1050, 0x0407, DeviceClass::SmartCard);
    match policy.is_device_allowed(&info) {
        PolicyResult::Denied { reason } => {
            assert!(reason.contains("security key"));
        }
        PolicyResult::Allowed => panic!("expected denial for security key"),
    }
}

#[test]
fn test_policy_blocks_vid_pid() {
    let mut config = UsbConfig::default();
    config.blocked_vid_pid.push("DEAD:BEEF".to_string());
    let policy = UsbPolicy::from_config(&config);
    let info = make_info(0xDEAD, 0xBEEF, DeviceClass::Filesystem);
    match policy.is_device_allowed(&info) {
        PolicyResult::Denied { reason } => {
            assert!(reason.contains("blocked pattern"));
        }
        PolicyResult::Allowed => panic!("expected denial"),
    }
}

#[test]
fn test_policy_blocks_device_class() {
    let mut config = UsbConfig::default();
    config.blocked_device_classes.push(DeviceClass::RawUsb);
    let policy = UsbPolicy::from_config(&config);
    let info = make_info(0x046D, 0xC534, DeviceClass::RawUsb);
    match policy.is_device_allowed(&info) {
        PolicyResult::Denied { reason } => {
            assert!(reason.contains("blocked"));
        }
        PolicyResult::Allowed => panic!("expected denial"),
    }
}

#[test]
fn test_policy_allowed_class_filter() {
    let mut config = UsbConfig::default();
    config.allowed_device_classes.push(DeviceClass::Printer);
    let policy = UsbPolicy::from_config(&config);

    // Printer should be allowed
    let printer = make_info(0x046D, 0xC534, DeviceClass::Printer);
    assert_eq!(policy.is_device_allowed(&printer), PolicyResult::Allowed);

    // Filesystem should be denied
    let fs = make_info(0x046D, 0xC534, DeviceClass::Filesystem);
    match policy.is_device_allowed(&fs) {
        PolicyResult::Denied { reason } => {
            assert!(reason.contains("not in the allowed list"));
        }
        PolicyResult::Allowed => panic!("expected denial"),
    }
}
