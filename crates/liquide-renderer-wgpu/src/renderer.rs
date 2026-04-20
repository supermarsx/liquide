//! High-level wgpu renderer that processes `FlatNode` lists.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::{Renderer, RenderResult};

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

/// Rect fill uniform — color + bounds for SDF rounded rect.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct RectUniforms {
    color: [f32; 4],
    /// bounds: x, y, w, h (used by SDF — x/y ignored in quad-positioned mode).
    bounds: [f32; 4],
    corner_radius: f32,
    opacity: f32,
    _pad: [f32; 2],
}

/// Shadow uniform — matches BOX_SHADOW_FRAG ShadowUniforms.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ShadowUniforms {
    bounds: [f32; 4],
    color: [f32; 4],
    offset: [f32; 2],
    blur: f32,
    spread: f32,
    radius: f32,
    inset: u32,
    _pad: [f32; 2],
}

/// Gradient uniform — matches GRADIENT_FRAG GradientUniforms.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GradientUniforms {
    kind: u32,
    angle: f32,
    center: [f32; 2],
    radius: f32,
    stop_count: u32,
    _pad: [f32; 2],
}

/// Gradient stop for GPU storage buffer (padded to 32 bytes for WGSL alignment).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GradientStopGpu {
    position: f32,
    _pad: [f32; 3],
    color: [f32; 4],
}

/// Blur uniform — matches BLUR_FRAG BlurUniforms.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BlurUniforms {
    direction: [f32; 2],
    radius: f32,
    _pad: f32,
}

