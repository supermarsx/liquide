//! Tile encoding state — manages tile-based frame encoding for remote transmission.

use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_encoder::encoder::TileEncoder;
use liquide_encoder::tile::{TileBatch, TileConfig};
use liquide_encoder::{BandwidthBudget, BandwidthEstimator};
use liquide_transport::tile_channel::TileSender;
use tracing::warn;

const BANDWIDTH_ESTIMATOR_WINDOW: usize = 10;
const BANDWIDTH_SAFETY_MARGIN: f64 = 0.1;
const DEFAULT_SESSION_TARGET_FPS: u32 = 60;

/// Maximum number of encoded `TileBatch`es retained in `pending_batches`.
///
/// `encode_frame` pushes one batch per presented frame. When no remote
/// consumer is attached (the local-display desktop path), `pending_batches`
/// is the only sink and would otherwise grow once per frame for the lifetime
/// of the session — an unbounded leak on the primary path (t49-e6-01). This
/// cap turns `pending_batches` into a bounded ring that drops the oldest batch
/// when full, so a missing/idle consumer can never grow memory without limit.
/// The value is a few frames' worth — enough for a consumer to drain a short
/// backlog, far below any leak threshold.
///
/// When a real transport sink IS attached via [`TileEncoderState::attach_sink`]
/// (t55-E8), newly encoded batches plus any retained backlog are forwarded to
/// the sink each frame, so `pending_batches` stays drained on the wired path.
/// The cap still applies as a safety net if the sink disconnects mid-session.
const MAX_PENDING_BATCHES: usize = 8;

