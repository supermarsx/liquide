//! CSS source parser.
//!
//! Implements a simple recursive-descent parser that handles the subset of CSS
//! used by the Liquide theming engine.  The parser is intentionally lenient:
//! malformed rules or declarations are skipped rather than causing errors, so
//! that partial or slightly-broken style sheets still produce useful results.

use crate::value::parse_value;
use crate::{StyleRule, StyleSheet};
use tracing::warn;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a CSS source string into a [`StyleSheet`].
///
/// Comments are stripped, rules are parsed one by one, and malformed rules are
/// silently skipped (with a `tracing::warn`).
///
/// # Errors
///
/// Returns `Ok` in all cases today; errors are reserved for future use when
/// strict mode is added.
pub fn parse(source: &str) -> crate::Result<StyleSheet> {
    let cleaned = strip_comments(source);
    let rules = parse_rules(&cleaned);
    Ok(StyleSheet {
        source: source.to_string(),
        rules,
    })
}

// ---------------------------------------------------------------------------
// Comment stripping
// ---------------------------------------------------------------------------

/// Remove all `/* ... */` block comments from the source.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Skip until closing `*/`.
            i += 2;
            while i + 1 < len {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            // If we ran off the end without finding `*/`, just stop.
            if i >= len {
                break;
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Rule parsing
// ---------------------------------------------------------------------------

/// Split the (comment-free) source into individual rules and parse each one.
fn parse_rules(source: &str) -> Vec<StyleRule> {
    let mut rules = Vec::new();
    let mut rest = source;

    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }

        // Find the next opening brace.
        let open = match rest.find('{') {
            Some(pos) => pos,
            None => {
                // No more rules — trailing garbage; skip it.
                break;
            }
        };

        let selector_part = &rest[..open];

        // Find the matching closing brace.  We support one level of nesting
        // (which is more than plain CSS needs, but is cheap insurance).
        let after_open = &rest[open + 1..];
        let close = match find_matching_brace(after_open) {
            Some(pos) => pos,
            None => {
                // Unterminated block — skip the rest.
                warn!("CSS: unterminated declaration block, skipping rest of input");
                break;
            }
        };

        let body = &after_open[..close];
        rest = &after_open[close + 1..];

        // Parse selectors (comma-separated).
        let selectors = parse_selectors(selector_part);
        if selectors.is_empty() {
            continue;
        }

        // Parse declarations.
        let declarations = parse_declarations(body);

        for selector in selectors {
            rules.push(StyleRule {
                selector,
                declarations: declarations.clone(),
            });
        }
    }

    rules
}

