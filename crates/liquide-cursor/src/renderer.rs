//! Cursor rendering for both software and hardware cursors.

use crate::shape::CursorShape;
use crate::state::CursorState;
use crate::vector_bridge;

/// Default nominal cursor size in logical pixels when the caller does not
/// override it. The effective device-pixel size is `DEFAULT_CURSOR_SIZE *
/// state.scale`.
pub const DEFAULT_CURSOR_SIZE: u32 = 24;

/// Round a float to the nearest integer with ties-to-even rounding
/// (banker's rounding). Matches IEEE-754 `roundTiesToEven` semantics — used
/// so fractional cursor positions don't introduce a consistent bias when
/// the compositor keeps sub-pixel precision and we quantise at submit.
fn round_ties_even_i32(v: f32) -> i32 {
    // Use the stdlib intrinsic when available (stable since 1.77).
    v.round_ties_even() as i32
}

/// Rendering target for software cursors.
pub enum RenderTarget<'a> {
    /// Render to an RGBA8 framebuffer.
    Rgba8 {
        /// Pixel data (row-major).
        pixels: &'a mut [u8],
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Stride in bytes.
        stride: usize,
    },

    /// Render to a BGRA8 framebuffer.
    Bgra8 {
        /// Pixel data (row-major).
        pixels: &'a mut [u8],
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Stride in bytes.
        stride: usize,
    },
}

/// Trait for cursor renderers.
pub trait CursorRenderer {
    /// Render the cursor to the given target.
    fn render(&self, cursor: &CursorState, target: RenderTarget) -> crate::Result<()>;

    /// Pre-load cursor shapes for better performance.
    fn preload(&mut self, shapes: &[CursorShape]) -> crate::Result<()>;
}

/// Software-based cursor renderer.
///
/// Renders cursors by compositing pixel data onto the framebuffer.
pub struct SoftwareCursorRenderer {
    /// Pre-rendered cursor images cache.
    cache: std::collections::HashMap<CursorShape, Vec<u8>>,
}

impl SoftwareCursorRenderer {
    /// Create a new software cursor renderer.
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// Generate a cursor image. If a `vector_bridge` backend is installed
    /// (typically by `liquide-cursor-vector`), it is consulted first so
    /// themed vector cursors are used at the correct DPI. Otherwise we
    /// fall back to a solid-colour debug square so an unconfigured system
    /// still renders *something* visible.
    fn generate_cursor_image(
        &self,
        shape: CursorShape,
        size: u32,
        scale: f32,
    ) -> (Vec<u8>, u32, u32) {
        if let Some(bmp) = vector_bridge::render(shape, size, scale) {
            return (bmp.pixels, bmp.width, bmp.height);
        }
        let device_size = ((size as f32) * scale.max(0.0)).round_ties_even() as u32;
        let device_size = device_size.max(1);
        let pixel_count = (device_size * device_size) as usize;
        let mut data = vec![0u8; pixel_count * 4];

        // Simple visualization: different colors per shape type
        let color = match shape {
            CursorShape::Arrow => [255, 255, 255, 255],
            CursorShape::Pointer => [100, 150, 255, 255],
            CursorShape::Text => [255, 255, 100, 255],
            CursorShape::Wait => [255, 100, 100, 255],
            CursorShape::Move => [100, 255, 100, 255],
            CursorShape::Resize(_) => [255, 150, 100, 255],
            _ => [200, 200, 200, 255],
        };

        for i in 0..pixel_count {
            data[i * 4] = color[0];
            data[i * 4 + 1] = color[1];
            data[i * 4 + 2] = color[2];
            data[i * 4 + 3] = color[3];
        }

        (data, device_size, device_size)
    }
}

impl Default for SoftwareCursorRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorRenderer for SoftwareCursorRenderer {
    fn render(&self, cursor: &CursorState, target: RenderTarget) -> crate::Result<()> {
        if !cursor.is_visible() {
            return Ok(());
        }

        // Get cursor image data
        let cursor_data = if let Some(ref custom) = cursor.custom_image {
            custom.as_slice()
        } else {
            // Use cached or generate built-in cursor at the correct DPI.
            // Scale is applied so HiDPI displays receive crisp pixels.
            let (data, w, h) =
                self.generate_cursor_image(cursor.shape, DEFAULT_CURSOR_SIZE, cursor.scale);
            return self.composite_cursor(cursor, &data, w, h, target);
        };

        self.composite_cursor(
            cursor,
            cursor_data,
            cursor.custom_width,
            cursor.custom_height,
            target,
        )
    }

