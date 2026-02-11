use crate::device::{DeviceClass, DeviceInfo, DeviceState, SecurityKeyDb, UsbDevice, VidPid};

fn sample_vid_pid() -> VidPid {
    VidPid { vendor: 0x046D, product: 0xC534 }
}

fn sample_info() -> DeviceInfo {
    DeviceInfo {
        vid_pid: sample_vid_pid(),
        device_class: DeviceClass::Filesystem,
        name: "Test Drive".to_string(),
        serial: Some("ABC123".to_string()),
        interfaces: 1,
    }
}

#[test]
fn test_device_class_display() {
    assert_eq!(DeviceClass::Filesystem.to_string(), "Filesystem");
    assert_eq!(DeviceClass::SmartCard.to_string(), "SmartCard");
    assert_eq!(DeviceClass::RawUsb.to_string(), "RawUsb");
}

#[test]
fn test_vid_pid_display() {
    let vp = VidPid { vendor: 0x1050, product: 0x0407 };
    assert_eq!(vp.to_string(), "1050:0407");
}

#[test]
fn test_vid_pid_matches_exact() {
    let vp = VidPid { vendor: 0x1050, product: 0x0407 };
    assert!(vp.matches_pattern("1050:0407"));
    assert!(!vp.matches_pattern("1050:0408"));
}

#[test]
fn test_vid_pid_matches_wildcard() {
    let vp = VidPid { vendor: 0x1050, product: 0x0407 };
    assert!(vp.matches_pattern("1050:*"));
    assert!(vp.matches_pattern("*:0407"));
    assert!(!vp.matches_pattern("1051:*"));
}

#[test]
fn test_usb_device_lifecycle() {
    let info = sample_info();
    let mut device = UsbDevice::new(info);
    assert_eq!(device.state(), DeviceState::Available);
    assert!(device.session_id().is_none());

    device.attach("session-1".to_string(), 1000);
    assert_eq!(device.state(), DeviceState::Forwarding);
    assert_eq!(device.session_id(), Some("session-1"));
    assert_eq!(device.attached_at(), Some(1000));

    device.detach();
    assert_eq!(device.state(), DeviceState::Disconnected);
    assert!(device.session_id().is_none());
}

#[test]
fn test_security_key_db_yubico() {
    let db = SecurityKeyDb::new();
    let yubikey = VidPid { vendor: 0x1050, product: 0x0407 };
    assert!(db.is_security_key(&yubikey));
}

#[test]
fn test_security_key_db_non_key() {
    let db = SecurityKeyDb::new();
    let mouse = VidPid { vendor: 0x046D, product: 0xC534 };
    assert!(!db.is_security_key(&mouse));
}

#[test]
fn test_security_key_db_overrides() {
    let db = SecurityKeyDb::with_overrides(
        &["AAAA:BBBB".to_string()],
        &["1050:*".to_string()],
    );
    // Custom addition should be recognized
    let custom = VidPid { vendor: 0xAAAA, product: 0xBBBB };
    assert!(db.is_security_key(&custom));

    // Yubico exception should no longer match
    let yubikey = VidPid { vendor: 0x1050, product: 0x0407 };
    assert!(!db.is_security_key(&yubikey));
}
