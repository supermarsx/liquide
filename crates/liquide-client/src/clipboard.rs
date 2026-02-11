//! Clipboard synchronisation between client and server.

use std::fmt;

/// Direction of clipboard sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMode {
    Bidirectional,
    ClientToServer,
    ServerToClient,
    Disabled,
}

impl fmt::Display for ClipboardMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Bidirectional => "Bidirectional",
            Self::ClientToServer => "ClientToServer",
            Self::ServerToClient => "ServerToClient",
            Self::Disabled => "Disabled",
        };
        f.write_str(label)
    }
}

/// Direction of the most recent clipboard sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    ToServer,
    ToClient,
}

/// A clipboard content entry in the history.
#[derive(Debug, Clone)]
pub struct ClipboardContent {
    pub mime_type: String,
    pub size_bytes: u64,
    pub preview: Option<String>,
}

/// Tracks clipboard synchronisation state and history.
pub struct ClipboardSync {
    mode: ClipboardMode,
    history: Vec<ClipboardContent>,
    max_history: usize,
    last_sync_direction: Option<SyncDirection>,
}

impl ClipboardSync {
    /// Create a new clipboard sync tracker.
    #[must_use]
    pub fn new(mode: ClipboardMode, max_history: usize) -> Self {
        Self {
            mode,
            history: Vec::new(),
            max_history,
            last_sync_direction: None,
        }
    }

    /// Current clipboard sharing mode.
    #[must_use]
    pub fn mode(&self) -> ClipboardMode {
        self.mode
    }

    /// Change the sharing mode.
    pub fn set_mode(&mut self, mode: ClipboardMode) {
        self.mode = mode;
    }

    /// Record a clipboard sync event. Returns `false` if the current mode
    /// does not allow the specified direction.
    pub fn record_sync(&mut self, direction: SyncDirection, content: ClipboardContent) -> bool {
        let allowed = match (self.mode, direction) {
            (ClipboardMode::Disabled, _) => false,
            (ClipboardMode::Bidirectional, _) => true,
            (ClipboardMode::ClientToServer, SyncDirection::ToServer) => true,
            (ClipboardMode::ServerToClient, SyncDirection::ToClient) => true,
            _ => false,
        };
        if !allowed {
            return false;
        }

        self.last_sync_direction = Some(direction);
        self.history.push(content);
        while self.history.len() > self.max_history {
            self.history.remove(0);
        }
        true
    }

    /// Clipboard content history.
    #[must_use]
    pub fn history(&self) -> &[ClipboardContent] {
        &self.history
    }

    /// Direction of the most recent sync, if any.
    #[must_use]
    pub fn last_direction(&self) -> Option<SyncDirection> {
        self.last_sync_direction
    }

    /// Clear the history and last-direction state.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.last_sync_direction = None;
    }

    /// Whether clipboard sync is enabled at all.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.mode != ClipboardMode::Disabled
    }
}
