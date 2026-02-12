//! Device types for the Devices tab (spec section 10).
//!
//! Hardware device inventory and status monitoring, including device info,
//! USB device tree, Bluetooth devices, and device resource details.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Category of hardware device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCategory {
    Processor,
    Display,
    Disk,
    Network,
    Audio,
    Usb,
    Bluetooth,
    InputDevice,
    PrinterScanner,
    Camera,
    Sensor,
    Other,
}

impl DeviceCategory {
    /// Returns the string representation of this device category.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Processor => "Processor",
            Self::Display => "Display",
            Self::Disk => "Disk",
            Self::Network => "Network",
            Self::Audio => "Audio",
            Self::Usb => "USB",
            Self::Bluetooth => "Bluetooth",
            Self::InputDevice => "Input Device",
            Self::PrinterScanner => "Printer/Scanner",
            Self::Camera => "Camera",
            Self::Sensor => "Sensor",
            Self::Other => "Other",
        }
    }
}

impl fmt::Display for DeviceCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Current operational status of a hardware device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Ok,
    Warning,
    Error,
    Disabled,
    Unknown,
}

impl DeviceStatus {
    /// Returns the string representation of this device status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Disabled => "Disabled",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bus type through which a device is connected to the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusType {
    Pci,
    PciExpress,
    Usb,
    Thunderbolt,
    Sata,
    Nvme,
    Bluetooth,
    Virtual,
}

impl BusType {
    /// Returns the string representation of this bus type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pci => "PCI",
            Self::PciExpress => "PCI Express",
            Self::Usb => "USB",
            Self::Thunderbolt => "Thunderbolt",
            Self::Sata => "SATA",
            Self::Nvme => "NVMe",
            Self::Bluetooth => "Bluetooth",
            Self::Virtual => "Virtual",
        }
    }
}

impl fmt::Display for BusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How devices are organized in the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceViewMode {
    ByType,
    ByConnection,
    ByStatus,
    ByDriver,
}

impl DeviceViewMode {
    /// Returns the string representation of this view mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ByType => "By Type",
            Self::ByConnection => "By Connection",
            Self::ByStatus => "By Status",
            Self::ByDriver => "By Driver",
        }
    }
}

impl fmt::Display for DeviceViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// USB protocol speed classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
    SuperPlus,
}

impl UsbSpeed {
    /// Returns the string representation of this USB speed.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "Low Speed (1.5 Mbps)",
            Self::Full => "Full Speed (12 Mbps)",
            Self::High => "High Speed (480 Mbps)",
            Self::Super => "SuperSpeed (5 Gbps)",
            Self::SuperPlus => "SuperSpeed+ (10+ Gbps)",
        }
    }
}

impl fmt::Display for UsbSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bluetooth radio type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BluetoothType {
    Classic,
    LowEnergy,
    Dual,
}

impl BluetoothType {
    /// Returns the string representation of this Bluetooth type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Classic => "Classic (BR/EDR)",
            Self::LowEnergy => "Low Energy (BLE)",
            Self::Dual => "Dual Mode",
        }
    }
}

impl fmt::Display for BluetoothType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bluetooth profile/protocol identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BtProtocol {
    A2dp,
    Hfp,
    Hid,
    Pan,
    Spp,
}

impl BtProtocol {
    /// Returns the string representation of this Bluetooth protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A2dp => "A2DP",
            Self::Hfp => "HFP",
            Self::Hid => "HID",
            Self::Pan => "PAN",
            Self::Spp => "SPP",
        }
    }
}

impl fmt::Display for BtProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Comprehensive information about a hardware device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Friendly device name.
    pub name: String,
    /// Unique device instance identifier.
    pub device_id: String,
    /// Device class category.
    pub category: DeviceCategory,
    /// Current device status.
    pub status: DeviceStatus,
    /// Hardware manufacturer name.
    pub manufacturer: Option<String>,
    /// Driver name.
    pub driver_name: Option<String>,
    /// Driver version string.
    pub driver_version: Option<String>,
    /// Driver release date.
    pub driver_date: Option<String>,
    /// Bus type through which the device is connected.
    pub bus_type: Option<BusType>,
    /// Physical location path (bus/slot/port).
    pub location: Option<String>,
    /// Physical device object name.
    pub physical_device_object: Option<String>,
    /// PnP hardware identifiers.
    pub hardware_ids: Vec<String>,
    /// Alternate compatible identifiers.
    pub compatible_ids: Vec<String>,
    /// Current power state (D0/D1/D2/D3).
    pub power_state: Option<String>,
    /// Interrupt request line, if applicable.
    pub irq: Option<u32>,
    /// Memory-mapped I/O range.
    pub memory_range: Option<String>,
    /// I/O port range.
    pub io_range: Option<String>,
    /// DMA channel, if applicable.
    pub dma_channel: Option<u32>,
    /// Device firmware version.
    pub firmware_version: Option<String>,
    /// Device serial number.
    pub serial_number: Option<String>,
    /// Device description text.
    pub description: Option<String>,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            device_id: String::new(),
            category: DeviceCategory::Other,
            status: DeviceStatus::Unknown,
            manufacturer: None,
            driver_name: None,
            driver_version: None,
            driver_date: None,
            bus_type: None,
            location: None,
            physical_device_object: None,
            hardware_ids: Vec::new(),
            compatible_ids: Vec::new(),
            power_state: None,
            irq: None,
            memory_range: None,
            io_range: None,
            dma_channel: None,
            firmware_version: None,
            serial_number: None,
            description: None,
        }
    }
}

/// Information about a USB device in the device tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDeviceInfo {
    /// Device name.
    pub name: String,
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// Negotiated USB speed.
    pub speed: UsbSpeed,
    /// Maximum power draw in milliamps.
    pub max_power_ma: u16,
    /// USB device class name.
    pub class_name: String,
    /// USB port path.
    pub port: Option<String>,
    /// Device serial number.
    pub serial: Option<String>,
}

/// Information about a Bluetooth device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    /// Device name.
    pub name: String,
    /// Bluetooth hardware address.
    pub address: String,
    /// Bluetooth radio type.
    pub bt_type: BluetoothType,
    /// Whether the device is currently connected.
    pub connected: bool,
    /// Whether the device is paired.
    pub paired: bool,
    /// Remote device battery level percentage, if reported.
    pub battery_percent: Option<u8>,
    /// Supported Bluetooth protocols/profiles.
    pub protocols: Vec<BtProtocol>,
    /// Signal strength in dBm.
    pub signal_strength_dbm: Option<i32>,
}
