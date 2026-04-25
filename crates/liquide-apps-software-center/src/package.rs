//! Package metadata and versioning.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A semantic version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    #[must_use]
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string like "1.2.3".
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// License model for a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum License {
    OpenSource,
    Proprietary,
    Freemium,
    Unknown,
}

impl fmt::Display for License {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenSource => f.write_str("open-source"),
            Self::Proprietary => f.write_str("proprietary"),
            Self::Freemium => f.write_str("freemium"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

/// Category for a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppCategory {
    Productivity,
    Development,
    Graphics,
    Multimedia,
    Games,
    Education,
    Internet,
    System,
    Utilities,
    Science,
    Office,
    Other,
}

impl AppCategory {
    pub const ALL: &'static [AppCategory] = &[
        Self::Productivity,
        Self::Development,
        Self::Graphics,
        Self::Multimedia,
        Self::Games,
        Self::Education,
        Self::Internet,
        Self::System,
        Self::Utilities,
        Self::Science,
        Self::Office,
        Self::Other,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Productivity => "Productivity",
            Self::Development => "Development",
            Self::Graphics => "Graphics",
            Self::Multimedia => "Audio & Video",
            Self::Games => "Games",
            Self::Education => "Education",
            Self::Internet => "Internet",
            Self::System => "System",
            Self::Utilities => "Utilities",
            Self::Science => "Science",
            Self::Office => "Office",
            Self::Other => "Other",
        }
    }
}

impl fmt::Display for AppCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Package metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Unique package identifier (e.g. "org.mozilla.firefox").
    pub id: String,
    /// Display name.
    pub name: String,
    /// Short description (one line).
    pub summary: String,
    /// Long description (multi-paragraph).
    pub description: String,
    /// Current version.
    pub version: Version,
    /// Package category.
    pub category: AppCategory,
    /// License model.
    pub license: License,
    /// Developer / publisher.
    pub developer: String,
    /// Homepage URL.
    pub homepage: String,
    /// Download size in bytes.
    pub download_size: u64,
    /// Installed size in bytes.
    pub installed_size: u64,
    /// Screenshot URLs.
    pub screenshots: Vec<String>,
    /// Icon name or path.
    pub icon: String,
    /// Whether the package is installed.
    pub installed: bool,
    /// The installed version (if different from latest).
    pub installed_version: Option<Version>,
    /// Repository this package comes from.
    pub repository_id: String,
}

impl PackageInfo {
    /// Whether an update is available.
    #[must_use]
    pub fn has_update(&self) -> bool {
        self.installed
            && self
                .installed_version
                .as_ref()
                .is_some_and(|v| v < &self.version)
    }

    /// Human-readable download size.
    #[must_use]
    pub fn human_download_size(&self) -> String {
        human_size(self.download_size)
    }

    /// Human-readable installed size.
    #[must_use]
    pub fn human_installed_size(&self) -> String {
        human_size(self.installed_size)
    }
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}
