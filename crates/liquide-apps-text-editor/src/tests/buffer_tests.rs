//! Tests for the text buffer.

use crate::buffer::TextBuffer;
use crate::cursor::{Cursor, MultiCursor, Position, Selection};
use crate::gutter::{Diagnostic, Gutter};
use crate::indent::{IndentStyle, auto_indent, detect_indent, indent_level, leading_whitespace};
use crate::search::SearchReplace;
use crate::undo::{EditOp, UndoHistory};

// ===========================================================================
// TextBuffer
// ===========================================================================

#[test]
fn test_buffer_new() {
    let b = TextBuffer::new();
    assert_eq!(b.line_count(), 1);
    assert_eq!(b.line(0), Some(""));
    assert!(!b.is_modified());
}

#[test]
fn test_buffer_from_text() {
    let b = TextBuffer::from_text("hello\nworld");
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), Some("hello"));
    assert_eq!(b.line(1), Some("world"));
}

#[test]
fn test_buffer_from_empty() {
    let b = TextBuffer::from_text("");
    assert_eq!(b.line_count(), 1);
}

#[test]
fn test_buffer_insert_char() {
    let mut b = TextBuffer::from_text("hello");
    b.insert_char(0, 5, '!').unwrap();
    assert_eq!(b.line(0), Some("hello!"));
    assert!(b.is_modified());
}

#[test]
fn test_buffer_delete_char() {
    let mut b = TextBuffer::from_text("hello");
    let ch = b.delete_char(0, 4).unwrap();
    assert_eq!(ch, 'o');
    assert_eq!(b.line(0), Some("hell"));
}

#[test]
fn test_buffer_insert_newline() {
    let mut b = TextBuffer::from_text("hello world");
    b.insert_newline(0, 5).unwrap();
    assert_eq!(b.line_count(), 2);
    assert_eq!(b.line(0), Some("hello"));
    assert_eq!(b.line(1), Some(" world"));
}

#[test]
fn test_buffer_join_line_up() {
    let mut b = TextBuffer::from_text("hello\nworld");
    let col = b.join_line_up(1).unwrap();
    assert_eq!(col, 5);
    assert_eq!(b.line(0), Some("helloworld"));
    assert_eq!(b.line_count(), 1);
}

#[test]
fn test_buffer_delete_range_single_line() {
    let mut b = TextBuffer::from_text("hello world");
    let deleted = b.delete_range(0, 5, 0, 11).unwrap();
    assert_eq!(deleted, " world");
    assert_eq!(b.line(0), Some("hello"));
}

#[test]
fn test_buffer_delete_range_multi_line() {
    let mut b = TextBuffer::from_text("line1\nline2\nline3");
    let deleted = b.delete_range(0, 3, 2, 3).unwrap();
    assert_eq!(deleted, "e1\nline2\nlin");
    assert_eq!(b.line_count(), 1);
    assert_eq!(b.line(0), Some("line3"));
}

#[test]
fn test_buffer_char_count() {
    let b = TextBuffer::from_text("ab\ncd");
    assert_eq!(b.char_count(), 5); // a, b, \n, c, d
}

#[test]
fn test_buffer_text() {
    let b = TextBuffer::from_text("a\nb\nc");
    assert_eq!(b.text(), "a\nb\nc");
}

#[test]
fn test_buffer_mark_saved() {
    let mut b = TextBuffer::from_text("test");
    b.insert_char(0, 0, 'x').unwrap();
    assert!(b.is_modified());
    b.mark_saved();
    assert!(!b.is_modified());
}

#[test]
fn test_buffer_line_out_of_range() {
    let b = TextBuffer::from_text("test");
    assert!(b.line(5).is_none());
}

#[test]
fn test_buffer_insert_char_out_of_range() {
    let mut b = TextBuffer::from_text("test");
    assert!(b.insert_char(0, 100, 'x').is_err());
    assert!(b.insert_char(5, 0, 'x').is_err());
}

// ===========================================================================
// Cursor / Selection
// ===========================================================================

#[test]
fn test_position_ordering() {
    let a = Position::new(0, 5);
    let b = Position::new(1, 0);
    assert!(a < b);
}

#[test]
fn test_selection_start_end() {
    let sel = Selection::new(Position::new(1, 5), Position::new(0, 3));
    assert_eq!(sel.start(), Position::new(0, 3));
    assert_eq!(sel.end(), Position::new(1, 5));
}

#[test]
fn test_selection_empty() {
    let sel = Selection::new(Position::new(0, 5), Position::new(0, 5));
    assert!(sel.is_empty());
}

#[test]
fn test_selection_multiline() {
    let sel = Selection::new(Position::new(0, 0), Position::new(1, 0));
    assert!(sel.is_multiline());
}

#[test]
fn test_cursor_move_to() {
    let mut c = Cursor::new();
    c.move_to(Position::new(3, 7));
    assert_eq!(c.position, Position::new(3, 7));
    assert!(!c.has_selection());
}

#[test]
fn test_cursor_select_to() {
    let mut c = Cursor::new();
    c.move_to(Position::new(0, 5));
    c.select_to(Position::new(0, 10));
    assert!(c.has_selection());
    let sel = c.selection.unwrap();
    assert_eq!(sel.anchor, Position::new(0, 5));
    assert_eq!(sel.cursor, Position::new(0, 10));
}

