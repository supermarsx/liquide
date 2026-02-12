use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use crate::negotiate::{
    NegotiateConfig, ProbeResult, TransportKind, TransportNegotiator, TransportStrategy,
};

// ---------------------------------------------------------------------------
// TransportKind
// ---------------------------------------------------------------------------

#[test]
fn transport_kind_ranking() {
    assert!(TransportKind::Quic.default_rank() < TransportKind::Tcp.default_rank());
    assert!(TransportKind::TlsTcp.default_rank() < TransportKind::WebSocket.default_rank());
}

#[test]
fn transport_kind_encryption() {
    assert!(TransportKind::Quic.is_encrypted());
    assert!(TransportKind::TlsTcp.is_encrypted());
    assert!(!TransportKind::Tcp.is_encrypted());
    assert!(!TransportKind::Udp.is_encrypted());
    assert!(TransportKind::WebSocket.is_encrypted());
}

#[test]
fn transport_kind_reliability() {
    assert!(TransportKind::Quic.is_reliable());
    assert!(TransportKind::TlsTcp.is_reliable());
    assert!(TransportKind::Tcp.is_reliable());
    assert!(!TransportKind::Udp.is_reliable());
    assert!(TransportKind::WebSocket.is_reliable());
}

#[test]
fn transport_kind_all() {
    let all: Vec<_> = TransportKind::all().collect();
    assert_eq!(all.len(), 5);
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

#[test]
fn strategy_auto_candidates() {
    let strategy = TransportStrategy::Auto;
    let candidates = strategy.candidates();
    assert!(!candidates.is_empty());
    // QUIC should be first (rank 0)
    assert_eq!(candidates[0], TransportKind::Quic);
}

#[test]
fn strategy_force_tcp() {
    let strategy = TransportStrategy::ForceTcp;
    let candidates = strategy.candidates();
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&TransportKind::TlsTcp));
    assert!(candidates.contains(&TransportKind::Tcp));
    assert!(!candidates.contains(&TransportKind::Quic));
    assert!(!candidates.contains(&TransportKind::Udp));
}

#[test]
fn strategy_specific() {
    let strategy = TransportStrategy::Specific(TransportKind::Udp);
    let candidates = strategy.candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0], TransportKind::Udp);
}

#[test]
fn strategy_priority_list() {
    let list = vec![TransportKind::Tcp, TransportKind::Quic];
    let strategy = TransportStrategy::PriorityList(list.clone());
    assert_eq!(strategy.candidates(), list);
}

#[test]
fn strategy_default_is_auto() {
    assert_eq!(TransportStrategy::default(), TransportStrategy::Auto);
}

// ---------------------------------------------------------------------------
// Negotiator
// ---------------------------------------------------------------------------

fn test_addr() -> SocketAddr {
    SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9000)
}

#[test]
fn negotiator_initial_state() {
    let neg = TransportNegotiator::with_defaults();
    assert_eq!(*neg.strategy(), TransportStrategy::Auto);
    assert!(neg.select_best().is_none());
}

#[test]
fn negotiator_select_best_by_rtt() {
    let mut neg = TransportNegotiator::with_defaults();
    let now = Instant::now();

    neg.record_probe(ProbeResult {
        kind: TransportKind::Tcp,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(100)),
        probed_at: now,
        error: None,
    });
    neg.record_probe(ProbeResult {
        kind: TransportKind::Quic,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(30)),
        probed_at: now,
        error: None,
    });
    neg.record_probe(ProbeResult {
        kind: TransportKind::Udp,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(50)),
        probed_at: now,
        error: None,
    });

    assert_eq!(neg.select_best(), Some(TransportKind::Quic));
}

#[test]
fn negotiator_ignores_failed_probes() {
    let mut neg = TransportNegotiator::with_defaults();
    let now = Instant::now();

    neg.record_probe(ProbeResult {
        kind: TransportKind::Quic,
        addr: test_addr(),
        success: false,
        rtt: None,
        probed_at: now,
        error: Some("connection refused".into()),
    });
    neg.record_probe(ProbeResult {
        kind: TransportKind::Tcp,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(80)),
        probed_at: now,
        error: None,
    });

    assert_eq!(neg.select_best(), Some(TransportKind::Tcp));
}

#[test]
fn negotiator_no_successful_probes() {
    let mut neg = TransportNegotiator::with_defaults();
    let now = Instant::now();

    neg.record_probe(ProbeResult {
        kind: TransportKind::Quic,
        addr: test_addr(),
        success: false,
        rtt: None,
        probed_at: now,
        error: Some("timeout".into()),
    });

    assert!(neg.select_best().is_none());
}

#[test]
fn negotiator_successful_probes_sorted() {
    let mut neg = TransportNegotiator::with_defaults();
    let now = Instant::now();

    neg.record_probe(ProbeResult {
        kind: TransportKind::Tcp,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(100)),
        probed_at: now,
        error: None,
    });
    neg.record_probe(ProbeResult {
        kind: TransportKind::Quic,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(20)),
        probed_at: now,
        error: None,
    });

    let probes = neg.successful_probes();
    assert_eq!(probes.len(), 2);
    assert_eq!(probes[0].kind, TransportKind::Quic);
    assert_eq!(probes[1].kind, TransportKind::Tcp);
}

#[test]
fn negotiator_clear_history() {
    let mut neg = TransportNegotiator::with_defaults();
    let now = Instant::now();
    neg.record_probe(ProbeResult {
        kind: TransportKind::Tcp,
        addr: test_addr(),
        success: true,
        rtt: Some(Duration::from_millis(50)),
        probed_at: now,
        error: None,
    });
    neg.clear_history();
    assert!(neg.select_best().is_none());
}

#[test]
fn negotiator_set_strategy() {
    let mut neg = TransportNegotiator::with_defaults();
    neg.set_strategy(TransportStrategy::ForceTcp);
    assert_eq!(*neg.strategy(), TransportStrategy::ForceTcp);
    let candidates = neg.candidates();
    assert!(!candidates.contains(&TransportKind::Quic));
}

#[test]
fn negotiator_probe_timeout() {
    let config = NegotiateConfig {
        probe_timeout: Duration::from_secs(10),
        ..NegotiateConfig::default()
    };
    let neg = TransportNegotiator::new(config);
    assert_eq!(neg.probe_timeout(), Duration::from_secs(10));
}
