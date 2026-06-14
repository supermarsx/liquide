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
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

// ── Clip complexity (B7) ────────────────────────────────────────────────
//
// A clip-complexity classifier for the compositor damage / paint fallback.
//
// The display-list / partial-damage fast path is only sound while the active
// clip stays a single axis-aligned rectangle: tile damage is computed against
// rectangular tile bounds, and a single clip rect simply trims those bounds.
// Once the effective clip becomes a multi-rect region (overlapping nested
// clips, rounded-corner clips reduced to several rects, etc.) the per-tile
// "is this pixel clipped?" test stops being a cheap rectangle check and the
// incremental-damage bookkeeping can disagree with what the renderer actually
// paints — the classic source of clipped-edge flicker. In that case the safe,
// roadmap-Feature-4.1 behaviour is to fall back to a full-layer (full-frame)
// refresh rather than trust partial tile damage.
//
// This is a clean-room helper: it models the well-known three-tier
// "no clip / one rect / many rects" classification from public compositor and
// GDI literature. It carries no leaked identifiers, tables, or constants.

/// Default number of effective clip rectangles beyond which the partial-damage
/// fast path is abandoned in favour of a full-frame refresh.
///
/// One rectangle is always cheap; two or more rectangles already require
/// per-tile multi-rect testing, so the default threshold is `1` (i.e. anything
/// strictly more complex than a single rect escalates).
pub const DEFAULT_CLIP_COMPLEXITY_THRESHOLD: usize = 1;

/// Three-tier clip-complexity classification used to gate the compositor's
/// partial-damage fast path.
///
/// * `Trivial` — no clipping is active; every damaged tile is fully visible.
/// * `Rect` — a single rectangular clip; tile bounds can be trimmed cheaply.
/// * `Complex(n)` — `n` (`>= 2`) effective clip rectangles; the partial-damage
///   path is no longer a cheap rectangle test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipComplexity {
    /// No clip is active.
    Trivial,
    /// Exactly one rectangular clip is active.
    Rect,
    /// `n >= 2` effective clip rectangles are active.
    Complex(usize),
}

impl ClipComplexity {
    /// Classify a slice of effective clip rectangles into a complexity tier.
    ///
    /// Empty (zero-area) rectangles are ignored: they contribute no visible
    /// clipping and an all-empty clip set is treated as `Trivial` here (the
    /// caller decides separately whether an empty clip means "draw nothing").
    #[must_use]
    pub fn classify(clip_rects: &[Rect]) -> Self {
        let non_empty = clip_rects
            .iter()
            .filter(|r| r.width > 0.0 && r.height > 0.0)
            .count();
        match non_empty {
            0 => Self::Trivial,
            1 => Self::Rect,
            n => Self::Complex(n),
        }
    }

    /// The number of effective clip rectangles this tier represents.
    #[must_use]
    pub fn rect_count(&self) -> usize {
        match self {
            Self::Trivial => 0,
            Self::Rect => 1,
            Self::Complex(n) => *n,
        }
    }

    /// Whether the clip is complex enough that `rect_count()` strictly exceeds
    /// `threshold`.
    #[must_use]
    pub fn exceeds_threshold(&self, threshold: usize) -> bool {
        self.rect_count() > threshold
    }

