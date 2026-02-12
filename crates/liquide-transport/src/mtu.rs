//! Path MTU discovery via binary search probing.
//!
//! Discovers the maximum transmission unit (MTU) on the network path
//! by sending probe packets of varying sizes and converging on the
//! largest size that passes through without fragmentation.

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum MTU for IPv4 (RFC 791).
pub const MIN_MTU_IPV4: usize = 576;

/// Minimum MTU for IPv6 (RFC 8200).
pub const MIN_MTU_IPV6: usize = 1280;

/// Commonly assumed safe MTU (IPv6 minimum).
pub const SAFE_MTU: usize = 1280;

/// Default maximum MTU to probe (jumbo frames).
pub const MAX_MTU_DEFAULT: usize = 9000;

/// Default probe timeout.
const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// Maximum consecutive probe failures before giving up.
const MAX_FAILURES: u32 = 3;

// ---------------------------------------------------------------------------
// Probe State
// ---------------------------------------------------------------------------

/// Current state of the MTU discovery process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    /// No probing has started yet; using safe default.
    Initial,
    /// Actively probing with binary search.
    Probing,
    /// Path MTU has been determined.
    Discovered,
    /// Probing failed after too many timeouts; using safe default.
    Failed,
}

// ---------------------------------------------------------------------------
// MTU Config
// ---------------------------------------------------------------------------

/// Configuration for MTU discovery.
#[derive(Debug, Clone)]
pub struct MtuConfig {
    /// Minimum MTU to probe (lower bound of binary search).
    pub min_mtu: usize,
    /// Maximum MTU to probe (upper bound of binary search).
    pub max_mtu: usize,
    /// Timeout for each probe packet.
    pub probe_timeout: Duration,
    /// Maximum consecutive failures before giving up.
    pub max_failures: u32,
}

impl MtuConfig {
    /// Create a config for IPv4 paths.
    #[must_use]
    pub fn ipv4() -> Self {
        Self {
            min_mtu: MIN_MTU_IPV4,
            max_mtu: MAX_MTU_DEFAULT,
            probe_timeout: DEFAULT_PROBE_TIMEOUT,
            max_failures: MAX_FAILURES,
        }
    }

    /// Create a config for IPv6 paths.
    #[must_use]
    pub fn ipv6() -> Self {
        Self {
            min_mtu: MIN_MTU_IPV6,
            max_mtu: MAX_MTU_DEFAULT,
            probe_timeout: DEFAULT_PROBE_TIMEOUT,
            max_failures: MAX_FAILURES,
        }
    }
}

impl Default for MtuConfig {
    fn default() -> Self {
        Self::ipv4()
    }
}

// ---------------------------------------------------------------------------
// MTU Probe
// ---------------------------------------------------------------------------

/// A probe packet descriptor.
#[derive(Debug, Clone, Copy)]
pub struct MtuProbe {
    /// The MTU size to test.
    pub size: usize,
    /// When this probe was created.
    pub sent_at: Instant,
}

// ---------------------------------------------------------------------------
// MTU Discoverer
// ---------------------------------------------------------------------------

/// Path MTU discovery via binary search probing.
#[derive(Debug)]
pub struct MtuDiscoverer {
    config: MtuConfig,
    state: ProbeState,
    /// Current known-good MTU (lower bound).
    lower: usize,
    /// Current known-bad MTU (upper bound, exclusive).
    upper: usize,
    /// The discovered or overridden MTU.
    current_mtu: usize,
    /// Consecutive probe failures.
    failures: u32,
    /// Override value, if set.
    mtu_override: Option<usize>,
    /// When the last probe was sent.
    last_probe: Option<Instant>,
}

impl MtuDiscoverer {
    /// Create a new discoverer with the given configuration.
    #[must_use]
    pub fn new(config: MtuConfig) -> Self {
        let safe = config.min_mtu;
        Self {
            lower: config.min_mtu,
            upper: config.max_mtu + 1,
            config,
            state: ProbeState::Initial,
            current_mtu: safe,
            failures: 0,
            mtu_override: None,
            last_probe: None,
        }
    }

