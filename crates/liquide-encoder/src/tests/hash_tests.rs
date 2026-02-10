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
    let r1 = TileRegion { tile_x: 0, tile_y: 0, tile_size: 2, fb_width: 4, fb_height: 4, bpp: 4 };
    let r2 = TileRegion { tile_x: 1, tile_y: 1, tile_size: 2, fb_width: 4, fb_height: 4, bpp: 4 };
    let h1 = crc32c_tile(&pixels, 4 * 4, &r1);
    let h2 = crc32c_tile(&pixels, 4 * 4, &r2);
    // Same data → same hash
    assert_eq!(h1, h2);
}

#[test]
fn crc32c_detects_change() {
    let mut pixels = vec![0u8; 4 * 4 * 4];
    let region = TileRegion { tile_x: 0, tile_y: 0, tile_size: 2, fb_width: 4, fb_height: 4, bpp: 4 };
    let h1 = crc32c_tile(&pixels, 4 * 4, &region);
    pixels[0] = 0xFF;
    let h2 = crc32c_tile(&pixels, 4 * 4, &region);
    assert_ne!(h1, h2);
}

#[test]
fn crc32c_large_data() {
    // 1 KB buffer filled with a repeating pattern
    let data: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
    let h1 = crc32c(&data);
    let h2 = crc32c(&data);
    assert_ne!(h1, 0, "CRC of a non-trivial 1KB buffer should be non-zero");
    assert_eq!(h1, h2, "CRC must be deterministic");
}

#[test]
fn crc32c_single_byte() {
    let h = crc32c(&[0x42]);
    assert_ne!(h, 0, "CRC of a single byte should be non-zero");
    // Verify determinism
    assert_eq!(h, crc32c(&[0x42]));
}

#[test]
fn crc32c_tile_region_edge() {
    // 5x5 pixel buffer with tile_size=4 — the bottom-right tile is partial (1x1)
    let fb_w = 5u32;
    let fb_h = 5u32;
    let bpp = 4u32;
    let stride = fb_w * bpp;
    let pixels: Vec<u8> = (0..(fb_w * fb_h * bpp) as usize)
        .map(|i| (i % 200) as u8)
        .collect();

    let region = TileRegion {
        tile_x: 1,
        tile_y: 1,
        tile_size: 4,
        fb_width: fb_w,
        fb_height: fb_h,
        bpp,
    };
    let h = crc32c_tile(&pixels, stride, &region);
    // The result should be deterministic and non-zero (the partial tile has real data)
    assert_ne!(h, 0);
    assert_eq!(h, crc32c_tile(&pixels, stride, &region));
}
