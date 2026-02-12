//! Tests for `devices` module types.

use liquide_apps_task_manager::devices::*;

// ---------------------------------------------------------------------------
// DeviceCategory
// ---------------------------------------------------------------------------

#[test]
fn device_category_all_variants() {
    let variants = [
        DeviceCategory::Display,
        DeviceCategory::Audio,
        DeviceCategory::Network,
        DeviceCategory::Storage,
        DeviceCategory::Input,
        DeviceCategory::Usb,
        DeviceCategory::Bluetooth,
        DeviceCategory::Printer,
        DeviceCategory::Camera,
        DeviceCategory::Biometric,
        DeviceCategory::System,
        DeviceCategory::Other,
    ];
    assert_eq!(variants.len(), 12);
}

#[test]
fn device_category_display() {
    assert_eq!(DeviceCategory::Display.as_str(), "Display");
    assert_eq!(DeviceCategory::Audio.as_str(), "Audio");
    assert_eq!(DeviceCategory::Network.as_str(), "Network");
    assert_eq!(DeviceCategory::Usb.as_str(), "USB");
    assert_eq!(DeviceCategory::Bluetooth.as_str(), "Bluetooth");
    assert_eq!(DeviceCategory::Other.as_str(), "Other");
}

#[test]
fn device_category_serde_roundtrip() {
    let val = DeviceCategory::Storage;
    let json = serde_json::to_string(&val).unwrap();
    let back: DeviceCategory = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// DeviceStatus
// ---------------------------------------------------------------------------

#[test]
fn device_status_all_variants() {
    let variants = [
        DeviceStatus::Ok,
        DeviceStatus::Disabled,
        DeviceStatus::Error,
        DeviceStatus::Warning,
        DeviceStatus::Unknown,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn device_status_display() {
    assert_eq!(DeviceStatus::Ok.as_str(), "OK");
    assert_eq!(DeviceStatus::Disabled.as_str(), "Disabled");
    assert_eq!(DeviceStatus::Error.as_str(), "Error");
    assert_eq!(DeviceStatus::Warning.as_str(), "Warning");
    assert_eq!(DeviceStatus::Unknown.as_str(), "Unknown");
}

// ---------------------------------------------------------------------------
// BusType
// ---------------------------------------------------------------------------

#[test]
fn bus_type_all_variants() {
    let variants = [
        BusType::Pci,
        BusType::Pcie,
        BusType::Usb,
        BusType::Thunderbolt,
        BusType::Sata,
        BusType::Nvme,
        BusType::I2c,
        BusType::Virtual,
    ];
    assert_eq!(variants.len(), 8);
}

#[test]
fn bus_type_display() {
    assert_eq!(BusType::Pci.as_str(), "PCI");
    assert_eq!(BusType::Pcie.as_str(), "PCIe");
    assert_eq!(BusType::Usb.as_str(), "USB");
    assert_eq!(BusType::Thunderbolt.as_str(), "Thunderbolt");
    assert_eq!(BusType::Nvme.as_str(), "NVMe");
}

// ---------------------------------------------------------------------------
// DeviceViewMode
// ---------------------------------------------------------------------------

#[test]
fn device_view_mode_all_variants() {
    let variants = [
        DeviceViewMode::ByCategory,
        DeviceViewMode::ByBus,
        DeviceViewMode::ByStatus,
        DeviceViewMode::FlatList,
    ];
    assert_eq!(variants.len(), 4);
}

// ---------------------------------------------------------------------------
// UsbSpeed
// ---------------------------------------------------------------------------

#[test]
fn usb_speed_all_variants() {
    let variants = [
        UsbSpeed::Low,
        UsbSpeed::Full,
        UsbSpeed::High,
        UsbSpeed::Super,
        UsbSpeed::SuperPlus,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn usb_speed_display() {
    assert_eq!(UsbSpeed::Low.as_str(), "Low (1.5 Mbps)");
    assert_eq!(UsbSpeed::Full.as_str(), "Full (12 Mbps)");
    assert_eq!(UsbSpeed::High.as_str(), "High (480 Mbps)");
    assert_eq!(UsbSpeed::Super.as_str(), "Super (5 Gbps)");
    assert_eq!(UsbSpeed::SuperPlus.as_str(), "Super+ (10 Gbps)");
}

// ---------------------------------------------------------------------------
// BluetoothType & BtProtocol
// ---------------------------------------------------------------------------

#[test]
fn bluetooth_type_all_variants() {
    let variants = [
        BluetoothType::Classic,
        BluetoothType::Le,
        BluetoothType::Dual,
    ];
    assert_eq!(variants.len(), 3);
}

#[test]
fn bt_protocol_all_variants() {
    let variants = [
        BtProtocol::A2dp,
        BtProtocol::Hfp,
        BtProtocol::Hid,
        BtProtocol::Pan,
        BtProtocol::Gatt,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// DeviceInfo construction
// ---------------------------------------------------------------------------

#[test]
fn device_info_construction() {
    let dev = DeviceInfo {
        instance_id: "PCI\\VEN_8086&DEV_1234".into(),
        name: "Intel Network Adapter".into(),
        category: DeviceCategory::Network,
        status: DeviceStatus::Ok,
        bus_type: Some(BusType::Pcie),
        manufacturer: Some("Intel".into()),
        driver_name: Some("e1000e".into()),
        driver_version: Some("12.19.0".into()),
        driver_date: Some("2025-11-01".into()),
        hardware_ids: vec!["PCI\\VEN_8086&DEV_1234".into()],
        location: Some("PCI bus 0, device 25".into()),
        power_state: Some("D0".into()),
        firmware_version: None,
        serial_number: None,
        class_guid: Some("{4D36E972-E325-11CE-BFC1-08002BE10318}".into()),
        device_class: Some("Net".into()),
        inf_name: Some("e1000e.inf".into()),
        inf_section: None,
        problem_code: None,
        parent_device: None,
        child_count: 0,
    };
    assert_eq!(dev.name, "Intel Network Adapter");
    assert_eq!(dev.category, DeviceCategory::Network);
    assert_eq!(dev.status, DeviceStatus::Ok);
}

#[test]
fn device_info_serde_roundtrip() {
    let dev = DeviceInfo {
        instance_id: "USB\\VID_1234".into(),
        name: "Test USB".into(),
        category: DeviceCategory::Usb,
        status: DeviceStatus::Ok,
        bus_type: Some(BusType::Usb),
        manufacturer: None,
        driver_name: None,
        driver_version: None,
        driver_date: None,
        hardware_ids: vec![],
        location: None,
        power_state: None,
        firmware_version: None,
        serial_number: None,
        class_guid: None,
        device_class: None,
        inf_name: None,
        inf_section: None,
        problem_code: None,
        parent_device: None,
        child_count: 0,
    };
    let json = serde_json::to_string(&dev).unwrap();
    let back: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.instance_id, "USB\\VID_1234");
}

// ---------------------------------------------------------------------------
// UsbDeviceInfo
// ---------------------------------------------------------------------------

#[test]
fn usb_device_info_construction() {
    let usb = UsbDeviceInfo {
        vendor_id: 0x1234,
        product_id: 0x5678,
        speed: UsbSpeed::High,
        power_ma: 500,
        usb_version: "2.0".into(),
        serial_number: Some("SN12345".into()),
        class_code: 8,
        subclass_code: 6,
        protocol_code: 80,
        port_number: 3,
        hub_depth: 0,
    };
    assert_eq!(usb.vendor_id, 0x1234);
    assert_eq!(usb.speed, UsbSpeed::High);
}

// ---------------------------------------------------------------------------
// BluetoothDevice
// ---------------------------------------------------------------------------

#[test]
fn bluetooth_device_construction() {
    let bt = BluetoothDevice {
        address: "AA:BB:CC:DD:EE:FF".into(),
        name: "AirPods".into(),
        bt_type: BluetoothType::Le,
        paired: true,
        connected: true,
        battery_percent: Some(85.0),
        signal_strength_dbm: Some(-52),
        protocols: vec![BtProtocol::A2dp, BtProtocol::Hfp],
        last_seen: Some("2026-02-12T10:00:00Z".into()),
    };
    assert_eq!(bt.name, "AirPods");
    assert!(bt.paired);
    assert_eq!(bt.protocols.len(), 2);
}
