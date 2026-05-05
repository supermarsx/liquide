//! Tile encoding state — manages tile-based frame encoding for remote transmission.

use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_encoder::encoder::TileEncoder;
use liquide_encoder::tile::{TileBatch, TileConfig};
use liquide_encoder::{BandwidthBudget, BandwidthEstimator};
use tracing::warn;

const BANDWIDTH_ESTIMATOR_WINDOW: usize = 10;
const BANDWIDTH_SAFETY_MARGIN: f64 = 0.1;
const DEFAULT_SESSION_TARGET_FPS: u32 = 60;

/// Tile encoding for remote frame transmission.
pub(super) struct TileEncoderState {
    encoder: Option<TileEncoder>,
    pending_batches: Vec<TileBatch>,
    bandwidth_estimator: BandwidthEstimator,
    current_budget: BandwidthBudget,
    last_batch_compressed_bytes: Option<u64>,
    pub(super) tile_size: u32,
    target_fps: u32,
}

impl TileEncoderState {
    pub(super) fn new(width: u32, height: u32, tile_size: u32) -> Self {
        let target_fps = DEFAULT_SESSION_TARGET_FPS;
        let bandwidth_estimator = Self::new_bandwidth_estimator(target_fps);
        Self {
            encoder: Some(Self::new_encoder(width, height, tile_size)),
            pending_batches: Vec::new(),
            current_budget: bandwidth_estimator.frame_budget(BANDWIDTH_SAFETY_MARGIN),
            bandwidth_estimator,
            last_batch_compressed_bytes: None,
            tile_size,
            target_fps,
        }
    }

    fn new_encoder(width: u32, height: u32, tile_size: u32) -> TileEncoder {
        TileEncoder::new(
            width,
            height,
            TileConfig {
                tile_size,
                ..TileConfig::default()
            },
        )
    }

    fn new_bandwidth_estimator(target_fps: u32) -> BandwidthEstimator {
        BandwidthEstimator::new(BANDWIDTH_ESTIMATOR_WINDOW, target_fps)
    }

    fn reset_bandwidth_state(&mut self) {
        self.bandwidth_estimator = Self::new_bandwidth_estimator(self.target_fps);
        self.current_budget = self
            .bandwidth_estimator
            .frame_budget(BANDWIDTH_SAFETY_MARGIN);
        self.last_batch_compressed_bytes = None;
    }

    /// Update the assumed session target FPS used for per-frame bandwidth budgeting.
    pub(super) fn set_target_fps(&mut self, target_fps: u32) {
        self.target_fps = if target_fps == 0 {
            DEFAULT_SESSION_TARGET_FPS
        } else {
            target_fps
        };
        self.reset_bandwidth_state();
    }

    fn refresh_budget_hint(&mut self) {
        self.current_budget
            .refresh_from_estimator(&self.bandwidth_estimator);

        if let Some(compressed_bytes) = self.last_batch_compressed_bytes {
            self.current_budget.observe(compressed_bytes);
        }
    }

    fn record_encoded_batch(&mut self, compressed_bytes: u64) {
        self.bandwidth_estimator.record_frame(compressed_bytes);
        self.last_batch_compressed_bytes = Some(compressed_bytes);
        self.refresh_budget_hint();
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
        if self.encoder.is_none() {
            self.encoder = Some(Self::new_encoder(width, height, self.tile_size));
            self.reset_bandwidth_state();
        }

        self.refresh_budget_hint();

        let grid_w = width.div_ceil(self.tile_size);
        let grid_h = height.div_ceil(self.tile_size);

        // Use provided damage or fall back to full-frame damage.
        let owned_damage;
        let owned_tiles;
        let tiles = if let Some(d) = damage {
            if d.is_full() {
                owned_tiles = d.materialize_tiles();
                &owned_tiles
            } else {
                &d.tiles
            }
        } else {
            owned_damage =
                DamageSet::full(self.tile_size, grid_w, grid_h, DamageClass::UiPrimitive);
            owned_tiles = owned_damage.materialize_tiles();
            &owned_tiles
        };

        let (encoder, budget_hint) = (&mut self.encoder, &self.current_budget);
        let encoder = match encoder.as_mut() {
            Some(encoder) => encoder,
            None => return,
        };

        match encoder.encode_frame_raw_with_budget_hint(
            pixels,
            width,
            height,
            stride,
            tiles,
            Some(budget_hint),
        ) {
            Ok(batch) => {
                self.record_encoded_batch(batch.compressed_bytes);
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
        } else {
            self.encoder = Some(Self::new_encoder(width, height, self.tile_size));
        }

        self.reset_bandwidth_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use liquide_compositor::damage::{DamageClass, DamageTile};
    use liquide_encoder::bandwidth::BudgetPressure;
    use liquide_encoder::strategy::CompressionMethod;

    fn bitmap_damage(tile_size: u32) -> DamageSet {
        let mut damage = DamageSet::new(tile_size);
        damage.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::BitmapRegion,
        });
        damage
    }

