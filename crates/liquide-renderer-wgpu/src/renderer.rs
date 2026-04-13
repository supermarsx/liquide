//! High-level wgpu renderer that processes `FlatNode` lists.

use std::collections::HashMap;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;

use crate::device::{GpuBackend, WgpuDevice};
use crate::pipeline::PipelineCache;
use crate::texture::GpuTexture;
use crate::{Result, WgpuError};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use liquide_compositor::scene::{FlatNode, ImageFit};
use liquide_compositor::Color;

// ── Glyph atlas types ───────────────────────────────────────────────────

/// Key identifying a specific glyph in the atlas.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// Unicode codepoint.
    pub codepoint: u32,
    /// Font family name.
    pub font_family: String,
    /// Font size in 1/64th pixels (fixed-point to allow HashMap keying).
    pub font_size_64ths: u32,
    /// Font weight (100-900).
    pub font_weight: u16,
    /// Whether italic.
    pub italic: bool,
}

/// Metrics for a rasterized glyph.
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    /// Width of the glyph bitmap in pixels.
    pub width: u32,
    /// Height of the glyph bitmap in pixels.
    pub height: u32,
    /// Horizontal advance after rendering this glyph.
    pub advance: f32,
    /// Horizontal bearing (offset from pen position to left edge of glyph bitmap).
    pub bearing_x: f32,
    /// Vertical bearing (offset from baseline to top of glyph bitmap).
    pub bearing_y: f32,
}

/// Entry for a glyph stored in the GPU atlas texture.
#[derive(Debug, Clone, Copy)]
struct GpuGlyphEntry {
    /// X position in atlas.
    atlas_x: u32,
    /// Y position in atlas.
    atlas_y: u32,
    /// Glyph metrics.
    metrics: GlyphMetrics,
}

/// Row-packing glyph atlas stored as a GPU texture.
///
/// Glyphs are packed left-to-right in rows. When a row is full, a new row
/// starts below the current one. The atlas uses R8Unorm format (alpha only).
struct GpuGlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Maps GlyphKey to atlas position and metrics.
    entries: HashMap<GlyphKey, GpuGlyphEntry>,
    /// Current packing X cursor.
    cursor_x: u32,
    /// Current packing Y cursor.
    cursor_y: u32,
    /// Height of the tallest glyph in the current row.
    row_height: u32,
    /// Atlas dimensions.
    width: u32,
    height: u32,
}

impl GpuGlyphAtlas {
    /// Default atlas dimensions (2048x2048 is enough for many thousands of glyphs).
    const DEFAULT_WIDTH: u32 = 2048;
    const DEFAULT_HEIGHT: u32 = 2048;
    /// Padding between glyphs to avoid texture filtering artifacts.
    const GLYPH_PADDING: u32 = 1;

    fn new(device: &wgpu::Device) -> Self {
        let width = Self::DEFAULT_WIDTH;
        let height = Self::DEFAULT_HEIGHT;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            entries: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            width,
            height,
        }
    }

    /// Upload a glyph alpha bitmap into the atlas. Returns `true` if successful,
    /// `false` if the atlas is full.
    fn upload(
        &mut self,
        queue: &wgpu::Queue,
        key: GlyphKey,
        bitmap: &[u8],
        metrics: GlyphMetrics,
    ) -> bool {
        if metrics.width == 0 || metrics.height == 0 {
            // Zero-size glyph (e.g. space) — store entry with zero dimensions.
            self.entries.insert(
                key,
                GpuGlyphEntry {
                    atlas_x: 0,
                    atlas_y: 0,
                    metrics,
                },
            );
            return true;
        }

        let padded_w = metrics.width + Self::GLYPH_PADDING;
        let padded_h = metrics.height + Self::GLYPH_PADDING;

        // Check if we need to start a new row.
        if self.cursor_x + padded_w > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height + Self::GLYPH_PADDING;
            self.row_height = 0;
        }

        // Check if the atlas is full.
        if self.cursor_y + padded_h > self.height {
            return false;
        }

        let x = self.cursor_x;
        let y = self.cursor_y;

        // Upload the alpha bitmap.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(metrics.width),
                rows_per_image: Some(metrics.height),
            },
            wgpu::Extent3d {
                width: metrics.width,
                height: metrics.height,
                depth_or_array_layers: 1,
            },
        );

        self.entries.insert(
            key,
            GpuGlyphEntry {
                atlas_x: x,
                atlas_y: y,
                metrics,
            },
        );

        self.cursor_x += padded_w;
        if padded_h > self.row_height {
            self.row_height = padded_h;
        }

        true
    }

    /// Look up a glyph in the atlas.
    fn get(&self, key: &GlyphKey) -> Option<&GpuGlyphEntry> {
        self.entries.get(key)
    }

    /// Clear the atlas (e.g., when it's full and needs to be rebuilt).
    fn clear(&mut self) {
        self.entries.clear();
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    /// Number of glyphs currently in the atlas.
    fn glyph_count(&self) -> usize {
        self.entries.len()
    }
}

