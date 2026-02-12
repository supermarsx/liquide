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
    assert_eq!(table.route(ChannelId::Control), TransportKind::Tcp);
    assert_eq!(table.route(ChannelId::Graphics), TransportKind::Tcp);
}

#[test]
fn standard_hybrid_routing() {
    let table = RoutingTable::standard_hybrid();

    assert_eq!(table.route(ChannelId::Control), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::Graphics), TransportKind::Quic);
    assert_eq!(table.route(ChannelId::Audio), TransportKind::Udp);
    assert_eq!(table.route(ChannelId::Input), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::Clipboard), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::File), TransportKind::TlsTcp);
    assert_eq!(table.route(ChannelId::Recording), TransportKind::Quic);
}

#[test]
fn custom_route() {
    let mut table = RoutingTable::new(TransportKind::Tcp);
    table.set_route(ChannelId::Audio, TransportKind::Udp);
    assert_eq!(table.route(ChannelId::Audio), TransportKind::Udp);
    assert_eq!(table.route(ChannelId::Control), TransportKind::Tcp); // fallback
    assert_eq!(table.len(), 1);
}

#[test]
fn set_fallback() {
    let mut table = RoutingTable::new(TransportKind::Tcp);
    table.set_fallback(TransportKind::WebSocket);
    assert_eq!(table.fallback(), TransportKind::WebSocket);
    assert_eq!(table.route(ChannelId::Control), TransportKind::WebSocket);
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
    table.set_route(ChannelId::Audio, TransportKind::Udp);
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
    assert!(corr.last_seq(ChannelId::Control).is_none());
    assert!(corr.last_seq(ChannelId::Graphics).is_none());
}

#[test]
fn correlator_record_and_read() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::Control, 42);
    assert_eq!(corr.last_seq(ChannelId::Control), Some(42));

    corr.record(ChannelId::Control, 43);
    assert_eq!(corr.last_seq(ChannelId::Control), Some(43));
}

#[test]
fn correlator_multiple_channels() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::Control, 10);
    corr.record(ChannelId::Audio, 20);
    corr.record(ChannelId::Graphics, 30);

    assert_eq!(corr.last_seq(ChannelId::Control), Some(10));
    assert_eq!(corr.last_seq(ChannelId::Audio), Some(20));
    assert_eq!(corr.last_seq(ChannelId::Graphics), Some(30));
}

#[test]
fn correlator_reset() {
    let mut corr = SequenceCorrelator::new();
    corr.record(ChannelId::Control, 100);
    corr.reset();
    assert!(corr.last_seq(ChannelId::Control).is_none());
}

#[test]
fn correlator_default_trait() {
    let corr = SequenceCorrelator::default();
    assert!(corr.last_seq(ChannelId::Control).is_none());
}
