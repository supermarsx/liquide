//! Tests for measurement, time series, and metrics.

use crate::measurement::{BenchMetrics, MetricSummary, TimeSeries};

// ===========================================================================
// TimeSeries basics
// ===========================================================================

#[test]
fn time_series_empty() {
    let ts = TimeSeries::new();
    assert_eq!(ts.count(), 0);
    assert!(ts.is_empty());
    assert!(ts.min().is_infinite());
    assert!(ts.max().is_infinite());
    assert!(ts.mean().is_nan());
    assert!(ts.std_dev().is_nan());
    assert!(ts.percentile(0.5).is_nan());
}

#[test]
fn time_series_single_sample() {
    let mut ts = TimeSeries::new();
    ts.record(1000, 42.0);
    assert_eq!(ts.count(), 1);
    assert!(!ts.is_empty());
    assert_eq!(ts.min(), 42.0);
    assert_eq!(ts.max(), 42.0);
    assert_eq!(ts.mean(), 42.0);
    assert_eq!(ts.std_dev(), 0.0);
    assert_eq!(ts.percentile(0.0), 42.0);
    assert_eq!(ts.percentile(0.5), 42.0);
    assert_eq!(ts.percentile(1.0), 42.0);
}

#[test]
fn time_series_multiple_samples() {
    let mut ts = TimeSeries::new();
    for i in 1..=10 {
        ts.record(i * 1000, i as f64);
    }
    assert_eq!(ts.count(), 10);
    assert_eq!(ts.min(), 1.0);
    assert_eq!(ts.max(), 10.0);
    assert!((ts.mean() - 5.5).abs() < 0.001);
}

#[test]
fn time_series_percentile_median() {
    let mut ts = TimeSeries::new();
    // Values: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10
    for i in 1..=10 {
        ts.record(i * 1000, i as f64);
    }
    let p50 = ts.percentile(0.5);
    // Nearest rank: 0.5 * 9 = 4.5 -> rounds to 5 -> value at index 5 = 6
    assert_eq!(p50, 6.0);
}

#[test]
fn time_series_percentile_p99() {
    let mut ts = TimeSeries::new();
    for i in 1..=100 {
        ts.record(i * 1000, i as f64);
    }
    let p99 = ts.percentile(0.99);
    // Nearest rank: 0.99 * 99 = 98.01 -> rounds to 98 -> value at index 98 = 99
    assert_eq!(p99, 99.0);
}

#[test]
fn time_series_percentile_p0_and_p100() {
    let mut ts = TimeSeries::new();
    for i in 1..=5 {
        ts.record(i * 1000, i as f64 * 10.0);
    }
    assert_eq!(ts.percentile(0.0), 10.0);
    assert_eq!(ts.percentile(1.0), 50.0);
}

#[test]
fn time_series_percentile_clamps() {
    let mut ts = TimeSeries::new();
    ts.record(0, 5.0);
    ts.record(1, 10.0);
    // Out-of-range percentiles should be clamped.
    assert_eq!(ts.percentile(-1.0), 5.0);
    assert_eq!(ts.percentile(2.0), 10.0);
}

#[test]
fn time_series_std_dev_uniform() {
    let mut ts = TimeSeries::new();
    // All the same value -> std_dev = 0.
    for i in 0..10 {
        ts.record(i * 1000, 7.0);
    }
    assert_eq!(ts.std_dev(), 0.0);
}

#[test]
fn time_series_std_dev_known_values() {
    let mut ts = TimeSeries::new();
    // Values: 2, 4, 4, 4, 5, 5, 7, 9
    // Mean = 5.0, variance = 4.0, std_dev = 2.0
    for (i, v) in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0].iter().enumerate() {
        ts.record(i as u64 * 1000, *v);
    }
    assert!((ts.mean() - 5.0).abs() < 0.001);
    assert!((ts.std_dev() - 2.0).abs() < 0.001);
}

#[test]
fn time_series_samples_access() {
    let mut ts = TimeSeries::new();
    ts.record(100, 1.0);
    ts.record(200, 2.0);
    let samples = ts.samples();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].timestamp_us, 100);
    assert_eq!(samples[0].value, 1.0);
    assert_eq!(samples[1].timestamp_us, 200);
    assert_eq!(samples[1].value, 2.0);
}

// ===========================================================================
// BenchMetrics
// ===========================================================================

#[test]
fn bench_metrics_empty() {
    let m = BenchMetrics::new();
    assert!(m.metric_names().is_empty());
    assert!(m.get("anything").is_none());
    assert!(m.summary().is_empty());
}

#[test]
fn bench_metrics_record_and_get() {
    let mut m = BenchMetrics::new();
    m.record("fps", 0, 60.0);
    m.record("fps", 1000, 59.0);
    m.record("latency", 0, 5.0);

    assert!(m.get("fps").is_some());
    assert_eq!(m.get("fps").unwrap().count(), 2);
    assert!(m.get("latency").is_some());
    assert_eq!(m.get("latency").unwrap().count(), 1);
    assert!(m.get("missing").is_none());
}

#[test]
fn bench_metrics_metric_names_sorted() {
    let mut m = BenchMetrics::new();
    m.record("zebra", 0, 1.0);
    m.record("alpha", 0, 2.0);
    m.record("middle", 0, 3.0);

    let names = m.metric_names();
    assert_eq!(names, vec!["alpha", "middle", "zebra"]);
}

#[test]
fn bench_metrics_summary() {
    let mut m = BenchMetrics::new();
    for i in 1..=10 {
        m.record("test_metric", i * 1000, i as f64);
    }

    let summaries = m.summary();
    assert_eq!(summaries.len(), 1);
    let s = &summaries[0];
    assert_eq!(s.name, "test_metric");
    assert_eq!(s.count, 10);
    assert_eq!(s.min, 1.0);
    assert_eq!(s.max, 10.0);
    assert!((s.mean - 5.5).abs() < 0.001);
}

#[test]
fn metric_summary_display() {
    let s = MetricSummary {
        name: "test".to_string(),
        count: 100,
        min: 1.0,
        max: 10.0,
        mean: 5.5,
        p50: 5.0,
        p95: 9.5,
        p99: 9.9,
        std_dev: 2.87,
    };
    let text = s.to_string();
    assert!(text.contains("test"));
    assert!(text.contains("n=100"));
    assert!(text.contains("min=1.00"));
    assert!(text.contains("max=10.00"));
}
