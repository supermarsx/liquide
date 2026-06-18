//! Template engine for dynamic HTML generation.
//!
//! Provides Mustache/Handlebars-like template syntax with variable substitution,
//! conditionals (`{{#if}}`/`{{#unless}}`/`{{else}}`), and loops (`{{#each}}`).
//!
//! The engine works in two phases:
//! 1. **Expand** — process the template string with a [`TemplateContext`], producing
//!    plain HTML.
//! 2. **Parse** — feed the expanded HTML into a built-in mini HTML parser that
//!    creates DOM nodes via [`Document`].
//!
//! # Template syntax
//!
//! - `{{var}}` — insert HTML-escaped value of `var`
//! - `{{{var}}}` — insert raw (unescaped) value of `var`
//! - `{{#if var}}...{{/if}}` — include block when `var` is truthy
//! - `{{#if var}}...{{else}}...{{/if}}` — if/else
//! - `{{#unless var}}...{{/unless}}` — include block when `var` is falsy
//! - `{{#each list}}...{{/each}}` — repeat block for each item in `list`
//!
//! Inside `{{#each}}`, the iteration context becomes the current scope, so
//! `{{field}}` refers to a field on the list item.
//!
//! # Example
//!
//! ```rust,ignore
//! use liquide_dom::template::{Template, TemplateContext};
//!
//! let tpl = Template::compile(r#"
//!   <dock>
//!     {{#each items}}
//!     <dock-item data-app-id="{{app_id}}"
//!                {{#if is_running}}class="active"{{/if}}>
//!       {{label}}
//!     </dock-item>
//!     {{/each}}
//!   </dock>
//! "#);
//!
//! let mut ctx = TemplateContext::new();
//! let mut item = TemplateContext::new();
//! item.set("app_id", "firefox").set("label", "Firefox").set_bool("is_running", true);
//! ctx.set_list("items", vec![item]);
//!
//! let html = tpl.render(&ctx);
//! ```

use std::collections::HashMap;

use crate::document::Document;
use crate::node::NodeId;

// ═══════════════════════════════════════════════════════════════════
// Template context
// ═══════════════════════════════════════════════════════════════════

/// Context data for template rendering — key-value pairs and lists.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    strings: HashMap<String, String>,
    bools: HashMap<String, bool>,
    lists: HashMap<String, Vec<TemplateContext>>,
}

