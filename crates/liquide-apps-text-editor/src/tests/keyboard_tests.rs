//! Tests for keyboard handling, file I/O, undo/redo integration, and visible_lines.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::config::EditorConfig;
use crate::document::{Document, LineEnding};
use crate::runtime::EditorRuntime;

/// Generate a unique temp directory for each test invocation to avoid
/// cross-process and cross-run collisions during `cargo test --workspace`.
fn unique_test_dir(name: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("{name}_{}_{id}", std::process::id()));
    // Clean up any stale directory from a previous run.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("failed to create test temp dir");
    dir
}

// ===========================================================================
// LineEnding detection
// ===========================================================================

#[test]
fn test_line_ending_lf() {
    let doc = Document::from_file(1, "test.txt", "a\nb\nc", 100);
    assert_eq!(doc.line_ending, LineEnding::Lf);
}

#[test]
fn test_line_ending_crlf() {
    let doc = Document::from_file(1, "test.txt", "a\r\nb\r\nc", 100);
    assert_eq!(doc.line_ending, LineEnding::CrLf);
}

#[test]
fn test_line_ending_cr() {
    let doc = Document::from_file(1, "test.txt", "a\rb\rc", 100);
    assert_eq!(doc.line_ending, LineEnding::Cr);
}

#[test]
fn test_line_ending_as_str() {
    assert_eq!(LineEnding::Lf.as_str(), "\n");
    assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    assert_eq!(LineEnding::Cr.as_str(), "\r");
}

// ===========================================================================
// File I/O (disk)
// ===========================================================================

#[test]
fn test_document_open_and_save() {
    let dir = unique_test_dir("liquide_editor_test_open_save");
    let path = dir.join("test_file.txt");
    std::fs::write(&path, "hello\nworld").unwrap();

    let mut doc = Document::open(1, &path, 100).unwrap();
    assert_eq!(doc.buffer.line_count(), 2);
    assert_eq!(doc.buffer.line(0), Some("hello"));
    assert_eq!(doc.buffer.line(1), Some("world"));
    assert_eq!(doc.title, "test_file.txt");
    assert!(!doc.is_modified());

    // Modify and save.
    doc.buffer.insert_char(0, 5, '!').unwrap();
    assert!(doc.is_modified());
    doc.save().unwrap();
    assert!(!doc.is_modified());

    // Re-read and verify.
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello!\nworld");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_document_save_as() {
    let dir = unique_test_dir("liquide_editor_test_save_as");
    let path1 = dir.join("original.txt");
    let path2 = dir.join("saved_as.rs");
    std::fs::write(&path1, "content").unwrap();

    let mut doc = Document::open(1, &path1, 100).unwrap();
    assert_eq!(doc.language_name(), "Plain Text");

    doc.save_as(&path2).unwrap();
    assert_eq!(doc.title, "saved_as.rs");
    assert_eq!(doc.language_name(), "Rust");

    let content = std::fs::read_to_string(&path2).unwrap();
    assert_eq!(content, "content");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_document_save_no_path() {
    let mut doc = Document::new(1, 100);
    let result = doc.save();
    assert!(result.is_err());
}

#[test]
fn test_runtime_open_path() {
    let dir = unique_test_dir("liquide_editor_test_open_path");
    let path = dir.join("hello.rs");
    std::fs::write(&path, "fn main() {}").unwrap();

    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.open_path(&path).unwrap();
    let doc = rt.document(id).unwrap();
    assert_eq!(doc.language_name(), "Rust");
    assert_eq!(doc.buffer.line(0), Some("fn main() {}"));

    // Opening the same path again returns the same ID.
    let id2 = rt.open_path(&path).unwrap();
    assert_eq!(id, id2);
    assert_eq!(rt.document_count(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// Keyboard: character insertion
// ===========================================================================

#[test]
fn test_handle_char_inserts() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();

    assert!(rt.handle_char('H'));
    assert!(rt.handle_char('i'));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line(0), Some("Hi"));
    assert_eq!(doc.cursors.primary().position.col, 2);
}

#[test]
fn test_handle_char_no_document() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    assert!(!rt.handle_char('x'));
}

// ===========================================================================
// Keyboard: Enter
// ===========================================================================

#[test]
fn test_handle_enter() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "hello world");

    // Move cursor to col 5
    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 5));

    let modified = rt.handle_key("Enter", false, false);
    assert!(modified);

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line_count(), 2);
    assert_eq!(doc.buffer.line(0), Some("hello"));
    assert_eq!(doc.buffer.line(1), Some(" world"));
    assert_eq!(doc.cursors.primary().position.line, 1);
    assert_eq!(doc.cursors.primary().position.col, 0);
}

// ===========================================================================
// Keyboard: Backspace
// ===========================================================================

#[test]
fn test_handle_backspace_mid_line() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 2));

    assert!(rt.handle_key("Backspace", false, false));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line(0), Some("ac"));
    assert_eq!(doc.cursors.primary().position.col, 1);
}