/// Find the index of the closing `}` that matches an already-consumed `{`.
/// Returns the byte offset *within* the given slice.
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth: usize = 1;
    let mut in_string: Option<char> = None;

    for (i, ch) in s.char_indices() {
        // Track quoted strings so that braces inside quotes are ignored.
        if let Some(q) = in_string {
            if ch == q {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Selector parsing
// ---------------------------------------------------------------------------

/// Split a raw selector string by commas and normalise each selector.
fn parse_selectors(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| normalise_selector(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Normalise whitespace in a single selector.
///
/// Consecutive whitespace between simple selectors is collapsed to a single
/// space (descendant combinator).  Leading/trailing whitespace is removed.
fn normalise_selector(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_was_space = true; // suppress leading space

    for ch in raw.chars() {
        if ch.is_ascii_whitespace() {
            if !prev_was_space && !out.is_empty() {
                out.push(' ');
                prev_was_space = true;
            }
        } else {
            out.push(ch);
            prev_was_space = false;
        }
    }

    // Remove trailing space.
    if out.ends_with(' ') {
        out.pop();
    }

    out
}

// ---------------------------------------------------------------------------
// Declaration parsing
// ---------------------------------------------------------------------------

/// Parse the inside of a `{ ... }` block into a list of `(property, CssValue)`
/// pairs.  Malformed declarations are skipped.
fn parse_declarations(block: &str) -> Vec<(String, crate::CssValue)> {
    let mut declarations = Vec::new();

    for decl in split_declarations(block) {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }

        // Split at the first colon.
        let colon = match decl.find(':') {
            Some(pos) => pos,
            None => {
                warn!("CSS: declaration missing colon, skipping: {:?}", decl);
                continue;
            }
        };

        let property = decl[..colon].trim();
        let value_raw = decl[colon + 1..].trim();

        if property.is_empty() || value_raw.is_empty() {
            warn!(
                "CSS: empty property or value, skipping: {:?} -> {:?}",
                property, value_raw
            );
            continue;
        }

        let property = property.to_ascii_lowercase();
        let value = parse_value(value_raw);
        declarations.push((property, value));
    }

    declarations
}

/// Split a declaration block body by `;`, being careful not to split inside
/// parentheses (e.g. `rgb(1, 2, 3)`) or quoted strings.
fn split_declarations(block: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut in_string: Option<char> = None;
    let mut start = 0;

    for (i, ch) in block.char_indices() {
        if let Some(q) = in_string {
            if ch == q {
                in_string = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
            }
            ';' if depth == 0 => {
                parts.push(&block[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    // Grab any trailing declaration without a final semicolon.
    let trailing = block[start..].trim();
    if !trailing.is_empty() {
        parts.push(&block[start..]);
    }

    parts
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, CssValue, LengthUnit};

    #[test]
    fn empty_input() {
        let sheet = parse("").unwrap();
        assert!(sheet.rules.is_empty());
    }

    #[test]
    fn single_rule() {
        let css = ".button { background-color: #ff0000; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector, ".button");
        assert_eq!(sheet.rules[0].declarations.len(), 1);
        assert_eq!(sheet.rules[0].declarations[0].0, "background-color");
        assert_eq!(
            sheet.rules[0].declarations[0].1,
            CssValue::Color(Color { r: 255, g: 0, b: 0, a: 1.0 })
        );
    }

    #[test]
    fn multiple_selectors() {
        let css = ".a, .b { color: white; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].selector, ".a");
        assert_eq!(sheet.rules[1].selector, ".b");
    }

    #[test]
    fn pseudo_class_selector() {
        let css = ".button:hover { opacity: 0.8; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].selector, ".button:hover");
        assert_eq!(
            sheet.rules[0].declarations[0],
            ("opacity".to_string(), CssValue::Number(0.8))
        );
    }

    #[test]
    fn combined_selector() {
        let css = ".panel .title { font-size: 16px; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].selector, ".panel .title");
        assert_eq!(
            sheet.rules[0].declarations[0].1,
            CssValue::Length(16.0, LengthUnit::Px)
        );
    }

    #[test]
    fn comments_are_stripped() {
        let css = "/* header */ .dock { /* inline */ padding: 4px; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector, ".dock");
    }

    #[test]
    fn multiple_declarations() {
        let css = ".panel {
            background-color: rgba(30, 30, 30, 0.9);
            border-radius: 8px;
            padding: 1.5em;
        }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].declarations.len(), 3);
    }

    #[test]
    fn length_units() {
        let css = ".x {
            a: 10vw;
            b: 5vh;
            c: 2rem;
            d: 1.5em;
            e: 12px;
        }";
        let sheet = parse(css).unwrap();
        let decls = &sheet.rules[0].declarations;
        assert_eq!(decls[0].1, CssValue::Length(10.0, LengthUnit::Vw));
        assert_eq!(decls[1].1, CssValue::Length(5.0, LengthUnit::Vh));
        assert_eq!(decls[2].1, CssValue::Length(2.0, LengthUnit::Rem));
        assert_eq!(decls[3].1, CssValue::Length(1.5, LengthUnit::Em));
        assert_eq!(decls[4].1, CssValue::Length(12.0, LengthUnit::Px));
    }

    #[test]
    fn percentage_value() {
        let css = ".x { width: 50%; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].declarations[0].1, CssValue::Percent(50.0));
    }

    #[test]
    fn keyword_values() {
        let css = ".x { color: inherit; margin: initial; display: none; width: auto; }";
        let sheet = parse(css).unwrap();
        let d = &sheet.rules[0].declarations;
        assert_eq!(d[0].1, CssValue::Inherit);
        assert_eq!(d[1].1, CssValue::Initial);
        assert_eq!(d[2].1, CssValue::String("none".to_string()));
        assert_eq!(d[3].1, CssValue::String("auto".to_string()));
    }

    #[test]
    fn named_colors() {
        let css = ".x { color: red; background-color: transparent; }";
        let sheet = parse(css).unwrap();
        assert_eq!(
            sheet.rules[0].declarations[0].1,
            CssValue::Color(Color { r: 255, g: 0, b: 0, a: 1.0 })
        );
        assert_eq!(
            sheet.rules[0].declarations[1].1,
            CssValue::Color(Color::TRANSPARENT)
        );
    }

    #[test]
    fn rgb_rgba_colors() {
        let css = ".x { a: rgb(100, 200, 50); b: rgba(10, 20, 30, 0.5); }";
        let sheet = parse(css).unwrap();
        assert_eq!(
            sheet.rules[0].declarations[0].1,
            CssValue::Color(Color { r: 100, g: 200, b: 50, a: 1.0 })
        );
        assert_eq!(
            sheet.rules[0].declarations[1].1,
            CssValue::Color(Color { r: 10, g: 20, b: 30, a: 0.5 })
        );
    }

    #[test]
    fn quoted_string_value() {
        let css = r#".x { font-family: "Segoe UI"; }"#;
        let sheet = parse(css).unwrap();
        assert_eq!(
            sheet.rules[0].declarations[0].1,
            CssValue::String("Segoe UI".to_string())
        );
    }

    #[test]
    fn malformed_input_skipped() {
        let css = "garbage .button { color: red; } more garbage";
        let sheet = parse(css).unwrap();
        // The rule should still be parsed; surrounding garbage is ignored.
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector, "garbage .button");
    }

    #[test]
    fn missing_closing_brace() {
        let css = ".x { color: red; ";
        let sheet = parse(css).unwrap();
        // Unterminated block is skipped gracefully.
        assert!(sheet.rules.is_empty());
    }

    #[test]
    fn multiple_rules() {
        let css = "
            .dock { background-color: #1e1e1e; }
            .panel { padding: 12px; }
            .button:active { opacity: 0.6; }
        ";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules.len(), 3);
    }

    #[test]
    fn element_selector() {
        let css = "body { color: black; }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].selector, "body");
    }

    #[test]
    fn hex_8_digit() {
        let css = ".x { color: #ff000080; }";
        let sheet = parse(css).unwrap();
        if let CssValue::Color(c) = &sheet.rules[0].declarations[0].1 {
            assert_eq!(c.r, 255);
            assert_eq!(c.g, 0);
            assert_eq!(c.b, 0);
            assert!((c.a - 128.0 / 255.0).abs() < 0.01);
        } else {
            panic!("expected Color");
        }
    }

    #[test]
    fn declaration_without_trailing_semicolon() {
        let css = ".x { color: red }";
        let sheet = parse(css).unwrap();
        assert_eq!(sheet.rules[0].declarations.len(), 1);
    }

    #[test]
    fn only_comments() {
        let css = "/* nothing here */";
        let sheet = parse(css).unwrap();
        assert!(sheet.rules.is_empty());
    }
}