    /// Whether the partial-damage fast path should be abandoned for a full
    /// refresh, using [`DEFAULT_CLIP_COMPLEXITY_THRESHOLD`].
    #[must_use]
    pub fn should_fall_back_to_full(&self) -> bool {
        self.exceeds_threshold(DEFAULT_CLIP_COMPLEXITY_THRESHOLD)
    }
}

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

    /// Upgrade the lazy full-frame class to the highest priority (lowest
    /// numeric) of the current full class and `class`.
    ///
    /// Returns `true` if the set is currently a lazy full-frame refresh (in
    /// which case the caller must NOT push a tile, as full coverage already
    /// includes it and pushing would not increase coverage). Returns `false`
    /// if the set is not full and the caller should add the tile normally.
    ///
    /// Adding damage must never *reduce* damage: a full-frame set stays full,
    /// and the strongest class seen wins so a higher-priority tile (e.g.
    /// `TextGlyph`) decorating a full `UiPrimitive` frame upgrades the whole
    /// frame's class rather than being dropped.
    fn upgrade_full_class(&mut self, class: DamageClass) -> bool {
        if let Some(full) = self.full.as_mut() {
            if class.priority() < full.class.priority() {
                full.class = class;
            }
            true
        } else {
            false
        }
    }

    /// Add a damaged tile.
    pub fn add(&mut self, tile: DamageTile) {
        // Preserve full-frame coverage: if already full, keep it full and only
        // upgrade the class. Never downgrade full damage to a single tile.
        if self.upgrade_full_class(tile.class) {
            return;
        }
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
            // The result stays a full-frame refresh, but the class must reflect
            // the highest priority (lowest numeric) class present on EITHER
            // side — including tile-level classes, not just the full classes.
            // Otherwise a `TextGlyph` tile merged into a full `UiPrimitive`
            // frame would silently lose its priority, violating the
            // "TextGlyph always encoded losslessly" contract on full frames.
            let mut merged_class = class;
            let mut consider = |c: DamageClass| {
                if c.priority() < merged_class.priority() {
                    merged_class = c;
                }
            };
            if let Some((_, _, self_full_class)) = self.full_grid_dimensions() {
                consider(self_full_class);
            }
            if let Some((_, _, other_full_class)) = other.full_grid_dimensions() {
                consider(other_full_class);
            }
            for t in self.tiles.iter().chain(other.tiles.iter()) {
                consider(t.class);
            }
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
        // Preserve full-frame coverage; only upgrade the class if needed.
        if self.upgrade_full_class(class) {
            return;
        }
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
        // Preserve full-frame coverage; only upgrade the class if needed. A
        // rect within an already-full frame is already covered.
        if self.upgrade_full_class(class) {
            return;
        }
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

    /// Escalate to a full-frame refresh when the active clip is too complex for
    /// the partial-damage fast path (B7, roadmap Feature 4.1 full-layer
    /// fallback).
    ///
    /// `clip_rects` are the effective clip rectangles active for the damaged
    /// content this frame. When their [`ClipComplexity`] exceeds
    /// [`DEFAULT_CLIP_COMPLEXITY_THRESHOLD`] (i.e. two or more rectangles), the
    /// per-tile clip test is no longer a cheap rectangle trim and partial tile
    /// damage can disagree with what the renderer paints, so this promotes the
    /// set to a lazy full-frame refresh covering the whole grid. The full
    /// frame's class is the highest priority (lowest numeric) class already
    /// present, falling back to `default_class` for an otherwise-empty set, so
    /// escalation never *downgrades* the encoding contract.
    ///
    /// Returns `true` if the set was escalated to (or already was) full-frame.
    /// A trivial or single-rect clip leaves the set untouched and returns
    /// whatever `is_full()` already was.
    pub fn escalate_for_clip_complexity(
        &mut self,
        clip_rects: &[Rect],
        grid_width: u32,
        grid_height: u32,
        default_class: DamageClass,
    ) -> bool {
        if self.is_full() {
            return true;
        }
        if !ClipComplexity::classify(clip_rects).should_fall_back_to_full() {
            return false;
        }
        // Preserve the strongest class already present so a full-frame
        // escalation never weakens (e.g.) a TextGlyph tile to UiPrimitive.
        let mut class = default_class;
        for t in &self.tiles {
            if t.class.priority() < class.priority() {
                class = t.class;
            }
        }
        self.tiles.clear();
        self.full = Some(FullDamage {
            grid_width,
            grid_height,
            class,
        });
        true
    }

    /// Auto-promote a partial damage set to a lazy full-frame refresh when its
    /// damaged-tile coverage meets or exceeds the whole grid (HIGH-005).
    ///
    /// `merge`/`add` on two partial sets carry no grid dimensions, so a
    /// sequence of merges can accumulate a tile count that equals (or, with
    /// out-of-grid or duplicate coordinates that survive a coarse count,
    /// exceeds) the grid size while the set still *claims* to be partial. A
    /// partial set that no longer fits the grid under-damages downstream: the
    /// session may skip clearing/painting tiles it believes are untouched.
    ///
    /// This deduplicates first (so the count reflects genuine unique tiles),
    /// then, if the unique tile count is at least `grid_width * grid_height`,
    /// collapses to a full-frame refresh whose class preserves the strongest
    /// (lowest-priority-number) class already present, falling back to
    /// `default_class` for an empty set. Escalation therefore never downgrades
    /// the encoding contract. Returns `true` if the set is full afterwards.
    pub fn escalate_if_saturated(
        &mut self,
        grid_width: u32,
        grid_height: u32,
        default_class: DamageClass,
    ) -> bool {
        if self.is_full() {
            return true;
        }
        // Count unique tiles, not raw pushes: duplicates must not inflate
        // coverage and trigger a spurious full refresh.
        self.dedup();
        let grid_tiles = grid_width.saturating_mul(grid_height);
        if grid_tiles == 0 || (self.tiles.len() as u64) < u64::from(grid_tiles) {
            return false;
        }
        let mut class = default_class;
        for t in &self.tiles {
            if t.class.priority() < class.priority() {
                class = t.class;
            }
        }
        self.tiles.clear();
        self.full = Some(FullDamage {
            grid_width,
            grid_height,
            class,
        });
        true
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

#[cfg(test)]
mod full_frame_damage_tests {
    use super::*;

    // Regression for t49-e1-F13: adding a tile to a lazy full-frame set must
    // NOT downgrade it to a single tile. Full coverage must survive `add`,
    // `mark_tile_with_class`, and `mark_rect_with_class`.
    #[test]
    fn full_frame_survives_add() {
        let mut set = DamageSet::full(64, 4, 4, DamageClass::UiPrimitive);
        assert!(set.is_full());
        set.add(DamageTile {
            x: 1,
            y: 1,
            class: DamageClass::CursorOnly,
        });
        assert!(set.is_full(), "add() must not drop full-frame coverage");
        // 4x4 grid = 16 tiles still fully damaged.
        assert_eq!(set.len(), 16);
    }

    #[test]
    fn full_frame_survives_mark_tile_and_rect() {
        let mut set = DamageSet::full(64, 4, 4, DamageClass::UiPrimitive);
        set.mark_tile_with_class(0, 0, DamageClass::CursorOnly);
        assert!(set.is_full(), "mark_tile must not drop full-frame coverage");

        set.mark_rect_with_class(0, 0, 64, 64, 4, 4, DamageClass::CursorOnly);
        assert!(set.is_full(), "mark_rect must not drop full-frame coverage");
        assert_eq!(set.len(), 16);
    }

    // Adding a higher-priority tile to a full frame upgrades the frame's class
    // rather than losing the priority (also part of F13/F14 contract).
    #[test]
    fn full_frame_add_upgrades_class() {
        let mut set = DamageSet::full(64, 2, 2, DamageClass::UiPrimitive);
        set.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::TextGlyph,
        });
        assert!(set.is_full());
        let (_, _, class) = set.full_grid_dimensions().unwrap();
        assert_eq!(
            class,
            DamageClass::TextGlyph,
            "higher-priority tile must upgrade the full-frame class"
        );
    }

    // Regression for t49-e1-F14: merging a tile-level TextGlyph damage set into
    // a full UiPrimitive frame must keep TextGlyph priority — the documented
    // "TextGlyph always encoded losslessly" contract on full frames.
    #[test]
    fn merge_full_keeps_highest_tile_priority() {
        let mut full = DamageSet::full(64, 4, 4, DamageClass::UiPrimitive);
        let mut text = DamageSet::new(64);
        text.mark_tile_with_class(2, 2, DamageClass::TextGlyph);

        full.merge(&text);

        assert!(full.is_full(), "merge result must remain full-frame");
        let (_, _, class) = full.full_grid_dimensions().unwrap();
        assert_eq!(
            class,
            DamageClass::TextGlyph,
            "merging a TextGlyph tile into a full UiPrimitive frame must keep \
             TextGlyph priority"
        );
    }

    // Symmetric case: a full TextGlyph frame must not be downgraded when a
    // lower-priority tile set is merged in.
    #[test]
    fn merge_full_not_downgraded_by_lower_priority() {
        let mut full = DamageSet::full(64, 4, 4, DamageClass::TextGlyph);
        let mut ui = DamageSet::new(64);
        ui.mark_tile_with_class(0, 0, DamageClass::UiPrimitive);

        full.merge(&ui);

        assert!(full.is_full());
        let (_, _, class) = full.full_grid_dimensions().unwrap();
        assert_eq!(class, DamageClass::TextGlyph);
    }
}

