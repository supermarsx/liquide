//! Sidebar with bookmarks and places.

use serde::{Deserialize, Serialize};

/// A sidebar bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// Display name.
    pub name: String,
    /// Target path.
    pub path: String,
    /// Icon name (None uses a default folder icon).
    pub icon: Option<String>,
    /// Whether this is a system bookmark (Home, Desktop, etc.).
    pub is_system: bool,
}

impl Bookmark {
    /// Create a new user bookmark.
    #[must_use]
    pub fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            icon: None,
            is_system: false,
        }
    }

    /// Create a system bookmark with a specific icon.
    #[must_use]
    pub fn system(name: &str, path: &str, icon: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            icon: Some(icon.to_string()),
            is_system: true,
        }
    }

    /// Get the icon name, falling back to "folder" if not set.
    #[must_use]
    pub fn icon_name(&self) -> &str {
        self.icon.as_deref().unwrap_or("folder")
    }
}

/// Return the default system bookmarks with platform-specific paths.
///
/// - Linux: `~/Desktop`, `~/Documents`, etc. (XDG standard)
/// - macOS: `~/Desktop`, `~/Documents`, etc.
/// - Windows: Uses `USERPROFILE` env var for paths
#[must_use]
pub fn default_bookmarks() -> Vec<Bookmark> {
    let home = home_dir();
    vec![
        Bookmark::system("Home", &home, "folder-home"),
        Bookmark::system("Desktop", &format!("{}/Desktop", home), "folder-desktop"),
        Bookmark::system(
            "Documents",
            &format!("{}/Documents", home),
            "folder-documents",
        ),
        Bookmark::system(
            "Downloads",
            &format!("{}/Downloads", home),
            "folder-downloads",
        ),
        Bookmark::system("Music", &format!("{}/Music", home), "folder-music"),
        Bookmark::system("Pictures", &format!("{}/Pictures", home), "folder-pictures"),
        Bookmark::system("Videos", &format!("{}/Videos", home), "folder-videos"),
    ]
}

/// Get the home directory path in a platform-appropriate way.
fn home_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return profile.replace('\\', "/");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
    }
    "~".to_string()
}

/// Bookmark manager with add/remove/reorder operations.
pub struct BookmarkManager {
    bookmarks: Vec<Bookmark>,
}

impl BookmarkManager {
    /// Create a new bookmark manager with default system bookmarks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bookmarks: default_bookmarks(),
        }
    }

    /// Create an empty bookmark manager (no defaults).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }

    /// Get all bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Add a user bookmark. Returns false if a bookmark with the same path already exists.
    pub fn add(&mut self, name: String, path: String, icon: Option<String>) -> bool {
        if self.bookmarks.iter().any(|b| b.path == path) {
            return false;
        }
        self.bookmarks.push(Bookmark {
            name,
            path,
            icon,
            is_system: false,
        });
        true
    }

    /// Remove a bookmark by path. System bookmarks cannot be removed.
    pub fn remove(&mut self, path: &str) -> crate::Result<()> {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.is_system || b.path != path);
        if self.bookmarks.len() == before {
            return Err(crate::FilesError::BookmarkNotFound {
                name: path.to_string(),
            });
        }
        Ok(())
    }

    /// Find a bookmark by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Bookmark> {
        self.bookmarks.iter().find(|b| b.name == name)
    }

    /// Move a bookmark to a new position (reorder).
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.bookmarks.len() || to >= self.bookmarks.len() {
            return;
        }
        let item = self.bookmarks.remove(from);
        self.bookmarks.insert(to, item);
    }

    /// Move a bookmark up by one position.
    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.bookmarks.len() {
            self.bookmarks.swap(index, index - 1);
        }
    }

    /// Move a bookmark down by one position.
    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.bookmarks.len() {
            self.bookmarks.swap(index, index + 1);
        }
    }

    /// Number of bookmarks.
    #[must_use]
    pub fn count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Number of user (non-system) bookmarks.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.bookmarks.iter().filter(|b| !b.is_system).count()
    }
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Sidebar state (wraps BookmarkManager for backward compatibility).
pub struct Sidebar {
    manager: BookmarkManager,
}

impl Sidebar {
    /// Create a new sidebar with default bookmarks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: BookmarkManager::new(),
        }
    }

    /// Get all bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> &[Bookmark] {
        self.manager.bookmarks()
    }

    /// Add a user bookmark.
    pub fn add_bookmark(&mut self, name: String, path: String) {
        self.manager.add(name, path, None);
    }

    /// Remove a user bookmark by path.
    pub fn remove_bookmark(&mut self, path: &str) -> crate::Result<()> {
        self.manager.remove(path)
    }

    /// Get a bookmark by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Bookmark> {
        self.manager.find(name)
    }

    /// Move a bookmark up in the list.
    pub fn move_up(&mut self, index: usize) {
        self.manager.move_up(index);
    }

    /// Move a bookmark down in the list.
    pub fn move_down(&mut self, index: usize) {
        self.manager.move_down(index);
    }

    /// Get the underlying bookmark manager.
    #[must_use]
    pub fn manager(&self) -> &BookmarkManager {
        &self.manager
    }

    /// Get mutable access to the bookmark manager.
    pub fn manager_mut(&mut self) -> &mut BookmarkManager {
        &mut self.manager
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}
