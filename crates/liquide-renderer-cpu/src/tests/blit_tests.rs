use crate::blit::*;
use liquide_compositor::pixel::PixelFormat;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

#[test]
fn clear_region_sets_color() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    clear_region(&mut fb, Rect::new(10.0, 10.0, 20.0, 20.0), Color::WHITE);
    // Inside region
    assert_eq!(fb.get_pixel(15, 15), Color::WHITE);
    // Outside region — still black/transparent
    assert_eq!(fb.get_pixel(5, 5).r, 0);
}

#[test]
fn blit_region_opaque() {
    let mut dst = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    let mut src = FrameBuffer::new(10, 10, PixelFormat::Bgra8);
    src.clear(Color::new(255, 0, 0, 255));

    blit_region(
        &mut dst,
        &src,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        5,
        5,
        BlendMode::SrcOver,
        1.0,
    );

    let c = dst.get_pixel(7, 7);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
}

#[test]
fn blit_region_src_copies_rows_directly() {
    let mut dst = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let mut src = FrameBuffer::new(6, 4, PixelFormat::Bgra8);
    src.clear(Color::new(12, 34, 56, 255));

    blit_region(
        &mut dst,
        &src,
        Rect::new(0.0, 0.0, 6.0, 4.0),
        3,
        5,
        BlendMode::Src,
        1.0,
    );

    assert_eq!(dst.get_pixel(3, 5), Color::new(12, 34, 56, 255));
    assert_eq!(dst.get_pixel(8, 8), Color::new(12, 34, 56, 255));
}

// --- blit_within (t164-blit-move) self-blit overlap correctness ----------

/// Paint a unique, position-dependent color at every pixel so a translated copy
/// can be checked for byte-exactness AND for smear (a wrong overlap order leaves
/// a streak of a single source row repeated across the destination).
fn paint_unique(fb: &mut FrameBuffer) {
    for y in 0..fb.height {
        for x in 0..fb.width {
            // Encode (x, y) into the channels so each pixel is distinguishable.
            fb.set_pixel(
                x,
                y,
                Color::new((x & 0xff) as u8, (y & 0xff) as u8, ((x + y) & 0xff) as u8, 255),
            );
        }
    }
}

/// A pristine reference copy of the whole framebuffer, used as the "old" source
/// to prove `blit_within` is byte-identical to a manual translate.
fn snapshot(fb: &FrameBuffer) -> Vec<Color> {
    let mut out = Vec::with_capacity((fb.width * fb.height) as usize);
    for y in 0..fb.height {
        for x in 0..fb.width {
            out.push(fb.get_pixel(x, y));
        }
    }
    out
}

fn at(snap: &[Color], w: u32, x: u32, y: u32) -> Color {
    snap[(y * w + x) as usize]
}

/// Run `blit_within` for a move of `(dx, dy)` over an overlapping region and
/// assert EVERY destination pixel equals the original source pixel it came from
/// (no smear, no ghost). The region overlaps the destination so the overlap
/// direction logic is exercised.
fn assert_self_blit_translates(dx: i32, dy: i32) {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    paint_unique(&mut fb);
    let before = snapshot(&fb);

    // Source rect chosen so source and destination overlap (the hard case).
    let src = Rect::new(8.0, 8.0, 12.0, 12.0);
    let (sx, sy, sw, sh) = (8i32, 8i32, 12i32, 12i32);
    let dst_x = sx + dx;
    let dst_y = sy + dy;

    blit_within(&mut fb, src, dst_x, dst_y);

    // Every destination pixel must equal the ORIGINAL value at the source pixel
    // it was copied from — proving overlap-safe ordering (a wrong order would
    // copy an already-overwritten value, smearing one row across the rect).
    for row in 0..sh {
        for col in 0..sw {
            let d = fb.get_pixel((dst_x + col) as u32, (dst_y + row) as u32);
            let expected = at(&before, 32, (sx + col) as u32, (sy + row) as u32);
            assert_eq!(
                d, expected,
                "self-blit dx={dx} dy={dy}: dst ({},{}) should equal src ({},{})",
                dst_x + col,
                dst_y + row,
                sx + col,
                sy + row
            );
        }
    }
}

#[test]
fn blit_within_moves_down_without_smear() {
    assert_self_blit_translates(0, 4);
}

#[test]
fn blit_within_moves_up_without_smear() {
    assert_self_blit_translates(0, -4);
}

#[test]
fn blit_within_moves_right_without_smear() {
    assert_self_blit_translates(4, 0);
}

#[test]
fn blit_within_moves_left_without_smear() {
    assert_self_blit_translates(-4, 0);
}

#[test]
fn blit_within_moves_diagonally_without_smear() {
    assert_self_blit_translates(3, 5);
    assert_self_blit_translates(-3, -5);
    assert_self_blit_translates(5, -3);
    assert_self_blit_translates(-5, 3);
}

#[test]
fn blit_within_noop_for_zero_move() {
    let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    paint_unique(&mut fb);
    let before = snapshot(&fb);
    blit_within(&mut fb, Rect::new(2.0, 2.0, 8.0, 8.0), 2, 2);
    assert_eq!(snapshot(&fb), before, "zero-offset self-blit must be a no-op");
}

#[test]
fn blit_within_clamps_out_of_bounds_destination() {
    // Destination partly off the right/bottom edge — must clamp, not panic, and
    // must not corrupt pixels outside the copy.
    let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    paint_unique(&mut fb);
    let before = snapshot(&fb);
    // Move a near-edge rect further toward the edge so part falls off-screen.
    blit_within(&mut fb, Rect::new(4.0, 4.0, 8.0, 8.0), 12, 12);
    // The in-bounds copied pixels equal their source; the test only asserts no
    // panic + a representative copied pixel is correct.
    assert_eq!(
        fb.get_pixel(12, 12),
        at(&before, 16, 4, 4),
        "clamped self-blit still copies the in-bounds top-left correctly"
    );
}
