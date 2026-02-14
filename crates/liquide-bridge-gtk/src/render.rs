//! GTK rendering surface.
//!
//! Provides integration with GTK's rendering infrastructure:
//! - **GL backend** via `GtkGLArea` for hardware-accelerated rendering
//! - **Cairo backend** via `GtkDrawingArea` for software rendering
//!
//! The Liquide scene graph is rendered into a framebuffer which is then
//! presented to GTK's compositor via the appropriate backend.

use serde::{Deserialize, Serialize};

/// Which rendering backend to use for the GTK surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderBackend {
    /// Software rendering via Cairo / `GtkDrawingArea`.
    Cairo,
    /// Hardware-accelerated via `GtkGLArea` (OpenGL / EGL).
    OpenGL,
    /// Vulkan rendering via `GtkGLArea` with Vulkan context.
    Vulkan,
}

/// Pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit RGBA, premultiplied alpha.
    Rgba8Premul,
    /// 8-bit BGRA, premultiplied alpha (Cairo native).
    Bgra8Premul,
    /// 8-bit RGB, no alpha.
    Rgb8,
}

/// A rendering surface backed by GTK.
pub struct GtkRenderSurface {
    /// Rendering backend in use.
    backend: RenderBackend,
    /// Surface width in physical pixels.
    width: u32,
    /// Surface height in physical pixels.
    height: u32,
    /// HiDPI scale factor.
    scale_factor: f64,
    /// Pixel format.
    format: PixelFormat,
    /// Whether the surface needs a full repaint.
    needs_full_repaint: bool,
    /// Damage regions (rectangles that need repainting).
    damage_regions: Vec<DamageRect>,
    /// Frame counter.
    frame_count: u64,
}

/// A rectangle marking a damaged (needs-repaint) area.
#[derive(Debug, Clone, Copy)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl GtkRenderSurface {
    /// Create a new render surface.
    #[must_use]
    pub fn new(backend: RenderBackend, width: u32, height: u32, scale_factor: f64) -> Self {
        let format = match backend {
            RenderBackend::Cairo => PixelFormat::Bgra8Premul,
            RenderBackend::OpenGL | RenderBackend::Vulkan => PixelFormat::Rgba8Premul,
        };
        Self {
            backend,
            width,
            height,
            scale_factor,
            format,
            needs_full_repaint: true,
            damage_regions: Vec::new(),
            frame_count: 0,
        }
    }

    /// Resize the surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.needs_full_repaint = true;
            tracing::debug!(width, height, "Surface resized");
        }
    }

    /// Update HiDPI scale factor.
    pub fn set_scale_factor(&mut self, factor: f64) {
        if (self.scale_factor - factor).abs() > f64::EPSILON {
            self.scale_factor = factor;
            self.needs_full_repaint = true;
        }
    }

    /// Mark a region as damaged.
    pub fn add_damage(&mut self, rect: DamageRect) {
        self.damage_regions.push(rect);
    }

    /// Mark the whole surface as damaged.
    pub fn invalidate(&mut self) {
        self.needs_full_repaint = true;
    }

    /// Begin a frame. Returns the damage region to repaint.
    #[must_use]
    pub fn begin_frame(&mut self) -> Vec<DamageRect> {
        self.frame_count += 1;
        if self.needs_full_repaint {
            self.needs_full_repaint = false;
            self.damage_regions.clear();
            vec![DamageRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            }]
        } else {
            std::mem::take(&mut self.damage_regions)
        }
    }

    /// End a frame (submit to GTK for presentation).
    pub fn end_frame(&mut self) {
        tracing::trace!(frame = self.frame_count, "Frame presented");
    }

    /// Logical size (accounts for scale factor).
    #[must_use]
    pub fn logical_size(&self) -> (f64, f64) {
        (
            self.width as f64 / self.scale_factor,
            self.height as f64 / self.scale_factor,
        )
    }

    /// Physical size in pixels.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn backend(&self) -> RenderBackend {
        self.backend
    }

    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_creation() {
        let s = GtkRenderSurface::new(RenderBackend::Cairo, 1920, 1080, 2.0);
        assert_eq!(s.physical_size(), (1920, 1080));
        assert_eq!(s.logical_size(), (960.0, 540.0));
        assert_eq!(s.format(), PixelFormat::Bgra8Premul);
    }

    #[test]
    fn test_damage_tracking() {
        let mut s = GtkRenderSurface::new(RenderBackend::OpenGL, 800, 600, 1.0);
        // First frame: full repaint
        let regions = s.begin_frame();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].width, 800);
        s.end_frame();

        // Second frame: incremental
        s.add_damage(DamageRect {
            x: 10,
            y: 10,
            width: 100,
            height: 50,
        });
        let regions = s.begin_frame();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].x, 10);
    }

    #[test]
    fn test_resize_invalidates() {
        let mut s = GtkRenderSurface::new(RenderBackend::Cairo, 800, 600, 1.0);
        let _ = s.begin_frame(); // consume initial
        s.resize(1024, 768);
        let regions = s.begin_frame();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].width, 1024);
    }
}
