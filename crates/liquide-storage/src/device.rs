//! Physical and virtual storage device representation.

use crate::partition::Partition;

/// The type of a storage device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    HDD,
    SSD,
    NVMe,
    USB,
    SDCard,
    Optical,
    NetworkDrive,
    Virtual,
}

impl StorageType {
    /// Parse a string into a `StorageType`.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "hdd" | "disk" | "hard disk" => Self::HDD,
            "ssd" | "solid state" => Self::SSD,
            "nvme" => Self::NVMe,
            "usb" | "usb drive" => Self::USB,
            "sd" | "sdcard" | "sd card" => Self::SDCard,
            "optical" | "cd" | "dvd" | "blu-ray" | "cdrom" => Self::Optical,
            "network" | "net" | "nas" | "iscsi" => Self::NetworkDrive,
            "virtual" | "loop" | "ram" | "vhd" | "vhdx" => Self::Virtual,
            _ => Self::HDD,
        }
    }

    /// Return the canonical name.
    pub fn name(&self) -> &str {
        match self {
            Self::HDD => "HDD",
            Self::SSD => "SSD",
            Self::NVMe => "NVMe",
            Self::USB => "USB",
            Self::SDCard => "SD Card",
            Self::Optical => "Optical",
            Self::NetworkDrive => "Network Drive",
            Self::Virtual => "Virtual",
        }
    }

    /// Returns `true` if this device type is typically removable.
    pub fn is_typically_removable(&self) -> bool {
        matches!(self, Self::USB | Self::SDCard | Self::Optical)
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A physical or virtual storage device.
#[derive(Debug, Clone)]
pub struct StorageDevice {
    /// Device path or identifier (e.g., "/dev/sda", "\\\\.\\PhysicalDrive0").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Manufacturer model string.
    pub model: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Total capacity in bytes.
    pub size_bytes: u64,
    /// Type of storage device.
    pub device_type: StorageType,
    /// Whether the device is removable.
    pub removable: bool,
    /// Partitions on this device.
    pub partitions: Vec<Partition>,
}

impl StorageDevice {
    /// Human-readable string for the device capacity.
    pub fn formatted_size(&self) -> String {
        crate::analyzer::format_size(self.size_bytes)
    }

    /// Total used space across all partitions.
    pub fn total_used_bytes(&self) -> u64 {
        self.partitions.iter().map(|p| p.used_bytes).sum()
    }

    /// Total available space across all partitions.
    pub fn total_available_bytes(&self) -> u64 {
        self.partitions.iter().map(|p| p.available_bytes).sum()
    }

    /// Returns `true` if any partition on this device contains the OS.
    pub fn has_system_partition(&self) -> bool {
        self.partitions.iter().any(|p| p.is_system)
    }

    /// Returns `true` if the device can be safely ejected (removable, no system partition).
    pub fn is_ejectable(&self) -> bool {
        self.removable && !self.has_system_partition()
    }
}
