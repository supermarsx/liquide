use crate::{Dialog, DialogId, DialogResult};
use liquide_popups::DialogInfo;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FilePickerConfig {
    pub id: DialogId,
    pub title: String,
    pub mode: FilePickerMode,
    pub initial_dir: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub show_hidden: bool,
    pub multiple_selection: bool,
    pub bookmarks: Vec<Bookmark>,
    pub recent_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    Open,
    Save,
    SelectFolder,
}

#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub name: String,
    pub path: PathBuf,
    pub icon: Option<String>,
}

/// File picker state machine
#[derive(Debug)]
pub struct FilePickerState {
    pub config: FilePickerConfig,
    pub current_dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: Vec<usize>,
    pub filename_input: String,
    pub active_filter: usize,
    pub sort_by: SortField,
    pub sort_ascending: bool,
    pub view_mode: ViewMode,
    pub search_query: String,
    pub navigation_history: Vec<PathBuf>,
    pub history_index: usize,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub is_hidden: bool,
    pub icon: FileIcon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIcon {
    Folder,
    File,
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Executable,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    Modified,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
    Details,
}

impl FilePickerState {
    pub fn new(config: FilePickerConfig) -> Self {
        let current_dir = config
            .initial_dir
            .clone()
            .unwrap_or_else(|| dirs_home().unwrap_or_else(|| PathBuf::from("/")));
        let current_dir_for_history = current_dir.clone();
        Self {
            config,
            current_dir,
            entries: Vec::new(),
            selected: Vec::new(),
            filename_input: String::new(),
            active_filter: 0,
            sort_by: SortField::Name,
            sort_ascending: true,
            view_mode: ViewMode::List,
            search_query: String::new(),
            navigation_history: vec![current_dir_for_history],
            history_index: 0,
        }
    }

    /// Scan current directory and populate entries
    pub fn refresh(&mut self) -> std::io::Result<()> {
        self.entries.clear();
        self.selected.clear();

        let read_dir = std::fs::read_dir(&self.current_dir)?;
        for entry in read_dir.flatten() {
            let metadata = entry.metadata().ok();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_hidden = name.starts_with('.');

            if !self.config.show_hidden && is_hidden {
                continue;
            }

            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata.as_ref().and_then(|m| {
                m.modified()
                    .ok()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs())
            });

            let icon = if is_dir {
                FileIcon::Folder
            } else {
                classify_file_icon(&name)
            };

            // Apply filter (skip non-matching files, but always show dirs)
            if !is_dir && !self.config.filters.is_empty() {
                let filter = &self.config.filters[self.active_filter];
                if !filter.extensions.is_empty() {
                    let ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if !filter.extensions.iter().any(|f| f.to_lowercase() == ext) {
                        continue;
                    }
                }
            }

            // Apply search filter
            if !self.search_query.is_empty()
                && !name
                    .to_lowercase()
                    .contains(&self.search_query.to_lowercase())
            {
                continue;
            }

            self.entries.push(DirEntry {
                name,
                path: entry.path(),
                is_dir,
                size,
                modified,
                is_hidden,
                icon,
            });
        }

        self.sort_entries();
        Ok(())
    }

