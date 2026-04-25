use std::collections::HashMap;

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group,
    criterion_main,
};

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::{BandwidthBudget, TileConfig, TileEncoder};

const FRAME_WIDTH: u32 = 1024;
const FRAME_HEIGHT: u32 = 640;
const TILE_SIZE: u32 = 64;
const BYTES_PER_PIXEL: u32 = 4;

struct EncodeScenario {
    name: &'static str,
    baseline: FrameBuffer,
    current: FrameBuffer,
    full_damage: Vec<DamageTile>,
    trimmed_damage: Vec<DamageTile>,
}

fn bench_tile_encode_damage_roi(c: &mut Criterion) {
    let sparse_ui = build_sparse_ui_delta();
    let cursor_micro = build_cursor_micro_updates();
    let scenarios = [&sparse_ui, &cursor_micro];

    let mut group = c.benchmark_group("tile_encode_damage_trim_roi");
    for scenario in scenarios {
        group.throughput(Throughput::Bytes(uncompressed_damage_bytes(
            scenario.full_damage.len(),
        )));
        group.bench_function(
            BenchmarkId::new(scenario.name, "full_damage_broad_grid"),
            |b| {
                b.iter_batched(
                    || warm_encoder(scenario),
                    |mut encoder| {
                        let batch = encoder
                            .encode_frame(
                                black_box(&scenario.current),
                                black_box(&scenario.full_damage),
                            )
                            .expect("tile encode benchmark should succeed");
                        black_box((
                            batch.compressed_bytes,
                            batch.stats.tiles_encoded,
                            batch.stats.bytes_saved,
                        ));
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.throughput(Throughput::Bytes(uncompressed_damage_bytes(
            scenario.trimmed_damage.len(),
        )));
        group.bench_function(
            BenchmarkId::new(scenario.name, "trimmed_damage_changed_tiles_only"),
            |b| {
                b.iter_batched(
                    || warm_encoder(scenario),
                    |mut encoder| {
                        let batch = encoder
                            .encode_frame(
                                black_box(&scenario.current),
                                black_box(&scenario.trimmed_damage),
                            )
                            .expect("tile encode benchmark should succeed");
                        black_box((
                            batch.compressed_bytes,
                            batch.stats.tiles_encoded,
                            batch.stats.bytes_saved,
                        ));
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_tile_encode_budget_roi(c: &mut Criterion) {
    let bitmap_heavy = build_bitmap_heavy_damage();
    let pressured_budget = pressured_budget_hint();

    let mut group = c.benchmark_group("tile_encode_budget_pressure_roi");
    group.throughput(Throughput::Bytes(uncompressed_damage_bytes(
        bitmap_heavy.trimmed_damage.len(),
    )));
    group.bench_function(
        BenchmarkId::new(bitmap_heavy.name, "normal_budget_zstd_bitmap_tiles"),
        |b| {
            b.iter_batched(
                || warm_encoder(&bitmap_heavy),
                |mut encoder| {
                    let batch = encoder
                        .encode_frame(
                            black_box(&bitmap_heavy.current),
                            black_box(&bitmap_heavy.trimmed_damage),
                        )
                        .expect("tile encode benchmark should succeed");
                    black_box((
                        batch.compressed_bytes,
                        batch.stats.zstd_tiles,
                        batch.stats.lz4_tiles,
                    ));
                },
                BatchSize::SmallInput,
            );
        },
    );

    group.throughput(Throughput::Bytes(uncompressed_damage_bytes(
        bitmap_heavy.trimmed_damage.len(),
    )));
    group.bench_function(
        BenchmarkId::new(bitmap_heavy.name, "pressured_budget_lz4_bitmap_tiles"),
        |b| {
            b.iter_batched(
                || warm_encoder(&bitmap_heavy),
                |mut encoder| {
                    let batch = encoder
                        .encode_frame_with_budget_hint(
                            black_box(&bitmap_heavy.current),
                            black_box(&bitmap_heavy.trimmed_damage),
                            Some(&pressured_budget),
                        )
                        .expect("tile encode benchmark should succeed");
                    black_box((
                        batch.compressed_bytes,
                        batch.stats.zstd_tiles,
                        batch.stats.lz4_tiles,
                    ));
                },
                BatchSize::SmallInput,
            );
        },
    );
    group.finish();
}

fn warm_encoder(scenario: &EncodeScenario) -> TileEncoder {
    let mut encoder = TileEncoder::new(FRAME_WIDTH, FRAME_HEIGHT, tile_config());
    let _ = encoder
        .encode_frame(&scenario.baseline, &scenario.full_damage)
        .expect("tile encode warm-up should succeed");
    encoder
}

fn tile_config() -> TileConfig {
    TileConfig {
        tile_size: TILE_SIZE,
        bpp: BYTES_PER_PIXEL,
    }
}

fn uncompressed_damage_bytes(tile_count: usize) -> u64 {
    tile_count as u64 * (TILE_SIZE * TILE_SIZE * BYTES_PER_PIXEL) as u64
}

fn pressured_budget_hint() -> BandwidthBudget {
    let mut budget = BandwidthBudget::new(12_288, 0.1);
    let _ = budget.observe(48_000);
    budget
}

fn build_sparse_ui_delta() -> EncodeScenario {
    let baseline = make_desktop_frame(17);
    let mut current = make_desktop_frame(17);

    let trimmed_damage = vec![
        DamageTile {
            x: 6,
            y: 3,
            class: DamageClass::TextGlyph,
        },
        DamageTile {
            x: 7,
            y: 3,
            class: DamageClass::TextGlyph,
        },
        DamageTile {
            x: 8,
            y: 3,
            class: DamageClass::TextGlyph,
        },
        DamageTile {
            x: 6,
            y: 4,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 7,
            y: 4,
            class: DamageClass::UiPrimitive,
        },
        DamageTile {
            x: 8,
            y: 4,
            class: DamageClass::UiPrimitive,
        },
    ];

    for (index, tile) in trimmed_damage.iter().enumerate() {
        match tile.class {
            DamageClass::TextGlyph => paint_text_tile(&mut current, tile.x, tile.y, 48 + index as u8),
            DamageClass::UiPrimitive => paint_ui_tile(&mut current, tile.x, tile.y, 72 + index as u8),
            DamageClass::BitmapRegion => paint_bitmap_tile(
                &mut current,
                tile.x,
                tile.y,
                10_000 + index as u64,
            ),
            DamageClass::CursorOnly => {}
        }
    }

    EncodeScenario {
        name: "sparse_ui_delta",
        baseline,
        current,
        full_damage: full_damage_with_overrides(&trimmed_damage, DamageClass::UiPrimitive),
        trimmed_damage,
    }
}

fn build_bitmap_heavy_damage() -> EncodeScenario {
    let baseline = make_desktop_frame(29);
    let mut current = make_desktop_frame(29);
    let mut trimmed_damage = Vec::new();

    for ty in 4..8 {
        for tx in 4..10 {
            trimmed_damage.push(DamageTile {
                x: tx,
                y: ty,
                class: DamageClass::BitmapRegion,
            });
            paint_bitmap_tile(
                &mut current,
                tx,
                ty,
                50_000 + (ty as u64 * 31) + tx as u64,
            );
        }
    }

    EncodeScenario {
        name: "bitmap_heavy_damage",
        baseline,
        current,
        full_damage: full_damage_with_overrides(&trimmed_damage, DamageClass::UiPrimitive),
        trimmed_damage,
    }
}

fn build_cursor_micro_updates() -> EncodeScenario {
    let mut baseline = make_desktop_frame(41);
    let mut current = make_desktop_frame(41);

    draw_cursor(&mut baseline, TILE_SIZE * 3 + 10, TILE_SIZE * 2 + 12);
    draw_cursor(&mut current, TILE_SIZE * 4 + 18, TILE_SIZE * 2 + 14);

    let trimmed_damage = vec![
        DamageTile {
            x: 3,
            y: 2,
            class: DamageClass::CursorOnly,
        },
        DamageTile {
            x: 4,
            y: 2,
            class: DamageClass::CursorOnly,
        },
    ];

    EncodeScenario {
        name: "cursor_heavy_micro_updates",
        baseline,
        current,
        full_damage: full_damage_with_overrides(&trimmed_damage, DamageClass::UiPrimitive),
        trimmed_damage,
    }
}

fn full_damage_with_overrides(
    overrides: &[DamageTile],
    default_class: DamageClass,
) -> Vec<DamageTile> {
    let cols = FRAME_WIDTH.div_ceil(TILE_SIZE);
    let rows = FRAME_HEIGHT.div_ceil(TILE_SIZE);
    let override_lookup: HashMap<(u32, u32), DamageClass> = overrides
        .iter()
        .map(|tile| ((tile.x, tile.y), tile.class))
        .collect();

    let mut damage = Vec::with_capacity((cols * rows) as usize);
    for ty in 0..rows {
        for tx in 0..cols {
            damage.push(DamageTile {
                x: tx,
                y: ty,
                class: override_lookup
                    .get(&(tx, ty))
                    .copied()
                    .unwrap_or(default_class),
            });
        }
    }
    damage
}

fn make_desktop_frame(seed: u8) -> FrameBuffer {
    let mut frame = FrameBuffer::new(FRAME_WIDTH, FRAME_HEIGHT, PixelFormat::Bgra8);
    let stride = frame.stride;
    let pixels = frame
        .pixels_mut()
        .expect("tile encode benches require CPU framebuffers");

    for y in 0..FRAME_HEIGHT {
        for x in 0..FRAME_WIDTH {
            let blue = seed
                .wrapping_add((x / 3) as u8)
                .wrapping_add(((y / 11) % 23) as u8);
            let green = seed
                .wrapping_add((y / 2) as u8)
                .wrapping_add(((x / 17) % 19) as u8);
            let red = seed
                .wrapping_add(((x + y) / 5) as u8)
                .wrapping_add((((x / 32) + (y / 24)) % 13) as u8);
            write_pixel(pixels, stride, x, y, [blue, green, red, 255]);
        }
    }

    fill_rect(&mut frame, 0, 0, FRAME_WIDTH, 44, [52, 56, 64, 255]);
    fill_rect(&mut frame, 0, FRAME_HEIGHT - 76, FRAME_WIDTH, 76, [36, 38, 46, 255]);
    fill_rect(&mut frame, 72, 84, 360, 220, [72, 78, 92, 255]);
    fill_rect(&mut frame, 84, 96, 336, 20, [98, 132, 182, 255]);
    fill_rect(&mut frame, 560, 120, 288, 184, [58, 64, 76, 255]);
    fill_rect(&mut frame, 572, 132, 264, 16, [182, 114, 78, 255]);
    frame
}

fn paint_ui_tile(frame: &mut FrameBuffer, tx: u32, ty: u32, accent: u8) {
    let (x0, y0, x1, y1) = tile_bounds(tx, ty);
    let width = x1.saturating_sub(x0);
    let height = y1.saturating_sub(y0);
    fill_rect(frame, x0, y0, width, height, [56, 60, 74, 255]);
    fill_rect(frame, x0, y0, width, 8, [accent, accent.wrapping_add(22), accent.wrapping_add(48), 255]);
    fill_rect(frame, x0 + 8, y0 + 16, width.saturating_sub(16), 10, [214, 214, 220, 255]);
    fill_rect(frame, x0 + 8, y0 + 32, width / 2, 12, [96, 142, 196, 255]);
    fill_rect(frame, x0 + 8, y0 + 48, width.saturating_sub(24), 8, [96, 102, 118, 255]);
}

fn paint_text_tile(frame: &mut FrameBuffer, tx: u32, ty: u32, accent: u8) {
    let (x0, y0, x1, y1) = tile_bounds(tx, ty);
    let width = x1.saturating_sub(x0);
    let height = y1.saturating_sub(y0);
    fill_rect(frame, x0, y0, width, height, [46, 50, 64, 255]);
    fill_rect(frame, x0, y0, width, 8, [accent, accent.wrapping_add(12), accent.wrapping_add(42), 255]);
    for (row, line_width) in [width - 18, width - 24, width - 28, width - 20]
        .into_iter()
        .enumerate()
    {
        fill_rect(
            frame,
            x0 + 8,
            y0 + 14 + (row as u32 * 11),
            line_width,
            4,
            [230, 232, 238, 255],
        );
    }
}

fn paint_bitmap_tile(frame: &mut FrameBuffer, tx: u32, ty: u32, seed: u64) {
    let (x0, y0, x1, y1) = tile_bounds(tx, ty);
    let stride = frame.stride;
    let pixels = frame
        .pixels_mut()
        .expect("tile encode benches require CPU framebuffers");
    let mut state = seed | 1;

    for y in y0..y1 {
        for x in x0..x1 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = state as u8;
            write_pixel(
                pixels,
                stride,
                x,
                y,
                [
                    noise,
                    noise.rotate_left(2),
                    noise.rotate_left(5),
                    255,
                ],
            );
        }
    }
}

fn draw_cursor(frame: &mut FrameBuffer, origin_x: u32, origin_y: u32) {
    draw_cursor_layer(frame, origin_x + 1, origin_y + 1, [18, 22, 28, 255]);
    draw_cursor_layer(frame, origin_x, origin_y, [244, 246, 250, 255]);
}

fn draw_cursor_layer(frame: &mut FrameBuffer, origin_x: u32, origin_y: u32, color: [u8; 4]) {
    let stride = frame.stride;
    let width = frame.width;
    let height = frame.height;
    let pixels = frame
        .pixels_mut()
        .expect("tile encode benches require CPU framebuffers");

    for dy in 0..18 {
        let row_width = dy.min(7) + 1;
        for dx in 0..row_width {
            write_pixel_safe(pixels, stride, width, height, origin_x + dx, origin_y + dy, color);
        }
    }

    for dy in 10..18 {
        for dx in 4..8 {
            write_pixel_safe(pixels, stride, width, height, origin_x + dx, origin_y + dy, color);
        }
    }
}

fn tile_bounds(tx: u32, ty: u32) -> (u32, u32, u32, u32) {
    let x0 = tx * TILE_SIZE;
    let y0 = ty * TILE_SIZE;
    let x1 = (x0 + TILE_SIZE).min(FRAME_WIDTH);
    let y1 = (y0 + TILE_SIZE).min(FRAME_HEIGHT);
    (x0, y0, x1, y1)
}

fn fill_rect(frame: &mut FrameBuffer, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    let stride = frame.stride;
    let frame_width = frame.width;
    let frame_height = frame.height;
    let x_end = x.saturating_add(width).min(frame_width);
    let y_end = y.saturating_add(height).min(frame_height);
    let pixels = frame
        .pixels_mut()
        .expect("tile encode benches require CPU framebuffers");

    for py in y..y_end {
        for px in x..x_end {
            write_pixel(pixels, stride, px, py, color);
        }
    }
}

fn write_pixel(pixels: &mut [u8], stride: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = (y * stride + x * BYTES_PER_PIXEL) as usize;
    pixels[offset..offset + BYTES_PER_PIXEL as usize].copy_from_slice(&color);
}

fn write_pixel_safe(
    pixels: &mut [u8],
    stride: u32,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    color: [u8; 4],
) {
    if x >= width || y >= height {
        return;
    }
    write_pixel(pixels, stride, x, y, color);
}

criterion_group!(
    benches,
    bench_tile_encode_damage_roi,
    bench_tile_encode_budget_roi,
);
criterion_main!(benches);
