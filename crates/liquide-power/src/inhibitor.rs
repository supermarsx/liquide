//! Sleep/suspend inhibitor registry.
//!
//! Applications and system services can register inhibitors to prevent the
//! system from suspending or turning off the display while important work
//! is in progress (e.g., video playback, downloads, presentations).
//!
//! Modelled after logind's `Inhibit()` interface.

use std::time::Instant;

// ---------------------------------------------------------------------------
// Inhibit reason
// ---------------------------------------------------------------------------

/// Why an application is inhibiting suspend/idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InhibitReason {
    /// Playing video content (prevents screen-off and suspend).
    VideoPlayback,
    /// Playing audio (prevents suspend).
    AudioPlayback,
    /// An active download is in progress.
    Download,
    /// A presentation or screen-share is running.
    Presentation,
    /// The user explicitly requested the system stay awake.
    UserRequest,
    /// A system update or package installation is running.
    SystemUpdate,
}

impl std::fmt::Display for InhibitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::VideoPlayback => "video-playback",
            Self::AudioPlayback => "audio-playback",
            Self::Download => "download",
            Self::Presentation => "presentation",
            Self::UserRequest => "user-request",
            Self::SystemUpdate => "system-update",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// What is being inhibited
// ---------------------------------------------------------------------------

/// Which system action is inhibited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InhibitWhat {
    /// Prevent the system from going to sleep/suspend.
    Sleep,
    /// Prevent the display from turning off or dimming.
    Idle,
    /// Prevent both sleep and idle.
    Both,
}

// ---------------------------------------------------------------------------
// Inhibitor
// ---------------------------------------------------------------------------

/// A single inhibit registration.
#[derive(Debug, Clone)]
pub struct Inhibitor {
    /// Unique identifier for this inhibitor.
    pub id: u64,
    /// Which application registered the inhibitor.
    pub app_id: String,
    /// Why the inhibitor was registered.
    pub reason: InhibitReason,
    /// Human-readable description.
    pub description: String,
    /// What is being inhibited.
    pub what: InhibitWhat,
    /// When the inhibitor was created.
    pub created_at: Instant,
}

// ---------------------------------------------------------------------------
// InhibitorRegistry
// ---------------------------------------------------------------------------

/// Registry of active inhibitors. The shell or power manager queries this
/// to decide whether suspend/idle actions should be suppressed.
pub struct InhibitorRegistry {
    inhibitors: Vec<Inhibitor>,
    next_id: u64,
}

impl InhibitorRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inhibitors: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a new inhibitor. Returns the unique id for later removal.
    pub fn add(
        &mut self,
        app_id: impl Into<String>,
        reason: InhibitReason,
        description: impl Into<String>,
        what: InhibitWhat,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.inhibitors.push(Inhibitor {
            id,
            app_id: app_id.into(),
            reason,
            description: description.into(),
            what,
            created_at: Instant::now(),
        });
        id
    }

    /// Remove an inhibitor by its id. Returns `true` if it was found and
    /// removed.
    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(pos) = self.inhibitors.iter().position(|i| i.id == id) {
            self.inhibitors.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove all inhibitors registered by a given application.
    pub fn remove_by_app(&mut self, app_id: &str) {
        self.inhibitors.retain(|i| i.app_id != app_id);
    }

    /// Check if sleep/suspend is currently inhibited.
    pub fn is_sleep_inhibited(&self) -> bool {
        self.inhibitors
            .iter()
            .any(|i| matches!(i.what, InhibitWhat::Sleep | InhibitWhat::Both))
    }

    /// Check if idle (display dim/off) is currently inhibited.
    pub fn is_idle_inhibited(&self) -> bool {
        self.inhibitors
            .iter()
            .any(|i| matches!(i.what, InhibitWhat::Idle | InhibitWhat::Both))
    }

    /// Convenience: check if *any* inhibitor is active.
    pub fn is_inhibited(&self) -> bool {
        !self.inhibitors.is_empty()
    }

    /// Return a snapshot of all active inhibitors.
    pub fn active_inhibitors(&self) -> &[Inhibitor] {
        &self.inhibitors
    }

    /// Number of active inhibitors.
    pub fn count(&self) -> usize {
        self.inhibitors.len()
    }

    /// Find inhibitors matching a specific reason.
    pub fn find_by_reason(&self, reason: InhibitReason) -> Vec<&Inhibitor> {
        self.inhibitors
            .iter()
            .filter(|i| i.reason == reason)
            .collect()
    }
}

