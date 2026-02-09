use crate::pixel::*;

#[test]
fn pixel_format_bpp() {
    assert_eq!(PixelFormat::Bgra8.bytes_per_pixel(), 4);
    assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), 3);
    assert_eq!(PixelFormat::Rgb565.bytes_per_pixel(), 2);
}

#[test]
fn pixel_format_wire_roundtrip() {
    for fmt in [
        PixelFormat::Bgra8,
        PixelFormat::Rgba8,
        PixelFormat::Rgb8,
        PixelFormat::Rgb565,
        PixelFormat::Rgb101010,
        PixelFormat::Rgba1010102,
    ] {
        assert_eq!(PixelFormat::from_wire_name(fmt.wire_name()), Some(fmt));
    }
}

#[test]
fn color_bgra_roundtrip() {
    let c = Color::new(100, 150, 200, 255);
    let bytes = c.to_bgra_bytes();
    assert_eq!(bytes, [200, 150, 100, 255]);
    assert_eq!(Color::from_bgra_bytes(bytes), c);
}

#[test]
fn color_premultiply() {
    let c = Color::new(200, 100, 50, 128);
    let pm = c.premultiply();
    // 200 * 128 / 255 ≈ 100
    assert!((pm.r as i16 - 100).abs() <= 1);
    assert_eq!(pm.a, 128);
}

#[test]
fn color_premultiply_opaque() {
    let c = Color::new(200, 100, 50, 255);
    assert_eq!(c.premultiply(), c);
}

#[test]
fn color_premultiply_transparent() {
    let c = Color::new(200, 100, 50, 0);
    assert_eq!(c.premultiply(), Color::TRANSPARENT);
}

#[test]
fn color_rgba_u32_roundtrip() {
    let c = Color::new(0xAA, 0xBB, 0xCC, 0xDD);
    let packed = c.to_rgba_u32();
    assert_eq!(packed, 0xAABBCCDD);
    assert_eq!(Color::from_rgba_u32(packed), c);
}
