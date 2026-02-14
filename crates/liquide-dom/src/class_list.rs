//! Class list with fast membership testing.

use serde::{Deserialize, Serialize};

/// A set of CSS class names for a DOM node.
///
/// Optimized for the common case of 0–4 classes per element.
/// Uses a sorted `Vec<String>` for deterministic iteration and
/// binary search for membership tests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClassList {
    classes: Vec<String>,
}

impl ClassList {
    /// Create an empty class list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a class. No-op if already present.
    pub fn add(&mut self, class: &str) {
        match self.classes.binary_search_by(|c| c.as_str().cmp(class)) {
            Ok(_) => {} // already present
            Err(pos) => self.classes.insert(pos, class.to_string()),
        }
    }

    /// Remove a class. Returns `true` if it was present.
    pub fn remove(&mut self, class: &str) -> bool {
        match self.classes.binary_search_by(|c| c.as_str().cmp(class)) {
            Ok(pos) => {
                self.classes.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    /// Toggle a class: add if absent, remove if present. Returns new state.
    pub fn toggle(&mut self, class: &str) -> bool {
        match self.classes.binary_search_by(|c| c.as_str().cmp(class)) {
            Ok(pos) => {
                self.classes.remove(pos);
                false
            }
            Err(pos) => {
                self.classes.insert(pos, class.to_string());
                true
            }
        }
    }

    /// Check if a class is present.
    pub fn contains(&self, class: &str) -> bool {
        self.classes
            .binary_search_by(|c| c.as_str().cmp(class))
            .is_ok()
    }

    /// Number of classes.
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// Is the list empty?
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// Iterate over class names (sorted).
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.classes.iter().map(String::as_str)
    }

    /// Get classes as a slice of Strings (for CSS engine compatibility).
    pub fn as_strings(&self) -> &[String] {
        &self.classes
    }

    /// Parse from a space-separated class string (like HTML `class` attribute).
    pub fn from_class_string(s: &str) -> Self {
        let mut list = Self::new();
        for class in s.split_whitespace() {
            if !class.is_empty() {
                list.add(class);
            }
        }
        list
    }

    /// Serialize to a space-separated class string.
    pub fn to_class_string(&self) -> String {
        self.classes.join(" ")
    }
}

impl std::fmt::Display for ClassList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_class_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_contains() {
        let mut cl = ClassList::new();
        cl.add("active");
        cl.add("glass");
        assert!(cl.contains("active"));
        assert!(cl.contains("glass"));
        assert!(!cl.contains("hidden"));
        assert_eq!(cl.len(), 2);
    }

    #[test]
    fn add_idempotent() {
        let mut cl = ClassList::new();
        cl.add("x");
        cl.add("x");
        assert_eq!(cl.len(), 1);
    }

    #[test]
    fn remove_returns_presence() {
        let mut cl = ClassList::new();
        cl.add("foo");
        assert!(cl.remove("foo"));
        assert!(!cl.remove("foo"));
    }

    #[test]
    fn toggle() {
        let mut cl = ClassList::new();
        assert!(cl.toggle("a"));  // added
        assert!(!cl.toggle("a")); // removed
        assert!(cl.is_empty());
    }

    #[test]
    fn from_class_string() {
        let cl = ClassList::from_class_string("  active  glass   dark ");
        assert_eq!(cl.len(), 3);
        assert!(cl.contains("active"));
        assert!(cl.contains("glass"));
        assert!(cl.contains("dark"));
    }

    #[test]
    fn sorted_iteration() {
        let mut cl = ClassList::new();
        cl.add("z");
        cl.add("a");
        cl.add("m");
        let names: Vec<_> = cl.iter().collect();
        assert_eq!(names, vec!["a", "m", "z"]);
    }
}
