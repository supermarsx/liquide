use crate::color::*;
use liquide_compositor::pixel::Color;

#[test]
fn roundtrip_black() {
    let lut = SrgbLut::new();
    let l = lut.linearize(0);
    assert!((l - 0.0).abs() < 0.001);
    assert_eq!(lut.delinearize(0.0), 0);
}

#[test]
fn roundtrip_white() {
    let lut = SrgbLut::new();
    let l = lut.linearize(255);
    assert!((l - 1.0).abs() < 0.001);
    assert_eq!(lut.delinearize(1.0), 255);
}

#[test]
fn roundtrip_mid() {
    let lut = SrgbLut::new();
    // sRGB 128 → linear ~0.2158
    let l = lut.linearize(128);
    assert!((l - 0.2158).abs() < 0.01);
    // And back
    let s = lut.delinearize(l);
    assert!((s as i16 - 128).abs() <= 1);
}

#[test]
fn lerp_endpoints() {
    let lut = SrgbLut::new();
    let a = Color::BLACK;
    let b = Color::WHITE;
    let mid = lerp_linear(&lut, a, b, 0.0);
    assert_eq!(mid.r, 0);
    let end = lerp_linear(&lut, a, b, 1.0);
    assert_eq!(end.r, 255);
}