#[test]
fn test_handle_backspace_start_of_line() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "ab\ncd");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(1, 0));

    assert!(rt.handle_key("Backspace", false, false));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line_count(), 1);
    assert_eq!(doc.buffer.line(0), Some("abcd"));
    assert_eq!(doc.cursors.primary().position.line, 0);
    assert_eq!(doc.cursors.primary().position.col, 2);
}

#[test]
fn test_handle_backspace_at_origin() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "hello");

    // Cursor already at (0, 0).
    assert!(!rt.handle_key("Backspace", false, false));
}

// ===========================================================================
// Keyboard: Delete
// ===========================================================================

#[test]
fn test_handle_delete_mid_line() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 1));

    assert!(rt.handle_key("Delete", false, false));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line(0), Some("ac"));
}

#[test]
fn test_handle_delete_end_of_line() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "ab\ncd");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 2));

    assert!(rt.handle_key("Delete", false, false));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line_count(), 1);
    assert_eq!(doc.buffer.line(0), Some("abcd"));
}

// ===========================================================================
// Keyboard: Tab
// ===========================================================================

#[test]
fn test_handle_tab_spaces() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "x");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 0));

    assert!(rt.handle_key("Tab", false, false));

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.buffer.line(0), Some("    x"));
    assert_eq!(doc.cursors.primary().position.col, 4);
}

// ===========================================================================
// Keyboard: Cursor movement
// ===========================================================================

#[test]
fn test_arrow_keys() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc\ndef\nghi");

    // Start at (0, 0), move right 2, then down 1.
    rt.handle_key("ArrowRight", false, false);
    rt.handle_key("ArrowRight", false, false);
    rt.handle_key("ArrowDown", false, false);

    let doc = rt.active_document().unwrap();
    assert_eq!(doc.cursors.primary().position.line, 1);
    assert_eq!(doc.cursors.primary().position.col, 2);

    // Move left to wrap to previous line end.
    rt.handle_key("ArrowLeft", false, false);
    rt.handle_key("ArrowLeft", false, false);
    rt.handle_key("ArrowLeft", false, false);
    let doc = rt.active_document().unwrap();
    assert_eq!(doc.cursors.primary().position.line, 0);
    assert_eq!(doc.cursors.primary().position.col, 3); // end of "abc"
}

#[test]
fn test_home_end() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "hello world");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 5));

    rt.handle_key("Home", false, false);
    assert_eq!(
        rt.active_document().unwrap().cursors.primary().position.col,
        0
    );

    rt.handle_key("End", false, false);
    assert_eq!(
        rt.active_document().unwrap().cursors.primary().position.col,
        11
    );
}

#[test]
fn test_ctrl_home_end() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc\ndef\nghi");

    rt.handle_key("End", true, false);
    let doc = rt.active_document().unwrap();
    assert_eq!(doc.cursors.primary().position.line, 2);
    assert_eq!(doc.cursors.primary().position.col, 3);

    rt.handle_key("Home", true, false);
    let doc = rt.active_document().unwrap();
    assert_eq!(doc.cursors.primary().position.line, 0);
    assert_eq!(doc.cursors.primary().position.col, 0);
}

#[test]
fn test_page_up_down() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    // Create a multi-line doc.
    let content: String = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    rt.open_file("t.txt", &content);

    rt.handle_key("PageDown", false, false);
    assert_eq!(
        rt.active_document()
            .unwrap()
            .cursors
            .primary()
            .position
            .line,
        30
    );

    rt.handle_key("PageUp", false, false);
    assert_eq!(
        rt.active_document()
            .unwrap()
            .cursors
            .primary()
            .position
            .line,
        0
    );
}

// ===========================================================================
// Keyboard: Selection
// ===========================================================================

#[test]
fn test_shift_arrow_selection() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "hello");

    rt.handle_key("ArrowRight", false, true);
    rt.handle_key("ArrowRight", false, true);
    rt.handle_key("ArrowRight", false, true);

    let doc = rt.active_document().unwrap();
    assert!(doc.cursors.primary().has_selection());
    let sel = doc.cursors.primary().selection.unwrap();
    assert_eq!(sel.start().col, 0);
    assert_eq!(sel.end().col, 3);
}

#[test]
fn test_select_all() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc\ndef");

    rt.handle_key("a", true, false);

    let doc = rt.active_document().unwrap();
    assert!(doc.cursors.primary().has_selection());
    let sel = doc.cursors.primary().selection.unwrap();
    assert_eq!(sel.start(), crate::cursor::Position::new(0, 0));
    assert_eq!(sel.end(), crate::cursor::Position::new(1, 3));
}

// ===========================================================================
// Keyboard: Undo / Redo
// ===========================================================================

#[test]
fn test_undo_redo_char_insert() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();

    rt.handle_char('a');
    rt.handle_char('b');
    rt.handle_char('c');
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("abc"));

    // Undo last char.
    assert!(rt.handle_key("z", true, false));
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("ab"));

    // Redo.
    assert!(rt.handle_key("y", true, false));
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("abc"));
}

