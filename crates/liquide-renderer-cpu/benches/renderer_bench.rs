use criterion::{black_box, criterion_group, criterion_main, Criterion};

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};

use liquide_renderer_cpu::blur;
use liquide_renderer_cpu::blend;
use liquide_renderer_cpu::color::{self, SrgbLut};
use liquide_renderer_cpu::glyph::{GlyphAtlas, GlyphKey, GlyphMetrics};
use liquide_renderer_cpu::path::{self, PathBuilder};
use liquide_renderer_cpu::rasterizer::{self, Fill};

// ---------------------------------------------------------------------------
// Blur benchmarks
// ---------------------------------------------------------------------------

fn bench_blur_region(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(256, 256, PixelFormat::Bgra8);
    // Fill with non-trivial data so the blur does real work
    for px in fb.pixels.chunks_exact_mut(4) {
        px[0] = 100; // B
        px[1] = 150; // G
        px[2] = 200; // R
        px[3] = 255; // A
    }
    let region = Rect::new(0.0, 0.0, 256.0, 256.0);

    c.bench_function("blur_region_256x256_r10", |b| {
        b.iter(|| {
            blur::blur_region(black_box(&mut fb), black_box(region), black_box(10));
        });
    });
}

fn bench_blur_fast(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(512, 512, PixelFormat::Bgra8);
    for px in fb.pixels.chunks_exact_mut(4) {
        px[0] = 80;
        px[1] = 120;
        px[2] = 180;
        px[3] = 255;
    }
    let region = Rect::new(0.0, 0.0, 512.0, 512.0);

    c.bench_function("blur_fast_512x512", |b| {
        b.iter(|| {
            blur::blur_fast(black_box(&mut fb), black_box(region), black_box(20));
        });
    });
}

// ---------------------------------------------------------------------------
// Rasterizer benchmarks
// ---------------------------------------------------------------------------

fn bench_fill_rect(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    let rect = Rect::new(100.0, 100.0, 800.0, 600.0);
    let color = Color::new(30, 120, 200, 255);

    c.bench_function("fill_rect_1080p", |b| {
        b.iter(|| {
            rasterizer::fill_rect(
                black_box(&mut fb),
                black_box(rect),
                black_box(color),
                black_box(BlendMode::SrcOver),
            );
        });
    });
}

fn bench_fill_rounded_rect(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    let rect = Rect::new(100.0, 100.0, 800.0, 600.0);
    let fill = Fill::Solid(Color::new(30, 120, 200, 180));
    let lut = SrgbLut::new();

    c.bench_function("fill_rounded_rect_1080p", |b| {
        b.iter(|| {
            rasterizer::fill_rounded_rect(
                black_box(&mut fb),
                black_box(rect),
                black_box(12.0),
                black_box(&fill),
                black_box(BlendMode::SrcOver),
                black_box(&lut),
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Blend benchmark
// ---------------------------------------------------------------------------

fn bench_blend_scanline(c: &mut Criterion) {
    let len = 1920 * 4; // 1920 pixels, BGRA
    let mut dst = vec![128u8; len];
    let src = vec![64u8; len];

    c.bench_function("blend_scanline_1920px", |b| {
        b.iter(|| {
            blend::blend_scanline(
                black_box(&mut dst),
                black_box(&src),
                black_box(BlendMode::SrcOver),
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Color / sRGB benchmark
// ---------------------------------------------------------------------------

fn bench_srgb_roundtrip(c: &mut Criterion) {
    let lut = SrgbLut::new();
    let colors: Vec<Color> = (0..10_000)
        .map(|i| {
            let v = (i % 256) as u8;
            Color::new(v, v.wrapping_mul(3), v.wrapping_mul(7), 255)
        })
        .collect();

    c.bench_function("srgb_roundtrip_10k", |b| {
        b.iter(|| {
            for c_val in &colors {
                let lin = color::linearize(black_box(&lut), black_box(*c_val));
                let _back = color::delinearize(black_box(&lut), black_box(lin));
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Path benchmark
// ---------------------------------------------------------------------------

fn bench_path_fill(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(512, 512, PixelFormat::Bgra8);
    let fill = Fill::Solid(Color::new(255, 0, 0, 200));
    let lut = SrgbLut::new();

    // Build a polygon (irregular hexagon-ish shape)
    let path = PathBuilder::new()
        .move_to(100.0, 50.0)
        .line_to(250.0, 30.0)
        .line_to(400.0, 100.0)
        .line_to(420.0, 300.0)
        .line_to(250.0, 450.0)
        .line_to(80.0, 350.0)
        .close()
        .build();

    c.bench_function("path_fill_polygon", |b| {
        b.iter(|| {
            path::fill_path(
                black_box(&mut fb),
                black_box(&path),
                black_box(&fill),
                black_box(BlendMode::SrcOver),
                black_box(&lut),
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Glyph benchmark
// ---------------------------------------------------------------------------

fn bench_glyph_blit(c: &mut Criterion) {
    let mut fb = FrameBuffer::new(1920, 1080, PixelFormat::Bgra8);
    let mut atlas = GlyphAtlas::new(1024, 1024);
    let color = Color::new(255, 255, 255, 255);

    // Insert 100 fake glyphs (16x20 each, filled with varying alpha)
    let glyph_w = 16u32;
    let glyph_h = 20u32;
    let mut keys = Vec::with_capacity(100);

    for i in 0..100u32 {
        let key = GlyphKey {
            font_id: 1,
            glyph_id: i,
            size_px: 16,
            subpixel: false,
        };
        let bitmap: Vec<u8> = (0..(glyph_w * glyph_h))
            .map(|p| ((p + i * 7) % 256) as u8)
            .collect();
        let metrics = GlyphMetrics {
            width: glyph_w,
            height: glyph_h,
            bearing_x: 0,
            bearing_y: glyph_h as i32,
            advance: glyph_w as f32,
        };
        atlas.insert(key, &bitmap, &metrics).unwrap();
        keys.push(key);
    }

    c.bench_function("glyph_blit_100", |b| {
        b.iter(|| {
            for (i, key) in keys.iter().enumerate() {
                let glyph = atlas.get(key).unwrap();
                let pos = Point::new((i as f32 % 50.0) * 18.0, 100.0 + (i as f32 / 50.0).floor() * 24.0);
                atlas.blit_glyph(
                    black_box(&mut fb),
                    black_box(glyph),
                    black_box(pos),
                    black_box(color),
                );
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Group and main
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_blur_region,
    bench_blur_fast,
    bench_fill_rect,
    bench_fill_rounded_rect,
    bench_blend_scanline,
    bench_srgb_roundtrip,
    bench_path_fill,
    bench_glyph_blit,
);
criterion_main!(benches);
