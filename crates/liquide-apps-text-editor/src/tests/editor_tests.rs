//! Tests for document and runtime.

use crate::config::EditorConfig;
use crate::document::Document;
use crate::runtime::EditorRuntime;
use crate::syntax::{Highlighter, Language, TokenKind};

// ===========================================================================
// Syntax
// ===========================================================================

#[test]
fn test_language_from_extension() {
    assert!(Language::from_extension("rs").is_some());
    assert!(Language::from_extension("py").is_some());
    assert!(Language::from_extension("js").is_some());
    assert!(Language::from_extension("xyz").is_none());
}

#[test]
fn test_highlighter_plain() {
    let h = Highlighter::new(None);
    assert_eq!(h.language_name(), "Plain Text");
    let tokens = h.tokenize_line("hello world");
    assert_eq!(tokens.len(), 1);
}

#[test]
fn test_highlighter_rust_keyword() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("fn main() {");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Keyword));
}

#[test]
fn test_highlighter_rust_string() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("let s = \"hello\";");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::String));
}

#[test]
fn test_highlighter_comment() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("// this is a comment");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Comment);
}

#[test]
fn test_highlighter_number() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("let x = 42;");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Number));
}

#[test]
fn test_highlighter_type() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("let x: u32 = 0;");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Type));
}

// ===========================================================================
// Document
// ===========================================================================

#[test]
fn test_document_new() {
    let doc = Document::new(1, 100);
    assert_eq!(doc.id, 1);
    assert_eq!(doc.title, "Untitled");
    assert!(!doc.is_modified());
    assert_eq!(doc.language_name(), "Plain Text");
}

#[test]
fn test_document_from_file() {
    let doc = Document::from_file(1, "src/main.rs", "fn main() {}", 100);
    assert_eq!(doc.title, "main.rs");
    assert_eq!(doc.language_name(), "Rust");
    assert!(doc.path.as_deref() == Some("src/main.rs"));
}

#[test]
fn test_document_display_title() {
    let mut doc = Document::new(1, 100);
    assert_eq!(doc.display_title(), "Untitled");
    doc.buffer.insert_char(0, 0, 'x').unwrap();
    assert_eq!(doc.display_title(), "Untitled *");
}

#[test]
fn test_document_status_info() {
    let doc = Document::new(1, 100);
    let (line, col, lang) = doc.status_info();
    assert_eq!(line, 1);
    assert_eq!(col, 1);
    assert_eq!(lang, "Plain Text");
}

// ===========================================================================
// Runtime
// ===========================================================================

#[test]
fn test_runtime_new() {
    let rt = EditorRuntime::new(EditorConfig::default());
    assert_eq!(rt.document_count(), 0);
    assert!(rt.active_document().is_none());
}

#[test]
fn test_runtime_new_document() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.new_document();
    assert_eq!(rt.document_count(), 1);
    assert!(rt.active_document().is_some());
    assert_eq!(rt.active_document().unwrap().id, id);
}

#[test]
fn test_runtime_open_file() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.open_file("test.rs", "fn main() {}");
    assert_eq!(rt.document_count(), 1);
    let doc = rt.document(id).unwrap();
    assert_eq!(doc.title, "test.rs");
    assert_eq!(doc.language_name(), "Rust");
}

#[test]
fn test_runtime_open_same_file_twice() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id1 = rt.open_file("test.rs", "code");
    let id2 = rt.open_file("test.rs", "code");
    assert_eq!(id1, id2);
    assert_eq!(rt.document_count(), 1);
}

#[test]
fn test_runtime_close_document() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.new_document();
    rt.close_document(id).unwrap();
    assert_eq!(rt.document_count(), 0);
}

#[test]
fn test_runtime_close_nonexistent() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    assert!(rt.close_document(42).is_err());
}

#[test]
fn test_runtime_multiple_documents() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id1 = rt.new_document();
    let id2 = rt.new_document();
    assert_eq!(rt.document_count(), 2);
    rt.set_active(id1).unwrap();
    assert_eq!(rt.active_document().unwrap().id, id1);
    rt.set_active(id2).unwrap();
    assert_eq!(rt.active_document().unwrap().id, id2);
}

#[test]
fn test_runtime_document_list() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    rt.new_document();
    rt.open_file("test.py", "print(1)");
    let list = rt.document_list();
    assert_eq!(list.len(), 2);
}

#[test]
fn test_runtime_has_unsaved_changes() {
    let mut rt = EditorRuntime::new(EditorConfig::default());
    let id = rt.new_document();
    assert!(!rt.has_unsaved_changes());
    rt.document_mut(id)
        .unwrap()
        .buffer
        .insert_char(0, 0, 'x')
        .unwrap();
    assert!(rt.has_unsaved_changes());
}