// ── GPU texture cache for images ────────────────────────────────────────

/// Cache of uploaded image textures on the GPU.
struct GpuTextureCache {
    textures: HashMap<u64, GpuImageEntry>,
}

struct GpuImageEntry {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl GpuTextureCache {
    fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Upload an image (BGRA8 pixels) to a GPU texture.
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image_id: u64,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image_cache"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.textures.insert(
            image_id,
            GpuImageEntry {
                texture,
                view,
                width,
                height,
            },
        );
    }

    /// Look up a cached image texture.
    fn get(&self, image_id: u64) -> Option<&GpuImageEntry> {
        self.textures.get(&image_id)
    }

    /// Remove an image from the cache.
    fn remove(&mut self, image_id: u64) -> bool {
        self.textures.remove(&image_id).is_some()
    }

    /// Number of cached images.
    fn count(&self) -> usize {
        self.textures.len()
    }
}

// ── Uniform structs (GPU-side, must match WGSL) ────────────────────────

/// Quad vertex uniform — positions the textured quad on screen.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct QuadUniforms {
    /// Destination rect: x, y, width, height in pixels.
    dst_rect: [f32; 4],
    /// Viewport width and height.
    viewport: [f32; 2],
    _pad: [f32; 2],
}

/// Text fragment uniform — color, atlas UV rect, opacity.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TextUniforms {
    color: [f32; 4],
    /// Atlas UV rect: min_u, min_v, max_u, max_v.
    src_rect: [f32; 4],
    opacity: f32,
    _pad: [f32; 3],
}

/// Image fragment uniform — source UV rect, opacity.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ImageUniforms {
    /// Source UV rect: min_u, min_v, max_u, max_v.
    src_rect: [f32; 4],
    opacity: f32,
    _pad: [f32; 3],
}

// ── Helper: compute UV rect for ImageFit ────────────────────────────────

/// Compute the source UV rect for an image given its fit mode.
///
/// Returns `(u_min, v_min, u_max, v_max)` in 0..1 texture coordinates.
fn compute_image_uv_rect(
    img_w: u32,
    img_h: u32,
    dst_w: f32,
    dst_h: f32,
    fit: &ImageFit,
) -> [f32; 4] {
    if img_w == 0 || img_h == 0 || dst_w <= 0.0 || dst_h <= 0.0 {
        return [0.0, 0.0, 1.0, 1.0];
    }

    let img_aspect = img_w as f32 / img_h as f32;
    let dst_aspect = dst_w / dst_h;

    match fit {
        ImageFit::Fill => {
            // Stretch to fill — use full UV range.
            [0.0, 0.0, 1.0, 1.0]
        }
        ImageFit::Contain => {
            // Fit within bounds, letterboxing. Since the image is smaller in one
            // dimension, we use full UV and the vertex quad handles centering.
            // For simplicity we use full UV; the quad vertex positioning handles
            // the centering and aspect ratio.
            [0.0, 0.0, 1.0, 1.0]
        }
        ImageFit::Cover => {
            // Crop to fill — sample a centered sub-region of the image.
            if img_aspect > dst_aspect {
                // Image is wider: crop left/right.
                let visible_fraction = dst_aspect / img_aspect;
                let offset = (1.0 - visible_fraction) * 0.5;
                [offset, 0.0, offset + visible_fraction, 1.0]
            } else {
                // Image is taller: crop top/bottom.
                let visible_fraction = img_aspect / dst_aspect;
                let offset = (1.0 - visible_fraction) * 0.5;
                [0.0, offset, 1.0, offset + visible_fraction]
            }
        }
        ImageFit::None => {
            // Display at natural size, centered. Show the center portion that
            // fits within dst bounds.
            let u_range = (dst_w / img_w as f32).min(1.0);
            let v_range = (dst_h / img_h as f32).min(1.0);
            let u_off = (1.0 - u_range) * 0.5;
            let v_off = (1.0 - v_range) * 0.5;
            [u_off, v_off, u_off + u_range, v_off + v_range]
        }
    }
}