#[test]
fn test_undo_redo_newline() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "hello world");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 5));

    rt.handle_key("Enter", false, false);
    assert_eq!(rt.active_document().unwrap().buffer.line_count(), 2);

    // Undo.
    rt.handle_key("z", true, false);
    assert_eq!(rt.active_document().unwrap().buffer.line_count(), 1);
    assert_eq!(
        rt.active_document().unwrap().buffer.line(0),
        Some("hello world")
    );

    // Redo.
    rt.handle_key("y", true, false);
    assert_eq!(rt.active_document().unwrap().buffer.line_count(), 2);
}

#[test]
fn test_undo_backspace() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 3));

    rt.handle_key("Backspace", false, false);
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("ab"));

    rt.handle_key("z", true, false);
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("abc"));
}

#[test]
fn test_undo_delete() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "abc");

    // Cursor at col 1, delete 'b'.
    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 1));

    rt.handle_key("Delete", false, false);
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("ac"));

    rt.handle_key("z", true, false);
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("abc"));
}

#[test]
fn test_undo_join_line_via_backspace() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "ab\ncd");

    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(1, 0));

    rt.handle_key("Backspace", false, false);
    assert_eq!(rt.active_document().unwrap().buffer.line_count(), 1);

    rt.handle_key("z", true, false);
    assert_eq!(rt.active_document().unwrap().buffer.line_count(), 2);
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("ab"));
    assert_eq!(rt.active_document().unwrap().buffer.line(1), Some("cd"));
}

// ===========================================================================
// Ctrl+S save
// ===========================================================================

#[test]
fn test_ctrl_s_save() {
    let dir = unique_test_dir("liquide_editor_test_ctrl_s");
    let path = dir.join("save_test.txt");
    std::fs::write(&path, "original").unwrap();

    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_path(&path).unwrap();

    // Type a character then Ctrl+S.
    let doc = rt.active_document_mut().unwrap();
    doc.cursors
        .primary_mut()
        .move_to(crate::cursor::Position::new(0, 8));
    rt.handle_char('!');
    rt.handle_key("s", true, false);

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "original!");

    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// visible_lines
// ===========================================================================

#[test]
fn test_visible_lines_basic() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.rs", "fn main() {\n    println!(\"hi\");\n}");

    let lines = rt.visible_lines(0, 10);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].number, 1);
    assert_eq!(lines[0].text, "fn main() {");
    assert!(lines[0].is_current); // cursor at line 0
    assert!(!lines[1].is_current);
    // Rust source should have highlights.
    assert!(!lines[0].highlights.is_empty());
}

#[test]
fn test_visible_lines_scroll_offset() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let content: String = (0..50)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    rt.open_file("t.txt", &content);

    let lines = rt.visible_lines(10, 5);
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0].number, 11); // 1-based, offset=10 -> line index 10 -> number 11
    assert_eq!(lines[0].text, "line 10");
}

#[test]
fn test_visible_lines_no_document() {
    let rt = EditorRuntime::new(EditorConfig::default());
    let lines = rt.visible_lines(0, 10);
    assert!(lines.is_empty());
}

#[test]
fn test_visible_lines_clamps_to_buffer_end() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.open_file("t.txt", "a\nb\nc");

    let lines = rt.visible_lines(0, 100);
    assert_eq!(lines.len(), 3);
}

// ===========================================================================
// Highlighter::detect
// ===========================================================================

#[test]
fn test_highlighter_detect() {
    use crate::syntax::Highlighter;
    use std::path::Path;

    let h = Highlighter::detect(Path::new("foo.rs"));
    assert_eq!(h.language_name(), "Rust");

    let h = Highlighter::detect(Path::new("bar.py"));
    assert_eq!(h.language_name(), "Python");

    let h = Highlighter::detect(Path::new("baz.txt"));
    assert_eq!(h.language_name(), "Plain Text");
}

// ===========================================================================
// Buffer::from_lines
// ===========================================================================

#[test]
fn test_buffer_from_lines() {
    use crate::buffer::TextBuffer;

    let b = TextBuffer::from_lines(vec!["hello".into(), "world".into()]);
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), Some("hello"));

    // Empty vec gets one empty line.
    let b = TextBuffer::from_lines(vec![]);
    assert_eq!(b.line_count(), 1);
    assert_eq!(b.line(0), Some(""));
}

// ===========================================================================
// handle_key returns false for no-document cases
// ===========================================================================

#[test]
fn test_handle_key_no_document() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    assert!(!rt.handle_key("Enter", false, false));
    assert!(!rt.handle_key("z", true, false));
}

// ===========================================================================
// Undo with nothing to undo
// ===========================================================================

#[test]
fn test_undo_nothing() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();
    // No edits yet, undo should return false.
    assert!(!rt.handle_key("z", true, false));
}

// ===========================================================================
// Ctrl+Shift+Z redo
// ===========================================================================

#[test]
fn test_ctrl_shift_z_redo() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();

    rt.handle_char('x');
    rt.handle_key("z", true, false); // undo
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some(""));

    // Ctrl+Shift+Z should redo.
    assert!(rt.handle_key("z", true, true));
    assert_eq!(rt.active_document().unwrap().buffer.line(0), Some("x"));
}