    fn sort_entries(&mut self) {
        // Directories first, then sort by field
        self.entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let ord = match self.sort_by {
                    SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortField::Size => a.size.cmp(&b.size),
                    SortField::Modified => a.modified.cmp(&b.modified),
                    SortField::Type => {
                        let ea = a.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        let eb = b.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        ea.cmp(eb)
                    }
                };
                if self.sort_ascending {
                    ord
                } else {
                    ord.reverse()
                }
            }
        });
    }

    /// Navigate into directory.
    ///
    /// Invariant: `navigation_history[history_index]` is always the current
    /// directory. Navigating drops any forward history, appends the new
    /// directory, and advances the index to point at it.
    pub fn navigate_to(&mut self, path: PathBuf) -> std::io::Result<()> {
        // Drop any forward history (everything after the current position).
        self.navigation_history.truncate(self.history_index + 1);
        self.navigation_history.push(path.clone());
        self.history_index = self.navigation_history.len() - 1;
        self.current_dir = path;
        self.refresh()
    }

    /// Step back one entry in the navigation history, if any.
    pub fn go_back(&mut self) -> std::io::Result<()> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.current_dir = self.navigation_history[self.history_index].clone();
            self.refresh()
        } else {
            Ok(())
        }
    }

    /// Step forward one entry in the navigation history, if any.
    pub fn go_forward(&mut self) -> std::io::Result<()> {
        if self.history_index + 1 < self.navigation_history.len() {
            self.history_index += 1;
            self.current_dir = self.navigation_history[self.history_index].clone();
            self.refresh()
        } else {
            Ok(())
        }
    }

    pub fn go_up(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            let parent = parent.to_path_buf();
            self.navigate_to(parent)
        } else {
            Ok(())
        }
    }

    /// Activate selected item (enter dir or confirm file)
    pub fn activate_selected(&mut self) -> std::io::Result<Option<DialogResult<Vec<PathBuf>>>> {
        if self.selected.is_empty() {
            return Ok(None);
        }

        let idx = self.selected[0];
        if idx < self.entries.len() && self.entries[idx].is_dir {
            let path = self.entries[idx].path.clone();
            self.navigate_to(path)?;
            Ok(None)
        } else {
            Ok(Some(self.confirm()))
        }
    }

    /// Confirm selection
    pub fn confirm(&self) -> DialogResult<Vec<PathBuf>> {
        if self.config.mode == FilePickerMode::Save {
            if self.filename_input.is_empty() {
                // A missing filename in Save mode is a validation failure, not
                // a user cancellation — keep the two distinct so hosts that
                // close on Cancelled don't silently discard the save.
                return DialogResult::Invalid("Filename required".into());
            }
            return DialogResult::Ok(vec![self.current_dir.join(&self.filename_input)]);
        }

        let paths: Vec<PathBuf> = self
            .selected
            .iter()
            .filter_map(|&i| self.entries.get(i).map(|e| e.path.clone()))
            .collect();

        if paths.is_empty() {
            DialogResult::Cancelled
        } else {
            DialogResult::Ok(paths)
        }
    }

    pub fn toggle_select(&mut self, index: usize) {
        if self.config.multiple_selection {
            if let Some(pos) = self.selected.iter().position(|&i| i == index) {
                self.selected.remove(pos);
            } else {
                self.selected.push(index);
            }
        } else {
            self.selected = vec![index];
        }
    }
}

fn classify_file_icon(name: &str) -> FileIcon {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => FileIcon::Image,
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" => FileIcon::Video,
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" | "wma" => FileIcon::Audio,
        "pdf" | "doc" | "docx" | "odt" | "txt" | "rtf" | "md" => FileIcon::Document,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => FileIcon::Archive,
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go" | "rb" | "css" | "html" => {
            FileIcon::Code
        }
        "exe" | "msi" | "appimage" | "deb" | "rpm" | "sh" | "bat" | "cmd" => FileIcon::Executable,
        _ => FileIcon::File,
    }
}

fn dirs_home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

impl Default for FilePickerConfig {
    fn default() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: crate::DialogId(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            title: "Open File".into(),
            mode: FilePickerMode::Open,
            initial_dir: None,
            filters: vec![FileFilter {
                name: "All Files".into(),
                extensions: vec![],
            }],
            show_hidden: false,
            multiple_selection: false,
            bookmarks: default_bookmarks(),
            recent_files: Vec::new(),
        }
    }
}

fn default_bookmarks() -> Vec<Bookmark> {
    let mut bookmarks = Vec::new();
    if let Some(home) = dirs_home() {
        bookmarks.push(Bookmark {
            name: "Home".into(),
            path: home.clone(),
            icon: Some("folder-home".into()),
        });
        for (name, dir, icon) in [
            ("Desktop", "Desktop", "user-desktop"),
            ("Documents", "Documents", "folder-documents"),
            ("Downloads", "Downloads", "folder-download"),
            ("Pictures", "Pictures", "folder-pictures"),
            ("Music", "Music", "folder-music"),
            ("Videos", "Videos", "folder-videos"),
        ] {
            let p = home.join(dir);
            if p.exists() {
                bookmarks.push(Bookmark {
                    name: name.into(),
                    path: p,
                    icon: Some(icon.into()),
                });
            }
        }
    }
    bookmarks
}

impl Dialog for FilePickerState {
    type Output = Vec<PathBuf>;
    fn id(&self) -> crate::DialogId {
        self.config.id
    }
    fn title(&self) -> &str {
        &self.config.title
    }
}

impl DialogInfo for FilePickerState {
    fn preferred_size(&self) -> (f32, f32) {
        (760.0, 520.0)
    }