impl TemplateContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a string variable. Returns `&mut Self` for chaining.
    pub fn set(&mut self, key: &str, value: &str) -> &mut Self {
        self.strings.insert(key.to_string(), value.to_string());
        self
    }

    /// Set a boolean variable.
    pub fn set_bool(&mut self, key: &str, value: bool) -> &mut Self {
        self.bools.insert(key.to_string(), value);
        self
    }

    /// Set a list variable (each item is a nested [`TemplateContext`]).
    pub fn set_list(&mut self, key: &str, items: Vec<TemplateContext>) -> &mut Self {
        self.lists.insert(key.to_string(), items);
        self
    }

    /// Look up a string value.
    fn get_string(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }

    /// Check if a key is "truthy":
    /// - Bool key: its value
    /// - String key: exists and non-empty
    /// - List key: exists and non-empty
    fn is_truthy(&self, key: &str) -> bool {
        if let Some(&b) = self.bools.get(key) {
            return b;
        }
        if let Some(s) = self.strings.get(key) {
            return !s.is_empty();
        }
        if let Some(list) = self.lists.get(key) {
            return !list.is_empty();
        }
        false
    }

    /// Get a list by key.
    fn get_list(&self, key: &str) -> Option<&[TemplateContext]> {
        self.lists.get(key).map(Vec::as_slice)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Template
// ═══════════════════════════════════════════════════════════════════

/// A compiled template ready for instantiation.
#[derive(Debug, Clone)]
pub struct Template {
    source: String,
}

impl Template {
    /// Compile a template from source. Currently this just stores the source;
    /// future versions may pre-parse the template for faster rendering.
    pub fn compile(source: &str) -> Self {
        Self {
            source: source.to_string(),
        }
    }

    /// Render the template with the given context, returning expanded HTML.
    pub fn render(&self, ctx: &TemplateContext) -> String {
        expand(&self.source, ctx)
    }

    /// Render the template directly into a DOM tree under `parent`.
    pub fn render_into(&self, doc: &mut Document, parent: NodeId, ctx: &TemplateContext) {
        let html = self.render(ctx);
        parse_html_into(doc, parent, &html);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Template expansion
// ═══════════════════════════════════════════════════════════════════

/// Expand all template directives in `source` using `ctx`.
fn expand(source: &str, ctx: &TemplateContext) -> String {
    let mut out = String::with_capacity(source.len());
    let mut pos = 0;
    let bytes = source.as_bytes();

    while pos < bytes.len() {
        // Look for the next `{{`
        if let Some(start) = find_substr(source, pos, "{{") {
            // Emit everything before the tag
            out.push_str(&source[pos..start]);

            // Triple-brace raw variable: {{{var}}}
            if source[start..].starts_with("{{{") {
                if let Some(end) = find_substr(source, start + 3, "}}}") {
                    let key = source[start + 3..end].trim();
                    if let Some(val) = ctx.get_string(key) {
                        out.push_str(val);
                    }
                    pos = end + 3;
                    continue;
                }
            }

            // Find closing `}}`
            if let Some(end) = find_substr(source, start + 2, "}}") {
                let tag = source[start + 2..end].trim();
                pos = end + 2;

                if let Some(key) = tag.strip_prefix("#if ") {
                    let key = key.trim();
                    let block_end = find_block_end(source, pos, "if");
                    let block = &source[pos..block_end];
                    // Look for {{else}} in this block (at the same nesting level)
                    let (then_part, else_part) = split_else(block, "if");
                    if ctx.is_truthy(key) {
                        out.push_str(&expand(then_part, ctx));
                    } else if let Some(ep) = else_part {
                        out.push_str(&expand(ep, ctx));
                    }
                    // Skip past {{/if}}
                    pos = skip_closing_tag(source, block_end, "if");
                } else if let Some(key) = tag.strip_prefix("#unless ") {
                    let key = key.trim();
                    let block_end = find_block_end(source, pos, "unless");
                    let block = &source[pos..block_end];
                    let (then_part, else_part) = split_else(block, "unless");
                    if !ctx.is_truthy(key) {
                        out.push_str(&expand(then_part, ctx));
                    } else if let Some(ep) = else_part {
                        out.push_str(&expand(ep, ctx));
                    }
                    pos = skip_closing_tag(source, block_end, "unless");
                } else if let Some(key) = tag.strip_prefix("#each ") {
                    let key = key.trim();
                    let block_end = find_block_end(source, pos, "each");
                    let block = &source[pos..block_end];
                    if let Some(items) = ctx.get_list(key) {
                        for (i, item) in items.iter().enumerate() {
                            // Provide @index and @first/@last as bools
                            let mut item_ctx = item.clone();
                            item_ctx.set("@index", &i.to_string());
                            item_ctx.set_bool("@first", i == 0);
                            item_ctx.set_bool("@last", i == items.len() - 1);
                            out.push_str(&expand(block, &item_ctx));
                        }
                    }
                    pos = skip_closing_tag(source, block_end, "each");
                } else if tag.starts_with('/') {
                    // Stray closing tag — should not happen with correct templates.
                    // Skip it silently.
                } else {
                    // Simple variable substitution
                    if let Some(val) = ctx.get_string(tag) {
                        out.push_str(&escape_html(val));
                    }
                    // Missing variable → empty string (nothing pushed)
                }
            } else {
                // No closing `}}` — emit the rest as-is
                out.push_str(&source[start..]);
                pos = bytes.len();
            }
        } else {
            // No more `{{` — emit the rest
            out.push_str(&source[pos..]);
            break;
        }
    }

    out
}

/// Find the position of the closing `{{/tag}}` at the same nesting level.
/// Returns the position of the `{{` of `{{/tag}}`.
fn find_block_end(source: &str, start: usize, tag: &str) -> usize {
    let open_prefix = format!("{{{{#{tag} ");
    let open_prefix2 = format!("{{{{#{tag}}}}}"); // e.g. {{#if}} without space (unlikely but safe)
    let close = format!("{{{{/{tag}}}}}");
    let mut depth = 1usize;
    let mut pos = start;

    while pos < source.len() {
        if let Some(next) = find_substr(source, pos, "{{") {
            if source[next..].starts_with(&close) {
                depth -= 1;
                if depth == 0 {
                    return next;
                }
                pos = next + close.len();
            } else if source[next..].starts_with(&open_prefix)
                || source[next..].starts_with(&open_prefix2)
            {
                depth += 1;
                pos = next + 2;
            } else {
                pos = next + 2;
            }
        } else {
            break;
        }
    }
    // Not found — return end of source
    source.len()
}

/// Skip past a closing tag like `{{/if}}`. Returns the position after the tag.
fn skip_closing_tag(source: &str, block_end: usize, tag: &str) -> usize {
    let close = format!("{{{{/{tag}}}}}");
    if source[block_end..].starts_with(&close) {
        block_end + close.len()
    } else {
        block_end
    }
}

/// Split a block on `{{else}}` at the top nesting level.
/// Returns `(then_part, Some(else_part))` or `(block, None)`.
fn split_else<'a>(block: &'a str, block_tag: &str) -> (&'a str, Option<&'a str>) {
    let else_tag = "{{else}}";
    let mut depth = 0usize;
    let mut pos = 0;

    while pos < block.len() {
        if let Some(next) = find_substr(block, pos, "{{") {
            let rest = &block[next..];
            // Track nesting of same block type
            if rest.starts_with(&format!("{{{{#{block_tag} "))
                || rest.starts_with(&format!("{{{{#{block_tag}}}}}"))
            {
                depth += 1;
                pos = next + 2;
            } else if rest.starts_with(&format!("{{{{/{block_tag}}}}}")) {
                if depth == 0 {
                    // End of our block — no else found before it
                    break;
                }
                depth -= 1;
                pos = next + 2;
            } else if depth == 0 && rest.starts_with(else_tag) {
                let then_part = &block[..next];
                let else_part = &block[next + else_tag.len()..];
                return (then_part, Some(else_part));
            } else {
                pos = next + 2;
            }
        } else {
            break;
        }
    }
    (block, None)
}

/// Find a substring starting from `start`. Returns byte offset from beginning.
fn find_substr(haystack: &str, start: usize, needle: &str) -> Option<usize> {
    haystack[start..].find(needle).map(|i| start + i)
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    if !s.contains('&') && !s.contains('<') && !s.contains('>') && !s.contains('"') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════
// Mini HTML parser → Document
// ═══════════════════════════════════════════════════════════════════

/// Void elements that cannot have children.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.iter().any(|&v| v.eq_ignore_ascii_case(tag))
}

/// Parse an HTML string and append the resulting nodes as children of `parent`
/// in the given [`Document`].
pub fn parse_html_into(doc: &mut Document, parent: NodeId, html: &str) {
    let mut parser = HtmlToDom::new(html);
    parser.parse_children(doc, parent);
}

/// Parse an HTML string into a new [`Document`], returning it.
/// The parsed nodes become children of the document root.
pub fn parse_html(html: &str) -> Document {
    let mut doc = Document::new();
    let root = doc.root();
    parse_html_into(&mut doc, root, html);
    doc
}

struct HtmlToDom<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> HtmlToDom<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.input.len());
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance(1);
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    /// Parse children into `parent` until EOF or a closing tag for `parent_tag`.
    fn parse_children(&mut self, doc: &mut Document, parent: NodeId) {
        self.parse_children_for(doc, parent, None);
    }

    fn parse_children_for(&mut self, doc: &mut Document, parent: NodeId, parent_tag: Option<&str>) {
        loop {
            // Don't skip whitespace here — we need to capture text nodes
            if self.rest().is_empty() {
                break;
            }

            // Check for closing tag
            if let Some(tag) = parent_tag {
                if self.starts_with("</") {
                    let after = &self.input[self.pos + 2..];
                    let close_name = Self::peek_tag_name(after);
                    if close_name.eq_ignore_ascii_case(tag) {
                        break;
                    }
                }
            }

            // Comment
            if self.starts_with("<!--") {
                self.skip_comment();
                continue;
            }

            // Opening tag
            if self.starts_with("<") && !self.starts_with("</") {
                self.parse_element(doc, parent);
                continue;
            }

            // Stray closing tag
            if self.starts_with("</") {
                if parent_tag.is_none() {
                    self.skip_past(">");
                    continue;
                }
                break;
            }

            // Text
            let text = self.parse_text();
            if !text.is_empty() {
                let text_id = doc.create_text(&text);
                doc.append_child(parent, text_id);
            }
        }
    }

    fn skip_comment(&mut self) {
        self.advance(4); // skip <!--
        if let Some(end) = self.rest().find("-->") {
            self.advance(end + 3);
        } else {
            self.pos = self.input.len();
        }
    }

    fn parse_element(&mut self, doc: &mut Document, parent: NodeId) {
        self.advance(1); // skip '<'

        let tag = self.read_tag_name();
        if tag.is_empty() {
            return;
        }

        let el = doc.create_element(&tag);

        // Parse attributes
        self.parse_attributes(doc, el);

        self.skip_whitespace();

        let self_closing = if self.starts_with("/>") {
            self.advance(2);
            true
        } else if self.peek() == Some('>') {
            self.advance(1);
            false
        } else {
            // Malformed — try to recover
            self.skip_past(">");
            false
        };

        doc.append_child(parent, el);

        // Promote `<img>` to a NodeData::Image content node so the painter emits
        // an Image display item (template-authored chrome/app images). The `src`
        // attribute is left in place for the layout replaced-element path.
        if tag.eq_ignore_ascii_case("img") {
            doc.convert_element_to_image(el);
        }

        if self_closing || is_void_element(&tag) {
            return;
        }

        // Parse children
        self.parse_children_for(doc, el, Some(&tag));

        // Consume closing tag
        if self.starts_with("</") {
            self.advance(2);
            let _close = self.read_tag_name();
            self.skip_whitespace();
            if self.peek() == Some('>') {
                self.advance(1);
            }
        }
    }

    fn parse_attributes(&mut self, doc: &mut Document, el: NodeId) {
        loop {
            self.skip_whitespace();
            if self.peek() == Some('>') || self.starts_with("/>") || self.rest().is_empty() {
                break;
            }

            let key = self.read_attr_name();
            if key.is_empty() {
                // Skip one char to avoid infinite loop on malformed input
                self.advance(1);
                continue;
            }

            self.skip_whitespace();

            let value = if self.peek() == Some('=') {
                self.advance(1);
                self.skip_whitespace();
                self.read_attr_value()
            } else {
                String::new()
            };

            // Apply to DOM node
            match key.as_str() {
                "id" => {
                    doc.set_id(el, &value);
                }
                "class" => {
                    for cls in value.split_whitespace() {
                        doc.add_class(el, cls);
                    }
                }
                "style" => {
                    for decl in value.split(';') {
                        let decl = decl.trim();
                        if let Some((prop, val)) = decl.split_once(':') {
                            doc.set_inline_style(el, prop.trim(), val.trim());
                        }
                    }
                }
                _ => {
                    doc.set_attribute(el, &key, &value);
                }
            }
        }
    }

    fn read_tag_name(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                self.advance(1);
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn peek_tag_name(s: &str) -> &str {
        let end = s
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(s.len());
        &s[..end]
    }

    fn read_attr_name(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':' || ch == '.' {
                self.advance(1);
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn read_attr_value(&mut self) -> String {
        match self.peek() {
            Some(q @ '"') | Some(q @ '\'') => {
                self.advance(1);
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch == q {
                        break;
                    }
                    self.advance(ch.len_utf8());
                }
                let val = self.input[start..self.pos].to_string();
                if self.peek() == Some(q) {
                    self.advance(1);
                }
                decode_entities(&val)
            }
            Some(_) => {
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_whitespace() || ch == '>' || ch == '/' {
                        break;
                    }
                    self.advance(ch.len_utf8());
                }
                decode_entities(&self.input[start..self.pos])
            }
            None => String::new(),
        }
    }

    fn parse_text(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == '<' {
                break;
            }
            self.advance(ch.len_utf8());
        }
        let raw = &self.input[start..self.pos];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        // Collapse whitespace
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
        decode_entities(&result)
    }

    fn skip_past(&mut self, needle: &str) {
        if let Some(idx) = self.rest().find(needle) {
            self.advance(idx + needle.len());
        } else {
            self.pos = self.input.len();
        }
    }
}

