use std::fmt;

use serde::{Deserialize, Serialize};

/// Icon context category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconContext {
    Actions,
    Applications,
    Categories,
    Devices,
    Emblems,
    MimeTypes,
    Places,
    Status,
}

/// Icon scaling type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconType {
    Fixed,
    Scalable,
    Threshold,
}

/// A directory within an icon theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconDirectory {
    pub path: String,
    pub size: u32,
    pub scale: u32,
    pub context: IconContext,
    pub icon_type: IconType,
}

impl IconDirectory {
    #[must_use]
    pub fn new(path: &str, size: u32, context: IconContext, icon_type: IconType) -> Self {
        Self {
            path: path.to_string(),
            size,
            scale: 1,
            context,
            icon_type,
        }
    }
}

/// An icon theme (freedesktop icon theme spec).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconTheme {
    pub name: String,
    pub comment: String,
    pub inherits: Vec<String>,
    pub directories: Vec<IconDirectory>,
}

impl IconTheme {
    #[must_use]
    pub fn new(name: &str, comment: &str) -> Self {
        Self {
            name: name.to_string(),
            comment: comment.to_string(),
            inherits: Vec::new(),
            directories: Vec::new(),
        }
    }

    /// Add a directory to this theme.
    pub fn add_directory(&mut self, dir: IconDirectory) {
        self.directories.push(dir);
    }
}

impl fmt::Display for IconTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IconTheme({}, {} dirs)",
            self.name,
            self.directories.len()
        )
    }
}

/// Result of an icon lookup.
#[derive(Debug, Clone)]
pub struct IconMatch {
    pub theme: String,
    pub path: String,
    pub size: u32,
    pub scale: u32,
    pub icon_type: IconType,
}

/// Icon lookup engine across multiple themes.
#[derive(Debug, Clone)]
pub struct IconLookup {
    themes: Vec<IconTheme>,
}

impl IconLookup {
    #[must_use]
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
        }
    }

    /// Add a theme to the lookup.
    pub fn add_theme(&mut self, theme: IconTheme) {
        self.themes.push(theme);
    }

    /// Find an icon by name, preferred size, and scale.
    #[must_use]
    pub fn find_icon(&self, name: &str, size: u32, scale: u32) -> Option<IconMatch> {
        // First pass: exact match on size and scale
        for theme in &self.themes {
            for dir in &theme.directories {
                if dir.size == size && dir.scale == scale && dir.path.contains(name) {
                    return Some(IconMatch {
                        theme: theme.name.clone(),
                        path: dir.path.clone(),
                        size: dir.size,
                        scale: dir.scale,
                        icon_type: dir.icon_type,
                    });
                }
            }
        }

        // Second pass: prefer scalable
        for theme in &self.themes {
            for dir in &theme.directories {
                if dir.icon_type == IconType::Scalable && dir.path.contains(name) {
                    return Some(IconMatch {
                        theme: theme.name.clone(),
                        path: dir.path.clone(),
                        size: dir.size,
                        scale: dir.scale,
                        icon_type: dir.icon_type,
                    });
                }
            }
        }

        // Third pass: closest size match
        let mut best: Option<(IconMatch, i64)> = None;
        for theme in &self.themes {
            for dir in &theme.directories {
                if dir.path.contains(name) {
                    let diff = (i64::from(dir.size) - i64::from(size)).abs();
                    let is_better = best.as_ref().is_none_or(|(_, d)| diff < *d);
                    if is_better {
                        best = Some((
                            IconMatch {
                                theme: theme.name.clone(),
                                path: dir.path.clone(),
                                size: dir.size,
                                scale: dir.scale,
                                icon_type: dir.icon_type,
                            },
                            diff,
                        ));
                    }
                }
            }
        }

        best.map(|(m, _)| m)
    }

    /// Number of themes loaded.
    #[must_use]
    pub fn theme_count(&self) -> usize {
        self.themes.len()
    }
}

impl Default for IconLookup {
    fn default() -> Self {
        Self::new()
    }
}
