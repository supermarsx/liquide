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

#[test]
fn blend_all_modes_dispatch() {
    let dst = Color::new(100, 100, 100, 255);
    let src = Color::new(200, 50, 50, 128);
    // Call blend for every BlendMode variant — just verify no panic
    let _ = blend(dst, src, BlendMode::SrcOver);
    let _ = blend(dst, src, BlendMode::Src);
    let _ = blend(dst, src, BlendMode::Multiply);
    let _ = blend(dst, src, BlendMode::Screen);
    let _ = blend(dst, src, BlendMode::SrcAtop);
}

#[test]
fn blend_scanline_empty() {
    let mut dst: [u8; 0] = [];
    let src: [u8; 0] = [];
    blend_scanline(&mut dst, &src, BlendMode::SrcOver);
    // Should not panic
}

#[test]
fn blend_multiply_color() {
    // white * red = red
    let white = Color::WHITE;
    let red = Color::new(255, 0, 0, 255);
    let result = blend_multiply(white, red);
    assert_eq!(result.r, 255, "multiply white*red R: got {}", result.r);
    assert_eq!(result.g, 0, "multiply white*red G: got {}", result.g);
    assert_eq!(result.b, 0, "multiply white*red B: got {}", result.b);
    assert_eq!(result.a, 255, "multiply white*red A: got {}", result.a);
}

#[test]
fn blend_screen_color() {
    // black screen red = red
    let black = Color::BLACK;
    let red = Color::new(255, 0, 0, 255);
    let result = blend_screen(black, red);
    assert_eq!(result.r, 255, "screen black+red R: got {}", result.r);
    assert_eq!(result.g, 0, "screen black+red G: got {}", result.g);
    assert_eq!(result.b, 0, "screen black+red B: got {}", result.b);
    assert_eq!(result.a, 255, "screen black+red A: got {}", result.a);
}
