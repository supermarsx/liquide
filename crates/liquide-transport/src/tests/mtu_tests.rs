use crate::mtu::{
    MtuConfig, MtuDiscoverer, ProbeState, MAX_MTU_DEFAULT, MIN_MTU_IPV4, MIN_MTU_IPV6, SAFE_MTU,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn mtu_constants() {
    assert_eq!(MIN_MTU_IPV4, 576);
    assert_eq!(MIN_MTU_IPV6, 1280);
    assert_eq!(SAFE_MTU, 1280);
    assert_eq!(MAX_MTU_DEFAULT, 9000);
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[test]
fn config_ipv4() {
    let cfg = MtuConfig::ipv4();
    assert_eq!(cfg.min_mtu, MIN_MTU_IPV4);
    assert_eq!(cfg.max_mtu, MAX_MTU_DEFAULT);
}

#[test]
fn config_ipv6() {
    let cfg = MtuConfig::ipv6();
    assert_eq!(cfg.min_mtu, MIN_MTU_IPV6);
    assert_eq!(cfg.max_mtu, MAX_MTU_DEFAULT);
}

#[test]
fn config_default_is_ipv4() {
    let def = MtuConfig::default();
    let ipv4 = MtuConfig::ipv4();
    assert_eq!(def.min_mtu, ipv4.min_mtu);
    assert_eq!(def.max_mtu, ipv4.max_mtu);
}

// ---------------------------------------------------------------------------
// Initial State
// ---------------------------------------------------------------------------

#[test]
fn initial_state() {
    let disc = MtuDiscoverer::with_defaults();
    assert_eq!(disc.state(), ProbeState::Initial);
    assert_eq!(disc.mtu(), MIN_MTU_IPV4);
    assert!(!disc.has_override());
}

#[test]
fn initial_state_ipv6() {
    let disc = MtuDiscoverer::new(MtuConfig::ipv6());
    assert_eq!(disc.mtu(), MIN_MTU_IPV6);
}

// ---------------------------------------------------------------------------
// Override
// ---------------------------------------------------------------------------

#[test]
fn override_mtu() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(1500);
    assert!(disc.has_override());
    assert_eq!(disc.mtu(), 1500);

    // Override suppresses probing
    assert!(disc.next_probe().is_none());

    disc.clear_override();
    assert!(!disc.has_override());
    assert_eq!(disc.mtu(), MIN_MTU_IPV4);
}

// ---------------------------------------------------------------------------
// Binary Search Probing
// ---------------------------------------------------------------------------

#[test]
fn probing_converges() {
    // Use a small range so we can drive it to completion
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 1500,
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // Simulate: actual MTU is 1200
    let actual_mtu = 1200;
    let mut iterations = 0;

    loop {
        let probe = disc.next_probe();
        if probe.is_none() {
            break;
        }
        let probe = probe.unwrap();
        iterations += 1;

        // Succeed if probe fits, fail otherwise
        disc.on_probe_result(probe.size, probe.size <= actual_mtu);

        // Safety: binary search on 1000-element range should converge in ~10 steps
        assert!(iterations < 20, "probing did not converge");
    }

    assert_eq!(disc.state(), ProbeState::Discovered);
    assert_eq!(disc.mtu(), actual_mtu);
}

#[test]
fn probing_finds_max() {
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 1500,
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // All probes succeed — MTU is at least max
    loop {
        let probe = disc.next_probe();
        if probe.is_none() {
            break;
        }
        disc.on_probe_result(probe.unwrap().size, true);
    }

    assert_eq!(disc.state(), ProbeState::Discovered);
    assert_eq!(disc.mtu(), 1500);
}

#[test]
fn probing_finds_min() {
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 1500,
        max_failures: 100, // Don't trigger failure
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // All probes fail — MTU is exactly min
    loop {
        let probe = disc.next_probe();
        if probe.is_none() {
            break;
        }
        disc.on_probe_result(probe.unwrap().size, false);
    }

    assert_eq!(disc.state(), ProbeState::Discovered);
    assert_eq!(disc.mtu(), 500);
}

// ---------------------------------------------------------------------------
// Failure Handling
// ---------------------------------------------------------------------------

#[test]
fn consecutive_failures_abort() {
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 9000,
        max_failures: 3,
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // Fail 3 times in a row
    for _ in 0..3 {
        let probe = disc.next_probe();
        assert!(probe.is_some());
        disc.on_probe_result(probe.unwrap().size, false);
    }

    assert_eq!(disc.state(), ProbeState::Failed);
    // MTU should be the last known-good (min since all failed)
    assert_eq!(disc.mtu(), 500);
    // No more probes
    assert!(disc.next_probe().is_none());
}

#[test]
fn success_resets_failure_count() {
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 2000,
        max_failures: 3,
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // Fail twice, then succeed — should not abort
    let p1 = disc.next_probe().unwrap();
    disc.on_probe_result(p1.size, false);
    let p2 = disc.next_probe().unwrap();
    disc.on_probe_result(p2.size, false);
    let p3 = disc.next_probe().unwrap();
    disc.on_probe_result(p3.size, true); // resets failure count

    // Should still be probing
    assert_eq!(disc.state(), ProbeState::Probing);
}

// ---------------------------------------------------------------------------
// Aligned Payload
// ---------------------------------------------------------------------------

#[test]
fn aligned_payload_basic() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(1500);

    // 1500 MTU - 40 byte header = 1460 available
    // Align to 128 → 1460 / 128 = 11.4 → 11 * 128 = 1408
    assert_eq!(disc.aligned_payload_size(40, 128), 1408);
}

#[test]
fn aligned_payload_exact_fit() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(1000);

    // 1000 - 0 header = 1000, align to 100 → 1000
    assert_eq!(disc.aligned_payload_size(0, 100), 1000);
}

#[test]
fn aligned_payload_too_small() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(50);

    // 50 - 100 header → 0
    assert_eq!(disc.aligned_payload_size(100, 16), 0);
}

#[test]
fn aligned_payload_zero_alignment() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(1500);

    assert_eq!(disc.aligned_payload_size(40, 0), 0);
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_restores_initial_state() {
    let config = MtuConfig {
        min_mtu: 500,
        max_mtu: 1500,
        ..MtuConfig::default()
    };
    let mut disc = MtuDiscoverer::new(config);

    // Drive some probing
    let probe = disc.next_probe().unwrap();
    disc.on_probe_result(probe.size, true);

    // Reset
    disc.reset();
    assert_eq!(disc.state(), ProbeState::Initial);
    assert_eq!(disc.mtu(), 500);
}

#[test]
fn reset_preserves_override() {
    let mut disc = MtuDiscoverer::with_defaults();
    disc.set_override(1400);
    disc.reset();
    assert!(disc.has_override());
    assert_eq!(disc.mtu(), 1400);
}
