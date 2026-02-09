use criterion::{criterion_group, criterion_main, Criterion};

fn bench_blur_simd(c: &mut Criterion) {
    c.bench_function("blur_simd", |b| {
        b.iter(|| {
            // TODO: Implement benchmark
        });
    });
}

criterion_group!(benches, bench_blur_simd);
criterion_main!(benches);
