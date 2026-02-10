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

#[test]
fn fill_rect_gradient_linear() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let gradient = Gradient::Linear {
        start: Point::new(10.0, 0.0),
        end: Point::new(50.0, 0.0),
        stops: vec![
            (0.0, Color::new(255, 0, 0, 255)), // red at start
            (1.0, Color::new(0, 0, 255, 255)), // blue at end
        ],
    };

    fill_rect_gradient(
        &mut fb,
        Rect::new(10.0, 10.0, 40.0, 40.0),
        &gradient,
        BlendMode::SrcOver,
        &lut,
    );

    let left = fb.get_pixel(11, 30);
    let right = fb.get_pixel(49, 30);
    // Left side should be more red
    assert!(left.r > left.b, "left side should be more red: got {:?}", left);
    // Right side should be more blue
    assert!(right.b > right.r, "right side should be more blue: got {:?}", right);
}

#[test]
fn fill_circle_basic() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    fill_circle(
        &mut fb,
        Point::new(32.0, 32.0),
        15.0,
        &Fill::Solid(Color::new(0, 200, 0, 255)),
        BlendMode::SrcOver,
        &lut,
    );

    // Center pixel should be green
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.g, 200, "center should be green: got {:?}", center);
    assert_eq!(center.a, 255, "center alpha should be 255: got {:?}", center);

    // Corner pixel should be untouched (black/transparent)
    let corner = fb.get_pixel(0, 0);
    assert_eq!(corner.g, 0, "corner should be untouched: got {:?}", corner);
}

#[test]
fn blit_alpha_with_opacity() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    // Black/transparent background (default from new)

    // 4x4 red image in BGRA
    let mut src = vec![0u8; 4 * 4 * 4];
    for px in src.chunks_exact_mut(4) {
        px[0] = 0;   // B
        px[1] = 0;   // G
        px[2] = 255; // R
        px[3] = 255; // A
    }

    blit_alpha(&mut fb, &src, 4, 4, 5, 5, 0.5);

    // With 50% opacity on black bg: result red should be ~128
    let p = fb.get_pixel(6, 6);
    assert!(p.r > 100 && p.r < 160,
        "blended pixel should be ~128 red: got {:?}", p);
    assert_eq!(p.g, 0, "blended pixel green should be 0: got {:?}", p);
}

#[test]
fn blit_scaled_basic() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    // 2x2 red image in BGRA
    let src = [
        0, 0, 255, 255, // pixel (0,0): red
        0, 0, 255, 255, // pixel (1,0): red
        0, 0, 255, 255, // pixel (0,1): red
        0, 0, 255, 255, // pixel (1,1): red
    ];

    // Scale 2x2 up to 8x8 region at position (4, 4)
    blit_scaled(
        &mut fb,
        &src,
        2,
        2,
        Rect::new(4.0, 4.0, 8.0, 8.0),
    );

    // Center of the scaled region should be red
    let center = fb.get_pixel(8, 8);
    assert!(center.r > 200, "center of scaled blit should be red: got {:?}", center);

    // Outside the scaled region should be untouched
    let outside = fb.get_pixel(0, 0);
    assert_eq!(outside.r, 0, "outside of scaled region should be untouched: got {:?}", outside);
}

#[test]
fn stroke_rect_width() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    stroke_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 44.0, 44.0),
        4.0,
        Color::new(0, 0, 255, 255),
        BlendMode::SrcOver,
    );

    // The top edge should have colored pixels
    let top_edge = fb.get_pixel(32, 10);
    assert!(top_edge.b > 200, "top edge should be blue: got {:?}", top_edge);

    // Center should be untouched
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.b, 0, "center should be untouched: got {:?}", center);
}

#[test]
fn stroke_rounded_rect_width() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    stroke_rounded_rect(
        &mut fb,
        Rect::new(10.0, 10.0, 44.0, 44.0),
        8.0,
        4.0,
        Color::new(255, 0, 0, 255),
        BlendMode::SrcOver,
        &lut,
    );

    // Top edge center should have colored pixels
    let top_edge = fb.get_pixel(32, 10);
    assert!(top_edge.r > 200, "top border should be red: got {:?}", top_edge);

    // Center should be untouched
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.r, 0, "center should be untouched: got {:?}", center);
}
