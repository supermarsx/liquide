//! Main tile encoder: orchestrates the full encoding pipeline.
//!
//! Pipeline: extract tile → CRC-32C hash → choose strategy →
//! XOR delta (if applicable) → compress → cache → produce `TileBatch`.

use std::collections::HashMap;

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;

use crate::cache::TilePayloadCache;
use crate::compress;
use crate::delta;
use crate::hash;
use crate::strategy::{self, EncodingStrategy, StrategyConfig};
use crate::tile::{TileBatch, TileCodec, TileConfig, TileEncoding, TileGrid, TileUpdate};

/// The tile encoder — stateful across frames for delta and cache.
pub struct TileEncoder {
    /// Tile grid geometry.
    grid: TileGrid,
    /// Tile codec for extraction.
    codec: TileCodec,
    /// Per-tile CRC from the previous frame (indexed by linear tile index).
    prev_crcs: Vec<u32>,
    /// Per-tile raw data from the previous frame.
    prev_tiles: Vec<Vec<u8>>,
    /// Payload cache for content-addressable deduplication.
    cache: TilePayloadCache,
    /// Strategy configuration.
    strategy_config: StrategyConfig,
    /// Frame sequence counter.
    sequence: u64,
    /// Zstd compression level.
    zstd_level: i32,
}

impl TileEncoder {
    /// Create a new tile encoder for the given frame buffer dimensions.
    #[must_use]
    pub fn new(fb_width: u32, fb_height: u32, config: TileConfig) -> Self {
        let grid = TileGrid::new(fb_width, fb_height, config.clone());
        let total = grid.total_tiles() as usize;
        Self {
            codec: TileCodec::new(config),
            grid,
            prev_crcs: vec![0; total],
            prev_tiles: vec![Vec::new(); total],
            cache: TilePayloadCache::new(2048),
            strategy_config: StrategyConfig::default(),
            sequence: 0,
            zstd_level: 3,
        }
    }

    /// Encode one frame. Only tiles listed in `damage_tiles` are processed;
    /// all others are assumed unchanged (Skip).
    pub fn encode_frame(
        &mut self,
        fb: &FrameBuffer,
        damage_tiles: &[DamageTile],
    ) -> crate::Result<TileBatch> {
        self.sequence += 1;
        self.cache.advance_frame();

        let mut batch = TileBatch::new(self.sequence);

        // Build set of damaged tile coordinates for quick lookup
        let damaged: HashMap<(u32, u32), DamageClass> = damage_tiles
            .iter()
            .map(|dt| ((dt.x, dt.y), dt.class))
            .collect();

        // Extract current tiles and compute CRCs for damaged tiles
        let mut current_crcs: Vec<u32> = self.prev_crcs.clone();
        let mut current_tiles: Vec<Option<Vec<u8>>> = vec![None; self.grid.total_tiles() as usize];

        for dt in damage_tiles {
            let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
            let tile_data = self.codec.extract_tile(
                &fb.pixels,
                fb.stride,
                fb.width,
                fb.height,
                dt.x,
                dt.y,
            );
            let crc = hash::crc32c(&tile_data);
            current_crcs[idx] = crc;
            current_tiles[idx] = Some(tile_data);
        }

        // Build copy index from all current CRCs
        let copy_index = strategy::build_copy_index(&current_crcs);

        // Encode each damaged tile
        for dt in damage_tiles {
            let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
            let current = current_tiles[idx].as_ref().unwrap();
            let crc = current_crcs[idx];
            let prev_crc = if self.prev_tiles[idx].is_empty() {
                None
            } else {
                Some(self.prev_crcs[idx])
            };
            let previous = if self.prev_tiles[idx].is_empty() {
                None
            } else {
                Some(self.prev_tiles[idx].as_slice())
            };

            let damage_class = damaged.get(&(dt.x, dt.y)).copied().unwrap_or(DamageClass::UiPrimitive);

            let strat = strategy::choose_strategy(
                current,
                previous,
                crc,
                prev_crc,
                &copy_index,
                damage_class,
                &self.strategy_config,
            );

            let (encoding, payload) = encode_tile(current, previous, &strat, self.zstd_level)?;
            let uncompressed_size = self.codec.config().tile_bytes() as u64;
            let compressed_size = payload.len() as u64;

            batch.uncompressed_bytes += uncompressed_size;
            batch.compressed_bytes += compressed_size;

            batch.tiles.push(TileUpdate {
                tx: dt.x,
                ty: dt.y,
                encoding,
                payload,
                crc,
                damage_class,
            });
        }

        // Update previous frame state
        for dt in damage_tiles {
            let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
            if let Some(tile_data) = current_tiles[idx].take() {
                self.prev_crcs[idx] = current_crcs[idx];
                self.prev_tiles[idx] = tile_data;
            }
        }

        Ok(batch)
    }

    /// Access the tile grid.
    #[must_use]
    pub fn grid(&self) -> &TileGrid {
        &self.grid
    }

    /// Access the payload cache.
    #[must_use]
    pub fn cache(&self) -> &TilePayloadCache {
        &self.cache
    }

    /// Current frame sequence number.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Resize the encoder for new frame buffer dimensions.
    pub fn resize(&mut self, fb_width: u32, fb_height: u32) {
        let config = self.codec.config().clone();
        self.grid = TileGrid::new(fb_width, fb_height, config.clone());
        let total = self.grid.total_tiles() as usize;
        self.prev_crcs = vec![0; total];
        self.prev_tiles = vec![Vec::new(); total];
        self.codec = TileCodec::new(config);
        self.cache.clear();
    }
}

/// Encode a single tile using the chosen strategy.
fn encode_tile(
    current: &[u8],
    previous: Option<&[u8]>,
    strategy: &EncodingStrategy,
    zstd_level: i32,
) -> crate::Result<(TileEncoding, Vec<u8>)> {
    match strategy {
        EncodingStrategy::Skip => Ok((TileEncoding::Skip, Vec::new())),

        EncodingStrategy::Solid { bgra } => Ok((TileEncoding::Solid, bgra.to_vec())),

        EncodingStrategy::Copy { source_index } => {
            Ok((TileEncoding::Copy { source_index: *source_index }, Vec::new()))
        }

        EncodingStrategy::Delta => {
            let prev = previous.expect("delta requires previous tile data");
            let xor = delta::xor_delta(current, prev);
            let compressed = compress::compress_zstd(&xor, zstd_level)?;
            Ok((TileEncoding::Delta, compressed))
        }

        EncodingStrategy::Full => {
            let compressed = compress::compress_zstd(current, zstd_level)?;
            Ok((TileEncoding::Full, compressed))
        }
    }
}

/// Trait for video encoder integration (H.264/H.265/AV1).
///
/// The tile encoder handles lossless UI tiles. For bitmap regions with
/// high change ratios, the compositor can optionally route tiles through
/// a video encoder instead.
pub trait VideoEncoderTrait {
    /// Encode a region of the frame buffer as a video frame.
    fn encode_region(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> crate::Result<Vec<u8>>;

    /// Flush any buffered video frames.
    fn flush(&mut self) -> crate::Result<Vec<Vec<u8>>>;
}
