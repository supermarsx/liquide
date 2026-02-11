//! Vulkan device abstraction and GPU capability probing.
//!
//! Provides types for enumerating GPU devices, querying capabilities, and
//! selecting a device for rendering.  This module does not call Vulkan
//! directly — it models the abstraction layer that would wrap `vkEnumeratePhysicalDevices`
//! and related queries.

use serde::{Deserialize, Serialize};

/// GPU hardware vendor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    /// Intel integrated or discrete GPU.
    Intel,
    /// NVIDIA discrete or mobile GPU.
    Nvidia,
    /// AMD/ATI discrete or APU.
    Amd,
    /// ARM Mali or similar.
    Arm,
    /// Unknown or unlisted vendor.
    Other(String),
}

impl std::fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intel => write!(f, "Intel"),
            Self::Nvidia => write!(f, "NVIDIA"),
            Self::Amd => write!(f, "AMD"),
            Self::Arm => write!(f, "ARM"),
            Self::Other(name) => write!(f, "{name}"),
        }
    }
}

/// GPU device type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuDeviceType {
    /// Dedicated / discrete GPU with its own VRAM.
    Discrete,
    /// Integrated GPU sharing system memory.
    Integrated,
    /// Virtual GPU (e.g. in a VM or cloud environment).
    Virtual,
    /// Software rasterizer posing as a GPU device.
    Cpu,
    /// Unrecognised device type.
    Other,
}

impl std::fmt::Display for GpuDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discrete => write!(f, "Discrete"),
            Self::Integrated => write!(f, "Integrated"),
            Self::Virtual => write!(f, "Virtual"),
            Self::Cpu => write!(f, "CPU"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Capability descriptor for a single GPU device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuCapabilities {
    /// Hardware vendor.
    pub vendor: GpuVendor,
    /// Device type (discrete, integrated, etc.).
    pub device_type: GpuDeviceType,
    /// Human-readable device name.
    pub device_name: String,
    /// Total VRAM in megabytes.
    pub vram_total_mb: u64,
    /// Vulkan API version string (e.g. "1.3.275").
    pub vulkan_version: String,
    /// Number of compute queues available.
    pub compute_queues: u32,
    /// Whether DMA-BUF import/export is supported.
    pub supports_dmabuf: bool,
    /// Whether hardware video encoding is supported.
    pub supports_hw_encoder: bool,
}

impl GpuCapabilities {
    /// Create a new capability descriptor.
    #[must_use]
    pub fn new(
        vendor: GpuVendor,
        device_type: GpuDeviceType,
        device_name: String,
    ) -> Self {
        Self {
            vendor,
            device_type,
            device_name,
            vram_total_mb: 0,
            vulkan_version: String::new(),
            compute_queues: 0,
            supports_dmabuf: false,
            supports_hw_encoder: false,
        }
    }

    /// Whether this device has compute queue support.
    #[must_use]
    pub fn has_compute(&self) -> bool {
        self.compute_queues > 0
    }
}

/// A GPU device handle with initialisation state.
#[derive(Debug)]
pub struct GpuDevice {
    /// The capabilities of this device.
    capabilities: GpuCapabilities,
    /// Whether the device has been initialised for rendering.
    is_initialized: bool,
}

impl GpuDevice {
    /// Create a new GPU device wrapper around the given capabilities.
    #[must_use]
    pub fn new(capabilities: GpuCapabilities) -> Self {
        Self {
            capabilities,
            is_initialized: false,
        }
    }

    /// Total VRAM in megabytes.
    #[must_use]
    pub fn vram_total(&self) -> u64 {
        self.capabilities.vram_total_mb
    }

    /// The vendor of this GPU.
    #[must_use]
    pub fn vendor(&self) -> &GpuVendor {
        &self.capabilities.vendor
    }

    /// Whether this device supports compute shaders.
    #[must_use]
    pub fn supports_compute(&self) -> bool {
        self.capabilities.has_compute()
    }

    /// Access the full capability descriptor.
    #[must_use]
    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    /// Whether the device is currently initialised.
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Mark the device as initialised.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.is_initialized = initialized;
    }
}

/// Result of probing the system for available GPU devices.
#[derive(Debug, Clone)]
pub struct GpuProbeResult {
    /// All discovered GPU devices.
    pub devices: Vec<GpuCapabilities>,
    /// Index of the selected device, if any.
    pub selected_device: Option<usize>,
}

/// Probe the system for available GPU devices.
///
/// In production this would call `vkEnumeratePhysicalDevices` and query
/// properties/features.  This implementation returns an empty probe result
/// since no actual Vulkan runtime is linked.
#[must_use]
pub fn probe_devices() -> GpuProbeResult {
    tracing::debug!("probing for GPU devices");

    // No actual Vulkan calls — return empty result.
    // A real implementation would enumerate physical devices here.
    GpuProbeResult {
        devices: Vec::new(),
        selected_device: None,
    }
}
