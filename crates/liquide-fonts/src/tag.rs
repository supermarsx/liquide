//! Font tagging system for organization and filtering.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Built-in tag categories.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagCategory {
    /// Visual style (e.g. "geometric", "humanist", "grotesque").
    Style,
    /// Use case (e.g. "heading", "body", "code", "ui").
    UseCase,
    /// Mood / character (e.g. "professional", "playful", "elegant").
    Mood,
    /// Technical (e.g. "variable", "color", "ligatures").
    Technical,
    /// User-defined custom category.
    Custom(String),
}

/// A tag applied to a font family.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontTag {
    /// Tag name.
    pub name: String,
    /// Category.
    pub category: TagCategory,
    /// Color hint for UI display (hex string).
    pub color: Option<String>,
}

/// Tag store — manages tags and their associations with font families.
pub struct TagStore {
    /// Defined tags.
    tags: Vec<FontTag>,
    /// Family name → set of tag names.
    family_tags: HashMap<String, HashSet<String>>,
}

impl TagStore {
    /// Create a new empty tag store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tags: Vec::new(),
            family_tags: HashMap::new(),
        }
    }

    /// Define a new tag.
    pub fn define_tag(&mut self, tag: FontTag) {
        if !self.tags.iter().any(|t| t.name == tag.name) {
            self.tags.push(tag);
        }
    }

    /// Apply a tag to a font family.
    pub fn tag_family(&mut self, family: &str, tag_name: &str) {
        self.family_tags
            .entry(family.to_string())
            .or_default()
            .insert(tag_name.to_string());
    }

    /// Remove a tag from a font family.
    pub fn untag_family(&mut self, family: &str, tag_name: &str) {
        if let Some(tags) = self.family_tags.get_mut(family) {
            tags.remove(tag_name);
        }
    }

    /// Get all tags for a font family.
    #[must_use]
    pub fn tags_for_family(&self, family: &str) -> Vec<&str> {
        self.family_tags
            .get(family)
            .map(|tags| tags.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get all families with a specific tag.
    #[must_use]
    pub fn families_with_tag(&self, tag_name: &str) -> Vec<&str> {
        self.family_tags
            .iter()
            .filter(|(_, tags)| tags.contains(tag_name))
            .map(|(family, _)| family.as_str())
            .collect()
    }

    /// Get all defined tags.
    #[must_use]
    pub fn all_tags(&self) -> &[FontTag] {
        &self.tags
    }
}

impl Default for TagStore {
    fn default() -> Self {
        Self::new()
    }
}
