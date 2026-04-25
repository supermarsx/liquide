use liquide_compositor::effects::{EffectParams, QualityProfile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};

use crate::effects::*;

#[test]
fn backdrop_blur_modifies_pixels() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    // Create a stripe pattern
    for y in 0..64 {
        let c = if y % 2 == 0 {
            Color::WHITE
        } else {
            Color::BLACK
        };
        for x in 0..64 {
            fb.set_pixel(x, y, c);
        }
    }
    let before = fb.pixels().to_vec();

    let params = EffectParams::for_profile(QualityProfile::Quality);
    let effect = BackdropBlur;
    effect.render(&mut fb, Rect::new(0.0, 0.0, 64.0, 64.0), &params);

    assert_ne!(
        fb.pixels(),
        &before[..],
        "backdrop blur should modify pixels"
    );
}

#[test]
fn backdrop_blur_zero_radius_only_tints() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    for y in 0..32 {
        for x in 0..32 {
            fb.set_pixel(x, y, Color::new(100, 100, 100, 255));
        }
    }
    let before = fb.pixels().to_vec();

    let mut params = EffectParams::for_profile(QualityProfile::Minimal);
    params.blur_radius = 0;
    let effect = BackdropBlur;
    effect.render(&mut fb, Rect::new(0.0, 0.0, 32.0, 32.0), &params);

    // The tint should have been applied, so pixels should change slightly
    // (default tint is white@40 alpha)
    assert_ne!(
        fb.pixels(),
        &before[..],
        "tint should modify pixels even with radius=0"
    );
}

#[test]
fn box_shadow_extends_beyond_surface() {
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb.clear(Color::WHITE);

    // Render a shadow for a rect in the center
    let surface = Rect::new(30.0, 30.0, 68.0, 68.0);
    BoxShadow::render_shadow(
        &mut fb,
        &ShadowParams {
            surface_rect: surface,
            corner_radius: 0.0,
            spread: 8.0,
            blur_radius: 8,
            offset_x: 0.0,
            offset_y: 0.0,
            shadow_color: Color::new(0, 0, 0, 200),
        },
    );

    // Check that shadow creates non-white pixels outside the surface bounds.
    // The shadow shape is expanded by spread=8, so its left edge starts at x=22.
    // After blur (radius=8), coverage bleeds further. Check a pixel just
    // outside the expanded shape at x=18 which is within blur bleed range.
    let mut found_shadow = false;
    for y in 30..98 {
        let p = fb.get_pixel(18, y);
        if p.r < 255 || p.g < 255 || p.b < 255 {
            found_shadow = true;
            break;
        }
    }
    assert!(
        found_shadow,
        "shadow should extend beyond the surface bounds"
    );
}

#[test]
fn box_shadow_no_effect_when_disabled() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb.clear(Color::WHITE);
    let before = fb.pixels().to_vec();

    // Zero radius and zero spread should be a no-op
    BoxShadow::render_shadow(
        &mut fb,
        &ShadowParams {
            surface_rect: Rect::new(16.0, 16.0, 32.0, 32.0),
            corner_radius: 0.0,
            spread: 0.0,
            blur_radius: 0,
            offset_x: 0.0,
            offset_y: 0.0,
            shadow_color: Color::BLACK,
        },
    );

    assert_eq!(fb.pixels(), &before[..]);
}

#[test]
fn inner_glow_only_affects_edges() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    // Fill with a mid-grey
    let grey = Color::new(128, 128, 128, 255);
    for y in 0..64 {
        for x in 0..64 {
            fb.set_pixel(x, y, grey);
        }
    }

    let region = Rect::new(8.0, 8.0, 48.0, 48.0);
    let glow_width = 3.0;

    InnerGlow::render_glow(
        &mut fb,
        region,
        4.0,
        glow_width,
        Color::new(255, 255, 255, 100),
    );

    // Center of the region should be unchanged
    let center = fb.get_pixel(32, 32);
    assert_eq!(center, grey, "center should be unaffected by inner glow");

    // Edge pixel should be brighter due to screen blend
    let edge = fb.get_pixel(9, 32);
    assert!(
        edge.r > grey.r || edge.g > grey.g || edge.b > grey.b,
        "edge should be lightened by inner glow: got {:?}",
        edge
    );
}

#[test]
fn inner_glow_zero_width_noop() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::new(100, 100, 100, 255));
    let before = fb.pixels().to_vec();

    InnerGlow::render_glow(
        &mut fb,
        Rect::new(0.0, 0.0, 32.0, 32.0),
        4.0,
        0.0,
        Color::WHITE,
    );

    assert_eq!(fb.pixels(), &before[..]);
}