    fn title(&self) -> &str {
        &self.config.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_config() {
        let cfg = FilePickerConfig::default();
        assert_eq!(cfg.title, "Open File");
        assert_eq!(cfg.mode, FilePickerMode::Open);
        assert!(!cfg.show_hidden);
        assert!(!cfg.multiple_selection);
        assert_eq!(cfg.filters.len(), 1);
        assert_eq!(cfg.filters[0].name, "All Files");
    }

    #[test]
    fn test_file_picker_new() {
        let cfg = FilePickerConfig::default();
        let state = FilePickerState::new(cfg);
        assert!(state.entries.is_empty());
        assert!(state.selected.is_empty());
        assert!(state.filename_input.is_empty());
        assert_eq!(state.sort_by, SortField::Name);
        assert!(state.sort_ascending);
        assert_eq!(state.view_mode, ViewMode::List);
    }

    #[test]
    fn test_navigate_builds_history() {
        let tmp = std::env::temp_dir().join("liquide_dialog_test_nav");
        let sub = tmp.join("subdir");
        let _ = fs::create_dir_all(&sub);

        let cfg = FilePickerConfig {
            initial_dir: Some(tmp.clone()),
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        assert_eq!(state.current_dir, tmp);
        // Invariant: history is seeded with the starting directory.
        assert_eq!(state.navigation_history, vec![tmp.clone()]);
        assert_eq!(state.history_index, 0);

        let _ = state.navigate_to(sub.clone());
        assert_eq!(state.current_dir, sub);
        assert_eq!(state.navigation_history, vec![tmp.clone(), sub.clone()]);
        assert_eq!(state.history_index, 1);

        let _ = state.go_back();
        assert_eq!(state.current_dir, tmp);
        assert_eq!(state.history_index, 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_history_back_navigate_back_sequence() {
        // Regression for the history off-by-one (t49-e5-F23): a
        // back -> navigate -> back sequence must land on the right entries
        // without dead Back presses or duplicate history entries.
        let tmp = std::env::temp_dir().join("liquide_dialog_test_hist_seq");
        let a = tmp.join("a");
        let b = tmp.join("b");
        let c = tmp.join("c");
        for d in [&a, &b, &c] {
            let _ = fs::create_dir_all(d);
        }

        let cfg = FilePickerConfig {
            initial_dir: Some(tmp.clone()),
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);

        // tmp -> a -> b
        state.navigate_to(a.clone()).unwrap();
        state.navigate_to(b.clone()).unwrap();
        assert_eq!(state.current_dir, b);
        assert_eq!(state.history_index, 2);

        // Back lands on a (not a no-op).
        state.go_back().unwrap();
        assert_eq!(state.current_dir, a);
        assert_eq!(state.history_index, 1);

        // Navigate to c: forward history (b) is dropped, c appended.
        state.navigate_to(c.clone()).unwrap();
        assert_eq!(state.current_dir, c);
        assert_eq!(
            state.navigation_history,
            vec![tmp.clone(), a.clone(), c.clone()]
        );
        assert_eq!(state.history_index, 2);

        // Back lands on a immediately — no dead press.
        state.go_back().unwrap();
        assert_eq!(state.current_dir, a);
        // Back again lands on tmp.
        state.go_back().unwrap();
        assert_eq!(state.current_dir, tmp);
        // Already at the start: another Back is a safe no-op.
        state.go_back().unwrap();
        assert_eq!(state.current_dir, tmp);
        assert_eq!(state.history_index, 0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_history_forward() {
        let tmp = std::env::temp_dir().join("liquide_dialog_test_hist_fwd");
        let a = tmp.join("a");
        for d in [&tmp, &a] {
            let _ = fs::create_dir_all(d);
        }
        let cfg = FilePickerConfig {
            initial_dir: Some(tmp.clone()),
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.navigate_to(a.clone()).unwrap();
        state.go_back().unwrap();
        assert_eq!(state.current_dir, tmp);
        state.go_forward().unwrap();
        assert_eq!(state.current_dir, a);
        // No further forward history.
        state.go_forward().unwrap();
        assert_eq!(state.current_dir, a);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_sorting() {
        let cfg = FilePickerConfig::default();
        let mut state = FilePickerState::new(cfg);
        state.entries = vec![
            DirEntry {
                name: "banana.txt".into(),
                path: PathBuf::from("/banana.txt"),
                is_dir: false,
                size: 100,
                modified: Some(1000),
                is_hidden: false,
                icon: FileIcon::Document,
            },
            DirEntry {
                name: "apple.txt".into(),
                path: PathBuf::from("/apple.txt"),
                is_dir: false,
                size: 200,
                modified: Some(2000),
                is_hidden: false,
                icon: FileIcon::Document,
            },
            DirEntry {
                name: "aaa_folder".into(),
                path: PathBuf::from("/aaa_folder"),
                is_dir: true,
                size: 0,
                modified: Some(500),
                is_hidden: false,
                icon: FileIcon::Folder,
            },
        ];

        state.sort_by = SortField::Name;
        state.sort_ascending = true;
        state.sort_entries();

        // Directories come first
        assert!(state.entries[0].is_dir);
        assert_eq!(state.entries[1].name, "apple.txt");
        assert_eq!(state.entries[2].name, "banana.txt");
    }

    #[test]
    fn test_toggle_select_single() {
        let cfg = FilePickerConfig {
            multiple_selection: false,
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.toggle_select(0);
        assert_eq!(state.selected, vec![0]);
        state.toggle_select(1);
        assert_eq!(state.selected, vec![1]);
    }

    #[test]
    fn test_toggle_select_multiple() {
        let cfg = FilePickerConfig {
            multiple_selection: true,
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.toggle_select(0);
        state.toggle_select(1);
        assert_eq!(state.selected, vec![0, 1]);
        state.toggle_select(0);
        assert_eq!(state.selected, vec![1]);
    }

    #[test]
    fn test_confirm_empty_returns_cancelled() {
        let cfg = FilePickerConfig::default();
        let state = FilePickerState::new(cfg);
        assert_eq!(state.confirm(), DialogResult::Cancelled);
    }

    #[test]
    fn test_confirm_save_empty_filename_is_invalid_not_cancelled() {
        // Regression for t49-e5-F22: a Save-mode validation failure (missing
        // filename) must be reported as Invalid, distinct from a user cancel.
        let cfg = FilePickerConfig {
            mode: FilePickerMode::Save,
            ..Default::default()
        };
        let state = FilePickerState::new(cfg);
        match state.confirm() {
            DialogResult::Invalid(msg) => assert_eq!(msg, "Filename required"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn test_confirm_open_no_selection_is_cancelled() {
        // The genuine-cancel path (Open mode, nothing selected) stays Cancelled,
        // so the failure/cancel distinction is preserved both ways.
        let cfg = FilePickerConfig {
            mode: FilePickerMode::Open,
            ..Default::default()
        };
        let state = FilePickerState::new(cfg);
        assert_eq!(state.confirm(), DialogResult::Cancelled);
    }

    #[test]
    fn test_confirm_save_with_filename() {
        let cfg = FilePickerConfig {
            mode: FilePickerMode::Save,
            initial_dir: Some(PathBuf::from("/tmp")),
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.filename_input = "test.txt".into();
        match state.confirm() {
            DialogResult::Ok(paths) => {
                assert_eq!(paths.len(), 1);
                assert_eq!(paths[0], PathBuf::from("/tmp/test.txt"));
            }
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn test_classify_file_icon() {
        assert_eq!(classify_file_icon("photo.png"), FileIcon::Image);
        assert_eq!(classify_file_icon("video.mp4"), FileIcon::Video);
        assert_eq!(classify_file_icon("song.mp3"), FileIcon::Audio);
        assert_eq!(classify_file_icon("notes.pdf"), FileIcon::Document);
        assert_eq!(classify_file_icon("backup.zip"), FileIcon::Archive);
        assert_eq!(classify_file_icon("main.rs"), FileIcon::Code);
        assert_eq!(classify_file_icon("app.exe"), FileIcon::Executable);
        assert_eq!(classify_file_icon("data.bin"), FileIcon::File);
    }

    #[test]
    fn test_refresh_reads_directory() {
        let tmp = std::env::temp_dir().join("liquide_dialog_test_refresh");
        let _ = fs::create_dir_all(&tmp);
        let _ = fs::write(tmp.join("hello.txt"), "world");
        let _ = fs::write(tmp.join("data.rs"), "fn main() {}");

        let cfg = FilePickerConfig {
            initial_dir: Some(tmp.clone()),
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.refresh().unwrap();

        assert!(state.entries.len() >= 2);
        let names: Vec<&str> = state.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"data.rs"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_filter_extensions() {
        let tmp = std::env::temp_dir().join("liquide_dialog_test_filter");
        let _ = fs::create_dir_all(&tmp);
        let _ = fs::write(tmp.join("image.png"), "");
        let _ = fs::write(tmp.join("code.rs"), "");

        let cfg = FilePickerConfig {
            initial_dir: Some(tmp.clone()),
            filters: vec![FileFilter {
                name: "Images".into(),
                extensions: vec!["png".into(), "jpg".into()],
            }],
            ..Default::default()
        };
        let mut state = FilePickerState::new(cfg);
        state.refresh().unwrap();

        let names: Vec<&str> = state.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"image.png"));
        assert!(!names.contains(&"code.rs"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
