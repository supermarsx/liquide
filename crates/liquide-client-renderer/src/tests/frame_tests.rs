use liquide_compositor::damage::DamageClass;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::compress::{compress_lz4, compress_zstd};
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{FrameStats, TileBatch, TileConfig, TileEncoding, TileUpdate};

use crate::frame::FrameAssembler;

fn default_config() -> TileConfig {
    TileConfig {
        tile_size: 4,
        bpp: 4,
    }
}

fn make_update(
    tx: u32,
    ty: u32,
    encoding: TileEncoding,
    payload: Vec<u8>,
    compression: CompressionMethod,
) -> TileUpdate {
    TileUpdate {
        tx,
        ty,
        encoding,
        payload,
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression,
    }
}

fn make_batch(sequence: u64, tiles: Vec<TileUpdate>) -> TileBatch {
    TileBatch {
        sequence,
        tiles,
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        stats: FrameStats::new(),
    }
}

#[test]
fn test_new_assembler() {
    let a = FrameAssembler::new(128, 128, PixelFormat::Bgra8, default_config());
    assert_eq!(a.surface().width(), 128);
    assert_eq!(a.surface().height(), 128);
    assert_eq!(a.frame_count(), 0);
}

#[test]
fn test_apply_single_full_tile() {
    let config = default_config();
    let tile_bytes = config.tile_bytes();
    let raw = vec![0xAA; tile_bytes];
    let compressed = compress_zstd(&raw, 3).unwrap();

    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);
    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Full, compressed, CompressionMethod::Zstd { level: 3 }),
    ]);

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 1);
    assert_eq!(result.tiles_skipped, 0);
    assert_eq!(a.frame_count(), 1);
}

#[test]
fn test_apply_multi_tile_batch() {
    let config = default_config();
    let tile_bytes = config.tile_bytes();

    let raw1 = vec![0x11; tile_bytes];
    let raw2 = vec![0x22; tile_bytes];
    let c1 = compress_lz4(&raw1);
    let c2 = compress_lz4(&raw2);

    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);
    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Full, c1, CompressionMethod::Lz4),
        make_update(1, 0, TileEncoding::Full, c2, CompressionMethod::Lz4),
    ]);

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 2);
    assert_eq!(result.tiles_skipped, 0);
}

#[test]
fn test_skip_counted() {
    let config = default_config();
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);

    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
        make_update(1, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
    ]);

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 0);
    assert_eq!(result.tiles_skipped, 2);
    assert_eq!(result.total_tiles(), 2);
}

#[test]
fn test_frame_count_increments() {
    let config = default_config();
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);

    for i in 0..5 {
        let batch = make_batch(i, vec![
            make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
        ]);
        a.apply_batch(&batch).unwrap();
    }

    assert_eq!(a.frame_count(), 5);
}

#[test]
fn test_resize_resets() {
    let config = default_config();
    let tile_bytes = config.tile_bytes();
    let raw = vec![0xAA; tile_bytes];
    let compressed = compress_lz4(&raw);

    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);
    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Full, compressed, CompressionMethod::Lz4),
    ]);
    a.apply_batch(&batch).unwrap();
    assert_eq!(a.frame_count(), 1);

    a.resize(16, 16);
    assert_eq!(a.frame_count(), 0);
    assert_eq!(a.surface().width(), 16);
    assert_eq!(a.surface().height(), 16);
}

#[test]
fn test_reset() {
    let config = default_config();
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);

    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
    ]);
    a.apply_batch(&batch).unwrap();
    assert_eq!(a.frame_count(), 1);

    a.reset();
    assert_eq!(a.frame_count(), 0);
}

#[test]
fn test_empty_batch() {
    let config = default_config();
    let mut a = FrameAssembler::new(8, 8, PixelFormat::Bgra8, config);
    let batch = make_batch(1, vec![]);

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 0);
    assert_eq!(result.tiles_skipped, 0);
    assert_eq!(a.frame_count(), 1);
}

#[test]
fn test_mixed_encoding_batch() {
    let config = default_config();
    let tile_bytes = config.tile_bytes();
    let raw = vec![0xFF; tile_bytes];
    let compressed = compress_lz4(&raw);
    let solid_color = vec![0xAA, 0xBB, 0xCC, 0xDD];

    let mut a = FrameAssembler::new(12, 12, PixelFormat::Bgra8, config);
    let batch = make_batch(1, vec![
        make_update(0, 0, TileEncoding::Full, compressed, CompressionMethod::Lz4),
        make_update(1, 0, TileEncoding::Solid, solid_color, CompressionMethod::Lz4),
        make_update(2, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4),
    ]);

    let result = a.apply_batch(&batch).unwrap();
    assert_eq!(result.tiles_decoded, 2);
    assert_eq!(result.tiles_skipped, 1);
}

#[test]
fn test_frame_result_display() {
    let result = crate::frame::FrameResult {
        tiles_decoded: 10,
        tiles_skipped: 5,
        bytes_decompressed: 1024,
        decode_time_us: 500,
    };
    let display = format!("{result}");
    assert!(display.contains("decoded=10"));
    assert!(display.contains("skipped=5"));
}

#[test]
fn test_assembler_display() {
    let a = FrameAssembler::new(1920, 1080, PixelFormat::Bgra8, TileConfig::default());
    let display = format!("{a}");
    assert!(display.contains("1920x1080"));
}