/// Blend uniform — matches BLEND_COMPUTE BlendUniforms.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct BlendUniforms {
    mode: u32,
    _pad: [u32; 3],
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Convert a `Color` to normalized `[f32; 4]`.
fn color_to_f32(c: &Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

/// Convert a `BlendMode` to the u32 index used by the blend compute shader.
fn blend_mode_to_gpu(mode: &liquide_compositor::pixel::BlendMode) -> u32 {
    use liquide_compositor::pixel::BlendMode;
    match mode {
        BlendMode::SrcOver => 0,
        BlendMode::Src => 1,
        BlendMode::SrcAtop => 2,
        BlendMode::Multiply => 3,
        BlendMode::Screen => 4,
        BlendMode::Overlay => 5,
        BlendMode::Darken => 6,
        BlendMode::Lighten => 7,
        BlendMode::ColorDodge => 8,
        BlendMode::ColorBurn => 9,
        BlendMode::HardLight => 10,
        BlendMode::SoftLight => 11,
        BlendMode::Difference => 12,
        BlendMode::Exclusion => 13,
        BlendMode::Hue => 14,
        BlendMode::Saturation => 15,
        BlendMode::ColorBlend => 16,
        BlendMode::Luminosity => 17,
    }
}

/// Build GPU gradient stops from a list of (position, Color) pairs.
fn build_gradient_stops(stops: &[(f32, Color)]) -> Vec<GradientStopGpu> {
    stops
        .iter()
        .map(|(pos, c)| GradientStopGpu {
            position: *pos,
            _pad: [0.0; 3],
            color: color_to_f32(c),
        })
        .collect()
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
    /// Intermediate texture for multi-pass effects (blur, blend).
    intermediate_texture: Option<GpuTexture>,
    width: u32,
    height: u32,
    frame_count: u64,
    glyph_atlas: GpuGlyphAtlas,
    texture_cache: GpuTextureCache,
    /// Sampler shared by text and image pipelines (linear filtering).
    sampler: wgpu::Sampler,
    /// Flag set when the GPU device is lost; triggers CPU fallback.
    device_lost: Arc<AtomicBool>,
}

impl WgpuRenderer {
    /// Create a new renderer with the given device.
    pub fn new(gpu: WgpuDevice, width: u32, height: u32) -> Result<Self> {
        let pipelines = PipelineCache::new(&gpu)?;
        let output_texture = Some(GpuTexture::new(&gpu.device, width, height, "output")?);
        let intermediate_texture =
            Some(GpuTexture::new(&gpu.device, width, height, "intermediate")?);
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

        let device_lost = Arc::new(AtomicBool::new(false));

        Ok(Self {
            gpu,
            pipelines,
            output_texture,
            intermediate_texture,
            width,
            height,
            frame_count: 0,
            glyph_atlas,
            texture_cache,
            sampler,
            device_lost,
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
        self.intermediate_texture = Some(GpuTexture::new(
            &self.gpu.device,
            width,
            height,
            "intermediate",
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

    /// Whether the GPU device has been lost (e.g. driver crash, TDR).
    ///
    /// When `true`, all rendering calls return an error and the caller
    /// should fall back to CPU rendering or re-create the renderer.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Acquire)
    }

    /// Mark the device as lost. Called internally on GPU errors or
    /// externally when the compositor detects a device-lost event.
    pub fn mark_device_lost(&self) {
        self.device_lost.store(true, Ordering::Release);
        log::error!("wgpu device marked as lost — GPU rendering disabled");
    }

    /// Render a frame from the flattened scene graph.
    ///
    /// Returns the number of draw calls issued.
    pub fn render_frame(&mut self, nodes: &[FlatNode]) -> Result<u32> {
        if self.device_lost.load(Ordering::Acquire) {
            return Err(WgpuError::RenderFailed("GPU device lost".into()));
        }

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
                SceneNodeKind::Background { color } => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        color, 0.0,
                    );
                }
                SceneNodeKind::Tint { color } => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        color, 0.0,
                    );
                }
                SceneNodeKind::Glass(params) => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        &params.tint_color, 0.0,
                    );
                }
                SceneNodeKind::Shadow {
                    spread,
                    blur_radius,
                    color,
                    corner_radius,
                } => {
                    draw_calls += self.render_shadow_node(
                        &mut encoder, output, node,
                        [0.0, 0.0], *blur_radius, *spread, color, *corner_radius, false,
                    );
                }
                SceneNodeKind::BoxShadows { shadows } => {
                    for s in shadows {
                        draw_calls += self.render_shadow_node(
                            &mut encoder, output, node,
                            [s.offset_x, s.offset_y],
                            s.blur_radius, s.spread_radius,
                            &s.color, node.corner_radius.0, s.inset,
                        );
                    }
                }
                SceneNodeKind::GradientFill { gradient } => {
                    draw_calls += self.render_gradient_node(
                        &mut encoder, output, node, gradient,
                    );
                }
                SceneNodeKind::Filter { filters } => {
                    draw_calls += self.render_blur_node(
                        &mut encoder, node, filters,
                    );
                }
                SceneNodeKind::BackdropFilter { filters } => {
                    draw_calls += self.render_backdrop_blur_node(
                        &mut encoder, node, filters,
                    );
                }
                SceneNodeKind::RenderLayer { blend_mode, .. } => {
                    draw_calls += self.render_blend_node(
                        &mut encoder, blend_mode,
                    );
                }
                SceneNodeKind::Surface { buffer, .. }
                | SceneNodeKind::ChildSurface { buffer, .. } => {
                    if let Some(buf) = buffer {
                        draw_calls += self.render_surface_node(
                            &mut encoder, output, node, buf,
                        );
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

    // ── Rect fill dispatch (Background / Tint / Glass) ──────────────────

    /// Render a solid-color rounded rectangle using the rect fill pipeline.
    fn render_rect_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        color: &Color,
        corner_radius: f32,
    ) -> u32 {
        let bounds = &node.absolute_bounds;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return 0;
        }

        // Use per-node corner radius if the arg is 0
        let cr = if corner_radius > 0.0 {
            corner_radius
        } else {
            node.corner_radius.0
        };

        let quad_uniforms = QuadUniforms {
            dst_rect: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("rect_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let rect_uniforms = RectUniforms {
            color: color_to_f32(color),
            bounds: [0.0, 0.0, bounds.width, bounds.height],
            corner_radius: cr,
            opacity: node.opacity,
            _pad: [0.0; 2],
        };
        let rect_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("rect_uniform"),
                contents: bytemuck::bytes_of(&rect_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect_quad_bg"),
            layout: &self.pipelines.quad_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quad_buf.as_entire_binding(),
            }],
        });

        let rect_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rect_bg"),
            layout: &self.pipelines.rect_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rect_buf.as_entire_binding(),
            }],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rect_pass"),
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

            pass.set_pipeline(&self.pipelines.rect_pipeline);
            pass.set_bind_group(0, &quad_bg, &[]);
            pass.set_bind_group(1, &rect_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        1
    }

    // ── Shadow dispatch (Shadow / BoxShadows) ───────────────────────────

    /// Render a box shadow using the shadow SDF pipeline.
    #[allow(clippy::too_many_arguments)]
    fn render_shadow_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        offset: [f32; 2],
        blur_radius: f32,
        spread: f32,
        color: &Color,
        corner_radius: f32,
        inset: bool,
    ) -> u32 {
        let bounds = &node.absolute_bounds;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return 0;
        }

        // Expand the draw area by blur + spread so the soft edge is visible.
        let expand = blur_radius + spread;
        let dst_x = bounds.x - expand;
        let dst_y = bounds.y - expand;
        let dst_w = bounds.width + expand * 2.0;
        let dst_h = bounds.height + expand * 2.0;

        let quad_uniforms = QuadUniforms {
            dst_rect: [dst_x, dst_y, dst_w, dst_h],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("shadow_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let shadow_uniforms = ShadowUniforms {
            bounds: [0.0, 0.0, dst_w, dst_h],
            color: color_to_f32(color),
            offset,
            blur: blur_radius,
            spread,
            radius: corner_radius,
            inset: if inset { 1 } else { 0 },
            _pad: [0.0; 2],
        };
        let shadow_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("shadow_uniform"),
                contents: bytemuck::bytes_of(&shadow_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_quad_bg"),
            layout: &self.pipelines.quad_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quad_buf.as_entire_binding(),
            }],
        });

        let shadow_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bg"),
            layout: &self.pipelines.shadow_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_buf.as_entire_binding(),
            }],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow_pass"),
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

            pass.set_pipeline(&self.pipelines.shadow_pipeline);
            pass.set_bind_group(0, &quad_bg, &[]);
            pass.set_bind_group(1, &shadow_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        1
    }

    // ── Gradient dispatch ───────────────────────────────────────────────

    /// Render a gradient fill (linear, radial, or conic) using the gradient pipeline.
    fn render_gradient_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        gradient: &liquide_compositor::scene::GradientSpec,
    ) -> u32 {
        use liquide_compositor::scene::GradientSpec;

        let bounds = &node.absolute_bounds;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return 0;
        }

        let (kind, angle, center, radius, stops) = match gradient {
            GradientSpec::Linear {
                start_x,
                start_y,
                end_x,
                end_y,
                stops,
            } => {
                let dx = end_x - start_x;
                let dy = end_y - start_y;
                let angle = dy.atan2(dx);
                (0u32, angle, [0.5f32, 0.5], 1.0f32, stops.as_slice())
            }
            GradientSpec::Radial {
                center_x,
                center_y,
                radius,
                stops,
                ..
            } => (1u32, 0.0, [*center_x, *center_y], *radius, stops.as_slice()),
            GradientSpec::Conic {
                center_x,
                center_y,
                start_angle,
                stops,
            } => (
                2u32,
                *start_angle,
                [*center_x, *center_y],
                1.0,
                stops.as_slice(),
            ),
            GradientSpec::Mesh { .. } => {
                // Mesh gradients not yet supported in the GPU shader.
                return 0;
            }
        };

        let gpu_stops = build_gradient_stops(stops);
        let stop_count = gpu_stops.len().min(64) as u32; // clamp to reasonable max

        let quad_uniforms = QuadUniforms {
            dst_rect: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("gradient_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let gradient_uniforms = GradientUniforms {
            kind,
            angle,
            center,
            radius,
            stop_count,
            _pad: [0.0; 2],
        };
        let gradient_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("gradient_uniform"),
                contents: bytemuck::bytes_of(&gradient_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        // Storage buffer for gradient stops.
        // Ensure at least one stop so the buffer is non-empty.
        let stops_data: Vec<GradientStopGpu> = if gpu_stops.is_empty() {
            vec![GradientStopGpu {
                position: 0.0,
                _pad: [0.0; 3],
                color: [0.0; 4],
            }]
        } else {
            gpu_stops
        };
        let stops_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("gradient_stops"),
                contents: bytemuck::cast_slice(&stops_data),
                usage: wgpu::BufferUsages::STORAGE,
            },
        );

        let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gradient_quad_bg"),
            layout: &self.pipelines.quad_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quad_buf.as_entire_binding(),
            }],
        });

        let gradient_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gradient_bg"),
            layout: &self.pipelines.gradient_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gradient_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: stops_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gradient_pass"),
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

            pass.set_pipeline(&self.pipelines.gradient_pipeline);
            pass.set_bind_group(0, &quad_bg, &[]);
            pass.set_bind_group(1, &gradient_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        1
    }

    // ── Blur dispatch (Filter / BackdropFilter) ─────────────────────────

    /// Apply filter effects. Handles Blur via two-pass Gaussian on the output.
    fn render_blur_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        node: &FlatNode,
        filters: &[liquide_compositor::scene::FilterSpec],
    ) -> u32 {
        use liquide_compositor::scene::FilterSpec;

        // Find the first Blur filter (the primary GPU-accelerated one).
        let blur_radius = filters.iter().find_map(|f| match f {
            FilterSpec::Blur { radius } => Some(*radius),
            _ => None,
        });

        let radius = match blur_radius {
            Some(r) if r > 0.0 => r,
            _ => return 0,
        };

        self.apply_blur_passes(encoder, node, radius)
    }

    /// Apply backdrop filter effects (blur behind element).
    fn render_backdrop_blur_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        node: &FlatNode,
        filters: &[liquide_compositor::scene::BackdropFilterSpec],
    ) -> u32 {
        use liquide_compositor::scene::BackdropFilterSpec;

        let blur_radius = filters.iter().find_map(|f| match f {
            BackdropFilterSpec::Blur { radius } => Some(*radius),
            _ => None,
        });

        let radius = match blur_radius {
            Some(r) if r > 0.0 => r,
            _ => return 0,
        };

        self.apply_blur_passes(encoder, node, radius)
    }

    /// Execute a two-pass Gaussian blur: horizontal then vertical.
    ///
    /// Pass 1: copy output → intermediate, blur horizontally → output.
    /// Pass 2: copy output → intermediate, blur vertically → output.
    fn apply_blur_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        _node: &FlatNode,
        radius: f32,
    ) -> u32 {
        let output = match self.output_texture.as_ref() {
            Some(t) => t,
            None => return 0,
        };
        let intermediate = match self.intermediate_texture.as_ref() {
            Some(t) => t,
            None => return 0,
        };

        // Pass 1: horizontal blur — copy output to intermediate, then blur to output.
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &intermediate.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            output.size,
        );

        let blur_h = BlurUniforms {
            direction: [1.0, 0.0],
            radius,
            _pad: 0.0,
        };
        let blur_h_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("blur_h_uniform"),
                contents: bytemuck::bytes_of(&blur_h),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let blur_h_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_h_bg"),
            layout: &self.pipelines.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&intermediate.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_h_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur_h_pass"),
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
            pass.set_pipeline(&self.pipelines.blur_pipeline);
            pass.set_bind_group(0, &blur_h_bg, &[]);
            pass.draw(0..3, 0..1); // fullscreen triangle
        }

        // Pass 2: vertical blur — copy output to intermediate, then blur to output.
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &intermediate.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            output.size,
        );

        let blur_v = BlurUniforms {
            direction: [0.0, 1.0],
            radius,
            _pad: 0.0,
        };
        let blur_v_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("blur_v_uniform"),
                contents: bytemuck::bytes_of(&blur_v),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let blur_v_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_v_bg"),
            layout: &self.pipelines.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&intermediate.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_v_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur_v_pass"),
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
            pass.set_pipeline(&self.pipelines.blur_pipeline);
            pass.set_bind_group(0, &blur_v_bg, &[]);
            pass.draw(0..3, 0..1); // fullscreen triangle
        }

        2 // two draw calls (h + v passes)
    }

    // ── Blend dispatch (RenderLayer) ────────────────────────────────────

    /// Dispatch blend compute shader for a RenderLayer node.
    ///
    /// In a full implementation this would render children to an offscreen
    /// texture and blend it back. Since `FlatNode` is already pre-flattened,
    /// we apply the blend as a post-process over the current output.
    fn render_blend_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        blend_mode: &liquide_compositor::pixel::BlendMode,
    ) -> u32 {
        use liquide_compositor::pixel::BlendMode;
        // SrcOver is the default compositing — no extra work needed.
        if *blend_mode == BlendMode::SrcOver {
            return 0;
        }

        let output = match self.output_texture.as_ref() {
            Some(t) => t,
            None => return 0,
        };
        let intermediate = match self.intermediate_texture.as_ref() {
            Some(t) => t,
            None => return 0,
        };

        // Snapshot current output into intermediate (as "dst" for the blend).
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &intermediate.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            output.size,
        );

        // Create a storage texture view for output (Bgra8Unorm, non-sRGB for storage).
        let blend_out_tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blend_out"),
            size: output.size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let blend_out_view =
            blend_out_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Src = output (what was just rendered), Dst = intermediate (snapshot).
        let src_view = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Bgra8Unorm),
            ..Default::default()
        });
        let dst_view = intermediate.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Bgra8Unorm),
            ..Default::default()
        });

        let blend_uniforms = BlendUniforms {
            mode: blend_mode_to_gpu(blend_mode),
            _pad: [0; 3],
        };
        let blend_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("blend_uniform"),
                contents: bytemuck::bytes_of(&blend_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let blend_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blend_bg"),
            layout: &self.pipelines.blend_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&dst_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&blend_out_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: blend_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("blend_pass"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(&self.pipelines.blend_pipeline);
            pass.set_bind_group(0, &blend_bg, &[]);
            let wg_x = (self.width + 7) / 8;
            let wg_y = (self.height + 7) / 8;
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // Copy the blend result back to the output texture.
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &blend_out_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            output.size,
        );

        1
    }

    // ── Surface blit dispatch (Surface / ChildSurface) ──────────────────

    /// Render a Wayland client surface buffer as a textured quad.
    fn render_surface_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        buffer: &liquide_compositor::scene::SurfaceBuffer,
    ) -> u32 {
        if buffer.width == 0 || buffer.height == 0 || buffer.pixels.is_empty() {
            return 0;
        }

        let bounds = &node.absolute_bounds;

        // Upload the surface pixel data to a temporary GPU texture.
        let tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("surface_tex"),
            size: wgpu::Extent3d {
                width: buffer.width,
                height: buffer.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &buffer.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(buffer.stride),
                rows_per_image: Some(buffer.height),
            },
            wgpu::Extent3d {
                width: buffer.width,
                height: buffer.height,
                depth_or_array_layers: 1,
            },
        );
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Render as a textured quad using the image pipeline.
        let quad_uniforms = QuadUniforms {
            dst_rect: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("surface_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let image_uniforms = ImageUniforms {
            src_rect: [0.0, 0.0, 1.0, 1.0],
            opacity: node.opacity,
            _pad: [0.0; 3],
        };
        let image_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("surface_img_uniform"),
                contents: bytemuck::bytes_of(&image_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surface_quad_bg"),
            layout: &self.pipelines.quad_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: quad_buf.as_entire_binding(),
            }],
        });

        let image_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surface_img_bg"),
            layout: &self.pipelines.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
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
                label: Some("surface_pass"),
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

        // ── Batched buffer allocation ───────────────────────────────
        // Instead of creating 2*N individual buffers (one QuadUniforms +
        // one TextUniforms per glyph), pack all uniforms into two batch
        // buffers with aligned offsets. This reduces GPU buffer churn
        // from O(N) allocations to O(1).

        let align = self.gpu.device.limits().min_uniform_buffer_offset_alignment as usize;
        let quad_stride = align.max(std::mem::size_of::<QuadUniforms>());
        let text_stride = align.max(std::mem::size_of::<TextUniforms>());
        let n = glyph_quads.len();

        let mut quad_data = vec![0u8; quad_stride * n];
        let mut text_data = vec![0u8; text_stride * n];

        for (i, &(gx, gy, entry)) in glyph_quads.iter().enumerate() {
            let qu = QuadUniforms {
                dst_rect: [gx, gy, entry.metrics.width as f32, entry.metrics.height as f32],
                viewport: [self.width as f32, self.height as f32],
                _pad: [0.0; 2],
            };

            let u_min = entry.atlas_x as f32 / atlas_w;
            let v_min = entry.atlas_y as f32 / atlas_h;
            let u_max = (entry.atlas_x + entry.metrics.width) as f32 / atlas_w;
            let v_max = (entry.atlas_y + entry.metrics.height) as f32 / atlas_h;

            let tu = TextUniforms {
                color: color_f,
                src_rect: [u_min, v_min, u_max, v_max],
                opacity: node.opacity,
                _pad: [0.0; 3],
            };

            let q_off = i * quad_stride;
            quad_data[q_off..q_off + std::mem::size_of::<QuadUniforms>()]
                .copy_from_slice(bytemuck::bytes_of(&qu));

            let t_off = i * text_stride;
            text_data[t_off..t_off + std::mem::size_of::<TextUniforms>()]
                .copy_from_slice(bytemuck::bytes_of(&tu));
        }

        let quad_batch_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("text_quad_batch"),
                contents: &quad_data,
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let text_batch_buf = self.gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("text_uniform_batch"),
                contents: &text_data,
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let quad_uniform_size = NonZeroU64::new(std::mem::size_of::<QuadUniforms>() as u64);
        let text_uniform_size = NonZeroU64::new(std::mem::size_of::<TextUniforms>() as u64);

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

            for i in 0..n {
                let q_off = (i * quad_stride) as u64;
                let t_off = (i * text_stride) as u64;

                let quad_bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("quad_bg"),
                    layout: &self.pipelines.quad_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &quad_batch_buf,
                            offset: q_off,
                            size: quad_uniform_size,
                        }),
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
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &text_batch_buf,
                                offset: t_off,
                                size: text_uniform_size,
                            }),
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
        if self.device_lost.load(Ordering::Acquire) {
            return Err(WgpuError::RenderFailed("GPU device lost".into()));
        }

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
        // Dispatch each damage-visible node through the same pipelines
        // as render_frame(). The only difference is the pre-filtered set.
        for node in nodes {
            use liquide_compositor::scene::SceneNodeKind;
            match &node.kind {
                SceneNodeKind::Background { color } => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        color, 0.0,
                    );
                }
                SceneNodeKind::Tint { color } => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        color, 0.0,
                    );
                }
                SceneNodeKind::Glass(params) => {
                    draw_calls += self.render_rect_node(
                        &mut encoder, output, node,
                        &params.tint_color, 0.0,
                    );
                }
                SceneNodeKind::Shadow {
                    spread,
                    blur_radius,
                    color,
                    corner_radius,
                } => {
                    draw_calls += self.render_shadow_node(
                        &mut encoder, output, node,
                        [0.0, 0.0], *blur_radius, *spread, color, *corner_radius, false,
                    );
                }
                SceneNodeKind::BoxShadows { shadows } => {
                    for s in shadows {
                        draw_calls += self.render_shadow_node(
                            &mut encoder, output, node,
                            [s.offset_x, s.offset_y],
                            s.blur_radius, s.spread_radius,
                            &s.color, node.corner_radius.0, s.inset,
                        );
                    }
                }
                SceneNodeKind::GradientFill { gradient } => {
                    draw_calls += self.render_gradient_node(
                        &mut encoder, output, node, gradient,
                    );
                }
                SceneNodeKind::Filter { filters } => {
                    draw_calls += self.render_blur_node(
                        &mut encoder, node, filters,
                    );
                }
                SceneNodeKind::BackdropFilter { filters } => {
                    draw_calls += self.render_backdrop_blur_node(
                        &mut encoder, node, filters,
                    );
                }
                SceneNodeKind::RenderLayer { blend_mode, .. } => {
                    draw_calls += self.render_blend_node(
                        &mut encoder, blend_mode,
                    );
                }
                SceneNodeKind::Surface { buffer, .. }
                | SceneNodeKind::ChildSurface { buffer, .. } => {
                    if let Some(buf) = buffer {
                        draw_calls += self.render_surface_node(
                            &mut encoder, output, node, buf,
                        );
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
                _ => {}
            }
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;
        Ok(draw_calls)
    }
}