#[test]
fn test_cursor_select_line() {
    let mut c = Cursor::new();
    c.select_line(3, 20);
    assert!(c.has_selection());
}

#[test]
fn test_multi_cursor() {
    let mut mc = MultiCursor::new();
    assert_eq!(mc.count(), 1);
    mc.add_cursor(Position::new(5, 0));
    assert_eq!(mc.count(), 2);
    mc.collapse();
    assert_eq!(mc.count(), 1);
}

// ===========================================================================
// Search/Replace
// ===========================================================================

#[test]
fn test_search_basic() {
    let mut s = SearchReplace::new();
    let lines = vec!["hello world".into(), "world peace".into()];
    s.search("world", &lines);
    assert_eq!(s.match_count(), 2);
}

#[test]
fn test_search_case_insensitive() {
    let mut s = SearchReplace::new();
    s.set_case_sensitive(false);
    let lines = vec!["Hello WORLD".into()];
    s.search("hello", &lines);
    assert_eq!(s.match_count(), 1);
}

#[test]
fn test_search_whole_word() {
    let mut s = SearchReplace::new();
    s.set_whole_word(true);
    let lines = vec!["testing test tested".into()];
    s.search("test", &lines);
    assert_eq!(s.match_count(), 1);
}

#[test]
fn test_search_navigation() {
    let mut s = SearchReplace::new();
    let lines = vec!["aaa".into(), "aaa".into()];
    s.search("aaa", &lines);
    assert_eq!(s.current_index(), 0);
    s.next_match();
    assert_eq!(s.current_index(), 1);
    s.next_match();
    assert_eq!(s.current_index(), 0); // wraps
    s.prev_match();
    assert_eq!(s.current_index(), 1); // wraps back
}

#[test]
fn test_search_clear() {
    let mut s = SearchReplace::new();
    let lines = vec!["test".into()];
    s.search("test", &lines);
    s.clear();
    assert_eq!(s.match_count(), 0);
    assert!(s.query().is_empty());
}

// ===========================================================================
// Undo/Redo
// ===========================================================================

#[test]
fn test_undo_history() {
    let mut h = UndoHistory::new(100);
    assert!(!h.can_undo());
    h.record(EditOp::Insert {
        line: 0,
        col: 0,
        text: "a".into(),
    });
    assert!(h.can_undo());
    assert_eq!(h.undo_depth(), 1);
}

#[test]
fn test_undo_redo() {
    let mut h = UndoHistory::new(100);
    h.record(EditOp::Insert {
        line: 0,
        col: 0,
        text: "a".into(),
    });
    let op = h.undo().unwrap();
    assert!(matches!(op, EditOp::Insert { .. }));
    assert!(h.can_redo());
    let op = h.redo().unwrap();
    assert!(matches!(op, EditOp::Insert { .. }));
}

#[test]
fn test_undo_limit() {
    let mut h = UndoHistory::new(3);
    for i in 0..5 {
        h.record(EditOp::Insert {
            line: 0,
            col: i,
            text: "x".into(),
        });
    }
    assert_eq!(h.undo_depth(), 3);
}

#[test]
fn test_undo_clears_redo() {
    let mut h = UndoHistory::new(100);
    h.record(EditOp::Insert {
        line: 0,
        col: 0,
        text: "a".into(),
    });
    h.undo();
    assert!(h.can_redo());
    h.record(EditOp::Insert {
        line: 0,
        col: 0,
        text: "b".into(),
    });
    assert!(!h.can_redo());
}

// ===========================================================================
// Indent
// ===========================================================================

#[test]
fn test_leading_whitespace() {
    assert_eq!(leading_whitespace("    hello"), "    ");
    assert_eq!(leading_whitespace("hello"), "");
}

#[test]
fn test_indent_level() {
    assert_eq!(indent_level("        code", 4), 2);
    assert_eq!(indent_level("code", 4), 0);
}

#[test]
fn test_auto_indent_brace() {
    let indent = auto_indent("    if true {", IndentStyle::Spaces(4));
    assert_eq!(indent, "        ");
}

#[test]
fn test_auto_indent_plain() {
    let indent = auto_indent("    hello", IndentStyle::Spaces(4));
    assert_eq!(indent, "    ");
}

#[test]
fn test_detect_indent_tabs() {
    let lines = vec!["\tfoo".into(), "\tbar".into(), " baz".into()];
    assert_eq!(detect_indent(&lines), IndentStyle::Tabs);
}

// ===========================================================================
// Gutter
// ===========================================================================

#[test]
fn test_gutter_width() {
    let mut g = Gutter::new();
    g.update_width(100);
    assert_eq!(g.width(), 3);
    g.update_width(10000);
    assert_eq!(g.width(), 5);
}

#[test]
fn test_gutter_diagnostics() {
    let mut g = Gutter::new();
    g.set_diagnostics(vec![
        Diagnostic::error(0, "error", "test"),
        Diagnostic::warning(0, "warning", "test"),
        Diagnostic::error(5, "error2", "test"),
    ]);
    assert_eq!(g.error_count(), 2);
    assert_eq!(g.warning_count(), 1);
    assert_eq!(g.diagnostics_for(0).len(), 2);
}

#[test]
fn test_gutter_breakpoints() {
    let mut g = Gutter::new();
    g.toggle_breakpoint(5);
    assert!(g.has_breakpoint(5));
    g.toggle_breakpoint(5);
    assert!(!g.has_breakpoint(5));
}
