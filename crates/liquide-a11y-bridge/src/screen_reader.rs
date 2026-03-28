//! Screen reader integration for the bridge layer.
//!
//! Provides the [`ScreenReaderBridge`] trait for announcing text to assistive
//! technology, WAI-ARIA live-region modelling, and navigation hints for the
//! screen reader's virtual cursor.

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

/// Priority for screen reader announcements (WAI-ARIA `aria-live`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnouncePriority {
    /// The announcement is polite — queued after current speech finishes
    /// (`aria-live="polite"`).
    Polite,
    /// The announcement is assertive — interrupts current speech immediately
    /// (`aria-live="assertive"`).
    Assertive,
    /// No announcement should be made (`aria-live="off"`).
    Off,
}

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

/// The operating mode of the screen reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenReaderMode {
    /// Normal mode — keyboard events are processed by the application.
    Normal,
    /// Focus mode (forms mode) — keyboard events go to the focused widget.
    FocusMode,
    /// Browse mode (virtual cursor) — arrow keys move the screen reader
    /// cursor through the accessibility tree.
    BrowseMode,
}

// ---------------------------------------------------------------------------
// Live region
// ---------------------------------------------------------------------------

/// Describes a WAI-ARIA live region.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveRegion {
    /// The live-region politeness: `polite`, `assertive`, or `off`.
    pub politeness: AnnouncePriority,
    /// `aria-atomic` — when `true`, the entire region is presented as a
    /// single unit on change; when `false`, only the changed nodes are
    /// announced.
    pub atomic: bool,
    /// `aria-relevant` — which mutations are relevant.  Valid tokens:
    /// `"additions"`, `"removals"`, `"text"`, `"all"`.
    pub relevant: Vec<String>,
    /// The node ID of the live region container.
    pub node_id: u64,
}

impl LiveRegion {
    /// Create a new live region with default settings (polite, non-atomic,
    /// relevant=\["additions", "text"\]).
    #[must_use]
    pub fn new(node_id: u64) -> Self {
        Self {
            politeness: AnnouncePriority::Polite,
            atomic: false,
            relevant: vec!["additions".to_string(), "text".to_string()],
            node_id,
        }
    }

    /// Create an assertive live region (e.g. for alerts).
    #[must_use]
    pub fn assertive(node_id: u64) -> Self {
        Self {
            politeness: AnnouncePriority::Assertive,
            atomic: true,
            relevant: vec!["all".to_string()],
            node_id,
        }
    }

    /// Check if the region is interested in a particular mutation kind.
    #[must_use]
    pub fn is_relevant(&self, kind: &str) -> bool {
        self.relevant.iter().any(|r| r == "all" || r == kind)
    }
}

// ---------------------------------------------------------------------------
// Navigation hint
// ---------------------------------------------------------------------------

/// A hint for the screen reader's virtual cursor movement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationHint {
    /// Move to the next element.
    NextElement,
    /// Move to the previous element.
    PreviousElement,
    /// Move to the next heading.
    NextHeading,
    /// Move to the previous heading.
    PreviousHeading,
    /// Move to the next landmark.
    NextLandmark,
    /// Move to the previous landmark.
    PreviousLandmark,
    /// Move to the next focusable element.
    NextFocusable,
    /// Move to the previous focusable element.
    PreviousFocusable,
    /// Move to the next link.
    NextLink,
    /// Move to the previous link.
    PreviousLink,
    /// Move to the next form field.
    NextFormField,
    /// Move to the previous form field.
    PreviousFormField,
}

// ---------------------------------------------------------------------------
// Screen reader bridge trait
// ---------------------------------------------------------------------------

/// Trait for the screen reader bridge — the channel between the desktop
/// environment and the assistive technology.
///
/// Implementations may communicate with AT-SPI (Linux), the platform
/// accessibility service, or a custom screen reader engine.
pub trait ScreenReaderBridge: Send {
    /// Announce `text` to the user with the given priority.
    fn announce(&mut self, text: &str, priority: AnnouncePriority);

