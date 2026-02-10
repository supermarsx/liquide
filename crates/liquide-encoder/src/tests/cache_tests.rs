use crate::cache::*;

#[test]
fn cache_insert_and_get() {
    let mut cache = TilePayloadCache::new(100);
    cache.insert(123, vec![1, 2, 3]);
    assert!(cache.contains(123));
    assert_eq!(cache.get(123), Some(vec![1, 2, 3].as_slice()));
    assert!(!cache.contains(999));
}

#[test]
fn cache_eviction_at_capacity() {
    let mut cache = TilePayloadCache::new(3);
    cache.insert(1, vec![1]);
    cache.insert(2, vec![2]);
    cache.insert(3, vec![3]);
    assert_eq!(cache.len(), 3);

    // Inserting a 4th should evict one
    cache.insert(4, vec![4]);
    assert_eq!(cache.len(), 3);
    assert!(cache.contains(4));
}

#[test]
fn cache_temperature_demotion() {
    let mut cache = TilePayloadCache::new(10);
    cache.insert(100, vec![10]);

    // Advance 31 frames → Warm
    for _ in 0..31 {
        cache.advance_frame();
    }

    // Check that it's still there (but warm internally)
    assert!(cache.contains(100));

    // Advance to 121 frames → Cold
    for _ in 0..90 {
        cache.advance_frame();
    }
    assert!(cache.contains(100));
}

#[test]
fn cache_cold_evicted_first() {
    let mut cache = TilePayloadCache::new(2);
    cache.insert(1, vec![1]);

    // Age entry 1 to cold
    for _ in 0..121 {
        cache.advance_frame();
    }

    cache.insert(2, vec![2]);
    assert_eq!(cache.len(), 2);

    // Insert 3 — should evict the cold entry (1), not the hot one (2)
    cache.insert(3, vec![3]);
    assert_eq!(cache.len(), 2);
    assert!(!cache.contains(1)); // evicted
    assert!(cache.contains(2));
    assert!(cache.contains(3));
}

#[test]
fn cache_clear() {
    let mut cache = TilePayloadCache::new(10);
    cache.insert(1, vec![1]);
    cache.insert(2, vec![2]);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn cache_contains_after_insert() {
    let mut cache = TilePayloadCache::new(10);
    assert!(!cache.contains(42));
    cache.insert(42, vec![0xAA, 0xBB]);
    assert!(cache.contains(42));
    assert_eq!(cache.get(42), Some(vec![0xAA, 0xBB].as_slice()));
}

#[test]
fn cache_single_entry_eviction() {
    let mut cache = TilePayloadCache::new(1);
    cache.insert(1, vec![10]);
    assert!(cache.contains(1));
    assert_eq!(cache.len(), 1);

    // Insert second entry — first should be evicted
    cache.insert(2, vec![20]);
    assert_eq!(cache.len(), 1);
    assert!(!cache.contains(1));
    assert!(cache.contains(2));
}

#[test]
fn cache_temperature_hot_cold_warm() {
    let mut cache = TilePayloadCache::new(10);
    cache.insert(1, vec![1]);

    // Immediately after insert the entry is hot; accessing it keeps it hot
    assert!(cache.get(1).is_some());

    // Advance 31 frames — entry becomes Warm (threshold is 30)
    for _ in 0..31 {
        cache.advance_frame();
    }
    // Still present even though warm
    assert!(cache.contains(1));

    // Access it to promote back to Hot
    assert!(cache.get(1).is_some());

    // Advance 31 more frames — again Warm
    for _ in 0..31 {
        cache.advance_frame();
    }
    assert!(cache.contains(1));

    // Advance more to Cold territory (total age > 120 from last access)
    for _ in 0..90 {
        cache.advance_frame();
    }
    assert!(cache.contains(1));
}

#[test]
fn cache_advance_many_frames() {
    let mut cache = TilePayloadCache::new(3);
    cache.insert(1, vec![1]);
    cache.insert(2, vec![2]);
    cache.insert(3, vec![3]);

    // Advance 200 frames — all entries become Cold
    for _ in 0..200 {
        cache.advance_frame();
    }

    // All entries still exist (eviction only happens on insert)
    assert_eq!(cache.len(), 3);

    // Now insert a new entry — the cold ones should be evicted first
    cache.insert(4, vec![4]);
    assert_eq!(cache.len(), 3);
    assert!(cache.contains(4));
    // At least one of the old entries was evicted
    let remaining_old = [1u32, 2, 3].iter().filter(|&&k| cache.contains(k)).count();
    assert_eq!(remaining_old, 2);
}
