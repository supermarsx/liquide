use crate::hash::*;

#[test]
fn crc32c_empty() {
    assert_eq!(crc32c(&[]), 0x0000_0000);
}

#[test]
fn crc32c_known_value() {
    // "123456789" should produce 0xE3069283 for CRC-32C
    let data = b"123456789";
    assert_eq!(crc32c(data), 0xE306_9283);
}

#[test]
fn crc32c_tile_basic() {
    // 4x4 pixel buffer, 4 bpp, tile_size=2
    let pixels = vec![0xABu8; 4 * 4 * 4];
    let h1 = crc32c_tile(&pixels, 4 * 4, 0, 0, 2, 4, 4, 4);
    let h2 = crc32c_tile(&pixels, 4 * 4, 1, 1, 2, 4, 4, 4);
    // Same data → same hash
    assert_eq!(h1, h2);
}

#[test]
fn crc32c_detects_change() {
    let mut pixels = vec![0u8; 4 * 4 * 4];
    let h1 = crc32c_tile(&pixels, 4 * 4, 0, 0, 2, 4, 4, 4);
    pixels[0] = 0xFF;
    let h2 = crc32c_tile(&pixels, 4 * 4, 0, 0, 2, 4, 4, 4);
    assert_ne!(h1, h2);
}