/// Tile encoding for remote frame transmission.
pub(super) struct TileEncoderState {
    encoder: Option<TileEncoder>,
    pending_batches: Vec<TileBatch>,
    /// Optional remote transport sink. When `Some`, encoded batches are
    /// forwarded to it each frame (the wired drain path); when `None`, batches
    /// accumulate in the bounded `pending_batches` ring (local-display path).
    sink: Option<TileSender>,
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
            sink: None,
            current_budget: bandwidth_estimator.frame_budget(BANDWIDTH_SAFETY_MARGIN),
            bandwidth_estimator,
            last_batch_compressed_bytes: None,
            tile_size,
            target_fps,
        }
    }

    /// Attach a remote transport sink to drain encoded tile batches into.
    ///
    /// Once attached, every encoded frame (plus any batch already buffered in
    /// the bounded ring) is forwarded to the sink, so `pending_batches` no
    /// longer accumulates on the live path. Passing the sink here is the
    /// wired-drain counterpart to the bounded-ring fallback used when no
    /// consumer exists. Any backlog already buffered is flushed immediately.
    pub(super) fn attach_sink(&mut self, sink: TileSender) {
        self.sink = Some(sink);
        self.flush_to_sink();
    }

    /// Forward all buffered batches to the attached sink, if any.
    ///
    /// Returns silently when no sink is attached (batches stay in the bounded
    /// ring). If the sink has disconnected, the sink is dropped and batches
    /// remain in the bounded ring (which still caps memory) rather than being
    /// silently lost.
    fn flush_to_sink(&mut self) {
        let Some(sink) = self.sink.as_ref() else {
            return;
        };
        // Forward in FIFO order, stopping (and retaining the remainder) if the
        // receiver has hung up so nothing is silently dropped beyond the cap.
        let mut disconnected = false;
        let mut sent = 0usize;
        for batch in self.pending_batches.iter() {
            if sink.send(batch.clone()).is_err() {
                disconnected = true;
                break;
            }
            sent += 1;
        }
        if sent > 0 {
            self.pending_batches.drain(..sent);
        }
        if disconnected {
            // The transport went away; stop forwarding and fall back to the
            // bounded-ring behaviour so memory stays capped without a consumer.
            warn!("tile transport sink disconnected; falling back to bounded ring");
            self.sink = None;
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
                // Bound the buffer regardless of whether a consumer is draining
                // it: drop the oldest batch once the cap is reached so the
                // primary desktop loop can never grow `pending_batches`
                // unbounded (t49-e6-01 contained fix, t50-e18). The cap holds
                // even with a sink attached, as a safety net for a stalled or
                // disconnected transport.
                if self.pending_batches.len() >= MAX_PENDING_BATCHES {
                    self.pending_batches.remove(0);
                }
                self.pending_batches.push(batch);
                // If a real transport sink is attached, drain the buffered
                // batches into it now so they actually reach the remote
                // consumer instead of only being capped (t55-E8 wired drain).
                self.flush_to_sink();
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
    use liquide_transport::tile_channel::tile_channel;

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

    #[test]
    fn t50_e18_pending_batches_are_bounded_without_a_consumer() {
        let width = 64;
        let height = 64;
        let tile_size = 64;
        let mut state = TileEncoderState::new(width, height, tile_size);

        let grid_w = width.div_ceil(tile_size);
        let grid_h = height.div_ceil(tile_size);
        let damage = DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive);

        // Encode far more frames than the cap, never draining (simulating the
        // live desktop loop where `drain_batches` has no caller).
        let frames = MAX_PENDING_BATCHES * 5 + 3;
        for i in 0..frames {
            let pixels = patterned_pixels(width, height, i as u8);
            state.encode_frame(&pixels, width, height, width * 4, Some(&damage));
            assert!(
                state.pending_batches.len() <= MAX_PENDING_BATCHES,
                "pending_batches must never exceed the cap (len={}, cap={})",
                state.pending_batches.len(),
                MAX_PENDING_BATCHES
            );
        }

        // The buffer is saturated at exactly the cap, and retains the newest
        // batches (oldest dropped): the last drained sequence is the most recent.
        assert_eq!(state.pending_batches.len(), MAX_PENDING_BATCHES);
        let drained = state.drain_batches();
        assert_eq!(drained.len(), MAX_PENDING_BATCHES);
        assert!(
            drained.windows(2).all(|w| w[1].sequence > w[0].sequence),
            "retained batches should be the newest, in ascending sequence order"
        );
    }

    #[test]
    fn t55_e8_attached_sink_drains_encoded_batches() {
        let width = 64;
        let height = 64;
        let tile_size = 64;
        let mut state = TileEncoderState::new(width, height, tile_size);

        let (tx, rx) = tile_channel();
        state.attach_sink(tx);

        let grid_w = width.div_ceil(tile_size);
        let grid_h = height.div_ceil(tile_size);
        let damage = DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive);

        // Encode several times the cap. With a sink attached, every batch is
        // forwarded, so the buffer stays drained and the receiver gets them all.
        let frames = MAX_PENDING_BATCHES * 4 + 1;
        for i in 0..frames {
            let pixels = patterned_pixels(width, height, i as u8);
            state.encode_frame(&pixels, width, height, width * 4, Some(&damage));
            assert!(
                state.pending_batches.is_empty(),
                "buffer should stay drained while a sink is attached (len={})",
                state.pending_batches.len()
            );
        }

        let received: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            received.len(),
            frames,
            "every encoded batch should reach the transport sink (no loss, no cap drops)"
        );
        assert!(
            received.windows(2).all(|w| w[1].sequence > w[0].sequence),
            "sink should receive batches in ascending sequence order"
        );
    }

    #[test]
    fn t55_e8_attach_sink_flushes_existing_backlog() {
        let width = 64;
        let height = 64;
        let tile_size = 64;
        let mut state = TileEncoderState::new(width, height, tile_size);

        let grid_w = width.div_ceil(tile_size);
        let grid_h = height.div_ceil(tile_size);
        let damage = DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive);

        // Encode a few frames BEFORE attaching the sink: they buffer in the ring.
        for i in 0..3 {
            let pixels = patterned_pixels(width, height, i as u8);
            state.encode_frame(&pixels, width, height, width * 4, Some(&damage));
        }
        assert_eq!(state.pending_batches.len(), 3);

        // Attaching the sink flushes the existing backlog immediately.
        let (tx, rx) = tile_channel();
        state.attach_sink(tx);

        assert!(
            state.pending_batches.is_empty(),
            "attaching a sink should flush the buffered backlog"
        );
        let received: Vec<_> = rx.try_iter().collect();
        assert_eq!(received.len(), 3, "backlog should be forwarded to the sink");
    }

    #[test]
    fn t55_e8_disconnected_sink_falls_back_to_bounded_ring() {
        let width = 64;
        let height = 64;
        let tile_size = 64;
        let mut state = TileEncoderState::new(width, height, tile_size);

        let (tx, rx) = tile_channel();
        state.attach_sink(tx);

        let grid_w = width.div_ceil(tile_size);
        let grid_h = height.div_ceil(tile_size);
        let damage = DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive);

        // Drop the receiver: the transport has hung up.
        drop(rx);

        // Keep encoding well past the cap. With the sink disconnected the
        // encoder must fall back to the bounded ring — never an unbounded leak.
        let frames = MAX_PENDING_BATCHES * 5 + 3;
        for i in 0..frames {
            let pixels = patterned_pixels(width, height, i as u8);
            state.encode_frame(&pixels, width, height, width * 4, Some(&damage));
            assert!(
                state.pending_batches.len() <= MAX_PENDING_BATCHES,
                "after sink disconnect, the bounded ring cap must still hold (len={})",
                state.pending_batches.len()
            );
        }

        assert!(
            state.sink.is_none(),
            "a disconnected sink should be dropped so it is not retried forever"
        );
        assert_eq!(state.pending_batches.len(), MAX_PENDING_BATCHES);
    }
}