    /// Interrupt any current speech output.
    fn interrupt(&mut self);

    /// Set the screen reader's operating mode.
    fn set_mode(&mut self, mode: ScreenReaderMode);

    /// Get the current operating mode.
    fn current_mode(&self) -> ScreenReaderMode;

    /// Check whether a screen reader is currently active and listening.
    fn is_active(&self) -> bool;
}

// ---------------------------------------------------------------------------
// LoggingScreenReader — test / debug implementation
// ---------------------------------------------------------------------------

/// A logging implementation of [`ScreenReaderBridge`] that records all
/// announcements for testing.
#[derive(Debug, Clone)]
pub struct LoggingScreenReader {
    pub messages: Vec<(String, AnnouncePriority)>,
    pub mode: ScreenReaderMode,
    pub active: bool,
    pub interrupted: bool,
}

impl LoggingScreenReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            mode: ScreenReaderMode::Normal,
            active: true,
            interrupted: false,
        }
    }

    /// Get all recorded messages.
    #[must_use]
    pub fn messages(&self) -> &[(String, AnnouncePriority)] {
        &self.messages
    }

    /// Clear recorded messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.interrupted = false;
    }
}

impl Default for LoggingScreenReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenReaderBridge for LoggingScreenReader {
    fn announce(&mut self, text: &str, priority: AnnouncePriority) {
        if priority != AnnouncePriority::Off {
            self.messages.push((text.to_string(), priority));
        }
    }

    fn interrupt(&mut self) {
        self.interrupted = true;
    }

    fn set_mode(&mut self, mode: ScreenReaderMode) {
        self.mode = mode;
    }

    fn current_mode(&self) -> ScreenReaderMode {
        self.mode
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

// ---------------------------------------------------------------------------
// LiveRegionMonitor
// ---------------------------------------------------------------------------

/// Tracks live regions and produces announcements when their content changes.
#[derive(Debug, Clone)]
pub struct LiveRegionMonitor {
    regions: Vec<LiveRegion>,
}

impl LiveRegionMonitor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Register a live region.
    pub fn add_region(&mut self, region: LiveRegion) {
        self.regions.push(region);
    }

    /// Remove a live region by node ID.
    pub fn remove_region(&mut self, node_id: u64) {
        self.regions.retain(|r| r.node_id != node_id);
    }

    /// Get a live region by node ID.
    #[must_use]
    pub fn get_region(&self, node_id: u64) -> Option<&LiveRegion> {
        self.regions.iter().find(|r| r.node_id == node_id)
    }

    /// Number of registered live regions.
    #[must_use]
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Given a mutation kind and node ID, return the announcement priority
    /// (if any).  Returns `None` if the region does not exist or the
    /// mutation is not relevant.
    #[must_use]
    pub fn should_announce(&self, node_id: u64, mutation_kind: &str) -> Option<AnnouncePriority> {
        self.regions
            .iter()
            .find(|r| r.node_id == node_id && r.is_relevant(mutation_kind))
            .map(|r| r.politeness)
    }
}

impl Default for LiveRegionMonitor {
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
    fn logging_reader_announce() {
        let mut sr = LoggingScreenReader::new();
        sr.announce("Hello", AnnouncePriority::Polite);
        sr.announce("World", AnnouncePriority::Assertive);
        assert_eq!(sr.messages().len(), 2);
        assert_eq!(sr.messages()[0].0, "Hello");
        assert_eq!(sr.messages()[0].1, AnnouncePriority::Polite);
        assert_eq!(sr.messages()[1].1, AnnouncePriority::Assertive);
    }

    #[test]
    fn logging_reader_off_is_silent() {
        let mut sr = LoggingScreenReader::new();
        sr.announce("silent", AnnouncePriority::Off);
        assert!(sr.messages().is_empty());
    }