    fn preload(&mut self, shapes: &[CursorShape]) -> crate::Result<()> {
        for &shape in shapes {
            if !self.cache.contains_key(&shape) {
                let (data, _, _) = self.generate_cursor_image(shape, DEFAULT_CURSOR_SIZE, 1.0);
                self.cache.insert(shape, data);
            }
        }
        Ok(())
    }
}

impl SoftwareCursorRenderer {
    /// Composite cursor image onto the framebuffer.
    fn composite_cursor(
        &self,
        cursor: &CursorState,
        cursor_data: &[u8],
        cursor_width: u32,
        cursor_height: u32,
        target: RenderTarget,
    ) -> crate::Result<()> {
        let (hotspot_x, hotspot_y) = cursor.effective_hotspot();
        // Keep f32 through the compositor; quantise to pixels at submit
        // using ties-to-even rounding so fractional positions don't bias
        // the cursor half a pixel in one direction.
        let draw_x = round_ties_even_i32(cursor.x - hotspot_x);
        let draw_y = round_ties_even_i32(cursor.y - hotspot_y);

        match target {
            RenderTarget::Rgba8 {
                pixels,
                width,
                height,
                stride,
            } => self.composite_rgba8(
                cursor_data,
                cursor_width,
                cursor_height,
                draw_x,
                draw_y,
                pixels,
                width,
                height,
                stride,
            ),
            RenderTarget::Bgra8 {
                pixels,
                width,
                height,
                stride,
            } => self.composite_bgra8(
                cursor_data,
                cursor_width,
                cursor_height,
                draw_x,
                draw_y,
                pixels,
                width,
                height,
                stride,
            ),
        }
    }

    /// Composite onto RGBA8 framebuffer.
    fn composite_rgba8(
        &self,
        cursor_data: &[u8],
        cursor_width: u32,
        cursor_height: u32,
        draw_x: i32,
        draw_y: i32,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        stride: usize,
    ) -> crate::Result<()> {
        for cy in 0..cursor_height {
            let screen_y = draw_y + cy as i32;
            if screen_y < 0 || screen_y >= height as i32 {
                continue;
            }

            for cx in 0..cursor_width {
                let screen_x = draw_x + cx as i32;
                if screen_x < 0 || screen_x >= width as i32 {
                    continue;
                }

                let cursor_idx = ((cy * cursor_width + cx) * 4) as usize;
                let screen_idx = (screen_y as usize * stride) + (screen_x as usize * 4);

                if cursor_idx + 3 >= cursor_data.len() || screen_idx + 3 >= pixels.len() {
                    continue;
                }

                let alpha = cursor_data[cursor_idx + 3] as u32;
                if alpha == 0 {
                    continue;
                }

                if alpha == 255 {
                    // Opaque: direct copy
                    pixels[screen_idx..screen_idx + 4]
                        .copy_from_slice(&cursor_data[cursor_idx..cursor_idx + 4]);
                } else {
                    // Alpha blend
                    let inv_alpha = 255 - alpha;
                    for i in 0..3 {
                        let src = cursor_data[cursor_idx + i] as u32;
                        let dst = pixels[screen_idx + i] as u32;
                        pixels[screen_idx + i] = ((src * alpha + dst * inv_alpha) / 255) as u8;
                    }
                    pixels[screen_idx + 3] = 255;
                }
            }
        }

        Ok(())
    }

