//! Network profile simulation for benchmarks.
//!
//! Provides network condition presets and a simple emulator that models
//! latency, bandwidth, and packet loss for protocol-layer benchmarks.

use serde::{Deserialize, Serialize};

/// Network conditions for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfile {
    /// Human-readable name.
    pub name: String,
    /// Round-trip time in milliseconds.
    pub rtt_ms: f64,
    /// Available bandwidth in megabits per second.
    pub bandwidth_mbps: f64,
    /// Packet loss percentage (0.0 - 100.0).
    pub packet_loss_percent: f64,
    /// Jitter in milliseconds.
    pub jitter_ms: f64,
}

impl NetworkProfile {
    /// Create a new network profile.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        rtt_ms: f64,
        bandwidth_mbps: f64,
        packet_loss_percent: f64,
        jitter_ms: f64,
    ) -> Self {
        Self {
            name: name.into(),
            rtt_ms,
            bandwidth_mbps,
            packet_loss_percent,
            jitter_ms,
        }
    }
}

/// Preset network conditions representing common deployment scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkPreset {
    /// Local area network (< 1ms RTT).
    Lan,
    /// Same-region datacenter (~2ms RTT).
    Datacenter,
    /// Good WAN connection (~30ms RTT).
    WanGood,
    /// Cross-continent WAN (~150ms RTT).
    WanCross,
    /// 4G cellular network (~50ms RTT).
    Cellular4g,
    /// 3G cellular network (~200ms RTT).
    Cellular3g,
    /// Hotel/airport Wi-Fi (~80ms RTT, lossy).
    HotelWifi,
    /// Satellite connection (~600ms RTT).
    Satellite,
}

/// All available network presets.
pub const ALL: &[NetworkPreset] = &[
    NetworkPreset::Lan,
    NetworkPreset::Datacenter,
    NetworkPreset::WanGood,
    NetworkPreset::WanCross,
    NetworkPreset::Cellular4g,
    NetworkPreset::Cellular3g,
    NetworkPreset::HotelWifi,
    NetworkPreset::Satellite,
];

impl NetworkPreset {
    /// Human-readable label for this preset.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lan => "lan",
            Self::Datacenter => "datacenter",
            Self::WanGood => "wan-good",
            Self::WanCross => "wan-cross",
            Self::Cellular4g => "4g",
            Self::Cellular3g => "3g",
            Self::HotelWifi => "hotel-wifi",
            Self::Satellite => "satellite",
        }
    }

    /// Convert this preset to a full `NetworkProfile`.
    #[must_use]
    pub fn to_profile(&self) -> NetworkProfile {
        match self {
            Self::Lan => NetworkProfile::new("lan", 0.5, 1000.0, 0.0, 0.1),
            Self::Datacenter => NetworkProfile::new("datacenter", 2.0, 1000.0, 0.0, 0.5),
            Self::WanGood => NetworkProfile::new("wan-good", 30.0, 100.0, 0.01, 5.0),
            Self::WanCross => NetworkProfile::new("wan-cross", 150.0, 50.0, 0.1, 20.0),
            Self::Cellular4g => NetworkProfile::new("4g", 50.0, 30.0, 0.5, 15.0),
            Self::Cellular3g => NetworkProfile::new("3g", 200.0, 2.0, 1.0, 50.0),
            Self::HotelWifi => NetworkProfile::new("hotel-wifi", 80.0, 10.0, 2.0, 30.0),
            Self::Satellite => NetworkProfile::new("satellite", 600.0, 20.0, 0.5, 10.0),
        }
    }

    /// Look up a preset by name.
    pub fn from_name(name: &str) -> crate::Result<Self> {
        match name.to_lowercase().as_str() {
            "lan" => Ok(Self::Lan),
            "datacenter" | "dc" => Ok(Self::Datacenter),
            "wan-good" | "wan_good" | "wangood" => Ok(Self::WanGood),
            "wan-cross" | "wan_cross" | "wancross" => Ok(Self::WanCross),
            "4g" | "cellular4g" | "cellular-4g" => Ok(Self::Cellular4g),
            "3g" | "cellular3g" | "cellular-3g" => Ok(Self::Cellular3g),
            "hotel-wifi" | "hotel_wifi" | "hotelwifi" => Ok(Self::HotelWifi),
            "satellite" | "sat" => Ok(Self::Satellite),
            _ => Err(crate::BenchError::UnknownNetwork {
                name: name.to_string(),
            }),
        }
    }
}

impl std::fmt::Display for NetworkPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Simple network emulator for protocol benchmarks.
///
/// Provides deterministic latency and loss simulation based on a
/// `NetworkProfile`. No actual network traffic is generated; the emulator
/// only computes simulated values for use in benchmark measurements.
#[derive(Debug, Clone)]
pub struct NetworkEmulator {
    profile: NetworkProfile,
    packet_counter: u64,
}

impl NetworkEmulator {
    /// Create a new emulator for the given profile.
    #[must_use]
    pub fn new(profile: NetworkProfile) -> Self {
        Self {
            profile,
            packet_counter: 0,
        }
    }

    /// Create an emulator from a preset.
    #[must_use]
    pub fn from_preset(preset: NetworkPreset) -> Self {
        Self::new(preset.to_profile())
    }

    /// The underlying network profile.
    #[must_use]
    pub fn profile(&self) -> &NetworkProfile {
        &self.profile
    }

    /// Simulated round-trip time in milliseconds for the given iteration.
    ///
    /// Uses a deterministic jitter pattern based on the iteration index to
    /// produce repeatable results.
    #[must_use]
    pub fn simulated_rtt(&self, iteration: u64) -> f64 {
        let jitter_factor = ((iteration % 7) as f64 - 3.0) / 3.0;
        let jitter = self.profile.jitter_ms * jitter_factor;
        (self.profile.rtt_ms + jitter).max(0.1)
    }

    /// Effective bandwidth in bytes per second, accounting for protocol
    /// overhead (assumed 5%).
    #[must_use]
    pub fn effective_bandwidth(&self) -> f64 {
        let bits_per_sec = self.profile.bandwidth_mbps * 1_000_000.0;
        let bytes_per_sec = bits_per_sec / 8.0;
        bytes_per_sec * 0.95
    }

    /// Whether a packet at the given index should be considered "dropped".
    ///
    /// Uses simple deterministic logic: a packet is dropped if
    /// `(index * 97 + 13) % 10000 < loss_rate * 100`.
    pub fn should_drop_packet(&mut self) -> bool {
        let idx = self.packet_counter;
        self.packet_counter += 1;
        if self.profile.packet_loss_percent <= 0.0 {
            return false;
        }
        let hash = (idx * 97 + 13) % 10_000;
        let threshold = (self.profile.packet_loss_percent * 100.0) as u64;
        hash < threshold
    }

    /// Reset the packet counter.
    pub fn reset(&mut self) {
        self.packet_counter = 0;
    }
}
