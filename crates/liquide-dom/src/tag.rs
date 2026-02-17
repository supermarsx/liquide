//! Interned tag names for constant-time comparison.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// A global string interner for tag names.
static INTERNER: LazyLock<Mutex<TagInterner>> =
    LazyLock::new(|| Mutex::new(TagInterner::new()));

/// An interned tag name. Comparison is O(1) via integer index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag(u32);

impl Tag {
    /// Intern a tag name string. Returns the same `Tag` for the same string.
    pub fn intern(name: &str) -> Self {
        let mut interner = INTERNER.lock().unwrap_or_else(|e| e.into_inner());
        interner.intern(name)
    }

    /// Get the string representation of the tag.
    pub fn as_str(&self) -> String {
        let interner = INTERNER.lock().unwrap_or_else(|e| e.into_inner());
        interner.resolve(self.0).unwrap_or_default()
    }

    /// Internal index (for serialization or debugging).
    pub fn index(self) -> u32 {
        self.0
    }

    // Well-known tags for the desktop shell, pre-interned on first access.
    pub fn root() -> Self {
        Self::intern("root")
    }
    pub fn div() -> Self {
        Self::intern("div")
    }
    pub fn span() -> Self {
        Self::intern("span")
    }
    pub fn text() -> Self {
        Self::intern("#text")
    }
    pub fn desktop_background() -> Self {
        Self::intern("desktop-background")
    }
    pub fn statusbar() -> Self {
        Self::intern("statusbar")
    }
    pub fn dock() -> Self {
        Self::intern("dock")
    }
    pub fn dock_item() -> Self {
        Self::intern("dock-item")
    }
    pub fn window() -> Self {
        Self::intern("window")
    }
    pub fn window_titlebar() -> Self {
        Self::intern("window-titlebar")
    }
    pub fn window_content() -> Self {
        Self::intern("window-content")
    }
    pub fn window_layer() -> Self {
        Self::intern("window-layer")
    }
    pub fn workspace_container() -> Self {
        Self::intern("workspace-container")
    }
    pub fn notification() -> Self {
        Self::intern("notification")
    }
    pub fn tooltip() -> Self {
        Self::intern("tooltip")
    }
    pub fn context_menu() -> Self {
        Self::intern("context-menu")
    }
    pub fn menu_item() -> Self {
        Self::intern("menu-item")
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

struct TagInterner {
    to_index: HashMap<String, u32>,
    to_string: Vec<String>,
}

impl TagInterner {
    fn new() -> Self {
        Self {
            to_index: HashMap::new(),
            to_string: Vec::new(),
        }
    }

    fn intern(&mut self, name: &str) -> Tag {
        if let Some(&idx) = self.to_index.get(name) {
            return Tag(idx);
        }
        let idx = self.to_string.len() as u32;
        self.to_string.push(name.to_string());
        self.to_index.insert(name.to_string(), idx);
        Tag(idx)
    }

    fn resolve(&self, idx: u32) -> Option<String> {
        self.to_string.get(idx as usize).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_same_string_returns_same_tag() {
        let a = Tag::intern("button");
        let b = Tag::intern("button");
        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_different_tags() {
        let a = Tag::intern("dock-test-a");
        let b = Tag::intern("dock-test-b");
        assert_ne!(a, b);
    }

    #[test]
    fn as_str_roundtrip() {
        let tag = Tag::intern("statusbar-clock");
        assert_eq!(tag.as_str(), "statusbar-clock");
    }

    #[test]
    fn well_known_tags_work() {
        assert_eq!(Tag::dock().as_str(), "dock");
        assert_eq!(Tag::window().as_str(), "window");
    }
}
