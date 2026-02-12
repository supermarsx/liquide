use crate::stats::TransportStats;

#[test]
fn new_stats_are_zero() {
    let s = TransportStats::new();
    assert_eq!(s.bytes_sent(), 0);
    assert_eq!(s.bytes_recv(), 0);
    assert_eq!(s.messages_sent(), 0);
    assert_eq!(s.messages_recv(), 0);
    assert_eq!(s.errors(), 0);
}

#[test]
fn record_send_increments() {
    let s = TransportStats::new();
    s.record_send(100);
    s.record_send(200);
    assert_eq!(s.bytes_sent(), 300);
    assert_eq!(s.messages_sent(), 2);
}

#[test]
fn record_recv_increments() {
    let s = TransportStats::new();
    s.record_recv(50);
    s.record_recv(75);
    assert_eq!(s.bytes_recv(), 125);
    assert_eq!(s.messages_recv(), 2);
}

#[test]
fn record_error_increments() {
    let s = TransportStats::new();
    s.record_error();
    s.record_error();
    s.record_error();
    assert_eq!(s.errors(), 3);
}

#[test]
fn snapshot_captures_current_values() {
    let s = TransportStats::new();
    s.record_send(1000);
    s.record_recv(500);
    s.record_error();
    let snap = s.snapshot();
    assert_eq!(snap.bytes_sent, 1000);
    assert_eq!(snap.bytes_recv, 500);
    assert_eq!(snap.messages_sent, 1);
    assert_eq!(snap.messages_recv, 1);
    assert_eq!(snap.errors, 1);
}

#[test]
fn reset_zeroes_all() {
    let s = TransportStats::new();
    s.record_send(999);
    s.record_recv(888);
    s.record_error();
    s.reset();
    assert_eq!(s.bytes_sent(), 0);
    assert_eq!(s.bytes_recv(), 0);
    assert_eq!(s.messages_sent(), 0);
    assert_eq!(s.messages_recv(), 0);
    assert_eq!(s.errors(), 0);
}

#[test]
fn snapshot_serialization_roundtrip() {
    let snap = crate::stats::StatsSnapshot {
        bytes_sent: 42,
        bytes_recv: 84,
        messages_sent: 10,
        messages_recv: 20,
        errors: 1,
    };
    let json = serde_json::to_string(&snap).unwrap();
    let deserialized: crate::stats::StatsSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(snap, deserialized);
}
