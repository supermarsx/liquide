//! Main tile encoder: orchestrates the full encoding pipeline.
//!
//! Pipeline: extract tile → CRC-32C hash → choose strategy →
//! choose compression (LZ4/Zstd by damage class) →
//! XOR delta (if applicable) → compress → cache → produce `TileBatch`.

use std::collections::HashMap;
use std::time::Instant;

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;

use crate::bandwidth::BandwidthBudget;
use crate::cache::TilePayloadCache;
use crate::compress;
use crate::delta;
use crate::hash;
use crate::strategy::{self, CompressionMethod, EncodingStrategy, StrategyConfig};
use crate::tile::{
    FrameStats, TileBatch, TileCodec, TileConfig, TileEncoding, TileGrid, TileUpdate,
};

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
    /// Monotonic fragment sequence counter across all fragmented batches.
    fragment_sequence: u64,
    /// Statistics from the most recently encoded frame.
    last_stats: Option<FrameStats>,
    /// Reusable scratch buffer for current frame tiles (avoids per-frame allocation).
    current_tiles_buf: Vec<Option<Vec<u8>>>,
    /// Reusable damaged tile lookup set (avoids per-frame HashMap allocation).
    damaged_set: HashMap<(u32, u32), DamageClass>,
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
            fragment_sequence: 0,
            last_stats: None,
            current_tiles_buf: vec![None; total],
            damaged_set: HashMap::new(),
        }
    }

    /// Encode one frame. Only tiles listed in `damage_tiles` are processed;
    /// all others are assumed unchanged (Skip).
    pub fn encode_frame(
        &mut self,
        fb: &FrameBuffer,
        damage_tiles: &[DamageTile],
    ) -> crate::Result<TileBatch> {
        self.encode_frame_with_budget_hint(fb, damage_tiles, None)
    }

    /// Encode one frame with an optional per-frame budget hint.
    ///
    /// When `budget_hint` is `None`, or when the provided budget is not
    /// currently under pressure, compression follows the existing no-pressure
    /// path. A pressured budget only changes compression policy for
    /// `BitmapRegion` tiles.
    pub fn encode_frame_with_budget_hint(
        &mut self,
        fb: &FrameBuffer,
        damage_tiles: &[DamageTile],
        budget_hint: Option<&BandwidthBudget>,
    ) -> crate::Result<TileBatch> {
        let frame_start = Instant::now();
        let under_budget_pressure = budget_hint.is_some_and(BandwidthBudget::under_pressure);

        self.sequence = self.sequence.saturating_add(1);
        self.cache.advance_frame();

        let mut batch = TileBatch::new(self.sequence);

        // Build set of damaged tile coordinates for quick lookup (reuse allocation)
        self.damaged_set.clear();
        for dt in damage_tiles {
            self.damaged_set.insert((dt.x, dt.y), dt.class);
        }

        // Extract current tiles and compute CRCs for damaged tiles.
        // This phase is embarrassingly parallel: extract_tile reads from a
        // shared &[u8] pixel buffer and crc32c is a pure function.
        // Use mem::take to avoid cloning prev_crcs (zero-copy swap).
        let mut current_crcs: Vec<u32> = std::mem::take(&mut self.prev_crcs);
        if current_crcs.is_empty() {
            current_crcs = vec![0; self.grid.total_tiles() as usize];
        }
        // Reuse current_tiles buffer
        self.current_tiles_buf.iter_mut().for_each(|s| *s = None);
        let total_tiles = self.grid.total_tiles() as usize;
        let mut current_tiles = std::mem::take(&mut self.current_tiles_buf);
        if current_tiles.len() != total_tiles {
            current_tiles = vec![None; total_tiles];
        }

        // Save previous CRCs for damaged tiles before they're overwritten.
        // (current_crcs starts as the previous frame's CRCs; extract phase overwrites damaged entries)
        let prev_crc_for_tile: HashMap<usize, u32> = damage_tiles
            .iter()
            .map(|dt| {
                let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
                (idx, current_crcs[idx])
            })
            .collect();

        if damage_tiles.len() <= 2 {
            // Small number of tiles — not worth threading overhead.
            for dt in damage_tiles {
                let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
                let tile_data = self.codec.extract_tile(
                    fb.pixels(),
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
        } else {
            // Parallel tile extraction + CRC using scoped threads.
            let num_workers = std::thread::available_parallelism()
                .map(|n| n.get().min(8))
                .unwrap_or(4);
            let chunk_size = (damage_tiles.len() + num_workers - 1) / num_workers;

            // Pre-compute work items: (tile_index, tx, ty)
            let work: Vec<(usize, u32, u32)> = damage_tiles
                .iter()
                .map(|dt| {
                    let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
                    (idx, dt.x, dt.y)
                })
                .collect();
            // Deduplicate: if two damage tiles map to the same index, only encode once.
            // Duplicate indices would cause the second write to silently overwrite the first.
            let mut seen = std::collections::HashSet::with_capacity(work.len());
            let work: Vec<(usize, u32, u32)> = work
                .into_iter()
                .filter(|(idx, _, _)| seen.insert(*idx))
                .collect();

            // Shared read-only references for the thread pool.
            let codec = &self.codec;
            let pixels = fb.pixels();
            let stride = fb.stride;
            let fb_width = fb.width;
            let fb_height = fb.height;

            std::thread::scope(|s| {
                let handles: Vec<_> = work
                    .chunks(chunk_size)
                    .map(|chunk| {
                        s.spawn(move || {
                            let mut results: Vec<(usize, u32, Vec<u8>)> =
                                Vec::with_capacity(chunk.len());
                            for &(idx, tx, ty) in chunk {
                                let tile_data =
                                    codec.extract_tile(pixels, stride, fb_width, fb_height, tx, ty);
                                let crc = hash::crc32c(&tile_data);
                                results.push((idx, crc, tile_data));
                            }
                            results
                        })
                    })
                    .collect();

                // Collect results back into the single-owner vectors.
                for handle in handles {
                    match handle.join() {
                        Ok(tiles) => {
                            for (idx, crc, tile_data) in tiles {
                                current_crcs[idx] = crc;
                                current_tiles[idx] = Some(tile_data);
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "tile encoder thread panicked: {:?}",
                                e.downcast_ref::<&str>()
                            );
                            // Mark all tiles from this chunk as needing fallback.
                            // We'll handle missing tiles below with an empty tile fallback.
                        }
                    }
                }
            });
        }

        // Build copy index from all current CRCs
        let copy_index = strategy::build_copy_index(&current_crcs);

        let mut lz4_tiles = 0u32;
        let mut zstd_tiles = 0u32;
        let mut tiles_encoded = 0u32;

        // Encode each damaged tile
        for dt in damage_tiles {
            let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
            let empty_tile;
            let current = match current_tiles[idx].as_ref() {
                Some(data) => data,
                None => {
                    // Tile extraction failed (thread panic). Use empty tile as fallback
                    // and skip encoding to avoid sending corrupted data.
                    tracing::warn!(
                        "tile ({}, {}): extraction missing, using fallback",
                        dt.x,
                        dt.y
                    );
                    empty_tile = vec![0u8; self.codec.config().tile_bytes()];
                    &empty_tile
                }
            };
            let crc = current_crcs[idx];
            let prev_crc = if self.prev_tiles[idx].is_empty() {
                None
            } else {
                prev_crc_for_tile.get(&idx).copied()
            };
            let previous = if self.prev_tiles[idx].is_empty() {
                None
            } else {
                Some(self.prev_tiles[idx].as_slice())
            };

            let damage_class = self
                .damaged_set
                .get(&(dt.x, dt.y))
                .copied()
                .unwrap_or(DamageClass::UiPrimitive);

            let strat = strategy::choose_strategy(
                current,
                previous,
                crc,
                prev_crc,
                &copy_index,
                damage_class,
                &self.strategy_config,
            );

            // Choose compression method based on damage class
            let compression = strategy::choose_compression(
                damage_class,
                &self.strategy_config,
                under_budget_pressure,
            );

            let (encoding, payload) = encode_tile(current, previous, &strat, &compression)?;
            let uncompressed_size = self.codec.config().tile_bytes() as u64;
            let compressed_size = payload.len() as u64;

            batch.uncompressed_bytes += uncompressed_size;
            batch.compressed_bytes += compressed_size;

            if encoding != TileEncoding::Skip {
                tiles_encoded += 1;
                match compression {
                    CompressionMethod::Lz4 => lz4_tiles += 1,
                    CompressionMethod::Zstd { .. } => zstd_tiles += 1,
                }
            }

            batch.tiles.push(TileUpdate {
                tx: dt.x,
                ty: dt.y,
                encoding,
                payload,
                crc,
                damage_class,
                compression,
            });
        }

        // Update previous frame state — move current_crcs back and update prev_tiles
        for dt in damage_tiles {
            let idx = self.grid.coords_to_index(dt.x, dt.y) as usize;
            if let Some(tile_data) = current_tiles[idx].take() {
                self.prev_tiles[idx] = tile_data;
            }
        }
        // Restore buffers for next frame reuse
        self.prev_crcs = current_crcs;
        self.current_tiles_buf = current_tiles;

        // Fill in frame stats
        let encode_time = frame_start.elapsed();
        batch.stats = FrameStats {
            encode_time_us: encode_time.as_micros() as u64,
            tiles_encoded,
            bytes_saved: batch
                .uncompressed_bytes
                .saturating_sub(batch.compressed_bytes),
            compression_ratio: batch.compression_ratio(),
            lz4_tiles,
            zstd_tiles,
        };

        self.last_stats = Some(batch.stats.clone());

        Ok(batch)
    }

    /// Access the statistics from the most recently encoded frame.
    ///
    /// Returns `None` if no frame has been encoded yet.
    #[must_use]
    pub fn frame_stats(&self) -> Option<&FrameStats> {
        self.last_stats.as_ref()
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

    /// Access the strategy configuration.
    #[must_use]
    pub fn strategy_config(&self) -> &StrategyConfig {
        &self.strategy_config
    }

    /// Update the strategy configuration.
    pub fn set_strategy_config(&mut self, config: StrategyConfig) {
        self.strategy_config = config;
    }

    /// Encode a frame and return fragments sized to fit within
    /// `max_payload_bytes` once CBOR-encoded and wrapped in the standard
    /// Liquide frame header. Fragments carry monotonic `sequence` numbers
    /// across all frames produced by this encoder instance, so transports
    /// can detect gaps/drops without state-tracking.
    ///
    /// Returns `Err(EncoderError::Internal)` if `max_payload_bytes == 0`.
    pub fn encode_frame_with_mtu(
        &mut self,
        fb: &FrameBuffer,
        damage_tiles: &[DamageTile],
        max_payload_bytes: usize,
    ) -> crate::Result<Vec<crate::fragment::BatchFragment>> {
        if max_payload_bytes == 0 {
            return Err(crate::EncoderError::Internal(
                "max_payload_bytes must be > 0".into(),
            ));
        }
        let batch = self.encode_frame_with_budget_hint(fb, damage_tiles, None)?;
        let starting_seq = self.fragment_sequence;
        let fragments = crate::fragment::fragment_batch(&batch, max_payload_bytes, starting_seq)
            .map_err(|e| crate::EncoderError::Internal(format!("fragment: {e}")))?;
        self.fragment_sequence = self
            .fragment_sequence
            .saturating_add(fragments.len() as u64);
        Ok(fragments)
    }

    /// Encode a frame from raw pixel bytes, avoiding `FrameBuffer` construction
    /// when the caller only has a pixel slice (e.g. from a `RenderedFrame`).
    ///
    /// `pixels` must contain at least `stride * height` bytes in BGRA8 format.
    pub fn encode_frame_raw(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        damage_tiles: &[DamageTile],
    ) -> crate::Result<TileBatch> {
        self.encode_frame_raw_with_budget_hint(pixels, width, height, stride, damage_tiles, None)
    }

    /// Encode a raw pixel frame with an optional per-frame budget hint.
    pub fn encode_frame_raw_with_budget_hint(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        damage_tiles: &[DamageTile],
        budget_hint: Option<&BandwidthBudget>,
    ) -> crate::Result<TileBatch> {
        let fb = FrameBuffer {
            memory: liquide_compositor::framebuffer::FrameMemory::Cpu(pixels.to_vec()),
            width,
            height,
            stride,
            format: liquide_compositor::pixel::PixelFormat::Bgra8,
        };
        self.encode_frame_with_budget_hint(&fb, damage_tiles, budget_hint)
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
        self.last_stats = None;
    }
}

/// Encode a single tile using the chosen strategy and compression method.
fn encode_tile(
    current: &[u8],
    previous: Option<&[u8]>,
    strategy: &EncodingStrategy,
    compression: &CompressionMethod,
) -> crate::Result<(TileEncoding, Vec<u8>)> {
    match strategy {
        EncodingStrategy::Skip => Ok((TileEncoding::Skip, Vec::new())),

        EncodingStrategy::Solid { bgra } => Ok((TileEncoding::Solid, bgra.to_vec())),

        EncodingStrategy::Copy { source_index } => Ok((
            TileEncoding::Copy {
                source_index: *source_index,
            },
            Vec::new(),
        )),

        EncodingStrategy::Delta => {
            let prev = previous.ok_or_else(|| {
                crate::EncoderError::Internal("delta requires previous tile data".into())
            })?;
            let xor = delta::xor_delta(current, prev);
            let compressed = compress_with_method(&xor, compression)?;
            Ok((TileEncoding::Delta, compressed))
        }

        EncodingStrategy::Full => {
            let compressed = compress_with_method(current, compression)?;
            Ok((TileEncoding::Full, compressed))
        }
    }
}

/// Compress data using the specified compression method.
fn compress_with_method(data: &[u8], method: &CompressionMethod) -> crate::Result<Vec<u8>> {
    match method {
        CompressionMethod::Zstd { level } => compress::compress_zstd(data, *level),
        CompressionMethod::Lz4 => Ok(compress::compress_lz4(data)),
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