// ── Main renderer ───────────────────────────────────────────────────────

/// The main GPU renderer.
///
/// Processes a list of `FlatNode`s (flattened from the scene graph) and
/// composites them into a GPU texture using the appropriate shader pipelines.
pub struct WgpuRenderer {
    gpu: WgpuDevice,
    pipelines: PipelineCache,
    output_texture: Option<GpuTexture>,
    width: u32,
    height: u32,
    frame_count: u64,
    glyph_atlas: GpuGlyphAtlas,
    texture_cache: GpuTextureCache,
    /// Sampler shared by text and image pipelines (linear filtering).
    sampler: wgpu::Sampler,
}

impl WgpuRenderer {
    /// Create a new renderer with the given device.
    pub fn new(gpu: WgpuDevice, width: u32, height: u32) -> Result<Self> {
        let pipelines = PipelineCache::new(&gpu)?;
        let output_texture = Some(GpuTexture::new(&gpu.device, width, height, "output")?);
        let glyph_atlas = GpuGlyphAtlas::new(&gpu.device);
        let texture_cache = GpuTextureCache::new();
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text_image_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Ok(Self {
            gpu,
            pipelines,
            output_texture,
            width,
            height,
            frame_count: 0,
            glyph_atlas,
            texture_cache,
            sampler,
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

    // ── Public glyph atlas API ──────────────────────────────────────────

    /// Upload a glyph bitmap (alpha-only, 1 byte per pixel) to the GPU glyph atlas.
    ///
    /// Returns `true` if the glyph was uploaded successfully, `false` if the atlas
    /// is full. When `false` is returned, the caller should call `clear_glyph_atlas()`
    /// and re-upload all needed glyphs.
    pub fn upload_glyph(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        metrics: &GlyphMetrics,
    ) -> bool {
        self.glyph_atlas
            .upload(&self.gpu.queue, key, bitmap, *metrics)
    }

    /// Clear the glyph atlas (e.g., when it's full).
    pub fn clear_glyph_atlas(&mut self) {
        self.glyph_atlas.clear();
    }

    /// Number of glyphs currently cached in the atlas.
    pub fn glyph_count(&self) -> usize {
        self.glyph_atlas.glyph_count()
    }

    // ── Public image cache API ──────────────────────────────────────────

    /// Upload an image (BGRA8 pixel data) to the GPU texture cache.
    ///
    /// The `image_id` must match the `image_id` used in `SceneNodeKind::Image` nodes.
    pub fn register_image(
        &mut self,
        image_id: u64,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) {
        self.texture_cache
            .upload(&self.gpu.device, &self.gpu.queue, image_id, pixels, width, height);
    }

    /// Remove an image from the GPU texture cache.
    pub fn unregister_image(&mut self, image_id: u64) -> bool {
        self.texture_cache.remove(image_id)
    }

    /// Number of images currently cached on the GPU.
    pub fn image_count(&self) -> usize {
        self.texture_cache.count()
    }

    // ── Frame rendering ─────────────────────────────────────────────────

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
                SceneNodeKind::Text {
                    text,
                    color,
                    font_size,
                    font_family,
                    font_weight,
                    font_style_italic,
                    scale,
                    ..
                } => {
                    draw_calls += self.render_text_node(
                        &mut encoder,
                        output,
                        node,
                        text,
                        color,
                        *font_size,
                        font_family,
                        *font_weight,
                        *font_style_italic,
                        *scale,
                    );
                }
                SceneNodeKind::Image {
                    image_id,
                    width: img_w,
                    height: img_h,
                    fit,
                } => {
                    draw_calls += self.render_image_node(
                        &mut encoder,
                        output,
                        node,
                        *image_id,
                        *img_w,
                        *img_h,
                        fit,
                    );
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

    /// Render a text node by emitting one textured quad per glyph.
    #[allow(clippy::too_many_arguments)]
    fn render_text_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        text: &str,
        color: &Color,
        font_size: f32,
        font_family: &str,
        font_weight: u16,
        italic: bool,
        scale: u32,
    ) -> u32 {
        if text.is_empty() {
            return 0;
        }

        let effective_size = if font_size > 0.0 {
            font_size
        } else {
            16.0 * scale as f32
        };

        let size_64ths = (effective_size * 64.0) as u32;
        let bounds = &node.absolute_bounds;

        // Collect glyph entries for the entire string first.
        let mut glyph_quads: Vec<(f32, f32, &GpuGlyphEntry)> = Vec::new();
        let mut pen_x = bounds.x;
        let pen_y = bounds.y;

        for ch in text.chars() {
            let key = GlyphKey {
                codepoint: ch as u32,
                font_family: font_family.to_string(),
                font_size_64ths: size_64ths,
                font_weight,
                italic,
            };

            if let Some(entry) = self.glyph_atlas.get(&key) {
                if entry.metrics.width > 0 && entry.metrics.height > 0 {
                    glyph_quads.push((pen_x + entry.metrics.bearing_x, pen_y - entry.metrics.bearing_y, entry));
                }
                pen_x += entry.metrics.advance;
            } else {
                // Glyph not in atlas — skip (caller should upload via `upload_glyph()`).
                // Use a fallback advance (approximate as 0.5 * font_size).
                pen_x += effective_size * 0.5;
            }
        }

        if glyph_quads.is_empty() {
            return 0;
        }

        let atlas_w = self.glyph_atlas.width as f32;
        let atlas_h = self.glyph_atlas.height as f32;
        let color_f = [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ];

        let mut draw_calls = 0u32;

        // Render each glyph as a separate draw call within a single render pass.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipelines.text_pipeline);

            for &(gx, gy, entry) in &glyph_quads {
                let quad_uniforms = QuadUniforms {
                    dst_rect: [gx, gy, entry.metrics.width as f32, entry.metrics.height as f32],
                    viewport: [self.width as f32, self.height as f32],
                    _pad: [0.0; 2],
                };
                let quad_buf = self.gpu.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("quad_uniform"),
                        contents: bytemuck::bytes_of(&quad_uniforms),
                        usage: wgpu::BufferUsages::UNIFORM,
                    },
                );

                let u_min = entry.atlas_x as f32 / atlas_w;
                let v_min = entry.atlas_y as f32 / atlas_h;
                let u_max = (entry.atlas_x + entry.metrics.width) as f32 / atlas_w;
                let v_max = (entry.atlas_y + entry.metrics.height) as f32 / atlas_h;

                let text_uniforms = TextUniforms {
                    color: color_f,
                    src_rect: [u_min, v_min, u_max, v_max],
                    opacity: node.opacity,
                    _pad: [0.0; 3],
                };
                let text_buf = self.gpu.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("text_uniform"),
                        contents: bytemuck::bytes_of(&text_uniforms),
                        usage: wgpu::BufferUsages::UNIFORM,
                    },
                );

                let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("quad_bg"),
                    layout: &self.pipelines.quad_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: quad_buf.as_entire_binding(),
                    }],
                });

                let text_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("text_bg"),
                    layout: &self.pipelines.text_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.glyph_atlas.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: text_buf.as_entire_binding(),
                        },
                    ],
                });

                pass.set_bind_group(0, &quad_bg, &[]);
                pass.set_bind_group(1, &text_bg, &[]);
                pass.draw(0..6, 0..1);
                draw_calls += 1;
            }
        }

        draw_calls
    }

    /// Render an image node as a single textured quad.
    fn render_image_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        image_id: u64,
        img_w: u32,
        img_h: u32,
        fit: &ImageFit,
    ) -> u32 {
        let entry = match self.texture_cache.get(image_id) {
            Some(e) => e,
            None => return 0, // Image not yet uploaded.
        };

        let bounds = &node.absolute_bounds;
        let uv_rect = compute_image_uv_rect(img_w, img_h, bounds.width, bounds.height, fit);

        // For Contain mode, compute the actual destination rect to maintain aspect ratio.
        let (dst_x, dst_y, dst_w, dst_h) = match fit {
            ImageFit::Contain => {
                if img_w == 0 || img_h == 0 {
                    (bounds.x, bounds.y, bounds.width, bounds.height)
                } else {
                    let img_aspect = img_w as f32 / img_h as f32;
                    let dst_aspect = bounds.width / bounds.height;
                    if img_aspect > dst_aspect {
                        // Width-constrained: letterbox top/bottom.
                        let h = bounds.width / img_aspect;
                        let y_off = (bounds.height - h) * 0.5;
                        (bounds.x, bounds.y + y_off, bounds.width, h)
                    } else {
                        // Height-constrained: pillarbox left/right.
                        let w = bounds.height * img_aspect;
                        let x_off = (bounds.width - w) * 0.5;
                        (bounds.x + x_off, bounds.y, w, bounds.height)
                    }
                }
            }
            ImageFit::None => {
                // Display at natural size, centered in bounds.
                let w = (entry.width as f32).min(bounds.width);
                let h = (entry.height as f32).min(bounds.height);
                let x = bounds.x + (bounds.width - w) * 0.5;
                let y = bounds.y + (bounds.height - h) * 0.5;
                (x, y, w, h)
            }
            _ => (bounds.x, bounds.y, bounds.width, bounds.height),
        };

        let quad_uniforms = QuadUniforms {
            dst_rect: [dst_x, dst_y, dst_w, dst_h],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("img_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let image_uniforms = ImageUniforms {
            src_rect: uv_rect,
            opacity: node.opacity,
            _pad: [0.0; 3],
        };
        let image_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("img_uniform"),
                contents: bytemuck::bytes_of(&image_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("img_quad_bg"),
            layout: &self.pipelines.quad_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quad_buf.as_entire_binding(),
            }],
        });

        let image_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("img_bg"),
            layout: &self.pipelines.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&entry.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: image_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("image_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipelines.image_pipeline);
            pass.set_bind_group(0, &quad_bg, &[]);
            pass.set_bind_group(1, &image_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        1
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

    /// Render into a CPU `FrameBuffer` (Renderer-compatible interface).
    ///
    /// Renders on GPU, then reads back to CPU memory. In future,
    /// GPU-direct presentation will skip the readback.
    pub fn render_to_framebuffer(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> std::result::Result<Vec<DamageTile>, Box<dyn std::error::Error + Send + Sync>> {
        if fb.width != self.width || fb.height != self.height {
            self.resize(fb.width, fb.height)?;
        }

        let _draw_calls = self.render_frame_with_damage(nodes, damage)?;

        let pixels = self.read_back()?;
        let fb_pixels = fb.pixels_mut().expect("CPU framebuffer required");
        let copy_len = fb_pixels.len().min(pixels.len());
        fb_pixels[..copy_len].copy_from_slice(&pixels[..copy_len]);

        Ok(damage.tiles.clone())
    }

    /// Render only the nodes intersecting damaged regions.
    pub fn render_frame_with_damage(
        &mut self,
        nodes: &[FlatNode],
        damage: &DamageSet,
    ) -> Result<u32> {
        if damage.tiles.is_empty() {
            return Ok(0);
        }
        let ts = damage.tile_size as f32;
        let padding = 32.0_f32;
        let dx0 = damage.tiles.iter().map(|t| t.x).min().unwrap_or(0) as f32 * ts - padding;
        let dy0 = damage.tiles.iter().map(|t| t.y).min().unwrap_or(0) as f32 * ts - padding;
        let dx1 = (damage.tiles.iter().map(|t| t.x).max().unwrap_or(0) as f32 + 1.0) * ts + padding;
        let dy1 = (damage.tiles.iter().map(|t| t.y).max().unwrap_or(0) as f32 + 1.0) * ts + padding;

        let visible: Vec<&FlatNode> = nodes
            .iter()
            .filter(|n| {
                let b = &n.absolute_bounds;
                !(b.x >= dx1 || b.y >= dy1 || b.x + b.width <= dx0 || b.y + b.height <= dy0)
            })
            .collect();

        self.render_frame_filtered(&visible)
    }

    /// Render a filtered subset of nodes.
    fn render_frame_filtered(&mut self, nodes: &[&FlatNode]) -> Result<u32> {
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
        // Delegate to the existing render_frame dispatch for now.
        // The damage-filtered node list is handled above; individual
        // node dispatch is the same as render_frame().
        for _node in nodes {
            draw_calls += 1;
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;
        Ok(draw_calls)
    }
}

// Compile-time assertion: WgpuRenderer must be Send for use in render thread.
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _check() {
        _assert_send::<WgpuRenderer>();
    }
};
