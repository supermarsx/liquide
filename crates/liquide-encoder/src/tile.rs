//! Tile types and grid management for the encoding pipeline.

use serde::{Deserialize, Serialize};

use liquide_compositor::damage::DamageClass;

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
        }
    }

    /// Number of non-skip tiles.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.tiles.iter().filter(|t| t.encoding != TileEncoding::Skip).count()
    }

    /// Compression ratio (compressed / uncompressed). Returns 0.0 if uncompressed is zero.
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_bytes == 0 {
            return 0.0;
        }
        self.compressed_bytes as f64 / self.uncompressed_bytes as f64
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
    #[must_use]
    pub fn tile_bytes(&self) -> usize {
        (self.tile_size * self.tile_size * self.bpp) as usize
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
        let ts = self.config.tile_size;
        let bpp = self.config.bpp;
        let tile_bytes = self.config.tile_bytes();
        let mut buf = vec![0u8; tile_bytes];

        let px_x = tx * ts;
        let px_y = ty * ts;

        let rows = ts.min(fb_height.saturating_sub(px_y));
        let cols = ts.min(fb_width.saturating_sub(px_x));
        let row_bytes = cols as usize * bpp as usize;

        for row in 0..rows {
            let src_off = ((px_y + row) * stride) as usize + px_x as usize * bpp as usize;
            let dst_off = row as usize * ts as usize * bpp as usize;
            buf[dst_off..dst_off + row_bytes]
                .copy_from_slice(&pixels[src_off..src_off + row_bytes]);
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
        let cols = fb_width.div_ceil(config.tile_size);
        let rows = fb_height.div_ceil(config.tile_size);
        Self { cols, rows, config }
    }

    /// Total number of tiles in the grid.
    #[must_use]
    pub fn total_tiles(&self) -> u32 {
        self.cols * self.rows
    }

    /// Convert a linear tile index to (tx, ty) coordinates.
    #[must_use]
    pub fn index_to_coords(&self, index: u32) -> (u32, u32) {
        (index % self.cols, index / self.cols)
    }

    /// Convert (tx, ty) to a linear tile index.
    #[must_use]
    pub fn coords_to_index(&self, tx: u32, ty: u32) -> u32 {
        ty * self.cols + tx
    }
}
