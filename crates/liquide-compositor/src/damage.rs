//! Damage tracking at surface and tile granularities.
//!
//! The damage tracker maintains CRC-32C hashes of every tile from the
//! previous frame and compares them against the current frame to determine
//! which tiles need re-encoding.
//!
//! # Renderer-Agnostic Damage Contract
//!
//! All renderer implementations (`liquide-renderer-cpu`, `liquide-renderer-wgpu`)
//! MUST classify returned damage tiles according to scene node content:
//!
//! - **TextGlyph** (priority 0): Tiles containing Text or TextCaret nodes.
//!   These are always encoded losslessly.
//! - **UiPrimitive** (priority 1): Tiles with UI elements (buttons, borders,
//!   icons, backgrounds, gradients, shadows, etc.).
//! - **BitmapRegion** (priority 2): Tiles with Surface, ChildSurface, Image,
//!   or BlurCache nodes (photographic or video content).
//! - **CursorOnly** (priority 3): Tiles damaged exclusively by cursor overlay
//!   movement.
//!
//! Structural scene nodes (Root, Workspace, Overlay, Content, ShellLayer,
//! RenderLayer, ClipPath, Filter, BackdropFilter) do NOT contribute to tile
//! classification; only their descendants do.
//!
//! When multiple classified nodes overlap a tile, the highest-priority
//! (lowest numeric value) class wins. Session tile encoding relies on this
//! classification to apply appropriate compression strategies per tile.

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

#[derive(Debug, Clone, Copy)]
struct FullDamage {
    grid_width: u32,
    grid_height: u32,
    class: DamageClass,
}

/// The set of damaged tiles for a single frame.
#[derive(Debug, Clone, Default)]
pub struct DamageSet {
    /// Tile size in pixels (e.g. 64).
    pub tile_size: u32,
    /// List of damaged tiles.
    pub tiles: Vec<DamageTile>,
    full: Option<FullDamage>,
}

