//! Tile types and grid management for the encoding pipeline.

use serde::{Deserialize, Serialize};

use liquide_compositor::damage::DamageClass;

/// Maximum allowed value for any single tile/grid dimension (pixels per axis).
///
/// Bounds frame-buffer width/height and grid `cols`/`rows` so that size
/// products (`cols * rows`) computed below can never wrap a `u32` and
/// under-allocate a buffer later indexed (memory-safety: t49-e7-F1). 16384
/// comfortably exceeds 8K displays while keeping widened products small.
pub const MAX_DIMENSION: u32 = 16384;

/// Maximum allowed tile edge length (pixels). Real tiles are ~64 px; a value
/// this large is already absurd, and `MAX_TILE_SIZE^2 * bpp` stays a modest,
/// non-wrapping allocation. Larger tiles are clamped.
pub const MAX_TILE_SIZE: u32 = 1024;

/// Clamp a frame/grid dimension to [`MAX_DIMENSION`].
#[inline]
#[must_use]
fn clamp_dim(value: u32) -> u32 {
    value.min(MAX_DIMENSION)
}

/// Clamp a tile edge length to [`MAX_TILE_SIZE`].
#[inline]
#[must_use]
fn clamp_tile(value: u32) -> u32 {
    value.min(MAX_TILE_SIZE)
}

use crate::strategy::CompressionMethod;

/// How a tile was encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileEncoding {
    /// Tile is unchanged — skip it.
    Skip,
    /// XOR delta against the previous frame, then compressed.
    Delta,
    /// Full tile data, compressed.
    Full,
    /// Copy from another tile in the same frame (content-addressable).
    Copy { source_index: u32 },
    /// Tile is a solid color (4 bytes only).
    Solid,
}

/// A single encoded tile update.
#[derive(Debug, Clone)]
pub struct TileUpdate {
    /// Tile X coordinate in tile-grid space.
    pub tx: u32,
    /// Tile Y coordinate in tile-grid space.
    pub ty: u32,
    /// How this tile was encoded.
    pub encoding: TileEncoding,
    /// Compressed payload (empty for Skip tiles).
    pub payload: Vec<u8>,
    /// CRC-32C of the uncompressed tile data (for verification).
    pub crc: u32,
    /// Damage classification for this tile.
    pub damage_class: DamageClass,
    /// Compression method used for the payload.
    pub compression: CompressionMethod,
}

/// Per-frame encoding statistics.
#[derive(Debug, Clone)]
pub struct FrameStats {
    /// Total encode time for this frame in microseconds.
    pub encode_time_us: u64,
    /// Number of tiles that were encoded (non-skip).
    pub tiles_encoded: u32,
    /// Total bytes saved by encoding (uncompressed - compressed).
    pub bytes_saved: u64,
    /// Overall compression ratio (compressed / uncompressed).
    pub compression_ratio: f64,
    /// Number of tiles using LZ4 compression.
    pub lz4_tiles: u32,
    /// Number of tiles using Zstd compression.
    pub zstd_tiles: u32,
}

impl FrameStats {
    /// Create empty stats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            encode_time_us: 0,
            tiles_encoded: 0,
            bytes_saved: 0,
            compression_ratio: 0.0,
            lz4_tiles: 0,
            zstd_tiles: 0,
        }
    }
}

impl Default for FrameStats {
    fn default() -> Self {
        Self::new()
    }
}

/// A batch of tile updates for one frame.
#[derive(Debug, Clone)]
pub struct TileBatch {
    /// Frame sequence number.
    pub sequence: u64,
    /// Tile updates in this batch.
    pub tiles: Vec<TileUpdate>,
    /// Total uncompressed size of all tile payloads.
    pub uncompressed_bytes: u64,
    /// Total compressed size of all tile payloads.
    pub compressed_bytes: u64,
    /// Per-frame statistics.
    pub stats: FrameStats,
}

impl TileBatch {
    /// Create a new empty batch for the given sequence number.
    #[must_use]
    pub fn new(sequence: u64) -> Self {
        Self {
            sequence,
            tiles: Vec::new(),
            uncompressed_bytes: 0,
            compressed_bytes: 0,
            stats: FrameStats::new(),
        }
    }

    /// Number of non-skip tiles.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|t| t.encoding != TileEncoding::Skip)
            .count()
    }

    /// Compression ratio (compressed / uncompressed). Returns 0.0 if uncompressed is zero.
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_bytes == 0 {
            return 0.0;
        }
        self.compressed_bytes as f64 / self.uncompressed_bytes as f64
    }

    /// Total payload bytes across all tile updates in this batch.
    #[must_use]
    pub fn total_payload_bytes(&self) -> usize {
        self.tiles.iter().map(|t| t.payload.len()).sum()
    }
}

/// Configuration for the tile grid.
#[derive(Debug, Clone)]
pub struct TileConfig {
    /// Tile size in pixels (typically 64).
    pub tile_size: u32,
    /// Bytes per pixel (typically 4 for BGRA8).
    pub bpp: u32,
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            tile_size: 64,
            bpp: 4,
        }
    }
}

