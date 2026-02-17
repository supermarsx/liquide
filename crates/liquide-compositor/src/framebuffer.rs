//! Frame buffer management and double buffering.

use crate::pixel::{Color, PixelFormat};

/// A raw pixel buffer for composited output.
pub struct FrameBuffer {
    /// Raw pixel data in the format specified by `format`.
    pub pixels: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row (may include padding for alignment).
    pub stride: u32,
    /// Pixel format of the buffer.
    pub format: PixelFormat,
}

impl FrameBuffer {
    /// Create a new frame buffer initialised to transparent black.
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bpp = format.bytes_per_pixel();
        let stride = width * bpp;
        Self::with_stride(width, height, stride, format)
    }

    /// Create a frame buffer with an explicit stride (for alignment).
    #[must_use]
    pub fn with_stride(width: u32, height: u32, stride: u32, format: PixelFormat) -> Self {
        let size = (stride * height) as usize;
        Self {
            pixels: vec![0u8; size],
            width,
            height,
            stride,
            format,
        }
    }

    /// Byte offset for the pixel at `(x, y)`.
    #[must_use]
    pub fn pixel_offset(&self, x: u32, y: u32) -> usize {
        (y * self.stride + x * self.format.bytes_per_pixel()) as usize
    }

    /// Get a slice of the pixel row at `y`.
    #[must_use]
    pub fn row(&self, y: u32) -> &[u8] {
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        &self.pixels[start..end]
    }

    /// Get a mutable slice of the pixel row at `y`.
    pub fn row_mut(&mut self, y: u32) -> &mut [u8] {
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        &mut self.pixels[start..end]
    }

    /// Extract raw pixel bytes for a tile region.
    ///
    /// Returns a contiguous buffer of `tile_h * tile_w * bpp` bytes.
    /// Tiles at the screen edge are clamped to the frame buffer bounds.
    #[must_use]
    pub fn tile_region(&self, tile_x: u32, tile_y: u32, tile_size: u32) -> Vec<u8> {
        let bpp = self.format.bytes_per_pixel();
        let px = tile_x * tile_size;
        let py = tile_y * tile_size;
        let tw = tile_size.min(self.width.saturating_sub(px));
        let th = tile_size.min(self.height.saturating_sub(py));
        let row_bytes = (tw * bpp) as usize;
        let mut out = vec![0u8; (tile_size * tile_size * bpp) as usize];

        for row in 0..th {
            let src_offset = ((py + row) * self.stride + px * bpp) as usize;
            let dst_offset = (row * tile_size * bpp) as usize;
            out[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&self.pixels[src_offset..src_offset + row_bytes]);
        }
        out
    }

    /// Clear the entire buffer to a solid color.
    pub fn clear(&mut self, color: Color) {
        let bgra = color.to_bgra_bytes();
        let bpp = self.format.bytes_per_pixel() as usize;
        for y in 0..self.height {
            let start = (y * self.stride) as usize;
            for x in 0..self.width {
                let offset = start + x as usize * bpp;
                match self.format {
                    PixelFormat::Bgra8 => {
                        self.pixels[offset..offset + 4].copy_from_slice(&bgra);
                    }
                    PixelFormat::Rgba8 => {
                        self.pixels[offset] = color.r;
                        self.pixels[offset + 1] = color.g;
                        self.pixels[offset + 2] = color.b;
                        self.pixels[offset + 3] = color.a;
                    }
                    PixelFormat::Rgb8 => {
                        self.pixels[offset] = color.r;
                        self.pixels[offset + 1] = color.g;
                        self.pixels[offset + 2] = color.b;
                    }
                    _ => {
                        // For other formats fall back to zero-fill
                        self.pixels[offset..offset + bpp].fill(0);
                    }
                }
            }
        }
    }

    /// Total size of the pixel data in bytes.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }

    /// Get the BGRA pixel at `(x, y)` as a [`Color`].
    ///
    /// Assumes `Bgra8` format. For other formats the result is approximate.
    /// Returns transparent black if coordinates are out of bounds.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if x >= self.width || y >= self.height {
            return Color::TRANSPARENT;
        }
        let off = self.pixel_offset(x, y);
        let bpp = self.format.bytes_per_pixel() as usize;
        if off + bpp > self.pixels.len() {
            return Color::TRANSPARENT;
        }
        match self.format {
            PixelFormat::Bgra8 | PixelFormat::Rgba8 => {
                Color::from_bgra_bytes([
                    self.pixels[off],
                    self.pixels[off + 1],
                    self.pixels[off + 2],
                    self.pixels[off + 3],
                ])
            }
            PixelFormat::Rgb8 => {
                Color {
                    b: self.pixels[off],
                    g: self.pixels[off + 1],
                    r: self.pixels[off + 2],
                    a: 255,
                }
            }
            _ => Color::TRANSPARENT,
        }
    }

    /// Set the BGRA pixel at `(x, y)`.
    /// Does nothing if coordinates are out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let off = self.pixel_offset(x, y);
        if off + 4 > self.pixels.len() {
            return;
        }
        let bgra = color.to_bgra_bytes();
        self.pixels[off..off + 4].copy_from_slice(&bgra);
    }

    /// Width of the tile grid for a given tile size.
    #[must_use]
    pub fn tile_grid_width(&self, tile_size: u32) -> u32 {
        self.width.div_ceil(tile_size)
    }

    /// Height of the tile grid for a given tile size.
    #[must_use]
    pub fn tile_grid_height(&self, tile_size: u32) -> u32 {
        self.height.div_ceil(tile_size)
    }
}

impl std::fmt::Debug for FrameBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameBuffer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("format", &self.format)
            .field("pixel_bytes", &self.pixels.len())
            .finish()
    }
}

/// Double-buffered frame management.
///
/// The back buffer is drawn into by the renderer, and the front buffer
/// is read by the encoder. Calling `swap()` exchanges them.
pub struct DoubleBuffer {
    front: FrameBuffer,
    back: FrameBuffer,
}

impl DoubleBuffer {
    /// Create a new double buffer.
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            front: FrameBuffer::new(width, height, format),
            back: FrameBuffer::new(width, height, format),
        }
    }

    /// Swap front and back buffers.
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
    }

    /// Access the back buffer (the one being drawn into).
    #[must_use]
    pub fn back(&self) -> &FrameBuffer {
        &self.back
    }

    /// Mutable access to the back buffer.
    pub fn back_mut(&mut self) -> &mut FrameBuffer {
        &mut self.back
    }

    /// Access the front buffer (the one being read by the encoder).
    #[must_use]
    pub fn front(&self) -> &FrameBuffer {
        &self.front
    }
}

impl std::fmt::Debug for DoubleBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoubleBuffer")
            .field("front", &self.front)
            .field("back", &self.back)
            .finish()
    }
}
