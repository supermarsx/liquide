use crate::strategy::*;
use crate::hash::crc32c;

use std::collections::HashMap;

use liquide_compositor::damage::DamageClass;

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
    assert_eq!(strategy, EncodingStrategy::Solid { bgra: [0xFF, 0x00, 0x00, 0xFF] });
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
