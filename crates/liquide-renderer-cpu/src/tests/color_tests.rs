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

#[test]
fn srgb_linearize_black() {
    let lut = SrgbLut::new();
    let val = lut.linearize(0);
    assert!((val - 0.0).abs() < 0.001, "linearize(0) should be ~0.0, got {val}");
}

#[test]
fn srgb_delinearize_white() {
    let lut = SrgbLut::new();
    let val = lut.delinearize(1.0);
    assert_eq!(val, 255, "delinearize(1.0) should be 255, got {val}");
}

#[test]
fn lerp_linear_midpoint() {
    let lut = SrgbLut::new();
    let black = Color::BLACK;
    let white = Color::WHITE;
    let mid = lerp_linear(&lut, black, white, 0.5);
    // In linear space, midpoint of [0,1] is 0.5
    // sRGB delinearize(0.5) is around 188 due to gamma curve
    assert!(mid.r > 170 && mid.r < 200,
        "midpoint R should be ~188 (linear mid-gray in sRGB), got {}", mid.r);
    assert!(mid.g > 170 && mid.g < 200,
        "midpoint G should be ~188 (linear mid-gray in sRGB), got {}", mid.g);
    assert!(mid.b > 170 && mid.b < 200,
        "midpoint B should be ~188 (linear mid-gray in sRGB), got {}", mid.b);
    assert_eq!(mid.a, 255, "midpoint alpha should be 255, got {}", mid.a);
}

#[test]
fn lerp_linear_endpoints_precise() {
    let lut = SrgbLut::new();
    let a = Color::new(50, 100, 150, 200);
    let b = Color::new(200, 50, 100, 250);

    let at_zero = lerp_linear(&lut, a, b, 0.0);
    assert!((at_zero.r as i16 - a.r as i16).abs() <= 1,
        "lerp at t=0 R: got {}, expected {}", at_zero.r, a.r);
    assert!((at_zero.g as i16 - a.g as i16).abs() <= 1,
        "lerp at t=0 G: got {}, expected {}", at_zero.g, a.g);
    assert!((at_zero.b as i16 - a.b as i16).abs() <= 1,
        "lerp at t=0 B: got {}, expected {}", at_zero.b, a.b);

    let at_one = lerp_linear(&lut, a, b, 1.0);
    assert!((at_one.r as i16 - b.r as i16).abs() <= 1,
        "lerp at t=1 R: got {}, expected {}", at_one.r, b.r);
    assert!((at_one.g as i16 - b.g as i16).abs() <= 1,
        "lerp at t=1 G: got {}, expected {}", at_one.g, b.g);
    assert!((at_one.b as i16 - b.b as i16).abs() <= 1,
        "lerp at t=1 B: got {}, expected {}", at_one.b, b.b);
}
