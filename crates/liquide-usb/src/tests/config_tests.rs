use crate::config::{PinEntry, SmartCardConfig, TierMode, TransportChannel, UsbConfig};
use crate::device::DeviceClass;

#[test]
fn test_usb_config_defaults() {
    let config = UsbConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.tier, TierMode::Auto);
    assert_eq!(config.transport_channel, TransportChannel::Dedicated);
    assert_eq!(config.max_devices_per_session, 5);
    assert_eq!(config.max_bandwidth_mbps, 50);
    assert!(config.audit_log);
    assert!(!config.mass_storage_read_only);
    assert!(config.allowed_device_classes.is_empty());
    assert!(config.blocked_vid_pid.is_empty());
}

#[test]
fn test_smartcard_config_defaults() {
    let config = SmartCardConfig::default();
    assert!(config.enabled);
    assert_eq!(config.pin_entry, PinEntry::ClientSide);
    assert_eq!(config.apdu_timeout_ms, 5000);
    assert_eq!(config.max_readers, 4);
}

#[test]
fn test_usb_config_custom() {
    let mut config = UsbConfig::default();
    config.enabled = true;
    config.tier = TierMode::Tier2;
    config.blocked_device_classes.push(DeviceClass::RawUsb);
    config.max_devices_per_session = 10;

    assert!(config.enabled);
    assert_eq!(config.tier, TierMode::Tier2);
    assert_eq!(config.blocked_device_classes.len(), 1);
    assert_eq!(config.max_devices_per_session, 10);
}

#[test]
fn test_tier_mode_variants() {
    let modes = [
        TierMode::Auto,
        TierMode::Tier1,
        TierMode::Tier2,
        TierMode::Tier3,
    ];
    assert_eq!(modes.len(), 4);
}

#[test]
fn test_transport_channel_variants() {
    assert_ne!(TransportChannel::Dedicated, TransportChannel::Shared);
}
