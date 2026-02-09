use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};

use crate::blur::*;

#[test]
fn kernel_radius_zero_is_identity() {
    let k = GaussianKernel::new(0);
    assert_eq!(k.half_width, 0);
    assert_eq!(k.weights.len(), 1);
    assert!((k.weights[0] - 1.0).abs() < 1e-6);
}

#[test]
fn kernel_weights_sum_to_one() {
    for radius in [1, 3, 5, 10, 20] {
        let k = GaussianKernel::new(radius);
        let sum: f32 = k.weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 0.001,
            "radius {radius}: sum = {sum}"
        );
    }
}

#[test]
fn kernel_is_symmetric() {
    let k = GaussianKernel::new(10);
    let n = k.weights.len();
    for i in 0..n / 2 {
        let diff = (k.weights[i] - k.weights[n - 1 - i]).abs();
        assert!(diff < 1e-6, "asymmetry at index {i}: {diff}");
    }
}

#[test]
fn blur_identity_radius_zero() {
    let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
    // Fill with a known pattern
    for y in 0..8 {
        for x in 0..8 {
            let c = Color::new((x * 30) as u8, (y * 30) as u8, 128, 255);
            fb.set_pixel(x, y, c);
        }
    }
    let before: Vec<u8> = fb.pixels.clone();
    blur_region(&mut fb, Rect::new(0.0, 0.0, 8.0, 8.0), 0);
    assert_eq!(fb.pixels, before);
}

#[test]
fn blur_region_changes_pixels() {
    let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
    // Set a bright spot in the center
    fb.set_pixel(16, 16, Color::WHITE);
    let before = fb.pixels.clone();

    blur_region(&mut fb, Rect::new(0.0, 0.0, 32.0, 32.0), 5);

    // The center pixel should have changed (spread out)
    assert_ne!(fb.pixels, before);

    // The bright spot should have been diffused, so the center pixel
    // should be dimmer than pure white
    let center = fb.get_pixel(16, 16);
    assert!(center.r < 255 || center.g < 255 || center.b < 255);

    // Neighbouring pixels should have picked up some light
    let neighbor = fb.get_pixel(15, 16);
    assert!(neighbor.r > 0 || neighbor.g > 0 || neighbor.b > 0);
}

#[test]
fn blur_uniform_region_stays_uniform() {
    let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
    let fill = Color::new(100, 150, 200, 255);
    for y in 0..16 {
        for x in 0..16 {
            fb.set_pixel(x, y, fill);
        }
    }

    blur_region(&mut fb, Rect::new(2.0, 2.0, 12.0, 12.0), 3);

    // Interior pixels (away from edge) should still be approximately uniform
    let mid = fb.get_pixel(8, 8);
    let diff_r = (mid.r as i32 - fill.r as i32).unsigned_abs();
    let diff_g = (mid.g as i32 - fill.g as i32).unsigned_abs();
    let diff_b = (mid.b as i32 - fill.b as i32).unsigned_abs();
    assert!(diff_r <= 1 && diff_g <= 1 && diff_b <= 1,
        "uniform blur deviated: got {:?}, expected ~{:?}", mid, fill);
}

#[test]
fn downsample_upsample_roundtrip() {
    // Create a uniform 8x8 buffer
    let w = 8u32;
    let h = 8u32;
    let color = [80u8, 120, 200, 255]; // BGRA
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&color);
    }

    let (small, dw, dh) = blur_downsample_2x(&buf, w, h);
    assert_eq!(dw, 4);
    assert_eq!(dh, 4);

    let restored = blur_upsample_2x_bilinear(&small, dw, dh, w, h);
    assert_eq!(restored.len(), buf.len());

    // For a uniform image, roundtrip should be near-exact
    for (i, (&orig, &res)) in buf.iter().zip(restored.iter()).enumerate() {
        let diff = (orig as i32 - res as i32).unsigned_abs();
        assert!(diff <= 1, "roundtrip diff at byte {i}: {orig} vs {res}");
    }
}

#[test]
fn blur_fast_produces_result() {
    let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
    // Create a pattern with a bright center
    for y in 28..36 {
        for x in 28..36 {
            fb.set_pixel(x, y, Color::WHITE);
        }
    }
    let before = fb.pixels.clone();

    blur_fast(&mut fb, Rect::new(0.0, 0.0, 64.0, 64.0), 12);

    assert_ne!(fb.pixels, before, "blur_fast should modify pixels");

    // The bright area should have been diffused
    let center = fb.get_pixel(32, 32);
    assert!(center.r > 0, "center should still have some brightness");
}

#[test]
fn blur_buffer_standalone() {
    let w = 16u32;
    let h = 16u32;
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // Single bright pixel in center
    let off = (8 * w as usize + 8) * 4;
    buf[off] = 255;
    buf[off + 1] = 255;
    buf[off + 2] = 255;
    buf[off + 3] = 255;

    let before = buf.clone();
    blur_buffer(&mut buf, w, h, 3);

    assert_ne!(buf, before, "blur_buffer should modify the data");
    // Center should be dimmer
    assert!(buf[off + 2] < 255);
}
