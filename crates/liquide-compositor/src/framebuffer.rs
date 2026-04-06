//! Frame buffer management and double buffering.

use crate::pixel::{Color, PixelFormat};

/// Backing memory for a frame buffer.
#[derive(Debug)]
pub enum FrameMemory {
    /// CPU heap-allocated pixel buffer (default, always available).
    Cpu(Vec<u8>),
    /// GPU texture with optional DMA-BUF export for zero-copy paths.
    Gpu {
        /// Opaque GPU handle (VkImage handle or D3D12 resource pointer).
        handle: u64,
        /// DMA-BUF file descriptor for Linux zero-copy (-1 if unavailable).
        dmabuf_fd: i32,
        /// Width and height for re-creating CPU fallback if needed.
        width: u32,
        height: u32,
    },
}

/// A raw pixel buffer for composited output.
pub struct FrameBuffer {
    /// Backing pixel storage (CPU or GPU).
    pub memory: FrameMemory,
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
            memory: FrameMemory::Cpu(vec![0u8; size]),
            width,
            height,
            stride,
            format,
        }
    }

    /// Get read-only pixel slice (CPU path only).
    /// Returns an empty slice for GPU-backed framebuffers.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        match &self.memory {
            FrameMemory::Cpu(v) => v,
            FrameMemory::Gpu { .. } => &[],
        }
    }

    /// Get mutable pixel slice (CPU path only).
    ///
    /// # Panics
    /// Panics if called on a GPU-backed framebuffer.
    pub fn pixels_mut(&mut self) -> &mut Vec<u8> {
        match &mut self.memory {
            FrameMemory::Cpu(v) => v,
            FrameMemory::Gpu { .. } => panic!("cannot get mutable pixel access on GPU framebuffer"),
        }
    }

    /// Whether this framebuffer uses GPU memory.
    #[must_use]
    pub fn is_gpu(&self) -> bool {
        matches!(self.memory, FrameMemory::Gpu { .. })
    }

    /// Get DMA-BUF fd if available (None if CPU or fd < 0).
    #[must_use]
    pub fn dmabuf_fd(&self) -> Option<i32> {
        match &self.memory {
            FrameMemory::Gpu { dmabuf_fd, .. } if *dmabuf_fd >= 0 => Some(*dmabuf_fd),
            _ => None,
        }
    }

    /// Byte offset for the pixel at `(x, y)`.
    #[must_use]
    pub fn pixel_offset(&self, x: u32, y: u32) -> usize {
        (y * self.stride + x * self.format.bytes_per_pixel()) as usize
    }

    /// Get a slice of the pixel row at `y`.
    ///
    /// # Panics
    /// Panics if `y >= self.height`.
    #[must_use]
    pub fn row(&self, y: u32) -> &[u8] {
        debug_assert!(y < self.height, "row({y}) out of bounds (height={})", self.height);
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        &self.pixels()[start..end]
    }

    /// Get a mutable slice of the pixel row at `y`.
    ///
    /// # Panics
    /// Panics if `y >= self.height`.
    pub fn row_mut(&mut self, y: u32) -> &mut [u8] {
        debug_assert!(y < self.height, "row_mut({y}) out of bounds (height={})", self.height);
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        &mut self.pixels_mut()[start..end]
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
        let out_stride = (tw * bpp) as usize;
        let mut out = vec![0u8; out_stride * th as usize];

        for row in 0..th {
            let src_offset = ((py + row) * self.stride + px * bpp) as usize;
            let dst_offset = (row as usize) * out_stride;
            out[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&self.pixels()[src_offset..src_offset + row_bytes]);
        }
        out
    }

    /// Clear the entire buffer to a solid color.
    pub fn clear(&mut self, color: Color) {
        let bpp = self.format.bytes_per_pixel() as usize;
        let row_bytes = (self.width as usize) * bpp;

        // Build the pixel pattern for the first row.
        let pixel_bytes: [u8; 4] = match self.format {
            PixelFormat::Bgra8 => color.to_bgra_bytes(),
            PixelFormat::Rgba8 => [color.r, color.g, color.b, color.a],
            PixelFormat::Rgb8 => [color.r, color.g, color.b, 0],
            _ => [0; 4],
        };

        // Copy dimensions before borrowing pixels mutably.
        let width = self.width as usize;
        let stride = self.stride as usize;
        let height = self.height as usize;

        // Fill the first scanline pixel-by-pixel.
        let pixels = self.pixels_mut();
        {
            let first_row = &mut pixels[..row_bytes];
            for x in 0..width {
                let offset = x * bpp;
                first_row[offset..offset + bpp].copy_from_slice(&pixel_bytes[..bpp]);
            }
        }

        // Copy the first scanline to all subsequent rows.
        for y in 1..height {
            let (src, dst) = pixels.split_at_mut(y * stride);
            dst[..row_bytes].copy_from_slice(&src[..row_bytes]);
        }
    }

    /// Total size of the pixel data in bytes.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixels().len()
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
        let px = self.pixels();
        if off + bpp > px.len() {
            return Color::TRANSPARENT;
        }
        match self.format {
            PixelFormat::Bgra8 => {
                Color::from_bgra_bytes([
                    px[off],
                    px[off + 1],
                    px[off + 2],
                    px[off + 3],
                ])
            }
            PixelFormat::Rgba8 => {
                Color::new(
                    px[off],
                    px[off + 1],
                    px[off + 2],
                    px[off + 3],
                )
            }
            PixelFormat::Rgb8 => {
                Color::new(
                    px[off],
                    px[off + 1],
                    px[off + 2],
                    255,
                )
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
        let bpp = self.format.bytes_per_pixel() as usize;
        let fmt = self.format;
        let px = self.pixels_mut();
        if off + bpp > px.len() {
            return;
        }
        match fmt {
            PixelFormat::Bgra8 => {
                let bgra = color.to_bgra_bytes();
                px[off..off + 4].copy_from_slice(&bgra);
            }
            PixelFormat::Rgba8 => {
                px[off] = color.r;
                px[off + 1] = color.g;
                px[off + 2] = color.b;
                px[off + 3] = color.a;
            }
            PixelFormat::Rgb8 => {
                px[off] = color.r;
                px[off + 1] = color.g;
                px[off + 2] = color.b;
            }
            _ => {}
        }
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
            .field("pixel_bytes", &self.pixels().len())
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
