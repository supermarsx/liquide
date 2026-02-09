use crate::blend::*;
use liquide_compositor::pixel::{BlendMode, Color};

#[test]
fn src_over_opaque() {
    let dst = Color::new(100, 100, 100, 255);
    let src = Color::new(200, 150, 50, 255);
    assert_eq!(blend_src_over(dst, src), src);
}

#[test]
fn src_over_transparent() {
    let dst = Color::new(100, 100, 100, 255);
    let src = Color::new(200, 150, 50, 0);
    assert_eq!(blend_src_over(dst, src), dst);
}

#[test]
fn src_over_half_alpha() {
    let dst = Color::new(0, 0, 0, 255);
    let src = Color::new(128, 0, 0, 128).premultiply();
    let result = blend_src_over(dst, src);
    // src.r = 64 (premultiplied), dst.r = 0 * (255-128)/255 ≈ 0
    // result.r ≈ 64
    assert!((result.r as i16 - 64).abs() <= 2);
}

#[test]
fn blend_src_mode() {
    let src = Color::new(42, 42, 42, 42);
    assert_eq!(blend_src(src), src);
}

#[test]
fn multiply_white_identity() {
    let c = Color::new(100, 150, 200, 255);
    let result = blend_multiply(c, Color::WHITE);
    assert_eq!(result.r, 100);
    assert_eq!(result.g, 150);
    assert_eq!(result.b, 200);
}

#[test]
fn screen_black_identity() {
    let c = Color::new(100, 150, 200, 255);
    let result = blend_screen(c, Color::BLACK);
    assert_eq!(result.r, 100);
    assert_eq!(result.g, 150);
    assert_eq!(result.b, 200);
}

#[test]
fn scanline_blend() {
    let mut dst = vec![0u8; 8]; // 2 pixels
    let src = [255, 0, 0, 255, 0, 255, 0, 255]; // blue, green (BGRA)
    blend_scanline(&mut dst, &src, BlendMode::SrcOver);
    assert_eq!(&dst[0..4], &[255, 0, 0, 255]); // blue
    assert_eq!(&dst[4..8], &[0, 255, 0, 255]); // green
}
