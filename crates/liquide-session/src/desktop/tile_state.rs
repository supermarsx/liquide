//! Tile encoding state — manages tile-based frame encoding for remote transmission.

use liquide_compositor::damage::DamageSet;
use liquide_encoder::encoder::TileEncoder;
use liquide_encoder::tile::{TileBatch, TileConfig};
use tracing::warn;

/// Tile encoding for remote frame transmission.
pub(super) struct TileEncoderState {
    encoder: Option<TileEncoder>,
    pending_batches: Vec<TileBatch>,
    pub(super) tile_size: u32,
}

impl TileEncoderState {
    pub(super) fn new(width: u32, height: u32, tile_size: u32) -> Self {
        Self {
            encoder: Some(TileEncoder::new(width, height, TileConfig::default())),
            pending_batches: Vec::new(),
            tile_size,
        }
    }

    /// Encode a rendered frame's pixels into tile batches for remote transmission.
    ///
    /// When `damage` is `Some`, only dirty tiles are encoded (incremental).
    /// When `None`, all tiles are marked dirty (full-frame fallback).
    pub(super) fn encode_frame(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        damage: Option<&DamageSet>,
    ) {
        let encoder = match self.encoder.as_mut() {
            Some(e) => e,
            None => return,
        };

        let grid_w = width.div_ceil(self.tile_size);
        let grid_h = height.div_ceil(self.tile_size);

        // Use provided damage or fall back to full-frame damage.
        let owned_damage;
        let tiles = if let Some(d) = damage {
            &d.tiles
        } else {
            owned_damage = {
                let mut d = DamageSet::new(self.tile_size);
                d.mark_all(grid_w, grid_h);
                d
            };
            &owned_damage.tiles
        };

        match encoder.encode_frame_raw(pixels, width, height, stride, tiles) {
            Ok(batch) => {
                self.pending_batches.push(batch);
            }
            Err(e) => {
                warn!("tile encode failed: {e}");
            }
        }
    }

    /// Drain encoded tile batches ready for network transmission.
    pub(super) fn drain_batches(&mut self) -> Vec<TileBatch> {
        std::mem::take(&mut self.pending_batches)
    }

    /// Resize the tile encoder to match new dimensions.
    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if let Some(ref mut encoder) = self.encoder {
            encoder.resize(width, height);
        }
    }
}