impl Default for InhibitorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_not_inhibited() {
        let reg = InhibitorRegistry::new();
        assert!(!reg.is_inhibited());
        assert!(!reg.is_sleep_inhibited());
        assert!(!reg.is_idle_inhibited());
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn add_and_query() {
        let mut reg = InhibitorRegistry::new();
        let id = reg.add(
            "vlc",
            InhibitReason::VideoPlayback,
            "Playing movie",
            InhibitWhat::Both,
        );
        assert!(id > 0);
        assert!(reg.is_inhibited());
        assert!(reg.is_sleep_inhibited());
        assert!(reg.is_idle_inhibited());
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn sleep_only_inhibitor() {
        let mut reg = InhibitorRegistry::new();
        reg.add(
            "wget",
            InhibitReason::Download,
            "Downloading ISO",
            InhibitWhat::Sleep,
        );
        assert!(reg.is_sleep_inhibited());
        assert!(!reg.is_idle_inhibited());
    }

    #[test]
    fn idle_only_inhibitor() {
        let mut reg = InhibitorRegistry::new();
        reg.add(
            "presentation",
            InhibitReason::Presentation,
            "Slideshow",
            InhibitWhat::Idle,
        );
        assert!(!reg.is_sleep_inhibited());
        assert!(reg.is_idle_inhibited());
    }

    #[test]
    fn remove_by_id() {
        let mut reg = InhibitorRegistry::new();
        let id = reg.add(
            "app",
            InhibitReason::UserRequest,
            "keep awake",
            InhibitWhat::Sleep,
        );
        assert!(reg.remove(id));
        assert!(!reg.is_inhibited());
        // Removing again returns false.
        assert!(!reg.remove(id));
    }

    #[test]
    fn remove_by_app() {
        let mut reg = InhibitorRegistry::new();
        reg.add(
            "firefox",
            InhibitReason::VideoPlayback,
            "YouTube",
            InhibitWhat::Both,
        );
        reg.add(
            "firefox",
            InhibitReason::Download,
            "Update",
            InhibitWhat::Sleep,
        );
        reg.add(
            "vlc",
            InhibitReason::AudioPlayback,
            "Music",
            InhibitWhat::Sleep,
        );
        assert_eq!(reg.count(), 3);

        reg.remove_by_app("firefox");
        assert_eq!(reg.count(), 1);
        assert_eq!(reg.active_inhibitors()[0].app_id, "vlc");
    }

    #[test]
    fn ids_are_unique() {
        let mut reg = InhibitorRegistry::new();
        let id1 = reg.add("a", InhibitReason::UserRequest, "1", InhibitWhat::Sleep);
        let id2 = reg.add("b", InhibitReason::UserRequest, "2", InhibitWhat::Sleep);
        let id3 = reg.add("c", InhibitReason::UserRequest, "3", InhibitWhat::Sleep);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn active_inhibitors_snapshot() {
        let mut reg = InhibitorRegistry::new();
        reg.add("app1", InhibitReason::Download, "file", InhibitWhat::Sleep);
        reg.add(
            "app2",
            InhibitReason::SystemUpdate,
            "apt",
            InhibitWhat::Both,
        );
        let active = reg.active_inhibitors();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].app_id, "app1");
        assert_eq!(active[1].app_id, "app2");
    }

    #[test]
    fn find_by_reason() {
        let mut reg = InhibitorRegistry::new();
        reg.add("a", InhibitReason::VideoPlayback, "v1", InhibitWhat::Both);
        reg.add("b", InhibitReason::Download, "d1", InhibitWhat::Sleep);
        reg.add("c", InhibitReason::VideoPlayback, "v2", InhibitWhat::Idle);
        let found = reg.find_by_reason(InhibitReason::VideoPlayback);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn inhibit_reason_display() {
        assert_eq!(InhibitReason::VideoPlayback.to_string(), "video-playback");
        assert_eq!(InhibitReason::AudioPlayback.to_string(), "audio-playback");
        assert_eq!(InhibitReason::Download.to_string(), "download");
        assert_eq!(InhibitReason::Presentation.to_string(), "presentation");
        assert_eq!(InhibitReason::UserRequest.to_string(), "user-request");
        assert_eq!(InhibitReason::SystemUpdate.to_string(), "system-update");
    }
}
