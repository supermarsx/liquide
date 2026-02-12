//! Tests for `devices` module types.

use liquide_apps_task_manager::devices::*;

// ---------------------------------------------------------------------------
// DeviceCategory
// ---------------------------------------------------------------------------

#[test]
fn device_category_all_variants() {
    let variants = [
        DeviceCategory::Processor,
        DeviceCategory::Display,
        DeviceCategory::Disk,
        DeviceCategory::Network,
        DeviceCategory::Audio,
        DeviceCategory::Usb,
        DeviceCategory::Bluetooth,
        DeviceCategory::InputDevice,
        DeviceCategory::PrinterScanner,
        DeviceCategory::Camera,
        DeviceCategory::Sensor,
        DeviceCategory::Other,
    ];
    assert_eq!(variants.len(), 12);
}

#[test]
fn device_category_serde_roundtrip() {
    let val = DeviceCategory::Disk;
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

// ---------------------------------------------------------------------------
// BusType
// ---------------------------------------------------------------------------

#[test]
fn bus_type_all_variants() {
    let variants = [
        BusType::Pci,
        BusType::PciExpress,
        BusType::Usb,
        BusType::Thunderbolt,
        BusType::Sata,
        BusType::Nvme,
        BusType::Bluetooth,
        BusType::Virtual,
    ];
    assert_eq!(variants.len(), 8);
}

// ---------------------------------------------------------------------------
// DeviceViewMode
// ---------------------------------------------------------------------------

#[test]
fn device_view_mode_all_variants() {
    let variants = [
        DeviceViewMode::ByType,
        DeviceViewMode::ByConnection,
        DeviceViewMode::ByStatus,
        DeviceViewMode::ByDriver,
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

// ---------------------------------------------------------------------------
// BluetoothType & BtProtocol
// ---------------------------------------------------------------------------

#[test]
fn bluetooth_type_all_variants() {
    let variants = [
        BluetoothType::Classic,
        BluetoothType::LowEnergy,
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
        BtProtocol::Spp,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// DeviceInfo construction
// ---------------------------------------------------------------------------

#[test]
fn device_info_construction() {
    let dev = DeviceInfo {
        name: "Intel Network Adapter".into(),
        device_id: "PCI\\VEN_8086&DEV_1234".into(),
        category: DeviceCategory::Network,
        status: DeviceStatus::Ok,
        manufacturer: Some("Intel".into()),
        driver_name: Some("e1000e".into()),
        driver_version: Some("12.19.0".into()),
        driver_date: Some("2025-11-01".into()),
        bus_type: Some(BusType::PciExpress),
        location: Some("PCI bus 0, device 25".into()),
        physical_device_object: None,
        hardware_ids: vec!["PCI\\VEN_8086&DEV_1234".into()],
        compatible_ids: vec![],
        power_state: Some("D0".into()),
        irq: None,
        memory_range: None,
        io_range: None,
        dma_channel: None,
        firmware_version: None,
        serial_number: None,
        description: Some("Intel Ethernet Controller".into()),
    };
    assert_eq!(dev.name, "Intel Network Adapter");
    assert_eq!(dev.category, DeviceCategory::Network);
    assert_eq!(dev.status, DeviceStatus::Ok);
}

#[test]
fn device_info_serde_roundtrip() {
    let dev = DeviceInfo {
        name: "Test USB".into(),
        device_id: "USB\\VID_1234".into(),
        category: DeviceCategory::Usb,
        status: DeviceStatus::Ok,
        manufacturer: None,
        driver_name: None,
        driver_version: None,
        driver_date: None,
        bus_type: Some(BusType::Usb),
        location: None,
        physical_device_object: None,
        hardware_ids: vec![],
        compatible_ids: vec![],
        power_state: None,
        irq: None,
        memory_range: None,
        io_range: None,
        dma_channel: None,
        firmware_version: None,
        serial_number: None,
        description: None,
    };
    let json = serde_json::to_string(&dev).unwrap();
    let back: DeviceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.device_id, "USB\\VID_1234");
}

// ---------------------------------------------------------------------------
// UsbDeviceInfo
// ---------------------------------------------------------------------------

#[test]
fn usb_device_info_construction() {
    let usb = UsbDeviceInfo {
        name: "USB Mass Storage".into(),
        vid: 0x1234,
        pid: 0x5678,
        speed: UsbSpeed::High,
        max_power_ma: 500,
        class_name: "Mass Storage".into(),
        port: Some("Port 3".into()),
        serial: Some("SN12345".into()),
    };
    assert_eq!(usb.vid, 0x1234);
    assert_eq!(usb.speed, UsbSpeed::High);
}

// ---------------------------------------------------------------------------
// BluetoothDevice
// ---------------------------------------------------------------------------

#[test]
fn bluetooth_device_construction() {
    let bt = BluetoothDevice {
        name: "AirPods".into(),
        address: "AA:BB:CC:DD:EE:FF".into(),
        bt_type: BluetoothType::LowEnergy,
        connected: true,
        paired: true,
        battery_percent: Some(85),
        protocols: vec![BtProtocol::A2dp, BtProtocol::Hfp],
        signal_strength_dbm: Some(-52),
    };
    assert_eq!(bt.name, "AirPods");
    assert!(bt.paired);
    assert_eq!(bt.protocols.len(), 2);
}
