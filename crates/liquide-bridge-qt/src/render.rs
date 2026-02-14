//! Qt rendering surface.
//!
//! Provides integration with Qt's rendering infrastructure:
//! - `QRhiWidget` for Qt 6.7+ hardware-accelerated rendering
//! - `QOpenGLWidget` for OpenGL rendering
//! - `QPainter` for software rendering

use serde::{Deserialize, Serialize};

/// Qt render backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtRenderBackend {
    /// Software rendering via `QPainter / QImage`.
    Software,
    /// OpenGL via `QOpenGLWidget`.
    OpenGL,
    /// RHI (Qt Rendering Hardware Interface) – Vulkan/Metal/D3D12.
    Rhi,
}

/// A render surface backed by Qt.
pub struct QtRenderSurface {
    backend: QtRenderBackend,
    width: u32,
    height: u32,
    device_pixel_ratio: f64,
    needs_full_repaint: bool,
    damage_rects: Vec<[u32; 4]>,
    frame_count: u64,
}

impl QtRenderSurface {
    #[must_use]
    pub fn new(backend: QtRenderBackend, width: u32, height: u32, dpr: f64) -> Self {
        Self {
            backend,
            width,
            height,
            device_pixel_ratio: dpr,
            needs_full_repaint: true,
            damage_rects: Vec::new(),
            frame_count: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.needs_full_repaint = true;
        }
    }

    pub fn set_device_pixel_ratio(&mut self, dpr: f64) {
        if (self.device_pixel_ratio - dpr).abs() > f64::EPSILON {
            self.device_pixel_ratio = dpr;
            self.needs_full_repaint = true;
        }
    }

    pub fn add_damage(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.damage_rects.push([x, y, w, h]);
    }

    pub fn invalidate(&mut self) {
        self.needs_full_repaint = true;
    }

    pub fn begin_frame(&mut self) -> Vec<[u32; 4]> {
        self.frame_count += 1;
        if self.needs_full_repaint {
            self.needs_full_repaint = false;
            self.damage_rects.clear();
            vec![[0, 0, self.width, self.height]]
        } else {
            std::mem::take(&mut self.damage_rects)
        }
    }

    pub fn end_frame(&mut self) {
        tracing::trace!(frame = self.frame_count, "Qt frame presented");
    }

    #[must_use]
    pub fn logical_size(&self) -> (f64, f64) {
        (
            self.width as f64 / self.device_pixel_ratio,
            self.height as f64 / self.device_pixel_ratio,
        )
    }

    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn backend(&self) -> QtRenderBackend {
        self.backend
    }

    #[must_use]
    pub fn device_pixel_ratio(&self) -> f64 {
        self.device_pixel_ratio
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
    fn test_qt_surface() {
        let mut s = QtRenderSurface::new(QtRenderBackend::Rhi, 3840, 2160, 2.0);
        assert_eq!(s.physical_size(), (3840, 2160));
        assert_eq!(s.logical_size(), (1920.0, 1080.0));

        let damage = s.begin_frame();
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0], [0, 0, 3840, 2160]);
        s.end_frame();

        s.add_damage(10, 10, 50, 50);
        let damage = s.begin_frame();
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0], [10, 10, 50, 50]);
    }
}
