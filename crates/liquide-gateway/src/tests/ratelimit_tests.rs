use crate::config::LimitsConfig;
use crate::ratelimit::RateLimiter;

fn make_limiter() -> RateLimiter {
    let config = LimitsConfig {
        per_ip_rate_per_sec: 10,
        auth_failure_ban_threshold: 3,
        auth_failure_window_sec: 60,
        ban_duration_sec: 300,
        ..LimitsConfig::default()
    };
    RateLimiter::new(config)
}

#[test]
fn test_rate_limit_allows_normal_traffic() {
    let mut limiter = make_limiter();
    // With 10 tokens/sec, the first 10 requests in second 0 should pass.
    for _ in 0..10 {
        assert!(limiter.check_rate("10.0.0.1", 1000).is_ok());
    }
}

#[test]
fn test_rate_limit_exceeds() {
    let mut limiter = make_limiter();
    // Drain all tokens at time 1000 (max is 20 with burst).
    for _ in 0..20 {
        let _ = limiter.check_rate("10.0.0.1", 1000);
    }
    // Next request at the same timestamp should fail.
    assert!(limiter.check_rate("10.0.0.1", 1000).is_err());
}

#[test]
fn test_rate_limit_refills() {
    let mut limiter = make_limiter();
    // Drain tokens.
    for _ in 0..20 {
        let _ = limiter.check_rate("10.0.0.1", 1000);
    }
    assert!(limiter.check_rate("10.0.0.1", 1000).is_err());

    // After 1 second, 10 new tokens should be available.
    assert!(limiter.check_rate("10.0.0.1", 1001).is_ok());
}

#[test]
fn test_auth_failure_ban() {
    let mut limiter = make_limiter();

    // First two failures should not trigger a ban.
    assert!(limiter.record_auth_failure("10.0.0.1", 1000).is_none());
    assert!(limiter.record_auth_failure("10.0.0.1", 1001).is_none());

    // Third failure should trigger a ban.
    let ban = limiter.record_auth_failure("10.0.0.1", 1002);
    assert!(ban.is_some());
    let ban = ban.unwrap();
    assert_eq!(ban.ip, "10.0.0.1");
    assert_eq!(ban.expires_at, 1002 + 300);

    // The IP should now be banned.
    assert!(limiter.is_banned("10.0.0.1", 1003));

    // Rate check should fail with IpBanned.
    assert!(limiter.check_rate("10.0.0.1", 1003).is_err());
}

#[test]
fn test_manual_ban_unban() {
    let mut limiter = make_limiter();
    assert!(!limiter.is_banned("10.0.0.1", 1000));

    limiter.ban_ip("10.0.0.1", "testing".to_string(), 1000, 60);
    assert!(limiter.is_banned("10.0.0.1", 1000));
    assert!(limiter.is_banned("10.0.0.1", 1059));
    assert!(!limiter.is_banned("10.0.0.1", 1060));

    limiter.unban_ip("10.0.0.1");
    assert!(!limiter.is_banned("10.0.0.1", 1000));
}

#[test]
fn test_cleanup_expired_bans() {
    let mut limiter = make_limiter();
    limiter.ban_ip("10.0.0.1", "test".to_string(), 1000, 60);
    limiter.ban_ip("10.0.0.2", "test".to_string(), 1000, 120);

    // At time 1070, the first ban has expired but the second hasn't.
    limiter.cleanup_expired(1070);
    assert!(!limiter.is_banned("10.0.0.1", 1070));
    assert!(limiter.is_banned("10.0.0.2", 1070));
}

#[test]
fn test_active_bans() {
    let mut limiter = make_limiter();
    limiter.ban_ip("10.0.0.1", "a".to_string(), 1000, 60);
    limiter.ban_ip("10.0.0.2", "b".to_string(), 1000, 60);

    let bans = limiter.active_bans();
    assert_eq!(bans.len(), 2);
}
