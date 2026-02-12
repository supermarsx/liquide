//! DNS query monitoring types.
//!
//! Models DNS resolution events including query type, protocol,
//! DNSSEC validation, and domain categorization (spec section 14.6).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// DnsQueryType
// ---------------------------------------------------------------------------

/// DNS record type being queried (spec 14.6 – Query Type column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsQueryType {
    A,
    Aaaa,
    Cname,
    Mx,
    Ns,
    Txt,
    Srv,
    Ptr,
}

impl DnsQueryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Cname => "CNAME",
            Self::Mx => "MX",
            Self::Ns => "NS",
            Self::Txt => "TXT",
            Self::Srv => "SRV",
            Self::Ptr => "PTR",
        }
    }
}

impl fmt::Display for DnsQueryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DnsProtocol
// ---------------------------------------------------------------------------

/// Transport used for DNS resolution (spec 14.6 – Protocol column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsProtocol {
    Udp,
    Tcp,
    DoH,
    DoT,
}

impl DnsProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Udp => "UDP",
            Self::Tcp => "TCP",
            Self::DoH => "DoH",
            Self::DoT => "DoT",
        }
    }
}

impl fmt::Display for DnsProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DnssecStatus
// ---------------------------------------------------------------------------

/// DNSSEC validation result (spec 14.6 – DNSSEC column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnssecStatus {
    Secure,
    Insecure,
    Bogus,
}

impl DnssecStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Secure => "Secure",
            Self::Insecure => "Insecure",
            Self::Bogus => "Bogus",
        }
    }
}

impl fmt::Display for DnssecStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DomainCategory
// ---------------------------------------------------------------------------

/// Domain classification category (spec 14.6 – Category column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainCategory {
    Normal,
    Advertising,
    Tracking,
    Malware,
    Phishing,
    Social,
    Cdn,
}

impl DomainCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Advertising => "Advertising",
            Self::Tracking => "Tracking",
            Self::Malware => "Malware",
            Self::Phishing => "Phishing",
            Self::Social => "Social",
            Self::Cdn => "CDN",
        }
    }
}

impl fmt::Display for DomainCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DnsResponse
// ---------------------------------------------------------------------------

/// A DNS response payload (spec 14.6 – Response column).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsResponse {
    /// Resolved IP address or record value, if any.
    pub address: Option<String>,
    /// Time-to-live of the cached response in seconds.
    pub ttl_secs: u32,
    /// DNS response code (e.g. "NOERROR", "NXDOMAIN", "SERVFAIL").
    pub response_code: String,
    /// Whether the response came from an authoritative server.
    pub authoritative: bool,
}

// ---------------------------------------------------------------------------
// DnsQueryEntry
// ---------------------------------------------------------------------------

/// A single DNS query event captured by the monitor (spec 14.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsQueryEntry {
    /// ISO-8601 timestamp when the query was issued.
    pub timestamp: String,
    /// Domain name being resolved.
    pub domain: String,
    /// DNS record type requested.
    pub query_type: DnsQueryType,
    /// DNS server that answered the query.
    pub server: String,
    /// Transport protocol used for the query.
    pub protocol: DnsProtocol,
    /// Time taken to resolve the query in milliseconds.
    pub response_time_ms: f64,
    /// The DNS response, if one was received.
    pub response: Option<DnsResponse>,
    /// PID of the process that initiated the query.
    pub pid: Option<u32>,
    /// Name of the process that initiated the query.
    pub process_name: Option<String>,
    /// DNSSEC validation status.
    pub dnssec_status: Option<DnssecStatus>,
    /// Whether the query was blocked by a domain blocklist.
    pub blocked: bool,
    /// Domain classification category.
    pub category: Option<DomainCategory>,
    /// Whether the response was served from cache.
    pub cached: bool,
}