#[cfg(test)]
mod clip_complexity_tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn classify_no_clip_is_trivial() {
        assert_eq!(ClipComplexity::classify(&[]), ClipComplexity::Trivial);
        assert_eq!(ClipComplexity::Trivial.rect_count(), 0);
        assert!(!ClipComplexity::Trivial.should_fall_back_to_full());
    }

    #[test]
    fn classify_single_rect_is_rect() {
        let c = ClipComplexity::classify(&[r(0.0, 0.0, 100.0, 100.0)]);
        assert_eq!(c, ClipComplexity::Rect);
        assert_eq!(c.rect_count(), 1);
        // A single rect is still the cheap path — no full fallback.
        assert!(!c.should_fall_back_to_full());
    }

    #[test]
    fn classify_multi_rect_is_complex_and_falls_back() {
        let c = ClipComplexity::classify(&[r(0.0, 0.0, 50.0, 50.0), r(60.0, 60.0, 50.0, 50.0)]);
        assert_eq!(c, ClipComplexity::Complex(2));
        assert_eq!(c.rect_count(), 2);
        assert!(c.should_fall_back_to_full());
        assert!(c.exceeds_threshold(1));
        assert!(!c.exceeds_threshold(2));
    }

    #[test]
    fn classify_ignores_empty_rects() {
        // Two zero-area rects + one real rect ⇒ effectively a single rect.
        let c = ClipComplexity::classify(&[
            r(0.0, 0.0, 0.0, 10.0),
            r(10.0, 10.0, 20.0, 20.0),
            r(30.0, 30.0, 5.0, 0.0),
        ]);
        assert_eq!(c, ClipComplexity::Rect);
    }

    #[test]
    fn escalate_trivial_clip_leaves_partial_damage_untouched() {
        let mut set = DamageSet::new(64);
        set.mark_tile_with_class(1, 1, DamageClass::UiPrimitive);
        let escalated = set.escalate_for_clip_complexity(&[], 4, 4, DamageClass::UiPrimitive);
        assert!(!escalated);
        assert!(!set.is_full());
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn escalate_single_rect_clip_leaves_partial_damage_untouched() {
        let mut set = DamageSet::new(64);
        set.mark_tile_with_class(1, 1, DamageClass::UiPrimitive);
        let escalated = set.escalate_for_clip_complexity(
            &[r(0.0, 0.0, 200.0, 200.0)],
            4,
            4,
            DamageClass::UiPrimitive,
        );
        assert!(!escalated);
        assert!(!set.is_full());
    }

    #[test]
    fn escalate_complex_clip_promotes_to_full_frame() {
        let mut set = DamageSet::new(64);
        set.mark_tile_with_class(1, 1, DamageClass::UiPrimitive);
        let escalated = set.escalate_for_clip_complexity(
            &[r(0.0, 0.0, 50.0, 50.0), r(60.0, 60.0, 50.0, 50.0)],
            4,
            4,
            DamageClass::UiPrimitive,
        );
        assert!(escalated);
        assert!(set.is_full());
        // 4x4 grid is now fully damaged.
        assert_eq!(set.len(), 16);
    }

    #[test]
    fn escalate_complex_clip_preserves_strongest_class() {
        // A TextGlyph tile present before escalation must keep its priority on
        // the resulting full frame, not be weakened to the UiPrimitive default.
        let mut set = DamageSet::new(64);
        set.mark_tile_with_class(0, 0, DamageClass::UiPrimitive);
        set.mark_tile_with_class(2, 2, DamageClass::TextGlyph);
        let escalated = set.escalate_for_clip_complexity(
            &[r(0.0, 0.0, 50.0, 50.0), r(60.0, 60.0, 50.0, 50.0)],
            4,
            4,
            DamageClass::UiPrimitive,
        );
        assert!(escalated);
        assert!(set.is_full());
        let (_, _, class) = set.full_grid_dimensions().unwrap();
        assert_eq!(class, DamageClass::TextGlyph);
    }

    #[test]
    fn escalate_already_full_stays_full() {
        let mut set = DamageSet::full(64, 4, 4, DamageClass::TextGlyph);
        let escalated = set.escalate_for_clip_complexity(
            &[r(0.0, 0.0, 50.0, 50.0), r(60.0, 60.0, 50.0, 50.0)],
            4,
            4,
            DamageClass::UiPrimitive,
        );
        assert!(escalated);
        assert!(set.is_full());
        // Class must not be downgraded.
        let (_, _, class) = set.full_grid_dimensions().unwrap();
        assert_eq!(class, DamageClass::TextGlyph);
    }
}
