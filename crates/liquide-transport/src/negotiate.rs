//! Transport selection, probing, and switching.
//!
//! Provides a negotiation layer that probes available transports (QUIC, UDP,
//! TLS/TCP, plain TCP, WebSocket) and selects the best one based on latency
//! and feature requirements.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Transport Kind
// ---------------------------------------------------------------------------

/// The available transport protocol types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportKind {
    /// QUIC (preferred for Graphics — multiplexed, 0-RTT).
    Quic,
    /// Raw UDP (preferred for Audio — low latency).
    Udp,
    /// TLS over TCP (preferred for Control, Input — reliable + encrypted).
    TlsTcp,
    /// Plain TCP (fallback).
    Tcp,
    /// WebSocket (browser compatibility).
    WebSocket,
}

impl TransportKind {
    /// Default quality ranking (lower is better).
    #[must_use]
    pub fn default_rank(self) -> u32 {
        match self {
            Self::Quic => 0,
            Self::TlsTcp => 1,
            Self::Udp => 2,
            Self::Tcp => 3,
            Self::WebSocket => 4,
        }
    }

    /// Whether this transport provides encryption.
    #[must_use]
    pub fn is_encrypted(self) -> bool {
        matches!(self, Self::Quic | Self::TlsTcp | Self::WebSocket)
    }

    /// Whether this transport provides reliable delivery.
    #[must_use]
    pub fn is_reliable(self) -> bool {
        matches!(
            self,
            Self::Quic | Self::TlsTcp | Self::Tcp | Self::WebSocket
        )
    }

    /// Iterator over all transport kinds.
    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Quic,
            Self::TlsTcp,
            Self::Udp,
            Self::Tcp,
            Self::WebSocket,
        ]
        .into_iter()
    }
}

// ---------------------------------------------------------------------------
// Transport Strategy
// ---------------------------------------------------------------------------

/// Strategy for selecting which transport(s) to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportStrategy {
    /// Automatically probe and select the best transport.
    Auto,
    /// Try transports in the given priority order.
    PriorityList(Vec<TransportKind>),
    /// Use a specific transport only.
    Specific(TransportKind),
    /// Force TCP only (disables QUIC/UDP).
    ForceTcp,
}

impl Default for TransportStrategy {
    fn default() -> Self {
        Self::Auto
    }
}

impl TransportStrategy {
    /// Return the candidate transports in order of preference.
    #[must_use]
    pub fn candidates(&self) -> Vec<TransportKind> {
        match self {
            Self::Auto => {
                let mut all: Vec<_> = TransportKind::all().collect();
                all.sort_by_key(|k| k.default_rank());
                all
            }
            Self::PriorityList(list) => list.clone(),
            Self::Specific(kind) => vec![*kind],
            Self::ForceTcp => vec![TransportKind::TlsTcp, TransportKind::Tcp],
        }
    }
}

// ---------------------------------------------------------------------------
// Probe Result
// ---------------------------------------------------------------------------

/// Result of probing a transport.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// The transport that was probed.
    pub kind: TransportKind,
    /// The target address.
    pub addr: SocketAddr,
    /// Whether the probe succeeded.
    pub success: bool,
    /// Round-trip time of the probe, if successful.
    pub rtt: Option<Duration>,
    /// When the probe was performed.
    pub probed_at: Instant,
    /// Error message, if the probe failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Negotiation Config
// ---------------------------------------------------------------------------

/// Configuration for transport negotiation.
#[derive(Debug, Clone)]
pub struct NegotiateConfig {
    /// Timeout for each probe attempt.
    pub probe_timeout: Duration,
    /// Maximum number of transports to probe.
    pub max_probes: usize,
    /// Strategy for transport selection.
    pub strategy: TransportStrategy,
}

impl Default for NegotiateConfig {
    fn default() -> Self {
        Self {
            probe_timeout: Duration::from_secs(5),
            max_probes: 5,
            strategy: TransportStrategy::Auto,
        }
    }
}

// ---------------------------------------------------------------------------
// Transport Negotiator
// ---------------------------------------------------------------------------

/// Probes available transports and selects the best one.
#[derive(Debug)]
pub struct TransportNegotiator {
    config: NegotiateConfig,
    /// Results from past probes.
    probe_history: Vec<ProbeResult>,
}

impl TransportNegotiator {
    /// Create a new negotiator with the given config.
    #[must_use]
    pub fn new(config: NegotiateConfig) -> Self {
        Self {
            config,
            probe_history: Vec::new(),
        }
    }

    /// Create with default config.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(NegotiateConfig::default())
    }

    /// The current strategy.
    #[must_use]
    pub fn strategy(&self) -> &TransportStrategy {
        &self.config.strategy
    }

    /// Set the strategy.
    pub fn set_strategy(&mut self, strategy: TransportStrategy) {
        self.config.strategy = strategy;
    }

    /// Get the candidate transports based on current strategy.
    #[must_use]
    pub fn candidates(&self) -> Vec<TransportKind> {
        self.config.strategy.candidates()
    }

    /// Record a probe result.
    pub fn record_probe(&mut self, result: ProbeResult) {
        self.probe_history.push(result);
    }

    /// Select the best transport from recorded probe results.
    ///
    /// Returns the transport kind with the lowest RTT among successful probes,
    /// or `None` if no probes succeeded.
    #[must_use]
    pub fn select_best(&self) -> Option<TransportKind> {
        self.probe_history
            .iter()
            .filter(|r| r.success)
            .min_by_key(|r| r.rtt.unwrap_or(Duration::MAX))
            .map(|r| r.kind)
    }

    /// Get all successful probe results, sorted by RTT.
    #[must_use]
    pub fn successful_probes(&self) -> Vec<&ProbeResult> {
        let mut results: Vec<_> = self.probe_history.iter().filter(|r| r.success).collect();
        results.sort_by_key(|r| r.rtt.unwrap_or(Duration::MAX));
        results
    }

    /// Probe timeout from config.
    #[must_use]
    pub fn probe_timeout(&self) -> Duration {
        self.config.probe_timeout
    }

    /// Clear probe history.
    pub fn clear_history(&mut self) {
        self.probe_history.clear();
    }
}
