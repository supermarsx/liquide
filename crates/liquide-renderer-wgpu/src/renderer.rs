//! High-level wgpu renderer that processes `FlatNode` lists.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::{
    Color, FrameMemoryKind, PixelFormat, RenderResult, Renderer, RendererBackendInfo,
    RendererBackendKind, RendererCapabilities, RendererNegotiation, RendererNegotiationError,
    RendererRejectReason,
};

use crate::device::{GpuBackend, WgpuDevice};
use crate::pipeline::PipelineCache;
use crate::texture::GpuTexture;
use crate::{Result, WgpuError};

use bytemuck::{Pod, Zeroable};
use liquide_compositor::scene::{
    BackgroundImage, BackgroundSpec, BorderSides, FlatNode, GradientSpec, ImageFit, OutlineSpec,
    SceneNodeKind,
};
use wgpu::util::DeviceExt;

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
///
/// `center` carries the linear-gradient start point (0..1) for `kind == 0`
/// and the radial/conic center for `kind == 1/2`. `line_end` carries the
/// linear-gradient end point (0..1) for `kind == 0` and is unused (zeroed)
/// otherwise.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GradientUniforms {
    kind: u32,
    angle: f32,
    center: [f32; 2],
    radius: f32,
    stop_count: u32,
    line_end: [f32; 2],
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
#[allow(dead_code)] // wired in once the blend-mode dispatch path lands.
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
        ImageFit::Sized { .. } => {
            // Explicit size scales the whole image into the target rect; the
            // destination geometry (handled by the quad) carries the size, so
            // the UV range is the full image.
            [0.0, 0.0, 1.0, 1.0]
        }
    }
}

// ── Scene-kind coverage contract ───────────────────────────────────────

/// Scene-kind support level for a renderer backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneKindSupport {
    /// The backend emits pixels for this kind when the node carries drawable content.
    Rendered,
    /// The kind is a container/marker/no-op for this backend and is safe to skip.
    Structural,
    /// The backend cannot render this kind correctly yet.
    Unsupported,
}

/// CPU-vs-wgpu coverage decision for one scene kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuSceneKindCoverage {
    pub kind: &'static str,
    pub cpu: SceneKindSupport,
    pub wgpu: SceneKindSupport,
    pub reason: &'static str,
}

fn coverage(
    kind: &'static str,
    cpu: SceneKindSupport,
    wgpu: SceneKindSupport,
    reason: &'static str,
) -> WgpuSceneKindCoverage {
    WgpuSceneKindCoverage {
        kind,
        cpu,
        wgpu,
        reason,
    }
}

/// Return the declared CPU-vs-wgpu coverage for a `SceneNodeKind` payload.
///
/// This is intentionally data-sensitive for variants where the wgpu backend
/// can render a strict subset correctly, such as tint-only glass or no-op
/// render-layer markers.
#[must_use]
pub fn scene_kind_coverage(kind: &SceneNodeKind) -> WgpuSceneKindCoverage {
    use SceneKindSupport::{Rendered, Structural, Unsupported};
    use liquide_compositor::pixel::BlendMode;

    match kind {
        SceneNodeKind::Root => coverage("Root", Structural, Structural, "scene root marker"),
        SceneNodeKind::Background { .. } => {
            coverage("Background", Rendered, Rendered, "solid rect pipeline")
        }
        SceneNodeKind::BlurCache => coverage(
            "BlurCache",
            Rendered,
            Unsupported,
            "requires region-scoped cached backdrop blur",
        ),
        SceneNodeKind::Workspace { .. } => {
            coverage("Workspace", Structural, Structural, "container marker")
        }
        SceneNodeKind::Surface { .. } => coverage(
            "Surface",
            Rendered,
            Rendered,
            "surface texture blit pipeline",
        ),
        SceneNodeKind::Shadow { .. } => {
            coverage("Shadow", Rendered, Rendered, "shadow SDF pipeline")
        }
        SceneNodeKind::Decoration { .. } => coverage(
            "Decoration",
            Rendered,
            Unsupported,
            "window chrome requires title/button/vector decoration drawing",
        ),
        SceneNodeKind::ChildSurface { .. } => coverage(
            "ChildSurface",
            Rendered,
            Rendered,
            "surface texture blit pipeline",
        ),
        SceneNodeKind::Overlay => coverage("Overlay", Structural, Structural, "container marker"),
        SceneNodeKind::Glass(params)
            if params.blur_radius == 0 && !params.inner_glow && !params.parallax =>
        {
            coverage(
                "Glass",
                Rendered,
                Rendered,
                "tint-only glass is a solid rect",
            )
        }
        SceneNodeKind::Glass(_) => coverage(
            "Glass",
            Rendered,
            Unsupported,
            "glass blur, inner glow, and parallax need a correct backdrop pass",
        ),
        SceneNodeKind::BlurBackdrop => coverage(
            "BlurBackdrop",
            Rendered,
            Unsupported,
            "requires region-scoped backdrop blur instead of whole-frame blur",
        ),
        SceneNodeKind::Tint { .. } => coverage("Tint", Rendered, Rendered, "solid rect pipeline"),
        SceneNodeKind::Content => coverage("Content", Structural, Structural, "container marker"),
        SceneNodeKind::ShellLayer => {
            coverage("ShellLayer", Structural, Structural, "container marker")
        }
        SceneNodeKind::Cursor { .. } => coverage(
            "Cursor",
            Rendered,
            Unsupported,
            "software cursor shapes need vector or bitmap cursor rendering",
        ),
        SceneNodeKind::Text { .. } => coverage("Text", Rendered, Rendered, "glyph atlas pipeline"),
        SceneNodeKind::Icon { .. } => coverage(
            "Icon",
            Rendered,
            Unsupported,
            "built-in vector icon atlas/path rendering is not wired to wgpu",
        ),
        SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: false,
        } => coverage(
            "RenderLayer",
            Rendered,
            Structural,
            "default SrcOver non-isolated marker has no extra GPU work",
        ),
        SceneNodeKind::RenderLayer { .. } => coverage(
            "RenderLayer",
            Rendered,
            Unsupported,
            "non-default blend/isolation needs pre-children layer snapshots",
        ),
        SceneNodeKind::ClipPath { .. } => coverage(
            "ClipPath",
            Rendered,
            Unsupported,
            "path/shape clipping requires a mask or stencil pass",
        ),
        SceneNodeKind::Filter { filters } if filters.is_empty() => coverage(
            "Filter",
            Structural,
            Structural,
            "empty filter chain is a no-op",
        ),
        SceneNodeKind::Filter { .. } => coverage(
            "Filter",
            Rendered,
            Unsupported,
            "filter chains need offscreen subtree rendering and scoped post-processing",
        ),
        SceneNodeKind::BackdropFilter { filters } if filters.is_empty() => coverage(
            "BackdropFilter",
            Structural,
            Structural,
            "empty backdrop-filter chain is a no-op",
        ),
        SceneNodeKind::BackdropFilter { .. } => coverage(
            "BackdropFilter",
            Rendered,
            Unsupported,
            "backdrop filters need a region-scoped backdrop snapshot",
        ),
        SceneNodeKind::Image { .. } => {
            coverage("Image", Rendered, Rendered, "image texture pipeline")
        }
        SceneNodeKind::GradientFill { gradient } if gradient_is_supported(gradient) => coverage(
            "GradientFill",
            Rendered,
            Rendered,
            "linear/radial/conic gradient pipeline",
        ),
        SceneNodeKind::GradientFill { .. } => coverage(
            "GradientFill",
            Rendered,
            Unsupported,
            "mesh gradients are not implemented on wgpu",
        ),
        SceneNodeKind::SvgPath { .. } => coverage(
            "SvgPath",
            Rendered,
            Unsupported,
            "SVG path tessellation/stroking is not implemented on wgpu",
        ),
        SceneNodeKind::BackgroundFill { background } => background_fill_coverage(background),
        SceneNodeKind::Outline { outline } if outline_is_noop(outline) => coverage(
            "Outline",
            Structural,
            Structural,
            "empty outline is a no-op",
        ),
        SceneNodeKind::Outline { .. } => coverage(
            "Outline",
            Rendered,
            Unsupported,
            "outline stroke styles are not implemented on wgpu",
        ),
        SceneNodeKind::BoxShadows { .. } => {
            coverage("BoxShadows", Rendered, Rendered, "shadow SDF pipeline")
        }
        SceneNodeKind::Mask { .. } => coverage(
            "Mask",
            Rendered,
            Unsupported,
            "mask image/gradient application requires a mask compositing pass",
        ),
        SceneNodeKind::Border { sides, .. } if border_is_noop(sides) => {
            coverage("Border", Structural, Structural, "empty border is a no-op")
        }
        SceneNodeKind::Border { .. } => coverage(
            "Border",
            Rendered,
            Unsupported,
            "per-side border styles and radii are not implemented on wgpu",
        ),
        SceneNodeKind::BorderImage { .. } => coverage(
            "BorderImage",
            Rendered,
            Unsupported,
            "border-image slicing/repeat is not implemented on wgpu",
        ),
        SceneNodeKind::TextCaret { .. } => {
            coverage("TextCaret", Rendered, Rendered, "solid caret rect")
        }
        SceneNodeKind::SelectionOverlay { .. } => coverage(
            "SelectionOverlay",
            Rendered,
            Rendered,
            "filled rect plus simple border rects",
        ),
        SceneNodeKind::LockScreen => coverage(
            "LockScreen",
            Rendered,
            Unsupported,
            "lockscreen blur/dim overlay requires a correct backdrop pass",
        ),
        SceneNodeKind::CrashScreen => coverage(
            "CrashScreen",
            Rendered,
            Rendered,
            "emergency overlay is a solid rect",
        ),
    }
}

