use std::sync::Arc;

use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use liquide_compositor::damage::DamageClass;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::compress::compress_lz4;
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{FrameStats, TileBatch, TileConfig, TileEncoding, TileUpdate};

use liquide_client_renderer::decoder::TileDecoder;
use liquide_client_renderer::frame::FrameAssembler;

const TILE_SIZE: u32 = 64;
const BYTES_PER_PIXEL: u32 = 4;

fn tile_config() -> TileConfig {
    TileConfig {
        tile_size: TILE_SIZE,
        bpp: BYTES_PER_PIXEL,
    }
}

fn make_tile_bytes(seed: u32, tile_bytes: usize) -> Vec<u8> {
    (0..tile_bytes)
        .map(|index| (seed.wrapping_mul(17).wrapping_add(index as u32) & 0xFF) as u8)
        .collect()
}

fn make_full_tile_update(tx: u32, ty: u32, tile_bytes: usize) -> TileUpdate {
    let raw = make_tile_bytes(tx * 31 + ty * 17, tile_bytes);
    let compressed = compress_lz4(&raw);
    TileUpdate {
        tx,
        ty,
        encoding: TileEncoding::Full,
        payload: compressed,
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    }
}

fn make_skip_tile_update(tx: u32, ty: u32) -> TileUpdate {
    TileUpdate {
        tx,
        ty,
        encoding: TileEncoding::Skip,
        payload: Vec::new(),
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    }
}

fn make_copy_tile_update(tx: u32, ty: u32, source_index: u32) -> TileUpdate {
    TileUpdate {
        tx,
        ty,
        encoding: TileEncoding::Copy { source_index },
        payload: Vec::new(),
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    }
}

fn seed_decoder(cols: u32, rows: u32, config: &TileConfig) -> TileDecoder {
    let mut decoder = TileDecoder::new(cols, rows, config.clone());
    let tile_bytes = config.tile_bytes();
    for ty in 0..rows {
        for tx in 0..cols {
            let tile = make_tile_bytes(tx * 97 + ty * 29, tile_bytes);
            decoder.commit_tile(tx, ty, Arc::from(tile));
        }
    }
    decoder
}

fn full_frame_batch(cols: u32, rows: u32, tile_bytes: usize) -> TileBatch {
    TileBatch {
        sequence: 0,
        tiles: (0..rows)
            .flat_map(|ty| (0..cols).map(move |tx| make_full_tile_update(tx, ty, tile_bytes)))
            .collect(),
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    }
}

fn reuse_heavy_batch(cols: u32, rows: u32, tile_bytes: usize) -> TileBatch {
    let mut tiles = Vec::with_capacity((cols * rows) as usize);
    for ty in 0..rows {
        for tx in 0..cols {
            let tile = if (tx + ty) % 11 == 0 {
                make_full_tile_update(tx, ty, tile_bytes)
            } else if tx > 0 && tx % 3 == 0 {
                make_copy_tile_update(tx, ty, ty * cols + tx - 1)
            } else {
                make_skip_tile_update(tx, ty)
            };
            tiles.push(tile);
        }
    }
    TileBatch {
        sequence: 1,
        tiles,
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    }
}

fn bench_decode_1000_full_tiles(c: &mut Criterion) {
    let config = tile_config();
    let tile_bytes = config.tile_bytes();
    let cols = 50;
    let rows = 20;
    let decoder = TileDecoder::new(cols, rows, config);

    let updates: Vec<TileUpdate> = (0..1000)
        .map(|i| {
            let tx = i % cols;
            let ty = i / cols;
            make_full_tile_update(tx, ty, tile_bytes)
        })
        .collect();

    c.bench_function("decode_1000_full_tiles_lz4", |b| {
        b.iter(|| {
            for update in &updates {
                let decoded = decoder.decode_tile(black_box(update)).unwrap();
                black_box(decoded.len());
            }
        });
    });
}

fn bench_decode_reuse_paths(c: &mut Criterion) {
    let config = tile_config();
    let cols = 50;
    let rows = 20;
    let decoder = seed_decoder(cols, rows, &config);

    let skip_updates: Vec<TileUpdate> = (0..1000)
        .map(|index| make_skip_tile_update(index % cols, index / cols))
        .collect();
    let copy_updates: Vec<TileUpdate> = (0..1000)
        .map(|index| {
            let tx = index % cols;
            let ty = index / cols;
            let source_index = ty * cols + tx.saturating_sub(1);
            make_copy_tile_update(tx, ty, source_index)
        })
        .collect();

    let mut group = c.benchmark_group("decode_reuse_paths");
    group.bench_function(BenchmarkId::new("skip_arc_reuse", skip_updates.len()), |b| {
        b.iter(|| {
            for update in &skip_updates {
                let decoded = decoder.decode_tile(black_box(update)).unwrap();
                black_box(Arc::as_ptr(&decoded));
            }
        });
    });
    group.bench_function(BenchmarkId::new("copy_arc_reuse", copy_updates.len()), |b| {
        b.iter(|| {
            for update in &copy_updates {
                let decoded = decoder.decode_tile(black_box(update)).unwrap();
                black_box(Arc::as_ptr(&decoded));
            }
        });
    });
    group.finish();
}

fn bench_apply_batch_full_frame(c: &mut Criterion) {
    let config = tile_config();
    let tile_bytes = config.tile_bytes();
    let width = 1920u32;
    let height = 1080u32;
    let cols = width.div_ceil(TILE_SIZE);
    let rows = height.div_ceil(TILE_SIZE);
    let batch = full_frame_batch(cols, rows, tile_bytes);

    c.bench_function("apply_batch_1920x1080_lz4", |b| {
        let mut assembler = FrameAssembler::new(width, height, PixelFormat::Bgra8, config.clone());
        b.iter(|| {
            let result = assembler.apply_batch(black_box(&batch)).unwrap();
            black_box((result.tiles_decoded, result.bytes_decompressed));
        });
    });
}

fn bench_apply_batch_skip_copy_reuse(c: &mut Criterion) {
    let config = tile_config();
    let tile_bytes = config.tile_bytes();
    let width = 1920u32;
    let height = 1080u32;
    let cols = width.div_ceil(TILE_SIZE);
    let rows = height.div_ceil(TILE_SIZE);
    let seed_batch = full_frame_batch(cols, rows, tile_bytes);
    let reuse_batch = reuse_heavy_batch(cols, rows, tile_bytes);

    c.bench_function("apply_batch_1920x1080_skip_copy_reuse", |b| {
        b.iter_batched(
            || {
                let mut assembler =
                    FrameAssembler::new(width, height, PixelFormat::Bgra8, config.clone());
                assembler.apply_batch(&seed_batch).unwrap();
                assembler
            },
            |mut assembler| {
                let result = assembler.apply_batch(black_box(&reuse_batch)).unwrap();
                black_box((result.tiles_decoded, result.tiles_skipped));
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_decode_1000_full_tiles,
    bench_decode_reuse_paths,
    bench_apply_batch_full_frame,
    bench_apply_batch_skip_copy_reuse
);
criterion_main!(benches);
