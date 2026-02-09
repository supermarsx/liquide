use crate::tile::*;
use crate::strategy::CompressionMethod;

use liquide_compositor::damage::DamageClass;

#[test]
fn tile_config_bytes() {
    let cfg = TileConfig::default();
    assert_eq!(cfg.tile_bytes(), 64 * 64 * 4);
}

#[test]
fn tile_grid_dimensions() {
    let grid = TileGrid::new(1920, 1080, TileConfig::default());
    assert_eq!(grid.cols, 30); // 1920 / 64 = 30
    assert_eq!(grid.rows, 17); // ceil(1080 / 64) = 17
    assert_eq!(grid.total_tiles(), 510);
}

#[test]
fn tile_grid_coords() {
    let grid = TileGrid::new(1920, 1080, TileConfig::default());
    assert_eq!(grid.index_to_coords(0), (0, 0));
    assert_eq!(grid.index_to_coords(30), (0, 1));
    assert_eq!(grid.coords_to_index(5, 3), 3 * 30 + 5);
}

#[test]
fn tile_codec_extract() {
    // 8x8 pixels, 4bpp, tile_size=4
    let cfg = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let codec = TileCodec::new(cfg);
    let mut pixels = vec![0u8; 8 * 8 * 4];
    // Mark first pixel
    pixels[0] = 0xFF;
    pixels[1] = 0xAA;
    pixels[2] = 0xBB;
    pixels[3] = 0xCC;

    let tile = codec.extract_tile(&pixels, 8 * 4, 8, 8, 0, 0);
    assert_eq!(tile.len(), 4 * 4 * 4);
    assert_eq!(tile[0], 0xFF);
    assert_eq!(tile[1], 0xAA);
}

#[test]
fn tile_batch_stats() {
    let mut batch = TileBatch::new(1);
    assert_eq!(batch.dirty_count(), 0);
    assert_eq!(batch.compression_ratio(), 0.0);

    batch.tiles.push(TileUpdate {
        tx: 0,
        ty: 0,
        encoding: TileEncoding::Skip,
        payload: vec![],
        crc: 0,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    });
    batch.tiles.push(TileUpdate {
        tx: 1,
        ty: 0,
        encoding: TileEncoding::Full,
        payload: vec![1, 2, 3],
        crc: 123,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Zstd { level: 3 },
    });
    assert_eq!(batch.dirty_count(), 1);
}
