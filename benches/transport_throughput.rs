use std::collections::{HashMap, HashSet};

use bytes::BytesMut;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use liquide_compositor::damage::{DamageClass, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_encoder::{BandwidthBudget, TileBatch, TileConfig, TileEncoder, fragment_batch};
use liquide_protocol::codec::cbor_encode;
use liquide_protocol::{ChannelId, FrameFlags, FrameHeader, MessageType};

const FRAME_WIDTH: u32 = 1024;
const FRAME_HEIGHT: u32 = 640;
const TILE_SIZE: u32 = 64;
const BYTES_PER_PIXEL: u32 = 4;
const MTU_BYTES: usize = 1200;

struct EncodeScenario {
    baseline: FrameBuffer,
    current: FrameBuffer,
    full_damage: Vec<DamageTile>,
    trimmed_damage: Vec<DamageTile>,
}

struct TransportScenario {
    name: &'static str,
    batch: TileBatch,
    mtu: usize,
    wire_bytes: u64,
    metrics: TransportMetrics,
}

struct TransportMetrics {
    dirty_tiles: usize,
    fragment_count: usize,
    metadata_only_fragments: usize,
    empty_tiles: usize,
    fragmented_tiles: usize,
    priority_fragments: usize,
    tile_payload_bytes: usize,
    encoded_fragment_payload_bytes: usize,
    avg_encoded_fragment_payload_bytes: f64,
    max_encoded_fragment_payload_bytes: usize,
    max_wire_fragment_bytes: usize,
    over_budget_fragments: usize,
    frame_overhead_bytes: u64,
    wire_bytes: u64,
}

impl TransportScenario {
    fn emit_summary(&self) {
        println!(
            "[transport-bench] {} | dirty_tiles={} fragments={} metadata_only_fragments={} empty_tiles={} fragmented_tiles={} priority_fragments={} tile_payload_bytes={} encoded_fragment_payload_bytes={} avg_encoded_fragment_payload_bytes={:.1} max_encoded_fragment_payload_bytes={} max_wire_fragment_bytes={} over_budget_fragments={} frame_overhead_bytes={} wire_bytes={} lz4_tiles={} zstd_tiles={}",
            self.name,
            self.metrics.dirty_tiles,
            self.metrics.fragment_count,
            self.metrics.metadata_only_fragments,
            self.metrics.empty_tiles,
            self.metrics.fragmented_tiles,
            self.metrics.priority_fragments,
            self.metrics.tile_payload_bytes,
            self.metrics.encoded_fragment_payload_bytes,
            self.metrics.avg_encoded_fragment_payload_bytes,
            self.metrics.max_encoded_fragment_payload_bytes,
            self.metrics.max_wire_fragment_bytes,
            self.metrics.over_budget_fragments,
            self.metrics.frame_overhead_bytes,
            self.metrics.wire_bytes,
            self.batch.stats.lz4_tiles,
            self.batch.stats.zstd_tiles,
        );
    }
}

fn bench_transport_damage_roi(c: &mut Criterion) {
    let sparse_ui = build_sparse_ui_delta();
    let cursor_micro = build_cursor_micro_updates();
    let scenarios = [
        transport_case(
            "sparse_ui_delta/full_damage_broad_grid",
            encode_batch(&sparse_ui, &sparse_ui.full_damage, None),
            MTU_BYTES,
        ),
        transport_case(
            "sparse_ui_delta/trimmed_damage_changed_tiles_only",
            encode_batch(&sparse_ui, &sparse_ui.trimmed_damage, None),
            MTU_BYTES,
        ),
        transport_case(
            "cursor_heavy_micro_updates/full_damage_broad_grid",
            encode_batch(&cursor_micro, &cursor_micro.full_damage, None),
            MTU_BYTES,
        ),
        transport_case(
            "cursor_heavy_micro_updates/trimmed_damage_changed_tiles_only",
            encode_batch(&cursor_micro, &cursor_micro.trimmed_damage, None),
            MTU_BYTES,
        ),
    ];

    let mut group = c.benchmark_group("transport_throughput_damage_trim_roi");
    for scenario in &scenarios {
        scenario.emit_summary();
        group.throughput(Throughput::Bytes(scenario.wire_bytes));
        group.bench_function(BenchmarkId::from_parameter(scenario.name), |b| {
            b.iter(|| black_box(packetize_batch_to_wire_bytes(&scenario.batch, scenario.mtu)));
        });
    }
    group.finish();
}

fn bench_transport_budget_roi(c: &mut Criterion) {
    let bitmap_heavy = build_bitmap_heavy_damage();
    let pressured_budget = pressured_budget_hint();
    let scenarios = [
        transport_case(
            "bitmap_heavy_damage/normal_budget_zstd_bitmap_tiles",
            encode_batch(&bitmap_heavy, &bitmap_heavy.trimmed_damage, None),
            MTU_BYTES,
        ),
        transport_case(
            "bitmap_heavy_damage/pressured_budget_lz4_bitmap_tiles",
            encode_batch(
                &bitmap_heavy,
                &bitmap_heavy.trimmed_damage,
                Some(&pressured_budget),
            ),
            MTU_BYTES,
        ),
    ];

    let mut group = c.benchmark_group("transport_throughput_budget_pressure_roi");
    for scenario in &scenarios {
        scenario.emit_summary();
        group.throughput(Throughput::Bytes(scenario.wire_bytes));
        group.bench_function(BenchmarkId::from_parameter(scenario.name), |b| {
            b.iter(|| black_box(packetize_batch_to_wire_bytes(&scenario.batch, scenario.mtu)));
        });
    }
    group.finish();
}

fn transport_case(name: &'static str, batch: TileBatch, mtu: usize) -> TransportScenario {
    let metrics = measure_transport_metrics(&batch, mtu);
    TransportScenario {
        name,
        batch,
        mtu,
        wire_bytes: metrics.wire_bytes,
        metrics,
    }
}

fn measure_transport_metrics(batch: &TileBatch, mtu: usize) -> TransportMetrics {
    let fragments = fragment_batch(batch, mtu, 0).expect("fragmentation should succeed");
    let mut encoded_fragment_payload_bytes = 0usize;
    let mut metadata_only_fragments = 0usize;
    let mut empty_tiles = 0usize;
    let mut max_encoded_fragment_payload_bytes = 0usize;
    let mut max_wire_fragment_bytes = 0usize;
    let mut over_budget_fragments = 0usize;
    let mut priority_fragments = 0usize;
    let mut fragmented_tiles = HashSet::new();

    for fragment in &fragments {
        if fragment.payload.is_empty()
            && fragment
                .bundled_tiles
                .iter()
                .all(|tile| tile.payload.is_empty())
        {
            metadata_only_fragments += 1;
        }
        let payload = cbor_encode(fragment).expect("fragment serialization should succeed");
        encoded_fragment_payload_bytes += payload.len();
        max_encoded_fragment_payload_bytes = max_encoded_fragment_payload_bytes.max(payload.len());
        let wire_bytes = payload.len() + FrameHeader::WIRE_SIZE;
        max_wire_fragment_bytes = max_wire_fragment_bytes.max(wire_bytes);
        if wire_bytes > mtu {
            over_budget_fragments += 1;
        }
        if fragment.damage_class == DamageClass::CursorOnly
            || fragment
                .bundled_tiles
                .iter()
                .any(|tile| tile.damage_class == DamageClass::CursorOnly)
        {
            priority_fragments += 1;
        }
        if fragment.payload.is_empty() {
            empty_tiles += 1;
        }
        if fragment.fragment_count > 1 {
            fragmented_tiles.insert(fragment.tile_index);
        }
        for tile in &fragment.bundled_tiles {
            if tile.payload.is_empty() {
                empty_tiles += 1;
            }
            if tile.fragment_count > 1 {
                fragmented_tiles.insert(tile.tile_index);
            }
        }
    }

    let fragment_count = fragments.len();
    let frame_overhead_bytes = fragment_count as u64 * FrameHeader::WIRE_SIZE as u64;
    let wire_bytes = encoded_fragment_payload_bytes as u64 + frame_overhead_bytes;

    TransportMetrics {
        dirty_tiles: batch.dirty_count(),
        fragment_count,
        metadata_only_fragments,
        empty_tiles,
        fragmented_tiles: fragmented_tiles.len(),
        priority_fragments,
        tile_payload_bytes: batch.total_payload_bytes(),
        encoded_fragment_payload_bytes,
        avg_encoded_fragment_payload_bytes: if fragment_count == 0 {
            0.0
        } else {
            encoded_fragment_payload_bytes as f64 / fragment_count as f64
        },
        max_encoded_fragment_payload_bytes,
        max_wire_fragment_bytes,
        over_budget_fragments,
        frame_overhead_bytes,
        wire_bytes,
    }
}

// Keep the throughput path local: fragment a batch, CBOR-encode each fragment,
// and wrap it in a protocol frame header without touching a live transport.
fn packetize_batch_to_wire_bytes(batch: &TileBatch, mtu: usize) -> u64 {
    let fragments = fragment_batch(batch, mtu, 0).expect("fragmentation should succeed");
    let mut total = 0u64;

    for fragment in &fragments {
        let payload = cbor_encode(fragment).expect("fragment serialization should succeed");
        let header = FrameHeader::new(
            ChannelId::TILE,
            fragment.sequence as u32,
            0,
            MessageType::TileBatch.as_u16(),
            frame_flags_for(fragment),
            payload.len() as u16,
        );
        let mut wire = BytesMut::with_capacity(FrameHeader::WIRE_SIZE + payload.len());
        header.encode(&mut wire);
        wire.extend_from_slice(payload.as_ref());
        total += wire.len() as u64;
    }

    total
}

fn frame_flags_for(fragment: &liquide_encoder::BatchFragment) -> u8 {
    let mut flags = FrameFlags::ORDERED;
    if fragment.fragment_count > 1
        || fragment
            .bundled_tiles
            .iter()
            .any(|tile| tile.fragment_count > 1)
    {
        flags |= FrameFlags::FRAGMENTED;
    }
    if fragment.damage_class == DamageClass::CursorOnly
        || fragment
            .bundled_tiles
            .iter()
            .any(|tile| tile.damage_class == DamageClass::CursorOnly)
    {
        flags |= FrameFlags::PRIORITY;
    }
    flags
}

fn encode_batch(
    scenario: &EncodeScenario,
    damage: &[DamageTile],
    budget_hint: Option<&BandwidthBudget>,
) -> TileBatch {
    let mut encoder = TileEncoder::new(FRAME_WIDTH, FRAME_HEIGHT, tile_config());
    let _ = encoder
        .encode_frame(&scenario.baseline, &scenario.full_damage)
        .expect("transport throughput warm-up should succeed");

    match budget_hint {
        Some(budget_hint) => encoder
            .encode_frame_with_budget_hint(&scenario.current, damage, Some(budget_hint))
            .expect("transport throughput encode should succeed"),
        None => encoder
            .encode_frame(&scenario.current, damage)
            .expect("transport throughput encode should succeed"),
    }
}

fn tile_config() -> TileConfig {
    TileConfig {
        tile_size: TILE_SIZE,
        bpp: BYTES_PER_PIXEL,
    }
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
            DamageClass::TextGlyph => {
                paint_text_tile(&mut current, tile.x, tile.y, 48 + index as u8)
            }
            DamageClass::UiPrimitive => {
                paint_ui_tile(&mut current, tile.x, tile.y, 72 + index as u8)
            }
            DamageClass::BitmapRegion => {
                paint_bitmap_tile(&mut current, tile.x, tile.y, 10_000 + index as u64)
            }
            DamageClass::CursorOnly => {}
        }
    }

    EncodeScenario {
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
            paint_bitmap_tile(&mut current, tx, ty, 50_000 + (ty as u64 * 31) + tx as u64);
        }
    }

    EncodeScenario {
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
        .expect("transport throughput benches require CPU framebuffers");

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
    fill_rect(
        &mut frame,
        0,
        FRAME_HEIGHT - 76,
        FRAME_WIDTH,
        76,
        [36, 38, 46, 255],
    );
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
    fill_rect(
        frame,
        x0,
        y0,
        width,
        8,
        [
            accent,
            accent.wrapping_add(22),
            accent.wrapping_add(48),
            255,
        ],
    );
    fill_rect(
        frame,
        x0 + 8,
        y0 + 16,
        width.saturating_sub(16),
        10,
        [214, 214, 220, 255],
    );
    fill_rect(frame, x0 + 8, y0 + 32, width / 2, 12, [96, 142, 196, 255]);
    fill_rect(
        frame,
        x0 + 8,
        y0 + 48,
        width.saturating_sub(24),
        8,
        [96, 102, 118, 255],
    );
}

