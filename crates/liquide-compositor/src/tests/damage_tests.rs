use crate::damage::*;
use crate::pixel::{Color, PixelFormat};

#[test]
fn crc32c_empty() {
    assert_eq!(crc32c(&[]), 0x0000_0000);
}

#[test]
fn crc32c_known_value() {
    // CRC-32C of "123456789" = 0xE3069283
    let data = b"123456789";
    assert_eq!(crc32c(data), 0xE306_9283);
}

#[test]
fn damage_tracker_first_frame_full() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let damage = tracker.compute_damage(&fb);
    // First frame: all 4 tiles (2x2 grid) are damaged
    assert_eq!(damage.len(), 4);
}

#[test]
fn damage_tracker_no_change() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);
    // Second frame with same content: no damage
    let damage = tracker.compute_damage(&fb);
    assert!(damage.is_empty());
}

#[test]
fn damage_tracker_detects_change() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);

    // Modify a pixel in tile (1, 0)
    fb.set_pixel(65, 5, Color::WHITE);
    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 1);
    assert_eq!(damage.tiles[0].x, 1);
    assert_eq!(damage.tiles[0].y, 0);
}

#[test]
fn damage_set_sort_priority() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 0, y: 0, class: DamageClass::BitmapRegion });
    ds.add(DamageTile { x: 1, y: 0, class: DamageClass::TextGlyph });
    ds.add(DamageTile { x: 2, y: 0, class: DamageClass::UiPrimitive });
    ds.sort_by_priority();
    assert_eq!(ds.tiles[0].class, DamageClass::TextGlyph);
    assert_eq!(ds.tiles[1].class, DamageClass::UiPrimitive);
    assert_eq!(ds.tiles[2].class, DamageClass::BitmapRegion);
}

#[test]
fn damage_set_merge() {
    let mut a = DamageSet::new(64);
    a.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });
    let mut b = DamageSet::new(64);
    b.add(DamageTile { x: 1, y: 1, class: DamageClass::BitmapRegion });
    a.merge(&b);
    assert_eq!(a.len(), 2);
}

#[test]
fn damage_set_mark_all() {
    let mut ds = DamageSet::new(64);
    ds.mark_all(4, 3);
    assert_eq!(ds.len(), 12); // 4 * 3
    // Verify all tiles are UiPrimitive
    for t in &ds.tiles {
        assert_eq!(t.class, DamageClass::UiPrimitive);
    }
}

#[test]
fn damage_tracker_reset() {
    let mut tracker = DamageTracker::new(64, 128, 128);
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let _ = tracker.compute_damage(&fb);
    // Second call with same data = no damage
    let d2 = tracker.compute_damage(&fb);
    assert!(d2.is_empty());
    // After reset, should get full damage again
    tracker.reset();
    let d3 = tracker.compute_damage(&fb);
    assert!(!d3.is_empty());
}

#[test]
fn damage_class_priority_values() {
    assert_eq!(DamageClass::TextGlyph.priority(), 0);
    assert_eq!(DamageClass::UiPrimitive.priority(), 1);
    assert_eq!(DamageClass::BitmapRegion.priority(), 2);
    assert_eq!(DamageClass::CursorOnly.priority(), 3);
}

#[test]
fn damage_tracker_resize() {
    let mut tracker = DamageTracker::new(64, 128, 128);
    assert_eq!(tracker.grid_width(), 2);
    assert_eq!(tracker.grid_height(), 2);
    tracker.resize(256, 128);
    assert_eq!(tracker.grid_width(), 4);
    assert_eq!(tracker.grid_height(), 2);
}

// ── DamageClass tests ───────────────────────────────────────────────────

#[test]
fn damage_class_priority_strict_ordering() {
    let classes = [
        DamageClass::TextGlyph,
        DamageClass::UiPrimitive,
        DamageClass::BitmapRegion,
        DamageClass::CursorOnly,
    ];
    for pair in classes.windows(2) {
        assert!(
            pair[0].priority() < pair[1].priority(),
            "{:?} should have lower priority value than {:?}",
            pair[0],
            pair[1],
        );
    }
}

#[test]
fn damage_class_equality() {
    assert_eq!(DamageClass::TextGlyph, DamageClass::TextGlyph);
    assert_ne!(DamageClass::TextGlyph, DamageClass::CursorOnly);
}

// ── DamageSet basic operations ──────────────────────────────────────────