    /// Composite onto BGRA8 framebuffer.
    fn composite_bgra8(
        &self,
        cursor_data: &[u8],
        cursor_width: u32,
        cursor_height: u32,
        draw_x: i32,
        draw_y: i32,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        stride: usize,
    ) -> crate::Result<()> {
        // Similar to RGBA8 but with B/R swapped
        for cy in 0..cursor_height {
            let screen_y = draw_y + cy as i32;
            if screen_y < 0 || screen_y >= height as i32 {
                continue;
            }

            for cx in 0..cursor_width {
                let screen_x = draw_x + cx as i32;
                if screen_x < 0 || screen_x >= width as i32 {
                    continue;
                }

                let cursor_idx = ((cy * cursor_width + cx) * 4) as usize;
                let screen_idx = (screen_y as usize * stride) + (screen_x as usize * 4);

                if cursor_idx + 3 >= cursor_data.len() || screen_idx + 3 >= pixels.len() {
                    continue;
                }

                let alpha = cursor_data[cursor_idx + 3] as u32;
                if alpha == 0 {
                    continue;
                }

                if alpha == 255 {
                    // Opaque: direct copy with BGR swap
                    pixels[screen_idx] = cursor_data[cursor_idx + 2]; // B
                    pixels[screen_idx + 1] = cursor_data[cursor_idx + 1]; // G
                    pixels[screen_idx + 2] = cursor_data[cursor_idx]; // R
                    pixels[screen_idx + 3] = cursor_data[cursor_idx + 3]; // A
                } else {
                    // Alpha blend with BGR swap
                    let inv_alpha = 255 - alpha;
                    let r = cursor_data[cursor_idx] as u32;
                    let g = cursor_data[cursor_idx + 1] as u32;
                    let b = cursor_data[cursor_idx + 2] as u32;

                    let dst_b = pixels[screen_idx] as u32;
                    let dst_g = pixels[screen_idx + 1] as u32;
                    let dst_r = pixels[screen_idx + 2] as u32;

                    pixels[screen_idx] = ((b * alpha + dst_b * inv_alpha) / 255) as u8;
                    pixels[screen_idx + 1] = ((g * alpha + dst_g * inv_alpha) / 255) as u8;
                    pixels[screen_idx + 2] = ((r * alpha + dst_r * inv_alpha) / 255) as u8;
                    pixels[screen_idx + 3] = 255;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_renderer_creation() {
        let renderer = SoftwareCursorRenderer::new();
        assert_eq!(renderer.cache.len(), 0);
    }

    #[test]
    fn test_preload() {
        let mut renderer = SoftwareCursorRenderer::new();
        let shapes = vec![CursorShape::Arrow, CursorShape::Pointer];

        let result = renderer.preload(&shapes);
        assert!(result.is_ok());
        assert_eq!(renderer.cache.len(), 2);
    }

    #[test]
    fn test_scale_doubles_pixel_count() {
        let renderer = SoftwareCursorRenderer::new();
        let (d1, w1, h1) =
            renderer.generate_cursor_image(CursorShape::Arrow, DEFAULT_CURSOR_SIZE, 1.0);
        let (d2, w2, h2) =
            renderer.generate_cursor_image(CursorShape::Arrow, DEFAULT_CURSOR_SIZE, 2.0);
        assert_eq!(w1, DEFAULT_CURSOR_SIZE);
        assert_eq!(h1, DEFAULT_CURSOR_SIZE);
        assert_eq!(w2, DEFAULT_CURSOR_SIZE * 2);
        assert_eq!(h2, DEFAULT_CURSOR_SIZE * 2);
        // scale 2.0 => 4× pixels (width×height both doubled)
        assert_eq!(d2.len(), d1.len() * 4);
    }

    #[test]
    fn test_round_ties_even_at_half() {
        // 0.5 → 0 (ties-to-even); 1.5 → 2; 2.5 → 2; -0.5 → 0
        assert_eq!(round_ties_even_i32(0.5), 0);
        assert_eq!(round_ties_even_i32(1.5), 2);
        assert_eq!(round_ties_even_i32(2.5), 2);
        assert_eq!(round_ties_even_i32(-0.5), 0);
        assert_eq!(round_ties_even_i32(1.4), 1);
        assert_eq!(round_ties_even_i32(1.6), 2);
    }

    #[test]
    fn test_vector_backend_is_consulted() {
        use crate::vector_bridge::{self, VectorCursorBackend, VectorCursorBitmap};

        struct Marker;
        impl VectorCursorBackend for Marker {
            fn render(
                &self,
                _shape: CursorShape,
                size: u32,
                scale: f32,
            ) -> Option<VectorCursorBitmap> {
                let n = ((size as f32) * scale) as u32;
                Some(VectorCursorBitmap {
                    pixels: vec![42u8; (n * n * 4) as usize],
                    width: n,
                    height: n,
                    hotspot_x: 0,
                    hotspot_y: 0,
                })
            }
        }

        vector_bridge::set_backend(std::sync::Arc::new(Marker));
        let renderer = SoftwareCursorRenderer::new();
        let (d, w, h) = renderer.generate_cursor_image(CursorShape::Arrow, 16, 1.0);
        assert_eq!(w, 16);
        assert_eq!(h, 16);
        assert!(d.iter().all(|&b| b == 42));
        vector_bridge::clear_backend();
    }
}
