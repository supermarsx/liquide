use criterion::{criterion_group, criterion_main, Criterion};

use liquide_compositor::damage::DamageClass;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::compress::compress_lz4;
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{FrameStats, TileBatch, TileConfig, TileEncoding, TileUpdate};

use liquide_client_renderer::decoder::TileDecoder;
use liquide_client_renderer::frame::FrameAssembler;

fn make_full_tile_update(tx: u32, ty: u32, tile_bytes: usize) -> TileUpdate {
    let raw = vec![((tx * 7 + ty * 13) & 0xFF) as u8; tile_bytes];
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

fn bench_decode_1000_full_tiles(c: &mut Criterion) {
    let config = TileConfig {
        tile_size: 64,
        bpp: 4,
    };
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
                let _ = decoder.decode_tile(update);
            }
        });
    });
}

fn bench_apply_batch_full_frame(c: &mut Criterion) {
    let config = TileConfig {
        tile_size: 64,
        bpp: 4,
    };
    let tile_bytes = config.tile_bytes();
    let width = 1920u32;
    let height = 1080u32;
    let cols = width.div_ceil(64);
    let rows = height.div_ceil(64);

    let tiles: Vec<TileUpdate> = (0..rows)
        .flat_map(|ty| (0..cols).map(move |tx| make_full_tile_update(tx, ty, tile_bytes)))
        .collect();

    let batch = TileBatch {
        sequence: 0,
        tiles,
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    };

    c.bench_function("apply_batch_1920x1080_lz4", |b| {
        let mut assembler =
            FrameAssembler::new(width, height, PixelFormat::Bgra8, config.clone());
        b.iter(|| {
            assembler.apply_batch(&batch).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_decode_1000_full_tiles,
    bench_apply_batch_full_frame
);
criterion_main!(benches);