    #[test]
    fn logging_reader_interrupt() {
        let mut sr = LoggingScreenReader::new();
        assert!(!sr.interrupted);
        sr.interrupt();
        assert!(sr.interrupted);
    }

    #[test]
    fn logging_reader_mode() {
        let mut sr = LoggingScreenReader::new();
        assert_eq!(sr.current_mode(), ScreenReaderMode::Normal);
        sr.set_mode(ScreenReaderMode::BrowseMode);
        assert_eq!(sr.current_mode(), ScreenReaderMode::BrowseMode);
        sr.set_mode(ScreenReaderMode::FocusMode);
        assert_eq!(sr.current_mode(), ScreenReaderMode::FocusMode);
    }

    #[test]
    fn logging_reader_active() {
        let sr = LoggingScreenReader::new();
        assert!(sr.is_active());
    }

    #[test]
    fn logging_reader_clear() {
        let mut sr = LoggingScreenReader::new();
        sr.announce("test", AnnouncePriority::Polite);
        sr.interrupt();
        sr.clear();
        assert!(sr.messages().is_empty());
        assert!(!sr.interrupted);
    }

    #[test]
    fn live_region_new() {
        let lr = LiveRegion::new(10);
        assert_eq!(lr.node_id, 10);
        assert_eq!(lr.politeness, AnnouncePriority::Polite);
        assert!(!lr.atomic);
        assert!(lr.is_relevant("additions"));
        assert!(lr.is_relevant("text"));
        assert!(!lr.is_relevant("removals"));
    }

    #[test]
    fn live_region_assertive() {
        let lr = LiveRegion::assertive(5);
        assert_eq!(lr.politeness, AnnouncePriority::Assertive);
        assert!(lr.atomic);
        assert!(lr.is_relevant("additions"));
        assert!(lr.is_relevant("removals"));
        assert!(lr.is_relevant("text"));
    }

    #[test]
    fn live_region_monitor_add_remove() {
        let mut mon = LiveRegionMonitor::new();
        mon.add_region(LiveRegion::new(1));
        mon.add_region(LiveRegion::assertive(2));
        assert_eq!(mon.region_count(), 2);
        mon.remove_region(1);
        assert_eq!(mon.region_count(), 1);
        assert!(mon.get_region(2).is_some());
        assert!(mon.get_region(1).is_none());
    }

    #[test]
    fn live_region_monitor_should_announce() {
        let mut mon = LiveRegionMonitor::new();
        mon.add_region(LiveRegion::new(10));
        assert_eq!(
            mon.should_announce(10, "text"),
            Some(AnnouncePriority::Polite)
        );
        assert_eq!(mon.should_announce(10, "removals"), None);
        assert_eq!(mon.should_announce(99, "text"), None);
    }

    #[test]
    fn navigation_hint_variants() {
        let hints = [
            NavigationHint::NextElement,
            NavigationHint::PreviousElement,
            NavigationHint::NextHeading,
            NavigationHint::PreviousHeading,
            NavigationHint::NextLandmark,
            NavigationHint::PreviousLandmark,
            NavigationHint::NextFocusable,
            NavigationHint::PreviousFocusable,
            NavigationHint::NextLink,
            NavigationHint::PreviousLink,
            NavigationHint::NextFormField,
            NavigationHint::PreviousFormField,
        ];
        // Ensure all variants are distinct.
        for (i, a) in hints.iter().enumerate() {
            for (j, b) in hints.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn screen_reader_mode_equality() {
        assert_eq!(ScreenReaderMode::Normal, ScreenReaderMode::Normal);
        assert_ne!(ScreenReaderMode::Normal, ScreenReaderMode::FocusMode);
        assert_ne!(ScreenReaderMode::FocusMode, ScreenReaderMode::BrowseMode);
    }

    #[test]
    fn announce_priority_off() {
        assert_ne!(AnnouncePriority::Polite, AnnouncePriority::Off);
        assert_ne!(AnnouncePriority::Assertive, AnnouncePriority::Off);
    }
}
