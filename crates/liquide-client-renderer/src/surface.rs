//! Client-side framebuffer for receiving and displaying decoded tiles.

use liquide_compositor::pixel::PixelFormat;
use serde::{Deserialize, Serialize};

/// Maximum allowed value for any single surface/tile dimension (pixels).
///
/// A malformed/hostile peer can send absurd width/height/tile values. Without a
/// bound, `width * height * bpp` can wrap a `u32` (e.g. `width = u32::MAX`
/// yields a 0-byte allocation that is then indexed with in-range-looking offsets
/// → out-of-bounds writes / UB). Every size computation widens to `u64`/`usize`
/// and validates dimensions against this bound *before* allocating/indexing, so
/// products can never wrap. 16384 px/axis comfortably exceeds 8K displays.
pub const MAX_DIMENSION: u32 = 16384;

/// Maximum total pixel count for a single surface.
///
/// A second, *total*-size sanity bound on top of [`MAX_DIMENSION`]: even within
/// the per-axis limit, `MAX_DIMENSION * MAX_DIMENSION` would be a ~1 GiB
/// allocation — itself a denial-of-service vector. Surfaces whose
/// `width * height` exceeds this collapse to an empty 0x0 surface rather than
/// being honoured. 64 mega-pixels covers 8K (33 MP) and large virtual desktops.
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// Validate `(width, height)` against the dimension and total-pixel bounds.
/// Returns the dimensions unchanged when plausible, or `(0, 0)` when they are
/// implausibly large / would overflow — collapsing to an empty surface is
/// memory-safe (every `get`/`set`/index then stays in-bounds) and avoids both
/// the wrap-to-under-allocation bug and a giant DoS allocation.
#[inline]
#[must_use]
fn sanitize_dims(width: u32, height: u32) -> (u32, u32) {
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return (0, 0);
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return (0, 0);
    }
    (width, height)
}

/// Compute a byte size with overflow-safe widened arithmetic.
///
/// Returns `None` if the product overflows `usize` (after `u64` widening).
/// Sanitized inputs guarantee `Some`, but the check is kept defensive.
#[inline]
#[must_use]
fn checked_byte_size(width: u32, height: u32, bpp: u32) -> Option<usize> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(u64::from(bpp))?;
    usize::try_from(bytes).ok()
}

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
        // Validate dims BEFORE any multiply so products cannot wrap and
        // under-allocate (memory-safety: t49-e7-F1). Implausible/overflowing
        // sizes collapse to an empty 0x0 surface.
        let (width, height) = sanitize_dims(width, height);
        let stride = width.saturating_mul(bpp);
        let size = checked_byte_size(width, height, bpp).unwrap_or(0);
        Self {
            pixels: vec![0u8; size],
            width,
            height,
            stride,
            format,
        }
    }

    /// Resize the surface, clearing all pixel data.
    ///
    /// Implausible/overflowing dimensions collapse to an empty 0x0 surface so
    /// the byte-size computation cannot overflow and under-allocate.
    pub fn resize(&mut self, width: u32, height: u32) {
        let bpp = self.format.bytes_per_pixel();
        let (width, height) = sanitize_dims(width, height);
        self.width = width;
        self.height = height;
        self.stride = width.saturating_mul(bpp);
        let size = checked_byte_size(width, height, bpp).unwrap_or(0);
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
        // Widen before multiply: `y * stride` can exceed u32 for valid
        // dimensions, so compute the byte offset in usize.
        let offset = y as usize * self.stride as usize + x as usize * bpp;
        Some(&self.pixels[offset..offset + bpp])
    }

    /// Set a pixel at (x, y). Panics if out of bounds or pixel length mismatch.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: &[u8]) {
        let bpp = self.format.bytes_per_pixel() as usize;
        assert!(x < self.width && y < self.height, "pixel out of bounds");
        assert_eq!(pixel.len(), bpp, "pixel size mismatch");
        let offset = y as usize * self.stride as usize + x as usize * bpp;
        self.pixels[offset..offset + bpp].copy_from_slice(pixel);
    }

    /// Write tile data into the surface at tile coordinates (tx, ty).
    ///
    /// The tile data is stored row-major with `tile_size * bpp` bytes per row.
    /// Edge tiles that extend past the surface boundary are clipped.
    pub fn write_tile(&mut self, tx: u32, ty: u32, tile_size: u32, data: &[u8]) -> bool {
        let bpp = self.format.bytes_per_pixel();
        let tile_size = tile_size.min(MAX_DIMENSION);
        // Saturating origin so a hostile tx/ty/tile_size cannot wrap into a
        // small in-range-looking offset.
        let px_x = tx.saturating_mul(tile_size);
        let px_y = ty.saturating_mul(tile_size);

        let rows = tile_size.min(self.height.saturating_sub(px_y));
        let cols = tile_size.min(self.width.saturating_sub(px_x));
        let row_bytes = cols as usize * bpp as usize;
        let tile_stride = tile_size as usize * bpp as usize;
        let mut all_written = true;

        for row in 0..rows {
            // All offsets computed in usize (saturating) so a malformed
            // coordinate cannot slip past the bounds check into a wrong region.
            let dst_off = (px_y as usize + row as usize)
                .saturating_mul(self.stride as usize)
                .saturating_add(px_x as usize * bpp as usize);
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
        // Sanitize + widen so `tile_size^2 * bpp` cannot wrap into an undersized
        // allocation, and an implausible tile_size collapses to an empty tile.
        let (tile_size, _) = sanitize_dims(tile_size, tile_size);
        let tile_bytes = checked_byte_size(tile_size, tile_size, bpp).unwrap_or(0);
        let mut buf = vec![0u8; tile_bytes];

        let px_x = tx.saturating_mul(tile_size);
        let px_y = ty.saturating_mul(tile_size);

        let rows = tile_size.min(self.height.saturating_sub(px_y));
        let cols = tile_size.min(self.width.saturating_sub(px_x));
        let row_bytes = cols as usize * bpp as usize;
        let tile_stride = tile_size as usize * bpp as usize;

        for row in 0..rows {
            // Widened/saturating offset math; guard both ends of the copy.
            let src_off = (px_y as usize + row as usize)
                .saturating_mul(self.stride as usize)
                .saturating_add(px_x as usize * bpp as usize);
            let dst_off = row as usize * tile_stride;
            if src_off + row_bytes <= self.pixels.len() && dst_off + row_bytes <= buf.len() {
                buf[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&self.pixels[src_off..src_off + row_bytes]);
            }
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
