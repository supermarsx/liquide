use crate::config::HealthCheckConfig;
use crate::health::HealthChecker;

fn make_checker() -> HealthChecker {
    HealthChecker::new(HealthCheckConfig {
        interval_sec: 10,
        unhealthy_threshold: 3,
        timeout_sec: 5,
    })
}

#[test]
fn test_health_initially_not_healthy() {
    let checker = make_checker();
    // No data yet: server is not considered healthy.
    assert!(!checker.is_healthy("srv-1"));
}

#[test]
fn test_health_successful_check() {
    let mut checker = make_checker();
    checker.record_check("srv-1", true, Some(5), 1000);
    assert!(checker.is_healthy("srv-1"));

    let status = checker.status("srv-1").unwrap();
    assert!(status.healthy);
    assert_eq!(status.response_time_ms, Some(5));
    assert_eq!(status.consecutive_failures, 0);
}

#[test]
fn test_health_threshold_breach() {
    let mut checker = make_checker();

    // Initial success.
    checker.record_check("srv-1", true, Some(5), 1000);
    assert!(checker.is_healthy("srv-1"));

    // Two failures: should still be healthy.
    checker.record_check("srv-1", false, None, 1010);
    assert!(checker.is_healthy("srv-1"));
    checker.record_check("srv-1", false, None, 1020);
    assert!(checker.is_healthy("srv-1"));

    // Third failure: crosses the threshold.
    checker.record_check("srv-1", false, None, 1030);
    assert!(!checker.is_healthy("srv-1"));
}

#[test]
fn test_health_recovery() {
    let mut checker = make_checker();

    // Fail three times.
    checker.record_check("srv-1", false, None, 1000);
    checker.record_check("srv-1", false, None, 1010);
    checker.record_check("srv-1", false, None, 1020);
    assert!(!checker.is_healthy("srv-1"));

    // A single success should recover.
    checker.record_check("srv-1", true, Some(3), 1030);
    assert!(checker.is_healthy("srv-1"));
}

#[test]
fn test_unhealthy_servers_list() {
    let mut checker = make_checker();

    checker.record_check("srv-1", true, Some(5), 1000);
    checker.record_check("srv-2", false, None, 1000);
    checker.record_check("srv-2", false, None, 1010);
    checker.record_check("srv-2", false, None, 1020);

    let unhealthy = checker.unhealthy_servers();
    assert_eq!(unhealthy.len(), 1);
    assert!(unhealthy.contains(&"srv-2".to_string()));
}

#[test]
fn test_all_statuses() {
    let mut checker = make_checker();
    checker.record_check("srv-1", true, Some(5), 1000);
    checker.record_check("srv-2", true, Some(8), 1000);

    let statuses = checker.all_statuses();
    assert_eq!(statuses.len(), 2);
}
