//! Criterion benchmarks for the liquide-encoder crate.

use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;

use liquide_encoder::bandwidth::{BandwidthBudget, BandwidthEstimator};
use liquide_encoder::compress::{compress_lz4, compress_zstd};
use liquide_encoder::delta::xor_delta;
use liquide_encoder::encoder::TileEncoder;
use liquide_encoder::hash::crc32c;
use liquide_encoder::strategy::{build_copy_index, choose_strategy, StrategyConfig};
use liquide_encoder::tile::TileConfig;

/// Generate a pseudo-random tile buffer with a given seed.
/// Uses a simple xorshift for deterministic but varied data.
fn make_tile_buffer(size: usize, seed: u64) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    let mut state = seed | 1; // ensure non-zero
    for byte in &mut buf {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state & 0xFF) as u8;
    }
    buf
}

/// Generate a slightly modified copy of a buffer (simulating a small delta).
fn make_modified_buffer(base: &[u8], change_fraction: f64, seed: u64) -> Vec<u8> {
    let mut buf = base.to_vec();
    let mut state = seed | 1;
    let threshold = (change_fraction * 256.0) as u8;
    for byte in &mut buf {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if (state & 0xFF) as u8 <= threshold {
            *byte = ((state >> 8) & 0xFF) as u8;
        }
    }
    buf
}

// ── Individual primitive benchmarks ─────────────────────────────────────

fn bench_crc32c(c: &mut Criterion) {
    // 64x64 BGRA tile = 16,384 bytes
    let tile = make_tile_buffer(64 * 64 * 4, 42);
    c.bench_function("crc32c_16kb_tile", |b| {
        b.iter(|| crc32c(black_box(&tile)));
    });
}

fn bench_xor_delta(c: &mut Criterion) {
    let tile_a = make_tile_buffer(64 * 64 * 4, 42);
    let tile_b = make_modified_buffer(&tile_a, 0.1, 99);
    c.bench_function("xor_delta_16kb_tile", |b| {
        b.iter(|| xor_delta(black_box(&tile_a), black_box(&tile_b)));
    });
}

fn bench_compress_zstd(c: &mut Criterion) {
    let tile = make_tile_buffer(64 * 64 * 4, 42);
    c.bench_function("compress_zstd_level3_16kb", |b| {
        b.iter(|| compress_zstd(black_box(&tile), 3).unwrap());
    });
}

fn bench_compress_lz4(c: &mut Criterion) {
    let tile = make_tile_buffer(64 * 64 * 4, 42);
    c.bench_function("compress_lz4_16kb", |b| {
        b.iter(|| compress_lz4(black_box(&tile)));
    });
}

// ── Full pipeline benchmark ─────────────────────────────────────────────

fn bench_encode_frame(c: &mut Criterion) {
    // 512x512 frame, 64px tiles => 8x8 = 64 tiles
    let width = 512u32;
    let height = 512u32;
    let config = TileConfig::default(); // tile_size=64, bpp=4

    let mut fb = FrameBuffer::new(width, height, PixelFormat::Bgra8);
    // Fill the frame buffer with pseudo-random data so compression is realistic
    let pixel_data = make_tile_buffer(fb.pixels().len(), 1337);
    fb.pixels_mut().copy_from_slice(&pixel_data);

    // Mark all 64 tiles as damaged
    let cols = width / config.tile_size;
    let rows = height / config.tile_size;
    let damage_tiles: Vec<DamageTile> = (0..rows)
        .flat_map(|ty| {
            (0..cols).map(move |tx| DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::UiPrimitive,
            })
        })
        .collect();

    c.bench_function("encode_frame_512x512_64tiles", |b| {
        b.iter_with_setup(
            || TileEncoder::new(width, height, config.clone()),
            |mut encoder| {
                let _ = encoder.encode_frame(black_box(&fb), black_box(&damage_tiles));
            },
        );
    });
}

// ── Strategy selection benchmark ────────────────────────────────────────

fn bench_choose_strategy(c: &mut Criterion) {
    let strategy_config = StrategyConfig::default();
    let tile_size = 64 * 64 * 4; // 16KB

    // Generate 100 tile buffers with some duplicates for copy detection
    let tiles_current: Vec<Vec<u8>> = (0..100)
        .map(|i| {
            if i % 10 == 0 {
                // Every 10th tile is a duplicate of tile 0
                make_tile_buffer(tile_size, 1000)
            } else {
                make_tile_buffer(tile_size, 1000 + i)
            }
        })
        .collect();

    let tiles_previous: Vec<Vec<u8>> = (0..100)
        .map(|i| make_modified_buffer(&tiles_current[i as usize], 0.05, 2000 + i))
        .collect();

    // Compute CRCs
    let current_crcs: Vec<u32> = tiles_current.iter().map(|t| crc32c(t)).collect();
    let prev_crcs: Vec<u32> = tiles_previous.iter().map(|t| crc32c(t)).collect();

    // Build copy index
    let copy_index: HashMap<u32, u32> = build_copy_index(&current_crcs);

    c.bench_function("choose_strategy_100_tiles", |b| {
        b.iter(|| {
            for i in 0..100usize {
                let _ = choose_strategy(
                    black_box(&tiles_current[i]),
                    black_box(Some(tiles_previous[i].as_slice())),
                    black_box(current_crcs[i]),
                    black_box(Some(prev_crcs[i])),
                    black_box(&copy_index),
                    black_box(DamageClass::UiPrimitive),
                    black_box(&strategy_config),
                );
            }
        });
    });
}

// ── Bandwidth estimator benchmark ───────────────────────────────────────

fn bench_bandwidth_estimator(c: &mut Criterion) {
    c.bench_function("bandwidth_estimator_1000_frames", |b| {
        b.iter(|| {
            let mut estimator = BandwidthEstimator::new(128, 60);
            for i in 0..1000u64 {
                estimator.record_frame(black_box(50_000 + (i % 100) * 100));
            }
            let bps = estimator.estimated_bandwidth_bps();
            let _budget = BandwidthBudget::from_estimator(black_box(&estimator), 0.1);
            black_box(bps)
        });
    });
}

// ── Group and main ──────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_crc32c,
    bench_xor_delta,
    bench_compress_zstd,
    bench_compress_lz4,
    bench_encode_frame,
    bench_choose_strategy,
    bench_bandwidth_estimator,
);
criterion_main!(benches);