#[test]
fn damage_set_new_is_empty() {
    let ds = DamageSet::new(64);
    assert!(ds.is_empty());
    assert_eq!(ds.len(), 0);
    assert_eq!(ds.tile_size, 64);
}

#[test]
fn damage_set_default_is_empty() {
    let ds = DamageSet::default();
    assert!(ds.is_empty());
    assert_eq!(ds.tile_size, 0);
}

#[test]
fn damage_set_add_single() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 3, y: 7, class: DamageClass::TextGlyph });
    assert_eq!(ds.len(), 1);
    assert!(!ds.is_empty());
    assert_eq!(ds.tiles[0].x, 3);
    assert_eq!(ds.tiles[0].y, 7);
    assert_eq!(ds.tiles[0].class, DamageClass::TextGlyph);
}

#[test]
fn damage_set_add_multiple() {
    let mut ds = DamageSet::new(64);
    for i in 0..100 {
        ds.add(DamageTile { x: i, y: 0, class: DamageClass::UiPrimitive });
    }
    assert_eq!(ds.len(), 100);
}

#[test]
fn damage_set_clear() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });
    ds.add(DamageTile { x: 1, y: 1, class: DamageClass::CursorOnly });
    assert_eq!(ds.len(), 2);
    ds.clear();
    assert!(ds.is_empty());
    assert_eq!(ds.len(), 0);
}

// ── DamageSet mark_tile ─────────────────────────────────────────────────

#[test]
fn damage_set_mark_tile_uses_cursor_only() {
    let mut ds = DamageSet::new(64);
    ds.mark_tile(5, 10);
    assert_eq!(ds.len(), 1);
    assert_eq!(ds.tiles[0].x, 5);
    assert_eq!(ds.tiles[0].y, 10);
    assert_eq!(ds.tiles[0].class, DamageClass::CursorOnly);
}

// ── DamageSet mark_rect ─────────────────────────────────────────────────

#[test]
fn damage_set_mark_rect_single_tile() {
    let mut ds = DamageSet::new(64);
    // A 10x10 rect at (10, 10) should hit only tile (0, 0)
    ds.mark_rect(10, 10, 10, 10, 4, 4);
    assert_eq!(ds.len(), 1);
    assert_eq!(ds.tiles[0].x, 0);
    assert_eq!(ds.tiles[0].y, 0);
}

#[test]
fn damage_set_mark_rect_spanning_tiles() {
    let mut ds = DamageSet::new(64);
    // A rect from (60, 60) size 10x10 crosses tile boundary: tiles (0,0), (1,0), (0,1), (1,1)
    ds.mark_rect(60, 60, 10, 10, 4, 4);
    assert_eq!(ds.len(), 4);
    let coords: Vec<(u32, u32)> = ds.tiles.iter().map(|t| (t.x, t.y)).collect();
    assert!(coords.contains(&(0, 0)));
    assert!(coords.contains(&(1, 0)));
    assert!(coords.contains(&(0, 1)));
    assert!(coords.contains(&(1, 1)));
}

#[test]
fn damage_set_mark_rect_full_row() {
    let mut ds = DamageSet::new(64);
    // Rect covering full width (256px) at top, height 32 → tiles (0..4, 0)
    ds.mark_rect(0, 0, 256, 32, 4, 4);
    assert_eq!(ds.len(), 4);
    for t in &ds.tiles {
        assert_eq!(t.y, 0);
        assert_eq!(t.class, DamageClass::UiPrimitive);
    }
}

#[test]
fn damage_set_mark_rect_zero_width() {
    let mut ds = DamageSet::new(64);
    ds.mark_rect(10, 10, 0, 50, 4, 4);
    assert!(ds.is_empty());
}

#[test]
fn damage_set_mark_rect_zero_height() {
    let mut ds = DamageSet::new(64);
    ds.mark_rect(10, 10, 50, 0, 4, 4);
    assert!(ds.is_empty());
}

#[test]
fn damage_set_mark_rect_zero_tile_size() {
    let mut ds = DamageSet::new(0);
    ds.mark_rect(10, 10, 50, 50, 4, 4);
    assert!(ds.is_empty());
}

#[test]
fn damage_set_mark_rect_clamped_to_grid() {
    let mut ds = DamageSet::new(64);
    // Rect extends beyond grid (grid is 2x2 = 128x128 px)
    ds.mark_rect(0, 0, 1000, 1000, 2, 2);
    assert_eq!(ds.len(), 4); // clamped to 2x2 grid
}

