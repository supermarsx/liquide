use crate::rasterizer::*;
use liquide_compositor::pixel::PixelFormat;

use crate::color::SrgbLut;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
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

#[test]
fn radial_gradient_center_vs_edge() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let gradient = Gradient::Radial {
        center: Point::new(32.0, 32.0),
        radius: 30.0,
        stops: vec![
            (0.0, Color::new(255, 0, 0, 255)),  // red at center
            (1.0, Color::new(0, 0, 255, 255)),  // blue at edge
        ],
    };

    fill_rect_gradient(
        &mut fb,
        Rect::new(0.0, 0.0, 64.0, 64.0),
        &gradient,
        BlendMode::SrcOver,
        &lut,
    );

    // Center pixel should be close to red
    let center = fb.get_pixel(32, 32);
    assert!(center.r > 200, "center should be red-ish: got {:?}", center);
    assert!(center.b < 50, "center should have low blue: got {:?}", center);

    // Edge pixel (at radius distance from center) should be close to blue
    let edge = fb.get_pixel(62, 32);
    assert!(edge.b > 200, "edge should be blue-ish: got {:?}", edge);
    assert!(edge.r < 50, "edge should have low red: got {:?}", edge);
}

#[test]
fn radial_gradient_in_rounded_rect() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let gradient = Gradient::Radial {
        center: Point::new(32.0, 32.0),
        radius: 20.0,
        stops: vec![
            (0.0, Color::WHITE),
            (1.0, Color::BLACK),
        ],
    };

    fill_rounded_rect(
        &mut fb,
        Rect::new(12.0, 12.0, 40.0, 40.0),
        8.0,
        &Fill::Gradient(gradient),
        BlendMode::SrcOver,
        &lut,
    );

    // Center should be bright (white-ish)
    let center = fb.get_pixel(32, 32);
    assert!(center.r > 200, "center should be bright: got {:?}", center);

    // Near the edge of the shape should be darker
    let near_edge = fb.get_pixel(20, 32);
    assert!(near_edge.r < center.r, "edge should be darker than center");
}

#[test]
fn stroke_rect_produces_outline() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb.clear(Color::WHITE);

    stroke_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 44.0, 44.0),
        2.0,
        Color::new(255, 0, 0, 255),
        BlendMode::SrcOver,
    );

    // Top edge should be red
    let top = fb.get_pixel(32, 10);
    assert_eq!(top.r, 255, "top edge should be red");

    // Center should still be white
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.r, 255);
    assert_eq!(center.g, 255);
    assert_eq!(center.b, 255);

    // Left edge should be red
    let left = fb.get_pixel(10, 32);
    assert_eq!(left.r, 255, "left edge should be red");
    assert_eq!(left.g, 0, "left edge green should be 0");
}

#[test]
fn stroke_rounded_rect_produces_outline() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb.clear(Color::WHITE);

    stroke_rounded_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 44.0, 44.0),
        8.0,
        2.0,
        Color::new(0, 0, 255, 255),
        BlendMode::SrcOver,
        &lut,
    );

    // Top edge center should be blue
    let top = fb.get_pixel(32, 10);
    assert!(top.b > 200, "top edge should be blue: got {:?}", top);

    // Center should still be white (inside the stroke)
    let center = fb.get_pixel(32, 32);
    assert_eq!(center, Color::WHITE, "center should be white");

    // Outside the rect should still be white
    let outside = fb.get_pixel(5, 5);
    assert_eq!(outside, Color::WHITE, "outside should be white");
}
