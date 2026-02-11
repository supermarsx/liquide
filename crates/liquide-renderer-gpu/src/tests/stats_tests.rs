use crate::stats::*;

#[test]
fn new_collector_has_zero_stats() {
    let collector = StatsCollector::new();
    let summary = collector.summary();

    assert_eq!(summary.frames_rendered, 0);
    assert_eq!(summary.avg_composite_us, 0.0);
    assert_eq!(summary.avg_total_us, 0.0);
    assert_eq!(summary.peak_vram_mb, 0);
    assert_eq!(summary.device_lost_count, 0);
    assert_eq!(summary.fallback_count, 0);
}

#[test]
fn record_single_frame() {
    let mut collector = StatsCollector::new();

    collector.record_frame(GpuFrameStats {
        composite_time_us: 500,
        blur_time_us: 200,
        total_time_us: 800,
        vram_used_mb: 64,
        frame_id: 1,
    });

    let summary = collector.summary();
    assert_eq!(summary.frames_rendered, 1);
    assert!((summary.avg_composite_us - 500.0).abs() < f64::EPSILON);
    assert!((summary.avg_total_us - 800.0).abs() < f64::EPSILON);
    assert_eq!(summary.peak_vram_mb, 64);
}

#[test]
fn record_multiple_frames_averages() {
    let mut collector = StatsCollector::new();

    collector.record_frame(GpuFrameStats {
        composite_time_us: 400,
        blur_time_us: 100,
        total_time_us: 600,
        vram_used_mb: 50,
        frame_id: 1,
    });

    collector.record_frame(GpuFrameStats {
        composite_time_us: 600,
        blur_time_us: 300,
        total_time_us: 1000,
        vram_used_mb: 100,
        frame_id: 2,
    });

    let summary = collector.summary();
    assert_eq!(summary.frames_rendered, 2);
    assert!((summary.avg_composite_us - 500.0).abs() < f64::EPSILON);
    assert!((summary.avg_total_us - 800.0).abs() < f64::EPSILON);
    assert_eq!(summary.peak_vram_mb, 100);
}

#[test]
fn peak_vram_tracks_maximum() {
    let mut collector = StatsCollector::new();

    collector.record_frame(GpuFrameStats {
        composite_time_us: 0,
        blur_time_us: 0,
        total_time_us: 0,
        vram_used_mb: 200,
        frame_id: 1,
    });

    collector.record_frame(GpuFrameStats {
        composite_time_us: 0,
        blur_time_us: 0,
        total_time_us: 0,
        vram_used_mb: 50,
        frame_id: 2,
    });

    assert_eq!(collector.summary().peak_vram_mb, 200);
}

#[test]
fn record_device_lost_increments_count() {
    let mut collector = StatsCollector::new();
    assert_eq!(collector.summary().device_lost_count, 0);

    collector.record_device_lost();
    assert_eq!(collector.summary().device_lost_count, 1);

    collector.record_device_lost();
    assert_eq!(collector.summary().device_lost_count, 2);
}

#[test]
fn record_fallback_increments_count() {
    let mut collector = StatsCollector::new();
    assert_eq!(collector.summary().fallback_count, 0);

    collector.record_fallback();
    collector.record_fallback();
    collector.record_fallback();
    assert_eq!(collector.summary().fallback_count, 3);
}

#[test]
fn reset_clears_everything() {
    let mut collector = StatsCollector::new();

    collector.record_frame(GpuFrameStats {
        composite_time_us: 500,
        blur_time_us: 200,
        total_time_us: 800,
        vram_used_mb: 128,
        frame_id: 1,
    });
    collector.record_device_lost();
    collector.record_fallback();

    collector.reset();

    let summary = collector.summary();
    assert_eq!(summary.frames_rendered, 0);
    assert_eq!(summary.avg_composite_us, 0.0);
    assert_eq!(summary.peak_vram_mb, 0);
    assert_eq!(summary.device_lost_count, 0);
    assert_eq!(summary.fallback_count, 0);
}

#[test]
fn default_collector() {
    let collector = StatsCollector::default();
    assert_eq!(collector.summary().frames_rendered, 0);
}