// ── Renderer trait implementation ────────────────────────────────────

impl Renderer for WgpuRenderer {
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> RenderResult<Vec<DamageTile>> {
        self.render_to_framebuffer(nodes, fb, damage)
    }

    fn blur_enabled(&self) -> bool {
        // GPU blur is always available when the device is alive.
        !self.is_device_lost()
    }

    fn has_pending_glyphs(&self) -> bool {
        false
    }
}

// Compile-time assertion: WgpuRenderer must be Send for use in render thread.
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _check() {
        _assert_send::<WgpuRenderer>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::BlendMode;
    use liquide_compositor::scene::{GradientSpec, GlassParams};
    use liquide_compositor::{BoxShadowSpec, FilterSpec, BackdropFilterSpec};

    // ── Uniform struct layout tests ─────────────────────────────────────

    #[test]
    fn rect_uniforms_size_and_alignment() {
        // Must be 48 bytes: color(16) + bounds(16) + corner_radius(4) + opacity(4) + pad(8).
        assert_eq!(std::mem::size_of::<RectUniforms>(), 48);
        assert_eq!(std::mem::align_of::<RectUniforms>(), 4);
    }

    #[test]
    fn shadow_uniforms_size_and_alignment() {
        // Must be 64 bytes:
        // bounds(16) + color(16) + offset(8) + blur(4) + spread(4) + radius(4) + inset(4) + pad(8).
        assert_eq!(std::mem::size_of::<ShadowUniforms>(), 64);
    }

    #[test]
    fn gradient_uniforms_size() {
        // kind(4) + angle(4) + center(8) + radius(4) + stop_count(4) + pad(8) = 32
        assert_eq!(std::mem::size_of::<GradientUniforms>(), 32);
    }

    #[test]
    fn gradient_stop_gpu_size() {
        // position(4) + pad(12) + color(16) = 32 bytes, matching WGSL alignment.
        assert_eq!(std::mem::size_of::<GradientStopGpu>(), 32);
    }

    #[test]
    fn blur_uniforms_size() {
        // direction(8) + radius(4) + pad(4) = 16
        assert_eq!(std::mem::size_of::<BlurUniforms>(), 16);
    }

    #[test]
    fn blend_uniforms_size() {
        // mode(4) + pad(12) = 16
        assert_eq!(std::mem::size_of::<BlendUniforms>(), 16);
    }

    // ── Helper function tests ───────────────────────────────────────────

    #[test]
    fn color_to_f32_white() {
        let c = Color::WHITE;
        let f = color_to_f32(&c);
        assert!((f[0] - 1.0).abs() < 0.01);
        assert!((f[1] - 1.0).abs() < 0.01);
        assert!((f[2] - 1.0).abs() < 0.01);
        assert!((f[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn color_to_f32_transparent() {
        let c = Color::TRANSPARENT;
        let f = color_to_f32(&c);
        assert!((f[0]).abs() < 0.01);
        assert!((f[3]).abs() < 0.01);
    }

    #[test]
    fn color_to_f32_half_red() {
        let c = Color::new(128, 0, 0, 255);
        let f = color_to_f32(&c);
        assert!((f[0] - 128.0 / 255.0).abs() < 0.01);
        assert!((f[1]).abs() < 0.01);
        assert!((f[2]).abs() < 0.01);
        assert!((f[3] - 1.0).abs() < 0.01);
    }

    // ── Blend mode mapping tests ────────────────────────────────────────

    #[test]
    fn blend_mode_to_gpu_covers_all_modes() {
        assert_eq!(blend_mode_to_gpu(&BlendMode::SrcOver), 0);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Src), 1);
        assert_eq!(blend_mode_to_gpu(&BlendMode::SrcAtop), 2);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Multiply), 3);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Screen), 4);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Overlay), 5);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Darken), 6);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Lighten), 7);
        assert_eq!(blend_mode_to_gpu(&BlendMode::ColorDodge), 8);
        assert_eq!(blend_mode_to_gpu(&BlendMode::ColorBurn), 9);
        assert_eq!(blend_mode_to_gpu(&BlendMode::HardLight), 10);
        assert_eq!(blend_mode_to_gpu(&BlendMode::SoftLight), 11);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Difference), 12);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Exclusion), 13);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Hue), 14);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Saturation), 15);
        assert_eq!(blend_mode_to_gpu(&BlendMode::ColorBlend), 16);
        assert_eq!(blend_mode_to_gpu(&BlendMode::Luminosity), 17);
    }

    // ── Gradient stop builder tests ─────────────────────────────────────

    #[test]
    fn build_gradient_stops_empty() {
        let stops = build_gradient_stops(&[]);
        assert!(stops.is_empty());
    }

    #[test]
    fn build_gradient_stops_two_colors() {
        let stops = build_gradient_stops(&[
            (0.0, Color::BLACK),
            (1.0, Color::WHITE),
        ]);
        assert_eq!(stops.len(), 2);
        assert!((stops[0].position - 0.0).abs() < f32::EPSILON);
        assert!((stops[1].position - 1.0).abs() < f32::EPSILON);
        // White color check
        assert!((stops[1].color[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn build_gradient_stops_preserves_positions() {
        let stops = build_gradient_stops(&[
            (0.0, Color::BLACK),
            (0.25, Color::new(255, 0, 0, 255)),
            (0.75, Color::new(0, 255, 0, 255)),
            (1.0, Color::WHITE),
        ]);
        assert_eq!(stops.len(), 4);
        assert!((stops[1].position - 0.25).abs() < f32::EPSILON);
        assert!((stops[2].position - 0.75).abs() < f32::EPSILON);
    }

    // ── Image UV rect tests ─────────────────────────────────────────────

    #[test]
    fn image_uv_fill_mode() {
        let uv = compute_image_uv_rect(100, 50, 200.0, 100.0, &ImageFit::Fill);
        assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn image_uv_cover_wider_image() {
        let uv = compute_image_uv_rect(200, 100, 100.0, 100.0, &ImageFit::Cover);
        // Image is wider (aspect 2:1) for a square dst.
        // visible_fraction = dst_aspect / img_aspect = 1.0 / 2.0 = 0.5
        // offset = (1.0 - 0.5) * 0.5 = 0.25
        assert!((uv[0] - 0.25).abs() < 0.01);
        assert!((uv[2] - 0.75).abs() < 0.01);
    }

    #[test]
    fn image_uv_zero_dims_returns_default() {
        let uv = compute_image_uv_rect(0, 0, 100.0, 100.0, &ImageFit::Cover);
        assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    }

    // ── QuadUniforms byte representation ────────────────────────────────

    #[test]
    fn quad_uniforms_bytemuck_roundtrip() {
        let q = QuadUniforms {
            dst_rect: [10.0, 20.0, 300.0, 400.0],
            viewport: [1920.0, 1080.0],
            _pad: [0.0; 2],
        };
        let bytes = bytemuck::bytes_of(&q);
        assert_eq!(bytes.len(), std::mem::size_of::<QuadUniforms>());
        let q2: &QuadUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(q2.dst_rect, q.dst_rect);
        assert_eq!(q2.viewport, q.viewport);
    }

    // ── Shadow parameter extraction tests ───────────────────────────────

    #[test]
    fn box_shadow_spec_inset_flag() {
        let spec = BoxShadowSpec {
            offset_x: 2.0,
            offset_y: 4.0,
            blur_radius: 8.0,
            spread_radius: 1.0,
            color: Color::BLACK,
            inset: true,
        };
        assert!(spec.inset);
        assert!((spec.blur_radius - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn glass_params_default_has_blur() {
        let g = GlassParams::default();
        assert!(g.blur_radius > 0);
    }

    // ── Gradient spec parsing ───────────────────────────────────────────

    #[test]
    fn linear_gradient_angle_calculation() {
        let spec = GradientSpec::Linear {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 1.0,
            end_y: 0.0,
            stops: vec![
                (0.0, Color::BLACK),
                (1.0, Color::WHITE),
            ],
        };
        if let GradientSpec::Linear { start_x, start_y, end_x, end_y, .. } = &spec {
            let dx = end_x - start_x;
            let dy = end_y - start_y;
            let angle = dy.atan2(dx);
            // Horizontal gradient: angle should be 0
            assert!((angle).abs() < 0.01);
        }
    }

    #[test]
    fn radial_gradient_center_extraction() {
        let spec = GradientSpec::Radial {
            center_x: 0.5,
            center_y: 0.5,
            radius: 1.0,
            radius_y: 1.0,
            stops: vec![(0.0, Color::WHITE), (1.0, Color::BLACK)],
        };
        if let GradientSpec::Radial { center_x, center_y, radius, .. } = &spec {
            assert!((*center_x - 0.5).abs() < f32::EPSILON);
            assert!((*center_y - 0.5).abs() < f32::EPSILON);
            assert!((*radius - 1.0).abs() < f32::EPSILON);
        }
    }

    // ── Filter spec extraction tests ────────────────────────────────────

    #[test]
    fn filter_spec_extracts_blur_radius() {
        let filters = vec![
            FilterSpec::Brightness(1.2),
            FilterSpec::Blur { radius: 5.0 },
            FilterSpec::Contrast(0.9),
        ];
        let blur_radius = filters.iter().find_map(|f| match f {
            FilterSpec::Blur { radius } => Some(*radius),
            _ => None,
        });
        assert_eq!(blur_radius, Some(5.0));
    }

    #[test]
    fn backdrop_filter_spec_extracts_blur() {
        let filters = vec![
            BackdropFilterSpec::Brightness(1.0),
            BackdropFilterSpec::Blur { radius: 12.0 },
        ];
        let blur_radius = filters.iter().find_map(|f| match f {
            BackdropFilterSpec::Blur { radius } => Some(*radius),
            _ => None,
        });
        assert_eq!(blur_radius, Some(12.0));
    }
}
