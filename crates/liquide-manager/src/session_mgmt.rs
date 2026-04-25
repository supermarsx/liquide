//! Session management operations.

use serde::{Deserialize, Serialize};

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Locked,
    Suspended,
    Disconnecting,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Locked => write!(f, "locked"),
            Self::Suspended => write!(f, "suspended"),
            Self::Disconnecting => write!(f, "disconnecting"),
        }
    }
}

/// Session summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub user: String,
    pub server: String,
    pub status: SessionStatus,
    pub duration_seconds: u64,
    pub resolution: String,
    pub encoder: String,
    pub transport: String,
    pub latency_ms: f32,
    pub fps: f32,
    pub bandwidth_bps: u64,
}

/// Detailed session information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub client_platform: String,
    pub client_version: String,
    pub client_ip: String,
    pub features: SessionFeatures,
    pub recording: bool,
}

/// Active features for a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFeatures {
    pub clipboard: bool,
    pub audio: bool,
    pub usb: bool,
    pub camera: bool,
    pub printing: bool,
}

/// Session store tracking known sessions across all servers.
pub struct SessionStore {
    sessions: Vec<SessionRecord>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    session_id: String,
    user: String,
    server: String,
    status: SessionStatus,
    started_at: u64,
    latency_ms: f32,
    fps: f32,
    bandwidth_bps: u64,
    resolution: String,
    encoder: String,
    transport: String,
    lock_message: Option<String>,
}

impl SessionStore {
    /// Create an empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    /// Register or update a session.
    pub fn upsert(&mut self, session_id: String, user: String, server: String, started_at: u64) {
        if let Some(s) = self
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            s.user = user;
            s.server = server;
        } else {
            self.sessions.push(SessionRecord {
                session_id,
                user,
                server,
                status: SessionStatus::Active,
                started_at,
                latency_ms: 0.0,
                fps: 0.0,
                bandwidth_bps: 0,
                resolution: "1920x1080".to_string(),
                encoder: "h264".to_string(),
                transport: "quic".to_string(),
                lock_message: None,
            });
        }
    }

    /// Update live metrics for a session.
    pub fn update_metrics(&mut self, session_id: &str, latency: f32, fps: f32, bandwidth: u64) {
        if let Some(s) = self
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            s.latency_ms = latency;
            s.fps = fps;
            s.bandwidth_bps = bandwidth;
        }
    }

    /// Lock a session.
    pub fn lock_session(&mut self, session_id: &str, message: Option<String>) -> crate::Result<()> {
        let s = self
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| crate::ManagerError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;
        s.status = SessionStatus::Locked;
        s.lock_message = message;
        Ok(())
    }

    /// Unlock a session.
    pub fn unlock_session(&mut self, session_id: &str) -> crate::Result<()> {
        let s = self
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| crate::ManagerError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;
        s.status = SessionStatus::Active;
        s.lock_message = None;
        Ok(())
    }

    /// Remove a session (disconnected).
    pub fn remove(&mut self, session_id: &str) {
        self.sessions.retain(|s| s.session_id != session_id);
    }

    /// List all sessions.
    #[must_use]
    pub fn list(&self, now: u64) -> Vec<SessionSummary> {
        self.sessions
            .iter()
            .map(|s| SessionSummary {
                session_id: s.session_id.clone(),
                user: s.user.clone(),
                server: s.server.clone(),
                status: s.status,
                duration_seconds: now.saturating_sub(s.started_at),
                resolution: s.resolution.clone(),
                encoder: s.encoder.clone(),
                transport: s.transport.clone(),
                latency_ms: s.latency_ms,
                fps: s.fps,
                bandwidth_bps: s.bandwidth_bps,
            })
            .collect()
    }

    /// List sessions for a specific user.
    #[must_use]
    pub fn sessions_for_user(&self, user: &str, now: u64) -> Vec<SessionSummary> {
        self.list(now)
            .into_iter()
            .filter(|s| s.user == user)
            .collect()
    }

    /// Get a session by ID.
    #[must_use]
    pub fn get(&self, session_id: &str, now: u64) -> Option<SessionSummary> {
        self.sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .map(|s| SessionSummary {
                session_id: s.session_id.clone(),
                user: s.user.clone(),
                server: s.server.clone(),
                status: s.status,
                duration_seconds: now.saturating_sub(s.started_at),
                resolution: s.resolution.clone(),
                encoder: s.encoder.clone(),
                transport: s.transport.clone(),
                latency_ms: s.latency_ms,
                fps: s.fps,
                bandwidth_bps: s.bandwidth_bps,
            })
    }

    /// Total session count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Unique user count.
    #[must_use]
    pub fn unique_users(&self) -> usize {
        let mut users: Vec<&str> = self.sessions.iter().map(|s| s.user.as_str()).collect();
        users.sort();
        users.dedup();
        users.len()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