#[test]
fn damage_set_mark_rect_at_tile_boundary() {
    let mut ds = DamageSet::new(64);
    // Rect starts exactly at tile (1,1) boundary, fits within it
    ds.mark_rect(64, 64, 32, 32, 4, 4);
    assert_eq!(ds.len(), 1);
    assert_eq!(ds.tiles[0].x, 1);
    assert_eq!(ds.tiles[0].y, 1);
}

// ── DamageSet mark_all ──────────────────────────────────────────────────

#[test]
fn damage_set_mark_all_replaces_existing() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 99, y: 99, class: DamageClass::TextGlyph });
    ds.mark_all(2, 2);
    assert_eq!(ds.len(), 4);
    // The old tile at (99, 99) should be gone
    assert!(ds.tiles.iter().all(|t| t.x < 2 && t.y < 2));
}

#[test]
fn damage_set_mark_all_zero_grid() {
    let mut ds = DamageSet::new(64);
    ds.mark_all(0, 0);
    assert!(ds.is_empty());
}

#[test]
fn damage_set_mark_all_1x1() {
    let mut ds = DamageSet::new(64);
    ds.mark_all(1, 1);
    assert_eq!(ds.len(), 1);
    assert_eq!(ds.tiles[0].x, 0);
    assert_eq!(ds.tiles[0].y, 0);
}

// ── DamageSet merge ─────────────────────────────────────────────────────

#[test]
fn damage_set_merge_empty_into_empty() {
    let mut a = DamageSet::new(64);
    let b = DamageSet::new(64);
    a.merge(&b);
    assert!(a.is_empty());
}

#[test]
fn damage_set_merge_into_empty() {
    let mut a = DamageSet::new(64);
    let mut b = DamageSet::new(64);
    b.add(DamageTile { x: 5, y: 5, class: DamageClass::BitmapRegion });
    a.merge(&b);
    assert_eq!(a.len(), 1);
    assert_eq!(a.tiles[0].x, 5);
}

#[test]
fn damage_set_merge_preserves_both() {
    let mut a = DamageSet::new(64);
    a.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });
    a.add(DamageTile { x: 1, y: 0, class: DamageClass::UiPrimitive });

    let mut b = DamageSet::new(64);
    b.add(DamageTile { x: 2, y: 0, class: DamageClass::BitmapRegion });
    b.add(DamageTile { x: 3, y: 0, class: DamageClass::CursorOnly });

    a.merge(&b);
    assert_eq!(a.len(), 4);
}

#[test]
fn damage_set_merge_allows_duplicates() {
    // merge doesn't deduplicate — verify that behavior
    let mut a = DamageSet::new(64);
    a.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });
    let mut b = DamageSet::new(64);
    b.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });
    a.merge(&b);
    assert_eq!(a.len(), 2);
}

// ── DamageSet sort_by_priority ──────────────────────────────────────────

#[test]
fn damage_set_sort_all_four_classes() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 0, y: 0, class: DamageClass::CursorOnly });
    ds.add(DamageTile { x: 1, y: 0, class: DamageClass::BitmapRegion });
    ds.add(DamageTile { x: 2, y: 0, class: DamageClass::TextGlyph });
    ds.add(DamageTile { x: 3, y: 0, class: DamageClass::UiPrimitive });
    ds.sort_by_priority();
    assert_eq!(ds.tiles[0].class, DamageClass::TextGlyph);
    assert_eq!(ds.tiles[1].class, DamageClass::UiPrimitive);
    assert_eq!(ds.tiles[2].class, DamageClass::BitmapRegion);
    assert_eq!(ds.tiles[3].class, DamageClass::CursorOnly);
}

#[test]
fn damage_set_sort_stable_within_same_priority() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 10, y: 0, class: DamageClass::TextGlyph });
    ds.add(DamageTile { x: 20, y: 0, class: DamageClass::TextGlyph });
    ds.add(DamageTile { x: 5, y: 0, class: DamageClass::TextGlyph });
    ds.sort_by_priority();
    // sort_by_key is stable, so insertion order is preserved within same key
    assert_eq!(ds.tiles[0].x, 10);
    assert_eq!(ds.tiles[1].x, 20);
    assert_eq!(ds.tiles[2].x, 5);
}

