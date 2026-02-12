//! Network connection types.
//!
//! Models individual network connections with full metadata including
//! protocol, state, TLS details, geolocation, and socket options
//! (spec section 14.4).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// NetworkProtocol
// ---------------------------------------------------------------------------

/// Transport-layer protocol of a connection (spec 14.4.1 – Protocol column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkProtocol {
    Tcp,
    Udp,
    Tcp6,
    Udp6,
    Sctp,
    Quic,
}

impl NetworkProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Tcp6 => "TCP6",
            Self::Udp6 => "UDP6",
            Self::Sctp => "SCTP",
            Self::Quic => "QUIC",
        }
    }
}

impl fmt::Display for NetworkProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ConnectionState
// ---------------------------------------------------------------------------

/// TCP connection state (spec 14.4.1 – State column, 14.4.2 visual).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Established,
    Listen,
    TimeWait,
    CloseWait,
    FinWait1,
    FinWait2,
    Closing,
    LastAck,
    SynSent,
    SynReceived,
    Closed,
}

impl ConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Established => "Established",
            Self::Listen => "Listen",
            Self::TimeWait => "Time Wait",
            Self::CloseWait => "Close Wait",
            Self::FinWait1 => "FIN Wait 1",
            Self::FinWait2 => "FIN Wait 2",
            Self::Closing => "Closing",
            Self::LastAck => "Last ACK",
            Self::SynSent => "SYN Sent",
            Self::SynReceived => "SYN Received",
            Self::Closed => "Closed",
        }
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// TlsVersion
// ---------------------------------------------------------------------------

/// TLS protocol version negotiated on a connection (spec 14.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsVersion {
    Tls10,
    Tls11,
    Tls12,
    Tls13,
    Dtls12,
}

impl TlsVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tls10 => "TLS 1.0",
            Self::Tls11 => "TLS 1.1",
            Self::Tls12 => "TLS 1.2",
            Self::Tls13 => "TLS 1.3",
            Self::Dtls12 => "DTLS 1.2",
        }
    }
}

impl fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ConnectionAction
// ---------------------------------------------------------------------------

/// Context-menu actions available for a connection (spec 14.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionAction {
    Close,
    Block,
    AllowPermanent,
    CopyDetails,
    Whois,
    Traceroute,
    GeoLookup,
    AddFirewallRule,
    CaptureTraffic,
}

impl ConnectionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::Block => "Block",
            Self::AllowPermanent => "Allow Permanent",
            Self::CopyDetails => "Copy Details",
            Self::Whois => "WHOIS",
            Self::Traceroute => "Traceroute",
            Self::GeoLookup => "Geo Lookup",
            Self::AddFirewallRule => "Add Firewall Rule",
            Self::CaptureTraffic => "Capture Traffic",
        }
    }
}

impl fmt::Display for ConnectionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// GeoInfo
// ---------------------------------------------------------------------------

/// Geolocation data for a remote endpoint (spec 14.4.1 – Remote Geo/ASN).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    /// Country name (e.g. "United States").
    pub country: Option<String>,
    /// City name.
    pub city: Option<String>,
    /// Geographic latitude.
    pub latitude: Option<f64>,
    /// Geographic longitude.
    pub longitude: Option<f64>,
    /// Internet service provider name.
    pub isp: Option<String>,
    /// Autonomous system number.
    pub asn: Option<u32>,
}

// ---------------------------------------------------------------------------
// ConnectionInfo
// ---------------------------------------------------------------------------

/// Full metadata for a single network connection (spec 14.4.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Process ID owning this connection.
    pub pid: u32,
    /// Name of the owning process.
    pub process_name: String,
    /// Transport-layer protocol.
    pub protocol: NetworkProtocol,
    /// Local IP address.
    pub local_address: String,
    /// Local port number.
    pub local_port: u16,
    /// Remote IP address.
    pub remote_address: String,
    /// Remote port number.
    pub remote_port: u16,
    /// Current connection state.
    pub state: ConnectionState,
    /// Total bytes sent on this connection.
    pub bytes_sent: u64,
    /// Total bytes received on this connection.
    pub bytes_received: u64,
    /// Current outbound throughput in bytes per second.
    pub bytes_sent_rate: u64,
    /// Current inbound throughput in bytes per second.
    pub bytes_received_rate: u64,
    /// Total packets sent.
    pub packets_sent: u64,
    /// Total packets received.
    pub packets_received: u64,
    /// Smoothed round-trip latency in milliseconds.
    pub latency_ms: Option<f64>,
    /// RTT variance (jitter) in milliseconds.
    pub jitter_ms: Option<f64>,
    /// Estimated packet loss percentage.
    pub packet_loss_percent: Option<f64>,
    /// Negotiated TLS version, if any.
    pub tls_version: Option<TlsVersion>,
    /// Negotiated TLS cipher suite.
    pub tls_cipher: Option<String>,
    /// TLS Server Name Indication hostname.
    pub sni_hostname: Option<String>,
    /// Remote certificate subject (abbreviated).
    pub certificate_subject: Option<String>,
    /// Remote certificate issuer (abbreviated).
    pub certificate_issuer: Option<String>,
    /// Remote certificate expiry (ISO-8601).
    pub certificate_expiry: Option<String>,
    /// Geolocation information for the remote endpoint.
    pub geo_info: Option<GeoInfo>,
    /// Reverse DNS name of the remote address.
    pub dns_name: Option<String>,
    /// Detected application-layer protocol.
    pub app_protocol: Option<String>,
    /// Socket options summary.
    pub socket_options: Option<String>,
    /// Send buffer size in bytes.
    pub send_buffer_bytes: Option<u64>,
    /// Receive buffer size in bytes.
    pub recv_buffer_bytes: Option<u64>,
    /// TCP retransmission count.
    pub retransmits: Option<u64>,
    /// Smoothed round-trip time in milliseconds.
    pub rtt_ms: Option<f64>,
    /// TCP congestion window size in segments.
    pub congestion_window: Option<u64>,
    /// Maximum segment size in bytes.
    pub mss: Option<u16>,
    /// TCP receive window size in bytes.
    pub window_size: Option<u32>,
    /// ISO-8601 timestamp when the connection was established.
    pub established_time: Option<String>,
}
