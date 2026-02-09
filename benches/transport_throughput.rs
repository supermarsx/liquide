use criterion::{criterion_group, criterion_main, Criterion};

fn bench_transport_throughput(c: &mut Criterion) {
    c.bench_function("transport_throughput", |b| {
        b.iter(|| {
            // TODO: Implement benchmark
        });
    });
}

criterion_group!(benches, bench_transport_throughput);
criterion_main!(benches);
