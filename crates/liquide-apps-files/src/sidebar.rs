//! Sidebar with bookmarks and places.

use serde::{Deserialize, Serialize};

/// A sidebar bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// Display name.
    pub name: String,
    /// Target path.
    pub path: String,
    /// Icon name.
    pub icon: String,
    /// Whether this is a system bookmark (Home, Desktop, etc.).
    pub system: bool,
}

/// Sidebar state.
pub struct Sidebar {
    bookmarks: Vec<Bookmark>,
}

impl Sidebar {
    /// Create a new sidebar with default bookmarks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bookmarks: vec![
                Bookmark { name: "Home".into(), path: "~".into(), icon: "folder-home".into(), system: true },
                Bookmark { name: "Desktop".into(), path: "~/Desktop".into(), icon: "folder-desktop".into(), system: true },
                Bookmark { name: "Documents".into(), path: "~/Documents".into(), icon: "folder-documents".into(), system: true },
                Bookmark { name: "Downloads".into(), path: "~/Downloads".into(), icon: "folder-downloads".into(), system: true },
                Bookmark { name: "Pictures".into(), path: "~/Pictures".into(), icon: "folder-pictures".into(), system: true },
                Bookmark { name: "Music".into(), path: "~/Music".into(), icon: "folder-music".into(), system: true },
                Bookmark { name: "Videos".into(), path: "~/Videos".into(), icon: "folder-videos".into(), system: true },
                Bookmark { name: "Trash".into(), path: "~/.local/share/Trash".into(), icon: "user-trash".into(), system: true },
            ],
        }
    }

    /// Get all bookmarks.
    #[must_use]
    pub fn bookmarks(&self) -> &[Bookmark] { &self.bookmarks }

    /// Add a user bookmark.
    pub fn add_bookmark(&mut self, name: String, path: String) {
        if !self.bookmarks.iter().any(|b| b.path == path) {
            self.bookmarks.push(Bookmark {
                name, path, icon: "folder".into(), system: false,
            });
        }
    }

    /// Remove a user bookmark by path.
    pub fn remove_bookmark(&mut self, path: &str) -> crate::Result<()> {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.system || b.path != path);
        if self.bookmarks.len() == before {
            return Err(crate::FilesError::BookmarkNotFound { name: path.to_string() });
        }
        Ok(())
    }

    /// Get a bookmark by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Bookmark> {
        self.bookmarks.iter().find(|b| b.name == name)
    }

    /// Move a bookmark up in the list.
    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.bookmarks.len() {
            self.bookmarks.swap(index, index - 1);
        }
    }

    /// Move a bookmark down in the list.
    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.bookmarks.len() {
            self.bookmarks.swap(index, index + 1);
        }
    }
}

impl Default for Sidebar {
    fn default() -> Self { Self::new() }
}
