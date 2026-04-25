use crate::bandwidth::*;

#[test]
fn t16_encoder_budget_from_cold_estimator_starts_unlimited() {
    let est = BandwidthEstimator::new(10, 60);

    let budget = BandwidthBudget::from_estimator(&est, 0.1);

    assert!(budget.is_unlimited());
    assert_eq!(budget.pressure(), BudgetPressure::Warmup);
    assert!(!budget.under_pressure());
}

#[test]
fn t16_encoder_estimator_warm_up_seeds_budget() {
    let mut est = BandwidthEstimator::new(10, 60);
    est.warm_up(12_000, 4);

    let budget = est.frame_budget(0.1);

    assert_eq!(est.sample_count(), 4);
    assert!(!budget.is_unlimited());
    assert_eq!(budget.pressure(), BudgetPressure::Nominal);
    assert!(budget.budget_bytes() > 10_000);
    assert!(budget.budget_bytes() < 12_000);
}

#[test]
fn t16_encoder_budget_pressure_toggles_with_observed_usage() {
    let mut budget = BandwidthBudget::new(10_000, 0.1);

    assert_eq!(budget.pressure(), BudgetPressure::Nominal);
    assert_eq!(budget.observe(12_000), BudgetPressure::Pressured);
    assert!(budget.under_pressure());
    assert_eq!(budget.observe(8_000), BudgetPressure::Nominal);
    assert!(!budget.under_pressure());
}

#[test]
fn t16_encoder_unlimited_budget_never_enters_pressure() {
    let mut budget = BandwidthBudget::unlimited();

    assert_eq!(budget.observe(u64::MAX), BudgetPressure::Nominal);
    assert!(!budget.under_pressure());
    assert!(!budget.should_degrade(u64::MAX));
}

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
    assert!(
        rtt > 4000.0 && rtt < 8000.0,
        "expected smoothed RTT: got {rtt}"
    );
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

#[test]
fn estimator_zero_bandwidth() {
    let est = BandwidthEstimator::new(10, 60);
    assert_eq!(est.estimated_bandwidth_bps(), 0.0);
    assert_eq!(est.sample_count(), 0);
    assert_eq!(est.average_frame_size(), 0.0);
    assert_eq!(est.peak_frame_size(), 0);
}

#[test]
fn estimator_set_alpha() {
    let mut est = BandwidthEstimator::new(10, 60);

    // Record with default alpha (0.3)
    est.record_frame(10_000);
    let bps_with_default = est.estimated_bandwidth_bps();

    // Create a fresh estimator with alpha = 1.0 (full weight on latest sample)
    let mut est2 = BandwidthEstimator::new(10, 60);
    est2.set_alpha(1.0);
    est2.record_frame(10_000);
    let bps_with_max_alpha = est2.estimated_bandwidth_bps();

    // Both should reflect the same single reading, since it's the first sample
    // and the first sample initializes EMA directly
    assert!(bps_with_default > 0.0);
    assert!(bps_with_max_alpha > 0.0);

    // Now record a second, different sample to see the alpha effect
    est.record_frame(20_000);
    est2.record_frame(20_000);

    // With alpha=1.0, the estimate should match the latest sample exactly.
    // frame_interval_us = 1_000_000 / 60 = 16666 (integer division),
    // so effective fps = 1_000_000 / 16666 ~= 60.0024
    let effective_fps = 1_000_000.0 / 16_666.0;
    let expected = 20_000.0 * effective_fps;
    assert!(
        (est2.estimated_bandwidth_bps() - expected).abs() < 1.0,
        "alpha=1.0 should track latest sample exactly, got {}",
        est2.estimated_bandwidth_bps()
    );
}

#[test]
fn budget_new_direct() {
    let budget = BandwidthBudget::new(5000, 0.2);
    assert_eq!(budget.budget_bytes(), 5000);
    assert!((budget.safety_margin() - 0.2).abs() < 0.001);
}

#[test]
fn budget_zero_budget_always_degrades() {
    let budget = BandwidthBudget::new(0, 0.0);
    assert_eq!(budget.budget_bytes(), 0);
    // Any positive batch size should recommend degradation
    assert!(budget.should_degrade(1));
    assert!(budget.should_degrade(100));
    assert!(budget.should_degrade(1_000_000));
    // Zero batch should not degrade (0 is not > 0)
    assert!(!budget.should_degrade(0));
}

#[test]
fn estimator_reset_clears_all_state() {
    let mut est = BandwidthEstimator::new(10, 60);

    // Record some data
    est.record_frame(10_000);
    est.record_frame(20_000);
    est.record_rtt(5000);
    assert_eq!(est.sample_count(), 2);
    assert!(est.estimated_bandwidth_bps() > 0.0);
    assert!(est.estimated_rtt_us() > 0.0);

    // Reset
    est.reset();

    // Everything should be zeroed
    assert_eq!(est.sample_count(), 0);
    assert_eq!(est.estimated_bandwidth_bps(), 0.0);
    assert_eq!(est.estimated_rtt_us(), 0.0);
    assert_eq!(est.average_frame_size(), 0.0);
    assert_eq!(est.peak_frame_size(), 0);
}

#[test]
fn estimator_reset_allows_reuse() {
    let mut est = BandwidthEstimator::new(10, 60);

    est.record_frame(5000);
    est.reset();

    // Record new data after reset
    est.record_frame(1000);
    assert_eq!(est.sample_count(), 1);
    assert!((est.average_frame_size() - 1000.0).abs() < 0.01);
    // First sample after reset initializes EMA directly
    let fps = 1_000_000.0 / 16_666.0;
    let expected_bps = 1000.0 * fps;
    assert!(
        (est.estimated_bandwidth_bps() - expected_bps).abs() < 1.0,
        "post-reset bandwidth should reflect new data only"
    );
}
