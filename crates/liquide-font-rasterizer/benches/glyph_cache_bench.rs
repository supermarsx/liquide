use std::sync::Arc;

use criterion::{
    BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main,
};

use liquide_font_rasterizer::{FontFaceId, GlyphBitmap, GlyphCache, GlyphCacheKey};

const GLYPH_WIDTH: u32 = 24;
const GLYPH_HEIGHT: u32 = 24;

fn dummy_bitmap(glyph_id: u32) -> GlyphBitmap {
    GlyphBitmap {
        glyph_id,
        width: GLYPH_WIDTH,
        height: GLYPH_HEIGHT,
        bearing_x: 1.0,
        bearing_y: 18.0,
        advance: 14.0,
        pixels: Arc::from(vec![glyph_id as u8; (GLYPH_WIDTH * GLYPH_HEIGHT) as usize]),
        is_subpixel: false,
    }
}

fn build_keys(start: u32, count: usize) -> Vec<GlyphCacheKey> {
    (0..count)
        .map(|index| {
            GlyphCacheKey::new(
                FontFaceId(1),
                start + index as u32,
                16.0 + (index % 3) as f32,
                0.0,
                0.0,
            )
        })
        .collect()
}

fn populate_cache(cache: &GlyphCache, keys: &[GlyphCacheKey]) {
    for key in keys {
        cache.insert(*key, dummy_bitmap(key.glyph_id));
    }
}

fn bench_glyph_cache_hot_hits(c: &mut Criterion) {
    let mut group = c.benchmark_group("glyph_cache_hot_hits");
    for &glyph_count in &[256usize, 1024usize] {
        let cache = GlyphCache::new(glyph_count * 2, 32 * 1024 * 1024);
        let keys = build_keys(0, glyph_count);
        populate_cache(&cache, &keys);

        group.bench_with_input(BenchmarkId::new("shared_bitmap_hits", glyph_count), &glyph_count, |b, _| {
            b.iter(|| {
                for key in &keys {
                    let bitmap = cache.get(black_box(key)).expect("glyph should be cached");
                    black_box(Arc::as_ptr(&bitmap.pixels));
                }
            });
        });
    }
    group.finish();
}

fn bench_glyph_cache_eviction_pressure(c: &mut Criterion) {
    let existing_keys = build_keys(0, 512);
    let incoming_keys = build_keys(10_000, 512);
    let max_bytes = 512 * (GLYPH_WIDTH * GLYPH_HEIGHT) as usize;

    c.bench_function("glyph_cache_eviction_pressure_512", |b| {
        b.iter_batched(
            || {
                let cache = GlyphCache::new(512, max_bytes);
                populate_cache(&cache, &existing_keys);
                cache
            },
            |cache| {
                for key in &incoming_keys {
                    cache.insert(*key, dummy_bitmap(key.glyph_id));
                }
                black_box(cache.stats().entries);
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_glyph_cache_hot_hits,
    bench_glyph_cache_eviction_pressure,
);
criterion_main!(benches);