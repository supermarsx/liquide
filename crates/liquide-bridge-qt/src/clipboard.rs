//! Qt clipboard integration.
//!
//! Maps Liquide clipboard operations to `QClipboard`.

use serde::{Deserialize, Serialize};

/// Qt clipboard mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QtClipboardMode {
    Clipboard,
    Selection,
    FindBuffer,
}

/// Clipboard content.
#[derive(Debug, Clone)]
pub struct QtClipboardData {
    pub text: Option<String>,
    pub html: Option<String>,
    pub image_data: Option<Vec<u8>>,
    pub urls: Vec<String>,
}

impl QtClipboardData {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            html: None,
            image_data: None,
            urls: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.html.is_none()
            && self.image_data.is_none()
            && self.urls.is_empty()
    }
}

/// Qt clipboard bridge.
pub struct QtClipboard {
    data: std::collections::HashMap<QtClipboardMode, QtClipboardData>,
    change_count: u64,
}

impl QtClipboard {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
            change_count: 0,
        }
    }

    pub fn set(&mut self, mode: QtClipboardMode, data: QtClipboardData) {
        self.data.insert(mode, data);
        self.change_count += 1;
    }

    pub fn set_text(&mut self, mode: QtClipboardMode, text: &str) {
        self.set(mode, QtClipboardData::text(text));
    }

    #[must_use]
    pub fn get(&self, mode: QtClipboardMode) -> Option<&QtClipboardData> {
        self.data.get(&mode)
    }

    #[must_use]
    pub fn text(&self, mode: QtClipboardMode) -> Option<&str> {
        self.get(mode).and_then(|d| d.text.as_deref())
    }

    pub fn clear(&mut self, mode: QtClipboardMode) {
        self.data.remove(&mode);
        self.change_count += 1;
    }

    #[must_use]
    pub fn change_count(&self) -> u64 {
        self.change_count
    }
}

impl Default for QtClipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qt_clipboard() {
        let mut cb = QtClipboard::new();
        cb.set_text(QtClipboardMode::Clipboard, "hello");
        assert_eq!(cb.text(QtClipboardMode::Clipboard), Some("hello"));
        cb.clear(QtClipboardMode::Clipboard);
        assert!(cb.text(QtClipboardMode::Clipboard).is_none());
    }
}
