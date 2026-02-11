use crate::metrics::EncoderMetrics;

#[test]
fn new_metrics_are_zero() {
    let m = EncoderMetrics::new();
    let snap = m.snapshot();
    assert_eq!(snap.active_sessions, 0);
    assert_eq!(snap.queue_depth, 0);
    assert_eq!(snap.avg_encode_time_us, 0);
    assert_eq!(snap.fallback_total, 0);
    assert_eq!(snap.errors_total, 0);
}

#[test]
fn record_encode_updates_average() {
    let mut m = EncoderMetrics::new();
    m.record_encode(100);
    m.record_encode(200);
    m.record_encode(300);
    let snap = m.snapshot();
    assert_eq!(snap.avg_encode_time_us, 200);
}

#[test]
fn record_fallback_and_error() {
    let mut m = EncoderMetrics::new();
    m.record_fallback();
    m.record_fallback();
    m.record_error();
    let snap = m.snapshot();
    assert_eq!(snap.fallback_total, 2);
    assert_eq!(snap.errors_total, 1);
}

#[test]
fn set_gauges() {
    let mut m = EncoderMetrics::new();
    m.set_active_sessions(4);
    m.set_queue_depth(12);
    let snap = m.snapshot();
    assert_eq!(snap.active_sessions, 4);
    assert_eq!(snap.queue_depth, 12);
}
