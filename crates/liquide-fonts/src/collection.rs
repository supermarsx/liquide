//! Font collections — named groups of fonts that can be exported,
//! imported, and shared.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{FontError, Result};

/// A named collection of font families.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontCollection {
    /// Collection name (e.g. "My Branding Fonts", "Development Fonts").
    pub name: String,
    /// Description.
    pub description: String,
    /// Font family names included.
    pub families: Vec<String>,
    /// Tags for organization.
    pub tags: Vec<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Last modified timestamp.
    pub modified_at: u64,
    /// Author of the collection.
    pub author: String,
    /// Version string.
    pub version: String,
}

impl FontCollection {
    /// Create a new empty collection.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            name: name.into(),
            description: String::new(),
            families: Vec::new(),
            tags: Vec::new(),
            created_at: now,
            modified_at: now,
            author: String::new(),
            version: "1.0".into(),
        }
    }

    /// Add a font family to the collection.
    pub fn add_family(&mut self, family: impl Into<String>) {
        let f = family.into();
        if !self.families.contains(&f) {
            self.families.push(f);
            self.touch();
        }
    }

    /// Remove a font family from the collection.
    pub fn remove_family(&mut self, family: &str) -> bool {
        let before = self.families.len();
        self.families.retain(|f| f != family);
        let removed = self.families.len() < before;
        if removed {
            self.touch();
        }
        removed
    }

    fn touch(&mut self) {
        self.modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Serialize the collection to JSON.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| FontError::Serde(e.to_string()))
    }

    /// Deserialize a collection from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| FontError::Serde(e.to_string()))
    }

    /// Serialize the collection to TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| FontError::Serde(e.to_string()))
    }

    /// Deserialize a collection from TOML.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).map_err(|e| FontError::Serde(e.to_string()))
    }
}

/// Persistent store for font collections.
pub struct CollectionStore {
    collections: Vec<FontCollection>,
    #[allow(dead_code)]
    storage_path: PathBuf,
}

impl CollectionStore {
    /// Create a new in-memory collection store.
    #[must_use]
    pub fn new() -> Self {
        let storage_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("liquide")
            .join("font-collections");
        Self {
            collections: Vec::new(),
            storage_path,
        }
    }

    /// Create a new collection.
    pub fn create(&mut self, name: impl Into<String>) -> &mut FontCollection {
        let collection = FontCollection::new(name);
        self.collections.push(collection);
        self.collections.last_mut().unwrap()
    }

    /// Get all collections.
    #[must_use]
    pub fn all(&self) -> &[FontCollection] {
        &self.collections
    }

    /// Find a collection by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&FontCollection> {
        self.collections.iter().find(|c| c.name == name)
    }

    /// Find a collection by name (mutable).
    pub fn find_mut(&mut self, name: &str) -> Option<&mut FontCollection> {
        self.collections.iter_mut().find(|c| c.name == name)
    }

    /// Remove a collection by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.collections.len();
        self.collections.retain(|c| c.name != name);
        self.collections.len() < before
    }

    /// Export a collection to a JSON file.
    pub fn export_json(&self, name: &str, path: &Path) -> Result<()> {
        let collection = self
            .find(name)
            .ok_or_else(|| FontError::CollectionNotFound {
                name: name.to_string(),
            })?;
        let json = collection.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Import a collection from a JSON file.
    pub fn import_json(&mut self, path: &Path) -> Result<String> {
        let json = std::fs::read_to_string(path)?;
        let collection = FontCollection::from_json(&json)?;
        let name = collection.name.clone();
        self.collections.push(collection);
        Ok(name)
    }

    /// Export a collection to a TOML file.
    pub fn export_toml(&self, name: &str, path: &Path) -> Result<()> {
        let collection = self
            .find(name)
            .ok_or_else(|| FontError::CollectionNotFound {
                name: name.to_string(),
            })?;
        let toml_str = collection.to_toml()?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    /// Import a collection from a TOML file.
    pub fn import_toml(&mut self, path: &Path) -> Result<String> {
        let toml_str = std::fs::read_to_string(path)?;
        let collection = FontCollection::from_toml(&toml_str)?;
        let name = collection.name.clone();
        self.collections.push(collection);
        Ok(name)
    }

    /// Get the count of collections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.collections.len()
    }

    /// Whether there are no collections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }
}

impl Default for CollectionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_collection_fields() {
        let c = FontCollection::new("My Fonts");
        assert_eq!(c.name, "My Fonts");
        assert!(c.families.is_empty());
        assert!(c.tags.is_empty());
        assert!(c.created_at > 0);
        assert_eq!(c.version, "1.0");
    }

    #[test]
    fn add_and_remove_family() {
        let mut c = FontCollection::new("Test");
        c.add_family("Manrope");
        c.add_family("Inter");
        assert_eq!(c.families.len(), 2);

        // Duplicate is ignored.
        c.add_family("Manrope");
        assert_eq!(c.families.len(), 2);

        assert!(c.remove_family("Inter"));
        assert_eq!(c.families, vec!["Manrope"]);

        // Removing absent family returns false.
        assert!(!c.remove_family("NoSuch"));
    }

    #[test]
    fn json_roundtrip() {
        let mut c = FontCollection::new("Roundtrip");
        c.add_family("Fira Code");
        c.description = "test".into();

        let json = c.to_json().unwrap();
        let c2 = FontCollection::from_json(&json).unwrap();
        assert_eq!(c2.name, "Roundtrip");
        assert_eq!(c2.families, vec!["Fira Code"]);
        assert_eq!(c2.description, "test");
    }

    #[test]
    fn toml_roundtrip() {
        let mut c = FontCollection::new("TOML Test");
        c.add_family("Manrope");

        let toml_str = c.to_toml().unwrap();
        let c2 = FontCollection::from_toml(&toml_str).unwrap();
        assert_eq!(c2.name, "TOML Test");
        assert_eq!(c2.families, vec!["Manrope"]);
    }

    #[test]
    fn collection_store_crud() {
        let mut store = CollectionStore::new();
        assert!(store.is_empty());

        store.create("Alpha");
        store.create("Beta");
        assert_eq!(store.len(), 2);

        assert!(store.find("Alpha").is_some());
        assert!(store.find("Gamma").is_none());

        store.find_mut("Alpha").unwrap().add_family("Inter");
        assert_eq!(store.find("Alpha").unwrap().families.len(), 1);

        assert!(store.remove("Alpha"));
        assert_eq!(store.len(), 1);
        assert!(!store.remove("Alpha"));
    }
}
