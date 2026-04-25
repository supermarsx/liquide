//! Per-IP rate limiting, banning, and tarpit support.

use std::collections::HashMap;

use crate::config::LimitsConfig;
use crate::{GatewayError, Result};

/// Tracks per-IP request state for rate-limiting.
#[derive(Debug, Clone)]
pub struct RateLimiterEntry {
    /// The IP address being tracked.
    pub ip: String,
    /// Remaining token-bucket tokens.
    pub tokens: f64,
    /// Epoch timestamp of the last token refill.
    pub last_refill: u64,
    /// Total requests observed for this IP in the current window.
    pub request_count: u64,
    /// Number of authentication failures in the current window.
    pub auth_failures: u32,
}

/// An active IP ban.
#[derive(Debug, Clone)]
pub struct IpBan {
    /// Banned IP address.
    pub ip: String,
    /// Reason for the ban.
    pub reason: String,
    /// Epoch timestamp when the ban was created.
    pub banned_at: u64,
    /// Epoch timestamp when the ban expires.
    pub expires_at: u64,
}

/// Tarpit mode determining how to slow a malicious connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarpitMode {
    /// Slow TCP handshake.
    Tcp,
    /// Slow TLS negotiation.
    Tls,
    /// Slow authentication exchange.
    Auth,
}

impl std::fmt::Display for TarpitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp => write!(f, "tcp"),
            Self::Tls => write!(f, "tls"),
            Self::Auth => write!(f, "auth"),
        }
    }
}

/// An active tarpit session.
#[derive(Debug, Clone)]
pub struct TarpitSession {
    /// The IP being tarpitted.
    pub ip: String,
    /// When the tarpit started.
    pub started_at: u64,
    /// Bytes sent so far (drip-feed).
    pub bytes_sent: u64,
    /// The tarpit mode.
    pub mode: TarpitMode,
}

/// Token-bucket rate limiter with automatic banning.
pub struct RateLimiter {
    entries: HashMap<String, RateLimiterEntry>,
    bans: HashMap<String, IpBan>,
    config: LimitsConfig,
}

impl RateLimiter {
    /// Create a new rate limiter.
    #[must_use]
    pub fn new(config: LimitsConfig) -> Self {
        Self {
            entries: HashMap::new(),
            bans: HashMap::new(),
            config,
        }
    }

    /// Check whether a request from `ip` at `now` is allowed.
    ///
    /// Returns `Ok(())` if the request is within limits, or an error
    /// if the IP is banned or rate-limited.
    pub fn check_rate(&mut self, ip: &str, now: u64) -> Result<()> {
        // Check ban first.
        if self.is_banned(ip, now) {
            let ban = &self.bans[ip];
            return Err(GatewayError::IpBanned {
                ip: ip.to_string(),
                until: ban.expires_at.to_string(),
            });
        }

        let rate = self.config.per_ip_rate_per_sec as f64;

        let entry = self
            .entries
            .entry(ip.to_string())
            .or_insert_with(|| RateLimiterEntry {
                ip: ip.to_string(),
                tokens: rate,
                last_refill: now,
                request_count: 0,
                auth_failures: 0,
            });

        // Refill tokens based on elapsed time.
        let elapsed = now.saturating_sub(entry.last_refill);
        if elapsed > 0 {
            entry.tokens = (entry.tokens + elapsed as f64 * rate).min(rate * 2.0);
            entry.last_refill = now;
        }

        if entry.tokens < 1.0 {
            return Err(GatewayError::RateLimitExceeded {
                ip: ip.to_string(),
                window_seconds: self.config.auth_failure_window_sec,
            });
        }

        entry.tokens -= 1.0;
        Ok(())
    }

    /// Record a successful request (for accounting).
    pub fn record_request(&mut self, ip: &str, now: u64) {
        let entry = self
            .entries
            .entry(ip.to_string())
            .or_insert_with(|| RateLimiterEntry {
                ip: ip.to_string(),
                tokens: self.config.per_ip_rate_per_sec as f64,
                last_refill: now,
                request_count: 0,
                auth_failures: 0,
            });
        entry.request_count += 1;
    }

    /// Record an authentication failure. Returns `Some(IpBan)` if the
    /// failure threshold is crossed and the IP is now banned.
    pub fn record_auth_failure(&mut self, ip: &str, now: u64) -> Option<IpBan> {
        let rate = self.config.per_ip_rate_per_sec as f64;
        let entry = self
            .entries
            .entry(ip.to_string())
            .or_insert_with(|| RateLimiterEntry {
                ip: ip.to_string(),
                tokens: rate,
                last_refill: now,
                request_count: 0,
                auth_failures: 0,
            });
        entry.auth_failures += 1;

        if entry.auth_failures >= self.config.auth_failure_ban_threshold {
            let ban = IpBan {
                ip: ip.to_string(),
                reason: format!(
                    "exceeded {} auth failures in {}s",
                    self.config.auth_failure_ban_threshold, self.config.auth_failure_window_sec,
                ),
                banned_at: now,
                expires_at: now + self.config.ban_duration_sec,
            };
            self.bans.insert(ip.to_string(), ban.clone());
            // Reset failure counter.
            entry.auth_failures = 0;
            Some(ban)
        } else {
            None
        }
    }

    /// Check if an IP is currently banned.
    #[must_use]
    pub fn is_banned(&self, ip: &str, now: u64) -> bool {
        self.bans.get(ip).map_or(false, |ban| now < ban.expires_at)
    }

    /// Manually ban an IP.
    pub fn ban_ip(&mut self, ip: &str, reason: String, now: u64, duration_sec: u64) {
        let ban = IpBan {
            ip: ip.to_string(),
            reason,
            banned_at: now,
            expires_at: now + duration_sec,
        };
        self.bans.insert(ip.to_string(), ban);
    }

    /// Remove a ban.
    pub fn unban_ip(&mut self, ip: &str) {
        self.bans.remove(ip);
    }

    /// Remove expired bans and stale rate-limiter entries.
    pub fn cleanup_expired(&mut self, now: u64) {
        self.bans.retain(|_, ban| now < ban.expires_at);

        // Remove entries that have been idle for longer than the failure window.
        let window = self.config.auth_failure_window_sec;
        self.entries
            .retain(|_, entry| now.saturating_sub(entry.last_refill) < window * 2);
    }

    /// List all active bans.
    #[must_use]
    pub fn active_bans(&self) -> Vec<&IpBan> {
        self.bans.values().collect()
    }
}
