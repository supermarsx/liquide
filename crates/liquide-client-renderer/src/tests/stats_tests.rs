use crate::stats::RenderStats;

#[test]
fn test_new_stats() {
    let s = RenderStats::new();
    assert_eq!(s.frames_rendered, 0);
    assert_eq!(s.tiles_decoded, 0);
    assert_eq!(s.tiles_skipped, 0);
    assert_eq!(s.bytes_received, 0);
    assert_eq!(s.bytes_decompressed, 0);
    assert_eq!(s.total_decode_time_us, 0);
    assert_eq!(s.last_frame_time_us, 0);
}

#[test]
fn test_record_frame() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 1000, 5000, 200);
    assert_eq!(s.frames_rendered, 1);
    assert_eq!(s.tiles_decoded, 10);
    assert_eq!(s.tiles_skipped, 5);
    assert_eq!(s.bytes_received, 1000);
    assert_eq!(s.bytes_decompressed, 5000);
    assert_eq!(s.total_decode_time_us, 200);
    assert_eq!(s.last_frame_time_us, 200);
}

#[test]
fn test_multiple_frames() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 1000, 5000, 200);
    s.record_frame(8, 7, 800, 4000, 150);
    assert_eq!(s.frames_rendered, 2);
    assert_eq!(s.tiles_decoded, 18);
    assert_eq!(s.tiles_skipped, 12);
    assert_eq!(s.bytes_received, 1800);
    assert_eq!(s.bytes_decompressed, 9000);
    assert_eq!(s.total_decode_time_us, 350);
    assert_eq!(s.last_frame_time_us, 150);
}

#[test]
fn test_avg_decode_time() {
    let mut s = RenderStats::new();
    assert_eq!(s.avg_decode_time_us(), 0);
    s.record_frame(1, 0, 100, 400, 100);
    s.record_frame(1, 0, 100, 400, 300);
    assert_eq!(s.avg_decode_time_us(), 200);
}

#[test]
fn test_avg_tiles_per_frame() {
    let mut s = RenderStats::new();
    assert_eq!(s.avg_tiles_per_frame(), 0.0);
    s.record_frame(10, 0, 0, 0, 0);
    s.record_frame(20, 0, 0, 0, 0);
    assert!((s.avg_tiles_per_frame() - 15.0).abs() < f64::EPSILON);
}

#[test]
fn test_compression_ratio() {
    let mut s = RenderStats::new();
    assert_eq!(s.compression_ratio(), 0.0);
    s.record_frame(1, 0, 250, 1000, 0);
    assert!((s.compression_ratio() - 0.25).abs() < f64::EPSILON);
}

#[test]
fn test_total_tiles() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 0, 0, 0);
    assert_eq!(s.total_tiles(), 15);
}

#[test]
fn test_skip_ratio() {
    let mut s = RenderStats::new();
    assert_eq!(s.skip_ratio(), 0.0);
    s.record_frame(5, 15, 0, 0, 0);
    assert!((s.skip_ratio() - 0.75).abs() < f64::EPSILON);
}

#[test]
fn test_reset() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 1000, 5000, 200);
    s.reset();
    assert_eq!(s.frames_rendered, 0);
    assert_eq!(s.tiles_decoded, 0);
}

#[test]
fn test_default() {
    let s = RenderStats::default();
    assert_eq!(s.frames_rendered, 0);
}

#[test]
fn test_serde_roundtrip() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 1000, 5000, 200);
    let json = serde_json::to_string(&s).unwrap();
    let deserialized: RenderStats = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.frames_rendered, 1);
    assert_eq!(deserialized.tiles_decoded, 10);
    assert_eq!(deserialized.bytes_received, 1000);
}

#[test]
fn test_display() {
    let mut s = RenderStats::new();
    s.record_frame(10, 5, 1000, 5000, 200);
    let display = format!("{s}");
    assert!(display.contains("frames=1"));
    assert!(display.contains("10"));
}
