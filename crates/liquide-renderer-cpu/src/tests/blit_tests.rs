use crate::blit::*;
use liquide_compositor::pixel::PixelFormat;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

#[test]
fn clear_region_sets_color() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    clear_region(&mut fb, Rect::new(10.0, 10.0, 20.0, 20.0), Color::WHITE);
    // Inside region
    assert_eq!(fb.get_pixel(15, 15), Color::WHITE);
    // Outside region — still black/transparent
    assert_eq!(fb.get_pixel(5, 5).r, 0);
}

#[test]
fn blit_region_opaque() {
    let mut dst = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    let mut src = FrameBuffer::new(10, 10, PixelFormat::Bgra8);
    src.clear(Color::new(255, 0, 0, 255));

    blit_region(
        &mut dst,
        &src,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        5,
        5,
        BlendMode::SrcOver,
        1.0,
    );

    let c = dst.get_pixel(7, 7);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
}
