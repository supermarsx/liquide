//! HTML template parser — converts HTML strings into [`TemplateNode`] trees.
//!
//! This is a special-purpose parser for application templates, **not** a full
//! spec-compliant browser HTML parser.  It handles:
//!
//! - Standard open/close tags and self-closing tags
//! - Attributes (class, id, style, data-*, etc.)
//! - Text nodes
//! - Nested elements
//! - `class` attribute auto-split into multiple classes
//! - `style` attribute split into `inline_styles`
//! - Void elements (`br`, `hr`, `img`, `input`, etc.)
//! - Comments (skipped)
//!
//! # Example
//!
//! ```rust,ignore
//! use liquide_components::html_parser::HtmlParser;
//!
//! let node = HtmlParser::parse(r#"
//!     <div class="container" id="main" style="width: 100px; background: red">
//!         <span class="label">Hello World</span>
//!         <button data-action="click">Click Me</button>
//!     </div>
//! "#).unwrap();
//!
//! assert_eq!(node.tag, "div");
//! assert_eq!(node.classes, vec!["container"]);
//! assert_eq!(node.children.len(), 2);
//! ```

use crate::template::TemplateNode;
use std::fmt;

// ── HTML entity decoding ─────────────────────────────────────────

/// Decode common HTML entities into their character equivalents.
fn decode_html_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            let mut entity = String::new();
            let mut terminated = false;
            for c in chars.by_ref() {
                if c == ';' {
                    terminated = true;
                    break;
                }
                entity.push(c);
                if entity.len() > 10 {
                    // Not a real entity — emit raw
                    result.push('&');
                    result.push_str(&entity);
                    entity.clear();
                    break;
                }
            }
            if entity.is_empty() {
                if !terminated {
                    // bare '&' at end of input
                    result.push('&');
                }
                continue;
            }
            match entity.as_str() {
                "amp" => result.push('&'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "quot" => result.push('"'),
                "apos" => result.push('\''),
                "nbsp" => result.push('\u{00A0}'),
                s if s.starts_with('#') => {
                    let code = if s.starts_with("#x") || s.starts_with("#X") {
                        u32::from_str_radix(&s[2..], 16).ok()
                    } else {
                        s[1..].parse::<u32>().ok()
                    };
                    if let Some(c) = code.and_then(char::from_u32) {
                        result.push(c);
                    } else {
                        result.push('&');
                        result.push_str(&entity);
                        if terminated {
                            result.push(';');
                        }
                    }
                }
                _ => {
                    // Unknown entity — preserve raw (only include ';' if
                    // the original input actually contained a terminating ';')
                    result.push('&');
                    result.push_str(&entity);
                    if terminated {
                        result.push(';');
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ── Error type ───────────────────────────────────────────────────

/// An error encountered while parsing an HTML template string.
#[derive(Debug, Clone)]
pub struct HtmlParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for HtmlParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTML parse error at position {}: {}", self.position, self.message)
    }
}

impl std::error::Error for HtmlParseError {}

// ── Void elements ────────────────────────────────────────────────

/// Elements that cannot have children and do not require a closing tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
];

fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag.to_ascii_lowercase().as_str())
}

// ── Parser ───────────────────────────────────────────────────────

/// A simple recursive-descent HTML parser that produces [`TemplateNode`] trees.
pub struct HtmlParser;

impl HtmlParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse an HTML string into a single [`TemplateNode`] tree.
    ///
    /// If the input contains exactly one root element, that element is returned.
    /// If there are multiple root elements, they are wrapped in a `<div>`.
    pub fn parse(html: &str) -> Result<TemplateNode, HtmlParseError> {
        let nodes = Self::parse_fragment(html)?;
        match nodes.len() {
            0 => Ok(TemplateNode::el("div")),
            1 => Ok(nodes.into_iter().next().unwrap()),
            _ => Ok(TemplateNode::el("div").children(nodes)),
        }
    }

    /// Parse an HTML fragment into a list of [`TemplateNode`]s.
    pub fn parse_fragment(html: &str) -> Result<Vec<TemplateNode>, HtmlParseError> {
        let mut cursor = Cursor::new(html);
        let nodes = cursor.parse_nodes(None)?;
        Ok(nodes)
    }
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

// ── Cursor (internal) ────────────────────────────────────────────

