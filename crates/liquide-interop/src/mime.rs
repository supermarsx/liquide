use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{InteropError, Result};

/// A MIME type (e.g. `text/plain`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MimeType {
    pub type_: String,
    pub subtype: String,
}

impl MimeType {
    /// Parse a MIME type string like `text/plain`.
    pub fn parse(s: &str) -> Result<Self> {
        let (type_, subtype) = s.split_once('/').ok_or_else(|| {
            InteropError::ParseError(format!("invalid MIME type: {s}"))
        })?;
        Ok(Self {
            type_: type_.to_string(),
            subtype: subtype.to_string(),
        })
    }

    /// Check if this MIME type matches another (supports wildcards).
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        (self.type_ == other.type_ || self.type_ == "*" || other.type_ == "*")
            && (self.subtype == other.subtype || self.subtype == "*" || other.subtype == "*")
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.type_, self.subtype)
    }
}

/// Source of a MIME association.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MimeSource {
    System,
    User,
    Application,
}

/// Association between a MIME type and a desktop entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MimeAssociation {
    pub mime_type: MimeType,
    pub desktop_entry_id: String,
    pub source: MimeSource,
}

/// Database of MIME type associations.
#[derive(Debug, Clone)]
pub struct MimeDatabase {
    associations: Vec<MimeAssociation>,
}

impl MimeDatabase {
    #[must_use]
    pub fn new() -> Self {
        Self {
            associations: Vec::new(),
        }
    }

    /// Add a MIME association.
    pub fn add_association(&mut self, assoc: MimeAssociation) {
        self.associations.push(assoc);
    }

    /// Look up desktop entry IDs that handle the given MIME type.
    #[must_use]
    pub fn lookup(&self, mime: &MimeType) -> Vec<String> {
        self.associations
            .iter()
            .filter(|a| a.mime_type.matches(mime))
            .map(|a| a.desktop_entry_id.clone())
            .collect()
    }

    /// Return the default (first user, then system) handler for a MIME type.
    #[must_use]
    pub fn default_for(&self, mime: &MimeType) -> Option<String> {
        // User associations take priority over system
        let user = self.associations.iter().find(|a| {
            a.mime_type.matches(mime) && a.source == MimeSource::User
        });
        if let Some(u) = user {
            return Some(u.desktop_entry_id.clone());
        }

        // Then system
        let system = self.associations.iter().find(|a| {
            a.mime_type.matches(mime) && a.source == MimeSource::System
        });
        if let Some(s) = system {
            return Some(s.desktop_entry_id.clone());
        }

        // Then application
        self.associations
            .iter()
            .find(|a| a.mime_type.matches(mime))
            .map(|a| a.desktop_entry_id.clone())
    }

    /// Number of associations in the database.
    #[must_use]
    pub fn len(&self) -> usize {
        self.associations.len()
    }

    /// Whether the database is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.associations.is_empty()
    }
}

impl Default for MimeDatabase {
    fn default() -> Self {
        Self::new()
    }
}
