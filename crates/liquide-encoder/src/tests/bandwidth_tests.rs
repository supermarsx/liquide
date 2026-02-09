use crate::bandwidth::*;

#[test]
fn estimator_records_frames() {
    let mut est = BandwidthEstimator::new(10, 60);
    assert_eq!(est.sample_count(), 0);

    est.record_frame(10_000);
    assert_eq!(est.sample_count(), 1);
    assert!(est.estimated_bandwidth_bps() > 0.0);
}

#[test]
fn estimator_smoothing() {
    let mut est = BandwidthEstimator::new(10, 60);
    est.set_alpha(0.5);

    // Record several identical frames
    for _ in 0..5 {
        est.record_frame(1000);
    }

    let bps = est.estimated_bandwidth_bps();
    // At 60fps, 1000 bytes/frame → ~60,000 bytes/sec
    assert!(bps > 50_000.0, "expected ~60K bps, got {bps}");
    assert!(bps < 70_000.0, "expected ~60K bps, got {bps}");
}

#[test]
fn estimator_average_and_peak() {
    let mut est = BandwidthEstimator::new(10, 60);
    est.record_frame(100);
    est.record_frame(200);
    est.record_frame(300);

    assert!((est.average_frame_size() - 200.0).abs() < 0.01);
    assert_eq!(est.peak_frame_size(), 300);
}

#[test]
fn estimator_rtt_tracking() {
    let mut est = BandwidthEstimator::new(10, 60);
    est.record_rtt(5000);
    est.record_rtt(7000);

    // Should be somewhere between 5000 and 7000
    let rtt = est.estimated_rtt_us();
    assert!(rtt > 4000.0 && rtt < 8000.0, "expected smoothed RTT: got {rtt}");
}

#[test]
fn estimator_window_eviction() {
    let mut est = BandwidthEstimator::new(3, 60);
    est.record_frame(100);
    est.record_frame(200);
    est.record_frame(300);
    est.record_frame(400); // should evict 100

    assert_eq!(est.sample_count(), 3);
    assert!((est.average_frame_size() - 300.0).abs() < 0.01);
}

#[test]
fn budget_from_estimator() {
    let mut est = BandwidthEstimator::new(10, 60);
    // Record a bandwidth estimate at a known level
    // 10000 bytes/frame * 60fps = 600,000 bytes/sec
    for _ in 0..10 {
        est.record_frame(10_000);
    }

    let budget = BandwidthBudget::from_estimator(&est, 0.1);
    // Budget should be ~90% of 10000 bytes/frame = ~9000
    let b = budget.budget_bytes();
    assert!(b > 8000, "budget should be ~9000, got {b}");
    assert!(b < 11000, "budget should be ~9000, got {b}");
}

#[test]
fn budget_should_degrade() {
    let budget = BandwidthBudget::new(10_000, 0.0);
    assert!(!budget.should_degrade(5_000));
    assert!(!budget.should_degrade(10_000));
    assert!(budget.should_degrade(10_001));
}

#[test]
fn budget_utilization() {
    let budget = BandwidthBudget::new(10_000, 0.0);
    let u = budget.utilization(5_000);
    assert!((u - 0.5).abs() < 0.001);

    let u2 = budget.utilization(15_000);
    assert!((u2 - 1.5).abs() < 0.001);
}

#[test]
fn budget_unlimited_never_degrades() {
    let budget = BandwidthBudget::unlimited();
    assert!(!budget.should_degrade(u64::MAX));
    assert!((budget.utilization(1_000_000)).abs() < 0.001);
}
