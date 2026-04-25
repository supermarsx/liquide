//! Additional syntax highlighting tests.

use crate::syntax::{Highlighter, Language, TokenKind};

#[test]
fn test_python_keywords() {
    let lang = Language::from_extension("py").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("def hello():");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Keyword));
}

#[test]
fn test_python_comment() {
    let lang = Language::from_extension("py").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("# comment");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Comment);
}

#[test]
fn test_javascript_keywords() {
    let lang = Language::from_extension("js").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("const x = 42;");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Keyword));
}

#[test]
fn test_c_keywords() {
    let lang = Language::from_extension("c").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("int main() { return 0; }");
    let keywords: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Keyword)
        .collect();
    assert!(keywords.len() >= 2); // int, return
}

#[test]
fn test_toml_keywords() {
    let lang = Language::from_extension("toml").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("enabled = true");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Keyword));
}

#[test]
fn test_operators() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("a + b");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Operator));
}

#[test]
fn test_punctuation() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("fn()");
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Punctuation));
}

#[test]
fn test_empty_line() {
    let lang = Language::from_extension("rs").unwrap();
    let h = Highlighter::new(Some(lang));
    let tokens = h.tokenize_line("");
    assert!(tokens.is_empty());
}
