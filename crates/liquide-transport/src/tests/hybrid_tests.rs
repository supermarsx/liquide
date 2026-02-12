use liquide_protocol::channel::ChannelId;

use crate::hybrid::{RoutingTable, SequenceCorrelator};
use crate::negotiate::TransportKind;

// ---------------------------------------------------------------------------
// Routing Table
// ---------------------------------------------------------------------------

#[test]
fn routing_table_empty() {
    let table = RoutingTable::new(TransportKind::Tcp);
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
    assert_eq!(table.fallback(), TransportKind::Tcp);
    // All channels fall back
    assert_eq!(table.route(ChannelId::CONTROL), TransportKind::Tcp);
    assert_eq!(table.route(ChannelId::VIDEO), TransportKind::Tcp);
}

#[test]
fn standard_hybrid_routing() {
    let table = RoutingTable::standard_hybrid();

    assert_eq!(table.route(ChannelId::CONTROL), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::VIDEO), TransportKind::Quic);
    assert_eq!(table.route(ChannelId::AUDIO_PLAYBACK), TransportKind::Udp);
    assert_eq!(table.route(ChannelId::INPUT), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::CLIPBOARD), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::FILE_TRANSFER), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::CAMERA), TransportKind::Quic);
}

#[test]
fn custom_route() {
    let mut table = RoutingTable::new(TransportKind::Tcp);
    table.set_route(ChannelId::AUDIO_PLAYBACK, TransportKind::Udp);
    assert_eq!(table.route(ChannelId::AUDIO_PLAYBACK), TransportKind::Udp);
    assert_eq!(table.route(ChannelId::CONTROL), TransportKind::Tcp); // fallback
    assert_eq!(table.len(), 1);
}

#[test]
fn set_fallback() {
    let mut table = RoutingTable::new(TransportKind::Tcp);
    table.set_fallback(TransportKind::WebSocket);
    assert_eq!(table.fallback(), TransportKind::WebSocket);
    assert_eq!(table.route(ChannelId::CONTROL), TransportKind::WebSocket);
}

#[test]
fn active_transports() {
    let table = RoutingTable::standard_hybrid();
    let active = table.active_transports();
    // Should include Quic, TlsTcp, Udp
    assert!(active.contains(&TransportKind::Quic));
    assert!(active.contains(&TransportKind::TlsTcp));
    assert!(active.contains(&TransportKind::Udp));
    // No duplicates
    let len = active.len();
    let mut deduped = active.clone();
    deduped.dedup();
    assert_eq!(len, deduped.len());
}

#[test]
fn active_transports_includes_fallback() {
    let mut table = RoutingTable::new(TransportKind::WebSocket);
    table.set_route(ChannelId::AUDIO_PLAYBACK, TransportKind::Udp);
    let active = table.active_transports();
    assert!(active.contains(&TransportKind::WebSocket));
    assert!(active.contains(&TransportKind::Udp));
}

// ---------------------------------------------------------------------------
// Sequence Correlator
// ---------------------------------------------------------------------------

#[test]
fn correlator_initial_empty() {
    let corr = SequenceCorrelator::new();
    assert!(corr.last_seq(ChannelId::CONTROL).is_none());
    assert!(corr.last_seq(ChannelId::VIDEO).is_none());
}

#[test]
fn correlator_record_and_read() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::CONTROL, 42);
    assert_eq!(corr.last_seq(ChannelId::CONTROL), Some(42));

    corr.record(ChannelId::CONTROL, 43);
    assert_eq!(corr.last_seq(ChannelId::CONTROL), Some(43));
}

#[test]
fn correlator_multiple_channels() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::CONTROL, 10);
    corr.record(ChannelId::AUDIO_PLAYBACK, 20);
    corr.record(ChannelId::VIDEO, 30);

    assert_eq!(corr.last_seq(ChannelId::CONTROL), Some(10));
    assert_eq!(corr.last_seq(ChannelId::AUDIO_PLAYBACK), Some(20));
    assert_eq!(corr.last_seq(ChannelId::VIDEO), Some(30));
}

#[test]
fn correlator_reset() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::CONTROL, 100);
    corr.reset();
    assert!(corr.last_seq(ChannelId::CONTROL).is_none());
}

#[test]
fn correlator_default_trait() {
    let corr = SequenceCorrelator::default();
    assert!(corr.last_seq(ChannelId::CONTROL).is_none());
}
