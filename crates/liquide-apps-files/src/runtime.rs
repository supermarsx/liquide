//! File manager runtime coordinator.

use crate::clipboard::FileClipboard;
use crate::config::FilesConfig;
use crate::entry::FileEntry;
use crate::listing::DirectoryListing;
use crate::operations::OperationQueue;
use crate::search::FileSearch;
use crate::sidebar::Sidebar;

/// Central coordinator for the file manager.
pub struct FilesRuntime {
    config: FilesConfig,
    sidebar: Sidebar,
    current_listing: DirectoryListing,
    clipboard: FileClipboard,
    search: FileSearch,
    operations: OperationQueue,
    navigation_history: Vec<String>,
    history_index: usize,
    selection: Vec<usize>,
    /// Live typed-text buffer for the search bar (the text-input target).
    search_query: String,
}

impl FilesRuntime {
    /// Create a new files runtime.
    #[must_use]
    pub fn new(config: FilesConfig) -> Self {
        let mut listing = DirectoryListing::new(config.initial_directory.clone());
        listing.show_hidden = config.show_hidden;
        listing.sort_field = config.default_sort;
        listing.sort_ascending = config.sort_ascending;
        listing.view_mode = config.view_mode;

        Self {
            config,
            sidebar: Sidebar::new(),
            current_listing: listing,
            clipboard: FileClipboard::new(),
            search: FileSearch::new(),
            operations: OperationQueue::new(),
            navigation_history: Vec::new(),
            history_index: 0,
            selection: Vec::new(),
            search_query: String::new(),
        }
    }

    /// The live typed search-bar buffer.
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Replace the live search-bar buffer. This only updates the text-input
    /// target; it does not re-run the heavy filesystem search.
    pub fn set_search_query(&mut self, q: String) {
        self.search_query = q;
    }

    /// Get config.
    #[must_use]
    pub fn config(&self) -> &FilesConfig {
        &self.config
    }

    /// Get the sidebar.
    #[must_use]
    pub fn sidebar(&self) -> &Sidebar {
        &self.sidebar
    }

    /// Get mutable sidebar.
    pub fn sidebar_mut(&mut self) -> &mut Sidebar {
        &mut self.sidebar
    }

    /// Get current directory listing.
    #[must_use]
    pub fn current_listing(&self) -> &DirectoryListing {
        &self.current_listing
    }

    /// Navigate to a directory.
    pub fn navigate(&mut self, path: String, entries: Vec<FileEntry>) {
        // Push to history.
        if self.navigation_history.is_empty()
            || self.navigation_history.last().map(|s| s.as_str()) != Some(&path)
        {
            // Truncate forward history.
            self.navigation_history.truncate(self.history_index + 1);
            self.navigation_history.push(path.clone());
            self.history_index = self.navigation_history.len() - 1;
        }

        self.current_listing = DirectoryListing::new(path);
        self.current_listing.show_hidden = self.config.show_hidden;
        self.current_listing.sort_field = self.config.default_sort;
        self.current_listing.sort_ascending = self.config.sort_ascending;
        self.current_listing.view_mode = self.config.view_mode;
        self.current_listing.set_entries(entries);
        self.selection.clear();
    }

    /// Navigate up to parent directory.
    pub fn navigate_up(&mut self) -> Option<String> {
        self.current_listing.parent()
    }

