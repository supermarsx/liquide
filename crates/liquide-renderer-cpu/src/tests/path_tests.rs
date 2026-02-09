use crate::path::*;
use crate::rasterizer::Fill;

use crate::color::SrgbLut;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};

#[test]
fn triangle_fill_covers_interior() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let path = PathBuilder::new()
        .move_to(32.0, 10.0)
        .line_to(10.0, 50.0)
        .line_to(54.0, 50.0)
        .close()
        .build();

    fill_path(
        &mut fb,
        &path,
        &Fill::Solid(Color::new(255, 0, 0, 255)),
        BlendMode::SrcOver,
        &lut,
    );

    // Center of triangle should be red
    let center = fb.get_pixel(32, 35);
    assert!(center.r > 200, "center should be red: got {:?}", center);

    // Outside the triangle should be black (untouched)
    let outside = fb.get_pixel(5, 5);
    assert_eq!(outside.r, 0, "outside should be untouched: got {:?}", outside);
}

#[test]
fn rectangle_fill_via_path() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let path = PathBuilder::new()
        .move_to(10.0, 10.0)
        .line_to(50.0, 10.0)
        .line_to(50.0, 50.0)
        .line_to(10.0, 50.0)
        .close()
        .build();

    fill_path(
        &mut fb,
        &path,
        &Fill::Solid(Color::new(0, 255, 0, 255)),
        BlendMode::SrcOver,
        &lut,
    );

    // Center should be green
    let center = fb.get_pixel(30, 30);
    assert_eq!(center.g, 255, "center should be green: got {:?}", center);

    // Corner should be green
    let corner = fb.get_pixel(11, 11);
    assert!(corner.g > 200, "interior corner should be green: got {:?}", corner);
}

#[test]
fn quadratic_bezier_produces_curve() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    // A shape using a quadratic Bézier to create a curved top
    let path = PathBuilder::new()
        .move_to(10.0, 50.0)
        .quad_to(32.0, 5.0, 54.0, 50.0)
        .close()
        .build();

    fill_path(
        &mut fb,
        &path,
        &Fill::Solid(Color::new(0, 0, 255, 255)),
        BlendMode::SrcOver,
        &lut,
    );

    // The shape interior near the bottom should be filled
    let bottom = fb.get_pixel(32, 45);
    assert!(bottom.b > 200, "bottom should be filled: got {:?}", bottom);

    // Top center (near the apex of the curve) should be filled
    let top = fb.get_pixel(32, 30);
    assert!(top.b > 200, "curved region should be filled: got {:?}", top);
}

#[test]
fn cubic_bezier_circle_approximation() {
    let lut = SrgbLut::new();
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    // Approximate a circle using 4 cubic Bézier segments
    // Radius 20, center (32, 32)
    let r = 20.0_f32;
    let cx = 32.0_f32;
    let cy = 32.0_f32;
    let k = r * 0.5522847498; // standard kappa for circle approximation

    let path = PathBuilder::new()
        .move_to(cx + r, cy)
        .cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r)
        .cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy)
        .cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r)
        .cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy)
        .close()
        .build();

    fill_path(
        &mut fb,
        &path,
        &Fill::Solid(Color::WHITE),
        BlendMode::SrcOver,
        &lut,
    );

    // Center should be white
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.r, 255, "center should be white: got {:?}", center);

    // Far outside should be black
    let outside = fb.get_pixel(5, 5);
    assert_eq!(outside.r, 0, "outside should be black: got {:?}", outside);
}

#[test]
fn stroke_triangle() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    let path = PathBuilder::new()
        .move_to(32.0, 10.0)
        .line_to(10.0, 50.0)
        .line_to(54.0, 50.0)
        .close()
        .build();

    stroke_path(
        &mut fb,
        &path,
        2.0,
        Color::new(255, 0, 0, 255),
        BlendMode::SrcOver,
    );

    // Bottom edge should have red pixels
    let bottom_edge = fb.get_pixel(32, 50);
    assert!(bottom_edge.r > 200, "bottom edge should be red: got {:?}", bottom_edge);

    // Center of the triangle should remain untouched (black)
    let center = fb.get_pixel(32, 35);
    assert!(center.r < 50, "center should be mostly untouched: got {:?}", center);
}

#[test]
fn stroke_width_affects_coverage() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);

    // Simple horizontal line
    let path = PathBuilder::new()
        .move_to(10.0, 32.0)
        .line_to(54.0, 32.0)
        .build();

    stroke_path(
        &mut fb,
        &path,
        4.0,
        Color::WHITE,
        BlendMode::SrcOver,
    );

    // Center of the line should be white
    let center = fb.get_pixel(32, 32);
    assert_eq!(center.r, 255, "center of stroke should be white: got {:?}", center);

    // 1 pixel above center should still be in the stroke (half width = 2)
    let above = fb.get_pixel(32, 31);
    assert!(above.r > 200, "1px above should be in stroke: got {:?}", above);

    // 3 pixels above center should be outside the stroke
    let far_above = fb.get_pixel(32, 29);
    assert!(far_above.r < 50, "3px above should be outside stroke: got {:?}", far_above);
}

#[test]
fn path_bounds_correct() {
    let path = PathBuilder::new()
        .move_to(10.0, 20.0)
        .line_to(50.0, 20.0)
        .line_to(50.0, 60.0)
        .line_to(10.0, 60.0)
        .close()
        .build();

    let b = path.bounds();
    assert!((b.x - 10.0).abs() < 0.01);
    assert!((b.y - 20.0).abs() < 0.01);
    assert!((b.width - 40.0).abs() < 0.01);
    assert!((b.height - 40.0).abs() < 0.01);
}
