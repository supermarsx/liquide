//! Benchmarks for SIMD-accelerated operations.

use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_blend_src_over(c: &mut Criterion) {
    let size = 1920 * 4; // 1920 pixels scanline
    let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let mut dst = vec![128u8; size];

    c.bench_function("blend_src_over_1920px", |b| {
        b.iter(|| {
            dst.fill(128);
            liquide_simd::blend::blend_scanline_src_over(black_box(&mut dst), black_box(&src));
        })
    });
}

fn bench_blend_src_over_scalar(c: &mut Criterion) {
    let size = 1920 * 4;
    let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let mut dst = vec![128u8; size];

    c.bench_function("blend_src_over_scalar_1920px", |b| {
        b.iter(|| {
            dst.fill(128);
            liquide_simd::blend::blend_scanline_src_over_scalar(
                black_box(&mut dst),
                black_box(&src),
            );
        })
    });
}

fn bench_xor_delta(c: &mut Criterion) {
    let size = 64 * 64 * 4; // 16KB tile
    let current: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let previous: Vec<u8> = (0..size).map(|i| ((i + 1) % 256) as u8).collect();
    let mut dst = vec![0u8; size];

    c.bench_function("xor_delta_16kb", |b| {
        b.iter(|| {
            liquide_simd::delta::xor_delta(
                black_box(&mut dst),
                black_box(&current),
                black_box(&previous),
            );
        })
    });
}

fn bench_xor_popcount(c: &mut Criterion) {
    let size = 64 * 64 * 4;
    let delta: Vec<u8> = (0..size).map(|i| if i % 3 == 0 { 0 } else { 1 }).collect();

    c.bench_function("xor_popcount_16kb", |b| {
        b.iter(|| {
            black_box(liquide_simd::delta::xor_popcount(black_box(&delta)));
        })
    });
}

fn bench_crc32c(c: &mut Criterion) {
    let data: Vec<u8> = (0..16384).map(|i| (i % 256) as u8).collect();

    c.bench_function("crc32c_16kb", |b| {
        b.iter(|| {
            black_box(liquide_simd::crc::crc32c(black_box(&data)));
        })
    });

    c.bench_function("crc32c_table_16kb", |b| {
        b.iter(|| {
            black_box(liquide_simd::crc::crc32c_table(black_box(&data)));
        })
    });
}

fn bench_fill_pattern(c: &mut Criterion) {
    let size = 1920 * 1080 * 4; // full 1080p
    let mut buf = vec![0u8; size];

    c.bench_function("fill_pattern_1080p", |b| {
        b.iter(|| {
            liquide_simd::fill::fill_pattern(black_box(&mut buf), [100, 150, 200, 255]);
        })
    });
}

fn bench_blur_horizontal(c: &mut Criterion) {
    let w = 256u32;
    let h = 256u32;
    let size = (w * h * 4) as usize;
    let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let mut dst = vec![0u8; size];
    let half = 5;
    let weights: Vec<f32> = (0..11).map(|i| {
        let x = (i as f32 - 5.0) / 3.0;
        (-x * x / 2.0).exp()
    }).collect();
    let sum: f32 = weights.iter().sum();
    let weights: Vec<f32> = weights.iter().map(|w| w / sum).collect();

    c.bench_function("blur_h_256x256_r5", |b| {
        b.iter(|| {
            liquide_simd::blur::blur_horizontal(
                black_box(&src),
                black_box(&mut dst),
                w,
                h,
                half,
                &weights,
            );
        })
    });
}

fn bench_premultiply(c: &mut Criterion) {
    let size = 1920 * 4;
    let mut buf: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    c.bench_function("premultiply_1920px", |b| {
        b.iter(|| {
            liquide_simd::fill::premultiply_alpha(black_box(&mut buf));
        })
    });
}

fn bench_invert(c: &mut Criterion) {
    let size = 1920 * 4;
    let mut buf = vec![128u8; size];

    c.bench_function("invert_1920px", |b| {
        b.iter(|| {
            liquide_simd::blend::invert_scanline(black_box(&mut buf));
        })
    });
}

fn bench_filter_brightness(c: &mut Criterion) {
    let size = 1920 * 4;
    let mut buf: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    c.bench_function("brightness_1920px", |b| {
        b.iter(|| {
            liquide_simd::filter::brightness(black_box(&mut buf), 1.5);
        })
    });
}

fn bench_blur_horizontal_scalar(c: &mut Criterion) {
    let w = 256u32;
    let h = 256u32;
    let size = (w * h * 4) as usize;
    let src: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let mut dst = vec![0u8; size];
    let half = 5;
    let weights: Vec<f32> = (0..11).map(|i| {
        let x = (i as f32 - 5.0) / 3.0;
        (-x * x / 2.0).exp()
    }).collect();
    let sum: f32 = weights.iter().sum();
    let weights: Vec<f32> = weights.iter().map(|w| w / sum).collect();

    c.bench_function("blur_h_scalar_256x256_r5", |b| {
        b.iter(|| {
            liquide_simd::blur::blur_horizontal_scalar(
                black_box(&src),
                black_box(&mut dst),
                w,
                h,
                half,
                &weights,
            );
        })
    });
}

fn bench_xor_delta_large(c: &mut Criterion) {
    let size = 1920 * 1080 * 4; // full 1080p frame
    let current: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    let previous: Vec<u8> = (0..size).map(|i| ((i + 3) % 256) as u8).collect();
    let mut dst = vec![0u8; size];

    c.bench_function("xor_delta_1080p", |b| {
        b.iter(|| {
            liquide_simd::delta::xor_delta(
                black_box(&mut dst),
                black_box(&current),
                black_box(&previous),
            );
        })
    });
}

fn bench_color_matrix(c: &mut Criterion) {
    let size = 1920 * 4;
    let mut buf: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
    #[rustfmt::skip]
    let sepia: [f32; 20] = [
        0.272, 0.534, 0.131, 0.0, 0.0,
        0.349, 0.686, 0.168, 0.0, 0.0,
        0.393, 0.769, 0.189, 0.0, 0.0,
        0.0,   0.0,   0.0,   1.0, 0.0,
    ];

    c.bench_function("color_matrix_sepia_1920px", |b| {
        b.iter(|| {
            liquide_simd::filter::color_matrix(black_box(&mut buf), &sepia);
        })
    });
}

criterion_group!(
    benches,
    bench_blend_src_over,
    bench_blend_src_over_scalar,
    bench_xor_delta,
    bench_xor_delta_large,
    bench_xor_popcount,
    bench_crc32c,
    bench_fill_pattern,
    bench_blur_horizontal,
    bench_blur_horizontal_scalar,
    bench_premultiply,
    bench_invert,
    bench_filter_brightness,
    bench_color_matrix,
);
criterion_main!(benches);
