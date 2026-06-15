//! Tile decoder — decompresses and reconstructs tiles from encoded updates.

use std::sync::Arc;

use liquide_encoder::compress::{decompress_lz4, decompress_zstd};
use liquide_encoder::delta::xor_apply;
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{TileConfig, TileEncoding, TileUpdate};

use crate::ClientRendererError;
use crate::surface::MAX_DIMENSION;

/// Number of grid slots for a `cols x rows` tile grid, computed with widened
/// arithmetic so a malformed `cols`/`rows` cannot wrap a u32 and under-allocate
/// the slot vector that is later indexed by `ty * cols + tx`
/// (memory-safety: t49-e7-F1).
#[inline]
#[must_use]
fn grid_slot_count(cols: u32, rows: u32) -> usize {
    let cols = cols.min(MAX_DIMENSION) as usize;
    let rows = rows.min(MAX_DIMENSION) as usize;
    cols * rows
}

/// Decodes compressed tile updates back into raw pixel data.
///
/// Maintains a tile cache of the last committed tile data at each grid
/// position, enabling delta decoding and skip reuse. Tiles are decoded
/// individually and then committed to update the cache.
pub struct TileDecoder {
    config: TileConfig,
    /// Cached tile data from previous commits, indexed by `ty * cols + tx`.
    previous_tiles: Vec<Option<Arc<[u8]>>>,
    zero_tile: Arc<[u8]>,
    cols: u32,
    rows: u32,
}

impl TileDecoder {
    /// Create a new decoder for a grid of the given dimensions.
    #[must_use]
    pub fn new(cols: u32, rows: u32, config: TileConfig) -> Self {
        let total = grid_slot_count(cols, rows);
        let zero_tile: Arc<[u8]> = vec![0u8; config.tile_bytes()].into();
        Self {
            config,
            previous_tiles: vec![None; total],
            zero_tile,
            cols,
            rows,
        }
    }

    /// Decode a single tile update into raw pixel data.
    ///
    /// The returned data is `tile_size * tile_size * bpp` bytes, suitable
    /// for writing directly into a [`RenderSurface`](crate::RenderSurface).
    pub fn decode_tile(&self, update: &TileUpdate) -> crate::Result<Arc<[u8]>> {
        if update.tx >= self.cols || update.ty >= self.rows {
            return Err(ClientRendererError::InvalidTileCoords {
                tx: update.tx,
                ty: update.ty,
                cols: self.cols,
                rows: self.rows,
            });
        }

        let idx = (update.ty * self.cols + update.tx) as usize;
        let tile_bytes = self.config.tile_bytes();

        match update.encoding {
            TileEncoding::Skip => {
                // Reuse the previously committed tile data.
                match &self.previous_tiles[idx] {
                    Some(data) => Ok(Arc::clone(data)),
                    None => Ok(Arc::clone(&self.zero_tile)),
                }
            }

            TileEncoding::Full => {
                // Decompress the full tile payload.
                let raw = self.decompress(&update.payload, &update.compression)?;
                if raw.len() != tile_bytes {
                    return Err(ClientRendererError::FrameSizeMismatch {
                        expected: tile_bytes,
                        got: raw.len(),
                    });
                }
                Ok(raw.into())
            }

            TileEncoding::Delta => {
                // Decompress delta, then XOR-apply against previous tile.
                let delta = self.decompress(&update.payload, &update.compression)?;
                let previous = self.previous_tiles[idx]
                    .as_deref()
                    .unwrap_or(self.zero_tile.as_ref());
                if delta.len() != previous.len() {
                    return Err(ClientRendererError::FrameSizeMismatch {
                        expected: previous.len(),
                        got: delta.len(),
                    });
                }
                Ok(xor_apply(previous, &delta).into())
            }

            TileEncoding::Solid => {
                // Fill the entire tile with the 4-byte color from the payload.
                if update.payload.len() < 4 {
                    return Err(ClientRendererError::DecodeError(
                        "solid tile payload too short".to_string(),
                    ));
                }
                if tile_bytes % 4 != 0 {
                    return Err(ClientRendererError::DecodeError(format!(
                        "solid tile: tile_bytes ({}) not divisible by 4",
                        tile_bytes
                    )));
                }
                let color = [
                    update.payload[0],
                    update.payload[1],
                    update.payload[2],
                    update.payload[3],
                ];
                let mut buf = vec![0u8; tile_bytes];
                for chunk in buf.chunks_exact_mut(4) {
                    chunk.copy_from_slice(&color);
                }
                Ok(buf.into())
            }

            TileEncoding::Copy { source_index } => {
                // Copy from another tile that was already decoded in this frame.
                let src = source_index as usize;
                if src >= self.previous_tiles.len() {
                    return Err(ClientRendererError::InvalidTileCoords {
                        tx: source_index % self.cols,
                        ty: source_index / self.cols,
                        cols: self.cols,
                        rows: self.rows,
                    });
                }
                match &self.previous_tiles[src] {
                    Some(data) => Ok(Arc::clone(data)),
                    None => {
                        tracing::debug!(
                            "copy tile: source index {} not yet committed, using zeros",
                            source_index,
                        );
                        Ok(Arc::clone(&self.zero_tile))
                    }
                }
            }
        }
    }

    /// Commit decoded tile data into the cache at the given coordinates.
    ///
    /// This must be called after [`decode_tile`](Self::decode_tile) so that
    /// subsequent delta and copy operations can reference this tile.
    pub fn commit_tile(&mut self, tx: u32, ty: u32, data: Arc<[u8]>) {
        let idx = (ty * self.cols + tx) as usize;
        if idx < self.previous_tiles.len() {
            self.previous_tiles[idx] = Some(data);
        }
    }

    /// Reset all cached tile data.
    pub fn reset(&mut self) {
        for slot in &mut self.previous_tiles {
            *slot = None;
        }
    }

    /// Resize the decoder for a new grid size, clearing all cached data.
    pub fn resize(&mut self, cols: u32, rows: u32) {
        self.cols = cols;
        self.rows = rows;
        let total = grid_slot_count(cols, rows);
        self.previous_tiles = vec![None; total];
        self.zero_tile = vec![0u8; self.config.tile_bytes()].into();
    }

    /// Number of tile columns.
    #[must_use]
    pub fn cols(&self) -> u32 {
        self.cols
    }

    /// Number of tile rows.
    #[must_use]
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// Tile configuration.
    #[must_use]
    pub fn config(&self) -> &TileConfig {
        &self.config
    }

    /// Decompress a payload using the specified compression method.
    fn decompress(&self, payload: &[u8], method: &CompressionMethod) -> crate::Result<Vec<u8>> {
        let max_size = self.config.tile_bytes();
        match method {
            CompressionMethod::Zstd { .. } => decompress_zstd(payload, max_size)
                .map_err(|e| ClientRendererError::CompressionError(e.to_string())),
            CompressionMethod::Lz4 => decompress_lz4(payload)
                .map_err(|e| ClientRendererError::CompressionError(e.to_string())),
        }
    }
}

impl std::fmt::Display for TileDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TileDecoder({}x{}, tile_size={})",
            self.cols, self.rows, self.config.tile_size
        )
    }
}
