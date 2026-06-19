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
        let stride = width as usize * bpp as usize;
        Self::with_stride(width, height, stride as u32, format)
    }

    /// Create a frame buffer with an explicit stride (for alignment).
    #[must_use]
    pub fn with_stride(width: u32, height: u32, stride: u32, format: PixelFormat) -> Self {
        let size = stride as usize * height as usize;
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
    /// Returns `None` for GPU-backed framebuffers.
    pub fn pixels_mut(&mut self) -> Option<&mut Vec<u8>> {
        match &mut self.memory {
            FrameMemory::Cpu(v) => Some(v),
            FrameMemory::Gpu { .. } => None,
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
        assert!(
            y < self.height,
            "row({y}) out of bounds (height={})",
            self.height
        );
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        &self.pixels()[start..end]
    }

    /// Get a mutable slice of the pixel row at `y`.
    ///
    /// Returns `None` if `y` is out of bounds or the framebuffer is GPU-backed.
    pub fn row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        if y >= self.height {
            return None;
        }
        let start = (y * self.stride) as usize;
        let end = start + (self.width * self.format.bytes_per_pixel()) as usize;
        self.pixels_mut().map(|px| &mut px[start..end])
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

    /// Capture a rectangular sub-region of the framebuffer into an owned
    /// [`SurfaceBuffer`] (t93-e6 cheap window thumbnails / gap #1).
    ///
    /// This is a READ-ONLY copy of the already-composited pixels: it never
    /// writes the framebuffer and never touches the damage model or the
    /// write-scissor, so it is safe to call after a present without perturbing
    /// the incremental-scene / disappear-class invariants. The returned buffer
    /// is a tight, zero-padded copy (its `stride == width * bpp`), so consumers
    /// (e.g. an overview thumbnail) can scale/blit it directly.
    ///
    /// Bounds handling (deterministic):
    /// - the requested rect is rounded to integer pixels (floor origin, the
    ///   right/bottom edge taken from the ceil of the far edge) and then CLAMPED
    ///   to the framebuffer bounds, so an off-screen or partially off-screen
    ///   window yields only its on-screen pixels;
    /// - a zero-area (or fully off-screen) request yields a `1x1` transparent
    ///   buffer rather than an empty one, so callers always get a paintable
    ///   surface and never index an empty slice.
    ///
    /// HONEST caveat (documented for gap #1): this captures whatever was DRAWN
    /// at those coordinates in the last frame. For an OCCLUDED window that means
    /// the pixels of whatever covered it — you cannot capture pixels that were
    /// never rasterised for the covered window. That is exactly what per-surface
    /// render-to-texture (E7) solves later; for an overview snapshot the
    /// last-composited content is an acceptable, cheap approximation.
    #[must_use]
    pub fn capture_region(&self, rect: crate::geometry::Rect) -> crate::scene::SurfaceBuffer {
        use crate::scene::SurfaceBuffer;
        use std::sync::Arc;

        let bpp = self.format.bytes_per_pixel();

        // Round to integer pixels, then clamp to the framebuffer. `floor` the
        // origin and `ceil` the far edge so a fractional rect captures every
        // pixel it touches; saturating math keeps a negative origin at 0.
        let x0 = rect.x.floor().max(0.0) as u32;
        let y0 = rect.y.floor().max(0.0) as u32;
        let x1 = rect.right().ceil().max(0.0) as u32;
        let y1 = rect.bottom().ceil().max(0.0) as u32;
        let x0 = x0.min(self.width);
        let y0 = y0.min(self.height);
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);

        let cw = x1.saturating_sub(x0);
        let ch = y1.saturating_sub(y0);

        // Empty / off-screen request → a 1x1 transparent surface (never empty).
        if cw == 0 || ch == 0 {
            return SurfaceBuffer {
                pixels: Arc::new(vec![0u8; bpp as usize]),
                width: 1,
                height: 1,
                stride: bpp,
                format: self.format,
            };
        }

        let out_stride = (cw * bpp) as usize;
        let row_bytes = out_stride;
        let mut out = vec![0u8; out_stride * ch as usize];
        let src = self.pixels();

        for row in 0..ch {
            let src_offset = ((y0 + row) * self.stride + x0 * bpp) as usize;
            let dst_offset = (row as usize) * out_stride;
            // Defensive bound: a GPU-backed (empty `pixels()`) framebuffer or a
            // truncated buffer leaves the destination zero-filled rather than
            // panicking.
            if src_offset + row_bytes <= src.len() {
                out[dst_offset..dst_offset + row_bytes]
                    .copy_from_slice(&src[src_offset..src_offset + row_bytes]);
            }
        }

        SurfaceBuffer {
            pixels: Arc::new(out),
            width: cw,
            height: ch,
            stride: cw * bpp,
            format: self.format,
        }
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
        let Some(pixels) = self.pixels_mut() else {
            return; // Cannot clear GPU-backed framebuffer via CPU path
        };
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

    /// Deterministic fingerprint of the buffer shape and current pixel bytes.
    #[must_use]
    pub fn content_hash(&self) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

        fn mix(hash: &mut u64, bytes: &[u8]) {
            for byte in bytes {
                *hash ^= u64::from(*byte);
                *hash = hash.wrapping_mul(FNV_PRIME);
            }
        }

        let mut hash = FNV_OFFSET;
        mix(&mut hash, &self.width.to_le_bytes());
        mix(&mut hash, &self.height.to_le_bytes());
        mix(&mut hash, &self.stride.to_le_bytes());
        mix(&mut hash, self.format.wire_name().as_bytes());
        mix(&mut hash, self.pixels());
        hash
    }

    /// Damage-scoped fingerprint: a deterministic hash of the buffer shape, the
    /// damage geometry, and ONLY the pixel bytes inside the damaged tiles.
    ///
    /// This is the hot-path replacement for [`content_hash`](Self::content_hash)
    /// (which scans the whole 8 MB framebuffer scalar-FNV every frame — t90 Lever
    /// 1). The only consumers of the frame hash compare it for *equality* against
    /// the previously presented frame to detect a changed/new surface and to tag
    /// presentation metadata; neither needs a whole-frame digest, only "did the
    /// presented (damaged) region change?".
    ///
    /// CORRECTNESS — why damage-scoping never misses a real change:
    /// - Any pixel that actually changed this frame is, by construction, inside
    ///   the frame's authoritative damage (the raster write-scissor forbids
    ///   writes outside it). So an in-damage change always flips this hash.
    /// - Pixels OUTSIDE the damage are, by definition, unchanged from the prior
    ///   frame, so excluding them cannot hide a change.
    /// - The damage GEOMETRY (tile coordinates + grid + tile size) is folded into
    ///   the hash, so two frames that touch *different* regions can never collide
    ///   to the same hash even if the touched bytes happen to match — a moved
    ///   surface is always detected as a new presentation.
    ///
    /// A full-frame [`DamageSet`] hashes the whole buffer (identical coverage to
    /// [`content_hash`](Self::content_hash), though not byte-identical output —
    /// the deterministic capture/`render_frame_sync` path must keep using
    /// [`content_hash`](Self::content_hash) for its full-frame digest).
    #[must_use]
    pub fn content_hash_damaged(&self, damage: &crate::damage::DamageSet) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

        #[inline]
        fn mix(hash: &mut u64, bytes: &[u8]) {
            for byte in bytes {
                *hash ^= u64::from(*byte);
                *hash = hash.wrapping_mul(FNV_PRIME);
            }
        }

        let mut hash = FNV_OFFSET;
        mix(&mut hash, &self.width.to_le_bytes());
        mix(&mut hash, &self.height.to_le_bytes());
        mix(&mut hash, &self.stride.to_le_bytes());
        mix(&mut hash, self.format.wire_name().as_bytes());

        // Empty damage (a no-op keepalive frame): identity is purely the shape
        // above plus a marker — distinct from any frame that hashed real bytes.
        if damage.is_empty() {
            mix(&mut hash, b"empty-damage");
            return hash;
        }

        let bpp = self.format.bytes_per_pixel();
        let tile_size = damage.tile_size.max(1);
        let stride = self.stride;
        let px = self.pixels();

        // Hash the bytes of one tile (clamped to the surface), row-by-row.
        let mix_tile = |hash: &mut u64, tx: u32, ty: u32| {
            let x0 = tx.saturating_mul(tile_size).min(self.width);
            let y0 = ty.saturating_mul(tile_size).min(self.height);
            let x1 = x0.saturating_add(tile_size).min(self.width);
            let y1 = y0.saturating_add(tile_size).min(self.height);
            // Fold tile coordinates into the digest so distinct damage layouts
            // never alias.
            mix(hash, &tx.to_le_bytes());
            mix(hash, &ty.to_le_bytes());
            for y in y0..y1 {
                let start = (y * stride + x0 * bpp) as usize;
                let end = (y * stride + x1 * bpp) as usize;
                if end <= px.len() {
                    mix(hash, &px[start..end]);
                }
            }
        };

        if let Some((grid_w, grid_h, _class)) = damage.full_grid_dimensions() {
            mix(&mut hash, b"full");
            for ty in 0..grid_h {
                for tx in 0..grid_w {
                    mix_tile(&mut hash, tx, ty);
                }
            }
        } else {
            for tile in &damage.tiles {
                mix_tile(&mut hash, tile.x, tile.y);
            }
        }
        hash
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
                Color::from_bgra_bytes([px[off], px[off + 1], px[off + 2], px[off + 3]])
            }
            PixelFormat::Rgba8 => Color::new(px[off], px[off + 1], px[off + 2], px[off + 3]),
            PixelFormat::Rgb8 => Color::new(px[off], px[off + 1], px[off + 2], 255),
            _ => Color::TRANSPARENT,
        }
    }

    /// Set the BGRA pixel at `(x, y)`.
    ///
    /// Does nothing if the coordinates are out of bounds, or if an active
    /// write-scissor is installed for this thread and `(x, y)` falls outside it
    /// ([`crate::scissor`]). The scissor check is the INESCAPABLE damage-only
    /// chokepoint: every pixel write in the engine ultimately lands here (raster
    /// helpers, raw get-modify-set node loops, blur write-back, future kinds), so
    /// no codepath can corrupt a pixel outside the active damage rect regardless
    /// of whether it threaded a clip argument. When no scissor is set the check
    /// is a single branch-predictable compare against `None` — a true no-op.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        if !crate::scissor::scissor_allows(x, y) {
            return;
        }
        let off = self.pixel_offset(x, y);
        let bpp = self.format.bytes_per_pixel() as usize;
        let fmt = self.format;
        let Some(px) = self.pixels_mut() else {
            return; // Cannot set pixel on GPU-backed framebuffer
        };
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
            _ => {
                debug_assert!(false, "unhandled pixel format: {:?}", fmt);
            }
        }
    }

    /// Width of the tile grid for a given tile size.
    #[must_use]
    pub fn tile_grid_width(&self, tile_size: u32) -> u32 {
        if tile_size == 0 {
            return 1;
        }
        self.width.div_ceil(tile_size)
    }

    /// Height of the tile grid for a given tile size.
    #[must_use]
    pub fn tile_grid_height(&self, tile_size: u32) -> u32 {
        if tile_size == 0 {
            return 1;
        }
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

// ----------------------------------------------------------------------------
// FrameMemoryPool
// ----------------------------------------------------------------------------

/// Recycling pool for `FrameBuffer` backing memory.
///
/// At 4K BGRA8 each framebuffer is ~33 MB; allocating a fresh one per frame
/// on each render thread was a measured hotspot (§3.6 t8 review).  Consumers
/// should `acquire(w, h, format)` at frame start and `release(fb)` at frame
/// end — the pool recycles the `Vec<u8>` behind the scenes.
#[derive(Default)]
pub struct FrameMemoryPool {
    /// Buffers bucketed by (width, height, format) tuples.
    buffers: std::collections::HashMap<(u32, u32, PixelFormat), Vec<Vec<u8>>>,
    /// Soft cap on buffers retained per bucket.
    max_per_bucket: usize,
    /// Statistics: total allocations performed.
    allocations: u64,
    /// Statistics: total reuses satisfied from the pool.
    reuses: u64,
}

impl FrameMemoryPool {
    /// Create a new pool with a default retention cap of 4 buffers per bucket.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: std::collections::HashMap::new(),
            max_per_bucket: 4,
            allocations: 0,
            reuses: 0,
        }
    }

    /// Create a pool with an explicit retention cap.
    #[must_use]
    pub fn with_capacity(max_per_bucket: usize) -> Self {
        Self {
            buffers: std::collections::HashMap::new(),
            max_per_bucket,
            allocations: 0,
            reuses: 0,
        }
    }

    /// Acquire a framebuffer of the requested dimensions.
    ///
    /// Reuses a released buffer when the `(width, height, format)` triple
    /// matches; otherwise allocates a fresh one.  The returned framebuffer's
    /// pixel memory is not cleared — callers must call `clear()` if they
    /// need a known starting state.
    pub fn acquire(&mut self, width: u32, height: u32, format: PixelFormat) -> FrameBuffer {
        let key = (width, height, format);
        let stride = width * format.bytes_per_pixel();
        if let Some(bucket) = self.buffers.get_mut(&key) {
            if let Some(mem) = bucket.pop() {
                self.reuses += 1;
                return FrameBuffer {
                    memory: FrameMemory::Cpu(mem),
                    width,
                    height,
                    stride,
                    format,
                };
            }
        }
        self.allocations += 1;
        FrameBuffer::with_stride(width, height, stride, format)
    }

    /// Return a framebuffer to the pool for recycling.
    ///
    /// GPU-backed framebuffers are dropped (the GPU handle cannot be
    /// reclaimed here).  CPU-backed buffers are retained up to
    /// `max_per_bucket` per `(width, height, format)` key.
    pub fn release(&mut self, fb: FrameBuffer) {
        let key = (fb.width, fb.height, fb.format);
        let FrameBuffer { memory, .. } = fb;
        if let FrameMemory::Cpu(mem) = memory {
            let bucket = self.buffers.entry(key).or_default();
            if bucket.len() < self.max_per_bucket {
                bucket.push(mem);
            }
            // else drop — bucket is full.
        }
    }

    /// Total allocations performed by the pool.
    #[must_use]
    pub fn allocations(&self) -> u64 {
        self.allocations
    }

    /// Total reuses satisfied by the pool.
    #[must_use]
    pub fn reuses(&self) -> u64 {
        self.reuses
    }

    /// Drop all retained buffers.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}