/// Return the coverage for a flattened node, including clip/opacity state that
/// lives outside the `SceneNodeKind` payload.
#[must_use]
pub fn flat_node_scene_kind_coverage(node: &FlatNode) -> WgpuSceneKindCoverage {
    use SceneKindSupport::{Rendered, Unsupported};

    if node.clip.is_some() || rounded_clip_radius_is_active(node.clip_radius) {
        return coverage(
            "Clip",
            Rendered,
            Unsupported,
            "flat-node clip rectangles or rounded clip radii are not applied by wgpu",
        );
    }

    match node.kind_ref() {
        SceneNodeKind::Content | SceneNodeKind::Overlay | SceneNodeKind::ShellLayer
            if node.opacity < 1.0 =>
        {
            coverage(
                "LayerOpacity",
                Rendered,
                Unsupported,
                "container opacity needs scoped alpha modulation of existing pixels",
            )
        }
        _ => scene_kind_coverage(node.kind_ref()),
    }
}

/// Collect unsupported scene-kind decisions for a flattened scene.
#[must_use]
pub fn unsupported_wgpu_scene_kind_coverage<'a>(
    nodes: impl IntoIterator<Item = &'a FlatNode>,
) -> Vec<WgpuSceneKindCoverage> {
    let mut unsupported = Vec::new();
    for node in nodes {
        let decision = flat_node_scene_kind_coverage(node);
        if decision.wgpu == SceneKindSupport::Unsupported
            && !unsupported
                .iter()
                .any(|seen: &WgpuSceneKindCoverage| seen.kind == decision.kind)
        {
            unsupported.push(decision);
        }
    }
    unsupported
}

/// Validate that all flattened nodes are safe for the wgpu renderer.
pub fn validate_wgpu_scene_kind_coverage<'a>(
    nodes: impl IntoIterator<Item = &'a FlatNode>,
) -> std::result::Result<(), String> {
    let unsupported = unsupported_wgpu_scene_kind_coverage(nodes);
    if unsupported.is_empty() {
        return Ok(());
    }

    let details = unsupported
        .iter()
        .map(|decision| format!("{} ({})", decision.kind, decision.reason))
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "unsupported SceneNodeKind(s) for wgpu renderer: {details}"
    ))
}

fn gradient_is_supported(gradient: &GradientSpec) -> bool {
    !matches!(gradient, GradientSpec::Mesh { .. })
}

fn background_fill_coverage(background: &BackgroundSpec) -> WgpuSceneKindCoverage {
    use SceneKindSupport::{Rendered, Structural, Unsupported};

    match &background.image {
        None if background.color.is_some() => coverage(
            "BackgroundFill",
            Rendered,
            Rendered,
            "background color maps to solid rect",
        ),
        None => coverage(
            "BackgroundFill",
            Structural,
            Structural,
            "empty background is a no-op",
        ),
        Some(BackgroundImage::Gradient(gradient)) if gradient_is_supported(gradient) => coverage(
            "BackgroundFill",
            Rendered,
            Rendered,
            "background gradient maps to gradient pipeline",
        ),
        Some(BackgroundImage::Gradient(_)) => coverage(
            "BackgroundFill",
            Rendered,
            Unsupported,
            "mesh background gradients are not implemented on wgpu",
        ),
        Some(BackgroundImage::ImageId(_)) | Some(BackgroundImage::Url(_)) => coverage(
            "BackgroundFill",
            Rendered,
            Unsupported,
            "background image sizing/repeat/loading is not implemented on wgpu",
        ),
    }
}

