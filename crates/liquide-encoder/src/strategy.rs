//! Encoding strategy selection for tile encoding.
//!
//! Given the current and previous tile data, CRC-32C hash, and damage class,
//! this module determines the best encoding strategy: Skip, Delta, Full, or
//! Solid. The `Copy` strategy is intentionally disabled until the wire protocol
//! explicitly defines whether copy references target previous-frame or
//! same-frame tile state.
//!
//! Additionally selects the compression method (Zstd vs LZ4) based on
//! the damage class of each tile: cursor-only tiles use LZ4 for lower
//! latency, text tiles use Zstd for better ratio.

use std::collections::HashMap;

use crate::delta;

use liquide_compositor::damage::DamageClass;

/// Compression method for a tile payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompressionMethod {
    /// Zstd compression at a given level (1–22). Better ratio, higher latency.
    Zstd { level: i32 },
    /// LZ4 block compression. Faster encode/decode, lower ratio.
    Lz4,
}

impl Default for CompressionMethod {
    fn default() -> Self {
        Self::Zstd { level: 3 }
    }
}

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
    /// Reserved for explicit copy-reference semantics.
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
    /// Default Zstd compression level.
    pub zstd_level: i32,
    /// Whether to use LZ4 for latency-sensitive tiles.
    pub use_lz4_for_cursor: bool,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            delta_threshold: 0.5,
            copy_min_tiles: 2,
            zstd_level: 3,
            use_lz4_for_cursor: true,
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
/// - `_copy_index`: reserved CRC → linear tile index for future copy semantics
/// - `damage_class`: the damage classification for this tile
/// - `config`: strategy selection thresholds
#[must_use]
pub fn choose_strategy(
    current: &[u8],
    previous: Option<&[u8]>,
    current_crc: u32,
    prev_crc: Option<u32>,
    _copy_index: &HashMap<u32, u32>,
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

    // 3. Delta vs Full: compare change ratio
    if let Some(prev) = previous {
        let xor = delta::xor_delta(current, prev);
        let ratio = delta::change_ratio(&xor);
        if ratio < config.delta_threshold {
            return EncodingStrategy::Delta;
        }
    }

    // 4. Default: full tile
    EncodingStrategy::Full
}

/// Choose the compression method for a tile based on its damage class.
///
/// - `CursorOnly`: use LZ4 (latency matters more than ratio)
/// - `TextGlyph`: use Zstd (text compresses well, ratio matters)
/// - `BitmapRegion`: use LZ4 if under budget pressure, Zstd otherwise
/// - `UiPrimitive`: use Zstd (good balance)
#[must_use]
pub fn choose_compression(
    damage_class: DamageClass,
    config: &StrategyConfig,
    under_budget_pressure: bool,
) -> CompressionMethod {
    match damage_class {
        DamageClass::CursorOnly if config.use_lz4_for_cursor => CompressionMethod::Lz4,
        DamageClass::TextGlyph => CompressionMethod::Zstd {
            level: config.zstd_level,
        },
        DamageClass::BitmapRegion if under_budget_pressure => CompressionMethod::Lz4,
        _ => CompressionMethod::Zstd {
            level: config.zstd_level,
        },
    }
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
