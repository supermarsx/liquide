use criterion::{criterion_group, criterion_main, Criterion};

fn bench_compositor_render(c: &mut Criterion) {
    c.bench_function("compositor_render", |b| {
        b.iter(|| {
            // TODO: Implement benchmark
        });
    });
}

criterion_group!(benches, bench_compositor_render);
criterion_main!(benches);
