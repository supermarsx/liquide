use crate::rasterizer::*;
use liquide_compositor::pixel::PixelFormat;

use crate::color::SrgbLut;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

#[test]
fn fill_rect_solid() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fill_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 20.0, 20.0),
        Color::new(255, 0, 0, 255),
        BlendMode::SrcOver,
    );
    let c = fb.get_pixel(15, 15);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    // Outside
    assert_eq!(fb.get_pixel(5, 5).r, 0);
}

#[test]
fn fill_rounded_rect_basic() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fill_rounded_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 40.0, 30.0),
        5.0,
        &Fill::Solid(Color::new(0, 255, 0, 255)),
        BlendMode::SrcOver,
        &lut,
    );
    // Centre should be green
    let c = fb.get_pixel(30, 25);
    assert_eq!(c.g, 255);
}

#[test]
fn blit_opaque_basic() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    let src = [255u8, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]; // 2x2 blue pixels (BGRA)
    blit_opaque(&mut fb, &src, 2, 2, 5, 5);
    let c = fb.get_pixel(5, 5);
    assert_eq!(c.b, 255);
    assert_eq!(c.a, 255);
}
