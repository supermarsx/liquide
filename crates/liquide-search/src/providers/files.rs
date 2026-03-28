//! File search provider.
//!
//! Maintains an in-memory index of file paths and supports fast
//! case-insensitive search by filename.  Results can be filtered by extension
//! or a simple MIME category.

use std::path::{Path, PathBuf};

use crate::provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction, fuzzy_score, clamp_score,
};

// ---------------------------------------------------------------------------
// FileIndex
// ---------------------------------------------------------------------------

/// In-memory index of file paths for fast search.
pub struct FileIndex {
    entries: Vec<FileEntry>,
}

/// A single indexed path.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub name_lower: String,
    pub extension: String,
    pub is_dir: bool,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a path to the index.
    pub fn add_path(&mut self, path: &Path, is_dir: bool) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let name_lower = name.to_lowercase();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        self.entries.push(FileEntry {
            path: path.to_path_buf(),
            name,
            name_lower,
            extension,
            is_dir,
        });
    }

    /// Number of indexed entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Search by filename (case-insensitive).
    pub fn search(&self, query: &str, max_results: usize) -> Vec<&FileEntry> {
        let mut scored: Vec<(f32, &FileEntry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let s = fuzzy_score(&e.name, query)?;
                Some((s, e))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);
        scored.into_iter().map(|(_, e)| e).collect()
    }

    /// Search with an extension filter.
    pub fn search_ext(&self, query: &str, ext: &str, max_results: usize) -> Vec<&FileEntry> {
        let ext_lower = ext.to_lowercase();
        let mut scored: Vec<(f32, &FileEntry)> = self
            .entries
            .iter()
            .filter(|e| e.extension == ext_lower)
            .filter_map(|e| {
                let s = fuzzy_score(&e.name, query)?;
                Some((s, e))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);
        scored.into_iter().map(|(_, e)| e).collect()
    }

    /// Search filtering by MIME category (image, video, audio, document, code).
    pub fn search_mime(&self, query: &str, mime: &str, max_results: usize) -> Vec<&FileEntry> {
        let exts = mime_extensions(mime);
        let mut scored: Vec<(f32, &FileEntry)> = self
            .entries
            .iter()
            .filter(|e| exts.contains(&e.extension.as_str()))
            .filter_map(|e| {
                let s = fuzzy_score(&e.name, query)?;
                Some((s, e))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);
        scored.into_iter().map(|(_, e)| e).collect()
    }
}

impl Default for FileIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FileSearchProvider
// ---------------------------------------------------------------------------

/// [`SearchProvider`] backed by a [`FileIndex`].
pub struct FileSearchProvider {
    index: FileIndex,
}

impl FileSearchProvider {
    pub fn new() -> Self {
        Self {
            index: FileIndex::new(),
        }
    }

    /// Mutable access to the underlying index for populating it.
    pub fn index_mut(&mut self) -> &mut FileIndex {
        &mut self.index
    }

    /// Shared access to the underlying index.
    pub fn index(&self) -> &FileIndex {
        &self.index
    }
}

impl Default for FileSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for FileSearchProvider {
    fn id(&self) -> &str { "files" }
    fn name(&self) -> &str { "Files" }
    fn icon(&self) -> &str { "system-file-manager" }
    fn priority(&self) -> u32 { 60 }

    fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        self.index
            .search(query, max_results)
            .into_iter()
            .map(|e| {
                let score = fuzzy_score(&e.name, query).unwrap_or(0.0);
                SearchResult {
                    id: e.path.to_string_lossy().into_owned(),
                    title: e.name.clone(),
                    description: e.path.parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    icon: if e.is_dir {
                        "folder".into()
                    } else {
                        icon_for_extension(&e.extension)
                    },
                    category: SearchCategory::File,
                    relevance_score: clamp_score(score),
                    action: SearchResultAction::Open(e.path.to_string_lossy().into_owned()),
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn icon_for_extension(ext: &str) -> String {
    match ext {
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "java" | "go" => "text-x-source",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => "image-x-generic",
        "mp4" | "mkv" | "avi" | "webm" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "wav" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "xz" | "7z" => "package-x-generic",
        _ => "text-x-generic",
    }
    .into()
}

fn mime_extensions(category: &str) -> Vec<&'static str> {
    match category.to_lowercase().as_str() {
        "image" => vec!["png", "jpg", "jpeg", "gif", "bmp", "svg", "webp", "ico", "tiff"],
        "video" => vec!["mp4", "mkv", "avi", "mov", "webm", "flv"],
        "audio" => vec!["mp3", "wav", "flac", "ogg", "aac", "m4a"],
        "document" => vec!["pdf", "doc", "docx", "odt", "txt", "rtf", "md"],
        "code" => vec!["rs", "py", "js", "ts", "c", "cpp", "h", "java", "go", "rb"],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn idx_with_files(names: &[&str]) -> FileIndex {
        let mut idx = FileIndex::new();
        for name in names {
            idx.add_path(Path::new(&format!("/home/user/{}", name)), false);
        }
        idx
    }

    // -- FileIndex basics -----------------------------------------------------

    #[test]
    fn index_empty() {
        let idx = FileIndex::new();
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    #[test]
    fn index_add_path() {
        let mut idx = FileIndex::new();
        idx.add_path(Path::new("/tmp/hello.txt"), false);
        assert_eq!(idx.len(), 1);
        assert!(!idx.is_empty());
    }

    #[test]
    fn index_add_directory() {
        let mut idx = FileIndex::new();
        idx.add_path(Path::new("/tmp/subdir"), true);
        assert_eq!(idx.entries[0].is_dir, true);
        assert_eq!(idx.entries[0].extension, "");
    }

    #[test]
    fn index_clear() {
        let mut idx = idx_with_files(&["a.txt", "b.txt"]);
        assert_eq!(idx.len(), 2);
        idx.clear();
        assert_eq!(idx.len(), 0);
    }

    // -- search ---------------------------------------------------------------

    #[test]
    fn search_exact() {
        let idx = idx_with_files(&["readme.md", "main.rs", "config.toml"]);
        let r = idx.search("readme.md", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "readme.md");
    }

    #[test]
    fn search_prefix() {
        let idx = idx_with_files(&["readme.md", "readme.txt", "main.rs"]);
        let r = idx.search("read", 10);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn search_contains() {
        let idx = idx_with_files(&["my_readme.md", "main.rs"]);
        let r = idx.search("readme", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_no_match() {
        let idx = idx_with_files(&["hello.txt"]);
        assert!(idx.search("zzz", 10).is_empty());
    }

    #[test]
    fn search_case_insensitive() {
        let idx = idx_with_files(&["README.md"]);
        assert_eq!(idx.search("readme", 10).len(), 1);
    }

    #[test]
    fn search_max_results() {
        let names: Vec<String> = (0..20).map(|i| format!("file_{}.txt", i)).collect();
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let idx = idx_with_files(&name_refs);
        let r = idx.search("file", 5);
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn search_exact_ranks_first() {
        let idx = idx_with_files(&["test_utils.rs", "test.rs", "testing.rs"]);
        let r = idx.search("test.rs", 10);
        assert_eq!(r[0].name, "test.rs");
    }

    // -- search_ext -----------------------------------------------------------

    #[test]
    fn search_ext_filter() {
        let idx = idx_with_files(&["main.rs", "style.css", "lib.rs", "index.html"]);
        let r = idx.search_ext("main", "rs", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "main.rs");
    }

    #[test]
    fn search_ext_case_insensitive() {
        let idx = idx_with_files(&["photo.PNG"]);
        let r = idx.search_ext("photo", "png", 10);
        assert_eq!(r.len(), 1);
    }

    // -- search_mime ----------------------------------------------------------

    #[test]
    fn search_mime_image() {
        let idx = idx_with_files(&["pic.png", "vid.mp4", "doc.pdf"]);
        let r = idx.search_mime("pic", "image", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "pic.png");
    }

    #[test]
    fn search_mime_code() {
        let idx = idx_with_files(&["main.rs", "style.css", "index.html"]);
        let r = idx.search_mime("main", "code", 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_mime_unknown_category() {
        let idx = idx_with_files(&["main.rs"]);
        assert!(idx.search_mime("main", "spreadsheet", 10).is_empty());
    }

    // -- icon_for_extension ---------------------------------------------------

    #[test]
    fn icon_source() {
        assert_eq!(icon_for_extension("rs"), "text-x-source");
        assert_eq!(icon_for_extension("py"), "text-x-source");
    }

    #[test]
    fn icon_image() {
        assert_eq!(icon_for_extension("png"), "image-x-generic");
    }

    #[test]
    fn icon_fallback() {
        assert_eq!(icon_for_extension("xyz"), "text-x-generic");
    }

    // -- mime_extensions ------------------------------------------------------

    #[test]
    fn mime_image_extensions() {
        let exts = mime_extensions("image");
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"jpg"));
    }

    #[test]
    fn mime_unknown_empty() {
        assert!(mime_extensions("foobar").is_empty());
    }

    // -- FileSearchProvider ---------------------------------------------------

    #[test]
    fn provider_metadata() {
        let p = FileSearchProvider::new();
        assert_eq!(p.id(), "files");
        assert_eq!(p.name(), "Files");
        assert_eq!(p.priority(), 60);
    }

    #[test]
    fn provider_search_returns_results() {
        let mut p = FileSearchProvider::new();
        p.index_mut().add_path(Path::new("/home/user/readme.md"), false);
        p.index_mut().add_path(Path::new("/home/user/main.rs"), false);

        let r = p.search("readme", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "readme.md");
        assert_eq!(r[0].category, SearchCategory::File);
        assert!(matches!(r[0].action, SearchResultAction::Open(_)));
    }

    #[test]
    fn provider_empty_index() {
        let p = FileSearchProvider::new();
        assert!(p.search("anything", 10).is_empty());
    }

    #[test]
    fn provider_directory_icon() {
        let mut p = FileSearchProvider::new();
        p.index_mut().add_path(Path::new("/home/user/docs"), true);
        let r = p.search("docs", 10);
        assert_eq!(r[0].icon, "folder");
    }
}
