//! Full-relay session management for proxying traffic through the gateway.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;

use crate::config::RelayConfig;
use crate::{GatewayError, Result};

/// Byte counters shared with a running relay forwarding task.
///
/// The forwarding task updates these atomically as it copies bytes; the
/// [`RelaySession`] reads them to report live traffic without owning the socket.
#[derive(Debug, Default)]
pub struct RelayCounters {
    /// Bytes copied from client to server.
    pub client_to_server: std::sync::atomic::AtomicU64,
    /// Bytes copied from server to client.
    pub server_to_client: std::sync::atomic::AtomicU64,
}

/// Forward bytes bidirectionally between an authenticated client stream and a
/// backend session stream until either side closes.
///
/// This is the post-login data path: the gateway no longer drops the TLS
/// stream after routing — it splices it to the backend so desktop frames and
/// input events survive the handshake. Byte totals are recorded into
/// `counters` as they flow.
pub async fn forward_bidirectional<C, S>(
    mut client: C,
    mut server: S,
    counters: Arc<RelayCounters>,
) -> std::io::Result<()>
where
    C: AsyncRead + AsyncWrite + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    use std::sync::atomic::Ordering;

    let (c2s, s2c) = tokio::io::copy_bidirectional(&mut client, &mut server).await?;
    counters.client_to_server.fetch_add(c2s, Ordering::Relaxed);
    counters.server_to_client.fetch_add(s2c, Ordering::Relaxed);
    Ok(())
}

/// Handle to a live relay's shared traffic counters, kept by the manager so
/// status reporting reflects bytes forwarded by the background task.
type SharedCounters = Arc<RelayCounters>;

/// A lock-protected registry of live relay counters, keyed by relay id.
pub type LiveRelayTable = Arc<Mutex<HashMap<String, SharedCounters>>>;

/// A relay session forwarding traffic between a client and a server.
pub struct RelaySession {
    relay_id: String,
    client_connection_id: String,
    server_connection_id: String,
    started_at: u64,
    bytes_forwarded_in: u64,
    bytes_forwarded_out: u64,
    active: bool,
}

impl RelaySession {
    /// Create a new active relay session.
    #[must_use]
    pub fn new(
        relay_id: String,
        client_connection_id: String,
        server_connection_id: String,
        started_at: u64,
    ) -> Self {
        Self {
            relay_id,
            client_connection_id,
            server_connection_id,
            started_at,
            bytes_forwarded_in: 0,
            bytes_forwarded_out: 0,
            active: true,
        }
    }

    /// Relay session identifier.
    #[must_use]
    pub fn relay_id(&self) -> &str {
        &self.relay_id
    }

    /// Client-side connection identifier.
    #[must_use]
    pub fn client_connection_id(&self) -> &str {
        &self.client_connection_id
    }

    /// Server-side connection identifier.
    #[must_use]
    pub fn server_connection_id(&self) -> &str {
        &self.server_connection_id
    }

    /// Epoch timestamp when the relay started.
    #[must_use]
    pub fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Total bytes forwarded from client to server.
    #[must_use]
    pub fn bytes_forwarded_in(&self) -> u64 {
        self.bytes_forwarded_in
    }

    /// Total bytes forwarded from server to client.
    #[must_use]
    pub fn bytes_forwarded_out(&self) -> u64 {
        self.bytes_forwarded_out
    }

    /// Record traffic flowing through the relay.
    pub fn record_traffic(&mut self, bytes_in: u64, bytes_out: u64) {
        self.bytes_forwarded_in += bytes_in;
        self.bytes_forwarded_out += bytes_out;
    }

    /// Terminate this relay session.
    pub fn terminate(&mut self) {
        self.active = false;
    }

    /// Whether the relay is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Duration of the relay in seconds, given the current timestamp.
    #[must_use]
    pub fn duration_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.started_at)
    }
}

/// Manages all active relay sessions.
pub struct RelayManager {
    sessions: HashMap<String, RelaySession>,
    config: RelayConfig,
    next_id: u64,
    /// Live byte counters for relays whose forwarding runs in a background
    /// task. Keyed by relay id; updated by the task, read for status.
    live_counters: HashMap<String, SharedCounters>,
}

impl RelayManager {
    /// Create a new relay manager.
    #[must_use]
    pub fn new(config: RelayConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            config,
            next_id: 1,
            live_counters: HashMap::new(),
        }
    }

    /// Create a relay session and return its id together with a shared counter
    /// handle to give to the forwarding task.
    ///
    /// The same capacity/enabled checks as [`create_relay`](Self::create_relay)
    /// apply.
    pub fn create_relay_with_counters(
        &mut self,
        client_connection_id: String,
        server_connection_id: String,
        timestamp: u64,
    ) -> Result<(String, SharedCounters)> {
        let relay_id =
            self.create_relay(client_connection_id, server_connection_id, timestamp)?;
        let counters: SharedCounters = Arc::new(RelayCounters::default());
        self.live_counters.insert(relay_id.clone(), counters.clone());
        Ok((relay_id, counters))
    }

    /// Snapshot the live byte totals for a relay, if it has a counter handle.
    #[must_use]
    pub fn live_traffic(&self, relay_id: &str) -> Option<(u64, u64)> {
        use std::sync::atomic::Ordering;
        self.live_counters.get(relay_id).map(|c| {
            (
                c.client_to_server.load(Ordering::Relaxed),
                c.server_to_client.load(Ordering::Relaxed),
            )
        })
    }

    /// Create a new relay session between a client and server connection.
    pub fn create_relay(
        &mut self,
        client_connection_id: String,
        server_connection_id: String,
        timestamp: u64,
    ) -> Result<String> {
        if !self.config.enabled {
            return Err(GatewayError::Internal(
                "relay subsystem is disabled".to_string(),
            ));
        }

        let active = self.active_count();
        if active >= self.config.max_relay_sessions as usize {
            return Err(GatewayError::RelayCapacityExceeded {
                max_sessions: self.config.max_relay_sessions,
            });
        }

        let relay_id = format!("relay-{}", self.next_id);
        self.next_id += 1;

        let session = RelaySession::new(
            relay_id.clone(),
            client_connection_id,
            server_connection_id,
            timestamp,
        );
        self.sessions.insert(relay_id.clone(), session);
        Ok(relay_id)
    }

    /// Terminate a relay session.
    pub fn terminate_relay(&mut self, relay_id: &str) -> Result<()> {
        let session =
            self.sessions
                .get_mut(relay_id)
                .ok_or_else(|| GatewayError::SessionNotFound {
                    session_id: relay_id.to_string(),
                })?;
        session.terminate();
        Ok(())
    }

    /// Record data passing through a relay.
    pub fn relay_data(&mut self, relay_id: &str, bytes_in: u64, bytes_out: u64) -> Result<()> {
        let session =
            self.sessions
                .get_mut(relay_id)
                .ok_or_else(|| GatewayError::SessionNotFound {
                    session_id: relay_id.to_string(),
                })?;
        session.record_traffic(bytes_in, bytes_out);
        Ok(())
    }

    /// Number of active relay sessions.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.is_active()).count()
    }

    /// Total bandwidth currently in flight across all sessions (bytes).
    #[must_use]
    pub fn total_bandwidth(&self) -> u64 {
        self.sessions
            .values()
            .filter(|s| s.is_active())
            .map(|s| s.bytes_forwarded_in + s.bytes_forwarded_out)
            .sum()
    }

    /// Get a reference to a relay session.
    #[must_use]
    pub fn get(&self, relay_id: &str) -> Option<&RelaySession> {
        self.sessions.get(relay_id)
    }
}
