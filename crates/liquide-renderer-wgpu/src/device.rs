//! GPU device discovery and initialization.

use crate::{Result, WgpuError};
use serde::{Deserialize, Serialize};

/// Which GPU backend is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    Vulkan,
    D3D12,
    Metal,
    OpenGl,
    WebGpu,
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuBackend::Vulkan => write!(f, "Vulkan"),
            GpuBackend::D3D12 => write!(f, "D3D12"),
            GpuBackend::Metal => write!(f, "Metal"),
            GpuBackend::OpenGl => write!(f, "OpenGL"),
            GpuBackend::WebGpu => write!(f, "WebGPU"),
        }
    }
}

/// Wraps a wgpu device + queue + adapter info.
pub struct WgpuDevice {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub backend: GpuBackend,
    pub device_name: String,
    pub vendor_id: u32,
}

impl WgpuDevice {
    /// Create a new wgpu device, preferring the given backend.
    ///
    /// Pass `None` to auto-select the best available backend.
    pub async fn new(preferred_backend: Option<GpuBackend>) -> Result<Self> {
        let backends = match preferred_backend {
            Some(GpuBackend::Vulkan) => wgpu::Backends::VULKAN,
            Some(GpuBackend::D3D12) => wgpu::Backends::DX12,
            Some(GpuBackend::Metal) => wgpu::Backends::METAL,
            Some(GpuBackend::OpenGl) => wgpu::Backends::GL,
            _ => wgpu::Backends::all(),
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(WgpuError::NoAdapter)?;

        let info = adapter.get_info();
        let backend = match info.backend {
            wgpu::Backend::Vulkan => GpuBackend::Vulkan,
            wgpu::Backend::Dx12 => GpuBackend::D3D12,
            wgpu::Backend::Metal => GpuBackend::Metal,
            wgpu::Backend::Gl => GpuBackend::OpenGl,
            _ => GpuBackend::Vulkan,
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("liquide-renderer"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| WgpuError::DeviceRequest(e.to_string()))?;

        log::info!(
            "wgpu device: {} ({}) via {}",
            info.name,
            info.vendor,
            backend
        );

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            backend,
            device_name: info.name.clone(),
            vendor_id: info.vendor as u32,
        })
    }

    /// List all available GPU adapters for diagnostics.
    pub fn enumerate_adapters(instance: &wgpu::Instance) -> Vec<(String, GpuBackend)> {
        instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .map(|a| {
                let info = a.get_info();
                let backend = match info.backend {
                    wgpu::Backend::Vulkan => GpuBackend::Vulkan,
                    wgpu::Backend::Dx12 => GpuBackend::D3D12,
                    wgpu::Backend::Metal => GpuBackend::Metal,
                    wgpu::Backend::Gl => GpuBackend::OpenGl,
                    _ => GpuBackend::Vulkan,
                };
                (info.name.clone(), backend)
            })
            .collect()
    }
}

impl std::fmt::Debug for WgpuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuDevice")
            .field("backend", &self.backend)
            .field("device_name", &self.device_name)
            .field("vendor_id", &self.vendor_id)
            .finish()
    }
}