/// Decode common HTML entities.
fn decode_entities(input: &str) -> String {
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
                    result.push('&');
                    result.push_str(&entity);
                    entity.clear();
                    break;
                }
            }
            if entity.is_empty() {
                if !terminated {
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

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Variable substitution ───────────────────────────────────

    #[test]
    fn variable_in_text() {
        let tpl = Template::compile("<window-title>{{title}}</window-title>");
        let mut ctx = TemplateContext::new();
        ctx.set("title", "My App");
        let html = tpl.render(&ctx);
        assert!(html.contains("My App"));
        assert!(html.contains("<window-title>"));
    }

    #[test]
    fn variable_in_attribute() {
        let tpl = Template::compile(r#"<dock-item data-app-id="{{app_id}}">{{label}}</dock-item>"#);
        let mut ctx = TemplateContext::new();
        ctx.set("app_id", "firefox");
        ctx.set("label", "Firefox");
        let html = tpl.render(&ctx);
        assert!(html.contains(r#"data-app-id="firefox""#));
        assert!(html.contains("Firefox"));
    }

    #[test]
    fn missing_variable_produces_empty() {
        let tpl = Template::compile("<span>{{missing}}</span>");
        let ctx = TemplateContext::new();
        let html = tpl.render(&ctx);
        assert_eq!(html, "<span></span>");
    }

    #[test]
    fn html_escaping() {
        let tpl = Template::compile("<span>{{text}}</span>");
        let mut ctx = TemplateContext::new();
        ctx.set("text", "<script>alert('xss')</script>");
        let html = tpl.render(&ctx);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn raw_triple_brace() {
        let tpl = Template::compile("<div>{{{content}}}</div>");
        let mut ctx = TemplateContext::new();
        ctx.set("content", "<b>bold</b>");
        let html = tpl.render(&ctx);
        assert!(html.contains("<b>bold</b>"));
    }

    // ── Conditionals ────────────────────────────────────────────

    #[test]
    fn if_true() {
        let tpl = Template::compile("{{#if show}}<div>visible</div>{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("show", true);
        let html = tpl.render(&ctx);
        assert!(html.contains("<div>visible</div>"));
    }

    #[test]
    fn if_false() {
        let tpl = Template::compile("{{#if show}}<div>visible</div>{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("show", false);
        let html = tpl.render(&ctx);
        assert!(!html.contains("visible"));
    }

    #[test]
    fn if_else() {
        let tpl = Template::compile("{{#if active}}ON{{else}}OFF{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("active", true);
        assert_eq!(tpl.render(&ctx), "ON");

        ctx.set_bool("active", false);
        assert_eq!(tpl.render(&ctx), "OFF");
    }

    #[test]
    fn unless_block() {
        let tpl = Template::compile("{{#unless hidden}}<span>shown</span>{{/unless}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("hidden", false);
        assert!(tpl.render(&ctx).contains("shown"));

        ctx.set_bool("hidden", true);
        assert!(!tpl.render(&ctx).contains("shown"));
    }

    #[test]
    fn if_with_string_truthy() {
        let tpl = Template::compile("{{#if name}}Hello {{name}}{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set("name", "Alice");
        assert_eq!(tpl.render(&ctx), "Hello Alice");

        let empty_ctx = TemplateContext::new();
        assert_eq!(tpl.render(&empty_ctx), "");
    }

    #[test]
    fn if_with_empty_string_falsy() {
        let tpl = Template::compile("{{#if name}}present{{else}}absent{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set("name", "");
        assert_eq!(tpl.render(&ctx), "absent");
    }

    // ── Loops ───────────────────────────────────────────────────

    #[test]
    fn each_loop() {
        let tpl = Template::compile("{{#each items}}<li>{{name}}</li>{{/each}}");
        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("name", "Alice");
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("name", "Bob");
                c
            },
        ];
        ctx.set_list("items", items);
        let html = tpl.render(&ctx);
        assert!(html.contains("<li>Alice</li>"));
        assert!(html.contains("<li>Bob</li>"));
    }

    #[test]
    fn empty_list_produces_nothing() {
        let tpl = Template::compile("{{#each items}}<li>{{name}}</li>{{/each}}");
        let mut ctx = TemplateContext::new();
        ctx.set_list("items", vec![]);
        assert_eq!(tpl.render(&ctx), "");
    }

    #[test]
    fn missing_list_produces_nothing() {
        let tpl = Template::compile("{{#each items}}<li>x</li>{{/each}}");
        let ctx = TemplateContext::new();
        assert_eq!(tpl.render(&ctx), "");
    }

    #[test]
    fn each_with_index() {
        let tpl = Template::compile("{{#each items}}{{@index}}:{{name}} {{/each}}");
        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("name", "A");
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("name", "B");
                c
            },
        ];
        ctx.set_list("items", items);
        let html = tpl.render(&ctx);
        assert!(html.contains("0:A"));
        assert!(html.contains("1:B"));
    }

    // ── Nested conditionals inside loops ────────────────────────

    #[test]
    fn conditionals_inside_loop() {
        let tpl = Template::compile(
            r#"{{#each items}}<dock-item data-app-id="{{app_id}}" {{#if is_running}}class="active"{{/if}}>{{label}}</dock-item>{{/each}}"#,
        );
        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "firefox");
                c.set("label", "Firefox");
                c.set_bool("is_running", true);
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "terminal");
                c.set("label", "Terminal");
                c.set_bool("is_running", false);
                c
            },
        ];
        ctx.set_list("items", items);
        let html = tpl.render(&ctx);
        assert!(html.contains(r#"data-app-id="firefox""#));
        assert!(html.contains(r#"class="active""#));
        assert!(html.contains(r#"data-app-id="terminal""#));
        // Terminal's dock-item should NOT have class="active"
        // Find the second <dock-item (terminal) and check it has no "active"
        let first_end = html.find("</dock-item>").unwrap();
        let terminal_section = &html[first_end..];
        assert!(!terminal_section.contains(r#"class="active""#));
    }

    // ── Class merging with conditionals ─────────────────────────

    #[test]
    fn class_merging() {
        let tpl = Template::compile(
            r#"<dock-item class="{{#if is_running}}active{{/if}} {{#if is_pinned}}pinned{{/if}}"></dock-item>"#,
        );
        let mut ctx = TemplateContext::new();
        ctx.set_bool("is_running", true);
        ctx.set_bool("is_pinned", true);
        let html = tpl.render(&ctx);
        assert!(html.contains("active"));
        assert!(html.contains("pinned"));
    }

    #[test]
    fn class_merging_partial() {
        let tpl = Template::compile(
            r#"<dock-item class="{{#if is_running}}active{{/if}} {{#if is_pinned}}pinned{{/if}}"></dock-item>"#,
        );
        let mut ctx = TemplateContext::new();
        ctx.set_bool("is_running", false);
        ctx.set_bool("is_pinned", true);
        let html = tpl.render(&ctx);
        assert!(!html.contains("active"));
        assert!(html.contains("pinned"));
    }

    // ── Full desktop layout ─────────────────────────────────────

    #[test]
    fn full_desktop_template() {
        let tpl = Template::compile(
            r#"<desktop-background>
  <statusbar>
    {{#each status_items}}<statusbar-item data-slot="{{slot}}">{{content}}</statusbar-item>{{/each}}
  </statusbar>
  <dock>
    {{#each dock_items}}
    <dock-item data-app-id="{{app_id}}" data-label="{{label}}" data-icon="{{icon}}"
               {{#if is_running}}class="active"{{/if}}>
      {{label}}
    </dock-item>
    {{/each}}
  </dock>
</desktop-background>"#,
        );

        let mut ctx = TemplateContext::new();

        // Status bar items
        let status_items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("slot", "left");
                c.set("content", "LiquiDE");
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("slot", "center");
                c.set("content", "14:30");
                c
            },
        ];
        ctx.set_list("status_items", status_items);

        // Dock items
        let dock_items = vec![{
            let mut c = TemplateContext::new();
            c.set("app_id", "files");
            c.set("label", "Files");
            c.set("icon", "folder");
            c.set_bool("is_running", true);
            c
        }];
        ctx.set_list("dock_items", dock_items);

        let html = tpl.render(&ctx);
        assert!(html.contains("<desktop-background>"));
        assert!(html.contains("<statusbar>"));
        assert!(html.contains(r#"data-slot="left""#));
        assert!(html.contains("14:30"));
        assert!(html.contains(r#"data-app-id="files""#));
        assert!(html.contains(r#"class="active""#));
    }

    // ── Window template ─────────────────────────────────────────

    #[test]
    fn window_template() {
        let tpl = Template::compile(
            r#"<window id="{{window_id}}" {{#if focused}}class="focused"{{/if}}>
  <window-titlebar>{{title}}</window-titlebar>
  <window-content></window-content>
</window>"#,
        );
        let mut ctx = TemplateContext::new();
        ctx.set("window_id", "win-42");
        ctx.set("title", "Text Editor");
        ctx.set_bool("focused", true);

        let html = tpl.render(&ctx);
        assert!(html.contains(r#"id="win-42""#));
        assert!(html.contains(r#"class="focused""#));
        assert!(html.contains("Text Editor"));
    }

    // ── Dock items loop template ────────────────────────────────

    #[test]
    fn dock_items_template() {
        let tpl = Template::compile(
            r#"<dock>{{#each items}}<dock-item data-app-id="{{app_id}}" data-label="{{label}}" data-icon="{{icon}}" {{#if is_running}}class="active"{{/if}} {{#if is_pinned}}data-pinned="true"{{/if}}>{{label}}</dock-item>{{/each}}</dock>"#,
        );

        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "firefox");
                c.set("label", "Firefox");
                c.set("icon", "firefox");
                c.set_bool("is_running", true);
                c.set_bool("is_pinned", true);
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "terminal");
                c.set("label", "Terminal");
                c.set("icon", "terminal");
                c.set_bool("is_running", false);
                c.set_bool("is_pinned", false);
                c
            },
        ];
        ctx.set_list("items", items);

        let html = tpl.render(&ctx);
        assert!(html.contains(r#"data-app-id="firefox""#));
        assert!(html.contains(r#"class="active""#));
        assert!(html.contains(r#"data-pinned="true""#));
        assert!(html.contains(r#"data-app-id="terminal""#));
        // Terminal should not have active or pinned
        let term_section = &html[html.find("terminal").unwrap()..];
        let next_dock = term_section.find("</dock-item>").unwrap();
        let term_chunk = &term_section[..next_dock];
        assert!(!term_chunk.contains(r#"class="active""#));
    }

    // ── Menu items template ─────────────────────────────────────

    #[test]
    fn menu_items_template() {
        let tpl = Template::compile(
            r#"<context-menu>{{#each items}}<menu-item data-action="{{action}}" {{#if disabled}}class="disabled"{{/if}}>{{label}}</menu-item>{{/each}}</context-menu>"#,
        );

        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("action", "copy");
                c.set("label", "Copy");
                c.set_bool("disabled", false);
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("action", "paste");
                c.set("label", "Paste");
                c.set_bool("disabled", true);
                c
            },
        ];
        ctx.set_list("items", items);

        let html = tpl.render(&ctx);
        assert!(html.contains("Copy"));
        assert!(html.contains("Paste"));
        // Only paste should be disabled
        let paste_pos = html.find("paste").unwrap();
        let paste_section = &html[paste_pos..];
        assert!(paste_section.contains(r#"class="disabled""#));
    }

    // ── Notification template ───────────────────────────────────

    #[test]
    fn notification_template() {
        let tpl = Template::compile(
            r#"<notification data-urgency="{{urgency}}">
  <notification-title>{{summary}}</notification-title>
  <notification-body>{{body}}</notification-body>
</notification>"#,
        );

        let mut ctx = TemplateContext::new();
        ctx.set("urgency", "critical");
        ctx.set("summary", "Low Battery");
        ctx.set("body", "Battery at 5%");

        let html = tpl.render(&ctx);
        assert!(html.contains(r#"data-urgency="critical""#));
        assert!(html.contains("Low Battery"));
        assert!(html.contains("Battery at 5%"));
    }

    // ── Statusbar items template ────────────────────────────────

    #[test]
    fn statusbar_items_template() {
        let tpl = Template::compile(
            r#"<statusbar>{{#each items}}<statusbar-item data-slot="{{slot}}" {{#if visible}}class="visible"{{/if}}>{{content}}</statusbar-item>{{/each}}</statusbar>"#,
        );

        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("slot", "left");
                c.set("content", "Logo");
                c.set_bool("visible", true);
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("slot", "center");
                c.set("content", "12:00");
                c.set_bool("visible", true);
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("slot", "right");
                c.set("content", "100%");
                c.set_bool("visible", false);
                c
            },
        ];
        ctx.set_list("items", items);

        let html = tpl.render(&ctx);
        assert!(html.contains(r#"data-slot="left""#));
        assert!(html.contains(r#"data-slot="center""#));
        assert!(html.contains(r#"data-slot="right""#));
        assert!(html.contains("12:00"));
    }

    // ── Render into DOM ─────────────────────────────────────────

    #[test]
    fn render_into_dom() {
        let tpl =
            Template::compile(r#"<div id="container"><span class="label">Hello</span></div>"#);
        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();
        tpl.render_into(&mut doc, root, &ctx);

        // Root should have one child: the div
        let children = doc.children(root);
        assert_eq!(children.len(), 1);

        let div = children[0];
        assert_eq!(doc.get(div).unwrap().tag_name(), "div");
        assert_eq!(doc.get_element_by_id("container"), Some(div));

        // Div should have one child: span
        let span = doc.children(div)[0];
        assert_eq!(doc.get(span).unwrap().tag_name(), "span");
        assert!(doc.get(span).unwrap().has_class("label"));

        // Span should have text child
        let text = doc.children(span)[0];
        assert_eq!(doc.get(text).unwrap().text_content(), Some("Hello"));
    }

    #[test]
    fn render_into_dom_with_variables() {
        let tpl = Template::compile(
            r#"<window id="{{id}}"><window-titlebar>{{title}}</window-titlebar></window>"#,
        );
        let mut doc = Document::new();
        let root = doc.root();
        let mut ctx = TemplateContext::new();
        ctx.set("id", "win-1");
        ctx.set("title", "Editor");
        tpl.render_into(&mut doc, root, &ctx);

        let win = doc.get_element_by_id("win-1").unwrap();
        assert_eq!(doc.get(win).unwrap().tag_name(), "window");

        let titlebar = doc.children(win)[0];
        assert_eq!(doc.get(titlebar).unwrap().tag_name(), "window-titlebar");

        let text = doc.children(titlebar)[0];
        assert_eq!(doc.get(text).unwrap().text_content(), Some("Editor"));
    }

    #[test]
    fn render_loop_into_dom() {
        let tpl = Template::compile(
            r#"<dock>{{#each items}}<dock-item data-app-id="{{app_id}}">{{label}}</dock-item>{{/each}}</dock>"#,
        );
        let mut doc = Document::new();
        let root = doc.root();
        let mut ctx = TemplateContext::new();
        let items = vec![
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "files");
                c.set("label", "Files");
                c
            },
            {
                let mut c = TemplateContext::new();
                c.set("app_id", "browser");
                c.set("label", "Browser");
                c
            },
        ];
        ctx.set_list("items", items);
        tpl.render_into(&mut doc, root, &ctx);

        let dock = doc.children(root)[0];
        assert_eq!(doc.get(dock).unwrap().tag_name(), "dock");
        let dock_children = doc.children(dock);
        assert_eq!(dock_children.len(), 2);

        assert_eq!(
            doc.get_attribute(dock_children[0], "data-app-id"),
            Some("files".to_string())
        );
        assert_eq!(
            doc.get_attribute(dock_children[1], "data-app-id"),
            Some("browser".to_string())
        );
    }

    #[test]
    fn parse_html_standalone() {
        let doc = parse_html(r#"<div id="main"><span>Hello</span></div>"#);
        let root = doc.root();
        let div = doc.children(root)[0];
        assert_eq!(doc.get(div).unwrap().tag_name(), "div");
        assert_eq!(doc.get_element_by_id("main"), Some(div));
    }

    // ── Nested if blocks ────────────────────────────────────────

    #[test]
    fn nested_if_blocks() {
        let tpl = Template::compile("{{#if a}}A{{#if b}}B{{/if}}{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("a", true);
        ctx.set_bool("b", true);
        assert_eq!(tpl.render(&ctx), "AB");

        ctx.set_bool("b", false);
        assert_eq!(tpl.render(&ctx), "A");

        ctx.set_bool("a", false);
        assert_eq!(tpl.render(&ctx), "");
    }

    #[test]
    fn list_truthy_for_if() {
        let tpl = Template::compile("{{#if items}}has items{{else}}no items{{/if}}");
        let mut ctx = TemplateContext::new();
        ctx.set_list("items", vec![TemplateContext::new()]);
        assert_eq!(tpl.render(&ctx), "has items");

        ctx.set_list("items", vec![]);
        assert_eq!(tpl.render(&ctx), "no items");
    }

    #[test]
    fn unless_with_else() {
        let tpl = Template::compile("{{#unless active}}inactive{{else}}active{{/unless}}");
        let mut ctx = TemplateContext::new();
        ctx.set_bool("active", false);
        assert_eq!(tpl.render(&ctx), "inactive");

        ctx.set_bool("active", true);
        assert_eq!(tpl.render(&ctx), "active");
    }

    #[test]
    fn void_elements_in_dom() {
        let doc = parse_html(r#"<div><br><img src="test.png"><hr></div>"#);
        let root = doc.root();
        let div = doc.children(root)[0];
        let children = doc.children(div);
        assert_eq!(children.len(), 3);
        assert_eq!(doc.get(children[0]).unwrap().tag_name(), "br");
        assert_eq!(doc.get(children[1]).unwrap().tag_name(), "img");
        assert_eq!(doc.get(children[2]).unwrap().tag_name(), "hr");
    }
}
