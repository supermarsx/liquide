use criterion::{criterion_group, criterion_main, Criterion};

fn bench_tile_encode(c: &mut Criterion) {
    c.bench_function("tile_encode", |b| {
        b.iter(|| {
            // TODO: Implement benchmark
        });
    });
}

criterion_group!(benches, bench_tile_encode);
criterion_main!(benches);
