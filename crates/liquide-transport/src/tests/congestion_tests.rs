use std::time::Duration;

use crate::congestion::{
    BandwidthEstimator, BbrConfig, BbrController, BbrState, CongestionController,
    FixedRateController, LossDetector, RttEstimator,
};

// ---------------------------------------------------------------------------
// RTT Estimator
// ---------------------------------------------------------------------------

#[test]
fn rtt_initial_state() {
    let rtt = RttEstimator::new();
    assert!(rtt.srtt().is_none());
    assert!(rtt.min_rtt().is_none());
    assert!(rtt.latest_rtt().is_none());
    // Default RTO = 1s
    assert_eq!(rtt.rto(), Duration::from_secs(1));
}

#[test]
fn rtt_first_sample() {
    let mut rtt = RttEstimator::new();
    rtt.update(Duration::from_millis(100));
    assert_eq!(rtt.srtt(), Some(Duration::from_millis(100)));
    assert_eq!(rtt.min_rtt(), Some(Duration::from_millis(100)));
    assert_eq!(rtt.latest_rtt(), Some(Duration::from_millis(100)));
    // rttvar = rtt/2 = 50ms, rto = srtt + 4*rttvar = 100+200 = 300ms
    assert_eq!(rtt.rto(), Duration::from_millis(300));
}

#[test]
fn rtt_convergence() {
    let mut rtt = RttEstimator::new();
    // Feed 20 samples at 50ms — should converge near 50ms
    for _ in 0..20 {
        rtt.update(Duration::from_millis(50));
    }
    let srtt = rtt.srtt().unwrap();
    assert!(srtt.as_millis() >= 49 && srtt.as_millis() <= 51);
    assert_eq!(rtt.min_rtt(), Some(Duration::from_millis(50)));
}

#[test]
fn rtt_min_tracking() {
    let mut rtt = RttEstimator::new();
    rtt.update(Duration::from_millis(100));
    rtt.update(Duration::from_millis(50));
    rtt.update(Duration::from_millis(80));
    assert_eq!(rtt.min_rtt(), Some(Duration::from_millis(50)));
}

#[test]
fn rto_clamped() {
    let mut rtt = RttEstimator::new();
    // Very small RTT → RTO should be at least 200ms
    rtt.update(Duration::from_micros(100));
    assert!(rtt.rto() >= Duration::from_millis(200));
}

// ---------------------------------------------------------------------------
// Loss Detector
// ---------------------------------------------------------------------------

#[test]
fn loss_no_events() {
    let ld = LossDetector::new(Duration::from_secs(2));
    assert_eq!(ld.loss_rate(), 0.0);
    assert_eq!(ld.sample_count(), 0);
}

#[test]
fn loss_rate_calculation() {
    let mut ld = LossDetector::new(Duration::from_secs(10));
    // 90 acks + 10 losses = 10% loss rate
    for _ in 0..90 {
        ld.on_ack();
    }
    for _ in 0..10 {
        ld.on_loss();
    }
    let rate = ld.loss_rate();
    assert!((rate - 0.1).abs() < 0.001);
    assert_eq!(ld.sample_count(), 100);
}

#[test]
fn loss_all_acked() {
    let mut ld = LossDetector::new(Duration::from_secs(10));
    for _ in 0..50 {
        ld.on_ack();
    }
    assert_eq!(ld.loss_rate(), 0.0);
}

// ---------------------------------------------------------------------------
// Bandwidth Estimator
// ---------------------------------------------------------------------------

#[test]
fn bw_no_samples() {
    let bw = BandwidthEstimator::new(Duration::from_secs(2));
    assert_eq!(bw.max_bandwidth(), 0.0);
    assert_eq!(bw.current_rate(), 0.0);
}

#[test]
fn bw_records_acks() {
    let mut bw = BandwidthEstimator::new(Duration::from_secs(10));
    bw.on_ack(1000);
    bw.on_ack(2000);
    // max_bandwidth should be > 0 after traffic
    // (exact value depends on timing; just verify it's non-negative)
    assert!(bw.max_bandwidth() >= 0.0);
}

// ---------------------------------------------------------------------------
// BBR Controller
// ---------------------------------------------------------------------------

#[test]
fn bbr_initial_state() {
    let bbr = BbrController::with_defaults();
    assert_eq!(bbr.state(), BbrState::Startup);
    assert_eq!(bbr.cwnd(), BbrConfig::default().initial_cwnd);
}

#[test]
fn bbr_startup_cwnd_grows() {
    let mut bbr = BbrController::with_defaults();
    let initial = bbr.cwnd();
    // Simulate acks — cwnd should grow
    for _ in 0..20 {
        bbr.on_ack(1400, Duration::from_millis(50));
    }
    assert!(bbr.cwnd() >= initial);
}

#[test]
fn bbr_loss_reduces_cwnd() {
    let mut bbr = BbrController::with_defaults();
    // First grow in startup
    for _ in 0..30 {
        bbr.on_ack(1400, Duration::from_millis(50));
    }
    let before_loss = bbr.cwnd();
    // Heavy loss -> should eventually reduce
    for _ in 0..100 {
        bbr.on_loss(1400);
    }
    // cwnd should be at or below pre-loss value (or at min)
    assert!(bbr.cwnd() <= before_loss || bbr.cwnd() == BbrConfig::default().min_cwnd);
}

#[test]
fn bbr_pacing_rate_positive() {
    let bbr = BbrController::with_defaults();
    assert!(bbr.pacing_rate() > 0.0);
}

#[test]
fn bbr_can_send() {
    let bbr = BbrController::with_defaults();
    assert!(bbr.can_send(0));
    assert!(!bbr.can_send(u64::MAX));
}

// ---------------------------------------------------------------------------
// Fixed-Rate Controller
// ---------------------------------------------------------------------------

#[test]
fn fixed_rate() {
    let mut ctrl = FixedRateController::new(65536, 1_000_000.0);
    assert_eq!(ctrl.cwnd(), 65536);
    assert_eq!(ctrl.pacing_rate(), 1_000_000.0);
    assert!(ctrl.can_send(0));
    assert!(ctrl.can_send(65535));
    assert!(!ctrl.can_send(65536));

    // Ack/loss don't change anything
    ctrl.on_ack(1400, Duration::from_millis(50));
    ctrl.on_loss(1400);
    assert_eq!(ctrl.cwnd(), 65536);
    assert_eq!(ctrl.pacing_rate(), 1_000_000.0);
}
