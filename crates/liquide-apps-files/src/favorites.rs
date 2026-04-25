//! Bookmarked / favorite locations.
//!
//! Maintains an ordered list of user-pinned URIs, similar to the GTK
//! bookmarks file (`~/.config/gtk-3.0/bookmarks`) and the GNOME Files
//! sidebar favourites section.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Favorite
// ---------------------------------------------------------------------------

/// A single bookmarked location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Favorite {
    /// URI of the location (e.g. `file:///home/user/Projects`).
    pub uri: String,
    /// Display name shown in the sidebar.
    pub display_name: String,
    /// Icon name (freedesktop icon-naming-spec).
    pub icon: String,
    /// Position in the ordered list (0-based).
    pub position: usize,
}

impl Favorite {
    /// Create a new favorite.
    #[must_use]
    pub fn new(uri: String, display_name: String, icon: String, position: usize) -> Self {
        Self {
            uri,
            display_name,
            icon,
            position,
        }
    }
}

// ---------------------------------------------------------------------------
// FavoriteStore
// ---------------------------------------------------------------------------

/// Ordered collection of favourite locations.
pub struct FavoriteStore {
    items: Vec<Favorite>,
}

impl FavoriteStore {
    /// Create a store pre-populated with the default favourite locations.
    #[must_use]
    pub fn new() -> Self {
        let mut store = Self { items: Vec::new() };
        store.populate_defaults();
        store
    }

    /// Create an empty store (no defaults).
    #[must_use]
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    /// Add a favourite.  Returns `false` if the URI already exists.
    pub fn add(&mut self, uri: &str, display_name: &str, icon: &str) -> bool {
        if self.items.iter().any(|f| f.uri == uri) {
            return false;
        }
        let position = self.items.len();
        self.items.push(Favorite::new(
            uri.to_string(),
            display_name.to_string(),
            icon.to_string(),
            position,
        ));
        true
    }

    /// Remove a favourite by URI.  Returns `true` if it was found.
    pub fn remove(&mut self, uri: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|f| f.uri != uri);
        let removed = self.items.len() < before;
        if removed {
            self.reindex();
        }
        removed
    }

    /// Move the favourite at `from` to position `to`.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() {
            return;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.reindex();
    }

    /// Return all favourites in order.
    #[must_use]
    pub fn list(&self) -> &[Favorite] {
        &self.items
    }

    /// Number of favourites.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Check whether a URI is bookmarked.
    #[must_use]
    pub fn is_favorite(&self, uri: &str) -> bool {
        self.items.iter().any(|f| f.uri == uri)
    }

    /// Find a favourite by URI.
    #[must_use]
    pub fn find(&self, uri: &str) -> Option<&Favorite> {
        self.items.iter().find(|f| f.uri == uri)
    }

    // -----------------------------------------------------------------------
    // Persistence (simple text file format)
    // -----------------------------------------------------------------------

    /// Serialize to a simple text format compatible with GTK bookmarks.
    ///
    /// Each line: `uri display_name icon_name`
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for f in &self.items {
            out.push_str(&format!("{} {} {}\n", f.uri, f.display_name, f.icon));
        }
        out
    }

    /// Deserialize from the text format produced by [`serialize`](Self::serialize).
    pub fn deserialize(&mut self, data: &str) {
        self.items.clear();
        for (idx, line) in data.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, ' ');
            let uri = match parts.next() {
                Some(u) => u,
                None => continue,
            };
            let display_name = parts.next().unwrap_or(uri);
            let icon = parts.next().unwrap_or("folder");
            self.items.push(Favorite::new(
                uri.to_string(),
                display_name.to_string(),
                icon.to_string(),
                idx,
            ));
        }
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    fn populate_defaults(&mut self) {
        let home = home_dir();
        let defaults = [
            ("Home", &format!("file://{home}"), "folder-home"),
            (
                "Documents",
                &format!("file://{home}/Documents"),
                "folder-documents",
            ),
            (
                "Downloads",
                &format!("file://{home}/Downloads"),
                "folder-download",
            ),
            ("Music", &format!("file://{home}/Music"), "folder-music"),
            (
                "Pictures",
                &format!("file://{home}/Pictures"),
                "folder-pictures",
            ),
            ("Videos", &format!("file://{home}/Videos"), "folder-videos"),
        ];
        for (i, (name, uri, icon)) in defaults.iter().enumerate() {
            self.items.push(Favorite::new(
                uri.to_string(),
                name.to_string(),
                icon.to_string(),
                i,
            ));
        }
    }

    fn reindex(&mut self) {
        for (i, f) in self.items.iter_mut().enumerate() {
            f.position = i;
        }
    }
}

impl Default for FavoriteStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Platform-independent home directory helper.
fn home_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return home;
    }
    #[cfg(target_os = "windows")]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return profile.replace('\\', "/");
    }
    "/home/user".to_string()
}
