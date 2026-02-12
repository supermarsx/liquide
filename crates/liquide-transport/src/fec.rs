//! XOR-based Forward Error Correction (FEC).
//!
//! Groups source packets into blocks and computes XOR-parity redundancy
//! packets.  A single lost packet per block can be recovered from the parity
//! and the remaining source packets.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// FEC Level
// ---------------------------------------------------------------------------

/// FEC redundancy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FecLevel {
    /// No FEC — zero overhead.
    Off,
    /// ~5 % overhead (1 parity per 20 source packets).
    Light,
    /// ~10 % overhead (1 parity per 10 source packets).
    Medium,
    /// ~25 % overhead (1 parity per 4 source packets).
    Aggressive,
}

impl FecLevel {
    /// The number of source packets per FEC block.
    #[must_use]
    pub fn block_size(self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Light => Some(20),
            Self::Medium => Some(10),
            Self::Aggressive => Some(4),
        }
    }

    /// Approximate overhead ratio (0.0–1.0).
    #[must_use]
    pub fn overhead(self) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::Light => 0.05,
            Self::Medium => 0.10,
            Self::Aggressive => 0.25,
        }
    }
}

// ---------------------------------------------------------------------------
// FEC Config
// ---------------------------------------------------------------------------

/// Configuration for adaptive FEC.
#[derive(Debug, Clone)]
pub struct FecConfig {
    /// Loss rate threshold to move from Off to Light.
    pub light_threshold: f64,
    /// Loss rate threshold to move from Light to Medium.
    pub medium_threshold: f64,
    /// Loss rate threshold to move from Medium to Aggressive.
    pub aggressive_threshold: f64,
}

impl Default for FecConfig {
    fn default() -> Self {
        Self {
            light_threshold: 0.005,
            medium_threshold: 0.02,
            aggressive_threshold: 0.05,
        }
    }
}

// ---------------------------------------------------------------------------
// Adaptive FEC
// ---------------------------------------------------------------------------

/// Selects FEC level based on observed loss rate.
#[derive(Debug, Clone)]
pub struct AdaptiveFec {
    config: FecConfig,
    level: FecLevel,
}

impl AdaptiveFec {
    /// Create with the given config, starting at `Off`.
    #[must_use]
    pub fn new(config: FecConfig) -> Self {
        Self {
            config,
            level: FecLevel::Off,
        }
    }

    /// Create with default config.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(FecConfig::default())
    }

    /// Current FEC level.
    #[must_use]
    pub fn level(&self) -> FecLevel {
        self.level
    }

    /// Update the FEC level based on the current loss rate (0.0–1.0).
    pub fn update(&mut self, loss_rate: f64) {
        self.level = if loss_rate >= self.config.aggressive_threshold {
            FecLevel::Aggressive
        } else if loss_rate >= self.config.medium_threshold {
            FecLevel::Medium
        } else if loss_rate >= self.config.light_threshold {
            FecLevel::Light
        } else {
            FecLevel::Off
        };
    }

    /// Force a specific level.
    pub fn set_level(&mut self, level: FecLevel) {
        self.level = level;
    }
}

// ---------------------------------------------------------------------------
// XOR FEC Encoder
// ---------------------------------------------------------------------------

/// Collects source packets into blocks and produces XOR parity packets.
#[derive(Debug)]
pub struct XorFecEncoder {
    block_size: usize,
    /// Source packets accumulated for the current block.
    buffer: Vec<Bytes>,
}

impl XorFecEncoder {
    /// Create an encoder for the given block size.
    ///
    /// # Panics
    ///
    /// Panics if `block_size` is 0.
    #[must_use]
    pub fn new(block_size: usize) -> Self {
        assert!(block_size > 0, "block_size must be > 0");
        Self {
            block_size,
            buffer: Vec::with_capacity(block_size),
        }
    }

    /// Create from a `FecLevel`.  Returns `None` for `Off`.
    #[must_use]
    pub fn from_level(level: FecLevel) -> Option<Self> {
        level.block_size().map(Self::new)
    }

    /// The number of source packets per block.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Add a source packet.  Returns a parity packet when the block is full.
    pub fn add_packet(&mut self, data: Bytes) -> Option<Bytes> {
        self.buffer.push(data);
        if self.buffer.len() >= self.block_size {
            let parity = Self::compute_parity(&self.buffer);
            self.buffer.clear();
            Some(parity)
        } else {
            None
        }
    }

    /// Flush any partial block, returning a parity packet for the buffered
    /// source packets.  Returns `None` if the buffer is empty.
    pub fn flush(&mut self) -> Option<Bytes> {
        if self.buffer.is_empty() {
            return None;
        }
        let parity = Self::compute_parity(&self.buffer);
        self.buffer.clear();
        Some(parity)
    }

    /// Number of source packets buffered so far in the current block.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// Compute XOR parity of the given packets.
    ///
    /// All packets are zero-padded to the length of the longest packet.
    fn compute_parity(packets: &[Bytes]) -> Bytes {
        let max_len = packets.iter().map(|p| p.len()).max().unwrap_or(0);
        let mut parity = vec![0u8; max_len];
        for pkt in packets {
            for (i, &b) in pkt.iter().enumerate() {
                parity[i] ^= b;
            }
        }
        Bytes::from(parity)
    }
}

// ---------------------------------------------------------------------------
// XOR FEC Decoder
// ---------------------------------------------------------------------------

/// Recovers a single lost packet per block using XOR parity.
#[derive(Debug)]
pub struct XorFecDecoder {
    block_size: usize,
}

impl XorFecDecoder {
    /// Create a decoder for the given block size.
    #[must_use]
    pub fn new(block_size: usize) -> Self {
        Self { block_size }
    }

    /// Create from a `FecLevel`.  Returns `None` for `Off`.
    #[must_use]
    pub fn from_level(level: FecLevel) -> Option<Self> {
        level.block_size().map(Self::new)
    }

    /// The number of source packets per block.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Attempt to recover a lost packet.
    ///
    /// `received` must contain exactly `block_size - 1` source packets (i.e.
    /// one packet is missing).  `parity` is the XOR parity for the complete
    /// block.
    ///
    /// Returns `Some(recovered)` on success, `None` if the input is invalid
    /// (wrong number of received packets, or more than one loss).
    #[must_use]
    pub fn recover(&self, received: &[Bytes], parity: &Bytes) -> Option<Bytes> {
        if received.len() != self.block_size - 1 {
            return None;
        }
        // recovered = parity XOR all received packets
        let max_len = std::iter::once(parity.len())
            .chain(received.iter().map(|p| p.len()))
            .max()
            .unwrap_or(0);

        let mut recovered = vec![0u8; max_len];
        // Start with parity
        for (i, &b) in parity.iter().enumerate() {
            recovered[i] ^= b;
        }
        // XOR with each received packet
        for pkt in received {
            for (i, &b) in pkt.iter().enumerate() {
                recovered[i] ^= b;
            }
        }
        Some(Bytes::from(recovered))
    }
}