fn border_is_noop(sides: &BorderSides) -> bool {
    use liquide_compositor::scene::BorderSideStyle;

    [&sides.top, &sides.right, &sides.bottom, &sides.left]
        .iter()
        .all(|side| {
            side.width <= 0.0
                || side.color.a == 0
                || matches!(side.style, BorderSideStyle::None | BorderSideStyle::Hidden)
        })
}

fn outline_is_noop(outline: &OutlineSpec) -> bool {
    use liquide_compositor::scene::OutlineStyle;

    outline.width <= 0.0 || outline.color.a == 0 || outline.style == OutlineStyle::None
}

fn rounded_clip_radius_is_active(radius: (f32, f32, f32, f32)) -> bool {
    radius.0 > 0.0 || radius.1 > 0.0 || radius.2 > 0.0 || radius.3 > 0.0
}

fn wgpu_backend_info(backend: GpuBackend, adapter: &str) -> RendererBackendInfo {
    let mut info = RendererBackendInfo::new(RendererBackendKind::Wgpu, format!("wgpu {backend}"));
    info.version = Some(env!("CARGO_PKG_VERSION").to_string());
    if !adapter.is_empty() {
        info.adapter = Some(adapter.to_string());
    }
    info
}

fn wgpu_renderer_capabilities(max_texture_dimension_2d: Option<u32>) -> RendererCapabilities {
    RendererCapabilities {
        frame_memory_kinds: vec![FrameMemoryKind::Cpu],
        pixel_formats: vec![PixelFormat::Bgra8],
        supports_partial_damage: false,
        supports_blur: false,
        supports_skeleton_window: false,
        supports_async_glyphs: false,
        max_framebuffer_width: max_texture_dimension_2d,
        max_framebuffer_height: max_texture_dimension_2d,
    }
}

fn negotiate_wgpu_framebuffer_target(
    fb: &FrameBuffer,
    capabilities: &RendererCapabilities,
) -> RendererNegotiation {
    let memory = FrameMemoryKind::of_framebuffer(fb);
    if !capabilities.supports_frame_memory(memory) {
        return RendererNegotiation::rejected(RendererRejectReason::UnsupportedFrameMemory {
            memory,
        });
    }

    if !capabilities.supports_pixel_format(fb.format) {
        return RendererNegotiation::rejected(RendererRejectReason::UnsupportedPixelFormat {
            format: fb.format,
        });
    }

    let width_too_large = capabilities
        .max_framebuffer_width
        .is_some_and(|max_width| fb.width > max_width);
    let height_too_large = capabilities
        .max_framebuffer_height
        .is_some_and(|max_height| fb.height > max_height);
    if width_too_large || height_too_large {
        return RendererNegotiation::rejected(RendererRejectReason::FramebufferTooLarge {
            width: fb.width,
            height: fb.height,
            max_width: capabilities.max_framebuffer_width,
            max_height: capabilities.max_framebuffer_height,
        });
    }

    RendererNegotiation::accepted()
}

