use std::sync::Arc;

use liquide_compositor::damage::DamageClass;
use liquide_encoder::compress::{compress_lz4, compress_zstd};
use liquide_encoder::delta::xor_delta;
use liquide_encoder::strategy::CompressionMethod;
use liquide_encoder::tile::{TileConfig, TileEncoding, TileUpdate};

use crate::decoder::TileDecoder;

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

#[test]
fn test_new_decoder() {
    let d = TileDecoder::new(4, 3, default_config());
    assert_eq!(d.cols(), 4);
    assert_eq!(d.rows(), 3);
    assert_eq!(d.config().tile_size, 4);
}

#[test]
fn test_decode_full_zstd() {
    let config = default_config();
    let d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();
    let raw = vec![0xAA; tile_bytes];
    let compressed = compress_zstd(&raw, 3).unwrap();

    let update = make_update(
        0,
        0,
        TileEncoding::Full,
        compressed,
        CompressionMethod::Zstd { level: 3 },
    );

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), raw.as_slice());
}

#[test]
fn test_decode_full_lz4() {
    let config = default_config();
    let d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();
    let raw = vec![0xBB; tile_bytes];
    let compressed = compress_lz4(&raw);

    let update = make_update(0, 0, TileEncoding::Full, compressed, CompressionMethod::Lz4);

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), raw.as_slice());
}

#[test]
fn test_decode_skip_with_previous() {
    let config = default_config();
    let mut d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();
    let previous = vec![0xCC; tile_bytes];
    d.commit_tile(0, 0, Arc::<[u8]>::from(previous.clone()));

    let update = make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4);

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), previous.as_slice());
}

#[test]
fn test_decode_skip_without_previous() {
    let config = default_config();
    let d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();

    let update = make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4);

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), vec![0u8; tile_bytes].as_slice());
}

#[test]
fn test_decode_delta() {
    let config = default_config();
    let mut d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();

    let previous = vec![0x10; tile_bytes];
    let current = vec![0x30; tile_bytes];
    d.commit_tile(1, 0, Arc::<[u8]>::from(previous.clone()));

    let delta = xor_delta(&current, &previous);
    let compressed = compress_zstd(&delta, 3).unwrap();

    let update = make_update(
        1,
        0,
        TileEncoding::Delta,
        compressed,
        CompressionMethod::Zstd { level: 3 },
    );

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), current.as_slice());
}

#[test]
fn test_decode_solid() {
    let config = default_config();
    let d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();

    let color = vec![0xFF, 0x00, 0xFF, 0x80];
    let update = make_update(
        0,
        0,
        TileEncoding::Solid,
        color.clone(),
        CompressionMethod::Lz4,
    );

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.len(), tile_bytes);
    for chunk in decoded.chunks_exact(4) {
        assert_eq!(chunk, &color[..]);
    }
}

#[test]
fn test_decode_copy() {
    let config = default_config();
    let mut d = TileDecoder::new(4, 4, config.clone());
    let tile_bytes = config.tile_bytes();
    let source_data = vec![0x42; tile_bytes];

    // Commit a tile at linear index 5 (tx=1, ty=1 in a 4-col grid)
    d.commit_tile(1, 1, Arc::<[u8]>::from(source_data.clone()));

    let update = make_update(
        2,
        2,
        TileEncoding::Copy { source_index: 5 },
        Vec::new(),
        CompressionMethod::Lz4,
    );

    let decoded = d.decode_tile(&update).unwrap();
    assert_eq!(decoded.as_ref(), source_data.as_slice());
}

#[test]
fn test_decode_invalid_coords() {
    let d = TileDecoder::new(2, 2, default_config());
    let update = make_update(5, 5, TileEncoding::Full, Vec::new(), CompressionMethod::Lz4);

    let result = d.decode_tile(&update);
    assert!(result.is_err());
}

#[test]
fn test_commit_and_delta_cycle() {
    let config = default_config();
    let mut d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();

    // Frame 1: full tile
    let frame1 = vec![0x10; tile_bytes];
    let compressed1 = compress_lz4(&frame1);
    let update1 = make_update(
        0,
        0,
        TileEncoding::Full,
        compressed1,
        CompressionMethod::Lz4,
    );
    let decoded1 = d.decode_tile(&update1).unwrap();
    assert_eq!(decoded1.as_ref(), frame1.as_slice());
    d.commit_tile(0, 0, decoded1);

    // Frame 2: delta
    let frame2 = vec![0x20; tile_bytes];
    let delta = xor_delta(&frame2, &frame1);
    let compressed2 = compress_lz4(&delta);
    let update2 = make_update(
        0,
        0,
        TileEncoding::Delta,
        compressed2,
        CompressionMethod::Lz4,
    );
    let decoded2 = d.decode_tile(&update2).unwrap();
    assert_eq!(decoded2.as_ref(), frame2.as_slice());
}

#[test]
fn test_reset() {
    let config = default_config();
    let mut d = TileDecoder::new(2, 2, config.clone());
    let tile_bytes = config.tile_bytes();
    d.commit_tile(0, 0, Arc::<[u8]>::from(vec![0xFF; tile_bytes]));
    d.reset();

    // After reset, skip should return zeros
    let update = make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4);
    let decoded = d.decode_tile(&update).unwrap();
    assert!(decoded.iter().all(|&b| b == 0));
}

#[test]
fn test_resize_decoder() {
    let mut d = TileDecoder::new(2, 2, default_config());
    d.commit_tile(0, 0, Arc::<[u8]>::from(vec![0xFF; 64]));
    d.resize(4, 4);
    assert_eq!(d.cols(), 4);
    assert_eq!(d.rows(), 4);
}

#[test]
fn test_skip_reuses_shared_tile_buffer() {
    let config = default_config();
    let mut d = TileDecoder::new(2, 2, config.clone());
    let previous: Arc<[u8]> = vec![0xAB; config.tile_bytes()].into();
    d.commit_tile(0, 0, Arc::clone(&previous));

    let update = make_update(0, 0, TileEncoding::Skip, Vec::new(), CompressionMethod::Lz4);

    let decoded = d.decode_tile(&update).unwrap();
    assert!(Arc::ptr_eq(&decoded, &previous));
}

#[test]
fn test_copy_reuses_shared_tile_buffer() {
    let config = default_config();
    let mut d = TileDecoder::new(4, 4, config.clone());
    let source: Arc<[u8]> = vec![0x3C; config.tile_bytes()].into();
    d.commit_tile(1, 0, Arc::clone(&source));

    let update = make_update(
        0,
        1,
        TileEncoding::Copy { source_index: 1 },
        Vec::new(),
        CompressionMethod::Lz4,
    );

    let decoded = d.decode_tile(&update).unwrap();
    assert!(Arc::ptr_eq(&decoded, &source));
}

#[test]
fn test_solid_payload_too_short() {
    let d = TileDecoder::new(2, 2, default_config());
    let update = make_update(
        0,
        0,
        TileEncoding::Solid,
        vec![0xFF, 0x00],
        CompressionMethod::Lz4,
    );
    assert!(d.decode_tile(&update).is_err());
}

#[test]
fn test_display() {
    let d = TileDecoder::new(
        30,
        17,
        TileConfig {
            tile_size: 64,
            bpp: 4,
        },
    );
    let display = format!("{d}");
    assert!(display.contains("30x17"));
    assert!(display.contains("64"));
}
