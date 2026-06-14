//! Per-app smoke test for the file manager (t57 A7 / t57-e8).
//!
//! Builds the files runtime and asserts the root view model is populated:
//! the sidebar carries the default system bookmarks, and navigating to a
//! directory with entries reflects them in the current listing. Real model
//! assertions, not bare construction.

use liquide_apps_files::config::FilesConfig;
use liquide_apps_files::entry::FileEntry;
use liquide_apps_files::runtime::FilesRuntime;

#[test]
fn root_view_has_default_sidebar_bookmarks() {
    let rt = FilesRuntime::new(FilesConfig::default());
    let bookmarks = rt.sidebar().bookmarks();
    assert!(
        !bookmarks.is_empty(),
        "sidebar must not be an empty placeholder; expected default system bookmarks"
    );
    // Home is one of the canonical default bookmarks.
    assert!(
        bookmarks.iter().any(|b| b.name == "Home"),
        "default bookmarks should include 'Home', got {:?}",
        bookmarks.iter().map(|b| &b.name).collect::<Vec<_>>()
    );
}

#[test]
fn navigating_populates_the_current_listing() {
    let mut rt = FilesRuntime::new(FilesConfig::default());
    let entries = vec![
        FileEntry::file("a.txt".into(), "/tmp/a.txt".into(), 10, 0),
        FileEntry::file("b.txt".into(), "/tmp/b.txt".into(), 20, 0),
    ];
    rt.navigate("/tmp".into(), entries);

    let listing = rt.current_listing();
    assert_eq!(listing.path, "/tmp");
    assert!(
        listing.visible_count() >= 2,
        "listing should reflect the two navigated entries, got {}",
        listing.visible_count()
    );
}