#[test]
fn box_shadow_produces_pixels() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb.clear(Color::WHITE);
    let before = fb.pixels().to_vec();

    BoxShadow::render_shadow(
        &mut fb,
        &ShadowParams {
            surface_rect: Rect::new(16.0, 16.0, 32.0, 32.0),
            corner_radius: 4.0,
            spread: 4.0,
            blur_radius: 4,
            offset_x: 0.0,
            offset_y: 0.0,
            shadow_color: Color::new(0, 0, 0, 160),
        },
    );

    assert_ne!(
        fb.pixels(),
        &before[..],
        "box shadow should produce visible pixels"
    );
}

#[test]
fn inner_glow_width_affects_coverage() {
    let grey = Color::new(128, 128, 128, 255);

    // Width 0 should be a no-op
    let mut fb_zero = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb_zero.clear(grey);
    let before_zero = fb_zero.pixels().to_vec();

    InnerGlow::render_glow(
        &mut fb_zero,
        Rect::new(8.0, 8.0, 48.0, 48.0),
        4.0,
        0.0,
        Color::new(255, 255, 255, 100),
    );
    let changed_zero: usize = fb_zero
        .pixels()
        .iter()
        .zip(before_zero.iter())
        .filter(|&(&a, &b)| a != b)
        .count();

    // Width 5 should affect more pixels
    let mut fb_five = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    fb_five.clear(grey);
    let before_five = fb_five.pixels().to_vec();

    InnerGlow::render_glow(
        &mut fb_five,
        Rect::new(8.0, 8.0, 48.0, 48.0),
        4.0,
        5.0,
        Color::new(255, 255, 255, 100),
    );
    let changed_five: usize = fb_five
        .pixels()
        .iter()
        .zip(before_five.iter())
        .filter(|&(&a, &b)| a != b)
        .count();

    assert_eq!(changed_zero, 0, "width=0 should change no pixels");
    assert!(changed_five > 0, "width=5 should change some pixels");
    assert!(
        changed_five > changed_zero,
        "width=5 should affect more pixels than width=0"
    );
}

#[test]
fn backdrop_blur_large_radius() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    // Create a striped pattern
    for y in 0..64 {
        let c = if y % 4 == 0 {
            Color::WHITE
        } else {
            Color::BLACK
        };
        for x in 0..64 {
            fb.set_pixel(x, y, c);
        }
    }
    let before = fb.pixels().to_vec();

    let mut params = EffectParams::for_profile(QualityProfile::Quality);
    params.blur_radius = 20;
    BackdropBlur::render_with_tint(
        &mut fb,
        Rect::new(0.0, 0.0, 64.0, 64.0),
        &params,
        Color::TRANSPARENT,
    );

    assert_ne!(
        fb.pixels(),
        &before[..],
        "backdrop blur with large radius should modify pixels"
    );
}

#[test]
fn backdrop_blur_with_colored_tint() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    fb.clear(Color::new(100, 100, 100, 255));
    let before = fb.pixels().to_vec();

    let params = EffectParams::for_profile(QualityProfile::Balanced);
    let tint = Color::new(255, 0, 0, 80); // red tint with alpha
    BackdropBlur::render_with_tint(&mut fb, Rect::new(0.0, 0.0, 32.0, 32.0), &params, tint);

    assert_ne!(
        fb.pixels(),
        &before[..],
        "backdrop blur with colored tint should modify pixels"
    );
    // The tint should shift the red channel up
    let p = fb.get_pixel(16, 16);
    assert!(
        p.r > 100,
        "red tint should increase the red channel: got {:?}",
        p
    );
}

#[test]
fn box_shadow_offset() {
    let mut fb_no_offset = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb_no_offset.clear(Color::WHITE);
    BoxShadow::render_shadow(
        &mut fb_no_offset,
        &ShadowParams {
            surface_rect: Rect::new(40.0, 40.0, 40.0, 40.0),
            corner_radius: 0.0,
            spread: 4.0,
            blur_radius: 4,
            offset_x: 0.0,
            offset_y: 0.0,
            shadow_color: Color::new(0, 0, 0, 200),
        },
    );

    let mut fb_offset = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    fb_offset.clear(Color::WHITE);
    BoxShadow::render_shadow(
        &mut fb_offset,
        &ShadowParams {
            surface_rect: Rect::new(40.0, 40.0, 40.0, 40.0),
            corner_radius: 0.0,
            spread: 4.0,
            blur_radius: 4,
            offset_x: 20.0,
            offset_y: 20.0,
            shadow_color: Color::new(0, 0, 0, 200),
        },
    );

    // The two renders should produce different framebuffers
    assert_ne!(
        fb_no_offset.pixels(),
        fb_offset.pixels(),
        "offset shadow should differ from non-offset shadow"
    );

    // Pixel (90, 90): inside offset shadow region but outside no-offset shadow region
    let p_no_offset = fb_no_offset.get_pixel(90, 90);
    let p_offset = fb_offset.get_pixel(90, 90);
    assert_eq!(
        p_no_offset,
        Color::WHITE,
        "no-offset shadow should not reach (90, 90)"
    );
    assert_ne!(
        p_offset,
        Color::WHITE,
        "offset shadow should affect (90, 90): got {:?}",
        p_offset
    );
}
