use crate::strategy::CompressionMethod;
use crate::tile::*;

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
    let config = TileConfig {
        tile_size: 64,
        bpp: 4,
    };
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
            tile[off + row_valid_bytes..off + 64 * 4]
                .iter()
                .all(|&b| b == 0),
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

// --- Overflow / memory-safety regression tests (t49-e7-F1 / t65-shm) ---

#[test]
fn tile_bytes_does_not_wrap_on_huge_tile_size() {
    let cfg = TileConfig {
        tile_size: u32::MAX,
        bpp: 4,
    };
    // Old: (u32::MAX * u32::MAX * 4) as usize -> wrapped tiny value.
    // New: tile edge clamped to MAX_TILE_SIZE, widened -> bounded, non-wrapped.
    let clamped = MAX_TILE_SIZE as usize;
    assert_eq!(cfg.tile_bytes(), clamped * clamped * 4);
}

#[test]
fn tile_bytes_clamps_huge_bpp() {
    let cfg = TileConfig {
        tile_size: 64,
        bpp: u32::MAX,
    };
    // bpp clamped to a sane maximum (16) so it cannot blow up the product.
    assert_eq!(cfg.tile_bytes(), 64usize * 64 * 16);
}

#[test]
fn tile_grid_total_tiles_does_not_wrap() {
    let cfg = TileConfig {
        tile_size: 1,
        bpp: 4,
    };
    let grid = TileGrid::new(u32::MAX, u32::MAX, cfg);
    assert_eq!(grid.cols, MAX_DIMENSION);
    assert_eq!(grid.rows, MAX_DIMENSION);
    let expected = MAX_DIMENSION as usize * MAX_DIMENSION as usize;
    assert_eq!(grid.total_tiles_usize(), expected);
}

#[test]
fn tile_grid_new_handles_zero_tile_size() {
    // tile_size 0 previously caused a divide-by-zero panic in div_ceil.
    let cfg = TileConfig {
        tile_size: 0,
        bpp: 4,
    };
    let grid = TileGrid::new(128, 128, cfg);
    assert_eq!(grid.cols, 128); // tile_size treated as 1
    assert_eq!(grid.rows, 128);
}

#[test]
fn coords_to_index_does_not_wrap() {
    let grid = TileGrid::new(640, 480, TileConfig::default());
    // ty * cols + tx computed in usize; large ty must not wrap a u32 product.
    let idx = grid.coords_to_index(0, u32::MAX);
    assert_eq!(idx, u32::MAX as usize * grid.cols as usize);
}

#[test]
fn extract_tile_hostile_coords_do_not_panic() {
    let cfg = TileConfig {
        tile_size: 4,
        bpp: 4,
    };
    let codec = TileCodec::new(cfg);
    let pixels = vec![0u8; 8 * 8 * 4];
    // Out-of-frame tile coords: must zero-pad, never panic / read OOB.
    let tile = codec.extract_tile(&pixels, 8 * 4, 8, 8, u32::MAX, u32::MAX);
    assert_eq!(tile.len(), 4 * 4 * 4);
    assert!(tile.iter().all(|&b| b == 0));
}

#[test]
fn tile_batch_total_payload_bytes() {
    let mut batch = TileBatch::new(1);

    // Empty batch has 0 payload bytes
    assert_eq!(batch.total_payload_bytes(), 0);

    batch.tiles.push(TileUpdate {
        tx: 0,
        ty: 0,
        encoding: TileEncoding::Full,
        payload: vec![0; 1000],
        crc: 1,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Zstd { level: 3 },
    });
    batch.tiles.push(TileUpdate {
        tx: 1,
        ty: 0,
        encoding: TileEncoding::Delta,
        payload: vec![0; 500],
        crc: 2,
        damage_class: DamageClass::TextGlyph,
        compression: CompressionMethod::Lz4,
    });
    batch.tiles.push(TileUpdate {
        tx: 2,
        ty: 0,
        encoding: TileEncoding::Skip,
        payload: vec![],
        crc: 3,
        damage_class: DamageClass::UiPrimitive,
        compression: CompressionMethod::Lz4,
    });

    assert_eq!(batch.total_payload_bytes(), 1500); // 1000 + 500 + 0
}
