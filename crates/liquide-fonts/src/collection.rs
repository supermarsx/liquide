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
