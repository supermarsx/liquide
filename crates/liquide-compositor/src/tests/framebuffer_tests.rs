use crate::framebuffer::*;
use crate::pixel::{Color, PixelFormat};

#[test]
fn framebuffer_basic() {
    let fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    assert_eq!(fb.byte_len(), 64 * 64 * 4);
    assert_eq!(fb.stride, 64 * 4);
}

#[test]
fn framebuffer_pixel_roundtrip() {
    let mut fb = FrameBuffer::new(10, 10, PixelFormat::Bgra8);
    let c = Color::new(100, 150, 200, 255);
    fb.set_pixel(3, 5, c);
    assert_eq!(fb.get_pixel(3, 5), c);
}

#[test]
fn framebuffer_clear() {
    let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
    fb.clear(Color::new(255, 0, 0, 255));
    let c = fb.get_pixel(2, 2);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn framebuffer_tile_region() {
    let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
    // Set pixel at (64, 64) which is tile (1, 1) in a 64-wide grid
    fb.set_pixel(64, 64, Color::WHITE);
    let tile = fb.tile_region(1, 1, 64);
    // First pixel of this tile should be white
    assert_eq!(tile[0], 255); // B
    assert_eq!(tile[1], 255); // G
    assert_eq!(tile[2], 255); // R
    assert_eq!(tile[3], 255); // A
}

#[test]
fn framebuffer_tile_grid_dims() {
    let fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    assert_eq!(fb.tile_grid_width(64), 30);
    assert_eq!(fb.tile_grid_height(64), 17); // ceil(1080/64)
}

#[test]
fn double_buffer_swap() {
    let mut db = DoubleBuffer::new(4, 4, PixelFormat::Bgra8);
    db.back_mut().set_pixel(0, 0, Color::WHITE);
    assert_eq!(db.front().get_pixel(0, 0), Color::TRANSPARENT);
    db.swap();
    assert_eq!(db.front().get_pixel(0, 0), Color::WHITE);
}

#[test]
fn framebuffer_with_stride() {
    // stride > width * bpp means padding exists
    let fb = FrameBuffer::with_stride(100, 100, 128 * 4, PixelFormat::Bgra8);
    assert_eq!(fb.stride, 128 * 4);
    assert_eq!(fb.width, 100);
    assert_eq!(fb.pixels().len(), (128 * 4 * 100) as usize);
}

#[test]
fn framebuffer_pixel_offset() {
    let fb = FrameBuffer::new(100, 100, PixelFormat::Bgra8);
    assert_eq!(fb.pixel_offset(0, 0), 0);
    assert_eq!(fb.pixel_offset(1, 0), 4);
    assert_eq!(fb.pixel_offset(0, 1), 100 * 4);
}

#[test]
fn framebuffer_row_and_row_mut() {
    let mut fb = FrameBuffer::new(10, 10, PixelFormat::Bgra8);
    // Write to row 3 via row_mut
    let row = fb
        .row_mut(3)
        .expect("row_mut: CPU framebuffer expected in test");
    row[0] = 0xFF;
    // Read back via row
    assert_eq!(fb.row(3)[0], 0xFF);
    assert_eq!(fb.row(3).len(), 40); // 10 pixels * 4 bytes
}

#[test]
fn double_buffer_back() {
    let db = DoubleBuffer::new(100, 100, PixelFormat::Bgra8);
    assert_eq!(db.back().width, 100);
    assert_eq!(db.front().width, 100);
}

#[test]
fn framebuffer_byte_len() {
    let fb = FrameBuffer::new(200, 150, PixelFormat::Bgra8);
    assert_eq!(fb.byte_len(), (200 * 150 * 4) as usize);
}

// ── t93-e6: cheap window-thumbnail capture (gap #1) ───────────────────────────

use crate::geometry::Rect;

/// `capture_region` returns the EXACT sub-rect pixels of a known framebuffer,
/// tightly packed (stride == width*bpp) and dimensioned to the rect. This is
/// the core correctness contract for overview thumbnails: the captured buffer
/// must be the real composited content under the window's screen rect, not a
/// placeholder.
#[test]
fn capture_region_returns_correct_subrect_pixels() {
    let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    // Paint a unique color at each pixel so any mis-offset is detectable.
    for y in 0..8u32 {
        for x in 0..8u32 {
            fb.set_pixel(x, y, Color::new(x as u8 * 10, y as u8 * 10, 7, 255));
        }
    }

    // Capture a 3x2 region at (2,3).
    let cap = fb.capture_region(Rect::new(2.0, 3.0, 3.0, 2.0));
    assert_eq!(cap.width, 3);
    assert_eq!(cap.height, 2);
    assert_eq!(cap.stride, 3 * 4, "tightly packed, no padding");
    assert_eq!(cap.pixels.len(), (3 * 2 * 4) as usize);

    // Verify every captured texel equals the source framebuffer texel.
    for ry in 0..2u32 {
        for rx in 0..3u32 {
            let src = fb.get_pixel(2 + rx, 3 + ry);
            let i = (ry * cap.stride + rx * 4) as usize;
            // Bgra8 byte order in the buffer.
            let got = Color::from_bgra_bytes([
                cap.pixels[i],
                cap.pixels[i + 1],
                cap.pixels[i + 2],
                cap.pixels[i + 3],
            ]);
            assert_eq!(got, src, "captured pixel ({rx},{ry}) must match source");
        }
    }
}

/// An out-of-bounds / partially off-screen request is CLAMPED to the
/// framebuffer (it never reads outside it and never panics), yielding only the
/// on-screen pixels. This is the off-screen-window fallback path.
#[test]
fn capture_region_clamps_out_of_bounds() {
    let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
    for y in 0..4u32 {
        for x in 0..4u32 {
            fb.set_pixel(x, y, Color::new(x as u8, y as u8, 0, 255));
        }
    }

    // Rect straddles the right/bottom edge: (2,2) size 10x10 → clamps to 2x2.
    let cap = fb.capture_region(Rect::new(2.0, 2.0, 10.0, 10.0));
    assert_eq!(cap.width, 2);
    assert_eq!(cap.height, 2);
    // Top-left of the clamped capture == source (2,2).
    let i = 0usize;
    let got = Color::from_bgra_bytes([
        cap.pixels[i],
        cap.pixels[i + 1],
        cap.pixels[i + 2],
        cap.pixels[i + 3],
    ]);
    assert_eq!(got, fb.get_pixel(2, 2));

    // A fully off-screen rect yields a 1x1 transparent (never empty) buffer.
    let off = fb.capture_region(Rect::new(100.0, 100.0, 10.0, 10.0));
    assert_eq!((off.width, off.height), (1, 1));
    assert!(!off.pixels.is_empty());

    // A zero-area rect also yields the 1x1 fallback.
    let zero = fb.capture_region(Rect::new(1.0, 1.0, 0.0, 0.0));
    assert_eq!((zero.width, zero.height), (1, 1));
}

/// `SurfaceBuffer::scaled_to` is a deterministic bilinear resize: a 1:1 request
/// is an exact copy, and a scaled request produces a buffer of the requested
/// dimensions whose corner samples track the source corners (so a real window
/// capture shrinks into a tile without garbage).
#[test]
fn surface_buffer_scaled_to_is_deterministic_bilinear() {
    let fb_src = {
        let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
        // Left half red, right half blue.
        for y in 0..4u32 {
            for x in 0..4u32 {
                let c = if x < 2 {
                    Color::new(200, 0, 0, 255)
                } else {
                    Color::new(0, 0, 200, 255)
                };
                fb.set_pixel(x, y, c);
            }
        }
        fb
    };
    let src = fb_src.capture_region(Rect::new(0.0, 0.0, 4.0, 4.0));

    // 1:1 is an exact tight copy.
    let same = src.scaled_to(4, 4);
    assert_eq!((same.width, same.height), (4, 4));
    assert_eq!(same.pixels.as_slice(), src.pixels.as_slice());

    // Scale to 2x2: dimensions exact, tightly packed, deterministic across runs.
    let small = src.scaled_to(2, 2);
    assert_eq!((small.width, small.height), (2, 2));
    assert_eq!(small.stride, 2 * 4);
    let small2 = src.scaled_to(2, 2);
    assert_eq!(
        small.pixels.as_slice(),
        small2.pixels.as_slice(),
        "scaling must be deterministic"
    );

    // Left column should still read reddish, right column bluish (no swap).
    let px = |b: &crate::scene::SurfaceBuffer, x: u32, y: u32| -> Color {
        let i = (y * b.stride + x * 4) as usize;
        Color::from_bgra_bytes([b.pixels[i], b.pixels[i + 1], b.pixels[i + 2], b.pixels[i + 3]])
    };
    let left = px(&small, 0, 0);
    let right = px(&small, 1, 0);
    assert!(left.r > left.b, "left tile sample stays red-dominant");
    assert!(right.b > right.r, "right tile sample stays blue-dominant");
}
