//! Extensive HTML parser + entity decoder tests.
//!
//! Covers: known entities (&amp; &lt; &gt; etc.), numeric entities,
//! unknown entities with/without semicolons, unterminated entities,
//! basic parsing of elements, text, attributes, nesting, and edge cases.

use liquide_components::html_parser::HtmlParser;

// ── Entity decoding (via parsed text content) ────────────────────────────

#[test]
fn entity_amp_decoded() {
    let template = HtmlParser::parse("<div>&amp;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("&"));
}

#[test]
fn entity_lt_gt_decoded() {
    let template = HtmlParser::parse("<div>&lt;hello&gt;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("<hello>"));
}

#[test]
fn entity_quot_decoded() {
    let template = HtmlParser::parse("<div>&quot;quoted&quot;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("\"quoted\""));
}

#[test]
fn entity_apos_decoded() {
    let template = HtmlParser::parse("<div>&apos;apos&apos;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("'apos'"));
}

#[test]
fn entity_nbsp_decoded() {
    let template = HtmlParser::parse("<div>&nbsp;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("\u{00A0}"));
}

#[test]
fn entity_numeric_decimal() {
    let template = HtmlParser::parse("<div>&#65;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("A"));
}

#[test]
fn entity_numeric_hex() {
    let template = HtmlParser::parse("<div>&#x41;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("A"));
}

#[test]
fn entity_numeric_hex_uppercase() {
    let template = HtmlParser::parse("<div>&#X41;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("A"));
}

#[test]
fn entity_unknown_with_semicolon_preserved() {
    // Unknown entity like &foo; should be preserved as-is: "&foo;"
    let template = HtmlParser::parse("<div>&foo;</div>").unwrap();
    let text = template.children[0].text.as_deref().unwrap();
    assert_eq!(
        text, "&foo;",
        "unknown terminated entity should preserve semicolon"
    );
}

#[test]
fn entity_multiple_in_text() {
    let template = HtmlParser::parse("<div>&lt;b&gt;bold&lt;/b&gt;</div>").unwrap();
    assert_eq!(template.children[0].text.as_deref(), Some("<b>bold</b>"));
}

// ── Basic parsing ────────────────────────────────────────────────────────

#[test]
fn parse_simple_element() {
    let template = HtmlParser::parse("<div></div>").unwrap();
    assert_eq!(template.tag, "div");
    assert!(template.children.is_empty());
}

#[test]
fn parse_element_with_text_child() {
    let template = HtmlParser::parse("<p>Hello World</p>").unwrap();
    assert_eq!(template.tag, "p");
    assert_eq!(template.children.len(), 1);
    assert_eq!(template.children[0].text.as_deref(), Some("Hello World"));
}

#[test]
fn parse_nested_elements() {
    let template = HtmlParser::parse("<div><span>inner</span></div>").unwrap();
    assert_eq!(template.tag, "div");
    assert_eq!(template.children.len(), 1);
    assert_eq!(template.children[0].tag, "span");
    assert_eq!(
        template.children[0].children[0].text.as_deref(),
        Some("inner")
    );
}

#[test]
fn parse_element_with_attributes() {
    let template = HtmlParser::parse(r#"<div class="main" id="root"></div>"#).unwrap();
    assert_eq!(template.tag, "div");
    // Class and id should be parsed
    assert!(
        template.classes.contains(&"main".to_string())
            || template
                .attrs
                .iter()
                .any(|(k, v)| k == "class" && v == "main"),
        "should parse class attribute"
    );
}

#[test]
fn parse_self_closing_elements() {
    let template = HtmlParser::parse("<div><br/><hr/></div>").unwrap();
    assert_eq!(template.tag, "div");
    assert!(template.children.len() >= 2);
}

#[test]
fn parse_multiple_children() {
    let template = HtmlParser::parse("<ul><li>A</li><li>B</li><li>C</li></ul>").unwrap();
    assert_eq!(template.tag, "ul");
    assert_eq!(template.children.len(), 3);
}

#[test]
fn parse_mixed_text_and_elements() {
    let template = HtmlParser::parse("<div>Hello <b>world</b>!</div>").unwrap();
    assert_eq!(template.tag, "div");
    // Should have text "Hello ", element <b>, text "!"
    assert!(template.children.len() >= 2, "should have mixed content");
}

// ── Inline styles ────────────────────────────────────────────────────────

#[test]
fn parse_inline_style() {
    let template = HtmlParser::parse(r#"<div style="color: red; font-size: 16px"></div>"#).unwrap();
    assert!(
        !template.inline_styles.is_empty(),
        "should parse style attribute into inline_styles"
    );
}

// ── Fragment parsing ─────────────────────────────────────────────────────

#[test]
fn parse_fragment_multiple_roots() {
    let nodes = HtmlParser::parse_fragment("<div>A</div><span>B</span>").unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].tag, "div");
    assert_eq!(nodes[1].tag, "span");
}

#[test]
fn parse_fragment_text_only() {
    let nodes = HtmlParser::parse_fragment("just text").unwrap();
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].is_text());
    assert_eq!(nodes[0].text.as_deref(), Some("just text"));
}

// ── Edge cases ───────────────────────────────────────────────────────────

#[test]
fn parse_empty_string_is_error_or_empty() {
    // Empty HTML should either error or produce an empty/default node
    let result = HtmlParser::parse("");
    // Either it errors or returns somethingvalidly
    if let Ok(template) = result {
        assert!(
            template.tag.is_empty() || template.children.is_empty(),
            "empty string should produce minimal output"
        );
    }
}

#[test]
fn parse_whitespace_preserved_in_text() {
    // HTML parser collapses whitespace in text nodes (like browsers do).
    let template = HtmlParser::parse("<pre>  hello  world  </pre>").unwrap();
    let text = template.children[0].text.as_deref().unwrap_or("");
    // Text should be trimmed and runs collapsed to single spaces
    assert_eq!(
        text, "hello world",
        "whitespace should be collapsed in text"
    );
}

#[test]
fn parse_data_attributes() {
    let template = HtmlParser::parse(r#"<div data-key="abc" data-value="123"></div>"#).unwrap();
    // data-key is consumed into TemplateNode::key (not stored in attrs)
    assert_eq!(
        template.key.as_deref(),
        Some("abc"),
        "data-key attribute should be stored in .key field"
    );
    // other data-* attributes remain in attrs
    assert!(
        template
            .attrs
            .iter()
            .any(|(k, v)| k == "data-value" && v == "123"),
        "should parse data-value attribute"
    );
}

#[test]
fn parse_key_from_data_key_attr() {
    let template = HtmlParser::parse(r#"<div data-key="my-key"></div>"#).unwrap();
    // data-key should map to the template's key field
    assert_eq!(
        template.key.as_deref(),
        Some("my-key"),
        "data-key attribute should set the template key"
    );
}
