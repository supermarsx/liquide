use crate::device::*;
use crate::profile::*;

#[test]
fn cpu_only_for_no_compute() {
    let caps = GpuCapabilities {
        vendor: GpuVendor::Intel,
        device_type: GpuDeviceType::Integrated,
        device_name: "Test Integrated".to_string(),
        vram_total_mb: 512,
        vulkan_version: "1.2.0".to_string(),
        compute_queues: 0,
        supports_dmabuf: false,
        supports_hw_encoder: false,
    };
    assert_eq!(select_profile(&caps), GpuProfile::CpuOnly);
}

#[test]
fn gpu_composite_for_basic_gpu() {
    let caps = GpuCapabilities {
        vendor: GpuVendor::Intel,
        device_type: GpuDeviceType::Integrated,
        device_name: "Test Integrated".to_string(),
        vram_total_mb: 256,
        vulkan_version: "1.2.0".to_string(),
        compute_queues: 2,
        supports_dmabuf: false,
        supports_hw_encoder: false,
    };
    assert_eq!(select_profile(&caps), GpuProfile::GpuComposite);
}

#[test]
fn gpu_full_for_capable_gpu() {
    let caps = GpuCapabilities {
        vendor: GpuVendor::Nvidia,
        device_type: GpuDeviceType::Discrete,
        device_name: "Test Discrete".to_string(),
        vram_total_mb: 4096,
        vulkan_version: "1.3.0".to_string(),
        compute_queues: 8,
        supports_dmabuf: true,
        supports_hw_encoder: false,
    };
    assert_eq!(select_profile(&caps), GpuProfile::GpuFull);
}

#[test]
fn gpu_dedicated_for_full_featured() {
    let caps = GpuCapabilities {
        vendor: GpuVendor::Nvidia,
        device_type: GpuDeviceType::Discrete,
        device_name: "RTX 4090".to_string(),
        vram_total_mb: 24576,
        vulkan_version: "1.3.275".to_string(),
        compute_queues: 16,
        supports_dmabuf: true,
        supports_hw_encoder: true,
    };
    assert_eq!(select_profile(&caps), GpuProfile::GpuDedicated);
}

#[test]
fn profile_display() {
    assert_eq!(GpuProfile::CpuOnly.to_string(), "cpu-only");
    assert_eq!(GpuProfile::GpuComposite.to_string(), "gpu-composite");
    assert_eq!(GpuProfile::GpuFull.to_string(), "gpu-full");
    assert_eq!(GpuProfile::GpuShared.to_string(), "gpu-shared");
    assert_eq!(GpuProfile::GpuDedicated.to_string(), "gpu-dedicated");
}

#[test]
fn profile_default_is_cpu_only() {
    assert_eq!(GpuProfile::default(), GpuProfile::CpuOnly);
}

#[test]
fn requirements_cpu_only_needs_nothing() {
    let reqs = GpuProfile::CpuOnly.requirements();
    assert_eq!(reqs.min_vram_mb, 0);
    assert!(!reqs.needs_compute);
    assert!(!reqs.needs_hw_encoder);
    assert!(!reqs.needs_dmabuf);
}

#[test]
fn requirements_gpu_dedicated_needs_everything() {
    let reqs = GpuProfile::GpuDedicated.requirements();
    assert!(reqs.min_vram_mb >= 512);
    assert!(reqs.needs_compute);
    assert!(reqs.needs_hw_encoder);
    assert!(reqs.needs_dmabuf);
}

#[test]
fn gpu_shared_for_shared_vram_gpu() {
    // Shared profile: has compute, dmabuf, but not enough VRAM for full
    // and no HW encoder (so can't get dedicated).
    let caps = GpuCapabilities {
        vendor: GpuVendor::Amd,
        device_type: GpuDeviceType::Integrated,
        device_name: "APU".to_string(),
        vram_total_mb: 256,
        vulkan_version: "1.3.0".to_string(),
        compute_queues: 4,
        supports_dmabuf: true,
        supports_hw_encoder: false,
    };
    assert_eq!(select_profile(&caps), GpuProfile::GpuShared);
}
