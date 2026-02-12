//! Per-component power breakdown data (spec section 15.5).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// PanelType
// ---------------------------------------------------------------------------

/// Display panel technology type (spec section 15.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelType {
    Lcd,
    Oled,
    MiniLed,
}

impl PanelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lcd => "LCD",
            Self::Oled => "OLED",
            Self::MiniLed => "Mini-LED",
        }
    }
}

impl fmt::Display for PanelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// CpuPower
// ---------------------------------------------------------------------------

/// CPU power draw breakdown (spec section 15.5.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuPower {
    /// Total CPU package power draw in watts.
    pub package_watts: f64,
    /// CPU cores power draw in watts.
    pub cores_watts: f64,
    /// Uncore (memory controller, cache, etc.) power draw in watts.
    pub uncore_watts: f64,
    /// DRAM / memory subsystem power draw in watts.
    pub dram_watts: f64,
}

// ---------------------------------------------------------------------------
// GpuPower
// ---------------------------------------------------------------------------

/// GPU power draw breakdown (spec section 15.5.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPower {
    /// Total GPU board power draw in watts.
    pub total_watts: f64,
    /// GPU core power draw in watts (if reported).
    pub core_watts: Option<f64>,
    /// GPU VRAM power draw in watts (if reported).
    pub memory_watts: Option<f64>,
    /// Fan motor power draw in watts (if reported).
    pub fan_watts: Option<f64>,
}

// ---------------------------------------------------------------------------
// DisplayPower
// ---------------------------------------------------------------------------

/// Display subsystem power data (spec section 15.5.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayPower {
    /// Backlight power draw in watts.
    pub backlight_watts: f64,
    /// Display panel power draw in watts.
    pub panel_watts: f64,
    /// Current brightness level as a percentage (0-100).
    pub brightness_percent: u8,
    /// Panel technology type.
    pub panel_type: PanelType,
    /// Whether HDR content is currently being displayed.
    pub hdr_active: bool,
}

// ---------------------------------------------------------------------------
// StoragePower
// ---------------------------------------------------------------------------

/// Per-drive storage power data (spec section 15.5.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePower {
    /// Drive name / identifier.
    pub name: String,
    /// Total power draw in watts.
    pub power_watts: f64,
    /// Power draw while actively processing I/O in watts.
    pub active_power_watts: f64,
    /// Power draw while idle in watts.
    pub idle_power_watts: f64,
    /// Spindle motor power for HDDs in watts (None for SSDs).
    pub spindle_power_watts: Option<f64>,
}

// ---------------------------------------------------------------------------
// NetworkPower
// ---------------------------------------------------------------------------

/// Network adapter power data (spec section 15.5.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPower {
    /// Network adapter name.
    pub adapter_name: String,
    /// Total power draw in watts.
    pub power_watts: f64,
    /// Wi-Fi transmit power in dBm (if Wi-Fi adapter).
    pub wifi_tx_power_dbm: Option<i32>,
    /// Whether power save mode is enabled.
    pub power_save_enabled: bool,
}

// ---------------------------------------------------------------------------
// PeripheralPower
// ---------------------------------------------------------------------------

/// Peripheral device power data (spec section 15.5.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeripheralPower {
    /// Device name.
    pub name: String,
    /// Power draw in watts.
    pub power_watts: f64,
    /// Device type (keyboard, mouse, audio, biometric, etc.).
    pub device_type: String,
    /// USB port identifier (if connected via USB).
    pub usb_port: Option<String>,
}
