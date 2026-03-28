//! Per-application audio sessions (PulseAudio/PipeWire session model).
//!
//! Tracks individual application audio streams with volume, mute, peak level,
//! and stream type classification. The [`AudioSessionManager`] aggregates all
//! active sessions and provides master volume control.

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for an application audio session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SessionId({})", self.0)
    }
}

/// Classification of an audio stream's purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    /// Music, video, game audio playback.
    Playback,
    /// Microphone / line-in capture.
    Capture,
    /// Short notification sounds (popups, alerts).
    Notification,
    /// Voice/video call audio (both directions).
    Communication,
    /// Desktop environment system sounds.
    System,
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Playback => write!(f, "Playback"),
            Self::Capture => write!(f, "Capture"),
            Self::Notification => write!(f, "Notification"),
            Self::Communication => write!(f, "Communication"),
            Self::System => write!(f, "System"),
        }
    }
}

/// Events emitted by the session manager when session state changes.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A session's volume was changed.
    VolumeChanged {
        session_id: SessionId,
        volume: f32,
    },
    /// A session's mute state was toggled.
    MuteChanged {
        session_id: SessionId,
        muted: bool,
    },
    /// A new session was created.
    SessionCreated {
        session_id: SessionId,
        app_id: String,
    },
    /// A session ended.
    SessionEnded {
        session_id: SessionId,
    },
    /// A session's peak level was updated.
    PeakUpdated {
        session_id: SessionId,
        peak_level: f32,
    },
}

impl fmt::Display for SessionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VolumeChanged { session_id, volume } => {
                write!(f, "VolumeChanged({session_id}, {volume:.2})")
            }
            Self::MuteChanged { session_id, muted } => {
                write!(f, "MuteChanged({session_id}, muted={muted})")
            }
            Self::SessionCreated { session_id, app_id } => {
                write!(f, "SessionCreated({session_id}, {app_id})")
            }
            Self::SessionEnded { session_id } => {
                write!(f, "SessionEnded({session_id})")
            }
            Self::PeakUpdated { session_id, peak_level } => {
                write!(f, "PeakUpdated({session_id}, {peak_level:.4})")
            }
        }
    }
}

/// Per-application audio session state.
///
/// Models a PulseAudio/PipeWire sink-input or source-output:
/// each application that produces or consumes audio gets its own session
/// with independent volume, mute, and metering.
#[derive(Debug, Clone)]
pub struct AppSession {
    /// Unique session identifier.
    pub id: SessionId,
    /// Application identifier (e.g. desktop file basename, PID-based).
    pub app_id: String,
    /// Human-readable display name for the application.
    pub display_name: String,
    /// Optional icon name or path for the application.
    pub icon: Option<String>,
    /// Volume level, linear 0.0 (silence) to 1.0 (full).
    pub volume: f32,
    /// Whether this session is muted.
    pub muted: bool,
    /// Current peak audio level (0.0 to 1.0), updated per buffer period.
    pub peak_level: f32,
    /// The type/purpose of this audio stream.
    pub stream_type: StreamType,
}

impl AppSession {
    /// Create a new application audio session.
    #[must_use]
    pub fn new(
        id: SessionId,
        app_id: String,
        display_name: String,
        stream_type: StreamType,
    ) -> Self {
        Self {
            id,
            app_id,
            display_name,
            icon: None,
            volume: 1.0,
            muted: false,
            peak_level: 0.0,
            stream_type,
        }
    }

    /// Set the volume, clamping to 0.0..=1.0.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set the mute state.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Update the peak level, clamping to 0.0..=1.0.
    pub fn update_peak(&mut self, peak: f32) {
        self.peak_level = peak.clamp(0.0, 1.0);
    }

    /// The effective volume taking mute into account.
    #[must_use]
    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }
}

impl fmt::Display for AppSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppSession({}, \"{}\", vol={:.0}%{}, {}, peak={:.4})",
            self.id,
            self.display_name,
            self.volume * 100.0,
            if self.muted { " [MUTED]" } else { "" },
            self.stream_type,
            self.peak_level,
        )
    }
}

