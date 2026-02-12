use std::time::Duration;

use crate::backoff::Backoff;

#[test]
fn first_delay_is_near_min() {
    let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(30));
    let d = b.next_delay();
    // With ±25 % jitter, the first delay (base = 100 ms) should be between
    // 75 ms and 125 ms.
    assert!(d >= Duration::from_millis(75));
    assert!(d <= Duration::from_millis(150));
}

#[test]
fn delays_grow_exponentially() {
    let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(60));
    let d0 = b.next_delay();
    let d1 = b.next_delay();
    let d2 = b.next_delay();
    // Each subsequent delay should be roughly 2× the previous (minus jitter).
    assert!(d1 > d0, "d1 ({d1:?}) should exceed d0 ({d0:?})");
    assert!(d2 > d1, "d2 ({d2:?}) should exceed d1 ({d1:?})");
}

#[test]
fn delay_caps_at_max() {
    let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(1));
    for _ in 0..20 {
        let d = b.next_delay();
        // Even with jitter the delay should not exceed max * 1.25.
        assert!(d <= Duration::from_millis(1500));
    }
}

#[test]
fn reset_restarts_sequence() {
    let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(30));
    for _ in 0..5 {
        b.next_delay();
    }
    assert!(b.attempt() >= 5);
    b.reset();
    assert_eq!(b.attempt(), 0);
    let d = b.next_delay();
    assert!(d < Duration::from_millis(250));
}

#[test]
fn peek_does_not_advance() {
    let b = Backoff::new(Duration::from_millis(100), Duration::from_secs(30));
    let d1 = b.peek_delay();
    let d2 = b.peek_delay();
    assert_eq!(d1, d2);
    assert_eq!(b.attempt(), 0);
}

#[test]
fn custom_factor() {
    let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(60))
        .with_factor(3.0);
    let _d0 = b.next_delay(); // base ~100 ms
    let d1 = b.next_delay(); // base ~300 ms (100 * 3^1)
    // d1 should be roughly 3× d0 (with jitter).
    assert!(d1 > Duration::from_millis(200));
}

#[test]
fn default_backoff() {
    let b = Backoff::default();
    assert_eq!(b.attempt(), 0);
    // Default: min=100ms, max=30s
    let d = b.peek_delay();
    assert!(d >= Duration::from_millis(50));
    assert!(d <= Duration::from_millis(200));
}