impl DamageSet {
    /// Create a new empty damage set.
    #[must_use]
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size,
            tiles: Vec::new(),
            full: None,
        }
    }

    /// Create a damage set from explicit tiles.
    #[must_use]
    pub fn from_tiles(tile_size: u32, tiles: Vec<DamageTile>) -> Self {
        Self {
            tile_size,
            tiles,
            full: None,
        }
    }

    /// Create a lazy full-frame damage set without materializing every tile.
    #[must_use]
    pub fn full(tile_size: u32, grid_width: u32, grid_height: u32, class: DamageClass) -> Self {
        Self {
            tile_size,
            tiles: Vec::new(),
            full: Some(FullDamage {
                grid_width,
                grid_height,
                class,
            }),
        }
    }

    /// Whether this set represents a full-frame refresh.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.full.is_some()
    }

    /// Full-frame grid dimensions, if this is a lazy full-frame refresh.
    #[must_use]
    pub fn full_grid_dimensions(&self) -> Option<(u32, u32, DamageClass)> {
        self.full
            .map(|full| (full.grid_width, full.grid_height, full.class))
    }

    /// Materialize all damaged tiles, expanding a lazy full-frame refresh if needed.
    #[must_use]
    pub fn materialize_tiles(&self) -> Vec<DamageTile> {
        match self.full {
            Some(full) => {
                let mut tiles =
                    Vec::with_capacity((full.grid_width.saturating_mul(full.grid_height)) as usize);
                for y in 0..full.grid_height {
                    for x in 0..full.grid_width {
                        tiles.push(DamageTile {
                            x,
                            y,
                            class: full.class,
                        });
                    }
                }
                tiles
            }
            None => self.tiles.clone(),
        }
    }

    /// Add a damaged tile.
    pub fn add(&mut self, tile: DamageTile) {
        self.full = None;
        self.tiles.push(tile);
    }

    /// Merge another damage set into this one.
    ///
    /// Deduplicates identical (x, y, class) tuples by preferring the
    /// highest-priority (lowest numeric value) damage class when the same
    /// tile coordinate is already present.  Without this the tile encoder
    /// may re-encode the same tile `N` times and the sort-by-priority
    /// step is not stable enough to rescue downstream consumers.
    pub fn merge(&mut self, other: &DamageSet) {
        if let Some((grid_width, grid_height, class)) = self
            .full_grid_dimensions()
            .or_else(|| other.full_grid_dimensions())
        {
            let merged_class = other
                .full_grid_dimensions()
                .map(|(_, _, other_class)| {
                    if other_class.priority() < class.priority() {
                        other_class
                    } else {
                        class
                    }
                })
                .unwrap_or(class);
            self.tiles.clear();
            self.full = Some(FullDamage {
                grid_width,
                grid_height,
                class: merged_class,
            });
            return;
        }

        use std::collections::HashMap;
        // Fast path: current set empty.
        if self.tiles.is_empty() {
            self.tiles.reserve(other.tiles.len());
            // Still dedup within `other` itself.
            let mut seen: HashMap<(u32, u32), DamageClass> =
                HashMap::with_capacity(other.tiles.len());
            for t in &other.tiles {
                let key = (t.x, t.y);
                match seen.get(&key) {
                    Some(existing) if existing.priority() <= t.class.priority() => {}
                    _ => {
                        seen.insert(key, t.class);
                    }
                }
            }
            self.tiles.extend(
                seen.into_iter()
                    .map(|((x, y), class)| DamageTile { x, y, class }),
            );
            return;
        }
        // General path.
        self.full = None;
        let mut seen: HashMap<(u32, u32), DamageClass> =
            HashMap::with_capacity(self.tiles.len() + other.tiles.len());
        for t in self.tiles.iter().chain(other.tiles.iter()) {
            let key = (t.x, t.y);
            match seen.get(&key) {
                Some(existing) if existing.priority() <= t.class.priority() => {}
                _ => {
                    seen.insert(key, t.class);
                }
            }
        }
        self.tiles.clear();
        self.tiles.extend(
            seen.into_iter()
                .map(|((x, y), class)| DamageTile { x, y, class }),
        );
    }

    /// Deduplicate tiles in place, keeping the highest-priority class per
    /// (x, y) coordinate.  Call after a sequence of `mark_*` / `add()` calls.
    pub fn dedup(&mut self) {
        if self.full.is_some() {
            return;
        }
        if self.tiles.len() < 2 {
            return;
        }
        use std::collections::HashMap;
        let mut seen: HashMap<(u32, u32), DamageClass> = HashMap::with_capacity(self.tiles.len());
        for t in self.tiles.iter() {
            let key = (t.x, t.y);
            match seen.get(&key) {
                Some(existing) if existing.priority() <= t.class.priority() => {}
                _ => {
                    seen.insert(key, t.class);
                }
            }
        }
        self.tiles.clear();
        self.tiles.extend(
            seen.into_iter()
                .map(|((x, y), class)| DamageTile { x, y, class }),
        );
    }

    /// Whether the damage set is empty (no tiles changed).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.full.is_none() && self.tiles.is_empty()
    }

    /// Number of damaged tiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.full.map_or_else(
            || self.tiles.len(),
            |full| (full.grid_width.saturating_mul(full.grid_height)) as usize,
        )
    }

    /// Sort tiles by priority (TextGlyph first, CursorOnly last).
    pub fn sort_by_priority(&mut self) {
        if self.full.is_some() {
            return;
        }
        self.tiles.sort_by_key(|t| t.class.priority());
    }

    /// Remove all tiles.
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.full = None;
    }

    /// Mark a single tile as damaged.
    pub fn mark_tile(&mut self, tx: u32, ty: u32) {
        self.mark_tile_with_class(tx, ty, DamageClass::CursorOnly);
    }

    /// Mark a single tile as damaged with a known semantic class.
    pub fn mark_tile_with_class(&mut self, tx: u32, ty: u32, class: DamageClass) {
        self.full = None;
        self.tiles.push(DamageTile {
            x: tx,
            y: ty,
            class,
        });
    }

    /// Mark all tiles overlapping a pixel-coordinate rectangle as damaged.
    pub fn mark_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        grid_width: u32,
        grid_height: u32,
    ) {
        self.mark_rect_with_class(
            x,
            y,
            width,
            height,
            grid_width,
            grid_height,
            DamageClass::UiPrimitive,
        );
    }

    /// Mark all tiles overlapping a pixel-coordinate rectangle as damaged
    /// with a known semantic class.
    pub fn mark_rect_with_class(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        grid_width: u32,
        grid_height: u32,
        class: DamageClass,
    ) {
        if self.tile_size == 0 || width == 0 || height == 0 {
            return;
        }
        self.full = None;
        let tx_start = x / self.tile_size;
        let ty_start = y / self.tile_size;
        let tx_end = (x + width).div_ceil(self.tile_size).min(grid_width);
        let ty_end = (y + height).div_ceil(self.tile_size).min(grid_height);
        for ty in ty_start..ty_end {
            for tx in tx_start..tx_end {
                self.tiles.push(DamageTile {
                    x: tx,
                    y: ty,
                    class,
                });
            }
        }
    }

    /// Mark all tiles in the grid as damaged (full-screen refresh).
    pub fn mark_all(&mut self, grid_width: u32, grid_height: u32) {
        self.mark_all_with_class(grid_width, grid_height, DamageClass::UiPrimitive);
    }

    /// Mark all tiles in the grid as damaged with a known semantic class.
    pub fn mark_all_with_class(&mut self, grid_width: u32, grid_height: u32, class: DamageClass) {
        self.full = None;
        self.tiles.clear();
        for y in 0..grid_height {
            for x in 0..grid_width {
                self.tiles.push(DamageTile { x, y, class });
            }
        }
    }
}

