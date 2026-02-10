use criterion::{criterion_group, criterion_main, Criterion};

use liquide_recording::format::RecordingHeader;
use liquide_recording::muxer::RecordingMuxer;
use liquide_recording::retention::{RecordingEntry, RetentionPolicy};
use liquide_recording::segment::VideoSegment;

fn bench_mux_10000_video_segments(c: &mut Criterion) {
    c.bench_function("mux_10000_video_segments", |b| {
        b.iter(|| {
            let header = RecordingHeader::new(1920, 1080, 64, "bgra8");
            let mut muxer = RecordingMuxer::new(header);
            muxer.start().unwrap();
            for i in 0..10_000u64 {
                let vs = VideoSegment::new(i * 16667, vec![0xAA; 256], 4);
                muxer.write_video(&vs).unwrap();
            }
            muxer.stop().unwrap();
        });
    });
}

fn bench_retention_enforce_1000_entries(c: &mut Criterion) {
    let policy = RetentionPolicy {
        max_age_hours: Some(24),
        max_size_bytes: Some(500_000),
        max_recordings: Some(100),
    };
    let entries: Vec<RecordingEntry> = (0..1000)
        .map(|i| RecordingEntry::new(&format!("rec_{i}"), i * 1_000_000, 1000))
        .collect();
    let now_us = 999_000_000u64;

    c.bench_function("retention_enforce_1000_entries", |b| {
        b.iter(|| {
            let _ = policy.enforce(&entries, now_us);
        });
    });
}

criterion_group!(
    benches,
    bench_mux_10000_video_segments,
    bench_retention_enforce_1000_entries
);
criterion_main!(benches);
