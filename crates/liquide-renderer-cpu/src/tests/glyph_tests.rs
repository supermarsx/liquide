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
        subpixel: false,
    };
    let bitmap = vec![128u8; 8 * 12]; // 8x12 glyph
    atlas.insert(key, &bitmap, 8, 12, 0, 10, 8.0).unwrap();

    assert_eq!(atlas.len(), 1);
    let g = atlas.get(&key).unwrap();
    assert_eq!(g.width, 8);
    assert_eq!(g.height, 12);
    assert!(!g.subpixel);
}

#[test]
fn atlas_row_wrap() {
    let mut atlas = GlyphAtlas::new(20, 100);
    let bitmap = vec![255u8; 8 * 10];
    // First glyph at (0, 0)
    let k1 = GlyphKey { font_id: 0, glyph_id: 1, size_px: 10, subpixel: false };
    atlas.insert(k1, &bitmap, 8, 10, 0, 8, 8.0).unwrap();
    // Second glyph at (9, 0)
    let k2 = GlyphKey { font_id: 0, glyph_id: 2, size_px: 10, subpixel: false };
    atlas.insert(k2, &bitmap, 8, 10, 0, 8, 8.0).unwrap();
    // Third glyph wraps to next row
    let k3 = GlyphKey { font_id: 0, glyph_id: 3, size_px: 10, subpixel: false };
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
        subpixel: false,
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

#[test]
fn subpixel_insert_and_lookup() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
        subpixel: true,
    };
    // 4 display pixels wide, 4 tall → 12 bytes per row (3 per pixel), 4 rows
    let bitmap = vec![200u8; 4 * 3 * 4];
    let glyph = atlas.insert_subpixel(key, &bitmap, 4, 4, 0, 4, 4.0).unwrap();

    assert_eq!(glyph.width, 4);
    assert_eq!(glyph.height, 4);
    assert!(glyph.subpixel);
    assert_eq!(atlas.len(), 1);
}

#[test]
fn subpixel_blit_rgb_per_channel() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
        subpixel: true,
    };

    // 2x1 subpixel bitmap: pixel 0 has (R=255, G=0, B=0), pixel 1 has (R=0, G=255, B=0)
    let bitmap = [
        255, 0, 0, // pixel 0: full R alpha, no G, no B
        0, 255, 0, // pixel 1: no R, full G alpha, no B
    ];
    let glyph = atlas
        .insert_subpixel(key, &bitmap, 2, 1, 0, 1, 2.0)
        .unwrap()
        .clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    // Fill background with white
    fb.clear(Color::WHITE);

    // Blit with white foreground color using RGB subpixel mode
    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::Rgb,
    );

    // Pixel 0 at (10, 9): R channel alpha=255, G=0, B=0
    // With white fg (255,255,255) on white bg (255,255,255):
    // R = 255*255/255 + 255*0/255 = 255
    // G = 255*0/255 + 255*255/255 = 255
    // B = 255*0/255 + 255*255/255 = 255
    // On white bg, all channels stay white regardless
    // Use a colored background to see the effect
    fb.clear(Color::BLACK);
    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::Rgb,
    );

    // Pixel 0 at (10, 9): R=255 alpha, G=0, B=0
    // R = 255*255/255 + 0*0/255 = 255
    // G = 255*0/255 + 0*255/255 = 0
    // B = 255*0/255 + 0*255/255 = 0
    let p0 = fb.get_pixel(10, 9);
    assert_eq!(p0.r, 255, "pixel 0 R should be full: got {:?}", p0);
    assert_eq!(p0.g, 0, "pixel 0 G should be 0: got {:?}", p0);
    assert_eq!(p0.b, 0, "pixel 0 B should be 0: got {:?}", p0);

    // Pixel 1 at (11, 9): R=0, G=255 alpha, B=0
    let p1 = fb.get_pixel(11, 9);
    assert_eq!(p1.r, 0, "pixel 1 R should be 0: got {:?}", p1);
    assert_eq!(p1.g, 255, "pixel 1 G should be full: got {:?}", p1);
    assert_eq!(p1.b, 0, "pixel 1 B should be 0: got {:?}", p1);
}

#[test]
fn subpixel_blit_bgr_swaps_channels() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 66,
        size_px: 16,
        subpixel: true,
    };

    // 1x1 subpixel bitmap: (a0=255, a1=0, a2=128)
    let bitmap = [255, 0, 128];
    let glyph = atlas
        .insert_subpixel(key, &bitmap, 1, 1, 0, 1, 1.0)
        .unwrap()
        .clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::BLACK);

    // BGR mode: a0→B channel, a1→G channel, a2→R channel
    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::Bgr,
    );

    let p = fb.get_pixel(10, 9);
    // BGR: R alpha = a2 = 128, G alpha = a1 = 0, B alpha = a0 = 255
    assert_eq!(p.r, 128, "BGR R should map from a2=128: got {:?}", p);
    assert_eq!(p.g, 0, "BGR G should map from a1=0: got {:?}", p);
    assert_eq!(p.b, 255, "BGR B should map from a0=255: got {:?}", p);
}

#[test]
fn subpixel_mode_none_averages_channels() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 67,
        size_px: 16,
        subpixel: true,
    };

    // 1x1 subpixel bitmap: (100, 200, 255) → average = (100+200+255+1)/3 = 185
    let bitmap = [100, 200, 255];
    let glyph = atlas
        .insert_subpixel(key, &bitmap, 1, 1, 0, 1, 1.0)
        .unwrap()
        .clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::BLACK);

    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::None,
    );

    let p = fb.get_pixel(10, 9);
    // Average alpha = (100+200+255+1)/3 = 185 (integer division)
    let expected_avg = ((100u16 + 200 + 255 + 1) / 3) as u8;
    assert_eq!(p.r, expected_avg, "None mode R: got {:?}", p);
    assert_eq!(p.g, expected_avg, "None mode G: got {:?}", p);
    assert_eq!(p.b, expected_avg, "None mode B: got {:?}", p);
}
