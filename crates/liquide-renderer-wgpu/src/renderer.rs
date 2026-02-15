//! High-level wgpu renderer that processes `FlatNode` lists.

use crate::device::{GpuBackend, WgpuDevice};
use crate::pipeline::PipelineCache;
use crate::texture::GpuTexture;
use crate::{Result, WgpuError};

use liquide_compositor::scene::FlatNode;

/// The main GPU renderer.
///
/// Processes a list of `FlatNode`s (flattened from the scene graph) and
/// composites them into a GPU texture using the appropriate shader pipelines.
pub struct WgpuRenderer {
    gpu: WgpuDevice,
    #[allow(dead_code)]
    pipelines: PipelineCache,
    output_texture: Option<GpuTexture>,
    width: u32,
    height: u32,
    frame_count: u64,
}

impl WgpuRenderer {
    /// Create a new renderer with the given device.
    pub fn new(gpu: WgpuDevice, width: u32, height: u32) -> Result<Self> {
        let pipelines = PipelineCache::new(&gpu)?;
        let output_texture = Some(GpuTexture::new(&gpu.device, width, height, "output")?);

        Ok(Self {
            gpu,
            pipelines,
            output_texture,
            width,
            height,
            frame_count: 0,
        })
    }

    /// Create a renderer with auto-detected GPU.
    pub async fn auto(width: u32, height: u32) -> Result<Self> {
        let gpu = WgpuDevice::new(None).await?;
        Self::new(gpu, width, height)
    }

    /// Create a renderer preferring a specific backend (D3D12, Vulkan, Metal).
    pub async fn with_backend(
        backend: GpuBackend,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let gpu = WgpuDevice::new(Some(backend)).await?;
        Self::new(gpu, width, height)
    }

    /// Resize the output texture.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.output_texture = Some(GpuTexture::new(
            &self.gpu.device,
            width,
            height,
            "output",
        )?);
        Ok(())
    }

    /// Render a frame from the flattened scene graph.
    ///
    /// Returns the number of draw calls issued.
    pub fn render_frame(&mut self, nodes: &[FlatNode]) -> Result<u32> {
        let output = self
            .output_texture
            .as_ref()
            .ok_or_else(|| WgpuError::RenderFailed("no output texture".into()))?;

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        // Clear to black
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        let mut draw_calls = 0u32;

        // Process each flat node
        for node in nodes {
            use liquide_compositor::scene::SceneNodeKind;
            match &node.kind {
                SceneNodeKind::Background { .. }
                | SceneNodeKind::Tint { .. }
                | SceneNodeKind::Glass(_) => {
                    // TODO: rect fill pipeline dispatch
                    draw_calls += 1;
                }
                SceneNodeKind::Shadow { .. } | SceneNodeKind::BoxShadows { .. } => {
                    // TODO: shadow pipeline dispatch
                    draw_calls += 1;
                }
                SceneNodeKind::GradientFill { .. } => {
                    // TODO: gradient pipeline dispatch
                    draw_calls += 1;
                }
                SceneNodeKind::Filter { .. } | SceneNodeKind::BackdropFilter { .. } => {
                    // TODO: blur pipeline dispatch (multi-pass)
                    draw_calls += 1;
                }
                SceneNodeKind::RenderLayer { .. } => {
                    // TODO: blend pipeline dispatch
                    draw_calls += 1;
                }
                SceneNodeKind::Surface { buffer, .. }
                | SceneNodeKind::ChildSurface { buffer, .. } => {
                    if buffer.is_some() {
                        // TODO: texture blit
                        draw_calls += 1;
                    }
                }
                _ => {
                    // Not yet handled by GPU pipeline
                }
            }
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;

        Ok(draw_calls)
    }

    /// Read back the output texture to CPU memory (BGRA8).
    pub fn read_back(&self) -> Result<Vec<u8>> {
        let output = self
            .output_texture
            .as_ref()
            .ok_or_else(|| WgpuError::RenderFailed("no output texture".into()))?;

        let buffer_size = (4 * self.width * self.height) as u64;
        let staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback_encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * self.width),
                    rows_per_image: Some(self.height),
                },
            },
            output.size,
        );

        self.gpu.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| WgpuError::RenderFailed(e.to_string()))?
            .map_err(|e| WgpuError::RenderFailed(e.to_string()))?;

        let data = slice.get_mapped_range().to_vec();
        Ok(data)
    }

    /// Get backend info.
    pub fn backend(&self) -> GpuBackend {
        self.gpu.backend
    }

    /// Get device name.
    pub fn device_name(&self) -> &str {
        &self.gpu.device_name
    }

    /// Total frames rendered.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Output dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
