//! Recording metadata — annotations, tags, and access log.

use serde::{Deserialize, Serialize};

/// Metadata associated with a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// Recording title.
    pub title: String,
    /// Recording description.
    pub description: String,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Time-stamped annotations.
    pub annotations: Vec<Annotation>,
    /// Access log entries.
    pub access_log: Vec<AccessLogEntry>,
}

impl RecordingMetadata {
    /// Create new empty metadata.
    #[must_use]
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            description: String::new(),
            tags: Vec::new(),
            annotations: Vec::new(),
            access_log: Vec::new(),
        }
    }

    /// Add a tag.
    pub fn add_tag(&mut self, tag: &str) {
        self.tags.push(tag.to_string());
    }

    /// Add an annotation.
    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    /// Log an access event.
    pub fn log_access(&mut self, entry: AccessLogEntry) {
        self.access_log.push(entry);
    }
}

impl Default for RecordingMetadata {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl std::fmt::Display for RecordingMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingMetadata(\"{}\", tags={}, annotations={})",
            self.title,
            self.tags.len(),
            self.annotations.len()
        )
    }
}

/// A time-stamped annotation on a recording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Annotation {
    /// Timestamp in microseconds from recording start.
    pub timestamp_us: u64,
    /// Annotation text.
    pub text: String,
    /// Author of the annotation.
    pub author: String,
}

impl Annotation {
    /// Create a new annotation.
    #[must_use]
    pub fn new(timestamp_us: u64, text: &str, author: &str) -> Self {
        Self {
            timestamp_us,
            text: text.to_string(),
            author: author.to_string(),
        }
    }
}

impl std::fmt::Display for Annotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Annotation(t={}, by {})", self.timestamp_us, self.author)
    }
}

/// An entry in the recording access log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessLogEntry {
    /// Timestamp in microseconds.
    pub timestamp_us: u64,
    /// User who performed the action.
    pub user: String,
    /// The action performed.
    pub action: AccessAction,
}

impl AccessLogEntry {
    /// Create a new access log entry.
    #[must_use]
    pub fn new(timestamp_us: u64, user: &str, action: AccessAction) -> Self {
        Self {
            timestamp_us,
            user: user.to_string(),
            action,
        }
    }
}

impl std::fmt::Display for AccessLogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AccessLog({} by {} at {})",
            self.action, self.user, self.timestamp_us
        )
    }
}

/// Actions that can be performed on a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessAction {
    /// Viewed the recording.
    View,
    /// Exported the recording.
    Export,
    /// Redacted part of the recording.
    Redact,
    /// Deleted the recording.
    Delete,
}

impl std::fmt::Display for AccessAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::View => write!(f, "View"),
            Self::Export => write!(f, "Export"),
            Self::Redact => write!(f, "Redact"),
            Self::Delete => write!(f, "Delete"),
        }
    }
}
