//! Encoding strategy selection for tile encoding.
//!
//! Given the current and previous tile data, CRC-32C hash, and damage class,
//! this module determines the best encoding strategy: Skip, Delta, Full,
//! Copy (content-addressable), or Solid.

use std::collections::HashMap;

use crate::delta;

use liquide_compositor::damage::DamageClass;

/// Encoding strategy — identical to `TileEncoding` but used before
/// payload construction to guide the encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingStrategy {
    /// Tile unchanged — skip entirely.
    Skip,
    /// Use XOR delta against previous frame.
    Delta,
    /// Send full tile data.
    Full,
    /// Copy from another tile in this frame (by CRC index).
    Copy { source_index: u32 },
    /// Tile is a single solid color (4 bytes).
    Solid { bgra: [u8; 4] },
}

/// Configuration for strategy selection thresholds.
#[derive(Debug, Clone)]
pub struct StrategyConfig {
    /// XOR change ratio below which delta encoding is preferred over full.
    pub delta_threshold: f32,
    /// Minimum number of tiles needed to consider copy deduplication.
    pub copy_min_tiles: usize,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            delta_threshold: 0.5,
            copy_min_tiles: 2,
        }
    }
}

/// Choose the best encoding strategy for a tile.
///
/// # Arguments
/// - `current`: current tile pixel data
/// - `previous`: previous frame's tile data (same coordinates), or `None` for first frame
/// - `current_crc`: CRC-32C of the current tile
/// - `prev_crc`: CRC-32C of the previous tile at the same coordinates
/// - `copy_index`: maps CRC → linear tile index for content-addressable copy
/// - `damage_class`: the damage classification for this tile
/// - `config`: strategy selection thresholds
#[must_use]
pub fn choose_strategy(
    current: &[u8],
    previous: Option<&[u8]>,
    current_crc: u32,
    prev_crc: Option<u32>,
    copy_index: &HashMap<u32, u32>,
    _damage_class: DamageClass,
    config: &StrategyConfig,
) -> EncodingStrategy {
    // 1. Skip: CRC matches previous frame
    if let Some(pc) = prev_crc {
        if current_crc == pc {
            return EncodingStrategy::Skip;
        }
    }

    // 2. Solid: all pixels are the same color
    if let Some(color) = detect_solid(current) {
        return EncodingStrategy::Solid { bgra: color };
    }

    // 3. Copy: another tile in this frame has the same CRC
    if let Some(&source_idx) = copy_index.get(&current_crc) {
        return EncodingStrategy::Copy { source_index: source_idx };
    }

    // 4. Delta vs Full: compare change ratio
    if let Some(prev) = previous {
        let xor = delta::xor_delta(current, prev);
        let ratio = delta::change_ratio(&xor);
        if ratio < config.delta_threshold {
            return EncodingStrategy::Delta;
        }
    }

    // 5. Default: full tile
    EncodingStrategy::Full
}

/// Check if a tile is a solid color. Returns the BGRA bytes if all pixels match.
#[must_use]
pub fn detect_solid(tile_data: &[u8]) -> Option<[u8; 4]> {
    if tile_data.len() < 4 || tile_data.len() % 4 != 0 {
        return None;
    }
    let first = [tile_data[0], tile_data[1], tile_data[2], tile_data[3]];
    for chunk in tile_data.chunks_exact(4) {
        if chunk != first {
            return None;
        }
    }
    Some(first)
}

/// Build a content-addressable copy index from the current frame's tiles.
///
/// Maps CRC-32C → first tile index with that CRC. Only tiles appearing
/// more than once are useful for copy deduplication.
#[must_use]
pub fn build_copy_index(tile_crcs: &[u32]) -> HashMap<u32, u32> {
    let mut first_seen: HashMap<u32, u32> = HashMap::new();
    let mut duplicates: HashMap<u32, u32> = HashMap::new();

    for (i, &crc) in tile_crcs.iter().enumerate() {
        if let Some(&first_idx) = first_seen.get(&crc) {
            duplicates.entry(crc).or_insert(first_idx);
        } else {
            first_seen.insert(crc, i as u32);
        }
    }
    duplicates
}