    /// Create with default (IPv4) configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(MtuConfig::default())
    }

    /// Current probe state.
    #[must_use]
    pub fn state(&self) -> ProbeState {
        self.state
    }

    /// The current effective MTU.
    ///
    /// Returns the override if set, the discovered MTU if probing is
    /// complete, or the safe default otherwise.
    #[must_use]
    pub fn mtu(&self) -> usize {
        self.mtu_override.unwrap_or(self.current_mtu)
    }

    /// Force a specific MTU, bypassing discovery.
    pub fn set_override(&mut self, mtu: usize) {
        self.mtu_override = Some(mtu);
    }

    /// Clear a previously set override.
    pub fn clear_override(&mut self) {
        self.mtu_override = None;
    }

    /// Whether an override is active.
    #[must_use]
    pub fn has_override(&self) -> bool {
        self.mtu_override.is_some()
    }

    /// Generate the next probe, or `None` if discovery is complete.
    #[must_use]
    pub fn next_probe(&mut self) -> Option<MtuProbe> {
        if self.mtu_override.is_some() {
            return None;
        }

        match self.state {
            ProbeState::Discovered | ProbeState::Failed => None,
            ProbeState::Initial => {
                self.state = ProbeState::Probing;
                self.next_probe()
            }
            ProbeState::Probing => {
                if self.upper - self.lower <= 1 {
                    // Converged
                    self.current_mtu = self.lower;
                    self.state = ProbeState::Discovered;
                    return None;
                }

                let mid = self.lower + (self.upper - self.lower) / 2;
                let now = Instant::now();
                self.last_probe = Some(now);
                Some(MtuProbe {
                    size: mid,
                    sent_at: now,
                })
            }
        }
    }

    /// Report the result of a probe.
    ///
    /// `success` indicates whether the probe packet was acknowledged
    /// (i.e. the MTU is at least `probe_size`).
    pub fn on_probe_result(&mut self, probe_size: usize, success: bool) {
        if self.state != ProbeState::Probing {
            return;
        }

        if success {
            self.failures = 0;
            self.lower = probe_size;
            self.current_mtu = probe_size;
        } else {
            self.failures += 1;
            self.upper = probe_size;
        }

        if self.failures >= self.config.max_failures {
            self.current_mtu = self.lower;
            self.state = ProbeState::Failed;
            return;
        }

        if self.upper - self.lower <= 1 {
            self.current_mtu = self.lower;
            self.state = ProbeState::Discovered;
        }
    }

    /// Check if the most recent probe has timed out.
    #[must_use]
    pub fn is_probe_timed_out(&self) -> bool {
        match self.last_probe {
            Some(sent) => sent.elapsed() > self.config.probe_timeout,
            None => false,
        }
    }

    /// Calculate the maximum payload size that fits within the current MTU
    /// and is aligned to the given boundary (e.g. NAL unit or tile size).
    ///
    /// Subtracts `header_size` bytes for protocol headers, then rounds down
    /// to the nearest multiple of `alignment`.
    ///
    /// Returns 0 if the MTU is too small for even one aligned payload.
    #[must_use]
    pub fn aligned_payload_size(&self, header_size: usize, alignment: usize) -> usize {
        let mtu = self.mtu();
        if mtu <= header_size || alignment == 0 {
            return 0;
        }
        let available = mtu - header_size;
        (available / alignment) * alignment
    }

    /// Reset the discoverer to its initial state for re-probing.
    pub fn reset(&mut self) {
        self.state = ProbeState::Initial;
        self.lower = self.config.min_mtu;
        self.upper = self.config.max_mtu + 1;
        self.current_mtu = self.config.min_mtu;
        self.failures = 0;
        self.last_probe = None;
        // Keep any override
    }
}
