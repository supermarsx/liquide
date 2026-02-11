use crate::bandwidth::BandwidthLimiter;

#[test]
fn test_bandwidth_limiter_new() {
    let limiter = BandwidthLimiter::new(50);
    assert!(limiter.available_bytes() > 0);
}

#[test]
fn test_bandwidth_limiter_consume() {
    let mut limiter = BandwidthLimiter::new(50);
    // 50 Mbps = ~6.25 MB/s capacity; consuming 1000 bytes should succeed
    assert!(limiter.try_consume(1000));
}

#[test]
fn test_bandwidth_limiter_exhaust() {
    let mut limiter = BandwidthLimiter::new(1); // 1 Mbps = 125,000 bytes/s
    // Try to consume more than the bucket capacity
    let huge = 200_000;
    let result = limiter.try_consume(huge);
    assert!(!result);
}

#[test]
fn test_bandwidth_limiter_reset() {
    let mut limiter = BandwidthLimiter::new(10);
    // Consume some tokens
    limiter.try_consume(500_000);
    // Reset refills the bucket
    limiter.reset();
    assert!(limiter.available_bytes() > 0);
}

#[test]
fn test_bandwidth_limiter_available() {
    let limiter = BandwidthLimiter::new(100);
    // 100 Mbps = 12,500,000 bytes/s capacity
    let avail = limiter.available_bytes();
    assert!(avail > 0);
}
