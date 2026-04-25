//! GPU profile selection based on device capabilities.
//!
//! Profiles control how much work is offloaded to the GPU:
//! from CPU-only rendering through full GPU compositing with
//! hardware encoding.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::device::GpuCapabilities;

/// GPU rendering profile controlling the division of work
/// between CPU and GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GpuProfile {
    /// All rendering on CPU — no GPU used.
    CpuOnly,
    /// GPU handles compositing only; blur and effects on CPU.
    GpuComposite,
    /// GPU handles all rendering stages including blur and shadows.
    GpuFull,
    /// GPU is shared with other workloads (e.g. VM host GPU).
    GpuShared,
    /// Dedicated GPU exclusively for this session.
    GpuDedicated,
}

impl fmt::Display for GpuProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CpuOnly => write!(f, "cpu-only"),
            Self::GpuComposite => write!(f, "gpu-composite"),
            Self::GpuFull => write!(f, "gpu-full"),
            Self::GpuShared => write!(f, "gpu-shared"),
            Self::GpuDedicated => write!(f, "gpu-dedicated"),
        }
    }
}

impl Default for GpuProfile {
    fn default() -> Self {
        Self::CpuOnly
    }
}

/// Minimum hardware requirements for a given profile.
#[derive(Debug, Clone)]
pub struct ProfileRequirements {
    /// Minimum total VRAM in megabytes.
    pub min_vram_mb: u64,
    /// Whether compute queue support is required.
    pub needs_compute: bool,
    /// Whether hardware video encoder support is required.
    pub needs_hw_encoder: bool,
    /// Whether DMA-BUF support is required.
    pub needs_dmabuf: bool,
}

impl GpuProfile {
    /// Return the minimum hardware requirements for this profile.
    #[must_use]
    pub fn requirements(&self) -> ProfileRequirements {
        match self {
            Self::CpuOnly => ProfileRequirements {
                min_vram_mb: 0,
                needs_compute: false,
                needs_hw_encoder: false,
                needs_dmabuf: false,
            },
            Self::GpuComposite => ProfileRequirements {
                min_vram_mb: 128,
                needs_compute: true,
                needs_hw_encoder: false,
                needs_dmabuf: false,
            },
            Self::GpuFull => ProfileRequirements {
                min_vram_mb: 256,
                needs_compute: true,
                needs_hw_encoder: false,
                needs_dmabuf: true,
            },
            Self::GpuShared => ProfileRequirements {
                min_vram_mb: 128,
                needs_compute: true,
                needs_hw_encoder: false,
                needs_dmabuf: true,
            },
            Self::GpuDedicated => ProfileRequirements {
                min_vram_mb: 512,
                needs_compute: true,
                needs_hw_encoder: true,
                needs_dmabuf: true,
            },
        }
    }
}

/// Select the best GPU profile for the given device capabilities.
///
/// Profiles are tested from most capable (GpuDedicated) down to CpuOnly.
/// The first profile whose requirements are met is returned.
/// Integrated/virtual GPUs are routed to `GpuShared` instead of `GpuFull`
/// since they share VRAM with the host.
#[must_use]
pub fn select_profile(caps: &GpuCapabilities) -> GpuProfile {
    use crate::device::GpuDeviceType;

    // Test from most capable to least capable.
    let candidates = [
        GpuProfile::GpuDedicated,
        GpuProfile::GpuFull,
        GpuProfile::GpuShared,
        GpuProfile::GpuComposite,
    ];

    for profile in candidates {
        let reqs = profile.requirements();
        if caps.vram_total_mb >= reqs.min_vram_mb
            && (!reqs.needs_compute || caps.has_compute())
            && (!reqs.needs_hw_encoder || caps.supports_hw_encoder)
            && (!reqs.needs_dmabuf || caps.supports_dmabuf)
        {
            // Integrated/virtual GPUs share VRAM with the host — cap them
            // at GpuShared instead of promoting to GpuFull or GpuDedicated.
            let selected = match (profile, caps.device_type) {
                (
                    GpuProfile::GpuFull | GpuProfile::GpuDedicated,
                    GpuDeviceType::Integrated | GpuDeviceType::Virtual,
                ) => GpuProfile::GpuShared,
                _ => profile,
            };
            tracing::info!(profile = %selected, "selected GPU profile");
            return selected;
        }
    }

    tracing::info!("no GPU profile matched, falling back to cpu-only");
    GpuProfile::CpuOnly
}