fn paint_text_tile(frame: &mut FrameBuffer, tx: u32, ty: u32, accent: u8) {
    let (x0, y0, x1, y1) = tile_bounds(tx, ty);
    let width = x1.saturating_sub(x0);
    let height = y1.saturating_sub(y0);
    fill_rect(frame, x0, y0, width, height, [46, 50, 64, 255]);
    fill_rect(
        frame,
        x0,
        y0,
        width,
        8,
        [
            accent,
            accent.wrapping_add(12),
            accent.wrapping_add(42),
            255,
        ],
    );
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
        .expect("transport throughput benches require CPU framebuffers");
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
                [noise, noise.rotate_left(2), noise.rotate_left(5), 255],
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
        .expect("transport throughput benches require CPU framebuffers");

    for dy in 0..18 {
        let row_width = dy.min(7) + 1;
        for dx in 0..row_width {
            write_pixel_safe(
                pixels,
                stride,
                width,
                height,
                origin_x + dx,
                origin_y + dy,
                color,
            );
        }
    }

    for dy in 10..18 {
        for dx in 4..8 {
            write_pixel_safe(
                pixels,
                stride,
                width,
                height,
                origin_x + dx,
                origin_y + dy,
                color,
            );
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
        .expect("transport throughput benches require CPU framebuffers");

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
    bench_transport_damage_roi,
    bench_transport_budget_roi,
);
criterion_main!(benches);
