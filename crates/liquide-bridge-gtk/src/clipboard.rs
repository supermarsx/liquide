//! GTK clipboard integration.
//!
//! Maps Liquide's clipboard operations to `GdkClipboard` (GTK4).
//! Supports text, HTML, image, and URI data.

use serde::{Deserialize, Serialize};

/// Clipboard target (selection in X11 terminology).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClipboardTarget {
    /// Primary clipboard (Ctrl+C / Ctrl+V).
    Clipboard,
    /// Primary selection (X11: select-to-copy, middle-click-paste).
    PrimarySelection,
}

/// Content type stored on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardContentType {
    Text,
    Html,
    Image,
    UriList,
    Custom(String),
}

/// Clipboard content.
#[derive(Debug, Clone)]
pub struct ClipboardContent {
    pub content_type: ClipboardContentType,
    pub data: Vec<u8>,
}

impl ClipboardContent {
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            content_type: ClipboardContentType::Text,
            data: text.as_bytes().to_vec(),
        }
    }

    #[must_use]
    pub fn html(html: &str) -> Self {
        Self {
            content_type: ClipboardContentType::Html,
            data: html.as_bytes().to_vec(),
        }
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }
}

/// GTK clipboard bridge.
///
/// In a real implementation, this would hold a reference to `GdkClipboard`
/// and use async content providers.
pub struct GtkClipboard {
    /// Stored content per target.
    contents: std::collections::HashMap<ClipboardTarget, Vec<ClipboardContent>>,
    /// Change counter (increments on every set).
    change_count: u64,
}

impl GtkClipboard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            contents: std::collections::HashMap::new(),
            change_count: 0,
        }
    }

    /// Set clipboard content (replaces existing).
    pub fn set(&mut self, target: ClipboardTarget, contents: Vec<ClipboardContent>) {
        self.contents.insert(target, contents);
        self.change_count += 1;
    }

    /// Set plain text on the clipboard.
    pub fn set_text(&mut self, target: ClipboardTarget, text: &str) {
        self.set(target, vec![ClipboardContent::text(text)]);
    }

    /// Get clipboard content.
    #[must_use]
    pub fn get(&self, target: ClipboardTarget) -> Option<&[ClipboardContent]> {
        self.contents.get(&target).map(|v| v.as_slice())
    }

    /// Get plain text from the clipboard.
    #[must_use]
    pub fn get_text(&self, target: ClipboardTarget) -> Option<&str> {
        self.get(target).and_then(|contents| {
            contents
                .iter()
                .find(|c| c.content_type == ClipboardContentType::Text)
                .and_then(|c| c.as_text())
        })
    }

    /// Check if the clipboard has content.
    #[must_use]
    pub fn has_content(&self, target: ClipboardTarget) -> bool {
        self.contents.get(&target).is_some_and(|v| !v.is_empty())
    }

    /// Clear the clipboard.
    pub fn clear(&mut self, target: ClipboardTarget) {
        self.contents.remove(&target);
        self.change_count += 1;
    }

    /// Get the change counter.
    #[must_use]
    pub fn change_count(&self) -> u64 {
        self.change_count
    }
}

impl Default for GtkClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_text() {
        let mut cb = GtkClipboard::new();
        cb.set_text(ClipboardTarget::Clipboard, "hello");
        assert_eq!(cb.get_text(ClipboardTarget::Clipboard), Some("hello"));
        assert!(cb.has_content(ClipboardTarget::Clipboard));
        assert!(!cb.has_content(ClipboardTarget::PrimarySelection));
    }

    #[test]
    fn test_clipboard_clear() {
        let mut cb = GtkClipboard::new();
        cb.set_text(ClipboardTarget::Clipboard, "test");
        cb.clear(ClipboardTarget::Clipboard);
        assert!(!cb.has_content(ClipboardTarget::Clipboard));
    }

    #[test]
    fn test_change_counter() {
        let mut cb = GtkClipboard::new();
        assert_eq!(cb.change_count(), 0);
        cb.set_text(ClipboardTarget::Clipboard, "a");
        assert_eq!(cb.change_count(), 1);
        cb.set_text(ClipboardTarget::Clipboard, "b");
        assert_eq!(cb.change_count(), 2);
    }
}
