use crate::device::*;

#[test]
fn probe_returns_empty_without_vulkan() {
    let result = probe_devices();
    assert!(result.devices.is_empty());
    assert!(result.selected_device.is_none());
}

#[test]
fn gpu_capabilities_has_compute() {
    let mut caps = GpuCapabilities::new(
        GpuVendor::Nvidia,
        GpuDeviceType::Discrete,
        "Test GPU".to_string(),
    );
    assert!(!caps.has_compute());

    caps.compute_queues = 4;
    assert!(caps.has_compute());
}

#[test]
fn gpu_device_construction() {
    let caps = GpuCapabilities {
        vendor: GpuVendor::Amd,
        device_type: GpuDeviceType::Discrete,
        device_name: "Radeon RX 7900".to_string(),
        vram_total_mb: 16384,
        vulkan_version: "1.3.275".to_string(),
        compute_queues: 8,
        supports_dmabuf: true,
        supports_hw_encoder: true,
    };

    let device = GpuDevice::new(caps);
    assert_eq!(device.vram_total(), 16384);
    assert_eq!(*device.vendor(), GpuVendor::Amd);
    assert!(device.supports_compute());
    assert!(!device.is_initialized());
}

#[test]
fn gpu_device_initialization_toggle() {
    let caps = GpuCapabilities::new(
        GpuVendor::Intel,
        GpuDeviceType::Integrated,
        "UHD 770".to_string(),
    );
    let mut device = GpuDevice::new(caps);

    assert!(!device.is_initialized());
    device.set_initialized(true);
    assert!(device.is_initialized());
    device.set_initialized(false);
    assert!(!device.is_initialized());
}

#[test]
fn gpu_vendor_display() {
    assert_eq!(GpuVendor::Intel.to_string(), "Intel");
    assert_eq!(GpuVendor::Nvidia.to_string(), "NVIDIA");
    assert_eq!(GpuVendor::Amd.to_string(), "AMD");
    assert_eq!(GpuVendor::Arm.to_string(), "ARM");
    assert_eq!(
        GpuVendor::Other("Qualcomm".to_string()).to_string(),
        "Qualcomm"
    );
}

#[test]
fn gpu_device_type_display() {
    assert_eq!(GpuDeviceType::Discrete.to_string(), "Discrete");
    assert_eq!(GpuDeviceType::Integrated.to_string(), "Integrated");
    assert_eq!(GpuDeviceType::Virtual.to_string(), "Virtual");
    assert_eq!(GpuDeviceType::Cpu.to_string(), "CPU");
    assert_eq!(GpuDeviceType::Other.to_string(), "Other");
}

#[test]
fn capabilities_default_new_has_no_features() {
    let caps = GpuCapabilities::new(
        GpuVendor::Nvidia,
        GpuDeviceType::Discrete,
        "Test".to_string(),
    );
    assert_eq!(caps.vram_total_mb, 0);
    assert_eq!(caps.compute_queues, 0);
    assert!(!caps.supports_dmabuf);
    assert!(!caps.supports_hw_encoder);
    assert!(caps.vulkan_version.is_empty());
}