/// Low-level cursor over the input string that drives the recursive descent.
struct Cursor<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    // ── Helpers ──────────────────────────────────────────────

    /// Remaining unconsumed input.
    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    /// Peek at the next character without consuming.
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    /// Advance past `n` bytes.
    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    /// Skip ASCII whitespace.
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance(ch.len_utf8());
            } else {
                break;
            }
        }
    }

    /// Check whether the remaining input starts with `prefix`.
    fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    /// Consume one expected character, or return an error.
    fn expect_char(&mut self, expected: char) -> Result<(), HtmlParseError> {
        match self.peek() {
            Some(ch) if ch == expected => {
                self.advance(ch.len_utf8());
                Ok(())
            }
            _ => Err(self.error(format!("expected '{expected}'"))),
        }
    }

    /// Build an error at the current position.
    fn error(&self, message: impl Into<String>) -> HtmlParseError {
        HtmlParseError {
            message: message.into(),
            position: self.pos,
        }
    }

    // ── Top-level node list parser ───────────────────────────

    /// Parse a sequence of sibling nodes until EOF or a closing tag for
    /// `parent_tag` is encountered.
    fn parse_nodes(
        &mut self,
        parent_tag: Option<&str>,
    ) -> Result<Vec<TemplateNode>, HtmlParseError> {
        let mut nodes = Vec::new();

        loop {
            self.skip_whitespace();
            if self.rest().is_empty() {
                break;
            }

            // Stop when we see the closing tag for our parent.
            if let Some(tag) = parent_tag {
                if self.starts_with("</") {
                    // Peek ahead to see if this closing tag matches our parent.
                    let after_slash = &self.input[self.pos + 2..];
                    let close_name = Self::peek_tag_name(after_slash);
                    if close_name.eq_ignore_ascii_case(tag) {
                        break;
                    }
                }
            }

            // Comment: <!-- ... -->
            if self.starts_with("<!--") {
                self.skip_comment()?;
                continue;
            }

            // Opening tag
            if self.starts_with("<") && !self.starts_with("</") {
                let node = self.parse_element()?;
                nodes.push(node);
                continue;
            }

            // Closing tag without a parent — unexpected, skip it gracefully.
            if self.starts_with("</") {
                // If there's no parent, skip the stray closing tag.
                if parent_tag.is_none() {
                    self.skip_past(">")?;
                    continue;
                }
                break;
            }

            // Text node
            let text = self.parse_text();
            if !text.is_empty() {
                nodes.push(TemplateNode::text(&text));
            }
        }

        Ok(nodes)
    }

    // ── Comment ──────────────────────────────────────────────

    fn skip_comment(&mut self) -> Result<(), HtmlParseError> {
        debug_assert!(self.starts_with("<!--"));
        self.advance(4); // skip "<!--"
        match self.rest().find("-->") {
            Some(end) => {
                self.advance(end + 3);
                Ok(())
            }
            None => Err(self.error("unterminated comment")),
        }
    }

    // ── Element ──────────────────────────────────────────────

    fn parse_element(&mut self) -> Result<TemplateNode, HtmlParseError> {
        self.expect_char('<')?;

        // Tag name
        let tag = self.parse_tag_name()?;

        // Attributes
        let attrs = self.parse_attributes()?;

        // Self-closing `/>` or just `>`
        self.skip_whitespace();
        let self_closing = if self.starts_with("/>") {
            self.advance(2);
            true
        } else {
            self.expect_char('>')?;
            false
        };

        // Build the TemplateNode, processing special attributes.
        let mut node = TemplateNode::el(&tag);

        for (key, value) in &attrs {
            match key.as_str() {
                "id" => {
                    node.element_id = Some(value.clone());
                }
                "class" => {
                    // Auto-split space-separated class names.
                    for cls in value.split_whitespace() {
                        node.classes.push(cls.to_string());
                    }
                }
                "style" => {
                    // Parse semicolon-delimited CSS declarations.
                    for decl in value.split(';') {
                        let decl = decl.trim();
                        if decl.is_empty() {
                            continue;
                        }
                        if let Some((prop, val)) = decl.split_once(':') {
                            node.inline_styles
                                .push((prop.trim().to_string(), val.trim().to_string()));
                        }
                    }
                }
                "data-key" => {
                    node.key = Some(value.clone());
                }
                _ => {
                    node.attrs.push((key.clone(), value.clone()));
                }
            }
        }

        // Void / self-closing elements don't have children.
        if self_closing || is_void_element(&tag) {
            return Ok(node);
        }

        // Parse children until the matching closing tag.
        let children = self.parse_nodes(Some(&tag))?;
        node.children = children;

        // Consume the closing tag `</tag>`.
        self.skip_whitespace();
        if self.starts_with("</") {
            self.advance(2);
            let close_tag = self.parse_tag_name()?;
            if !close_tag.eq_ignore_ascii_case(&tag) {
                return Err(self.error(format!(
                    "mismatched closing tag: expected </{tag}>, found </{close_tag}>"
                )));
            }
            self.skip_whitespace();
            self.expect_char('>')?;
        }

        Ok(node)
    }

    // ── Tag name ─────────────────────────────────────────────

    fn parse_tag_name(&mut self) -> Result<String, HtmlParseError> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                self.advance(1);
            } else {
                break;
            }
        }
        let name = &self.input[start..self.pos];
        if name.is_empty() {
            return Err(self.error("expected tag name"));
        }
        Ok(name.to_string())
    }

    /// Peek a tag name from a slice without advancing the cursor.
    fn peek_tag_name(s: &str) -> &str {
        let end = s
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(s.len());
        &s[..end]
    }

    // ── Attributes ───────────────────────────────────────────

    fn parse_attributes(&mut self) -> Result<Vec<(String, String)>, HtmlParseError> {
        let mut attrs = Vec::new();

        loop {
            self.skip_whitespace();
            // Stop at end of tag.
            if self.peek() == Some('>') || self.starts_with("/>") {
                break;
            }
            if self.rest().is_empty() {
                return Err(self.error("unexpected end of input inside tag"));
            }

            let key = self.parse_attr_name()?;
            self.skip_whitespace();

            let value = if self.peek() == Some('=') {
                self.advance(1); // skip '='
                self.skip_whitespace();
                self.parse_attr_value()?
            } else {
                // Boolean attribute (e.g. `disabled`).
                String::new()
            };

            attrs.push((key, value));
        }

        Ok(attrs)
    }

    fn parse_attr_name(&mut self) -> Result<String, HtmlParseError> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':' || ch == '.' {
                self.advance(1);
            } else {
                break;
            }
        }
        let name = &self.input[start..self.pos];
        if name.is_empty() {
            return Err(self.error("expected attribute name"));
        }
        Ok(name.to_string())
    }

    fn parse_attr_value(&mut self) -> Result<String, HtmlParseError> {
        match self.peek() {
            // Quoted value
            Some(q @ '"') | Some(q @ '\'') => {
                self.advance(1); // opening quote
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch == q {
                        break;
                    }
                    self.advance(ch.len_utf8());
                }
                let value = self.input[start..self.pos].to_string();
                // Gracefully handle unterminated quotes at EOF instead of panicking
                if self.peek() == Some(q) {
                    self.advance(1); // closing quote
                }
                Ok(decode_html_entities(&value))
            }
            // Unquoted value (until whitespace or `>`)
            Some(_) => {
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_whitespace() || ch == '>' || ch == '/' {
                        break;
                    }
                    self.advance(ch.len_utf8());
                }
                Ok(decode_html_entities(&self.input[start..self.pos]))
            }
            None => Err(self.error("unexpected end of input in attribute value")),
        }
    }

    // ── Text ─────────────────────────────────────────────────

    /// Parse raw text content up to the next `<`.
    fn parse_text(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == '<' {
                break;
            }
            self.advance(ch.len_utf8());
        }
        let raw = &self.input[start..self.pos];
        // Collapse whitespace the way HTML does: trim, and compress inner runs
        // into single spaces.
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let mut result = String::with_capacity(trimmed.len());
        let mut prev_ws = false;
        for ch in trimmed.chars() {
            if ch.is_ascii_whitespace() {
                if !prev_ws {
                    result.push(' ');
                    prev_ws = true;
                }
            } else {
                result.push(ch);
                prev_ws = false;
            }
        }
        decode_html_entities(&result)
    }

    // ── Utility ──────────────────────────────────────────────

    /// Skip forward past the next occurrence of `needle`.
    fn skip_past(&mut self, needle: &str) -> Result<(), HtmlParseError> {
        match self.rest().find(needle) {
            Some(idx) => {
                self.advance(idx + needle.len());
                Ok(())
            }
            None => Err(self.error(format!("expected '{needle}'"))),
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_element() {
        let node = HtmlParser::parse("<div></div>").unwrap();
        assert_eq!(node.tag, "div");
        assert!(node.children.is_empty());
    }

    #[test]
    fn parse_with_text() {
        let node = HtmlParser::parse("<span>Hello</span>").unwrap();
        assert_eq!(node.tag, "span");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].text.as_deref(), Some("Hello"));
    }

    #[test]
    fn parse_nested() {
        let node = HtmlParser::parse("<div><span>Hi</span></div>").unwrap();
        assert_eq!(node.tag, "div");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].tag, "span");
        assert_eq!(node.children[0].children[0].text.as_deref(), Some("Hi"));
    }

    #[test]
    fn parse_attributes_class_split() {
        let node = HtmlParser::parse(r#"<div class="foo bar" id="main"></div>"#).unwrap();
        assert_eq!(node.tag, "div");
        assert_eq!(node.element_id.as_deref(), Some("main"));
        assert_eq!(node.classes, vec!["foo", "bar"]);
    }

    #[test]
    fn parse_inline_styles() {
        let node =
            HtmlParser::parse(r#"<div style="width: 100px; color: red"></div>"#).unwrap();
        assert_eq!(
            node.inline_styles,
            vec![
                ("width".to_string(), "100px".to_string()),
                ("color".to_string(), "red".to_string()),
            ]
        );
    }

    #[test]
    fn parse_self_closing_slash() {
        let node = HtmlParser::parse(r#"<br/>"#).unwrap();
        assert_eq!(node.tag, "br");
        assert!(node.children.is_empty());

        let node = HtmlParser::parse(r#"<img src="test.png"/>"#).unwrap();
        assert_eq!(node.tag, "img");
        assert_eq!(node.attrs, vec![("src".to_string(), "test.png".to_string())]);
    }

    #[test]
    fn parse_void_elements_without_slash() {
        let node = HtmlParser::parse(r#"<br>"#).unwrap();
        assert_eq!(node.tag, "br");
        assert!(node.children.is_empty());

        let node = HtmlParser::parse(r#"<input type="text">"#).unwrap();
        assert_eq!(node.tag, "input");
        assert_eq!(node.attrs, vec![("type".to_string(), "text".to_string())]);
    }

    #[test]
    fn parse_fragment_multiple_roots() {
        let nodes =
            HtmlParser::parse_fragment(r#"<span>A</span><span>B</span>"#).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].tag, "span");
        assert_eq!(nodes[1].tag, "span");
    }

    #[test]
    fn parse_wraps_multiple_roots() {
        let node = HtmlParser::parse(r#"<span>A</span><span>B</span>"#).unwrap();
        // Multiple roots get wrapped in a <div>.
        assert_eq!(node.tag, "div");
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn parse_skips_comments() {
        let node =
            HtmlParser::parse(r#"<!-- comment --><div>Text</div>"#).unwrap();
        assert_eq!(node.tag, "div");
        assert_eq!(node.children[0].text.as_deref(), Some("Text"));
    }

    #[test]
    fn parse_data_key_attribute() {
        let node = HtmlParser::parse(r#"<li data-key="item-1"></li>"#).unwrap();
        assert_eq!(node.key.as_deref(), Some("item-1"));
    }

    #[test]
    fn parse_data_attributes() {
        let node =
            HtmlParser::parse(r#"<button data-action="click" data-id="42"></button>"#)
                .unwrap();
        assert!(node.attrs.contains(&("data-action".to_string(), "click".to_string())));
        assert!(node.attrs.contains(&("data-id".to_string(), "42".to_string())));
    }

    #[test]
    fn parse_deep_nesting() {
        let html = r#"
            <div class="container" id="main" style="width: 100px; background: red">
                <span class="label">Hello World</span>
                <button data-action="click">Click Me</button>
            </div>
        "#;
        let node = HtmlParser::parse(html).unwrap();
        assert_eq!(node.tag, "div");
        assert_eq!(node.element_id.as_deref(), Some("main"));
        assert_eq!(node.classes, vec!["container"]);
        assert_eq!(node.inline_styles.len(), 2);
        assert_eq!(node.children.len(), 2);

        let span = &node.children[0];
        assert_eq!(span.tag, "span");
        assert_eq!(span.classes, vec!["label"]);
        assert_eq!(span.children[0].text.as_deref(), Some("Hello World"));

        let button = &node.children[1];
        assert_eq!(button.tag, "button");
        assert_eq!(button.children[0].text.as_deref(), Some("Click Me"));
    }

    #[test]
    fn parse_boolean_attribute() {
        let node = HtmlParser::parse(r#"<input disabled>"#).unwrap();
        assert!(node.attrs.contains(&("disabled".to_string(), String::new())));
    }

    #[test]
    fn parse_single_quoted_attrs() {
        let node = HtmlParser::parse("<div class='hello'></div>").unwrap();
        assert_eq!(node.classes, vec!["hello"]);
    }

    #[test]
    fn parse_empty_input() {
        let node = HtmlParser::parse("").unwrap();
        assert_eq!(node.tag, "div");
        assert!(node.children.is_empty());
    }

    #[test]
    fn parse_mixed_content() {
        let html = r#"<p>Hello <strong>world</strong>!</p>"#;
        let node = HtmlParser::parse(html).unwrap();
        assert_eq!(node.tag, "p");
        assert_eq!(node.children.len(), 3);
        assert_eq!(node.children[0].text.as_deref(), Some("Hello"));
        assert_eq!(node.children[1].tag, "strong");
        assert_eq!(node.children[2].text.as_deref(), Some("!"));
    }

    #[test]
    fn error_unterminated_comment() {
        let res = HtmlParser::parse("<!-- unterminated");
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("unterminated comment"));
    }

    #[test]
    fn error_mismatched_tags() {
        let res = HtmlParser::parse("<div></span>");
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("mismatched closing tag"));
    }
}
