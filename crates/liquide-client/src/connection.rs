//! Connection state machine and connection manager.

use std::fmt;

use crate::{ClientError, Result};

/// Connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Negotiating,
    Connected,
    Reconnecting,
    Failed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Authenticating => "Authenticating",
            Self::Negotiating => "Negotiating",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting",
            Self::Failed => "Failed",
        };
        f.write_str(label)
    }
}

/// Coarse quality assessment derived from live metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Bad,
    Disconnected,
}

impl ConnectionQuality {
    /// Derive quality from round-trip time (ms), packet loss (0.0..1.0), and
    /// whether the server has signalled quality degradation.
    #[must_use]
    pub fn from_metrics(rtt_ms: f64, loss_percent: f64, degraded: bool) -> Self {
        if rtt_ms <= 0.0 {
            return Self::Disconnected;
        }
        if degraded || loss_percent > 10.0 || rtt_ms > 300.0 {
            return Self::Bad;
        }
        if loss_percent > 5.0 || rtt_ms > 200.0 {
            return Self::Poor;
        }
        if loss_percent > 2.0 || rtt_ms > 100.0 {
            return Self::Fair;
        }
        if loss_percent > 0.5 || rtt_ms > 50.0 {
            return Self::Good;
        }
        Self::Excellent
    }

    /// CSS-style colour code for UI indicators.
    #[must_use]
    pub fn color(&self) -> &str {
        match self {
            Self::Excellent => "#00c853",
            Self::Good => "#64dd17",
            Self::Fair => "#ffd600",
            Self::Poor => "#ff6d00",
            Self::Bad => "#d50000",
            Self::Disconnected => "#9e9e9e",
        }
    }
}

/// A saved connection profile (server bookmark).
#[derive(Debug, Clone)]
pub struct ConnectionProfile {
    pub name: String,
    pub address: String,
    pub username: Option<String>,
    pub transport: String,
    pub encoder: String,
    pub encryption: String,
    pub monitors: u32,
    pub audio_playback: bool,
    pub audio_microphone: bool,
    pub clipboard: bool,
    pub performance: String,
    pub cursor_mode: String,
}

/// Manages the connection to a single remote server.
pub struct ConnectionManager {
    state: ConnectionState,
    profiles: Vec<ConnectionProfile>,
    active_profile: Option<usize>,
    server_addr: String,
    rtt_ms: f64,
    packet_loss_percent: f64,
    bandwidth_mbps: f64,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
}

impl ConnectionManager {
    /// Create a new connection manager.
    #[must_use]
    pub fn new(max_reconnect_attempts: u32) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            profiles: Vec::new(),
            active_profile: None,
            server_addr: String::new(),
            rtt_ms: 0.0,
            packet_loss_percent: 0.0,
            bandwidth_mbps: 0.0,
            reconnect_attempts: 0,
            max_reconnect_attempts,
        }
    }

    /// Initiate a connection to the given server.
    pub fn connect(&mut self, server: &str) -> Result<()> {
        if self.state == ConnectionState::Connected {
            self.disconnect();
        }

        self.server_addr = server.to_string();
        self.reconnect_attempts = 0;

        // Transition: Disconnected -> Connecting -> Authenticating -> Negotiating -> Connected
        self.state = ConnectionState::Connecting;
        self.state = ConnectionState::Authenticating;
        self.state = ConnectionState::Negotiating;
        self.state = ConnectionState::Connected;

        Ok(())
    }

    /// Disconnect from the current server.
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.rtt_ms = 0.0;
        self.packet_loss_percent = 0.0;
        self.bandwidth_mbps = 0.0;
        self.reconnect_attempts = 0;
    }

    /// Attempt to reconnect to the last server.
    pub fn reconnect(&mut self) -> Result<()> {
        if self.server_addr.is_empty() {
            return Err(ClientError::ServerUnreachable {
                server: "(none)".to_string(),
            });
        }

        if !self.should_reconnect() {
            return Err(ClientError::ReconnectFailed {
                attempts: self.reconnect_attempts,
            });
        }

        self.reconnect_attempts += 1;
        self.state = ConnectionState::Reconnecting;

        // Simulate reconnection attempt succeeding.
        self.state = ConnectionState::Connected;
        Ok(())
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Assess current connection quality from live metrics.
    #[must_use]
    pub fn quality(&self) -> ConnectionQuality {
        if self.state != ConnectionState::Connected {
            return ConnectionQuality::Disconnected;
        }
        ConnectionQuality::from_metrics(self.rtt_ms, self.packet_loss_percent, false)
    }

    /// Add a connection profile.
    pub fn add_profile(&mut self, profile: ConnectionProfile) {
        self.profiles.push(profile);
    }

    /// Remove a connection profile by name. Returns `true` if found.
    pub fn remove_profile(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < before
    }

    /// List all saved profiles.
    #[must_use]
    pub fn profiles(&self) -> &[ConnectionProfile] {
        &self.profiles
    }

    /// Index of the currently active profile, if any.
    #[must_use]
    pub fn active_profile(&self) -> Option<&ConnectionProfile> {
        self.active_profile.and_then(|i| self.profiles.get(i))
    }

    /// Update live metrics from the transport layer.
    pub fn update_metrics(&mut self, rtt_ms: f64, packet_loss_percent: f64, bandwidth_mbps: f64) {
        self.rtt_ms = rtt_ms;
        self.packet_loss_percent = packet_loss_percent;
        self.bandwidth_mbps = bandwidth_mbps;
    }

    /// Whether another reconnect attempt is allowed.
    #[must_use]
    pub fn should_reconnect(&self) -> bool {
        // max_reconnect_attempts == 0 means unlimited.
        self.max_reconnect_attempts == 0
            || self.reconnect_attempts < self.max_reconnect_attempts
    }

    /// Compute the delay before the next reconnect attempt (exponential back-off).
    #[must_use]
    pub fn next_reconnect_delay_ms(&self) -> u32 {
        let base: u32 = 1000;
        let max_delay: u32 = 30000;
        let exp = self.reconnect_attempts.min(15);
        let delay = base.saturating_mul(1u32.wrapping_shl(exp));
        delay.min(max_delay)
    }
}
