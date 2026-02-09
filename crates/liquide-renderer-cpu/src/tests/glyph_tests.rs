use crate::glyph::*;
use liquide_compositor::pixel::PixelFormat;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Point;
use liquide_compositor::pixel::Color;

#[test]
fn atlas_insert_and_lookup() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
    };
    let bitmap = vec![128u8; 8 * 12]; // 8x12 glyph
    atlas.insert(key, &bitmap, 8, 12, 0, 10, 8.0).unwrap();

    assert_eq!(atlas.len(), 1);
    let g = atlas.get(&key).unwrap();
    assert_eq!(g.width, 8);
    assert_eq!(g.height, 12);
}

#[test]
fn atlas_row_wrap() {
    let mut atlas = GlyphAtlas::new(20, 100);
    let bitmap = vec![255u8; 8 * 10];
    // First glyph at (0, 0)
    let k1 = GlyphKey { font_id: 0, glyph_id: 1, size_px: 10 };
    atlas.insert(k1, &bitmap, 8, 10, 0, 8, 8.0).unwrap();
    // Second glyph at (9, 0)
    let k2 = GlyphKey { font_id: 0, glyph_id: 2, size_px: 10 };
    atlas.insert(k2, &bitmap, 8, 10, 0, 8, 8.0).unwrap();
    // Third glyph wraps to next row
    let k3 = GlyphKey { font_id: 0, glyph_id: 3, size_px: 10 };
    atlas.insert(k3, &bitmap, 8, 10, 0, 8, 8.0).unwrap();
    let g3 = atlas.get(&k3).unwrap();
    assert_eq!(g3.atlas_y, 11); // wrapped to row below (10 + 1 padding)
}

#[test]
fn atlas_blit() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
    };
    let bitmap = vec![255u8; 4 * 4]; // 4x4 fully opaque
    let glyph = atlas.insert(key, &bitmap, 4, 4, 0, 4, 4.0).unwrap().clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    atlas.blit_glyph(&mut fb, &glyph, Point::new(10.0, 10.0), Color::new(255, 0, 0, 255));

    // Glyph renders at (10 + 0, 10 - 4) = (10, 6) with 4x4 size
    let c = fb.get_pixel(10, 6);
    assert_eq!(c.r, 255);
    assert_eq!(c.a, 255);
}
