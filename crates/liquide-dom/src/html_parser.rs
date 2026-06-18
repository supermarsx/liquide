//! Lightweight HTML template parser for building DOM trees from markup.
//!
//! Parses a subset of HTML sufficient for defining LiquiDE UI layouts:
//! elements, attributes, text nodes, self-closing tags, and comments.
//!
//! # Example
//!
//! ```rust
//! use liquide_dom::html_parser::parse_html;
//!
//! let doc = parse_html(r#"
//!     <statusbar id="shell-statusbar">
//!         <statusbar-slot class="left" id="slot-left" />
//!     </statusbar>
//! "#);
//!
//! assert!(doc.get_element_by_id("shell-statusbar").is_some());
//! assert!(doc.get_element_by_id("slot-left").is_some());
//! ```

use crate::document::Document;
use crate::node::NodeId;

/// Parse HTML and append the resulting nodes as children of `parent`.
pub fn parse_html_into(doc: &mut Document, parent: NodeId, html: &str) {
    let mut parser = Parser::new(html);
    parser.parse_children(doc, parent);
}

/// Create a new [`Document`] and parse the given HTML into its root.
pub fn parse_html(html: &str) -> Document {
    let mut doc = Document::new();
    let root = doc.root();
    parse_html_into(&mut doc, root, html);
    doc
}