impl TileConfig {
    /// Bytes per uncompressed tile.
    ///
    /// Clamped + `u64`-widened so a malformed `tile_size`/`bpp` cannot wrap and
    /// produce an undersized allocation later indexed out of bounds
    /// (memory-safety: t49-e7-F1).
    #[must_use]
    pub fn tile_bytes(&self) -> usize {
        let ts = u64::from(clamp_tile(self.tile_size));
        let bpp = u64::from(self.bpp.min(16));
        let bytes = ts.saturating_mul(ts).saturating_mul(bpp);
        usize::try_from(bytes).unwrap_or(usize::MAX)
    }
}

/// Codec for tile extraction from a frame buffer.
pub struct TileCodec {
    config: TileConfig,
}

impl TileCodec {
    /// Create a new tile codec.
    #[must_use]
    pub fn new(config: TileConfig) -> Self {
        Self { config }
    }

    /// Extract raw tile bytes from a pixel buffer at tile coordinates (tx, ty).
    ///
    /// Returns a buffer of `tile_size * tile_size * bpp` bytes,
    /// zero-padded for edge tiles that extend past the frame boundary.
    #[must_use]
    pub fn extract_tile(
        &self,
        pixels: &[u8],
        stride: u32,
        fb_width: u32,
        fb_height: u32,
        tx: u32,
        ty: u32,
    ) -> Vec<u8> {
        let ts = clamp_tile(self.config.tile_size);
        let bpp = self.config.bpp.min(16);
        let tile_bytes = self.config.tile_bytes();
        let mut buf = vec![0u8; tile_bytes];

        // Saturating origin so a hostile tx/ty cannot wrap into a small offset.
        let px_x = tx.saturating_mul(ts);
        let px_y = ty.saturating_mul(ts);

        let rows = ts.min(fb_height.saturating_sub(px_y));
        let cols = ts.min(fb_width.saturating_sub(px_x));
        let row_bytes = cols as usize * bpp as usize;

        for row in 0..rows {
            // Widened/saturating offset math; guard the copy against a src that
            // runs past the supplied buffer (memory-safety: t49-e7-F1).
            let src_off = (px_y as usize + row as usize)
                .saturating_mul(stride as usize)
                .saturating_add(px_x as usize * bpp as usize);
            let dst_off = row as usize * ts as usize * bpp as usize;
            if src_off + row_bytes <= pixels.len() && dst_off + row_bytes <= buf.len() {
                buf[dst_off..dst_off + row_bytes]
                    .copy_from_slice(&pixels[src_off..src_off + row_bytes]);
            }
        }

        buf
    }

    /// Access the tile config.
    #[must_use]
    pub fn config(&self) -> &TileConfig {
        &self.config
    }
}

/// Grid of tiles covering a frame buffer.
pub struct TileGrid {
    /// Number of tiles in the X direction.
    pub cols: u32,
    /// Number of tiles in the Y direction.
    pub rows: u32,
    /// Tile config.
    pub config: TileConfig,
}

impl TileGrid {
    /// Create a tile grid for the given frame buffer dimensions.
    #[must_use]
    pub fn new(fb_width: u32, fb_height: u32, config: TileConfig) -> Self {
        // Clamp frame dims and guard zero tile size (div_ceil would panic) so
        // cols/rows stay bounded and well-defined.
        let fb_width = clamp_dim(fb_width);
        let fb_height = clamp_dim(fb_height);
        let tile_size = config.tile_size.max(1);
        let cols = fb_width.div_ceil(tile_size);
        let rows = fb_height.div_ceil(tile_size);
        Self { cols, rows, config }
    }

    /// Total number of tiles in the grid.
    ///
    /// Saturates instead of wrapping; for allocation sizing use
    /// [`TileGrid::total_tiles_usize`].
    #[must_use]
    pub fn total_tiles(&self) -> u32 {
        self.cols.saturating_mul(self.rows)
    }

    /// Total number of tiles as a `usize`, widened so it never wraps even when
    /// `cols * rows` exceeds `u32::MAX`. Use this for allocation sizing.
    #[must_use]
    pub fn total_tiles_usize(&self) -> usize {
        self.cols as usize * self.rows as usize
    }

    /// Convert a linear tile index to (tx, ty) coordinates.
    #[must_use]
    pub fn index_to_coords(&self, index: u32) -> (u32, u32) {
        if self.cols == 0 {
            return (0, 0);
        }
        (index % self.cols, index / self.cols)
    }

    /// Convert (tx, ty) to a linear tile index, computed in `usize` so the
    /// `ty * cols + tx` product cannot wrap into a small in-range-looking index.
    #[must_use]
    pub fn coords_to_index(&self, tx: u32, ty: u32) -> usize {
        ty as usize * self.cols as usize + tx as usize
    }
}