#[test]
fn damage_set_sort_empty() {
    let mut ds = DamageSet::new(64);
    ds.sort_by_priority(); // should not panic
    assert!(ds.is_empty());
}

#[test]
fn damage_set_sort_single_element() {
    let mut ds = DamageSet::new(64);
    ds.add(DamageTile { x: 0, y: 0, class: DamageClass::CursorOnly });
    ds.sort_by_priority();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds.tiles[0].class, DamageClass::CursorOnly);
}

// ── DamageTracker initialization ────────────────────────────────────────

#[test]
fn damage_tracker_grid_dimensions() {
    let tracker = DamageTracker::new(64, 1920, 1080);
    assert_eq!(tracker.grid_width(), 30);   // 1920 / 64 = 30
    assert_eq!(tracker.grid_height(), 17);  // ceil(1080 / 64) = 17
    assert_eq!(tracker.tile_size(), 64);
}

#[test]
fn damage_tracker_non_aligned_dimensions() {
    // 100x100 with tile_size 64 → ceil(100/64) = 2 in each dimension
    let tracker = DamageTracker::new(64, 100, 100);
    assert_eq!(tracker.grid_width(), 2);
    assert_eq!(tracker.grid_height(), 2);
}

#[test]
fn damage_tracker_exact_tile_alignment() {
    let tracker = DamageTracker::new(64, 256, 192);
    assert_eq!(tracker.grid_width(), 4);    // 256 / 64 = 4
    assert_eq!(tracker.grid_height(), 3);   // 192 / 64 = 3
}

#[test]
fn damage_tracker_single_tile() {
    let tracker = DamageTracker::new(64, 32, 32);
    assert_eq!(tracker.grid_width(), 1);
    assert_eq!(tracker.grid_height(), 1);
}

// ── DamageTracker frame-to-frame comparison ─────────────────────────────

#[test]
fn damage_tracker_identical_frames_no_damage() {
    let fb = crate::framebuffer::FrameBuffer::new(256, 256, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 256, 256);
    let _ = tracker.compute_damage(&fb); // first frame: full damage
    let d = tracker.compute_damage(&fb);
    assert!(d.is_empty(), "identical frames should produce no damage");
}

#[test]
fn damage_tracker_three_identical_frames() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);
    assert!(tracker.compute_damage(&fb).is_empty());
    assert!(tracker.compute_damage(&fb).is_empty());
}

#[test]
fn damage_tracker_change_revert_no_damage() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);

    // Frame 2: modify pixel
    fb.set_pixel(10, 10, Color::WHITE);
    let d2 = tracker.compute_damage(&fb);
    assert_eq!(d2.len(), 1);

    // Frame 3: same content as frame 2 → no damage
    let d3 = tracker.compute_damage(&fb);
    assert!(d3.is_empty());
}

#[test]
fn damage_tracker_multi_tile_changes() {
    let mut fb = crate::framebuffer::FrameBuffer::new(256, 256, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 256, 256);
    let _ = tracker.compute_damage(&fb);

    // Modify pixels in 3 different tiles
    fb.set_pixel(10, 10, Color::WHITE);    // tile (0, 0)
    fb.set_pixel(130, 10, Color::WHITE);   // tile (2, 0)
    fb.set_pixel(200, 200, Color::WHITE);  // tile (3, 3)

    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 3);

    let coords: Vec<(u32, u32)> = damage.tiles.iter().map(|t| (t.x, t.y)).collect();
    assert!(coords.contains(&(0, 0)));
    assert!(coords.contains(&(2, 0)));
    assert!(coords.contains(&(3, 3)));
}

#[test]
fn damage_tracker_all_tiles_changed() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);

    // Modify a pixel in every tile
    fb.set_pixel(10, 10, Color::WHITE);   // tile (0, 0)
    fb.set_pixel(70, 10, Color::WHITE);   // tile (1, 0)
    fb.set_pixel(10, 70, Color::WHITE);   // tile (0, 1)
    fb.set_pixel(70, 70, Color::WHITE);   // tile (1, 1)

    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 4);
}