impl std::fmt::Debug for FrameMemoryPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameMemoryPool")
            .field("buckets", &self.buffers.len())
            .field("allocations", &self.allocations)
            .field("reuses", &self.reuses)
            .finish()
    }
}

#[cfg(test)]
mod pool_tests {
    use super::*;

    #[test]
    fn framebuffer_content_hash_tracks_pixel_changes() {
        let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
        let initial = fb.content_hash();

        fb.set_pixel(1, 1, Color::new(40, 80, 120, 255));
        let changed = fb.content_hash();

        assert_ne!(initial, changed);
        assert_eq!(changed, fb.content_hash());
    }

    // ── t90 Lever 1: damage-scoped content hash ──────────────────────────
    use crate::damage::{DamageClass, DamageSet, DamageTile};

    /// (a) A change INSIDE the damaged tile flips the damage-scoped hash; a
    /// change OUTSIDE the damaged tile is NOT hashed (so the out-of-damage byte
    /// cannot affect the digest). This proves the hash touches only ~damage
    /// bytes, not the whole 8 MB framebuffer — and that it never misses a real
    /// in-damage change. If `content_hash_damaged` ever reverted to scanning the
    /// whole buffer, the out-of-damage assertion below would FAIL.
    #[test]
    fn content_hash_damaged_scopes_to_damage_tiles() {
        // 128x128, tile 64 => 2x2 tile grid. Damage only tile (0,0).
        let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut damage = DamageSet::new(64);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        });

        let baseline = fb.content_hash_damaged(&damage);

        // Change a pixel OUTSIDE the damaged tile (tile (1,1), pixel (100,100)).
        fb.set_pixel(100, 100, Color::new(10, 20, 30, 255));
        assert_eq!(
            baseline,
            fb.content_hash_damaged(&damage),
            "out-of-damage change must NOT be hashed (proves damage-scoping; not a whole-frame scan)"
        );

        // Change a pixel INSIDE the damaged tile (tile (0,0), pixel (5,5)).
        fb.set_pixel(5, 5, Color::new(40, 80, 120, 255));
        assert_ne!(
            baseline,
            fb.content_hash_damaged(&damage),
            "in-damage change MUST be detected (no missed-change regression)"
        );
    }

    /// Distinct damage GEOMETRY yields distinct hashes even when the hashed
    /// bytes coincide — so a surface moving between two equal-content regions is
    /// still detected as a new presentation (no collision).
    #[test]
    fn content_hash_damaged_folds_in_tile_geometry() {
        let fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut d00 = DamageSet::new(64);
        d00.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::UiPrimitive,
        });
        let mut d11 = DamageSet::new(64);
        d11.add(DamageTile {
            x: 1,
            y: 1,
            class: DamageClass::UiPrimitive,
        });
        // Both tiles are all-zero (identical bytes), but the geometry differs.
        assert_ne!(
            fb.content_hash_damaged(&d00),
            fb.content_hash_damaged(&d11),
            "distinct damage layouts must not collide"
        );
    }

    /// A full-frame damage set hashes the whole buffer, so it still tracks any
    /// pixel change anywhere (parity with `content_hash` coverage).
    #[test]
    fn content_hash_damaged_full_tracks_any_change() {
        let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let full = DamageSet::full(64, 2, 2, DamageClass::UiPrimitive);
        let baseline = fb.content_hash_damaged(&full);
        fb.set_pixel(100, 100, Color::new(1, 2, 3, 255));
        assert_ne!(baseline, fb.content_hash_damaged(&full));
    }

    #[test]
    fn pool_reuses_buffer() {
        let mut pool = FrameMemoryPool::new();
        let fb1 = pool.acquire(100, 100, PixelFormat::Bgra8);
        assert_eq!(pool.allocations(), 1);
        assert_eq!(pool.reuses(), 0);
        pool.release(fb1);
        let fb2 = pool.acquire(100, 100, PixelFormat::Bgra8);
        assert_eq!(pool.allocations(), 1, "should have reused memory");
        assert_eq!(pool.reuses(), 1);
        pool.release(fb2);
    }

    #[test]
    fn pool_distinguishes_dimensions() {
        let mut pool = FrameMemoryPool::new();
        let a = pool.acquire(100, 100, PixelFormat::Bgra8);
        pool.release(a);
        let _b = pool.acquire(200, 200, PixelFormat::Bgra8);
        assert_eq!(pool.allocations(), 2);
        assert_eq!(pool.reuses(), 0);
    }

    #[test]
    fn pool_bucket_cap() {
        let mut pool = FrameMemoryPool::with_capacity(2);
        // Allocate 4 fresh buffers, then release them all so the bucket
        // sees 4 distinct returns; with cap=2 only the first two are retained.
        let mut fbs = Vec::with_capacity(4);
        for _ in 0..4 {
            fbs.push(pool.acquire(10, 10, PixelFormat::Bgra8));
        }
        assert_eq!(pool.allocations(), 4);
        assert_eq!(pool.reuses(), 0);
        for fb in fbs {
            pool.release(fb);
        }
        // Only 2 retained; the remaining releases are discarded.
        let _fb = pool.acquire(10, 10, PixelFormat::Bgra8);
        let _fb2 = pool.acquire(10, 10, PixelFormat::Bgra8);
        // Two reuses from the cap-2 bucket.
        assert_eq!(pool.reuses(), 2);
        // Third acquire must allocate fresh because the bucket was capped.
        let _fb3 = pool.acquire(10, 10, PixelFormat::Bgra8);
        assert_eq!(pool.allocations(), 5);
        assert_eq!(pool.reuses(), 2);
    }
}
