//! Tests for metrics collection.

use crate::metrics::{MetricsCollector, MetricsSnapshot};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_snapshot(ts: u64, sessions: u32) -> MetricsSnapshot {
    MetricsSnapshot {
        total_sessions: sessions,
        total_users: sessions / 2,
        avg_fps: 60.0,
        avg_latency_ms: 5.0,
        total_bandwidth_in_bps: 1_000_000,
        total_bandwidth_out_bps: 500_000,
        timestamp: ts,
    }
}

// ===========================================================================
// Snapshots
// ===========================================================================

#[test]
fn test_new_collector_is_empty() {
    let coll = MetricsCollector::new(100);
    assert_eq!(coll.snapshot_count(), 0);
    assert!(coll.latest_snapshot().is_none());
}

#[test]
fn test_record_snapshot() {
    let mut coll = MetricsCollector::new(100);
    coll.record_snapshot(make_snapshot(1000, 10));
    assert_eq!(coll.snapshot_count(), 1);
    let latest = coll.latest_snapshot().unwrap();
    assert_eq!(latest.total_sessions, 10);
    assert_eq!(latest.timestamp, 1000);
}

#[test]
fn test_snapshot_retention() {
    let mut coll = MetricsCollector::new(3);
    for i in 0..5 {
        coll.record_snapshot(make_snapshot(i, i as u32));
    }
    assert_eq!(coll.snapshot_count(), 3);
    // oldest should be dropped
    let snapshots = coll.snapshots();
    assert_eq!(snapshots[0].timestamp, 2);
    assert_eq!(snapshots[2].timestamp, 4);
}

// ===========================================================================
// Time series
// ===========================================================================

#[test]
fn test_record_point() {
    let mut coll = MetricsCollector::new(100);
    coll.record_point("cpu", "srv-a", 1000, 45.5);
    coll.record_point("cpu", "srv-a", 1005, 50.0);
    assert_eq!(coll.series_count(), 1);
    let series = coll.get_series("cpu", "srv-a").unwrap();
    assert_eq!(series.points.len(), 2);
    assert!((series.points[0].value - 45.5).abs() < f64::EPSILON);
    assert!((series.points[1].value - 50.0).abs() < f64::EPSILON);
}

#[test]
fn test_separate_series_per_server() {
    let mut coll = MetricsCollector::new(100);
    coll.record_point("cpu", "srv-a", 1000, 45.0);
    coll.record_point("cpu", "srv-b", 1000, 60.0);
    assert_eq!(coll.series_count(), 2);
    assert!(coll.get_series("cpu", "srv-a").is_some());
    assert!(coll.get_series("cpu", "srv-b").is_some());
}

#[test]
fn test_series_retention() {
    let mut coll = MetricsCollector::new(3);
    for i in 0..5 {
        coll.record_point("cpu", "srv-a", i, i as f64);
    }
    let series = coll.get_series("cpu", "srv-a").unwrap();
    assert_eq!(series.points.len(), 3);
    assert!((series.points[0].value - 2.0).abs() < f64::EPSILON);
}

#[test]
fn test_get_unknown_series() {
    let coll = MetricsCollector::new(100);
    assert!(coll.get_series("cpu", "srv-a").is_none());
}

// ===========================================================================
// Default collector
// ===========================================================================

#[test]
fn test_default_collector() {
    let coll = MetricsCollector::default();
    assert_eq!(coll.snapshot_count(), 0);
    assert_eq!(coll.series_count(), 0);
}

// ===========================================================================
// Serde
// ===========================================================================

#[test]
fn test_snapshot_serde() {
    let snap = make_snapshot(1000, 10);
    let json = serde_json::to_string(&snap).unwrap();
    let parsed: MetricsSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_sessions, 10);
    assert_eq!(parsed.timestamp, 1000);
}
