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
