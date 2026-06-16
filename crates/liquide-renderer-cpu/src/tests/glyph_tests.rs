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
    atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 12,
                bearing_x: 0,
                bearing_y: 10,
                advance: 8.0,
            },
        )
        .unwrap();

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
    let k1 = GlyphKey {
        font_id: 0,
        glyph_id: 1,
        size_px: 10,
        subpixel: false,
    };
    atlas
        .insert(
            k1,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 10,
                bearing_x: 0,
                bearing_y: 8,
                advance: 8.0,
            },
        )
        .unwrap();
    // Second glyph at (9, 0)
    let k2 = GlyphKey {
        font_id: 0,
        glyph_id: 2,
        size_px: 10,
        subpixel: false,
    };
    atlas
        .insert(
            k2,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 10,
                bearing_x: 0,
                bearing_y: 8,
                advance: 8.0,
            },
        )
        .unwrap();
    // Third glyph wraps to next row
    let k3 = GlyphKey {
        font_id: 0,
        glyph_id: 3,
        size_px: 10,
        subpixel: false,
    };
    atlas
        .insert(
            k3,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 10,
                bearing_x: 0,
                bearing_y: 8,
                advance: 8.0,
            },
        )
        .unwrap();
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
    let glyph = atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 4,
                height: 4,
                bearing_x: 0,
                bearing_y: 4,
                advance: 4.0,
            },
        )
        .unwrap()
        .clone();

    let lut = crate::color::SrgbLut::new();
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    atlas.blit_glyph(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::new(255, 0, 0, 255),
        None,
        &lut,
    );

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
    let glyph = atlas
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 4,
                height: 4,
                bearing_x: 0,
                bearing_y: 4,
                advance: 4.0,
            },
        )
        .unwrap();

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
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 2,
                height: 1,
                bearing_x: 0,
                bearing_y: 1,
                advance: 2.0,
            },
        )
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
        None,
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
        None,
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
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 1,
                height: 1,
                bearing_x: 0,
                bearing_y: 1,
                advance: 1.0,
            },
        )
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
        None,
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
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 1,
                height: 1,
                bearing_x: 0,
                bearing_y: 1,
                advance: 1.0,
            },
        )
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
        None,
    );

    let p = fb.get_pixel(10, 9);
    // Average alpha = (100+200+255+1)/3 = 185 (integer division)
    let expected_avg = ((100u16 + 200 + 255 + 1) / 3) as u8;
    assert_eq!(p.r, expected_avg, "None mode R: got {:?}", p);
    assert_eq!(p.g, expected_avg, "None mode G: got {:?}", p);
    assert_eq!(p.b, expected_avg, "None mode B: got {:?}", p);
}

#[test]
fn atlas_full_returns_error() {
    let mut atlas = GlyphAtlas::new(4, 4);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 1,
        size_px: 16,
        subpixel: false,
    };
    // Try to insert a glyph larger than the atlas (8x8 into 4x4)
    let bitmap = vec![255u8; 8 * 8];
    let result = atlas.insert(
        key,
        &bitmap,
        &GlyphMetrics {
            width: 8,
            height: 8,
            bearing_x: 0,
            bearing_y: 8,
            advance: 8.0,
        },
    );
    assert!(
        result.is_err(),
        "inserting a glyph larger than the atlas should fail"
    );
}

#[test]
fn subpixel_vrgb_mode() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 100,
        size_px: 16,
        subpixel: true,
    };
    // 2x2 subpixel bitmap (6 bytes per row, 2 rows)
    let bitmap = vec![200u8; 2 * 3 * 2];
    let glyph = atlas
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 2,
                height: 2,
                bearing_x: 0,
                bearing_y: 2,
                advance: 2.0,
            },
        )
        .unwrap()
        .clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::BLACK);

    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::Vrgb,
        None,
    );

    // Check that some pixels were modified
    let p = fb.get_pixel(10, 8);
    assert!(
        p.r > 0 || p.g > 0 || p.b > 0,
        "VRGB subpixel blit should produce non-zero pixels: got {:?}",
        p
    );
}

#[test]
fn subpixel_vbgr_mode() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 101,
        size_px: 16,
        subpixel: true,
    };
    // 2x2 subpixel bitmap (6 bytes per row, 2 rows)
    let bitmap = vec![180u8; 2 * 3 * 2];
    let glyph = atlas
        .insert_subpixel(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 2,
                height: 2,
                bearing_x: 0,
                bearing_y: 2,
                advance: 2.0,
            },
        )
        .unwrap()
        .clone();

    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::BLACK);

    atlas.blit_glyph_subpixel(
        &mut fb,
        &glyph,
        Point::new(10.0, 10.0),
        Color::WHITE,
        SubpixelMode::Vbgr,
        None,
    );

    // Check that some pixels were modified
    let p = fb.get_pixel(10, 8);
    assert!(
        p.r > 0 || p.g > 0 || p.b > 0,
        "VBGR subpixel blit should produce non-zero pixels: got {:?}",
        p
    );
}

