use serde::{Deserialize, Serialize};

use crate::node::AccessibleNode;
use crate::Result;

/// Priority for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnouncePriority {
    Polite,
    Assertive,
}

/// Trait for screen reader integration.
pub trait ScreenReader: Send {
    /// Announce text with the given priority.
    fn announce(&mut self, text: &str, priority: AnnouncePriority) -> Result<()>;
    /// Describe a node to the user.
    fn describe_node(&mut self, node: &AccessibleNode) -> Result<()>;
    /// Stop any current speech.
    fn stop(&mut self) -> Result<()>;
    /// Check if the reader is active.
    fn is_active(&self) -> bool;
}

/// Null screen reader — discards all output.
pub struct NullReader;

impl ScreenReader for NullReader {
    fn announce(&mut self, _text: &str, _priority: AnnouncePriority) -> Result<()> {
        Ok(())
    }

    fn describe_node(&mut self, _node: &AccessibleNode) -> Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    fn is_active(&self) -> bool {
        false
    }
}

/// Logging screen reader — captures messages for testing.
pub struct LogReader {
    messages: Vec<(String, AnnouncePriority)>,
    active: bool,
}

impl LogReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            active: true,
        }
    }

    /// Get all captured messages.
    #[must_use]
    pub fn messages(&self) -> &[(String, AnnouncePriority)] {
        &self.messages
    }

    /// Clear captured messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

impl Default for LogReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenReader for LogReader {
    fn announce(&mut self, text: &str, priority: AnnouncePriority) -> Result<()> {
        self.messages.push((text.to_string(), priority));
        Ok(())
    }

    fn describe_node(&mut self, node: &AccessibleNode) -> Result<()> {
        let desc = format!("{} {} {}", node.role, node.name, node.description);
        self.messages.push((desc, AnnouncePriority::Polite));
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.active = false;
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active
    }
}