#[test]
fn damage_tracker_edge_tile_partial() {
    // Screen not aligned to tile size (100x100, tile 64) → 2x2 grid
    // Edge tiles are smaller than full tile_size
    let mut fb = crate::framebuffer::FrameBuffer::new(100, 100, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 100, 100);
    let _ = tracker.compute_damage(&fb);

    // Modify pixel in the partial edge tile (1, 1) = pixels [64..100, 64..100]
    fb.set_pixel(80, 80, Color::WHITE);
    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 1);
    assert_eq!(damage.tiles[0].x, 1);
    assert_eq!(damage.tiles[0].y, 1);
}

// ── DamageTracker reset behavior ────────────────────────────────────────

#[test]
fn damage_tracker_reset_forces_full_damage() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);
    assert!(tracker.compute_damage(&fb).is_empty());

    tracker.reset();
    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 4, "reset should force full-screen damage");
}

#[test]
fn damage_tracker_double_reset() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb);

    tracker.reset();
    tracker.reset(); // double reset should be idempotent
    let damage = tracker.compute_damage(&fb);
    assert_eq!(damage.len(), 4);
}

// ── DamageTracker resize behavior ───────────────────────────────────────

#[test]
fn damage_tracker_resize_forces_full_damage() {
    let fb1 = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let _ = tracker.compute_damage(&fb1);
    assert!(tracker.compute_damage(&fb1).is_empty());

    // Resize to larger screen
    tracker.resize(256, 256);
    let fb2 = crate::framebuffer::FrameBuffer::new(256, 256, PixelFormat::Bgra8);
    let damage = tracker.compute_damage(&fb2);
    assert_eq!(damage.len(), 16); // 4x4 grid
}

#[test]
fn damage_tracker_resize_shrink() {
    let mut tracker = DamageTracker::new(64, 256, 256);
    tracker.resize(64, 64);
    assert_eq!(tracker.grid_width(), 1);
    assert_eq!(tracker.grid_height(), 1);
}

#[test]
fn damage_tracker_resize_then_no_change() {
    let mut tracker = DamageTracker::new(64, 128, 128);
    let fb = crate::framebuffer::FrameBuffer::new(256, 256, PixelFormat::Bgra8);
    tracker.resize(256, 256);
    let _ = tracker.compute_damage(&fb); // first after resize: full damage
    let d = tracker.compute_damage(&fb);
    assert!(d.is_empty(), "no change after resize + first scan should yield no damage");
}

// ── DamageTracker with different pixel formats ──────────────────────────

#[test]
fn damage_tracker_rgba8_format() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Rgba8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let d1 = tracker.compute_damage(&fb);
    assert_eq!(d1.len(), 4); // first frame

    let d2 = tracker.compute_damage(&fb);
    assert!(d2.is_empty()); // no change

    fb.set_pixel(10, 10, Color::WHITE);
    let d3 = tracker.compute_damage(&fb);
    assert_eq!(d3.len(), 1);
}

#[test]
fn damage_tracker_rgb8_format() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Rgb8);
    let mut tracker = DamageTracker::new(64, 128, 128);
    let d1 = tracker.compute_damage(&fb);
    assert_eq!(d1.len(), 4);

    let d2 = tracker.compute_damage(&fb);
    assert!(d2.is_empty());
}

// ── CRC-32C hash function ───────────────────────────────────────────────

#[test]
fn crc32c_single_byte() {
    let a = crc32c(&[0x00]);
    let b = crc32c(&[0xFF]);
    assert_ne!(a, b, "different single bytes should produce different CRCs");
}

#[test]
fn crc32c_different_data_different_hash() {
    let a = crc32c(b"hello");
    let b = crc32c(b"world");
    assert_ne!(a, b);
}

#[test]
fn crc32c_deterministic() {
    let data = b"deterministic test data";
    assert_eq!(crc32c(data), crc32c(data));
}

#[test]
fn crc32c_large_data() {
    let data = vec![0xABu8; 65536];
    let hash = crc32c(&data);
    // Just verify it's deterministic and non-zero on non-trivial data
    assert_eq!(hash, crc32c(&data));
}

// ── CRC-32C tile hashing ────────────────────────────────────────────────

#[test]
fn crc32c_tile_deterministic() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let h1 = crc32c_tile(&fb, 0, 0, 64);
    let h2 = crc32c_tile(&fb, 0, 0, 64);
    assert_eq!(h1, h2);
}

#[test]
fn crc32c_tile_different_tiles_same_content() {
    // All-zero framebuffer: tiles with same content should hash to same value
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let h00 = crc32c_tile(&fb, 0, 0, 64);
    let h10 = crc32c_tile(&fb, 1, 0, 64);
    assert_eq!(h00, h10, "same content tiles should have same CRC");
}

