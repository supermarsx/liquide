//! Multi-transport channel routing.
//!
//! Routes frames to the appropriate transport based on channel type.
//! For example, Graphics frames go over QUIC for multiplexed low-latency
//! delivery, Audio goes over UDP, and Control/Input go over TLS/TCP for
//! reliability.

use std::collections::HashMap;

use liquide_protocol::channel::ChannelId;

use crate::negotiate::TransportKind;

// ---------------------------------------------------------------------------
// Routing Table
// ---------------------------------------------------------------------------

/// Maps channels to their preferred transport.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    /// Per-channel routing.
    routes: HashMap<ChannelId, TransportKind>,
    /// Fallback transport for unmapped channels.
    fallback: TransportKind,
}

impl RoutingTable {
    /// Create an empty routing table with the given fallback.
    #[must_use]
    pub fn new(fallback: TransportKind) -> Self {
        Self {
            routes: HashMap::new(),
            fallback,
        }
    }

    /// Create the standard hybrid routing table per spec.
    ///
    /// - Control, Input → TLS/TCP (reliable, encrypted)
    /// - Graphics → QUIC (multiplexed, 0-RTT)
    /// - Audio → UDP (low latency)
    /// - Everything else → TLS/TCP
    #[must_use]
    pub fn standard_hybrid() -> Self {
        let mut routes = HashMap::new();
        routes.insert(ChannelId::Control, TransportKind::TlsTcp);
        routes.insert(ChannelId::Graphics, TransportKind::Quic);
        routes.insert(ChannelId::Audio, TransportKind::Udp);
        routes.insert(ChannelId::Input, TransportKind::TlsTcp);
        routes.insert(ChannelId::Clipboard, TransportKind::TlsTcp);
        routes.insert(ChannelId::Usb, TransportKind::TlsTcp);
        routes.insert(ChannelId::File, TransportKind::TlsTcp);
        routes.insert(ChannelId::Print, TransportKind::TlsTcp);
        routes.insert(ChannelId::Serial, TransportKind::TlsTcp);
        routes.insert(ChannelId::Plugin, TransportKind::TlsTcp);
        routes.insert(ChannelId::Recording, TransportKind::Quic);

        Self {
            routes,
            fallback: TransportKind::TlsTcp,
        }
    }

    /// Look up the transport for a channel.
    #[must_use]
    pub fn route(&self, channel: ChannelId) -> TransportKind {
        self.routes
            .get(&channel)
            .copied()
            .unwrap_or(self.fallback)
    }

    /// Override the transport for a channel.
    pub fn set_route(&mut self, channel: ChannelId, kind: TransportKind) {
        self.routes.insert(channel, kind);
    }

    /// Set the fallback transport.
    pub fn set_fallback(&mut self, kind: TransportKind) {
        self.fallback = kind;
    }

    /// The fallback transport.
    #[must_use]
    pub fn fallback(&self) -> TransportKind {
        self.fallback
    }

    /// Get all distinct transport kinds in use.
    #[must_use]
    pub fn active_transports(&self) -> Vec<TransportKind> {
        let mut kinds: Vec<_> = self.routes.values().copied().collect();
        kinds.push(self.fallback);
        kinds.sort_by_key(|k| k.default_rank());
        kinds.dedup();
        kinds
    }

    /// Number of channel→transport mappings (excluding fallback).
    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether the routing table has no explicit mappings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Sequence Correlator
// ---------------------------------------------------------------------------

/// Tracks sequence numbers across transports for reconnect consistency.
#[derive(Debug, Clone)]
pub struct SequenceCorrelator {
    /// Per-channel last-seen sequence number.
    sequences: HashMap<ChannelId, u32>,
}

impl SequenceCorrelator {
    /// Create a new correlator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequences: HashMap::new(),
        }
    }

    /// Record the sequence number for a channel.
    pub fn record(&mut self, channel: ChannelId, seq: u32) {
        self.sequences.insert(channel, seq);
    }

    /// Get the last recorded sequence for a channel.
    #[must_use]
    pub fn last_seq(&self, channel: ChannelId) -> Option<u32> {
        self.sequences.get(&channel).copied()
    }

    /// Reset all sequence tracking.
    pub fn reset(&mut self) {
        self.sequences.clear();
    }
}

impl Default for SequenceCorrelator {
    fn default() -> Self {
        Self::new()
    }
}
