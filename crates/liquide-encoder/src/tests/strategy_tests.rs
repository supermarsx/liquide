use crate::hash::crc32c;
use crate::strategy::*;

use std::collections::HashMap;

use liquide_compositor::damage::DamageClass;

#[test]
fn t16_encoder_compression_invariants_hold_per_damage_class() {
    let config = StrategyConfig::default();

    assert!(matches!(
        choose_compression(DamageClass::TextGlyph, &config, false),
        CompressionMethod::Zstd { .. }
    ));
    assert!(matches!(
        choose_compression(DamageClass::TextGlyph, &config, true),
        CompressionMethod::Zstd { .. }
    ));
    assert_eq!(
        choose_compression(DamageClass::CursorOnly, &config, false),
        CompressionMethod::Lz4
    );
    assert_eq!(
        choose_compression(DamageClass::CursorOnly, &config, true),
        CompressionMethod::Lz4
    );
    assert!(matches!(
        choose_compression(DamageClass::BitmapRegion, &config, false),
        CompressionMethod::Zstd { .. }
    ));
    assert_eq!(
        choose_compression(DamageClass::BitmapRegion, &config, true),
        CompressionMethod::Lz4
    );
}

#[test]
fn detect_solid_uniform() {
    let tile = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xAA, 0xBB, 0xCC, 0xDD];
    assert_eq!(detect_solid(&tile), Some([0xAA, 0xBB, 0xCC, 0xDD]));
}

#[test]
fn detect_solid_varied() {
    let tile = vec![0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44];
    assert_eq!(detect_solid(&tile), None);
}

#[test]
fn strategy_skip_when_unchanged() {
    let data = vec![1, 2, 3, 4];
    let crc = crc32c(&data);
    let strategy = choose_strategy(
        &data,
        Some(&data),
        crc,
        Some(crc),
        &HashMap::new(),
        DamageClass::UiPrimitive,
        &StrategyConfig::default(),
    );
    assert_eq!(strategy, EncodingStrategy::Skip);
}

#[test]
fn strategy_solid() {
    let data = vec![0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF];
    let crc = crc32c(&data);
    let strategy = choose_strategy(
        &data,
        None,
        crc,
        None,
        &HashMap::new(),
        DamageClass::UiPrimitive,
        &StrategyConfig::default(),
    );
    assert_eq!(
        strategy,
        EncodingStrategy::Solid {
            bgra: [0xFF, 0x00, 0x00, 0xFF]
        }
    );
}

#[test]
fn strategy_delta_small_change() {
    let prev = vec![0u8; 64];
    let mut curr = vec![0u8; 64];
    curr[0] = 1; // tiny change
    let crc = crc32c(&curr);
    let strategy = choose_strategy(
        &curr,
        Some(&prev),
        crc,
        Some(crc32c(&prev)),
        &HashMap::new(),
        DamageClass::UiPrimitive,
        &StrategyConfig::default(),
    );
    assert_eq!(strategy, EncodingStrategy::Delta);
}

#[test]
fn strategy_full_large_change() {
    let prev = vec![0u8; 64];
    // Non-uniform data so solid detection doesn't trigger
    let curr: Vec<u8> = (0..64).map(|i| (i * 7 + 3) as u8).collect();
    let crc = crc32c(&curr);
    let strategy = choose_strategy(
        &curr,
        Some(&prev),
        crc,
        Some(crc32c(&prev)),
        &HashMap::new(),
        DamageClass::BitmapRegion,
        &StrategyConfig::default(),
    );
    assert_eq!(strategy, EncodingStrategy::Full);
}

#[test]
fn build_copy_index_finds_duplicates() {
    let crcs = vec![100, 200, 300, 200, 100, 400];
    let index = build_copy_index(&crcs);
    // 100 first seen at 0, duplicate at 4
    assert_eq!(index.get(&100), Some(&0));
    // 200 first seen at 1, duplicate at 3
    assert_eq!(index.get(&200), Some(&1));
    // 300 and 400 are unique — not in index
    assert!(!index.contains_key(&300));
    assert!(!index.contains_key(&400));
}

// --- Compression method selection tests ---

#[test]
fn compression_cursor_uses_lz4() {
    let config = StrategyConfig::default();
    let method = choose_compression(DamageClass::CursorOnly, &config, false);
    assert_eq!(method, CompressionMethod::Lz4);
}

#[test]
fn compression_text_uses_zstd() {
    let config = StrategyConfig::default();
    let method = choose_compression(DamageClass::TextGlyph, &config, false);
    assert!(matches!(method, CompressionMethod::Zstd { .. }));
}

#[test]
fn compression_bitmap_under_pressure_uses_lz4() {
    let config = StrategyConfig::default();
    let method = choose_compression(DamageClass::BitmapRegion, &config, true);
    assert_eq!(method, CompressionMethod::Lz4);
}

#[test]
fn compression_bitmap_normal_uses_zstd() {
    let config = StrategyConfig::default();
    let method = choose_compression(DamageClass::BitmapRegion, &config, false);
    assert!(matches!(method, CompressionMethod::Zstd { .. }));
}

#[test]
fn compression_cursor_disabled_uses_zstd() {
    let config = StrategyConfig {
        use_lz4_for_cursor: false,
        ..Default::default()
    };
    let method = choose_compression(DamageClass::CursorOnly, &config, false);
    assert!(matches!(method, CompressionMethod::Zstd { .. }));
}

#[test]
fn strategy_duplicate_crc_uses_full_until_copy_semantics_are_explicit() {
    // Build data that is not solid (so solid detection won't trigger)
    let data: Vec<u8> = (0..64).map(|i| (i * 7 + 3) as u8).collect();
    let crc_val = crc32c(&data);

    // Build a copy_index that maps crc_val to tile index 5
    let mut copy_index = HashMap::new();
    copy_index.insert(crc_val, 5u32);

    let strategy = choose_strategy(
        &data,
        None,
        crc_val,
        None, // no prev_crc → won't skip
        &copy_index,
        DamageClass::UiPrimitive,
        &StrategyConfig::default(),
    );
    assert_eq!(strategy, EncodingStrategy::Full);
}

#[test]
fn strategy_config_custom() {
    let config = StrategyConfig {
        delta_threshold: 0.1,
        copy_min_tiles: 10,
        zstd_level: 15,
        use_lz4_for_cursor: false,
    };
    assert!((config.delta_threshold - 0.1).abs() < f32::EPSILON);
    assert_eq!(config.copy_min_tiles, 10);
    assert_eq!(config.zstd_level, 15);
    assert!(!config.use_lz4_for_cursor);

    // Verify the custom zstd_level is reflected in compression selection
    let method = choose_compression(DamageClass::TextGlyph, &config, false);
    assert_eq!(method, CompressionMethod::Zstd { level: 15 });

    // Verify cursor uses Zstd when use_lz4_for_cursor is false
    let method2 = choose_compression(DamageClass::CursorOnly, &config, false);
    assert!(matches!(method2, CompressionMethod::Zstd { .. }));
}
