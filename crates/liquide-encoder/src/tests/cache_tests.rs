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
