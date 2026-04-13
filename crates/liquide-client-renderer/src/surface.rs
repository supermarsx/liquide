//! Client-side framebuffer for receiving and displaying decoded tiles.

use liquide_compositor::pixel::PixelFormat;
use serde::{Deserialize, Serialize};

/// Client-side pixel buffer that receives decoded tile data.
///
/// The surface stores pixels in a contiguous buffer with a configurable
/// pixel format. Tiles are written into the surface at grid-aligned
/// positions during frame reconstruction.
#[derive(Debug, Clone)]
pub struct RenderSurface {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
    format: PixelFormat,
}

impl RenderSurface {
    /// Create a new surface with the given dimensions and pixel format.
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bpp = format.bytes_per_pixel();
        let stride = width * bpp;
        let size = (stride * height) as usize;
        Self {
            pixels: vec![0u8; size],
            width,
            height,
            stride,
            format,
        }
    }

    /// Resize the surface, clearing all pixel data.
    pub fn resize(&mut self, width: u32, height: u32) {
        let bpp = self.format.bytes_per_pixel();
        self.width = width;
        self.height = height;
        self.stride = width * bpp;
        let size = (self.stride * height) as usize;
        self.pixels.clear();
        self.pixels.resize(size, 0);
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Row stride in bytes.
    #[must_use]
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Pixel format.
    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Raw pixel data.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Mutable raw pixel data.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Total byte size of the pixel buffer.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }

    /// Get a pixel at (x, y). Returns the pixel bytes or `None` if out of bounds.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let bpp = self.format.bytes_per_pixel() as usize;
        let offset = (y * self.stride) as usize + x as usize * bpp;
        Some(&self.pixels[offset..offset + bpp])
    }

    /// Set a pixel at (x, y). Panics if out of bounds or pixel length mismatch.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: &[u8]) {
        let bpp = self.format.bytes_per_pixel() as usize;
        assert!(x < self.width && y < self.height, "pixel out of bounds");
        assert_eq!(pixel.len(), bpp, "pixel size mismatch");
        let offset = (y * self.stride) as usize + x as usize * bpp;
        self.pixels[offset..offset + bpp].copy_from_slice(pixel);
    }

    /// Write tile data into the surface at tile coordinates (tx, ty).
    ///
    /// The tile data is stored row-major with `tile_size * bpp` bytes per row.
    /// Edge tiles that extend past the surface boundary are clipped.
    pub fn write_tile(&mut self, tx: u32, ty: u32, tile_size: u32, data: &[u8]) -> bool {
        let bpp = self.format.bytes_per_pixel();
        let px_x = tx * tile_size;
        let px_y = ty * tile_size;

        let rows = tile_size.min(self.height.saturating_sub(px_y));
        let cols = tile_size.min(self.width.saturating_sub(px_x));
        let row_bytes = (cols * bpp) as usize;
        let tile_stride = (tile_size * bpp) as usize;
        let mut all_written = true;

        for row in 0..rows {
            let dst_off = ((px_y + row) * self.stride) as usize + (px_x * bpp) as usize;
            let src_off = row as usize * tile_stride;
            if src_off + row_bytes <= data.len() && dst_off + row_bytes <= self.pixels.len() {
                self.pixels[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&data[src_off..src_off + row_bytes]);
            } else {
                all_written = false;
            }
        }
        all_written
    }

    /// Read tile data from the surface at tile coordinates (tx, ty).
    ///
    /// Returns a buffer of `tile_size * tile_size * bpp` bytes, zero-padded
    /// for edge tiles that extend past the surface boundary.
    #[must_use]
    pub fn read_tile(&self, tx: u32, ty: u32, tile_size: u32) -> Vec<u8> {
        let bpp = self.format.bytes_per_pixel();
        let tile_bytes = (tile_size * tile_size * bpp) as usize;
        let mut buf = vec![0u8; tile_bytes];

        let px_x = tx * tile_size;
        let px_y = ty * tile_size;

        let rows = tile_size.min(self.height.saturating_sub(px_y));
        let cols = tile_size.min(self.width.saturating_sub(px_x));
        let row_bytes = (cols * bpp) as usize;
        let tile_stride = (tile_size * bpp) as usize;

        for row in 0..rows {
            let src_off = ((px_y + row) * self.stride) as usize + (px_x * bpp) as usize;
            let dst_off = row as usize * tile_stride;
            buf[dst_off..dst_off + row_bytes]
                .copy_from_slice(&self.pixels[src_off..src_off + row_bytes]);
        }

        buf
    }

    /// Clear the surface to all zeros.
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }
}

impl std::fmt::Display for RenderSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RenderSurface({}x{}, {}, {} bytes)",
            self.width,
            self.height,
            self.format.wire_name(),
            self.byte_size()
        )
    }
}

/// Information about the current surface state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: u32,
    /// Pixel format wire name.
    pub format: String,
    /// Total byte size.
    pub byte_size: usize,
}

impl SurfaceInfo {
    /// Create surface info from a render surface.
    #[must_use]
    pub fn from_surface(surface: &RenderSurface) -> Self {
        Self {
            width: surface.width(),
            height: surface.height(),
            stride: surface.stride(),
            format: surface.format().wire_name().to_string(),
            byte_size: surface.byte_size(),
        }
    }
}