/// Manages all active per-application audio sessions.
///
/// Modelled after the PulseAudio/PipeWire session model: each application
/// stream is registered as a session and can be independently controlled.
/// The manager also provides system-wide master volume and mute.
pub struct AudioSessionManager {
    sessions: HashMap<SessionId, AppSession>,
    next_id: u64,
    /// System-wide master volume (0.0 to 1.0).
    master_volume: f32,
    /// System-wide master mute.
    master_mute: bool,
    /// Pending events since last drain.
    events: Vec<SessionEvent>,
}

impl AudioSessionManager {
    /// Create a new session manager with master volume at 100%.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
            master_volume: 1.0,
            master_mute: false,
            events: Vec::new(),
        }
    }

    /// Register a new application audio session.
    ///
    /// Returns the assigned [`SessionId`].
    pub fn register(
        &mut self,
        app_id: String,
        display_name: String,
        stream_type: StreamType,
    ) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        let session = AppSession::new(id, app_id.clone(), display_name, stream_type);
        self.sessions.insert(id, session);
        self.events.push(SessionEvent::SessionCreated {
            session_id: id,
            app_id,
        });
        id
    }

    /// Unregister a session by its id.
    ///
    /// Returns the removed session, or `None` if not found.
    pub fn unregister(&mut self, session_id: SessionId) -> Option<AppSession> {
        let removed = self.sessions.remove(&session_id);
        if removed.is_some() {
            self.events.push(SessionEvent::SessionEnded { session_id });
        }
        removed
    }

    /// Get a reference to a session by id.
    #[must_use]
    pub fn get(&self, session_id: SessionId) -> Option<&AppSession> {
        self.sessions.get(&session_id)
    }

    /// Get a mutable reference to a session by id.
    pub fn get_mut(&mut self, session_id: SessionId) -> Option<&mut AppSession> {
        self.sessions.get_mut(&session_id)
    }

    /// Set the volume for a session. Returns `false` if not found.
    pub fn set_volume(&mut self, session_id: SessionId, volume: f32) -> bool {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.set_volume(volume);
            self.events.push(SessionEvent::VolumeChanged {
                session_id,
                volume: session.volume,
            });
            true
        } else {
            false
        }
    }

    /// Set the mute state for a session. Returns `false` if not found.
    pub fn set_mute(&mut self, session_id: SessionId, muted: bool) -> bool {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.set_muted(muted);
            self.events.push(SessionEvent::MuteChanged {
                session_id,
                muted,
            });
            true
        } else {
            false
        }
    }

    /// Update the peak level for a session. Returns `false` if not found.
    pub fn update_peak(&mut self, session_id: SessionId, peak: f32) -> bool {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.update_peak(peak);
            self.events.push(SessionEvent::PeakUpdated {
                session_id,
                peak_level: session.peak_level,
            });
            true
        } else {
            false
        }
    }

    /// Get a snapshot of all active sessions.
    #[must_use]
    pub fn get_sessions(&self) -> Vec<AppSession> {
        self.sessions.values().cloned().collect()
    }

    /// Number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get sessions filtered by stream type.
    #[must_use]
    pub fn sessions_by_type(&self, stream_type: StreamType) -> Vec<&AppSession> {
        self.sessions
            .values()
            .filter(|s| s.stream_type == stream_type)
            .collect()
    }

    /// The current master volume (0.0 to 1.0).
    #[must_use]
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Set the system-wide master volume, clamped to 0.0..=1.0.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// Whether the system-wide master mute is active.
    #[must_use]
    pub fn master_mute(&self) -> bool {
        self.master_mute
    }

    /// Set the system-wide master mute state.
    pub fn set_master_mute(&mut self, muted: bool) {
        self.master_mute = muted;
    }

    /// Drain all pending events since the last call.
    pub fn drain_events(&mut self) -> Vec<SessionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Compute the effective output volume for a session,
    /// considering session volume, session mute, master volume, and master mute.
    #[must_use]
    pub fn effective_volume(&self, session_id: SessionId) -> f32 {
        if self.master_mute {
            return 0.0;
        }
        match self.sessions.get(&session_id) {
            Some(session) => session.effective_volume() * self.master_volume,
            None => 0.0,
        }
    }
}

impl Default for AudioSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AudioSessionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioSessionManager({} sessions, master={:.0}%{})",
            self.sessions.len(),
            self.master_volume * 100.0,
            if self.master_mute { " [MUTED]" } else { "" },
        )
    }
}