// ── CRC-32C (Castagnoli) ────────────────────────────────────────────────

/// Compute CRC-32C checksum over a byte slice.
///
/// Delegates to the SIMD-accelerated implementation in `liquide_simd`.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    liquide_simd::crc::crc32c(data)
}

/// Compute CRC-32C for a tile region within a frame buffer.
/// Returns 0 if the tile is out of bounds.
///
/// Delegates to the SIMD-accelerated implementation in `liquide_simd`.
#[must_use]
pub fn crc32c_tile(fb: &FrameBuffer, tile_x: u32, tile_y: u32, tile_size: u32) -> u32 {
    liquide_simd::crc::crc32c_tile(
        fb.pixels(),
        fb.stride,
        tile_x,
        tile_y,
        tile_size,
        fb.width,
        fb.height,
        fb.format.bytes_per_pixel(),
    )
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
        self.compute_damage_with_class(fb, DamageClass::UiPrimitive)
    }

    /// Compute damage using the provided class for changed tiles.
    pub fn compute_damage_with_class(&mut self, fb: &FrameBuffer, class: DamageClass) -> DamageSet {
        if self.first_frame {
            for ty in 0..self.grid_height {
                for tx in 0..self.grid_width {
                    let idx = (ty * self.grid_width + tx) as usize;
                    self.previous_hashes[idx] = crc32c_tile(fb, tx, ty, self.tile_size);
                }
            }
            self.first_frame = false;
            return DamageSet::full(self.tile_size, self.grid_width, self.grid_height, class);
        }

        let mut damage = DamageSet::new(self.tile_size);

        for ty in 0..self.grid_height {
            for tx in 0..self.grid_width {
                let idx = (ty * self.grid_width + tx) as usize;
                let hash = crc32c_tile(fb, tx, ty, self.tile_size);

                if hash != self.previous_hashes[idx] {
                    damage.add(DamageTile {
                        x: tx,
                        y: ty,
                        class,
                    });
                }

                self.previous_hashes[idx] = hash;
            }
        }

        if damage.len() as u32 >= self.grid_width.saturating_mul(self.grid_height) {
            DamageSet::full(self.tile_size, self.grid_width, self.grid_height, class)
        } else {
            damage
        }
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
