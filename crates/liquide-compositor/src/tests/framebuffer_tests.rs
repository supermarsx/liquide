use crate::framebuffer::*;
use crate::pixel::{Color, PixelFormat};

#[test]
fn framebuffer_basic() {
    let fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    assert_eq!(fb.byte_len(), 64 * 64 * 4);
    assert_eq!(fb.stride, 64 * 4);
}

#[test]
fn framebuffer_pixel_roundtrip() {
    let mut fb = FrameBuffer::new(10, 10, PixelFormat::Bgra8);
    let c = Color::new(100, 150, 200, 255);
    fb.set_pixel(3, 5, c);
    assert_eq!(fb.get_pixel(3, 5), c);
}

#[test]
fn framebuffer_clear() {
    let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
    fb.clear(Color::new(255, 0, 0, 255));
    let c = fb.get_pixel(2, 2);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn framebuffer_tile_region() {
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    // Set pixel at (64, 64) which is tile (1, 1) in a 64-wide grid
    fb.set_pixel(64, 64, Color::WHITE);
    let tile = fb.tile_region(1, 1, 64);
    // First pixel of this tile should be white
    assert_eq!(tile[0], 255); // B
    assert_eq!(tile[1], 255); // G
    assert_eq!(tile[2], 255); // R
    assert_eq!(tile[3], 255); // A
}

#[test]
fn framebuffer_tile_grid_dims() {
    let fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    assert_eq!(fb.tile_grid_width(64), 30);
    assert_eq!(fb.tile_grid_height(64), 17); // ceil(1080/64)
}

#[test]
fn double_buffer_swap() {
    let mut db = DoubleBuffer::new(4, 4, PixelFormat::Bgra8);
    db.back_mut().set_pixel(0, 0, Color::WHITE);
    assert_eq!(db.front().get_pixel(0, 0), Color::TRANSPARENT);
    db.swap();
    assert_eq!(db.front().get_pixel(0, 0), Color::WHITE);
}