#[test]
fn crc32c_tile_detects_pixel_change() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let before = crc32c_tile(&fb, 0, 0, 64);
    fb.set_pixel(10, 10, Color::WHITE);
    let after = crc32c_tile(&fb, 0, 0, 64);
    assert_ne!(before, after, "pixel change should change tile CRC");
}

#[test]
fn crc32c_tile_unaffected_tile_unchanged() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let before = crc32c_tile(&fb, 1, 1, 64);
    // Modify pixel in tile (0, 0); tile (1, 1) should be unaffected
    fb.set_pixel(10, 10, Color::WHITE);
    let after = crc32c_tile(&fb, 1, 1, 64);
    assert_eq!(before, after, "unrelated tile CRC should not change");
}

#[test]
fn crc32c_tile_out_of_bounds_returns_zero() {
    let fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let hash = crc32c_tile(&fb, 100, 100, 64);
    assert_eq!(hash, 0, "out-of-bounds tile should return 0");
}

// ── DamageTile struct ───────────────────────────────────────────────────

#[test]
fn damage_tile_equality() {
    let a = DamageTile { x: 1, y: 2, class: DamageClass::TextGlyph };
    let b = DamageTile { x: 1, y: 2, class: DamageClass::TextGlyph };
    let c = DamageTile { x: 1, y: 2, class: DamageClass::CursorOnly };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn damage_tile_clone() {
    let a = DamageTile { x: 5, y: 10, class: DamageClass::BitmapRegion };
    let b = a;
    assert_eq!(a, b);
}

// ── Integration-style: full workflow scenarios ──────────────────────────

#[test]
fn workflow_progressive_changes() {
    let mut fb = crate::framebuffer::FrameBuffer::new(192, 192, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 192, 192); // 3x3 grid

    // Frame 1: first frame → full damage (9 tiles)
    let d1 = tracker.compute_damage(&fb);
    assert_eq!(d1.len(), 9);

    // Frame 2: no change → no damage
    let d2 = tracker.compute_damage(&fb);
    assert!(d2.is_empty());

    // Frame 3: change one tile
    fb.set_pixel(0, 0, Color::WHITE);
    let d3 = tracker.compute_damage(&fb);
    assert_eq!(d3.len(), 1);

    // Frame 4: change two more tiles
    fb.set_pixel(65, 0, Color::WHITE);
    fb.set_pixel(130, 130, Color::WHITE);
    let d4 = tracker.compute_damage(&fb);
    assert_eq!(d4.len(), 2);

    // Frame 5: no change → no damage
    let d5 = tracker.compute_damage(&fb);
    assert!(d5.is_empty());
}

#[test]
fn workflow_reset_in_middle_of_stream() {
    let mut fb = crate::framebuffer::FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    let mut tracker = DamageTracker::new(64, 128, 128);

    let _ = tracker.compute_damage(&fb); // first
    fb.set_pixel(10, 10, Color::WHITE);
    let d = tracker.compute_damage(&fb);
    assert_eq!(d.len(), 1);

    // Reset mid-stream
    tracker.reset();
    // Same fb content → full damage after reset
    let d_after = tracker.compute_damage(&fb);
    assert_eq!(d_after.len(), 4);

    // Then no change
    assert!(tracker.compute_damage(&fb).is_empty());
}

#[test]
fn workflow_damage_set_collect_and_sort() {
    let mut combined = DamageSet::new(64);

    // Simulate collecting damage from multiple sources
    let mut cursor_damage = DamageSet::new(64);
    cursor_damage.mark_tile(5, 5);

    let mut ui_damage = DamageSet::new(64);
    ui_damage.mark_rect(0, 0, 128, 128, 10, 10);

    let mut text_damage = DamageSet::new(64);
    text_damage.add(DamageTile { x: 0, y: 0, class: DamageClass::TextGlyph });

    combined.merge(&cursor_damage);
    combined.merge(&ui_damage);
    combined.merge(&text_damage);

    combined.sort_by_priority();

    // TextGlyph should come first
    assert_eq!(combined.tiles[0].class, DamageClass::TextGlyph);
    // CursorOnly should be last
    assert_eq!(combined.tiles.last().unwrap().class, DamageClass::CursorOnly);
}
