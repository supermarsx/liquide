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

#[test]
fn tile_grid_edge_tiles() {
    // 100x100 pixels with tile_size=64 gives 2x2 grid
    // Edge tiles (col 1, row 1) cover pixels 64..99 = 36 pixels wide/tall
    let config = TileConfig { tile_size: 64, bpp: 4 };
    let grid = TileGrid::new(100, 100, config.clone());
    assert_eq!(grid.cols, 2);
    assert_eq!(grid.rows, 2);

    // Extract the bottom-right edge tile from a 100x100 frame buffer
    let codec = TileCodec::new(config);
    let pixels = vec![0xABu8; 100 * 100 * 4];
    let stride = 100 * 4;
    let tile = codec.extract_tile(&pixels, stride, 100, 100, 1, 1);

    // The extracted tile is always tile_size^2 * bpp bytes
    assert_eq!(tile.len(), 64 * 64 * 4);

    // The first 36 pixels of each of the first 36 rows should be 0xAB (from the buffer)
    // and the remaining pixels should be zero-padded
    let row_valid_bytes = 36 * 4;
    for row in 0..36 {
        let off = row * 64 * 4;
        // Valid region should be filled with 0xAB
        assert!(
            tile[off..off + row_valid_bytes].iter().all(|&b| b == 0xAB),
            "row {row}: valid region should be 0xAB"
        );
        // Padding region should be zero
        assert!(
            tile[off + row_valid_bytes..off + 64 * 4].iter().all(|&b| b == 0),
            "row {row}: padding should be zero"
        );
    }

    // Rows 36..63 should be entirely zero (past the frame boundary)
    for row in 36..64 {
        let off = row * 64 * 4;
        assert!(
            tile[off..off + 64 * 4].iter().all(|&b| b == 0),
            "row {row}: past-boundary rows should be zero"
        );
    }
}

#[test]
fn tile_batch_compression_ratio() {
    let mut batch = TileBatch::new(42);
    assert_eq!(batch.compression_ratio(), 0.0);

    batch.uncompressed_bytes = 10000;
    batch.compressed_bytes = 2500;
    let ratio = batch.compression_ratio();
    assert!((ratio - 0.25).abs() < 0.001, "expected 0.25, got {ratio}");

    // Also push some tiles to verify dirty_count interaction
    batch.tiles.push(TileUpdate {
        tx: 0,
        ty: 0,
        encoding: TileEncoding::Full,
        payload: vec![0; 1000],
        crc: 111,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Zstd { level: 3 },
    });
    batch.tiles.push(TileUpdate {
        tx: 1,
        ty: 0,
        encoding: TileEncoding::Delta,
        payload: vec![0; 500],
        crc: 222,
        damage_class: DamageClass::TextGlyph,
        compression: CompressionMethod::Zstd { level: 3 },
    });
    batch.tiles.push(TileUpdate {
        tx: 2,
        ty: 0,
        encoding: TileEncoding::Skip,
        payload: vec![],
        crc: 333,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    });
    assert_eq!(batch.dirty_count(), 2); // Full + Delta, not Skip
}