fn negotiate_wgpu_render_request(
    nodes: &[FlatNode],
    fb: &FrameBuffer,
    _damage: &DamageSet,
    unavailable_reason: Option<String>,
    max_texture_dimension_2d: Option<u32>,
) -> RendererNegotiation {
    if let Some(reason) = unavailable_reason {
        return RendererNegotiation::rejected(RendererRejectReason::BackendUnavailable(reason));
    }

    let capabilities = wgpu_renderer_capabilities(max_texture_dimension_2d);
    let framebuffer = negotiate_wgpu_framebuffer_target(fb, &capabilities);
    if !framebuffer.is_accepted() {
        return framebuffer;
    }

    match validate_wgpu_scene_kind_coverage(nodes.iter()) {
        Ok(()) => RendererNegotiation::accepted(),
        Err(reason) => RendererNegotiation::rejected(RendererRejectReason::Other(reason)),
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
    pub async fn with_backend(backend: GpuBackend, width: u32, height: u32) -> Result<Self> {
        let gpu = WgpuDevice::new(Some(backend)).await?;
        Self::new(gpu, width, height)
    }

    /// Resize the output texture.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        self.width = width;
        self.height = height;
        self.output_texture = Some(GpuTexture::new(&self.gpu.device, width, height, "output")?);
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
    pub fn upload_glyph(&mut self, key: GlyphKey, bitmap: &[u8], metrics: &GlyphMetrics) -> bool {
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
    pub fn register_image(&mut self, image_id: u64, pixels: &[u8], width: u32, height: u32) {
        self.texture_cache.upload(
            &self.gpu.device,
            &self.gpu.queue,
            image_id,
            pixels,
            width,
            height,
        );
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

    fn device_unavailable_reason(&self) -> Option<String> {
        self.is_device_lost().then(|| {
            "wgpu device is lost; fall back to another renderer or recreate the device".to_string()
        })
    }

    fn max_texture_dimension_2d(&self) -> Option<u32> {
        Some(self.gpu.device.limits().max_texture_dimension_2d)
    }

    /// Render a frame from the flattened scene graph.
    ///
    /// Returns the number of draw calls issued.
    pub fn render_frame(&mut self, nodes: &[FlatNode]) -> Result<u32> {
        if self.device_lost.load(Ordering::Acquire) {
            return Err(WgpuError::RenderFailed("GPU device lost".into()));
        }

        validate_wgpu_scene_kind_coverage(nodes.iter()).map_err(WgpuError::RenderFailed)?;

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

        // Process each flat node.
        for node in nodes {
            draw_calls += self.render_supported_node(&mut encoder, output, node);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;

        Ok(draw_calls)
    }

    /// Dispatch one already-validated scene node through the wgpu pipelines.
    fn render_supported_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
    ) -> u32 {
        match node.kind_ref() {
            SceneNodeKind::Background { color } => {
                self.render_rect_node(encoder, output, node, color, 0.0)
            }
            SceneNodeKind::Tint { color } => {
                self.render_rect_node(encoder, output, node, color, 0.0)
            }
            SceneNodeKind::Glass(params) => {
                self.render_rect_node(encoder, output, node, &params.tint_color, 0.0)
            }
            SceneNodeKind::Shadow {
                spread,
                blur_radius,
                color,
                corner_radius,
            } => self.render_shadow_node(
                encoder,
                output,
                node,
                [0.0, 0.0],
                *blur_radius,
                *spread,
                color,
                *corner_radius,
                false,
            ),
            SceneNodeKind::BoxShadows { shadows } => shadows
                .iter()
                .map(|shadow| {
                    self.render_shadow_node(
                        encoder,
                        output,
                        node,
                        [shadow.offset_x, shadow.offset_y],
                        shadow.blur_radius,
                        shadow.spread_radius,
                        &shadow.color,
                        node.corner_radius.0,
                        shadow.inset,
                    )
                })
                .sum(),
            SceneNodeKind::GradientFill { gradient } => {
                self.render_gradient_node(encoder, output, node, gradient)
            }
            SceneNodeKind::BackgroundFill { background } => {
                self.render_background_fill_node(encoder, output, node, background)
            }
            SceneNodeKind::Filter { filters } => self.render_blur_node(encoder, node, filters),
            SceneNodeKind::BackdropFilter { filters } => {
                self.render_backdrop_blur_node(encoder, node, filters)
            }
            SceneNodeKind::RenderLayer { blend_mode, .. } => {
                self.render_blend_node(encoder, blend_mode)
            }
            SceneNodeKind::Surface { buffer, .. } | SceneNodeKind::ChildSurface { buffer, .. } => {
                buffer.as_ref().map_or(0, |buf| {
                    self.render_surface_node(encoder, output, node, buf)
                })
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
            } => self.render_text_node(
                encoder,
                output,
                node,
                text,
                color,
                *font_size,
                font_family,
                *font_weight,
                *font_style_italic,
                *scale,
            ),
            SceneNodeKind::Image {
                image_id,
                width: img_w,
                height: img_h,
                fit,
            } => self.render_image_node(encoder, output, node, *image_id, *img_w, *img_h, fit),
            SceneNodeKind::TextCaret { color, width } => {
                self.render_text_caret_node(encoder, output, node, color, *width)
            }
            SceneNodeKind::SelectionOverlay {
                fill,
                border_color,
                border_width,
            } => self.render_selection_overlay_node(
                encoder,
                output,
                node,
                fill,
                border_color,
                *border_width,
            ),
            SceneNodeKind::CrashScreen => self.render_solid_rect(
                encoder,
                output,
                &node.absolute_bounds,
                &Color::new(180, 0, 0, 200),
                node.opacity,
                0.0,
            ),
            SceneNodeKind::Root
            | SceneNodeKind::BlurCache
            | SceneNodeKind::Workspace { .. }
            | SceneNodeKind::Decoration { .. }
            | SceneNodeKind::Overlay
            | SceneNodeKind::BlurBackdrop
            | SceneNodeKind::Content
            | SceneNodeKind::ShellLayer
            | SceneNodeKind::Cursor { .. }
            | SceneNodeKind::Icon { .. }
            | SceneNodeKind::ClipPath { .. }
            | SceneNodeKind::SvgPath { .. }
            | SceneNodeKind::Outline { .. }
            | SceneNodeKind::Mask { .. }
            | SceneNodeKind::Border { .. }
            | SceneNodeKind::BorderImage { .. }
            | SceneNodeKind::LockScreen => 0,
        }
    }

    fn render_background_fill_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        background: &BackgroundSpec,
    ) -> u32 {
        let mut draw_calls = 0;
        if let Some(color) = &background.color {
            draw_calls += self.render_rect_node(encoder, output, node, color, 0.0);
        }
        if let Some(BackgroundImage::Gradient(gradient)) = &background.image {
            draw_calls += self.render_gradient_node(encoder, output, node, gradient);
        }
        draw_calls
    }

    fn render_text_caret_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        color: &Color,
        width: f32,
    ) -> u32 {
        if width <= 0.0 || color.a == 0 {
            return 0;
        }
        let bounds = Rect::new(
            node.absolute_bounds.x,
            node.absolute_bounds.y,
            width.min(node.absolute_bounds.width),
            node.absolute_bounds.height,
        );
        self.render_solid_rect(encoder, output, &bounds, color, node.opacity, 0.0)
    }

    fn render_selection_overlay_node(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        node: &FlatNode,
        fill: &Color,
        border_color: &Color,
        border_width: f32,
    ) -> u32 {
        let mut draw_calls = 0;
        if fill.a > 0 {
            draw_calls += self.render_solid_rect(
                encoder,
                output,
                &node.absolute_bounds,
                fill,
                node.opacity,
                0.0,
            );
        }
        if border_width > 0.0 && border_color.a > 0 {
            draw_calls += self.render_rect_stroke(
                encoder,
                output,
                &node.absolute_bounds,
                border_width,
                border_color,
                node.opacity,
            );
        }
        draw_calls
    }

    fn render_rect_stroke(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        bounds: &Rect,
        width: f32,
        color: &Color,
        opacity: f32,
    ) -> u32 {
        if bounds.width <= 0.0 || bounds.height <= 0.0 || width <= 0.0 {
            return 0;
        }

        let stroke = width.min(bounds.width * 0.5).min(bounds.height * 0.5);
        let top = Rect::new(bounds.x, bounds.y, bounds.width, stroke);
        let bottom = Rect::new(
            bounds.x,
            bounds.y + bounds.height - stroke,
            bounds.width,
            stroke,
        );
        let left = Rect::new(
            bounds.x,
            bounds.y + stroke,
            stroke,
            bounds.height - stroke * 2.0,
        );
        let right = Rect::new(
            bounds.x + bounds.width - stroke,
            bounds.y + stroke,
            stroke,
            bounds.height - stroke * 2.0,
        );

        [&top, &bottom, &left, &right]
            .iter()
            .map(|rect| self.render_solid_rect(encoder, output, rect, color, opacity, 0.0))
            .sum()
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
        let cr = if corner_radius > 0.0 {
            corner_radius
        } else {
            node.corner_radius.0
        };
        self.render_solid_rect(encoder, output, bounds, color, node.opacity, cr)
    }

    fn render_solid_rect(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &GpuTexture,
        bounds: &Rect,
        color: &Color,
        opacity: f32,
        corner_radius: f32,
    ) -> u32 {
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return 0;
        }

        let quad_uniforms = QuadUniforms {
            dst_rect: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let rect_uniforms = RectUniforms {
            color: color_to_f32(color),
            bounds: [0.0, 0.0, bounds.width, bounds.height],
            corner_radius,
            opacity,
            _pad: [0.0; 2],
        };
        let rect_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_uniform"),
                contents: bytemuck::bytes_of(&rect_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let quad_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rect_quad_bg"),
                layout: &self.pipelines.quad_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quad_buf.as_entire_binding(),
                }],
            });

        let rect_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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
        let quad_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shadow_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

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
        let shadow_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shadow_uniform"),
                contents: bytemuck::bytes_of(&shadow_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let quad_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow_quad_bg"),
                layout: &self.pipelines.quad_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quad_buf.as_entire_binding(),
                }],
            });

        let shadow_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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

        let (kind, angle, center, radius, line_end, stops) = match gradient {
            GradientSpec::Linear {
                start_x,
                start_y,
                end_x,
                end_y,
                stops,
                ..
            } => {
                let dx = end_x - start_x;
                let dy = end_y - start_y;
                let angle = dy.atan2(dx);
                (
                    0u32,
                    angle,
                    [*start_x, *start_y],
                    1.0f32,
                    [*end_x, *end_y],
                    stops.as_slice(),
                )
            }
            GradientSpec::Radial {
                center_x,
                center_y,
                radius,
                stops,
                ..
            } => (
                1u32,
                0.0,
                [*center_x, *center_y],
                *radius,
                [0.0, 0.0],
                stops.as_slice(),
            ),
            GradientSpec::Conic {
                center_x,
                center_y,
                start_angle,
                stops,
                ..
            } => (
                2u32,
                *start_angle,
                [*center_x, *center_y],
                1.0,
                [0.0, 0.0],
                stops.as_slice(),
            ),
            GradientSpec::Mesh { .. } => {
                return 0;
            }
        };

        let gpu_stops = build_gradient_stops(stops);
        // Keep in sync with the shader's `stops` storage-buffer sizing. 256 is
        // well above anything real CSS emits and avoids clipping on complex
        // radial/conic gradients built from parsed keyword stops.
        let stop_count = gpu_stops.len().min(256) as u32;

        let quad_uniforms = QuadUniforms {
            dst_rect: [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gradient_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let gradient_uniforms = GradientUniforms {
            kind,
            angle,
            center,
            radius,
            stop_count,
            line_end,
        };
        let gradient_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gradient_uniform"),
                contents: bytemuck::bytes_of(&gradient_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

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
        let stops_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gradient_stops"),
                contents: bytemuck::cast_slice(&stops_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let quad_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gradient_quad_bg"),
                layout: &self.pipelines.quad_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quad_buf.as_entire_binding(),
                }],
            });

        let gradient_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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
        let blur_h_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("blur_h_uniform"),
                contents: bytemuck::bytes_of(&blur_h),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let blur_h_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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
        let blur_v_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("blur_v_uniform"),
                contents: bytemuck::bytes_of(&blur_v),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let blur_v_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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
    /// texture and blend it back. With the current flat scene the marker
    /// arrives after the children are already drawn, so we cannot produce a
    /// correct backdrop snapshot here — see note below.
    fn render_blend_node(
        &self,
        _encoder: &mut wgpu::CommandEncoder,
        blend_mode: &liquide_compositor::pixel::BlendMode,
    ) -> u32 {
        use liquide_compositor::pixel::BlendMode;
        // SrcOver is the default compositing — no extra work needed.
        if *blend_mode == BlendMode::SrcOver {
            return 0;
        }

        // Correctly applying a non-SrcOver RenderLayer blend requires a
        // backdrop snapshot taken BEFORE the layer's children are drawn.
        // The flattened FlatNode stream currently emits only a single marker
        // (no push/pop pair), so there is no pre-children snapshot to blend
        // against. The previous implementation copied the post-children
        // output and blended it against itself, which produced visibly
        // garbled output (see t8 §3.7). Until the scene flattener emits
        // explicit RenderLayer push/pop markers we warn once per mode and
        // skip the blend rather than corrupt the framebuffer.
        tracing::warn!(
            mode = ?blend_mode,
            "wgpu renderer: non-SrcOver RenderLayer blend skipped — \
             scene flattener lacks pre-children backdrop snapshot (t8 §3.7 follow-up)"
        );
        0
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
        let quad_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("surface_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let image_uniforms = ImageUniforms {
            src_rect: [0.0, 0.0, 1.0, 1.0],
            opacity: node.opacity,
            _pad: [0.0; 3],
        };
        let image_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("surface_img_uniform"),
                contents: bytemuck::bytes_of(&image_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let quad_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("surface_quad_bg"),
                layout: &self.pipelines.quad_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quad_buf.as_entire_binding(),
                }],
            });

        let image_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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
                    glyph_quads.push((
                        pen_x + entry.metrics.bearing_x,
                        pen_y - entry.metrics.bearing_y,
                        entry,
                    ));
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
                dst_rect: [
                    gx,
                    gy,
                    entry.metrics.width as f32,
                    entry.metrics.height as f32,
                ],
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

        let quad_batch_buf =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text_quad_batch"),
                    contents: &quad_data,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let text_batch_buf =
            self.gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text_uniform_batch"),
                    contents: &text_data,
                    usage: wgpu::BufferUsages::UNIFORM,
                });

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

                let quad_bg = self
                    .gpu
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
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

                let text_bg = self
                    .gpu
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("text_bg"),
                        layout: &self.pipelines.text_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.glyph_atlas.view,
                                ),
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
            ImageFit::Sized { width, height } => {
                // Explicit size (CSS background-size: <w> <h>) anchored at the
                // node's top-left.
                (bounds.x, bounds.y, *width, *height)
            }
            _ => (bounds.x, bounds.y, bounds.width, bounds.height),
        };

        let quad_uniforms = QuadUniforms {
            dst_rect: [dst_x, dst_y, dst_w, dst_h],
            viewport: [self.width as f32, self.height as f32],
            _pad: [0.0; 2],
        };
        let quad_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("img_quad_uniform"),
                contents: bytemuck::bytes_of(&quad_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let image_uniforms = ImageUniforms {
            src_rect: uv_rect,
            opacity: node.opacity,
            _pad: [0.0; 3],
        };
        let image_buf = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("img_uniform"),
                contents: bytemuck::bytes_of(&image_uniforms),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let quad_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("img_quad_bg"),
                layout: &self.pipelines.quad_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: quad_buf.as_entire_binding(),
                }],
            });

        let image_bg = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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

    /// Classify scene node kind to damage class for tile encoding.
    ///
    /// This mapping MUST match `liquide-renderer-cpu` for cross-renderer parity.
    fn classify_node_kind(kind: &SceneNodeKind) -> Option<DamageClass> {
        match kind {
            SceneNodeKind::Cursor { .. } => Some(DamageClass::CursorOnly),
            SceneNodeKind::Text { .. } | SceneNodeKind::TextCaret { .. } => {
                Some(DamageClass::TextGlyph)
            }
            SceneNodeKind::Surface { .. }
            | SceneNodeKind::ChildSurface { .. }
            | SceneNodeKind::Image { .. }
            | SceneNodeKind::BlurCache => Some(DamageClass::BitmapRegion),
            SceneNodeKind::Root
            | SceneNodeKind::Workspace { .. }
            | SceneNodeKind::Overlay
            | SceneNodeKind::Content
            | SceneNodeKind::ShellLayer
            | SceneNodeKind::RenderLayer { .. }
            | SceneNodeKind::ClipPath { .. }
            | SceneNodeKind::Filter { .. }
            | SceneNodeKind::BackdropFilter { .. } => None,
            _ => Some(DamageClass::UiPrimitive),
        }
    }

    /// Classify damage tiles according to scene node content.
    ///
    /// This logic MUST match the CPU renderer's classification for
    /// cross-renderer damage parity. Session tile encoding relies on
    /// these classifications to apply appropriate compression strategies.
    fn classify_damage_tiles(
        &self,
        nodes: &[FlatNode],
        damage: &DamageSet,
        width: u32,
        height: u32,
    ) -> Vec<DamageTile> {
        if damage.is_empty() {
            return Vec::new();
        }

        let expanded_damage_tiles = if damage.is_full() {
            damage.materialize_tiles()
        } else {
            damage.tiles.clone()
        };

        use std::collections::HashMap;
        let mut damage_tiles: HashMap<(u32, u32), DamageClass> =
            HashMap::with_capacity(expanded_damage_tiles.len());
        for tile in &expanded_damage_tiles {
            damage_tiles
                .entry((tile.x, tile.y))
                .and_modify(|existing| {
                    if tile.class.priority() < existing.priority() {
                        *existing = tile.class;
                    }
                })
                .or_insert(tile.class);
        }

        let mut classified: HashMap<(u32, u32), DamageClass> =
            HashMap::with_capacity(damage_tiles.len());

        let fb_bounds = Rect::new(0.0, 0.0, width as f32, height as f32);
        let tile_size = damage.tile_size as f32;
        let max_tx = width.div_ceil(damage.tile_size);
        let max_ty = height.div_ceil(damage.tile_size);

        for node in nodes {
            let Some(node_class) = Self::classify_node_kind(node.kind_ref()) else {
                continue;
            };

            let clipped_bounds = node
                .clip
                .as_ref()
                .map_or(Some(node.absolute_bounds), |clip| {
                    node.absolute_bounds.intersection(clip)
                })
                .and_then(|bounds| bounds.intersection(&fb_bounds));

            let Some(bounds) = clipped_bounds else {
                continue;
            };

            let tx_start = (bounds.x.max(0.0) / tile_size).floor() as u32;
            let ty_start = (bounds.y.max(0.0) / tile_size).floor() as u32;
            let tx_end = (bounds.right().max(0.0) / tile_size).ceil() as u32;
            let ty_end = (bounds.bottom().max(0.0) / tile_size).ceil() as u32;

            for ty in ty_start..ty_end.min(max_ty) {
                for tx in tx_start..tx_end.min(max_tx) {
                    if damage_tiles.contains_key(&(tx, ty)) {
                        classified
                            .entry((tx, ty))
                            .and_modify(|existing| {
                                if node_class.priority() < existing.priority() {
                                    *existing = node_class;
                                }
                            })
                            .or_insert(node_class);
                    }
                }
            }
        }

        // Fallback to original classification for tiles without explicit classification
        for (&coords, &fallback_class) in &damage_tiles {
            classified.entry(coords).or_insert(fallback_class);
        }

        let mut tiles: Vec<DamageTile> = classified
            .into_iter()
            .map(|((x, y), class)| DamageTile { x, y, class })
            .collect();
        tiles.sort_by_key(|tile| (tile.class.priority(), tile.y, tile.x));
        tiles
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
        if let Some(reason) = self
            .negotiate_render(nodes, fb, damage)
            .reject_reason()
            .cloned()
        {
            return Err(Box::new(RendererNegotiationError {
                backend: self.backend_info(),
                reason,
            }));
        }

        if fb.width != self.width || fb.height != self.height {
            self.resize(fb.width, fb.height)?;
        }

        if damage.is_empty() {
            return Ok(Vec::new());
        }

        let promoted_damage;
        let render_damage = if damage.is_full() {
            damage
        } else {
            promoted_damage = promote_partial_damage_to_full_frame(damage, fb.width, fb.height);
            &promoted_damage
        };

        let _draw_calls = self.render_frame_with_damage(nodes, render_damage)?;

        let pixels = self.read_back()?;
        let fb_pixels = fb.pixels_mut().expect("CPU framebuffer required");
        let copy_len = fb_pixels.len().min(pixels.len());
        fb_pixels[..copy_len].copy_from_slice(&pixels[..copy_len]);

        // Classify damage tiles according to scene content for session tile encoding
        Ok(self.classify_damage_tiles(nodes, render_damage, fb.width, fb.height))
    }

    /// Render only the nodes intersecting damaged regions.
    pub fn render_frame_with_damage(
        &mut self,
        nodes: &[FlatNode],
        damage: &DamageSet,
    ) -> Result<u32> {
        if damage.is_empty() {
            return Ok(0);
        }

        validate_wgpu_scene_kind_coverage(nodes.iter()).map_err(WgpuError::RenderFailed)?;

        let ts = damage.tile_size as f32;
        let padding = 32.0_f32;
        let (dx0, dy0, dx1, dy1) = if let Some((grid_width, grid_height, _)) =
            damage.full_grid_dimensions()
        {
            (
                -padding,
                -padding,
                grid_width as f32 * ts + padding,
                grid_height as f32 * ts + padding,
            )
        } else {
            (
                damage.tiles.iter().map(|t| t.x).min().unwrap_or(0) as f32 * ts - padding,
                damage.tiles.iter().map(|t| t.y).min().unwrap_or(0) as f32 * ts - padding,
                (damage.tiles.iter().map(|t| t.x).max().unwrap_or(0) as f32 + 1.0) * ts + padding,
                (damage.tiles.iter().map(|t| t.y).max().unwrap_or(0) as f32 + 1.0) * ts + padding,
            )
        };

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

        validate_wgpu_scene_kind_coverage(nodes.iter().copied())
            .map_err(WgpuError::RenderFailed)?;

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
            draw_calls += self.render_supported_node(&mut encoder, output, node);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.frame_count += 1;
        Ok(draw_calls)
    }
}