    /// Navigate back in history.
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    /// Navigate forward in history.
    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.navigation_history.len()
    }

    /// Go back, returning the path.
    pub fn go_back(&mut self) -> Option<&str> {
        if self.can_go_back() {
            self.history_index -= 1;
            Some(&self.navigation_history[self.history_index])
        } else {
            None
        }
    }

    /// Go forward, returning the path.
    pub fn go_forward(&mut self) -> Option<&str> {
        if self.can_go_forward() {
            self.history_index += 1;
            Some(&self.navigation_history[self.history_index])
        } else {
            None
        }
    }

    /// Get the clipboard.
    #[must_use]
    pub fn clipboard(&self) -> &FileClipboard {
        &self.clipboard
    }

    /// Get mutable clipboard.
    pub fn clipboard_mut(&mut self) -> &mut FileClipboard {
        &mut self.clipboard
    }

    /// Get the search state.
    #[must_use]
    pub fn search(&self) -> &FileSearch {
        &self.search
    }

    /// Get mutable search state.
    pub fn search_mut(&mut self) -> &mut FileSearch {
        &mut self.search
    }

    /// Get the operation queue.
    #[must_use]
    pub fn operations(&self) -> &OperationQueue {
        &self.operations
    }

    /// Get mutable operation queue.
    pub fn operations_mut(&mut self) -> &mut OperationQueue {
        &mut self.operations
    }

    /// Current selection indices.
    #[must_use]
    pub fn selection(&self) -> &[usize] {
        &self.selection
    }

    /// Set selection.
    pub fn set_selection(&mut self, indices: Vec<usize>) {
        self.selection = indices;
    }

    /// Select all visible entries.
    pub fn select_all(&mut self) {
        self.selection = (0..self.current_listing.visible_count()).collect();
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Get selected entries.
    #[must_use]
    pub fn selected_entries(&self) -> Vec<&FileEntry> {
        self.selection
            .iter()
            .filter_map(|&i| self.current_listing.get(i))
            .collect()
    }

    /// Navigation history.
    #[must_use]
    pub fn history(&self) -> &[String] {
        &self.navigation_history
    }

    /// The current 0-based position within the navigation history.
    #[must_use]
    pub fn history_index(&self) -> usize {
        self.history_index
    }

    /// Replace the current listing's entries in place, applying the active
    /// sort/filter settings, without touching the navigation history. Used by
    /// the widget seam to refresh a directory whose contents were resolved
    /// out-of-band (e.g. a host-supplied listing).
    pub fn set_current_entries(&mut self, entries: Vec<FileEntry>) {
        self.current_listing.set_entries(entries);
        self.selection.clear();
    }

    /// Point the current listing at `path` (with the given entries) **without**
    /// recording a history step. This is the in-memory primitive behind the
    /// toolbar back/forward/up actions, which move the history cursor
    /// themselves; using [`navigate`](Self::navigate) there would corrupt the
    /// history by pushing the destination again.
    fn show_path(&mut self, path: String, entries: Vec<FileEntry>) {
        self.current_listing = DirectoryListing::new(path);
        self.current_listing.show_hidden = self.config.show_hidden;
        self.current_listing.sort_field = self.config.default_sort;
        self.current_listing.sort_ascending = self.config.sort_ascending;
        self.current_listing.view_mode = self.config.view_mode;
        self.current_listing.set_entries(entries);
        self.selection.clear();
    }

    /// In-memory "back": move the history cursor back one step and show that
    /// path (with empty entries — the in-memory history records only paths).
    /// Returns the new current path, or `None` at the start of history.
    pub fn go_back_to_listing(&mut self) -> Option<String> {
        if !self.can_go_back() {
            return None;
        }
        self.history_index -= 1;
        let path = self.navigation_history[self.history_index].clone();
        self.show_path(path.clone(), Vec::new());
        Some(path)
    }

    /// In-memory "forward": move the history cursor forward one step and show
    /// that path. Returns the new current path, or `None` at the end of history.
    pub fn go_forward_to_listing(&mut self) -> Option<String> {
        if !self.can_go_forward() {
            return None;
        }
        self.history_index += 1;
        let path = self.navigation_history[self.history_index].clone();
        self.show_path(path.clone(), Vec::new());
        Some(path)
    }

    /// In-memory "up": navigate to the current directory's parent, recording a
    /// new history step (like [`navigate`](Self::navigate)). Returns the parent
    /// path that was opened, or `None` at the filesystem root.
    pub fn go_up_to_listing(&mut self) -> Option<String> {
        let parent = self.current_listing.parent()?;
        self.navigate(parent.clone(), Vec::new());
        Some(parent)
    }

    // =========================================================================
    // Real filesystem navigation
    // =========================================================================

    /// Navigate to a real directory on disk, reading its contents with
    /// [`DirectoryListing::load_directory`].
    pub fn navigate_to(&mut self, path: &std::path::Path) -> crate::Result<()> {
        self.current_listing.load_directory(path)?;

        let path_str = self.current_listing.path.clone();
        // Push to history (same logic as `navigate`).
        if self.navigation_history.is_empty()
            || self.navigation_history.last().map(|s| s.as_str()) != Some(&path_str)
        {
            self.navigation_history.truncate(self.history_index + 1);
            self.navigation_history.push(path_str);
            self.history_index = self.navigation_history.len() - 1;
        }
        self.selection.clear();
        Ok(())
    }

    /// Navigate back in history, loading the directory from disk.
    pub fn navigate_back(&mut self) -> crate::Result<bool> {
        if !self.can_go_back() {
            return Ok(false);
        }
        self.history_index -= 1;
        let path = self.navigation_history[self.history_index].clone();
        self.current_listing
            .load_directory(std::path::Path::new(&path))?;
        self.selection.clear();
        Ok(true)
    }

    /// Navigate forward in history, loading the directory from disk.
    pub fn navigate_forward_disk(&mut self) -> crate::Result<bool> {
        if !self.can_go_forward() {
            return Ok(false);
        }
        self.history_index += 1;
        let path = self.navigation_history[self.history_index].clone();
        self.current_listing
            .load_directory(std::path::Path::new(&path))?;
        self.selection.clear();
        Ok(true)
    }

    /// Navigate up to the parent directory on disk.
    pub fn navigate_up_disk(&mut self) -> crate::Result<bool> {
        let parent = match self.current_listing.parent() {
            Some(p) => p,
            None => return Ok(false),
        };
        self.navigate_to(std::path::Path::new(&parent))?;
        Ok(true)
    }

    /// Open entry by index: if it's a directory, navigate into it.
    /// Returns `true` if navigation occurred.
    pub fn open_entry(&mut self, index: usize) -> crate::Result<bool> {
        let entry = match self.current_listing.get(index) {
            Some(e) => e,
            None => return Ok(false),
        };
        if !entry.is_dir() {
            return Ok(false);
        }
        let path = entry.path.clone();
        self.navigate_to(std::path::Path::new(&path))?;
        Ok(true)
    }

    /// Reload the current directory from disk.
    pub fn refresh(&mut self) -> crate::Result<()> {
        let path = self.current_listing.path.clone();
        self.current_listing
            .load_directory(std::path::Path::new(&path))?;
        self.selection.clear();
        Ok(())
    }
}
