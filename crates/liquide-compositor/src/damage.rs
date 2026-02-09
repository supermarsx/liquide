//! Damage tracking at surface and tile granularities.
//!
//! The damage tracker maintains CRC-32C hashes of every tile from the
//! previous frame and compares them against the current frame to determine
//! which tiles need re-encoding.

use crate::framebuffer::FrameBuffer;
use serde::{Deserialize, Serialize};

/// Damage classification for a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageClass {
    /// Tile contains text rendering (highest priority, always lossless).
    TextGlyph,
    /// Tile contains UI elements (buttons, borders, icons).
    UiPrimitive,
    /// Tile contains photographic / video content.
    BitmapRegion,
    /// Tile was only damaged by cursor overlay movement.
    CursorOnly,
}

impl DamageClass {
    /// Priority ordering (lower number = higher priority in encoding order).
    #[must_use]
    pub fn priority(&self) -> u8 {
        match self {
            Self::TextGlyph => 0,
            Self::UiPrimitive => 1,
            Self::BitmapRegion => 2,
            Self::CursorOnly => 3,
        }
    }
}

/// A single damaged tile in the tile grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageTile {
    /// Tile grid column (0-based).
    pub x: u32,
    /// Tile grid row (0-based).
    pub y: u32,
    /// Damage classification.
    pub class: DamageClass,
}

/// The set of damaged tiles for a single frame.
#[derive(Debug, Clone, Default)]
pub struct DamageSet {
    /// Tile size in pixels (e.g. 64).
    pub tile_size: u32,
    /// List of damaged tiles.
    pub tiles: Vec<DamageTile>,
}

impl DamageSet {
    /// Create a new empty damage set.
    #[must_use]
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size,
            tiles: Vec::new(),
        }
    }

    /// Add a damaged tile.
    pub fn add(&mut self, tile: DamageTile) {
        self.tiles.push(tile);
    }

    /// Merge another damage set into this one.
    pub fn merge(&mut self, other: &DamageSet) {
        self.tiles.extend_from_slice(&other.tiles);
    }

    /// Whether the damage set is empty (no tiles changed).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Number of damaged tiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Sort tiles by priority (TextGlyph first, CursorOnly last).
    pub fn sort_by_priority(&mut self) {
        self.tiles.sort_by_key(|t| t.class.priority());
    }

    /// Remove all tiles.
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// Mark all tiles in the grid as damaged (full-screen refresh).
    pub fn mark_all(&mut self, grid_width: u32, grid_height: u32) {
        self.tiles.clear();
        for y in 0..grid_height {
            for x in 0..grid_width {
                self.tiles.push(DamageTile {
                    x,
                    y,
                    class: DamageClass::UiPrimitive,
                });
            }
        }
    }
}

// ── CRC-32C (Castagnoli) ────────────────────────────────────────────────

/// CRC-32C lookup table (Castagnoli polynomial 0x1EDC6F41).
const CRC32C_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x82F6_3B78; // reversed polynomial
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute CRC-32C checksum over a byte slice.
// TODO: SIMD CRC-32C using SSE4.2 _mm_crc32_u64 intrinsic
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc = CRC32C_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Compute CRC-32C for a tile region within a frame buffer.
#[must_use]
pub fn crc32c_tile(fb: &FrameBuffer, tile_x: u32, tile_y: u32, tile_size: u32) -> u32 {
    let bpp = fb.format.bytes_per_pixel();
    let px = tile_x * tile_size;
    let py = tile_y * tile_size;
    let tw = tile_size.min(fb.width.saturating_sub(px));
    let th = tile_size.min(fb.height.saturating_sub(py));

    let mut crc = !0u32;
    for row in 0..th {
        let offset = ((py + row) * fb.stride + px * bpp) as usize;
        let end = offset + (tw * bpp) as usize;
        for &byte in &fb.pixels[offset..end] {
            crc = CRC32C_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
    }
    !crc
}

/// Tracks tile-level damage between frames using CRC-32C hash comparison.
pub struct DamageTracker {
    tile_size: u32,
    grid_width: u32,
    grid_height: u32,
    /// CRC-32C hashes of the previous frame's tiles.
    previous_hashes: Vec<u32>,
    /// Whether this is the first frame (forces full damage).
    first_frame: bool,
}

impl DamageTracker {
    /// Create a new damage tracker.
    #[must_use]
    pub fn new(tile_size: u32, screen_width: u32, screen_height: u32) -> Self {
        let grid_width = screen_width.div_ceil(tile_size);
        let grid_height = screen_height.div_ceil(tile_size);
        let tile_count = (grid_width * grid_height) as usize;
        Self {
            tile_size,
            grid_width,
            grid_height,
            previous_hashes: vec![0; tile_count],
            first_frame: true,
        }
    }

    /// Compute damage by hashing every tile and comparing against previous.
    pub fn compute_damage(&mut self, fb: &FrameBuffer) -> DamageSet {
        let mut damage = DamageSet::new(self.tile_size);

        for ty in 0..self.grid_height {
            for tx in 0..self.grid_width {
                let idx = (ty * self.grid_width + tx) as usize;
                let hash = crc32c_tile(fb, tx, ty, self.tile_size);

                if self.first_frame || hash != self.previous_hashes[idx] {
                    damage.add(DamageTile {
                        x: tx,
                        y: ty,
                        // Default classification; the renderer can reclassify
                        class: DamageClass::UiPrimitive,
                    });
                }

                self.previous_hashes[idx] = hash;
            }
        }

        self.first_frame = false;
        damage
    }

    /// Reset damage tracking (forces full damage on next frame).
    pub fn reset(&mut self) {
        self.first_frame = true;
        self.previous_hashes.fill(0);
    }

    /// Resize the tracked region.
    pub fn resize(&mut self, screen_width: u32, screen_height: u32) {
        self.grid_width = screen_width.div_ceil(self.tile_size);
        self.grid_height = screen_height.div_ceil(self.tile_size);
        let tile_count = (self.grid_width * self.grid_height) as usize;
        self.previous_hashes = vec![0; tile_count];
        self.first_frame = true;
    }

    /// Tile grid width.
    #[must_use]
    pub fn grid_width(&self) -> u32 {
        self.grid_width
    }

    /// Tile grid height.
    #[must_use]
    pub fn grid_height(&self) -> u32 {
        self.grid_height
    }

    /// Tile size in pixels.
    #[must_use]
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }
}