fn promote_partial_damage_to_full_frame(damage: &DamageSet, width: u32, height: u32) -> DamageSet {
    let class = damage
        .tiles
        .iter()
        .map(|tile| tile.class)
        .min_by_key(DamageClass::priority)
        .unwrap_or(DamageClass::UiPrimitive);
    let tile_size = damage.tile_size.max(1);
    DamageSet::full(
        damage.tile_size,
        width.div_ceil(tile_size),
        height.div_ceil(tile_size),
        class,
    )
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

    fn backend_info(&self) -> RendererBackendInfo {
        wgpu_backend_info(self.gpu.backend, &self.gpu.device_name)
    }

    fn capabilities(&self) -> RendererCapabilities {
        wgpu_renderer_capabilities(self.max_texture_dimension_2d())
    }

    fn negotiate_render(
        &self,
        nodes: &[FlatNode],
        fb: &FrameBuffer,
        damage: &DamageSet,
    ) -> RendererNegotiation {
        negotiate_wgpu_render_request(
            nodes,
            fb,
            damage,
            self.device_unavailable_reason(),
            self.max_texture_dimension_2d(),
        )
    }

    fn blur_enabled(&self) -> bool {
        false
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
    use std::sync::Arc;

    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::BlendMode;
    use liquide_compositor::scene::{
        BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec, ClipPathKind,
        CursorShape, GlassParams, GradientSpec, MaskMode, MaskSpec, SceneNodeKind,
    };
    use liquide_compositor::{BackdropFilterSpec, BoxShadowSpec, FilterSpec, FrameMemory};

    fn test_gradient() -> GradientSpec {
        GradientSpec::Linear {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 1.0,
            end_y: 1.0,
            stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
            repeating: false,
        }
    }

    fn background_with_image(image: Option<BackgroundImage>) -> BackgroundSpec {
        BackgroundSpec {
            color: Some(Color::new(10, 20, 30, 255)),
            image,
            size: BackgroundSize::Auto,
            position: (0.0, 0.0),
            repeat: BackgroundRepeat::NoRepeat,
        }
    }

    fn flat_node(kind: SceneNodeKind) -> FlatNode {
        FlatNode {
            id: 1,
            kind: Arc::new(kind),
            absolute_bounds: Rect::new(0.0, 0.0, 32.0, 32.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    fn test_damage() -> DamageSet {
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        });
        damage
    }

    // ── Uniform struct layout tests ─────────────────────────────────────

    #[test]
    fn wgpu_backend_info_reports_backend_kind_version_and_adapter() {
        let info = wgpu_backend_info(GpuBackend::Vulkan, "Synthetic Adapter");

        assert_eq!(info.kind, RendererBackendKind::Wgpu);
        assert_eq!(info.name, "wgpu Vulkan");
        assert_eq!(info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        assert_eq!(info.adapter.as_deref(), Some("Synthetic Adapter"));
    }

    #[test]
    fn wgpu_capabilities_report_current_cpu_readback_contract() {
        let capabilities = wgpu_renderer_capabilities(Some(4096));

        assert_eq!(capabilities.frame_memory_kinds, vec![FrameMemoryKind::Cpu]);
        assert_eq!(capabilities.pixel_formats, vec![PixelFormat::Bgra8]);
        assert!(!capabilities.supports_frame_memory(FrameMemoryKind::Gpu));
        assert!(!capabilities.supports_frame_memory(FrameMemoryKind::DmaBuf));
        assert!(!capabilities.supports_pixel_format(PixelFormat::Rgba8));
        assert!(!capabilities.supports_partial_damage);
        assert!(!capabilities.supports_blur);
        assert_eq!(capabilities.max_framebuffer_width, Some(4096));
        assert_eq!(capabilities.max_framebuffer_height, Some(4096));
    }

    #[test]
    fn negotiate_wgpu_render_request_accepts_supported_cpu_bgra_scene() {
        let nodes = vec![flat_node(SceneNodeKind::Background {
            color: Color::BLACK,
        })];
        let fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);

        let negotiation = negotiate_wgpu_render_request(&nodes, &fb, &test_damage(), None, None);

        assert!(negotiation.is_accepted());
    }

    #[test]
    fn negotiate_wgpu_render_request_rejects_unsupported_filters() {
        let nodes = vec![flat_node(SceneNodeKind::Filter {
            filters: vec![FilterSpec::Brightness(1.2)],
        })];
        let fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);

        let negotiation = negotiate_wgpu_render_request(&nodes, &fb, &test_damage(), None, None);

        let Some(RendererRejectReason::Other(reason)) = negotiation.reject_reason() else {
            panic!("expected unsupported scene coverage rejection, got {negotiation:?}");
        };
        assert!(reason.contains("Filter"));
        assert!(reason.contains("filter chains need offscreen subtree rendering"));
    }

    #[test]
    fn negotiate_wgpu_render_request_rejects_gpu_framebuffer_targets() {
        let nodes = vec![flat_node(SceneNodeKind::Background {
            color: Color::BLACK,
        })];
        let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
        fb.memory = FrameMemory::Gpu {
            handle: 7,
            dmabuf_fd: -1,
            width: 32,
            height: 32,
        };

        let negotiation = negotiate_wgpu_render_request(&nodes, &fb, &test_damage(), None, None);

        assert_eq!(
            negotiation.reject_reason(),
            Some(&RendererRejectReason::UnsupportedFrameMemory {
                memory: FrameMemoryKind::Gpu,
            })
        );
    }

    #[test]
    fn negotiate_wgpu_render_request_rejects_non_bgra_framebuffers() {
        let nodes = vec![flat_node(SceneNodeKind::Background {
            color: Color::BLACK,
        })];
        let fb = FrameBuffer::new(32, 32, PixelFormat::Rgba8);

        let negotiation = negotiate_wgpu_render_request(&nodes, &fb, &test_damage(), None, None);

        assert_eq!(
            negotiation.reject_reason(),
            Some(&RendererRejectReason::UnsupportedPixelFormat {
                format: PixelFormat::Rgba8,
            })
        );
    }

    #[test]
    fn negotiate_wgpu_render_request_rejects_unavailable_device_with_fallback_hint() {
        let nodes = vec![flat_node(SceneNodeKind::Background {
            color: Color::BLACK,
        })];
        let fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);

        let negotiation = negotiate_wgpu_render_request(
            &nodes,
            &fb,
            &test_damage(),
            Some("wgpu device is lost; fall back to another renderer".to_string()),
            None,
        );

        let Some(RendererRejectReason::BackendUnavailable(reason)) = negotiation.reject_reason()
        else {
            panic!("expected unavailable-device rejection, got {negotiation:?}");
        };
        assert!(reason.contains("fall back"));
    }

    #[test]
    fn scene_kind_coverage_allows_safe_wgpu_subsets() {
        let tint_only_glass = SceneNodeKind::Glass(GlassParams {
            blur_radius: 0,
            tint_color: Color::new(255, 255, 255, 32),
            inner_glow: false,
            parallax: false,
        });
        assert_eq!(
            scene_kind_coverage(&tint_only_glass).wgpu,
            SceneKindSupport::Rendered
        );

        let gradient_background = SceneNodeKind::BackgroundFill {
            background: background_with_image(Some(BackgroundImage::Gradient(test_gradient()))),
        };
        assert_eq!(
            scene_kind_coverage(&gradient_background).wgpu,
            SceneKindSupport::Rendered
        );

        let no_op_layer = SceneNodeKind::RenderLayer {
            blend_mode: BlendMode::SrcOver,
            isolate: false,
        };
        assert_eq!(
            scene_kind_coverage(&no_op_layer).wgpu,
            SceneKindSupport::Structural
        );
    }

    #[test]
    fn scene_kind_coverage_rejects_partial_or_unimplemented_kinds() {
        let unsupported = vec![
            SceneNodeKind::Glass(GlassParams::default()),
            SceneNodeKind::Filter {
                filters: vec![FilterSpec::Brightness(1.2)],
            },
            SceneNodeKind::BackdropFilter {
                filters: vec![BackdropFilterSpec::Blur { radius: 8.0 }],
            },
            SceneNodeKind::RenderLayer {
                blend_mode: BlendMode::Multiply,
                isolate: false,
            },
            SceneNodeKind::Cursor {
                shape: CursorShape::Arrow,
            },
            SceneNodeKind::Icon {
                icon_id: 7,
                color: Color::WHITE,
            },
            SceneNodeKind::Mask {
                mask: MaskSpec::Gradient {
                    gradient: test_gradient(),
                    mode: MaskMode::Alpha,
                },
            },
            SceneNodeKind::ClipPath {
                clip_kind: ClipPathKind::RoundedRect { corner_radius: 4.0 },
            },
            SceneNodeKind::SvgPath {
                d: "M0 0 L1 1".to_string(),
                fill: Some(Color::WHITE),
                stroke: Color::BLACK,
                stroke_width: 1.0,
            },
        ];

        for kind in unsupported {
            assert_eq!(
                scene_kind_coverage(&kind).wgpu,
                SceneKindSupport::Unsupported,
                "{kind:?} should be explicitly unsupported on wgpu"
            );
        }
    }

    #[test]
    fn flat_node_coverage_rejects_clip_and_container_opacity() {
        let mut clipped = flat_node(SceneNodeKind::Background {
            color: Color::WHITE,
        });
        clipped.clip = Some(Rect::new(4.0, 4.0, 8.0, 8.0));
        let clipped_coverage = flat_node_scene_kind_coverage(&clipped);
        assert_eq!(clipped_coverage.kind, "Clip");
        assert_eq!(clipped_coverage.wgpu, SceneKindSupport::Unsupported);

        let mut translucent_layer = flat_node(SceneNodeKind::Content);
        translucent_layer.opacity = 0.5;
        let layer_coverage = flat_node_scene_kind_coverage(&translucent_layer);
        assert_eq!(layer_coverage.kind, "LayerOpacity");
        assert_eq!(layer_coverage.wgpu, SceneKindSupport::Unsupported);
    }

    #[test]
    fn validate_wgpu_scene_kind_coverage_reports_unsupported_names() {
        let nodes = vec![
            flat_node(SceneNodeKind::Background {
                color: Color::BLACK,
            }),
            flat_node(SceneNodeKind::Cursor {
                shape: CursorShape::Arrow,
            }),
            flat_node(SceneNodeKind::Filter {
                filters: vec![FilterSpec::Contrast(1.2)],
            }),
        ];

        let err = validate_wgpu_scene_kind_coverage(nodes.iter()).unwrap_err();
        assert!(err.contains("Cursor"));
        assert!(err.contains("Filter"));
        assert!(err.contains("unsupported SceneNodeKind"));
    }

    #[test]
    fn mesh_gradient_and_background_images_are_unsupported_on_wgpu() {
        let mesh = SceneNodeKind::GradientFill {
            gradient: GradientSpec::Mesh {
                rows: 2,
                cols: 2,
                colors: vec![Color::WHITE; 4],
            },
        };
        assert_eq!(
            scene_kind_coverage(&mesh).wgpu,
            SceneKindSupport::Unsupported
        );

        let image_background = SceneNodeKind::BackgroundFill {
            background: background_with_image(Some(BackgroundImage::ImageId(9))),
        };
        assert_eq!(
            scene_kind_coverage(&image_background).wgpu,
            SceneKindSupport::Unsupported
        );
    }

    #[test]
    fn rect_uniforms_size_and_alignment() {
        // Must be 48 bytes: color(16) + bounds(16) + corner_radius(4) + opacity(4) + pad(8).
        assert_eq!(std::mem::size_of::<RectUniforms>(), 48);
        assert_eq!(std::mem::align_of::<RectUniforms>(), 4);
    }

    #[test]
    fn partial_damage_promotes_to_full_frame_for_cpu_readback() {
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 1,
            y: 0,
            class: DamageClass::TextGlyph,
        });

        let promoted = promote_partial_damage_to_full_frame(&damage, 128, 96);

        assert!(promoted.is_full());
        assert_eq!(
            promoted.full_grid_dimensions(),
            Some((2, 2, DamageClass::TextGlyph))
        );
        assert_eq!(promoted.materialize_tiles().len(), 4);
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
        let stops = build_gradient_stops(&[(0.0, Color::BLACK), (1.0, Color::WHITE)]);
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
            stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
            repeating: false,
        };
        if let GradientSpec::Linear {
            start_x,
            start_y,
            end_x,
            end_y,
            ..
        } = &spec
        {
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
            repeating: false,
        };
        if let GradientSpec::Radial {
            center_x,
            center_y,
            radius,
            ..
        } = &spec
        {
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