#[test]
fn blit_glyph_clipping() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 102,
        size_px: 16,
        subpixel: false,
    };
    // 8x8 glyph
    let bitmap = vec![255u8; 8 * 8];
    let glyph = atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 8,
                bearing_x: 0,
                bearing_y: 4,
                advance: 8.0,
            },
        )
        .unwrap()
        .clone();

    let lut = crate::color::SrgbLut::new();
    let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    // Blit at the edge: glyph at (14, 14-4)=(14, 10), spans x=14..22, y=10..18
    // Extends beyond 16x16 FB — should clip without panic
    atlas.blit_glyph(
        &mut fb,
        &glyph,
        Point::new(14.0, 14.0),
        Color::new(255, 0, 0, 255),
        None,
        &lut,
    );

    // Also test negative position — should clip without panic
    atlas.blit_glyph(
        &mut fb,
        &glyph,
        Point::new(-5.0, -5.0),
        Color::new(0, 255, 0, 255),
        None,
        &lut,
    );
    // If we reach here without panicking, the test passes
}

#[test]
fn atlas_clear_resets() {
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
        subpixel: false,
    };
    let bitmap = vec![128u8; 8 * 12];
    atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 8,
                height: 12,
                bearing_x: 0,
                bearing_y: 10,
                advance: 8.0,
            },
        )
        .unwrap();
    assert_eq!(atlas.len(), 1);

    atlas.clear();
    assert_eq!(atlas.len(), 0);
    assert!(atlas.is_empty());
    // After clearing, pixels should be zeroed
    assert!(atlas.pixels().iter().all(|&b| b == 0));
}

// t87-crisp #1: subpixel positioning is real — a glyph drawn at a fractional pen
// X must land differently than at an integer pen X. Anti-fake-green: if blit
// floor-snaps (the old behavior), both phases produce IDENTICAL output and this
// test fails.
#[test]
fn subpixel_phase_changes_glyph_placement() {
    let lut = crate::color::SrgbLut::new();
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 65,
        size_px: 16,
        subpixel: false,
    };
    // A single-column-wide, fully opaque 1x1 glyph isolates horizontal phase.
    let bitmap = vec![255u8; 1 * 4];
    let glyph = atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 1,
                height: 4,
                bearing_x: 0,
                bearing_y: 0,
                advance: 1.0,
            },
        )
        .unwrap()
        .clone();

    let white = Color::new(255, 255, 255, 255);

    // Phase 0.0: full coverage lands entirely on column 10.
    let mut fb0 = FrameBuffer::new(32, 8, PixelFormat::Bgra8);
    atlas.blit_glyph(&mut fb0, &glyph, Point::new(10.0, 0.0), white, None, &lut);

    // Phase 0.5: coverage splits between columns 10 and 11.
    let mut fb_half = FrameBuffer::new(32, 8, PixelFormat::Bgra8);
    atlas.blit_glyph(
        &mut fb_half,
        &glyph,
        Point::new(10.5, 0.0),
        white,
        None,
        &lut,
    );

    // The two phases must differ: at phase 0.5, column 11 receives coverage that
    // it does not get at phase 0.0.
    assert_eq!(
        fb0.get_pixel(11, 0).r,
        0,
        "integer phase must not touch the next column"
    );
    assert!(
        fb_half.get_pixel(11, 0).r > 0,
        "fractional phase 0.5 must spill coverage into the next column \
         (subpixel positioning is dead if this is 0)"
    );
    // And column 10 must be dimmer at phase 0.5 than at phase 0.0 (coverage was
    // split off into column 11).
    assert!(
        fb_half.get_pixel(10, 0).r < fb0.get_pixel(10, 0).r,
        "phase 0.5 should reduce column-10 coverage vs phase 0.0"
    );
}

// t87-crisp #4c: grayscale glyph AA must composite in LINEAR light, not sRGB. A
// 50%-coverage white-on-black pixel blended in linear space is brighter than the
// naive sRGB midpoint (~188 vs 128). Anti-fake-green: if the blit reverts to
// sRGB-space coverage blending the value drops back toward ~128 and this fails.
#[test]
fn glyph_aa_is_gamma_correct_linear() {
    let lut = crate::color::SrgbLut::new();
    let mut atlas = GlyphAtlas::new(256, 256);
    let key = GlyphKey {
        font_id: 0,
        glyph_id: 66,
        size_px: 16,
        subpixel: false,
    };
    // Single pixel at exactly 50% coverage (alpha 128).
    let bitmap = vec![128u8; 1];
    let glyph = atlas
        .insert(
            key,
            &bitmap,
            &GlyphMetrics {
                width: 1,
                height: 1,
                bearing_x: 0,
                bearing_y: 0,
                advance: 1.0,
            },
        )
        .unwrap()
        .clone();

    // White text over a black framebuffer at integer phase.
    let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    atlas.blit_glyph(
        &mut fb,
        &glyph,
        Point::new(2.0, 0.0),
        Color::new(255, 255, 255, 255),
        None,
        &lut,
    );

    let px = fb.get_pixel(2, 0);

    // Reference: composite white over black at coverage = 128/255 in LINEAR
    // light, then convert back to sRGB.
    let a = 128.0 / 255.0;
    let fg_lin = lut.linearize(255); // 1.0
    let bg_lin = lut.linearize(0); // 0.0
    let expected = lut.delinearize(fg_lin * a + bg_lin * (1.0 - a));

    assert_eq!(
        px.r, expected,
        "glyph AA must match the linear-space reference ({} expected)",
        expected
    );
    // Sanity: the gamma-correct result is meaningfully brighter than the naive
    // sRGB midpoint — proves we are NOT blending in sRGB.
    assert!(
        px.r > 150,
        "linear-light 50% coverage should be ~188, got {} (sRGB-space regression?)",
        px.r
    );
}
