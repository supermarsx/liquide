//! Test suite definitions and categorisation.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Named conformance suites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuiteName {
    /// Protocol handshake and capability negotiation.
    Handshake,
    /// Authentication flows (password, token, MFA).
    Auth,
    /// Frame streaming, tile delivery, damage regions.
    Streaming,
    /// Clipboard copy/paste and MIME negotiation.
    Clipboard,
    /// Security constraints (TLS, rate limiting, injection).
    Security,
    /// All suites combined.
    All,
}

impl SuiteName {
    /// All individual suite names (excluding `All`).
    pub const INDIVIDUAL: &'static [SuiteName] = &[
        SuiteName::Handshake,
        SuiteName::Auth,
        SuiteName::Streaming,
        SuiteName::Clipboard,
        SuiteName::Security,
    ];

    /// Parse a suite name from a string.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "handshake" => Some(Self::Handshake),
            "auth" => Some(Self::Auth),
            "streaming" => Some(Self::Streaming),
            "clipboard" => Some(Self::Clipboard),
            "security" => Some(Self::Security),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Handshake => "Handshake",
            Self::Auth => "Authentication",
            Self::Streaming => "Streaming",
            Self::Clipboard => "Clipboard",
            Self::Security => "Security",
            Self::All => "All Suites",
        }
    }

    /// Whether this selection includes a given individual suite.
    #[must_use]
    pub fn includes(self, other: SuiteName) -> bool {
        self == SuiteName::All || self == other
    }

    /// Expand `All` into the individual suites; otherwise return self.
    #[must_use]
    pub fn expand(self) -> &'static [SuiteName] {
        if self == SuiteName::All {
            Self::INDIVIDUAL
        } else {
            // Trick: match to a static slice of one.
            match self {
                SuiteName::Handshake => &[SuiteName::Handshake],
                SuiteName::Auth => &[SuiteName::Auth],
                SuiteName::Streaming => &[SuiteName::Streaming],
                SuiteName::Clipboard => &[SuiteName::Clipboard],
                SuiteName::Security => &[SuiteName::Security],
                SuiteName::All => Self::INDIVIDUAL,
            }
        }
    }
}

impl fmt::Display for SuiteName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handshake => write!(f, "handshake"),
            Self::Auth => write!(f, "auth"),
            Self::Streaming => write!(f, "streaming"),
            Self::Clipboard => write!(f, "clipboard"),
            Self::Security => write!(f, "security"),
            Self::All => write!(f, "all"),
        }
    }
}

/// Metadata about a suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteInfo {
    /// Suite identifier.
    pub name: SuiteName,
    /// Human-readable description.
    pub description: String,
    /// Number of test cases in this suite.
    pub case_count: usize,
}

impl SuiteInfo {
    /// Create suite info.
    #[must_use]
    pub fn new(name: SuiteName, description: impl Into<String>, case_count: usize) -> Self {
        Self {
            name,
            description: description.into(),
            case_count,
        }
    }
}