    fn patterned_pixels(width: u32, height: u32, seed: u8) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let value = seed.wrapping_add((x + y * width) as u8);
                pixels.extend_from_slice(&[
                    value,
                    value.wrapping_mul(3),
                    value.wrapping_mul(5),
                    255,
                ]);
            }
        }
        pixels
    }

    #[test]
    fn t16_tile_estimator_starts_in_warmup() {
        let state = TileEncoderState::new(4, 4, 4);

        assert_eq!(state.bandwidth_estimator.sample_count(), 0);
        assert!(state.current_budget.is_unlimited());
        assert_eq!(state.current_budget.pressure(), BudgetPressure::Warmup);
        assert!(!state.current_budget.under_pressure());
    }

    #[test]
    fn t16_tile_oversized_batch_enters_pressure_and_uses_hint() {
        let mut state = TileEncoderState::new(4, 4, 4);
        let damage = bitmap_damage(4);

        let frame_one = patterned_pixels(4, 4, 3);
        state.encode_frame(&frame_one, 4, 4, 16, Some(&damage));

        assert_eq!(state.bandwidth_estimator.sample_count(), 1);
        assert!(state.current_budget.under_pressure());

        let first_batch = state.drain_batches().pop().expect("first batch");
        assert!(matches!(
            first_batch.tiles[0].compression,
            CompressionMethod::Zstd { .. }
        ));

        let frame_two = patterned_pixels(4, 4, 19);
        state.encode_frame(&frame_two, 4, 4, 16, Some(&damage));

        let second_batch = state.drain_batches().pop().expect("second batch");
        assert_eq!(second_batch.tiles.len(), 1);
        assert_eq!(second_batch.tiles[0].compression, CompressionMethod::Lz4);
    }

    #[test]
    fn t16_tile_recovers_after_sustained_smaller_batches() {
        let mut state = TileEncoderState::new(64, 64, 64);

        state.record_encoded_batch(12_000);
        assert!(state.current_budget.under_pressure());

        for _ in 0..3 {
            state.record_encoded_batch(4_000);
        }

        assert_eq!(state.current_budget.pressure(), BudgetPressure::Nominal);
        assert!(!state.current_budget.under_pressure());
        assert_eq!(state.bandwidth_estimator.sample_count(), 4);
    }

    #[test]
    fn t16_tile_resize_resets_bandwidth_state() {
        let mut state = TileEncoderState::new(64, 64, 64);

        state.record_encoded_batch(8_000);
        assert_eq!(state.bandwidth_estimator.sample_count(), 1);
        assert!(state.last_batch_compressed_bytes.is_some());

        state.resize(128, 128);

        assert_eq!(state.bandwidth_estimator.sample_count(), 0);
        assert!(state.current_budget.is_unlimited());
        assert_eq!(state.current_budget.pressure(), BudgetPressure::Warmup);
        assert_eq!(state.last_batch_compressed_bytes, None);
    }

    #[test]
    fn t16_tile_target_fps_updates_bandwidth_interval() {
        let mut state = TileEncoderState::new(64, 64, 64);

        state.set_target_fps(1000);

        assert_eq!(state.bandwidth_estimator.frame_interval_us(), 1_000);
        assert_eq!(state.bandwidth_estimator.sample_count(), 0);
        assert!(state.current_budget.is_unlimited());
    }

    #[test]
    fn t47_tile_batches_can_drain_and_transport() {
        let width = 128;
        let height = 128;
        let tile_size = 64;
        let mut state = TileEncoderState::new(width, height, tile_size);

        // Use full damage for first frame (all tiles dirty)
        let grid_w = width.div_ceil(tile_size);
        let grid_h = height.div_ceil(tile_size);
        let damage = DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive);

        // Encode one frame
        let frame_1 = patterned_pixels(width, height, 10);
        state.encode_frame(&frame_1, width, height, width * 4, Some(&damage));

        // Drain batches
        let batches = state.drain_batches();
        assert!(!batches.is_empty(), "should have at least one batch");
        assert!(batches[0].tiles.len() > 0, "batch should have tiles");
        assert!(
            batches[0].compressed_bytes > 0,
            "batch should have compressed bytes"
        );

        // After drain, pending should be empty
        assert!(
            state.drain_batches().is_empty(),
            "second drain should be empty"
        );

        // Encode another frame
        let frame_2 = patterned_pixels(width, height, 20);
        state.encode_frame(&frame_2, width, height, width * 4, Some(&damage));

        let batches_2 = state.drain_batches();
        assert!(!batches_2.is_empty(), "should have a second batch");
        assert!(
            batches_2[0].sequence > batches[0].sequence,
            "sequences should advance"
        );
    }
}
