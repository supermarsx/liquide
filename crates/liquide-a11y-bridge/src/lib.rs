//! Platform accessibility bridge — connects the `liquide-a11y` accessibility
//! tree to native accessibility APIs (AT-SPI on Linux, UI Automation on
//! Windows, NSAccessibility on macOS).

mod platform;
pub mod tree;
pub mod events;
pub mod text;
pub mod actions;
pub mod screen_reader;
pub mod magnifier;

pub use platform::AccessibilityBridge;
pub use tree::{AccessibleNode as BridgeAccessibleNode, AccessibleRole, AccessibleState, AccessibleTree, Bounds};
pub use events::{A11yEvent, A11yEventTarget, A11yEventQueue};
pub use text::{AccessibleText, TextBoundary, TextAttribute, SimpleAccessibleText, get_text_at_offset};
pub use actions::{AccessibleAction, ActionSet, ActionHandler};
pub use screen_reader::{
    ScreenReaderBridge, LoggingScreenReader, LiveRegion, LiveRegionMonitor,
    NavigationHint, ScreenReaderMode,
    AnnouncePriority as BridgeAnnouncePriority,
};
pub use magnifier::{MagnifierConfig, MagnifierState, MagnifierLens, Rect};

use liquide_a11y::AccessibilityTree;

/// Events that the bridge needs to communicate to the platform.
#[derive(Debug, Clone)]
pub enum A11yBridgeEvent {
    /// A node was created.
    NodeCreated { id: u64 },
    /// A node was destroyed.
    NodeDestroyed { id: u64 },
    /// A node's property changed.
    NodeChanged { id: u64, property: A11yProperty },
    /// Focus moved to a node.
    FocusChanged { id: u64 },
    /// A node's value changed.
    ValueChanged { id: u64, value: String },
    /// Announcement for screen readers.
    Announce {
        text: String,
        priority: AnnouncePriority,
    },
}

/// Which property of an accessible node changed.
#[derive(Debug, Clone)]
pub enum A11yProperty {
    Name,
    Description,
    Role,
    State,
    Value,
    Bounds,
}

/// Priority level for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncePriority {
    /// Queued after current speech finishes.
    Polite,
    /// Interrupts current speech immediately.
    Assertive,
}

/// Errors produced by the bridge.
#[derive(Debug, Clone)]
pub enum BridgeError {
    /// The platform does not support accessibility bridging.
    NotSupported,
    /// Failed to connect to the platform accessibility service.
    ConnectionFailed(String),
    /// A platform-specific error occurred.
    PlatformError(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupported => write!(f, "accessibility bridge not supported on this platform"),
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::PlatformError(msg) => write!(f, "platform error: {msg}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// Trait for platform accessibility bridge implementations.
pub trait A11yBridgeBackend: Send {
    /// Initialize the bridge, connecting to the platform a11y service.
    fn init(&mut self) -> Result<(), BridgeError>;

    /// Shut down the bridge.
    fn shutdown(&mut self);

    /// Push a batch of events to the platform.
    fn push_events(&mut self, events: &[A11yBridgeEvent]) -> Result<(), BridgeError>;

    /// Update the full tree (for initial sync or major changes).
    fn sync_tree(&mut self, tree: &AccessibilityTree) -> Result<(), BridgeError>;

    /// Check if a screen reader is active.
    fn is_screen_reader_active(&self) -> bool;

    /// Get the platform's preferred reduced-motion setting.
    fn prefers_reduced_motion(&self) -> bool;

    /// Get the platform's preferred high-contrast setting.
    fn prefers_high_contrast(&self) -> bool;

    /// Get the platform's preferred font scale (1.0 = normal).
    fn font_scale(&self) -> f32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_a11y::{AccessibilityTree, AccessibleNode, Role};

    #[test]
    fn bridge_creation() {
        let bridge = AccessibilityBridge::new();
        assert!(!bridge.is_screen_reader_active());
    }

    #[test]
    fn event_creation() {
        let events = vec![
            A11yBridgeEvent::NodeCreated { id: 1 },
            A11yBridgeEvent::NodeDestroyed { id: 2 },
            A11yBridgeEvent::NodeChanged {
                id: 3,
                property: A11yProperty::Name,
            },
            A11yBridgeEvent::FocusChanged { id: 4 },
            A11yBridgeEvent::ValueChanged {
                id: 5,
                value: "hello".to_string(),
            },
            A11yBridgeEvent::Announce {
                text: "test".to_string(),
                priority: AnnouncePriority::Polite,
            },
        ];
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn default_values() {
        let bridge = AccessibilityBridge::new();
        // On all platforms, a freshly-constructed (un-initialized) bridge
        // should report no screen reader active.
        assert!(!bridge.is_screen_reader_active());
        // font_scale returns a positive value (platform-dependent, but > 0).
        assert!(bridge.font_scale() > 0.0);
        // prefers_reduced_motion / prefers_high_contrast are platform-dependent
        // booleans — just verify they don't panic.
        let _ = bridge.prefers_reduced_motion();
        let _ = bridge.prefers_high_contrast();
    }

    #[test]
    fn bridge_init_shutdown() {
        let mut bridge = AccessibilityBridge::new();
        // On the stub and Windows/Linux/macOS initial impls, init succeeds
        let result = bridge.init();
        assert!(result.is_ok());
        bridge.shutdown();
    }

    #[test]
    fn bridge_push_events() {
        let mut bridge = AccessibilityBridge::new();
        bridge.init().unwrap();
        let events = vec![
            A11yBridgeEvent::FocusChanged { id: 1 },
            A11yBridgeEvent::Announce {
                text: "hello".to_string(),
                priority: AnnouncePriority::Assertive,
            },
        ];
        let result = bridge.push_events(&events);
        assert!(result.is_ok());
    }

    #[test]
    fn bridge_sync_tree() {
        let mut bridge = AccessibilityBridge::new();
        bridge.init().unwrap();

        let mut tree = AccessibilityTree::new();
        let root = AccessibleNode::new(tree.allocate_id(), Role::Application, "Test App");
        tree.set_root(root);

        let result = bridge.sync_tree(&tree);
        assert!(result.is_ok());
    }

    #[test]
    fn bridge_error_display() {
        let e1 = BridgeError::NotSupported;
        assert_eq!(
            e1.to_string(),
            "accessibility bridge not supported on this platform"
        );

        let e2 = BridgeError::ConnectionFailed("timeout".to_string());
        assert_eq!(e2.to_string(), "connection failed: timeout");

        let e3 = BridgeError::PlatformError("COM init failed".to_string());
        assert_eq!(e3.to_string(), "platform error: COM init failed");
    }

    #[test]
    fn announce_priority_eq() {
        assert_eq!(AnnouncePriority::Polite, AnnouncePriority::Polite);
        assert_ne!(AnnouncePriority::Polite, AnnouncePriority::Assertive);
    }
}