// ---------------------------------------------------------------------------
// Internal parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    // -- Primitives ---------------------------------------------------------

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
        }
    }

    fn starts_with(&self, s: &[u8]) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    // -- Tag / attribute name -----------------------------------------------

    fn is_tag_char(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.'
    }

    fn read_name(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if Self::is_tag_char(c) {
                self.advance();
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.input[start..self.pos]).into_owned()
    }

    // -- Attribute value ----------------------------------------------------

    fn read_quoted_value(&mut self) -> String {
        let quote = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return String::new(),
        };
        self.advance(); // skip opening quote

        let mut value = Vec::new();
        while !self.eof() {
            let c = self.input[self.pos];
            if c == quote {
                self.advance(); // skip closing quote
                break;
            }
            // Handle basic entities
            if c == b'&' {
                if let Some(decoded) = self.try_decode_entity() {
                    value.extend_from_slice(decoded.as_bytes());
                    continue;
                }
            }
            value.push(c);
            self.advance();
        }
        String::from_utf8_lossy(&value).into_owned()
    }

    fn try_decode_entity(&mut self) -> Option<&'static str> {
        let remaining = &self.input[self.pos..];
        let entities: &[(&[u8], &str)] = &[
            (b"&amp;", "&"),
            (b"&lt;", "<"),
            (b"&gt;", ">"),
            (b"&quot;", "\""),
            (b"&apos;", "'"),
        ];
        for &(pattern, replacement) in entities {
            if remaining.starts_with(pattern) {
                self.pos += pattern.len();
                return Some(replacement);
            }
        }
        None
    }

    // -- Comment ------------------------------------------------------------

    fn try_parse_comment(&mut self) -> bool {
        if !self.starts_with(b"<!--") {
            return false;
        }
        self.pos += 4; // skip "<!--"
        while !self.eof() {
            if self.starts_with(b"-->") {
                self.pos += 3;
                return true;
            }
            self.advance();
        }
        true // unterminated comment — consume rest
    }

    // -- Element parsing ----------------------------------------------------

    /// Parse children until we hit EOF or a closing tag for `parent_tag`.
    fn parse_children(&mut self, doc: &mut Document, parent: NodeId) {
        self.parse_children_until(doc, parent, None);
    }

    fn parse_children_until(
        &mut self,
        doc: &mut Document,
        parent: NodeId,
        parent_tag: Option<&str>,
    ) {
        loop {
            if self.eof() {
                break;
            }

            // Check for closing tag
            if let Some(tag) = parent_tag {
                let saved = self.pos;
                if self.starts_with(b"</") {
                    // Peek ahead to see if this closes *our* parent
                    self.pos += 2;
                    self.skip_whitespace();
                    let close_name = self.read_name();
                    self.skip_whitespace();
                    if close_name.eq_ignore_ascii_case(tag) {
                        // Consume the '>'
                        if self.peek() == Some(b'>') {
                            self.advance();
                        }
                        return;
                    }
                    // Not our closing tag — restore position and handle below
                    self.pos = saved;
                }
            }

            // Check for any closing tag (mismatched) — stop so parent can handle
            if self.starts_with(b"</") {
                if parent_tag.is_none() {
                    // At top level, skip stray closing tags
                    while !self.eof() && self.peek() != Some(b'>') {
                        self.advance();
                    }
                    if self.peek() == Some(b'>') {
                        self.advance();
                    }
                    continue;
                }
                break;
            }

            // Comment
            if self.starts_with(b"<!--") {
                self.try_parse_comment();
                continue;
            }

            // Opening tag
            if self.peek() == Some(b'<') {
                self.parse_element(doc, parent);
                continue;
            }

            // Text content
            self.parse_text(doc, parent);
        }
    }

    fn parse_text(&mut self, doc: &mut Document, parent: NodeId) {
        let mut text = Vec::new();
        while !self.eof() {
            if self.peek() == Some(b'<') {
                break;
            }
            let c = self.input[self.pos];
            // Handle entities in text
            if c == b'&' {
                if let Some(decoded) = self.try_decode_entity() {
                    text.extend_from_slice(decoded.as_bytes());
                    continue;
                }
            }
            text.push(c);
            self.advance();
        }

        let s = String::from_utf8_lossy(&text);
        // Skip whitespace-only text nodes
        if s.trim().is_empty() {
            return;
        }
        // Collapse and trim leading/trailing whitespace but preserve the trimmed content
        let trimmed = s.trim();
        let node = doc.create_text(trimmed);
        doc.append_child(parent, node);
    }

    fn parse_element(&mut self, doc: &mut Document, parent: NodeId) {
        // Skip '<'
        self.advance();

        // Read tag name
        self.skip_whitespace();
        let tag_name = self.read_name();
        if tag_name.is_empty() {
            // Malformed — skip
            return;
        }

        let el = doc.create_element(&tag_name);

        // Parse attributes
        loop {
            self.skip_whitespace();
            if self.eof() {
                break;
            }
            let c = self.peek().unwrap();

            // Self-closing />
            if c == b'/' {
                self.advance();
                if self.peek() == Some(b'>') {
                    self.advance();
                }
                // Apply element to parent
                Self::apply_special_attrs(doc, el);
                if tag_name.eq_ignore_ascii_case("img") {
                    doc.convert_element_to_image(el);
                }
                doc.append_child(parent, el);
                return;
            }

            // End of opening tag
            if c == b'>' {
                self.advance();
                break;
            }

            // Read attribute
            let attr_name = self.read_name();
            if attr_name.is_empty() {
                // Skip unexpected character
                self.advance();
                continue;
            }

            self.skip_whitespace();
            if self.peek() == Some(b'=') {
                self.advance(); // skip '='
                self.skip_whitespace();
                let value = self.read_quoted_value();
                doc.set_attribute(el, &attr_name, &value);
            } else {
                // Boolean attribute (no value)
                doc.set_attribute(el, &attr_name, "");
            }
        }

        // Apply id/class from attributes
        Self::apply_special_attrs(doc, el);

        // `<img>` is a void replaced element: promote it to a NodeData::Image
        // content node (so the painter emits an Image display item) and do NOT
        // parse children — an `<img>` never has any. (The template parser already
        // treats img as void; mirror that here for parsed documents.)
        if tag_name.eq_ignore_ascii_case("img") {
            doc.convert_element_to_image(el);
            doc.append_child(parent, el);
            return;
        }

        // Append to parent before parsing children (so children can reference parent)
        doc.append_child(parent, el);

        // Parse children
        self.parse_children_until(doc, el, Some(&tag_name));
    }

    /// Process `id` and `class` attributes: call `set_id` / `add_class` and
    /// remove them from the generic attribute map so they only live in their
    /// dedicated fields.
    fn apply_special_attrs(doc: &mut Document, node: NodeId) {
        // Handle id
        if let Some(id_val) = doc.get_attribute(node, "id") {
            if !id_val.is_empty() {
                doc.set_id(node, &id_val);
            }
            doc.remove_attribute(node, "id");
        }

        // Handle class — split by whitespace
        if let Some(class_val) = doc.get_attribute(node, "class") {
            for cls in class_val.split_whitespace() {
                if !cls.is_empty() {
                    doc.add_class(node, cls);
                }
            }
            doc.remove_attribute(node, "class");
        }

        // Handle style — parse into inline styles
        if let Some(style_val) = doc.get_attribute(node, "style") {
            for declaration in style_val.split(';') {
                let declaration = declaration.trim();
                if declaration.is_empty() {
                    continue;
                }
                if let Some((prop, value)) = declaration.split_once(':') {
                    let prop = prop.trim();
                    let value = value.trim();
                    if !prop.is_empty() && !value.is_empty() {
                        doc.set_inline_style(node, prop, value);
                    }
                }
            }
            doc.remove_attribute(node, "style");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_element() {
        let doc = parse_html("<div></div>");
        let root = doc.root();
        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "div");
    }

    #[test]
    fn parse_with_attributes() {
        let doc = parse_html(r#"<div id="main" class="active"></div>"#);
        let root = doc.root();
        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        let div = kids[0];
        assert_eq!(doc.get_element_by_id("main"), Some(div));
        assert!(doc.get(div).unwrap().has_class("active"));
    }

    #[test]
    fn parse_nested_elements() {
        let doc = parse_html("<div><span>hello</span></div>");
        let root = doc.root();
        let div = doc.children(root)[0];
        let span = doc.children(div)[0];
        assert_eq!(doc.tag_name(span).unwrap(), "span");
        // span has a text child
        let text_node = doc.children(span)[0];
        assert_eq!(doc.get(text_node).unwrap().text_content(), Some("hello"));
    }

    #[test]
    fn parse_self_closing() {
        let doc = parse_html("<br />");
        let root = doc.root();
        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "br");
        assert_eq!(doc.children(kids[0]).len(), 0);
    }

    #[test]
    fn parse_self_closing_no_space() {
        let doc = parse_html("<br/>");
        let root = doc.root();
        assert_eq!(doc.children(root).len(), 1);
        assert_eq!(doc.tag_name(doc.children(root)[0]).unwrap(), "br");
    }

    #[test]
    fn img_element_becomes_image_node_keeping_src_attribute() {
        use crate::node::NodeData;
        // An `<img src=...>` must parse into a NodeData::Image content node (so
        // the painter emits an Image display item) AND keep its `src` attribute
        // (so the layout replaced-element path can resolve intrinsic size). It is
        // void: no children.
        // (html, parent_is_root) — the bare-img cases sit directly under root,
        // the wrapped case sits under the first <div>.
        for (html, wrapped) in [
            (r#"<img src="photo.png" alt="a photo">"#, false),
            (r#"<img src="photo.png" alt="a photo" />"#, false),
            (r#"<div><img src="photo.png"></div>"#, true),
        ] {
            let doc = parse_html(html);
            let root = doc.root();
            let img = if wrapped {
                let div = doc.children(root)[0];
                doc.children(div)[0]
            } else {
                doc.children(root)[0]
            };
            assert_eq!(doc.tag_name(img).as_deref(), Some("img"));
            match &doc.get(img).unwrap().data {
                NodeData::Image { src, alt, .. } => {
                    assert_eq!(src, "photo.png", "src mirrored into NodeData::Image");
                    if html.contains("alt") {
                        assert_eq!(alt, "a photo");
                    }
                }
                other => panic!("img must be NodeData::Image, got {other:?}"),
            }
            // src attribute preserved for the layout ImageMeasurer.
            assert_eq!(
                doc.get_attribute(img, "src"),
                Some("photo.png".to_string()),
                "src attribute must survive conversion (layout reads it)"
            );
            // Void: no children.
            assert_eq!(doc.children(img).len(), 0, "img is void");
        }
    }

    #[test]
    fn parse_text_content() {
        let doc = parse_html("<p>Hello world</p>");
        let root = doc.root();
        let p = doc.children(root)[0];
        let txt = doc.children(p)[0];
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("Hello world"));
    }

    #[test]
    fn parse_multiple_children() {
        let doc = parse_html("<div><a /><b /><c /></div>");
        let root = doc.root();
        let div = doc.children(root)[0];
        let kids = doc.children(div);
        assert_eq!(kids.len(), 3);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "a");
        assert_eq!(doc.tag_name(kids[1]).unwrap(), "b");
        assert_eq!(doc.tag_name(kids[2]).unwrap(), "c");
    }

    #[test]
    fn parse_classes_and_ids() {
        let doc = parse_html(r#"<div id="test" class="foo bar baz"></div>"#);
        let div = doc.get_element_by_id("test").unwrap();
        let node = doc.get(div).unwrap();
        assert!(node.has_class("foo"));
        assert!(node.has_class("bar"));
        assert!(node.has_class("baz"));
    }

    #[test]
    fn parse_comment() {
        let doc = parse_html("<!-- this is a comment --><div></div>");
        let root = doc.root();
        // Comments are skipped — only the div should be present
        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "div");
    }

    #[test]
    fn parse_comment_between_elements() {
        let doc = parse_html("<a /><!-- comment --><b />");
        let root = doc.root();
        let kids = doc.children(root);
        assert_eq!(kids.len(), 2);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "a");
        assert_eq!(doc.tag_name(kids[1]).unwrap(), "b");
    }

    #[test]
    fn parse_mixed_text_and_elements() {
        let doc = parse_html("<div>Hello <span>world</span> !</div>");
        let root = doc.root();
        let div = doc.children(root)[0];
        let kids = doc.children(div);
        // "Hello", <span>, "!"
        assert_eq!(kids.len(), 3);
        assert_eq!(doc.get(kids[0]).unwrap().text_content(), Some("Hello"));
        assert_eq!(doc.tag_name(kids[1]).unwrap(), "span");
        assert_eq!(doc.get(kids[2]).unwrap().text_content(), Some("!"));
    }

    #[test]
    fn parse_whitespace_only_text_skipped() {
        let doc = parse_html(
            "
            <div>
                <span></span>
            </div>
            ",
        );
        let root = doc.root();
        let div = doc.children(root)[0];
        // Only the span child, no whitespace text nodes
        let kids = doc.children(div);
        assert_eq!(kids.len(), 1);
        assert_eq!(doc.tag_name(kids[0]).unwrap(), "span");
    }

    #[test]
    fn parse_entities_in_text() {
        let doc = parse_html("<p>&amp; &lt; &gt; &quot;</p>");
        let root = doc.root();
        let p = doc.children(root)[0];
        let txt = doc.children(p)[0];
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("& < > \""));
    }

    #[test]
    fn parse_entities_in_attribute() {
        let doc = parse_html(r#"<div data-val="a &amp; b"></div>"#);
        let root = doc.root();
        let div = doc.children(root)[0];
        assert_eq!(
            doc.get_attribute(div, "data-val"),
            Some("a & b".to_string())
        );
    }

    #[test]
    fn parse_desktop_layout() {
        let html = r#"
            <desktop-background id="desktop-bg" />
            <statusbar id="shell-statusbar">
                <statusbar-slot class="left" id="statusbar-slot-left" />
                <statusbar-slot class="center" id="statusbar-slot-center" />
                <statusbar-slot class="right" id="statusbar-slot-right" />
            </statusbar>
            <workspace-container id="workspace-container" />
            <dock id="shell-dock" />
            <notification-area id="notification-area" />
        "#;

        let doc = parse_html(html);
        let root = doc.root();

        // 5 top-level elements
        assert_eq!(doc.children(root).len(), 5);

        // Check IDs
        assert!(doc.get_element_by_id("desktop-bg").is_some());
        assert!(doc.get_element_by_id("shell-statusbar").is_some());
        assert!(doc.get_element_by_id("statusbar-slot-left").is_some());
        assert!(doc.get_element_by_id("statusbar-slot-center").is_some());
        assert!(doc.get_element_by_id("statusbar-slot-right").is_some());
        assert!(doc.get_element_by_id("workspace-container").is_some());
        assert!(doc.get_element_by_id("shell-dock").is_some());
        assert!(doc.get_element_by_id("notification-area").is_some());

        // Check classes on statusbar slots
        let slot_left = doc.get_element_by_id("statusbar-slot-left").unwrap();
        assert!(doc.get(slot_left).unwrap().has_class("left"));

        let slot_center = doc.get_element_by_id("statusbar-slot-center").unwrap();
        assert!(doc.get(slot_center).unwrap().has_class("center"));

        let slot_right = doc.get_element_by_id("statusbar-slot-right").unwrap();
        assert!(doc.get(slot_right).unwrap().has_class("right"));

        // Statusbar has 3 children
        let statusbar = doc.get_element_by_id("shell-statusbar").unwrap();
        assert_eq!(doc.children(statusbar).len(), 3);

        // Tag names
        assert_eq!(
            doc.tag_name(doc.children(root)[0]).unwrap(),
            "desktop-background"
        );
        assert_eq!(doc.tag_name(doc.children(root)[1]).unwrap(), "statusbar");
    }

    #[test]
    fn parse_window_layout() {
        let html = r#"
            <window id="win-1" class="focused">
                <window-titlebar>
                    <window-title>My App</window-title>
                    <titlebar-buttons>
                        <minimize-button />
                        <maximize-button />
                        <close-button />
                    </titlebar-buttons>
                </window-titlebar>
                <window-content />
            </window>
        "#;

        let doc = parse_html(html);

        let win = doc.get_element_by_id("win-1").unwrap();
        assert!(doc.get(win).unwrap().has_class("focused"));

        // Window has 2 children: titlebar + content
        let win_kids = doc.children(win);
        assert_eq!(win_kids.len(), 2);
        assert_eq!(doc.tag_name(win_kids[0]).unwrap(), "window-titlebar");
        assert_eq!(doc.tag_name(win_kids[1]).unwrap(), "window-content");

        // Titlebar has title + buttons
        let titlebar = win_kids[0];
        let tb_kids = doc.children(titlebar);
        assert_eq!(tb_kids.len(), 2);
        assert_eq!(doc.tag_name(tb_kids[0]).unwrap(), "window-title");
        assert_eq!(doc.tag_name(tb_kids[1]).unwrap(), "titlebar-buttons");

        // Title text
        let title = tb_kids[0];
        let title_txt = doc.children(title)[0];
        assert_eq!(doc.get(title_txt).unwrap().text_content(), Some("My App"));

        // Buttons
        let btn_group = tb_kids[1];
        let buttons = doc.children(btn_group);
        assert_eq!(buttons.len(), 3);
        assert_eq!(doc.tag_name(buttons[0]).unwrap(), "minimize-button");
        assert_eq!(doc.tag_name(buttons[1]).unwrap(), "maximize-button");
        assert_eq!(doc.tag_name(buttons[2]).unwrap(), "close-button");
    }

    #[test]
    fn parse_html_into_existing_doc() {
        let mut doc = Document::new();
        let root = doc.root();

        let container = doc.create_element("container");
        doc.append_child(root, container);

        parse_html_into(&mut doc, container, r#"<child id="c1" /><child id="c2" />"#);

        assert_eq!(doc.children(container).len(), 2);
        assert!(doc.get_element_by_id("c1").is_some());
        assert!(doc.get_element_by_id("c2").is_some());
    }

    #[test]
    fn parse_multiple_classes() {
        let doc = parse_html(r#"<div class="a  b   c"></div>"#);
        let root = doc.root();
        let div = doc.children(root)[0];
        let node = doc.get(div).unwrap();
        assert!(node.has_class("a"));
        assert!(node.has_class("b"));
        assert!(node.has_class("c"));
    }

    #[test]
    fn parse_data_attributes() {
        let doc = parse_html(r#"<dock-item data-app-id="files" data-label="Files" />"#);
        let root = doc.root();
        let item = doc.children(root)[0];
        assert_eq!(
            doc.get_attribute(item, "data-app-id"),
            Some("files".to_string())
        );
        assert_eq!(
            doc.get_attribute(item, "data-label"),
            Some("Files".to_string())
        );
    }

    #[test]
    fn parse_single_quoted_attributes() {
        let doc = parse_html("<div id='test' class='foo bar'></div>");
        let node = doc.get_element_by_id("test").unwrap();
        assert!(doc.get(node).unwrap().has_class("foo"));
        assert!(doc.get(node).unwrap().has_class("bar"));
    }

    #[test]
    fn parse_boolean_attribute() {
        let doc = parse_html("<input disabled />");
        let root = doc.root();
        let input = doc.children(root)[0];
        assert_eq!(doc.get_attribute(input, "disabled"), Some(String::new()));
    }

    #[test]
    fn parse_empty_html() {
        let doc = parse_html("");
        assert_eq!(doc.children(doc.root()).len(), 0);
    }

    #[test]
    fn parse_whitespace_only_html() {
        let doc = parse_html("   \n\t  ");
        assert_eq!(doc.children(doc.root()).len(), 0);
    }

    #[test]
    fn parse_deeply_nested() {
        let doc = parse_html("<a><b><c><d>deep</d></c></b></a>");
        let root = doc.root();
        let a = doc.children(root)[0];
        let b = doc.children(a)[0];
        let c = doc.children(b)[0];
        let d = doc.children(c)[0];
        let txt = doc.children(d)[0];
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("deep"));
    }

    #[test]
    fn parse_multiple_top_level_elements() {
        let doc = parse_html("<a /><b /><c />");
        let root = doc.root();
        assert_eq!(doc.children(root).len(), 3);
    }

    #[test]
    fn parse_apos_entity() {
        let doc = parse_html("<p>&apos;</p>");
        let root = doc.root();
        let p = doc.children(root)[0];
        let txt = doc.children(p)[0];
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("'"));
    }

    #[test]
    fn parse_style_attribute() {
        let mut doc = Document::new();
        let root = doc.root();
        parse_html_into(
            &mut doc,
            root,
            r#"<div style="left: 100; top: 200">Hello</div>"#,
        );
        let div = doc.children(root)[0];
        assert_eq!(doc.get_inline_style(div, "left"), Some("100".to_string()));
        assert_eq!(doc.get_inline_style(div, "top"), Some("200".to_string()));
        // The style attribute should be removed from regular attrs
        assert!(doc.get_attribute(div, "style").is_none());
    }

    #[test]
    fn parse_style_multiple_properties() {
        let mut doc = Document::new();
        let root = doc.root();
        parse_html_into(
            &mut doc,
            root,
            r#"<span style="color: red; font-size: 14; display: flex">text</span>"#,
        );
        let span = doc.children(root)[0];
        assert_eq!(doc.get_inline_style(span, "color"), Some("red".to_string()));
        assert_eq!(
            doc.get_inline_style(span, "font-size"),
            Some("14".to_string())
        );
        assert_eq!(
            doc.get_inline_style(span, "display"),
            Some("flex".to_string())
        );
    }

    #[test]
    fn parse_style_with_trailing_semicolon() {
        let mut doc = Document::new();
        let root = doc.root();
        parse_html_into(&mut doc, root, r#"<div style="width: 50;">content</div>"#);
        let div = doc.children(root)[0];
        assert_eq!(doc.get_inline_style(div, "width"), Some("50".to_string()));
    }

    #[test]
    fn parse_empty_style_attribute() {
        let mut doc = Document::new();
        let root = doc.root();
        parse_html_into(&mut doc, root, r#"<div style="">content</div>"#);
        let div = doc.children(root)[0];
        // No inline styles should be set
        assert!(doc.get_inline_style(div, "").is_none());
    }

    #[test]
    fn parse_style_does_not_appear_as_attribute() {
        let mut doc = Document::new();
        let root = doc.root();
        parse_html_into(
            &mut doc,
            root,
            r#"<div style="left: 50" data-foo="bar">x</div>"#,
        );
        let div = doc.children(root)[0];
        // style should NOT be in regular attributes
        assert!(doc.get_attribute(div, "style").is_none());
        // data-foo should still be a regular attribute
        assert_eq!(doc.get_attribute(div, "data-foo"), Some("bar".to_string()));
        // inline style should be set
        assert_eq!(doc.get_inline_style(div, "left"), Some("50".to_string()));
    }
}
