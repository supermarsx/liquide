//! Tests for `aggregator` module types.

use liquide_apps_task_manager::aggregator::*;

// ---------------------------------------------------------------------------
// RingBuffer – basic operations
// ---------------------------------------------------------------------------

#[test]
fn ring_buffer_new_is_empty() {
    let rb: RingBuffer<f64> = RingBuffer::new(10);
    assert_eq!(rb.len(), 0);
    assert!(rb.is_empty());
    assert_eq!(rb.capacity(), 10);
}

#[test]
fn ring_buffer_push_and_len() {
    let mut rb = RingBuffer::new(4);
    rb.push(1.0);
    rb.push(2.0);
    rb.push(3.0);
    assert_eq!(rb.len(), 3);
    assert!(!rb.is_empty());
}

#[test]
fn ring_buffer_wraps_at_capacity() {
    let mut rb = RingBuffer::new(3);
    rb.push(1.0);
    rb.push(2.0);
    rb.push(3.0);
    rb.push(4.0); // wraps, evicts 1.0
    assert_eq!(rb.len(), 3);
    let items: Vec<f64> = rb.iter().copied().collect();
    assert_eq!(items, vec![2.0, 3.0, 4.0]);
}

#[test]
fn ring_buffer_last() {
    let mut rb = RingBuffer::new(5);
    assert!(rb.last().is_none());
    rb.push(10.0);
    assert_eq!(*rb.last().unwrap(), 10.0);
    rb.push(20.0);
    assert_eq!(*rb.last().unwrap(), 20.0);
}

#[test]
fn ring_buffer_clear() {
    let mut rb = RingBuffer::new(5);
    rb.push(1.0);
    rb.push(2.0);
    rb.clear();
    assert!(rb.is_empty());
    assert_eq!(rb.len(), 0);
}

#[test]
fn ring_buffer_iter_order() {
    let mut rb = RingBuffer::new(5);
    for i in 0..5 {
        rb.push(i as f64);
    }
    let items: Vec<f64> = rb.iter().copied().collect();
    assert_eq!(items, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn ring_buffer_iter_after_wrap() {
    let mut rb = RingBuffer::new(3);
    for i in 0..6 {
        rb.push(i as f64);
    }
    // Should contain [3, 4, 5]
    let items: Vec<f64> = rb.iter().copied().collect();
    assert_eq!(items, vec![3.0, 4.0, 5.0]);
}

// ---------------------------------------------------------------------------
// Sample
// ---------------------------------------------------------------------------

#[test]
fn sample_construction() {
    let s = Sample {
        timestamp_ms: 1000,
        value: 42.5,
    };
    assert_eq!(s.timestamp_ms, 1000);
    assert_eq!(s.value, 42.5);
}

// ---------------------------------------------------------------------------
// TimeSeries
// ---------------------------------------------------------------------------

#[test]
fn time_series_new_is_empty() {
    let ts = TimeSeries::new(100);
    assert_eq!(ts.len(), 0);
    assert!(ts.is_empty());
}

#[test]
fn time_series_push_and_len() {
    let mut ts = TimeSeries::new(100);
    ts.push(1000, 10.0);
    ts.push(2000, 20.0);
    ts.push(3000, 30.0);
    assert_eq!(ts.len(), 3);
}

#[test]
fn time_series_average() {
    let mut ts = TimeSeries::new(100);
    ts.push(1000, 10.0);
    ts.push(2000, 20.0);
    ts.push(3000, 30.0);
    assert!((ts.average() - 20.0).abs() < f64::EPSILON);
}

#[test]
fn time_series_min() {
    let mut ts = TimeSeries::new(100);
    ts.push(1000, 10.0);
    ts.push(2000, 5.0);
    ts.push(3000, 15.0);
    assert!((ts.min() - 5.0).abs() < f64::EPSILON);
}

#[test]
fn time_series_max() {
    let mut ts = TimeSeries::new(100);
    ts.push(1000, 10.0);
    ts.push(2000, 5.0);
    ts.push(3000, 15.0);
    assert!((ts.max() - 15.0).abs() < f64::EPSILON);
}

#[test]
fn time_series_empty_stats() {
    let ts = TimeSeries::new(100);
    assert!(ts.average().is_nan() || ts.average() == 0.0);
    assert!(ts.min().is_infinite() || ts.min() == 0.0);
}

#[test]
fn time_series_last() {
    let mut ts = TimeSeries::new(100);
    assert!(ts.last().is_none());
    ts.push(1000, 42.0);
    let last = ts.last().unwrap();
    assert_eq!(last.value, 42.0);
}

// ---------------------------------------------------------------------------
// Aggregator
// ---------------------------------------------------------------------------

#[test]
fn aggregator_new() {
    let agg = Aggregator::new(300);
    assert!(agg.get("cpu_total").is_none());
}

#[test]
fn aggregator_push_and_get() {
    let mut agg = Aggregator::new(100);
    agg.push("cpu_total", 1000, 55.0);
    agg.push("cpu_total", 2000, 60.0);
    let ts = agg.get("cpu_total").unwrap();
    assert_eq!(ts.len(), 2);
}

#[test]
fn aggregator_multiple_series() {
    let mut agg = Aggregator::new(100);
    agg.push("cpu_total", 1000, 55.0);
    agg.push("mem_percent", 1000, 70.0);
    assert!(agg.get("cpu_total").is_some());
    assert!(agg.get("mem_percent").is_some());
    assert!(agg.get("nonexistent").is_none());
}

#[test]
fn aggregator_clear() {
    let mut agg = Aggregator::new(100);
    agg.push("cpu_total", 1000, 55.0);
    agg.clear();
    assert!(agg.get("cpu_total").is_none());
}
