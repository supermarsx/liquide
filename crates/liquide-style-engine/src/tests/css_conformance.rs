//! CSS Conformance Test Harness
//!
//! Loads CSS, constructs DOM programmatically, computes styles via StyleEngine,
//! and compares results against expected values. Covers style computation,
//! layout properties, visual properties, and advanced CSS features.

use super::*;
use liquide_dom::Document;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════
// Test Infrastructure
// ═══════════════════════════════════════════════════════════════════════════

/// Expected value for a style property assertion.
#[derive(Debug, Clone)]
enum ExpectedValue {
    /// Exact f32 comparison (with epsilon).
    Float(f32),
    /// Exact u16 comparison (for font-weight, etc.).
    U16(u16),
    /// Exact i32 comparison (for z-index, order, etc.).
    I32(i32),
    /// Color RGBA comparison.
    Color { r: u8, g: u8, b: u8, a: u8 },
    /// Display enum comparison.
    Display(Display),
    /// Position enum comparison.
    Position(Position),
    /// BoxSizing enum comparison.
    BoxSizing(BoxSizing),
    /// Visibility enum comparison.
    Visibility(Visibility),
    /// Float enum comparison.
    CssFloat(Float),
    /// Clear enum comparison.
    Clear(Clear),
    /// FlexDirection enum comparison.
    FlexDirection(FlexDirection),
    /// FlexWrap enum comparison.
    FlexWrap(FlexWrap),
    /// JustifyContent enum comparison.
    JustifyContent(JustifyContent),
    /// AlignItems enum comparison.
    AlignItems(AlignItems),
    /// AlignSelf enum comparison.
    AlignSelf(AlignSelf),
    /// AlignContent enum comparison.
    AlignContent(AlignContent),
    /// Dimension comparison.
    Dimension(Dimension),
    /// Overflow comparison.
    Overflow(liquide_compositor::scene::Overflow),
    /// TextAlign comparison.
    TextAlign(TextAlign),
    /// TextTransform comparison.
    TextTransform(TextTransform),
    /// TextOverflow comparison.
    TextOverflow(TextOverflow),
    /// FontStyle comparison.
    FontStyle(FontStyle),
    /// WritingMode comparison.
    WritingMode(WritingMode),
    /// Direction comparison.
    Direction(Direction),
    /// GridAutoFlow comparison.
    GridAutoFlow(GridAutoFlow),
    /// Cursor comparison.
    Cursor(Cursor),
    /// PointerEvents comparison.
    PointerEvents(PointerEvents),
    /// WhiteSpace comparison.
    WhiteSpace(WhiteSpace),
    /// WordBreak comparison.
    WordBreak(WordBreak),
    /// Opacity (f32 with epsilon).
    Opacity(f32),
    /// Bool comparison.
    Bool(bool),
    /// Optional i32 (z-index).
    OptionalI32(Option<i32>),
    /// BorderLineStyle comparison.
    BorderStyle(BorderLineStyle),
    /// Isolation comparison.
    Isolation(Isolation),
    /// TransformStyle comparison.
    TransformStyle(TransformStyle),
    /// BackfaceVisibility comparison.
    BackfaceVisibility(BackfaceVisibility),
    /// Resize comparison.
    Resize(Resize),
    /// ObjectFit comparison.
    ObjectFit(ObjectFit),
    /// UserSelect comparison.
    UserSelect(UserSelect),
    /// BorderCollapse comparison.
    BorderCollapse(BorderCollapse),
    /// ListStyleType comparison.
    ListStyleType(ListStyleType),
    /// ListStylePosition comparison.
    ListStylePosition(ListStylePosition),
    /// ContainerType comparison.
    ContainerType(ContainerType),
    /// ScrollBehavior comparison.
    ScrollBehavior(ScrollBehavior),
    /// ContentVisibility comparison.
    ContentVisibility(ContentVisibility),
}

/// A single style property assertion on a node.
struct StyleAssertion {
    /// Description for error messages.
    description: &'static str,
    /// Function to check the computed style.
    check: Box<dyn Fn(&ComputedStyle) -> Result<(), String>>,
}

/// A CSS conformance test case.
struct CssTestCase {
    /// Name of the test.
    name: &'static str,
    /// CSS source text.
    css: &'static str,
    /// Builds the DOM and returns node IDs to check.
    build_dom: Box<dyn Fn(&mut Document) -> Vec<liquide_dom::NodeId>>,
    /// Assertions: (node index in returned vec, assertion).
    assertions: Vec<(usize, StyleAssertion)>,
}

/// Runner for CSS conformance tests.
struct CssTestRunner;

impl CssTestRunner {
    fn run(test: &CssTestCase) {
        let mut engine = StyleEngine::default();
        engine.add_stylesheet(test.css);

        let mut doc = Document::new();
        let nodes = (test.build_dom)(&mut doc);

        let style_map = engine.restyle_all(&doc);

        for (node_idx, assertion) in &test.assertions {
            let node_id = nodes[*node_idx];
            let style = style_map.get(node_id).unwrap_or_else(|| {
                panic!("[{}] No style found for node index {}", test.name, node_idx)
            });

            if let Err(msg) = (assertion.check)(style) {
                panic!(
                    "[{}] Assertion failed for node {}: {} — {}",
                    test.name, node_idx, assertion.description, msg
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper macros and functions
// ═══════════════════════════════════════════════════════════════════════════

/// Helper to create a style assertion comparing a field to an expected value.
macro_rules! assert_style {
    ($desc:expr, $field:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = &style.$field;
                let expected = &$expected;
                if actual == expected {
                    Ok(())
                } else {
                    Err(format!("expected {:?}, got {:?}", expected, actual))
                }
            }),
        }
    };
}

/// Helper for float comparisons with epsilon.
macro_rules! assert_style_f32 {
    ($desc:expr, $field:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = style.$field;
                let expected: f32 = $expected;
                if (actual - expected).abs() < 0.01 {
                    Ok(())
                } else {
                    Err(format!("expected {}, got {}", expected, actual))
                }
            }),
        }
    };
}

/// Helper for color comparisons.
macro_rules! assert_color {
    ($desc:expr, $field:ident == ($r:expr, $g:expr, $b:expr)) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let c = &style.$field;
                if c.r == $r && c.g == $g && c.b == $b {
                    Ok(())
                } else {
                    Err(format!(
                        "expected rgb({},{},{}), got rgb({},{},{})",
                        $r, $g, $b, c.r, c.g, c.b
                    ))
                }
            }),
        }
    };
}

/// Helper for dimension comparison.
macro_rules! assert_dimension {
    ($desc:expr, $field:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = &style.$field;
                let expected = &$expected;
                if actual == expected {
                    Ok(())
                } else {
                    Err(format!("expected {:?}, got {:?}", expected, actual))
                }
            }),
        }
    };
}

/// Helper for side-field comparisons (margin.top, padding.left, etc.).
macro_rules! assert_side {
    ($desc:expr, $field:ident . $side:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = &style.$field.$side;
                let expected = &$expected;
                if actual == expected {
                    Ok(())
                } else {
                    Err(format!("expected {:?}, got {:?}", expected, actual))
                }
            }),
        }
    };
}

/// Helper for side-field f32 comparisons (border_width.top, etc.).
macro_rules! assert_side_f32 {
    ($desc:expr, $field:ident . $side:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = style.$field.$side;
                let expected: f32 = $expected;
                if (actual - expected).abs() < 0.01 {
                    Ok(())
                } else {
                    Err(format!("expected {}, got {}", expected, actual))
                }
            }),
        }
    };
}

/// Helper for corner-field f32 comparisons (border_radius).
macro_rules! assert_corner_f32 {
    ($desc:expr, $field:ident . $corner:ident == $expected:expr) => {
        StyleAssertion {
            description: $desc,
            check: Box::new(move |style| {
                let actual = style.$field.$corner;
                let expected: f32 = $expected;
                if (actual - expected).abs() < 0.01 {
                    Ok(())
                } else {
                    Err(format!("expected {}, got {}", expected, actual))
                }
            }),
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. DISPLAY, POSITION & BOX MODEL TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn display_block() {
    CssTestRunner::run(&CssTestCase {
        name: "display: block",
        css: "div { display: block; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Block))],
    });
}

#[test]
fn display_flex() {
    CssTestRunner::run(&CssTestCase {
        name: "display: flex",
        css: "div { display: flex; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Flex))],
    });
}

#[test]
fn display_inline() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline",
        css: "span { display: inline; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Inline))],
    });
}

#[test]
fn display_inline_block() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline-block",
        css: "span { display: inline-block; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::InlineBlock))],
    });
}

#[test]
fn display_inline_flex() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline-flex",
        css: "div { display: inline-flex; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::InlineFlex))],
    });
}

#[test]
fn display_grid() {
    CssTestRunner::run(&CssTestCase {
        name: "display: grid",
        css: "div { display: grid; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Grid))],
    });
}

#[test]
fn display_inline_grid() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline-grid",
        css: "div { display: inline-grid; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::InlineGrid))],
    });
}

#[test]
fn display_none() {
    CssTestRunner::run(&CssTestCase {
        name: "display: none",
        css: "div { display: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::None))],
    });
}

#[test]
fn display_contents() {
    CssTestRunner::run(&CssTestCase {
        name: "display: contents",
        css: "div { display: contents; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Contents))],
    });
}

#[test]
fn display_table() {
    CssTestRunner::run(&CssTestCase {
        name: "display: table",
        css: "div { display: table; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Table))],
    });
}

#[test]
fn display_flow_root() {
    CssTestRunner::run(&CssTestCase {
        name: "display: flow-root",
        css: "div { display: flow-root; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::FlowRoot))],
    });
}

#[test]
fn display_list_item() {
    CssTestRunner::run(&CssTestCase {
        name: "display: list-item",
        css: "li { display: list-item; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::ListItem))],
    });
}

#[test]
fn position_static() {
    CssTestRunner::run(&CssTestCase {
        name: "position: static",
        css: "div { position: static; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("position", position == Position::Static))],
    });
}

#[test]
fn position_relative() {
    CssTestRunner::run(&CssTestCase {
        name: "position: relative",
        css: "div { position: relative; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("position", position == Position::Relative))],
    });
}

#[test]
fn position_absolute() {
    CssTestRunner::run(&CssTestCase {
        name: "position: absolute",
        css: "div { position: absolute; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("position", position == Position::Absolute))],
    });
}

#[test]
fn position_fixed() {
    CssTestRunner::run(&CssTestCase {
        name: "position: fixed",
        css: "div { position: fixed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("position", position == Position::Fixed))],
    });
}

#[test]
fn position_sticky() {
    CssTestRunner::run(&CssTestCase {
        name: "position: sticky",
        css: "div { position: sticky; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("position", position == Position::Sticky))],
    });
}

#[test]
fn box_sizing_content_box() {
    CssTestRunner::run(&CssTestCase {
        name: "box-sizing: content-box",
        css: "div { box-sizing: content-box; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("box-sizing", box_sizing == BoxSizing::ContentBox),
        )],
    });
}

#[test]
fn box_sizing_border_box() {
    CssTestRunner::run(&CssTestCase {
        name: "box-sizing: border-box",
        css: "div { box-sizing: border-box; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("box-sizing", box_sizing == BoxSizing::BorderBox),
        )],
    });
}

#[test]
fn width_px() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 200px",
        css: "div { width: 200px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Px(200.0)))],
    });
}

#[test]
fn width_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 50%",
        css: "div { width: 50%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::Percent(50.0)),
        )],
    });
}

#[test]
fn width_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "width: auto",
        css: "div { width: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Auto))],
    });
}

#[test]
fn height_px() {
    CssTestRunner::run(&CssTestCase {
        name: "height: 300px",
        css: "div { height: 300px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("height", height == Dimension::Px(300.0)),
        )],
    });
}

#[test]
fn min_width_px() {
    CssTestRunner::run(&CssTestCase {
        name: "min-width: 100px",
        css: "div { min-width: 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("min-width", min_width == Dimension::Px(100.0)),
        )],
    });
}

#[test]
fn max_width_px() {
    CssTestRunner::run(&CssTestCase {
        name: "max-width: 500px",
        css: "div { max-width: 500px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-width", max_width == Dimension::Px(500.0)),
        )],
    });
}

#[test]
fn max_width_none() {
    CssTestRunner::run(&CssTestCase {
        name: "max-width: none",
        css: "div { max-width: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-width", max_width == Dimension::None),
        )],
    });
}

#[test]
fn min_height_px() {
    CssTestRunner::run(&CssTestCase {
        name: "min-height: 50px",
        css: "div { min-height: 50px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("min-height", min_height == Dimension::Px(50.0)),
        )],
    });
}

#[test]
fn max_height_px() {
    CssTestRunner::run(&CssTestCase {
        name: "max-height: 800px",
        css: "div { max-height: 800px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-height", max_height == Dimension::Px(800.0)),
        )],
    });
}

#[test]
fn margin_all_sides() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 10px",
        css: "div { margin: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(10.0)),
            ),
        ],
    });
}

#[test]
fn margin_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: auto",
        css: "div { margin: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_side!("margin-top", margin.top == Dimension::Auto)),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Auto),
            ),
        ],
    });
}

#[test]
fn margin_individual_sides() {
    CssTestRunner::run(&CssTestCase {
        name: "margin individual sides",
        css: "div { margin-top: 5px; margin-right: 10px; margin-bottom: 15px; margin-left: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(5.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(15.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn padding_all_sides() {
    CssTestRunner::run(&CssTestCase {
        name: "padding: 20px",
        css: "div { padding: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("padding-right", padding.right == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("padding-bottom", padding.bottom == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("padding-left", padding.left == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn padding_individual_sides() {
    CssTestRunner::run(&CssTestCase {
        name: "padding individual sides",
        css:
            "div { padding-top: 2px; padding-right: 4px; padding-bottom: 6px; padding-left: 8px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(2.0)),
            ),
            (
                0,
                assert_side!("padding-right", padding.right == Dimension::Px(4.0)),
            ),
            (
                0,
                assert_side!("padding-bottom", padding.bottom == Dimension::Px(6.0)),
            ),
            (
                0,
                assert_side!("padding-left", padding.left == Dimension::Px(8.0)),
            ),
        ],
    });
}

#[test]
fn padding_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "padding: 5%",
        css: "div { padding: 5%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!("padding-top", padding.top == Dimension::Percent(5.0)),
        )],
    });
}

#[test]
fn border_width_all() {
    CssTestRunner::run(&CssTestCase {
        name: "border-width: 2px",
        css: "div { border-width: 2px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side_f32!("border-width-top", border_width.top == 2.0),
            ),
            (
                0,
                assert_side_f32!("border-width-right", border_width.right == 2.0),
            ),
            (
                0,
                assert_side_f32!("border-width-bottom", border_width.bottom == 2.0),
            ),
            (
                0,
                assert_side_f32!("border-width-left", border_width.left == 2.0),
            ),
        ],
    });
}

#[test]
fn border_style_solid() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: solid",
        css: "div { border-style: solid; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!(
                    "border-style-top",
                    border_style.top == BorderLineStyle::Solid
                ),
            ),
            (
                0,
                assert_side!(
                    "border-style-left",
                    border_style.left == BorderLineStyle::Solid
                ),
            ),
        ],
    });
}

#[test]
fn border_style_dashed() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: dashed",
        css: "div { border-style: dashed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::Dashed
            ),
        )],
    });
}

#[test]
fn border_style_dotted() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: dotted",
        css: "div { border-style: dotted; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::Dotted
            ),
        )],
    });
}

#[test]
fn border_radius_uniform() {
    CssTestRunner::run(&CssTestCase {
        name: "border-radius: 8px",
        css: "div { border-radius: 8px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_corner_f32!("border-top-left-radius", border_radius.top_left == 8.0),
            ),
            (
                0,
                assert_corner_f32!("border-top-right-radius", border_radius.top_right == 8.0),
            ),
            (
                0,
                assert_corner_f32!(
                    "border-bottom-right-radius",
                    border_radius.bottom_right == 8.0
                ),
            ),
            (
                0,
                assert_corner_f32!(
                    "border-bottom-left-radius",
                    border_radius.bottom_left == 8.0
                ),
            ),
        ],
    });
}

#[test]
fn border_color_hex() {
    CssTestRunner::run(&CssTestCase {
        name: "border-color: #ff0000",
        css: "div { border-color: #ff0000; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "border-color-top red",
                check: Box::new(|style| {
                    if style.border_color.top.r == 255 && style.border_color.top.g == 0 {
                        Ok(())
                    } else {
                        Err(format!("expected red, got {:?}", style.border_color.top))
                    }
                }),
            },
        )],
    });
}

#[test]
fn position_offsets() {
    CssTestRunner::run(&CssTestCase {
        name: "top/right/bottom/left offsets",
        css: "div { position: absolute; top: 10px; right: 20px; bottom: 30px; left: 40px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("position", position == Position::Absolute)),
            (0, assert_dimension!("top", top == Dimension::Px(10.0))),
            (0, assert_dimension!("right", right == Dimension::Px(20.0))),
            (
                0,
                assert_dimension!("bottom", bottom == Dimension::Px(30.0)),
            ),
            (0, assert_dimension!("left", left == Dimension::Px(40.0))),
        ],
    });
}

#[test]
fn float_left() {
    CssTestRunner::run(&CssTestCase {
        name: "float: left",
        css: "div { float: left; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("float", float == Float::Left))],
    });
}

#[test]
fn float_right() {
    CssTestRunner::run(&CssTestCase {
        name: "float: right",
        css: "div { float: right; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("float", float == Float::Right))],
    });
}

#[test]
fn clear_both() {
    CssTestRunner::run(&CssTestCase {
        name: "clear: both",
        css: "div { clear: both; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("clear", clear == Clear::Both))],
    });
}

#[test]
fn visibility_hidden() {
    CssTestRunner::run(&CssTestCase {
        name: "visibility: hidden",
        css: "div { visibility: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("visibility", visibility == Visibility::Hidden),
        )],
    });
}

#[test]
fn z_index_value() {
    CssTestRunner::run(&CssTestCase {
        name: "z-index: 10",
        css: "div { z-index: 10; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("z-index", z_index == Some(10)))],
    });
}

#[test]
fn width_em_units() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 10em",
        css: "div { width: 10em; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Em(10.0)))],
    });
}

#[test]
fn width_rem_units() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 2rem",
        css: "div { width: 2rem; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Rem(2.0)))],
    });
}

#[test]
fn width_vw_units() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 100vw",
        css: "div { width: 100vw; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Vw(100.0)))],
    });
}

#[test]
fn height_vh_units() {
    CssTestRunner::run(&CssTestCase {
        name: "height: 100vh",
        css: "div { height: 100vh; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("height", height == Dimension::Vh(100.0)),
        )],
    });
}

#[test]
fn width_min_content() {
    CssTestRunner::run(&CssTestCase {
        name: "width: min-content",
        css: "div { width: min-content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::MinContent),
        )],
    });
}

#[test]
fn width_max_content() {
    CssTestRunner::run(&CssTestCase {
        name: "width: max-content",
        css: "div { width: max-content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::MaxContent),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. COLOR & TYPOGRAPHY TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn color_named_red() {
    CssTestRunner::run(&CssTestCase {
        name: "color: red",
        css: "div { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (255, 0, 0)))],
    });
}

#[test]
fn color_named_green() {
    CssTestRunner::run(&CssTestCase {
        name: "color: green",
        css: "div { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (0, 128, 0)))],
    });
}

#[test]
fn color_named_blue() {
    CssTestRunner::run(&CssTestCase {
        name: "color: blue",
        css: "div { color: blue; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (0, 0, 255)))],
    });
}

#[test]
fn color_hex_6digit() {
    CssTestRunner::run(&CssTestCase {
        name: "color: #ff8800",
        css: "div { color: #ff8800; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (255, 136, 0)))],
    });
}

#[test]
fn color_hex_3digit() {
    CssTestRunner::run(&CssTestCase {
        name: "color: #f00",
        css: "div { color: #f00; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (255, 0, 0)))],
    });
}

#[test]
fn color_rgb_function() {
    CssTestRunner::run(&CssTestCase {
        name: "color: rgb(100, 200, 50)",
        css: "div { color: rgb(100, 200, 50); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (100, 200, 50)))],
    });
}

#[test]
fn background_color_hex() {
    CssTestRunner::run(&CssTestCase {
        name: "background-color: #00ff00",
        css: "div { background-color: #00ff00; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("background-color", background_color == (0, 255, 0)),
        )],
    });
}

#[test]
fn background_color_named() {
    CssTestRunner::run(&CssTestCase {
        name: "background-color: white",
        css: "div { background-color: white; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("background-color", background_color == (255, 255, 255)),
        )],
    });
}

#[test]
fn font_size_px() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size: 24px",
        css: "div { font-size: 24px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("font-size", font_size == 24.0))],
    });
}

#[test]
fn font_size_em() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size: 2em inherits",
        css: ".parent { font-size: 20px; } .child { font-size: 2em; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.add_class(child, "child");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (0, assert_style_f32!("parent font-size", font_size == 20.0)),
            (1, assert_style_f32!("child font-size", font_size == 40.0)),
        ],
    });
}

#[test]
fn font_weight_bold() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: bold",
        css: "div { font-weight: bold; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight: bold (700)",
                check: Box::new(|style| {
                    if style.font_weight == 700 {
                        Ok(())
                    } else {
                        Err(format!("expected 700, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_weight_numeric() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: 300",
        css: "div { font-weight: 300; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight: 300",
                check: Box::new(|style| {
                    if style.font_weight == 300 {
                        Ok(())
                    } else {
                        Err(format!("expected 300, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_style_italic() {
    CssTestRunner::run(&CssTestCase {
        name: "font-style: italic",
        css: "div { font-style: italic; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("font-style", font_style == FontStyle::Italic),
        )],
    });
}

#[test]
fn text_align_center() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: center",
        css: "div { text-align: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Center),
        )],
    });
}

#[test]
fn text_align_right() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: right",
        css: "div { text-align: right; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Right),
        )],
    });
}

#[test]
fn text_align_justify() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: justify",
        css: "div { text-align: justify; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Justify),
        )],
    });
}

#[test]
fn text_transform_uppercase() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: uppercase",
        css: "div { text-transform: uppercase; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-transform", text_transform == TextTransform::Uppercase),
        )],
    });
}

#[test]
fn text_transform_lowercase() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: lowercase",
        css: "div { text-transform: lowercase; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-transform", text_transform == TextTransform::Lowercase),
        )],
    });
}

#[test]
fn text_transform_capitalize() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: capitalize",
        css: "div { text-transform: capitalize; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "text-transform",
                text_transform == TextTransform::Capitalize
            ),
        )],
    });
}

#[test]
fn text_overflow_ellipsis() {
    CssTestRunner::run(&CssTestCase {
        name: "text-overflow: ellipsis",
        css: "div { text-overflow: ellipsis; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-overflow", text_overflow == TextOverflow::Ellipsis),
        )],
    });
}

#[test]
fn line_height_number() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height: 1.5",
        css: "div { line-height: 1.5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("line-height", line_height == LineHeight::Number(1.5)),
        )],
    });
}

#[test]
fn line_height_px() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height: 24px",
        css: "div { line-height: 24px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("line-height", line_height == LineHeight::Px(24.0)),
        )],
    });
}

#[test]
fn letter_spacing_px() {
    CssTestRunner::run(&CssTestCase {
        name: "letter-spacing: 2px",
        css: "div { letter-spacing: 2px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style_f32!("letter-spacing", letter_spacing == 2.0),
        )],
    });
}

#[test]
fn word_spacing_px() {
    CssTestRunner::run(&CssTestCase {
        name: "word-spacing: 4px",
        css: "div { word-spacing: 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("word-spacing", word_spacing == 4.0))],
    });
}

#[test]
fn text_indent_px() {
    CssTestRunner::run(&CssTestCase {
        name: "text-indent: 32px",
        css: "div { text-indent: 32px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("text-indent", text_indent == 32.0))],
    });
}

#[test]
fn color_named_black() {
    CssTestRunner::run(&CssTestCase {
        name: "color: black",
        css: "div { color: black; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (0, 0, 0)))],
    });
}

#[test]
fn color_named_white() {
    CssTestRunner::run(&CssTestCase {
        name: "color: white",
        css: "div { color: white; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color", color == (255, 255, 255)))],
    });
}

#[test]
fn background_color_transparent() {
    CssTestRunner::run(&CssTestCase {
        name: "background-color: transparent",
        css: "div { background-color: transparent; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "background-color alpha = 0",
                check: Box::new(|style| {
                    if style.background_color.a == 0 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected alpha=0, got {}",
                            style.background_color.a
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn writing_mode_vertical_rl() {
    CssTestRunner::run(&CssTestCase {
        name: "writing-mode: vertical-rl",
        css: "div { writing-mode: vertical-rl; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("writing-mode", writing_mode == WritingMode::VerticalRl),
        )],
    });
}

#[test]
fn writing_mode_vertical_lr() {
    CssTestRunner::run(&CssTestCase {
        name: "writing-mode: vertical-lr",
        css: "div { writing-mode: vertical-lr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("writing-mode", writing_mode == WritingMode::VerticalLr),
        )],
    });
}

#[test]
fn direction_rtl() {
    CssTestRunner::run(&CssTestCase {
        name: "direction: rtl",
        css: "div { direction: rtl; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("direction", direction == Direction::Rtl))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. FLEXBOX TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flex_direction_row() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-direction: row",
        css: "div { display: flex; flex-direction: row; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Flex)),
            (
                0,
                assert_style!("flex-direction", flex_direction == FlexDirection::Row),
            ),
        ],
    });
}

#[test]
fn flex_direction_column() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-direction: column",
        css: "div { display: flex; flex-direction: column; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("flex-direction", flex_direction == FlexDirection::Column),
        )],
    });
}

#[test]
fn flex_direction_row_reverse() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-direction: row-reverse",
        css: "div { display: flex; flex-direction: row-reverse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "flex-direction",
                flex_direction == FlexDirection::RowReverse
            ),
        )],
    });
}

#[test]
fn flex_direction_column_reverse() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-direction: column-reverse",
        css: "div { display: flex; flex-direction: column-reverse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "flex-direction",
                flex_direction == FlexDirection::ColumnReverse
            ),
        )],
    });
}

#[test]
fn flex_wrap_wrap() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-wrap: wrap",
        css: "div { display: flex; flex-wrap: wrap; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("flex-wrap", flex_wrap == FlexWrap::Wrap))],
    });
}

#[test]
fn flex_wrap_nowrap() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-wrap: nowrap",
        css: "div { display: flex; flex-wrap: nowrap; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("flex-wrap", flex_wrap == FlexWrap::NoWrap))],
    });
}

#[test]
fn flex_wrap_wrap_reverse() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-wrap: wrap-reverse",
        css: "div { display: flex; flex-wrap: wrap-reverse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("flex-wrap", flex_wrap == FlexWrap::WrapReverse),
        )],
    });
}

#[test]
fn justify_content_center() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: center",
        css: "div { display: flex; justify-content: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("justify-content", justify_content == JustifyContent::Center),
        )],
    });
}

#[test]
fn justify_content_space_between() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: space-between",
        css: "div { display: flex; justify-content: space-between; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "justify-content",
                justify_content == JustifyContent::SpaceBetween
            ),
        )],
    });
}

#[test]
fn justify_content_space_around() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: space-around",
        css: "div { display: flex; justify-content: space-around; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "justify-content",
                justify_content == JustifyContent::SpaceAround
            ),
        )],
    });
}

#[test]
fn justify_content_space_evenly() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: space-evenly",
        css: "div { display: flex; justify-content: space-evenly; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "justify-content",
                justify_content == JustifyContent::SpaceEvenly
            ),
        )],
    });
}

#[test]
fn justify_content_flex_end() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: flex-end",
        css: "div { display: flex; justify-content: flex-end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "justify-content",
                justify_content == JustifyContent::FlexEnd
            ),
        )],
    });
}

#[test]
fn align_items_center() {
    CssTestRunner::run(&CssTestCase {
        name: "align-items: center",
        css: "div { display: flex; align-items: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-items", align_items == AlignItems::Center),
        )],
    });
}

#[test]
fn align_items_flex_start() {
    CssTestRunner::run(&CssTestCase {
        name: "align-items: flex-start",
        css: "div { display: flex; align-items: flex-start; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-items", align_items == AlignItems::FlexStart),
        )],
    });
}

#[test]
fn align_items_flex_end() {
    CssTestRunner::run(&CssTestCase {
        name: "align-items: flex-end",
        css: "div { display: flex; align-items: flex-end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-items", align_items == AlignItems::FlexEnd),
        )],
    });
}

#[test]
fn align_items_stretch() {
    CssTestRunner::run(&CssTestCase {
        name: "align-items: stretch",
        css: "div { display: flex; align-items: stretch; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-items", align_items == AlignItems::Stretch),
        )],
    });
}

#[test]
fn align_self_center() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: center",
        css: ".item { align-self: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::Center),
        )],
    });
}

#[test]
fn flex_grow_value() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-grow: 2",
        css: ".item { flex-grow: 2; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-grow", flex_grow == 2.0))],
    });
}

#[test]
fn flex_shrink_value() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-shrink: 0",
        css: ".item { flex-shrink: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-shrink", flex_shrink == 0.0))],
    });
}

#[test]
fn flex_basis_px() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: 100px",
        css: ".item { flex-basis: 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Px(100.0)),
        )],
    });
}

#[test]
fn flex_basis_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: auto",
        css: ".item { flex-basis: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Auto),
        )],
    });
}

#[test]
fn gap_px() {
    CssTestRunner::run(&CssTestCase {
        name: "gap: 10px",
        css: "div { display: flex; gap: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "gap width",
                    check: Box::new(|style| {
                        if style.gap.width == Dimension::Px(10.0) {
                            Ok(())
                        } else {
                            Err(format!("expected Px(10.0), got {:?}", style.gap.width))
                        }
                    }),
                },
            ),
            (
                0,
                StyleAssertion {
                    description: "gap height",
                    check: Box::new(|style| {
                        if style.gap.height == Dimension::Px(10.0) {
                            Ok(())
                        } else {
                            Err(format!("expected Px(10.0), got {:?}", style.gap.height))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn order_value() {
    CssTestRunner::run(&CssTestCase {
        name: "order: 3",
        css: ".item { order: 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "order",
                check: Box::new(|style| {
                    if style.order == 3 {
                        Ok(())
                    } else {
                        Err(format!("expected 3, got {}", style.order))
                    }
                }),
            },
        )],
    });
}

#[test]
fn align_content_center() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: center",
        css: "div { display: flex; flex-wrap: wrap; align-content: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::Center),
        )],
    });
}

#[test]
fn align_content_space_between() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: space-between",
        css: "div { display: flex; flex-wrap: wrap; align-content: space-between; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::SpaceBetween),
        )],
    });
}

#[test]
fn flex_container_with_children() {
    CssTestRunner::run(&CssTestCase {
        name: "flex container with flex items",
        css: r#"
            .container { display: flex; flex-direction: row; gap: 8px; }
            .item { flex-grow: 1; flex-shrink: 0; flex-basis: 50px; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let container = doc.create_element("div");
            doc.add_class(container, "container");
            doc.append_child(root, container);

            let item1 = doc.create_element("div");
            doc.add_class(item1, "item");
            doc.append_child(container, item1);

            let item2 = doc.create_element("div");
            doc.add_class(item2, "item");
            doc.append_child(container, item2);

            vec![container, item1, item2]
        }),
        assertions: vec![
            (
                0,
                assert_style!("container display", display == Display::Flex),
            ),
            (
                0,
                assert_style!(
                    "container flex-direction",
                    flex_direction == FlexDirection::Row
                ),
            ),
            (1, assert_style_f32!("item1 flex-grow", flex_grow == 1.0)),
            (
                1,
                assert_style_f32!("item1 flex-shrink", flex_shrink == 0.0),
            ),
            (
                1,
                assert_dimension!("item1 flex-basis", flex_basis == Dimension::Px(50.0)),
            ),
            (2, assert_style_f32!("item2 flex-grow", flex_grow == 1.0)),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. GRID TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn grid_auto_flow_row() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: row",
        css: "div { display: grid; grid-auto-flow: row; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::Row),
        )],
    });
}

#[test]
fn grid_auto_flow_column() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: column",
        css: "div { display: grid; grid-auto-flow: column; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::Column),
        )],
    });
}

#[test]
fn grid_template_columns_px() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-columns: 100px 200px",
        css: "div { display: grid; grid-template-columns: 100px 200px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-template-columns",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 2
                        && style.grid_template_columns[0] == TrackSize::Px(100.0)
                        && style.grid_template_columns[1] == TrackSize::Px(200.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [100px, 200px], got {:?}",
                            style.grid_template_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_template_columns_fr() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-columns: 1fr 2fr",
        css: "div { display: grid; grid-template-columns: 1fr 2fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-template-columns fr",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 2
                        && style.grid_template_columns[0] == TrackSize::Fr(1.0)
                        && style.grid_template_columns[1] == TrackSize::Fr(2.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [1fr, 2fr], got {:?}",
                            style.grid_template_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_template_rows_px() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-rows: 50px 100px",
        css: "div { display: grid; grid-template-rows: 50px 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-template-rows",
                check: Box::new(|style| {
                    if style.grid_template_rows.len() == 2
                        && style.grid_template_rows[0] == TrackSize::Px(50.0)
                        && style.grid_template_rows[1] == TrackSize::Px(100.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [50px, 100px], got {:?}",
                            style.grid_template_rows
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_column_line() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-column: 1 / 3",
        css: ".item { grid-column: 1 / 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-column placement",
                check: Box::new(|style| {
                    if style.grid_column.start == GridLine::Line(1)
                        && style.grid_column.end == GridLine::Line(3)
                    {
                        Ok(())
                    } else {
                        Err(format!("expected 1/3, got {:?}", style.grid_column))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_row_span() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-row: span 2",
        css: ".item { grid-row: span 2; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-row span",
                check: Box::new(|style| {
                    if style.grid_row.start == GridLine::Span(2) {
                        Ok(())
                    } else {
                        Err(format!("expected span 2, got {:?}", style.grid_row.start))
                    }
                }),
            },
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. VISUAL PROPERTY TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn opacity_half() {
    CssTestRunner::run(&CssTestCase {
        name: "opacity: 0.5",
        css: "div { opacity: 0.5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("opacity", opacity == 0.5))],
    });
}

#[test]
fn opacity_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "opacity: 0",
        css: "div { opacity: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("opacity", opacity == 0.0))],
    });
}

#[test]
fn opacity_full() {
    CssTestRunner::run(&CssTestCase {
        name: "opacity: 1",
        css: "div { opacity: 1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("opacity", opacity == 1.0))],
    });
}

#[test]
fn overflow_hidden() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow: hidden",
        css: "div { overflow: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "overflow-x",
                    overflow_x == liquide_compositor::scene::Overflow::Hidden
                ),
            ),
            (
                0,
                assert_style!(
                    "overflow-y",
                    overflow_y == liquide_compositor::scene::Overflow::Hidden
                ),
            ),
        ],
    });
}

#[test]
fn overflow_scroll() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow: scroll",
        css: "div { overflow: scroll; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "overflow-x",
                    overflow_x == liquide_compositor::scene::Overflow::Scroll
                ),
            ),
            (
                0,
                assert_style!(
                    "overflow-y",
                    overflow_y == liquide_compositor::scene::Overflow::Scroll
                ),
            ),
        ],
    });
}

#[test]
fn overflow_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow: auto",
        css: "div { overflow: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "overflow-x",
                    overflow_x == liquide_compositor::scene::Overflow::Auto
                ),
            ),
            (
                0,
                assert_style!(
                    "overflow-y",
                    overflow_y == liquide_compositor::scene::Overflow::Auto
                ),
            ),
        ],
    });
}

#[test]
fn cursor_pointer() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: pointer",
        css: "div { cursor: pointer; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Pointer))],
    });
}

#[test]
fn cursor_text() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: text",
        css: "div { cursor: text; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Text))],
    });
}

#[test]
fn cursor_not_allowed() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: not-allowed",
        css: "div { cursor: not-allowed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::NotAllowed))],
    });
}

#[test]
fn pointer_events_none() {
    CssTestRunner::run(&CssTestCase {
        name: "pointer-events: none",
        css: "div { pointer-events: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("pointer-events", pointer_events == PointerEvents::None),
        )],
    });
}

#[test]
fn object_fit_cover() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: cover",
        css: "img { object-fit: cover; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::Cover),
        )],
    });
}

#[test]
fn object_fit_contain() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: contain",
        css: "img { object-fit: contain; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::Contain),
        )],
    });
}

#[test]
fn resize_both() {
    CssTestRunner::run(&CssTestCase {
        name: "resize: both",
        css: "div { resize: both; overflow: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("resize", resize == Resize::Both))],
    });
}

#[test]
fn isolation_isolate() {
    CssTestRunner::run(&CssTestCase {
        name: "isolation: isolate",
        css: "div { isolation: isolate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("isolation", isolation == Isolation::Isolate),
        )],
    });
}

#[test]
fn backface_visibility_hidden() {
    CssTestRunner::run(&CssTestCase {
        name: "backface-visibility: hidden",
        css: "div { backface-visibility: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "backface-visibility",
                backface_visibility == BackfaceVisibility::Hidden
            ),
        )],
    });
}

#[test]
fn user_select_none() {
    CssTestRunner::run(&CssTestCase {
        name: "user-select: none",
        css: "div { user-select: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("user-select", user_select == UserSelect::None),
        )],
    });
}

#[test]
fn border_collapse_collapse() {
    CssTestRunner::run(&CssTestCase {
        name: "border-collapse: collapse",
        css: "table { border-collapse: collapse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let table = doc.create_element("table");
            doc.append_child(root, table);
            vec![table]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "border-collapse",
                border_collapse == BorderCollapse::Collapse
            ),
        )],
    });
}

#[test]
fn scroll_behavior_smooth() {
    CssTestRunner::run(&CssTestCase {
        name: "scroll-behavior: smooth",
        css: "div { scroll-behavior: smooth; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("scroll-behavior", scroll_behavior == ScrollBehavior::Smooth),
        )],
    });
}

#[test]
fn content_visibility_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "content-visibility: auto",
        css: "div { content-visibility: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "content-visibility",
                content_visibility == ContentVisibility::Auto
            ),
        )],
    });
}

#[test]
fn content_visibility_hidden() {
    CssTestRunner::run(&CssTestCase {
        name: "content-visibility: hidden",
        css: "div { content-visibility: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "content-visibility",
                content_visibility == ContentVisibility::Hidden
            ),
        )],
    });
}

#[test]
fn list_style_type_disc() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-type: disc",
        css: "li { list-style-type: disc; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!("list-style-type", list_style_type == ListStyleType::Disc),
        )],
    });
}

#[test]
fn list_style_type_none() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-type: none",
        css: "li { list-style-type: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!("list-style-type", list_style_type == ListStyleType::None),
        )],
    });
}

#[test]
fn list_style_position_inside() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-position: inside",
        css: "li { list-style-position: inside; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "list-style-position",
                list_style_position == ListStylePosition::Inside
            ),
        )],
    });
}

#[test]
fn border_spacing_px() {
    CssTestRunner::run(&CssTestCase {
        name: "border-spacing: 4px",
        css: "table { border-spacing: 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let table = doc.create_element("table");
            doc.append_child(root, table);
            vec![table]
        }),
        assertions: vec![(
            0,
            assert_style_f32!("border-spacing", border_spacing == 4.0),
        )],
    });
}

#[test]
fn tab_size_value() {
    CssTestRunner::run(&CssTestCase {
        name: "tab-size: 4",
        css: "pre { tab-size: 4; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let pre = doc.create_element("pre");
            doc.append_child(root, pre);
            vec![pre]
        }),
        assertions: vec![(0, assert_style_f32!("tab-size", tab_size == 4.0))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. INHERITANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn color_inherits_to_child() {
    CssTestRunner::run(&CssTestCase {
        name: "color inherits to child",
        css: ".parent { color: #ff0000; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("span");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (0, assert_color!("parent color", color == (255, 0, 0))),
            (
                1,
                assert_color!("child inherits color", color == (255, 0, 0)),
            ),
        ],
    });
}

#[test]
fn font_size_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size inherits to child",
        css: ".parent { font-size: 32px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("span");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (0, assert_style_f32!("parent font-size", font_size == 32.0)),
            (
                1,
                assert_style_f32!("child inherits font-size", font_size == 32.0),
            ),
        ],
    });
}

#[test]
fn display_does_not_inherit() {
    CssTestRunner::run(&CssTestCase {
        name: "display does not inherit",
        css: ".parent { display: flex; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (0, assert_style!("parent display", display == Display::Flex)),
            (
                1,
                assert_style!("child default display", display == Display::Block),
            ),
        ],
    });
}

#[test]
fn font_family_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "font-family inherits",
        css: r#".parent { font-family: "Helvetica"; }"#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("span");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "parent font-family",
                    check: Box::new(|style| {
                        if style.font_family.contains(&"Helvetica".to_string()) {
                            Ok(())
                        } else {
                            Err(format!("expected Helvetica, got {:?}", style.font_family))
                        }
                    }),
                },
            ),
            (
                1,
                StyleAssertion {
                    description: "child inherits font-family",
                    check: Box::new(|style| {
                        if style.font_family.contains(&"Helvetica".to_string()) {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected Helvetica inherited, got {:?}",
                                style.font_family
                            ))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn text_align_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align inherits",
        css: ".parent { text-align: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style!("parent text-align", text_align == TextAlign::Center),
            ),
            (
                1,
                assert_style!("child inherits text-align", text_align == TextAlign::Center),
            ),
        ],
    });
}

#[test]
fn line_height_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height inherits",
        css: ".parent { line-height: 1.6; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("span");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style!("parent line-height", line_height == LineHeight::Number(1.6)),
            ),
            (
                1,
                assert_style!(
                    "child inherits line-height",
                    line_height == LineHeight::Number(1.6)
                ),
            ),
        ],
    });
}

#[test]
fn visibility_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "visibility inherits",
        css: ".parent { visibility: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style!("parent visibility", visibility == Visibility::Hidden),
            ),
            (
                1,
                assert_style!(
                    "child inherits visibility",
                    visibility == Visibility::Hidden
                ),
            ),
        ],
    });
}

#[test]
fn writing_mode_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "writing-mode inherits",
        css: ".parent { writing-mode: vertical-rl; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "parent writing-mode",
                    writing_mode == WritingMode::VerticalRl
                ),
            ),
            (
                1,
                assert_style!(
                    "child inherits writing-mode",
                    writing_mode == WritingMode::VerticalRl
                ),
            ),
        ],
    });
}

#[test]
fn direction_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "direction inherits",
        css: ".parent { direction: rtl; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("div");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style!("parent direction", direction == Direction::Rtl),
            ),
            (
                1,
                assert_style!("child inherits direction", direction == Direction::Rtl),
            ),
        ],
    });
}

#[test]
fn letter_spacing_inherits() {
    CssTestRunner::run(&CssTestCase {
        name: "letter-spacing inherits",
        css: ".parent { letter-spacing: 3px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let child = doc.create_element("span");
            doc.append_child(parent, child);
            vec![parent, child]
        }),
        assertions: vec![
            (
                0,
                assert_style_f32!("parent letter-spacing", letter_spacing == 3.0),
            ),
            (
                1,
                assert_style_f32!("child inherits letter-spacing", letter_spacing == 3.0),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. CASCADE & SPECIFICITY TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn specificity_class_beats_tag() {
    CssTestRunner::run(&CssTestCase {
        name: "class selector beats tag selector",
        css: r#"
            div { color: red; }
            .blue { color: blue; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "blue");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("class beats tag", color == (0, 0, 255)))],
    });
}

#[test]
fn specificity_id_beats_class() {
    CssTestRunner::run(&CssTestCase {
        name: "id selector beats class selector",
        css: r#"
            .red { color: red; }
            #green { color: green; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "red");
            doc.set_id(div, "green");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("id beats class", color == (0, 128, 0)))],
    });
}

#[test]
fn source_order_later_wins() {
    CssTestRunner::run(&CssTestCase {
        name: "later rule of same specificity wins",
        css: r#"
            div { color: red; }
            div { color: blue; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("later rule wins", color == (0, 0, 255)))],
    });
}

#[test]
fn multiple_class_specificity() {
    CssTestRunner::run(&CssTestCase {
        name: "two classes beat one class",
        css: r#"
            .a.b { color: green; }
            .a { color: red; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "a");
            doc.add_class(div, "b");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("two classes beat one", color == (0, 128, 0)),
        )],
    });
}

#[test]
fn descendant_selector_specificity() {
    CssTestRunner::run(&CssTestCase {
        name: "descendant selector adds specificity",
        css: r#"
            div span { color: green; }
            span { color: red; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            let span = doc.create_element("span");
            doc.append_child(div, span);
            vec![div, span]
        }),
        assertions: vec![(
            1,
            assert_color!("descendant specificity", color == (0, 128, 0)),
        )],
    });
}

#[test]
fn multiple_stylesheets_cascade() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet("div { color: red; }");
    engine.add_stylesheet("div { color: blue; }");

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
    assert_eq!(style.color.r, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. MEDIA QUERY TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn media_query_prefers_color_scheme_light() {
    CssTestRunner::run(&CssTestCase {
        name: "prefers-color-scheme: light",
        css: r#"
            div { color: black; }
            @media (prefers-color-scheme: light) {
                div { color: green; }
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("light scheme active", color == (0, 128, 0)),
        )],
    });
}

#[test]
fn media_query_prefers_color_scheme_dark() {
    let mut engine = StyleEngine::default();
    engine.set_preferred_color_scheme("dark");
    engine.add_stylesheet(
        r#"
            div { color: black; }
            @media (prefers-color-scheme: dark) {
                div { color: white; }
            }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.r, 255);
    assert_eq!(style.color.g, 255);
    assert_eq!(style.color.b, 255);
}

#[test]
fn media_query_dark_not_active_by_default() {
    CssTestRunner::run(&CssTestCase {
        name: "dark media query not active by default",
        css: r#"
            div { color: red; }
            @media (prefers-color-scheme: dark) {
                div { color: blue; }
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("dark not active", color == (255, 0, 0)))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. @SUPPORTS TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn supports_display_grid() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (display: grid)",
        css: r#"
            div { color: red; }
            @supports (display: grid) {
                div { color: green; }
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports grid", color == (0, 128, 0)))],
    });
}

#[test]
fn supports_nonexistent_property() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (nonexistent-prop: foo) should not apply",
        css: r#"
            div { color: red; }
            @supports (nonexistent-prop: foo) {
                div { color: blue; }
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("@supports unsupported", color == (255, 0, 0)),
        )],
    });
}

#[test]
fn supports_display_flex() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (display: flex)",
        css: r#"
            div { color: red; }
            @supports (display: flex) {
                div { color: green; }
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports flex", color == (0, 128, 0)))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. @SCOPE TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn scope_basic() {
    CssTestRunner::run(&CssTestCase {
        name: "@scope basic scoping",
        css: r#"
            @scope (.panel) {
                button { color: green; }
            }
            button { color: red; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let panel = doc.create_element("div");
            doc.add_class(panel, "panel");
            doc.append_child(root, panel);
            let scoped = doc.create_element("button");
            doc.append_child(panel, scoped);
            let unscoped = doc.create_element("button");
            doc.append_child(root, unscoped);
            vec![scoped, unscoped]
        }),
        assertions: vec![
            (
                0,
                assert_color!("scoped button green", color == (0, 128, 0)),
            ),
            (
                1,
                assert_color!("unscoped button red", color == (255, 0, 0)),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. SELECTOR TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn selector_tag() {
    CssTestRunner::run(&CssTestCase {
        name: "tag selector",
        css: "span { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(0, assert_color!("tag selector", color == (255, 0, 0)))],
    });
}

#[test]
fn selector_class() {
    CssTestRunner::run(&CssTestCase {
        name: "class selector",
        css: ".highlight { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "highlight");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("class selector", color == (0, 128, 0)))],
    });
}

#[test]
fn selector_id() {
    CssTestRunner::run(&CssTestCase {
        name: "id selector",
        css: "#main { color: blue; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.set_id(div, "main");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("id selector", color == (0, 0, 255)))],
    });
}

#[test]
fn selector_descendant() {
    CssTestRunner::run(&CssTestCase {
        name: "descendant selector",
        css: ".outer span { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let outer = doc.create_element("div");
            doc.add_class(outer, "outer");
            doc.append_child(root, outer);
            let inner = doc.create_element("div");
            doc.append_child(outer, inner);
            let span = doc.create_element("span");
            doc.append_child(inner, span);
            vec![outer, inner, span]
        }),
        assertions: vec![(2, assert_color!("descendant matches", color == (0, 128, 0)))],
    });
}

#[test]
fn selector_child_combinator() {
    CssTestRunner::run(&CssTestCase {
        name: "child combinator >",
        css: ".parent > span { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let parent = doc.create_element("div");
            doc.add_class(parent, "parent");
            doc.append_child(root, parent);
            let direct_child = doc.create_element("span");
            doc.append_child(parent, direct_child);
            vec![parent, direct_child]
        }),
        assertions: vec![(1, assert_color!("child combinator", color == (0, 128, 0)))],
    });
}

#[test]
fn selector_multiple_classes() {
    CssTestRunner::run(&CssTestCase {
        name: "multiple class selector",
        css: ".a.b { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "a");
            doc.add_class(div, "b");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("multi-class match", color == (0, 128, 0)))],
    });
}

#[test]
fn selector_does_not_match() {
    CssTestRunner::run(&CssTestCase {
        name: "class selector does not match wrong class",
        css: ".special { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "normal");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "should not match .special",
                check: Box::new(|style| {
                    // Default color is black (0,0,0), not red
                    if style.color.r == 0 {
                        Ok(())
                    } else {
                        Err(format!("unexpected red color: r={}", style.color.r))
                    }
                }),
            },
        )],
    });
}

#[test]
fn selector_universal() {
    CssTestRunner::run(&CssTestCase {
        name: "universal selector",
        css: "* { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![div, span]
        }),
        assertions: vec![
            (
                0,
                assert_color!("universal matches div", color == (255, 0, 0)),
            ),
            (
                1,
                assert_color!("universal matches span", color == (255, 0, 0)),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. CONTAINMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn contain_strict() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: strict",
        css: "div { contain: strict; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain strict",
                check: Box::new(|style| {
                    let c = &style.contain;
                    if c.size && c.layout && c.style && c.paint {
                        Ok(())
                    } else {
                        Err(format!("expected strict containment, got {:?}", c))
                    }
                }),
            },
        )],
    });
}

#[test]
fn contain_content() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: content",
        css: "div { contain: content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain content",
                check: Box::new(|style| {
                    let c = &style.contain;
                    if !c.size && c.layout && c.style && c.paint {
                        Ok(())
                    } else {
                        Err(format!("expected content containment, got {:?}", c))
                    }
                }),
            },
        )],
    });
}

#[test]
fn contain_layout_only() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: layout",
        css: "div { contain: layout; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain layout only",
                check: Box::new(|style| {
                    if style.contain.layout {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected layout containment, got {:?}",
                            style.contain
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn contain_paint_only() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: paint",
        css: "div { contain: paint; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain paint only",
                check: Box::new(|style| {
                    if style.contain.paint {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected paint containment, got {:?}",
                            style.contain
                        ))
                    }
                }),
            },
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. CONTAINER QUERY TYPE TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn container_type_inline_size() {
    CssTestRunner::run(&CssTestCase {
        name: "container-type: inline-size",
        css: "div { container-type: inline-size; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "container-type",
                container_type == ContainerType::InlineSize
            ),
        )],
    });
}

#[test]
fn container_type_size() {
    CssTestRunner::run(&CssTestCase {
        name: "container-type: size",
        css: "div { container-type: size; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("container-type", container_type == ContainerType::Size),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. LOGICAL PROPERTIES TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn inline_size_px() {
    CssTestRunner::run(&CssTestCase {
        name: "inline-size: 200px",
        css: "div { inline-size: 200px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("inline-size", inline_size == Dimension::Px(200.0)),
        )],
    });
}

#[test]
fn block_size_px() {
    CssTestRunner::run(&CssTestCase {
        name: "block-size: 300px",
        css: "div { block-size: 300px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("block-size", block_size == Dimension::Px(300.0)),
        )],
    });
}

#[test]
fn margin_inline_start() {
    CssTestRunner::run(&CssTestCase {
        name: "margin-inline-start: 10px",
        css: "div { margin-inline-start: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "margin-inline-start",
                margin_inline_start == Dimension::Px(10.0)
            ),
        )],
    });
}

#[test]
fn margin_inline_end() {
    CssTestRunner::run(&CssTestCase {
        name: "margin-inline-end: 20px",
        css: "div { margin-inline-end: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "margin-inline-end",
                margin_inline_end == Dimension::Px(20.0)
            ),
        )],
    });
}

#[test]
fn padding_inline_start() {
    CssTestRunner::run(&CssTestCase {
        name: "padding-inline-start: 15px",
        css: "div { padding-inline-start: 15px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "padding-inline-start",
                padding_inline_start == Dimension::Px(15.0)
            ),
        )],
    });
}

#[test]
fn padding_block_start() {
    CssTestRunner::run(&CssTestCase {
        name: "padding-block-start: 8px",
        css: "div { padding-block-start: 8px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "padding-block-start",
                padding_block_start == Dimension::Px(8.0)
            ),
        )],
    });
}

#[test]
fn inset_inline_start() {
    CssTestRunner::run(&CssTestCase {
        name: "inset-inline-start: 5px",
        css: "div { position: relative; inset-inline-start: 5px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "inset-inline-start",
                inset_inline_start == Dimension::Px(5.0)
            ),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. TRANSFORM & EFFECTS TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn transform_translate() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: translate(10px, 20px)",
        css: "div { transform: translate(10px, 20px); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform translate",
                check: Box::new(|style| {
                    if !style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err("expected transform, got empty".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_scale() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: scale(2)",
        css: "div { transform: scale(2); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform scale",
                check: Box::new(|style| {
                    if !style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err("expected transform, got empty".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_rotate() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate(45deg)",
        css: "div { transform: rotate(45deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform rotate",
                check: Box::new(|style| {
                    if !style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err("expected transform, got empty".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_style_preserve_3d() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-style: preserve-3d",
        css: "div { transform-style: preserve-3d; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "transform-style",
                transform_style == TransformStyle::Preserve3d
            ),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. SHORTHAND EXPANSION TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn border_shorthand() {
    CssTestRunner::run(&CssTestCase {
        name: "border: 1px solid red",
        css: "div { border: 1px solid red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side_f32!("border-width-top", border_width.top == 1.0),
            ),
            (
                0,
                assert_side!(
                    "border-style-top",
                    border_style.top == BorderLineStyle::Solid
                ),
            ),
            (
                0,
                StyleAssertion {
                    description: "border-color-top red",
                    check: Box::new(|style| {
                        if style.border_color.top.r == 255 {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected red border, got {:?}",
                                style.border_color.top
                            ))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn margin_shorthand_two_values() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 10px 20px",
        css: "div { margin: 10px 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn margin_shorthand_three_values() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 10px 20px 30px",
        css: "div { margin: 10px 20px 30px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(30.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn margin_shorthand_four_values() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 1px 2px 3px 4px",
        css: "div { margin: 1px 2px 3px 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(1.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(2.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(3.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(4.0)),
            ),
        ],
    });
}

#[test]
fn padding_shorthand_two_values() {
    CssTestRunner::run(&CssTestCase {
        name: "padding: 5px 10px",
        css: "div { padding: 5px 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(5.0)),
            ),
            (
                0,
                assert_side!("padding-right", padding.right == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("padding-bottom", padding.bottom == Dimension::Px(5.0)),
            ),
            (
                0,
                assert_side!("padding-left", padding.left == Dimension::Px(10.0)),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. MULTI-PROPERTY COMPOUND TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_box_model_test() {
    CssTestRunner::run(&CssTestCase {
        name: "full box model",
        css: r#"
            .box {
                display: block;
                box-sizing: border-box;
                width: 300px;
                height: 200px;
                margin: 10px;
                padding: 20px;
                border-width: 1px;
                border-style: solid;
                border-color: #333333;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "box");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Block)),
            (
                0,
                assert_style!("box-sizing", box_sizing == BoxSizing::BorderBox),
            ),
            (0, assert_dimension!("width", width == Dimension::Px(300.0))),
            (
                0,
                assert_dimension!("height", height == Dimension::Px(200.0)),
            ),
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side_f32!("border-width-top", border_width.top == 1.0),
            ),
            (
                0,
                assert_side!(
                    "border-style-top",
                    border_style.top == BorderLineStyle::Solid
                ),
            ),
        ],
    });
}

#[test]
fn flex_layout_complete() {
    CssTestRunner::run(&CssTestCase {
        name: "complete flex layout",
        css: r#"
            .flex {
                display: flex;
                flex-direction: column;
                flex-wrap: wrap;
                justify-content: space-between;
                align-items: center;
                align-content: stretch;
                gap: 16px;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "flex");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Flex)),
            (
                0,
                assert_style!("flex-direction", flex_direction == FlexDirection::Column),
            ),
            (0, assert_style!("flex-wrap", flex_wrap == FlexWrap::Wrap)),
            (
                0,
                assert_style!(
                    "justify-content",
                    justify_content == JustifyContent::SpaceBetween
                ),
            ),
            (
                0,
                assert_style!("align-items", align_items == AlignItems::Center),
            ),
            (
                0,
                assert_style!("align-content", align_content == AlignContent::Stretch),
            ),
        ],
    });
}

#[test]
fn grid_layout_complete() {
    CssTestRunner::run(&CssTestCase {
        name: "complete grid layout",
        css: r#"
            .grid {
                display: grid;
                grid-template-columns: 1fr 2fr 1fr;
                grid-template-rows: 100px 200px;
                grid-auto-flow: row;
                gap: 8px;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "grid");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Grid)),
            (
                0,
                assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::Row),
            ),
            (
                0,
                StyleAssertion {
                    description: "grid-template-columns count",
                    check: Box::new(|style| {
                        if style.grid_template_columns.len() == 3 {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected 3 columns, got {}",
                                style.grid_template_columns.len()
                            ))
                        }
                    }),
                },
            ),
            (
                0,
                StyleAssertion {
                    description: "grid-template-rows count",
                    check: Box::new(|style| {
                        if style.grid_template_rows.len() == 2 {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected 2 rows, got {}",
                                style.grid_template_rows.len()
                            ))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn typography_complete() {
    CssTestRunner::run(&CssTestCase {
        name: "complete typography",
        css: r#"
            .text {
                color: #333333;
                font-size: 18px;
                font-weight: 600;
                font-style: italic;
                line-height: 1.5;
                letter-spacing: 1px;
                text-align: center;
                text-transform: uppercase;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "text");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style_f32!("font-size", font_size == 18.0)),
            (
                0,
                StyleAssertion {
                    description: "font-weight 600",
                    check: Box::new(|style| {
                        if style.font_weight == 600 {
                            Ok(())
                        } else {
                            Err(format!("expected 600, got {}", style.font_weight))
                        }
                    }),
                },
            ),
            (
                0,
                assert_style!("font-style", font_style == FontStyle::Italic),
            ),
            (
                0,
                assert_style!("line-height", line_height == LineHeight::Number(1.5)),
            ),
            (
                0,
                assert_style_f32!("letter-spacing", letter_spacing == 1.0),
            ),
            (
                0,
                assert_style!("text-align", text_align == TextAlign::Center),
            ),
            (
                0,
                assert_style!("text-transform", text_transform == TextTransform::Uppercase),
            ),
        ],
    });
}

#[test]
fn positioning_complete() {
    CssTestRunner::run(&CssTestCase {
        name: "complete positioning",
        css: r#"
            .positioned {
                position: absolute;
                top: 0;
                right: 0;
                bottom: 0;
                left: 0;
                z-index: 100;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.add_class(div, "positioned");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("position", position == Position::Absolute)),
            (0, assert_dimension!("top", top == Dimension::Px(0.0))),
            (0, assert_dimension!("right", right == Dimension::Px(0.0))),
            (0, assert_dimension!("bottom", bottom == Dimension::Px(0.0))),
            (0, assert_dimension!("left", left == Dimension::Px(0.0))),
            (0, assert_style!("z-index", z_index == Some(100))),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. DEEP INHERITANCE CHAIN TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn deep_inheritance_chain() {
    CssTestRunner::run(&CssTestCase {
        name: "deep 3-level inheritance",
        css: r#"
            .root { color: #ff0000; font-size: 20px; }
            .mid { font-size: 24px; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let r = doc.create_element("div");
            doc.add_class(r, "root");
            doc.append_child(root, r);
            let mid = doc.create_element("div");
            doc.add_class(mid, "mid");
            doc.append_child(r, mid);
            let leaf = doc.create_element("span");
            doc.append_child(mid, leaf);
            vec![r, mid, leaf]
        }),
        assertions: vec![
            (0, assert_color!("root color", color == (255, 0, 0))),
            (0, assert_style_f32!("root font-size", font_size == 20.0)),
            (1, assert_color!("mid inherits color", color == (255, 0, 0))),
            (
                1,
                assert_style_f32!("mid font-size override", font_size == 24.0),
            ),
            (
                2,
                assert_color!("leaf inherits color from root", color == (255, 0, 0)),
            ),
            (
                2,
                assert_style_f32!("leaf inherits font-size from mid", font_size == 24.0),
            ),
        ],
    });
}

#[test]
fn deep_inheritance_overridden_at_leaf() {
    CssTestRunner::run(&CssTestCase {
        name: "inheritance overridden at leaf",
        css: r#"
            .root { color: red; }
            .leaf { color: blue; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let r = doc.create_element("div");
            doc.add_class(r, "root");
            doc.append_child(root, r);
            let mid = doc.create_element("div");
            doc.append_child(r, mid);
            let leaf = doc.create_element("span");
            doc.add_class(leaf, "leaf");
            doc.append_child(mid, leaf);
            vec![r, mid, leaf]
        }),
        assertions: vec![
            (0, assert_color!("root red", color == (255, 0, 0))),
            (1, assert_color!("mid inherits red", color == (255, 0, 0))),
            (
                2,
                assert_color!("leaf overrides to blue", color == (0, 0, 255)),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn empty_stylesheet() {
    CssTestRunner::run(&CssTestCase {
        name: "empty stylesheet",
        css: "",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("default display", display == Display::Block),
        )],
    });
}

#[test]
fn no_matching_rules() {
    CssTestRunner::run(&CssTestCase {
        name: "no matching rules",
        css: "span { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!("default display", display == Display::Block),
            ),
            (
                0,
                assert_style!("default position", position == Position::Static),
            ),
        ],
    });
}

#[test]
fn multiple_elements_same_rule() {
    CssTestRunner::run(&CssTestCase {
        name: "multiple elements match same rule",
        css: ".item { color: green; display: flex; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let a = doc.create_element("div");
            doc.add_class(a, "item");
            doc.append_child(root, a);
            let b = doc.create_element("div");
            doc.add_class(b, "item");
            doc.append_child(root, b);
            let c = doc.create_element("div");
            doc.add_class(c, "item");
            doc.append_child(root, c);
            vec![a, b, c]
        }),
        assertions: vec![
            (0, assert_style!("a display", display == Display::Flex)),
            (0, assert_color!("a color", color == (0, 128, 0))),
            (1, assert_style!("b display", display == Display::Flex)),
            (1, assert_color!("b color", color == (0, 128, 0))),
            (2, assert_style!("c display", display == Display::Flex)),
            (2, assert_color!("c color", color == (0, 128, 0))),
        ],
    });
}

#[test]
fn default_values_are_sane() {
    CssTestRunner::run(&CssTestCase {
        name: "default computed values",
        css: "",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!("default display", display == Display::Block),
            ),
            (
                0,
                assert_style!("default position", position == Position::Static),
            ),
            (
                0,
                assert_style!("default box-sizing", box_sizing == BoxSizing::ContentBox),
            ),
            (
                0,
                assert_style!("default visibility", visibility == Visibility::Visible),
            ),
            (0, assert_style!("default float", float == Float::None)),
            (0, assert_style!("default clear", clear == Clear::None)),
            (0, assert_style_f32!("default opacity", opacity == 1.0)),
            (
                0,
                assert_style!(
                    "default flex-direction",
                    flex_direction == FlexDirection::Row
                ),
            ),
            (
                0,
                assert_style!("default flex-wrap", flex_wrap == FlexWrap::NoWrap),
            ),
        ],
    });
}

#[test]
fn zero_dimensions() {
    CssTestRunner::run(&CssTestCase {
        name: "zero dimensions",
        css: "div { width: 0; height: 0; margin: 0; padding: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_dimension!("width zero", width == Dimension::Px(0.0)),
            ),
            (
                0,
                assert_dimension!("height zero", height == Dimension::Px(0.0)),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. BATCH RUNNER TEST — validate harness itself
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn harness_batch_run() {
    let tests: Vec<CssTestCase> = vec![
        CssTestCase {
            name: "batch: display",
            css: "div { display: flex; }",
            build_dom: Box::new(|doc| {
                let root = doc.root();
                let div = doc.create_element("div");
                doc.append_child(root, div);
                vec![div]
            }),
            assertions: vec![(0, assert_style!("display", display == Display::Flex))],
        },
        CssTestCase {
            name: "batch: color",
            css: "div { color: red; }",
            build_dom: Box::new(|doc| {
                let root = doc.root();
                let div = doc.create_element("div");
                doc.append_child(root, div);
                vec![div]
            }),
            assertions: vec![(0, assert_color!("color", color == (255, 0, 0)))],
        },
        CssTestCase {
            name: "batch: position",
            css: "div { position: fixed; }",
            build_dom: Box::new(|doc| {
                let root = doc.root();
                let div = doc.create_element("div");
                doc.append_child(root, div);
                vec![div]
            }),
            assertions: vec![(0, assert_style!("position", position == Position::Fixed))],
        },
    ];

    for test in &tests {
        CssTestRunner::run(test);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. EXTENDED FLEXBOX TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flex_grow_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-grow: 0",
        css: ".item { flex-grow: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-grow", flex_grow == 0.0))],
    });
}

#[test]
fn flex_grow_fractional() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-grow: 0.5",
        css: ".item { flex-grow: 0.5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-grow", flex_grow == 0.5))],
    });
}

#[test]
fn flex_grow_large() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-grow: 10",
        css: ".item { flex-grow: 10; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-grow", flex_grow == 10.0))],
    });
}

#[test]
fn flex_shrink_one() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-shrink: 1 (default)",
        css: ".item { flex-shrink: 1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-shrink", flex_shrink == 1.0))],
    });
}

#[test]
fn flex_shrink_large() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-shrink: 5",
        css: ".item { flex-shrink: 5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-shrink", flex_shrink == 5.0))],
    });
}

#[test]
fn flex_basis_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: 50%",
        css: ".item { flex-basis: 50%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Percent(50.0)),
        )],
    });
}

#[test]
fn flex_basis_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: 0",
        css: ".item { flex-basis: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Px(0.0)),
        )],
    });
}

#[test]
fn flex_basis_content() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: content",
        css: ".item { flex-basis: content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Content),
        )],
    });
}

#[test]
fn order_negative() {
    CssTestRunner::run(&CssTestCase {
        name: "order: -1",
        css: ".item { order: -1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "order",
                check: Box::new(|style| {
                    if style.order == -1 {
                        Ok(())
                    } else {
                        Err(format!("expected -1, got {}", style.order))
                    }
                }),
            },
        )],
    });
}

#[test]
fn order_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "order: 0",
        css: ".item { order: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "order",
                check: Box::new(|style| {
                    if style.order == 0 {
                        Ok(())
                    } else {
                        Err(format!("expected 0, got {}", style.order))
                    }
                }),
            },
        )],
    });
}

#[test]
fn gap_row_and_column_separate() {
    CssTestRunner::run(&CssTestCase {
        name: "row-gap: 10px; column-gap: 20px",
        css: "div { display: flex; row-gap: 10px; column-gap: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_dimension!("row-gap", row_gap == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_dimension!("column-gap", column_gap == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn gap_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "gap: 5%",
        css: "div { display: flex; gap: 5%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "gap percent",
                check: Box::new(|style| {
                    if style.gap.width == Dimension::Percent(5.0) {
                        Ok(())
                    } else {
                        Err(format!("expected Percent(5.0), got {:?}", style.gap.width))
                    }
                }),
            },
        )],
    });
}

#[test]
fn align_items_baseline() {
    CssTestRunner::run(&CssTestCase {
        name: "align-items: baseline",
        css: "div { display: flex; align-items: baseline; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-items", align_items == AlignItems::Baseline),
        )],
    });
}

#[test]
fn align_self_flex_start() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: flex-start",
        css: ".item { align-self: flex-start; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::FlexStart),
        )],
    });
}

#[test]
fn align_self_flex_end() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: flex-end",
        css: ".item { align-self: flex-end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::FlexEnd),
        )],
    });
}

#[test]
fn align_self_stretch() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: stretch",
        css: ".item { align-self: stretch; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::Stretch),
        )],
    });
}

#[test]
fn align_self_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: auto",
        css: ".item { align-self: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::Auto),
        )],
    });
}

#[test]
fn align_self_baseline() {
    CssTestRunner::run(&CssTestCase {
        name: "align-self: baseline",
        css: ".item { align-self: baseline; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_style!("align-self", align_self == AlignSelf::Baseline),
        )],
    });
}

#[test]
fn align_content_flex_start() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: flex-start",
        css: "div { display: flex; flex-wrap: wrap; align-content: flex-start; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::FlexStart),
        )],
    });
}

#[test]
fn align_content_flex_end() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: flex-end",
        css: "div { display: flex; flex-wrap: wrap; align-content: flex-end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::FlexEnd),
        )],
    });
}

#[test]
fn align_content_space_around() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: space-around",
        css: "div { display: flex; flex-wrap: wrap; align-content: space-around; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::SpaceAround),
        )],
    });
}

#[test]
fn align_content_space_evenly() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: space-evenly",
        css: "div { display: flex; flex-wrap: wrap; align-content: space-evenly; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::SpaceEvenly),
        )],
    });
}

#[test]
fn align_content_stretch() {
    CssTestRunner::run(&CssTestCase {
        name: "align-content: stretch",
        css: "div { display: flex; flex-wrap: wrap; align-content: stretch; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("align-content", align_content == AlignContent::Stretch),
        )],
    });
}

#[test]
fn justify_content_flex_start() {
    CssTestRunner::run(&CssTestCase {
        name: "justify-content: flex-start",
        css: "div { display: flex; justify-content: flex-start; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "justify-content",
                justify_content == JustifyContent::FlexStart
            ),
        )],
    });
}

#[test]
fn flex_item_min_width() {
    CssTestRunner::run(&CssTestCase {
        name: "flex item with min-width",
        css: ".item { display: flex; flex-grow: 1; min-width: 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (0, assert_style_f32!("flex-grow", flex_grow == 1.0)),
            (
                0,
                assert_dimension!("min-width", min_width == Dimension::Px(100.0)),
            ),
        ],
    });
}

#[test]
fn flex_item_max_width() {
    CssTestRunner::run(&CssTestCase {
        name: "flex item with max-width",
        css: ".item { flex-grow: 1; max-width: 300px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (0, assert_style_f32!("flex-grow", flex_grow == 1.0)),
            (
                0,
                assert_dimension!("max-width", max_width == Dimension::Px(300.0)),
            ),
        ],
    });
}

#[test]
fn flex_wrap_with_gap() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-wrap with gap",
        css: ".container { display: flex; flex-wrap: wrap; gap: 12px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let c = doc.create_element("div");
            doc.add_class(c, "container");
            doc.append_child(root, c);
            vec![c]
        }),
        assertions: vec![
            (0, assert_style!("flex-wrap", flex_wrap == FlexWrap::Wrap)),
            (
                0,
                StyleAssertion {
                    description: "gap",
                    check: Box::new(|style| {
                        if style.gap.width == Dimension::Px(12.0)
                            && style.gap.height == Dimension::Px(12.0)
                        {
                            Ok(())
                        } else {
                            Err(format!("expected 12px gap, got {:?}", style.gap))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn flex_three_items_different_grow() {
    CssTestRunner::run(&CssTestCase {
        name: "three flex items with different flex-grow",
        css: r#"
            .container { display: flex; }
            .a { flex-grow: 1; }
            .b { flex-grow: 2; }
            .c { flex-grow: 3; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let container = doc.create_element("div");
            doc.add_class(container, "container");
            doc.append_child(root, container);
            let a = doc.create_element("div");
            doc.add_class(a, "a");
            doc.append_child(container, a);
            let b = doc.create_element("div");
            doc.add_class(b, "b");
            doc.append_child(container, b);
            let c = doc.create_element("div");
            doc.add_class(c, "c");
            doc.append_child(container, c);
            vec![container, a, b, c]
        }),
        assertions: vec![
            (1, assert_style_f32!("a flex-grow", flex_grow == 1.0)),
            (2, assert_style_f32!("b flex-grow", flex_grow == 2.0)),
            (3, assert_style_f32!("c flex-grow", flex_grow == 3.0)),
        ],
    });
}

#[test]
fn flex_column_reverse_with_items() {
    CssTestRunner::run(&CssTestCase {
        name: "flex column-reverse container",
        css: r#"
            .col { display: flex; flex-direction: column-reverse; align-items: stretch; }
            .item { flex-shrink: 0; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let col = doc.create_element("div");
            doc.add_class(col, "col");
            doc.append_child(root, col);
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(col, item);
            vec![col, item]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "flex-direction",
                    flex_direction == FlexDirection::ColumnReverse
                ),
            ),
            (
                0,
                assert_style!("align-items", align_items == AlignItems::Stretch),
            ),
            (1, assert_style_f32!("flex-shrink", flex_shrink == 0.0)),
        ],
    });
}

#[test]
fn flex_nowrap_default() {
    CssTestRunner::run(&CssTestCase {
        name: "default flex-wrap is nowrap",
        css: "div { display: flex; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("flex-wrap", flex_wrap == FlexWrap::NoWrap))],
    });
}

#[test]
fn flex_basis_rem() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: 5rem",
        css: ".item { flex-basis: 5rem; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Rem(5.0)),
        )],
    });
}

#[test]
fn flex_item_order_precedence() {
    CssTestRunner::run(&CssTestCase {
        name: "flex item order values",
        css: r#"
            .a { order: 2; }
            .b { order: -1; }
            .c { order: 0; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let a = doc.create_element("div");
            doc.add_class(a, "a");
            doc.append_child(root, a);
            let b = doc.create_element("div");
            doc.add_class(b, "b");
            doc.append_child(root, b);
            let c = doc.create_element("div");
            doc.add_class(c, "c");
            doc.append_child(root, c);
            vec![a, b, c]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "order a",
                    check: Box::new(|style| {
                        if style.order == 2 {
                            Ok(())
                        } else {
                            Err(format!("expected 2, got {}", style.order))
                        }
                    }),
                },
            ),
            (
                1,
                StyleAssertion {
                    description: "order b",
                    check: Box::new(|style| {
                        if style.order == -1 {
                            Ok(())
                        } else {
                            Err(format!("expected -1, got {}", style.order))
                        }
                    }),
                },
            ),
            (
                2,
                StyleAssertion {
                    description: "order c",
                    check: Box::new(|style| {
                        if style.order == 0 {
                            Ok(())
                        } else {
                            Err(format!("expected 0, got {}", style.order))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn flex_inline_flex_display() {
    CssTestRunner::run(&CssTestCase {
        name: "inline-flex with flex properties",
        css: ".box { display: inline-flex; flex-direction: row; gap: 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let b = doc.create_element("div");
            doc.add_class(b, "box");
            doc.append_child(root, b);
            vec![b]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::InlineFlex)),
            (
                0,
                assert_style!("flex-direction", flex_direction == FlexDirection::Row),
            ),
        ],
    });
}

#[test]
fn flex_grow_decimal_3_7() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-grow: 3.7",
        css: ".item { flex-grow: 3.7; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-grow", flex_grow == 3.7))],
    });
}

#[test]
fn flex_shrink_fractional() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-shrink: 0.3",
        css: ".item { flex-shrink: 0.3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(0, assert_style_f32!("flex-shrink", flex_shrink == 0.3))],
    });
}

#[test]
fn flex_basis_em() {
    CssTestRunner::run(&CssTestCase {
        name: "flex-basis: 3em",
        css: ".item { flex-basis: 3em; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            assert_dimension!("flex-basis", flex_basis == Dimension::Em(3.0)),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 22. EXTENDED GRID TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn grid_auto_flow_row_dense() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: row dense",
        css: "div { display: grid; grid-auto-flow: row dense; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::RowDense),
        )],
    });
}

#[test]
fn grid_auto_flow_column_dense() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: column dense",
        css: "div { display: grid; grid-auto-flow: column dense; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "grid-auto-flow",
                grid_auto_flow == GridAutoFlow::ColumnDense
            ),
        )],
    });
}

#[test]
fn grid_template_columns_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-columns: auto auto",
        css: "div { display: grid; grid-template-columns: auto auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-template-columns auto",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 2
                        && style.grid_template_columns[0] == TrackSize::Auto
                        && style.grid_template_columns[1] == TrackSize::Auto
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [auto, auto], got {:?}",
                            style.grid_template_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_template_columns_mixed() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-columns: 200px 1fr auto",
        css: "div { display: grid; grid-template-columns: 200px 1fr auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid mixed columns",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 3
                        && style.grid_template_columns[0] == TrackSize::Px(200.0)
                        && style.grid_template_columns[1] == TrackSize::Fr(1.0)
                        && style.grid_template_columns[2] == TrackSize::Auto
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [200px, 1fr, auto], got {:?}",
                            style.grid_template_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_template_rows_fr() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-rows: 1fr 1fr 1fr",
        css: "div { display: grid; grid-template-rows: 1fr 1fr 1fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid rows 3x1fr",
                check: Box::new(|style| {
                    if style.grid_template_rows.len() == 3
                        && style
                            .grid_template_rows
                            .iter()
                            .all(|t| *t == TrackSize::Fr(1.0))
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [1fr, 1fr, 1fr], got {:?}",
                            style.grid_template_rows
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_auto_columns_px() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-columns: 100px",
        css: "div { display: grid; grid-auto-columns: 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-auto-columns",
                check: Box::new(|style| {
                    if style.grid_auto_columns == TrackSize::Px(100.0) {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected Px(100), got {:?}",
                            style.grid_auto_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_auto_rows_fr() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-rows: 1fr",
        css: "div { display: grid; grid-auto-rows: 1fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-auto-rows",
                check: Box::new(|style| {
                    if style.grid_auto_rows == TrackSize::Fr(1.0) {
                        Ok(())
                    } else {
                        Err(format!("expected Fr(1.0), got {:?}", style.grid_auto_rows))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_column_start_end() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-column-start/end",
        css: ".item { grid-column-start: 2; grid-column-end: 4; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "grid-column-start",
                    check: Box::new(|style| {
                        if style.grid_column_start == GridLine::Line(2) {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected Line(2), got {:?}",
                                style.grid_column_start
                            ))
                        }
                    }),
                },
            ),
            (
                0,
                StyleAssertion {
                    description: "grid-column-end",
                    check: Box::new(|style| {
                        if style.grid_column_end == GridLine::Line(4) {
                            Ok(())
                        } else {
                            Err(format!("expected Line(4), got {:?}", style.grid_column_end))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn grid_row_start_end() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-row-start/end",
        css: ".item { grid-row-start: 1; grid-row-end: 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "grid-row-start",
                    check: Box::new(|style| {
                        if style.grid_row_start == GridLine::Line(1) {
                            Ok(())
                        } else {
                            Err(format!("expected Line(1), got {:?}", style.grid_row_start))
                        }
                    }),
                },
            ),
            (
                0,
                StyleAssertion {
                    description: "grid-row-end",
                    check: Box::new(|style| {
                        if style.grid_row_end == GridLine::Line(3) {
                            Ok(())
                        } else {
                            Err(format!("expected Line(3), got {:?}", style.grid_row_end))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn grid_column_span_3() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-column: span 3",
        css: ".item { grid-column: span 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-column span 3",
                check: Box::new(|style| {
                    if style.grid_column.start == GridLine::Span(3) {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected span 3, got {:?}",
                            style.grid_column.start
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_row_line_placement() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-row: 2 / 5",
        css: ".item { grid-row: 2 / 5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-row placement",
                check: Box::new(|style| {
                    if style.grid_row.start == GridLine::Line(2)
                        && style.grid_row.end == GridLine::Line(5)
                    {
                        Ok(())
                    } else {
                        Err(format!("expected 2/5, got {:?}", style.grid_row))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_gap_separate() {
    CssTestRunner::run(&CssTestCase {
        name: "grid row-gap and column-gap",
        css: "div { display: grid; row-gap: 8px; column-gap: 16px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_dimension!("row-gap", row_gap == Dimension::Px(8.0)),
            ),
            (
                0,
                assert_dimension!("column-gap", column_gap == Dimension::Px(16.0)),
            ),
        ],
    });
}

#[test]
fn grid_template_areas_basic() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-areas",
        css: r#"div { display: grid; grid-template-areas: "header header" "sidebar main"; }"#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-template-areas",
                check: Box::new(|style| {
                    if style.grid_template_areas.len() == 2 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 2 area rows, got {}",
                            style.grid_template_areas.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_inline_display() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline-grid with columns",
        css: "div { display: inline-grid; grid-template-columns: 1fr 1fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::InlineGrid)),
            (
                0,
                StyleAssertion {
                    description: "2 columns",
                    check: Box::new(|style| {
                        if style.grid_template_columns.len() == 2 {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected 2, got {}",
                                style.grid_template_columns.len()
                            ))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn grid_auto_rows_px() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-rows: 50px",
        css: "div { display: grid; grid-auto-rows: 50px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-auto-rows 50px",
                check: Box::new(|style| {
                    if style.grid_auto_rows == TrackSize::Px(50.0) {
                        Ok(())
                    } else {
                        Err(format!("expected Px(50), got {:?}", style.grid_auto_rows))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_auto_columns_fr() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-columns: 1fr",
        css: "div { display: grid; grid-auto-columns: 1fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-auto-columns fr",
                check: Box::new(|style| {
                    if style.grid_auto_columns == TrackSize::Fr(1.0) {
                        Ok(())
                    } else {
                        Err(format!("expected Fr(1), got {:?}", style.grid_auto_columns))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_template_columns_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-template-columns: 50% 50%",
        css: "div { display: grid; grid-template-columns: 50% 50%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid cols percent",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 2
                        && style.grid_template_columns[0] == TrackSize::Percent(50.0)
                        && style.grid_template_columns[1] == TrackSize::Percent(50.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected [50%, 50%], got {:?}",
                            style.grid_template_columns
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_column_negative_line() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-column-end: -1",
        css: ".item { grid-column-end: -1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "grid-column-end -1",
                check: Box::new(|style| {
                    if style.grid_column_end == GridLine::Line(-1) {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected Line(-1), got {:?}",
                            style.grid_column_end
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_five_column_layout() {
    CssTestRunner::run(&CssTestCase {
        name: "5-column grid",
        css: "div { display: grid; grid-template-columns: 100px 1fr 2fr 1fr 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "5 columns",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 5 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 5, got {}",
                            style.grid_template_columns.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn grid_item_with_alignment() {
    CssTestRunner::run(&CssTestCase {
        name: "grid item align-self + justify-self",
        css: ".item { align-self: center; justify-self: end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (
                0,
                assert_style!("align-self", align_self == AlignSelf::Center),
            ),
            (
                0,
                assert_style!("justify-self", justify_self == JustifySelf::End),
            ),
        ],
    });
}

#[test]
fn grid_container_justify_items() {
    CssTestRunner::run(&CssTestCase {
        name: "grid justify-items: center",
        css: "div { display: grid; justify-items: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("justify-items", justify_items == JustifyItems::Center),
        )],
    });
}

#[test]
fn grid_column_row_combined() {
    CssTestRunner::run(&CssTestCase {
        name: "grid item column and row placement",
        css: ".item { grid-column: 1 / 3; grid-row: 2 / 4; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (
                0,
                StyleAssertion {
                    description: "grid-column 1/3",
                    check: Box::new(|style| {
                        if style.grid_column.start == GridLine::Line(1)
                            && style.grid_column.end == GridLine::Line(3)
                        {
                            Ok(())
                        } else {
                            Err(format!("got {:?}", style.grid_column))
                        }
                    }),
                },
            ),
            (
                0,
                StyleAssertion {
                    description: "grid-row 2/4",
                    check: Box::new(|style| {
                        if style.grid_row.start == GridLine::Line(2)
                            && style.grid_row.end == GridLine::Line(4)
                        {
                            Ok(())
                        } else {
                            Err(format!("got {:?}", style.grid_row))
                        }
                    }),
                },
            ),
        ],
    });
}

#[test]
fn grid_single_column_fr() {
    CssTestRunner::run(&CssTestCase {
        name: "grid single 1fr column",
        css: "div { display: grid; grid-template-columns: 1fr; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "1 column",
                check: Box::new(|style| {
                    if style.grid_template_columns.len() == 1
                        && style.grid_template_columns[0] == TrackSize::Fr(1.0)
                    {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.grid_template_columns))
                    }
                }),
            },
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 23. EXTENDED TRANSFORM TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn transform_skew() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: skew(10deg, 20deg)",
        css: "div { transform: skew(10deg, 20deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform skew",
                check: Box::new(|style| {
                    if !style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err("expected transform, got empty".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_matrix() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: matrix(1,0,0,1,0,0)",
        css: "div { transform: matrix(1, 0, 0, 1, 0, 0); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform matrix identity",
                check: Box::new(|style| {
                    if !style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err("expected transform, got empty".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_translate3d() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: translate3d(10px, 20px, 30px)",
        css: "div { transform: translate3d(10px, 20px, 30px); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform translate3d",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Translate3d(x, y, z)
                                if (x.resolve(0.0) - 10.0).abs() < 0.01
                                    && (y.resolve(0.0) - 20.0).abs() < 0.01
                                    && (*z - 30.0).abs() < 0.01 =>
                            {
                                Ok(())
                            }
                            other => {
                                Err(format!("expected translate3d(10,20,30), got {:?}", other))
                            }
                        }
                    } else {
                        Err(format!(
                            "expected 1 transform, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_rotate3d() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate3d(1, 0, 0, 45deg)",
        css: "div { transform: rotate3d(1, 0, 0, 45deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform rotate3d",
                check: Box::new(|style| {
                    if !style.transform.is_empty() && style.transform[0].is_3d() {
                        Ok(())
                    } else {
                        Err(format!("expected 3d transform, got {:?}", style.transform))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_scale3d() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: scale3d(2, 2, 2)",
        css: "div { transform: scale3d(2, 2, 2); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform scale3d",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Scale3d(x, y, z)
                                if (*x - 2.0).abs() < 0.01
                                    && (*y - 2.0).abs() < 0.01
                                    && (*z - 2.0).abs() < 0.01 =>
                            {
                                Ok(())
                            }
                            other => Err(format!("expected scale3d(2,2,2), got {:?}", other)),
                        }
                    } else {
                        Err(format!(
                            "expected 1 transform, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_perspective_fn() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: perspective(500px)",
        css: "div { transform: perspective(500px); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform perspective fn",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::PerspectiveFn(v) if (*v - 500.0).abs() < 0.01 => Ok(()),
                            other => Err(format!("expected perspective(500), got {:?}", other)),
                        }
                    } else {
                        Err(format!(
                            "expected 1 transform, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_multiple() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate(45deg) scale(2)",
        css: "div { transform: rotate(45deg) scale(2); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "multiple transforms",
                check: Box::new(|style| {
                    if style.transform.len() == 2 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 2 transforms, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_translate_values() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: translate(50px, 100px) exact values",
        css: "div { transform: translate(50px, 100px); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "translate values",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Translate(x, y)
                                if (*x - 50.0).abs() < 0.01 && (*y - 100.0).abs() < 0.01 =>
                            {
                                Ok(())
                            }
                            other => Err(format!("expected translate(50,100), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_scale_xy() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: scale(1.5, 2.0)",
        css: "div { transform: scale(1.5, 2.0); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "scale values",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Scale(x, y)
                                if (*x - 1.5).abs() < 0.01 && (*y - 2.0).abs() < 0.01 =>
                            {
                                Ok(())
                            }
                            other => Err(format!("expected scale(1.5,2), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_rotate_value() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate(90deg) value",
        css: "div { transform: rotate(90deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "rotate 90deg",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Rotate(v) if (*v - 90.0).abs() < 0.01 => Ok(()),
                            other => Err(format!("expected rotate(90), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_origin_center() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-origin: center center (default)",
        css: "div { transform: rotate(45deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform-origin default 50% 50%",
                check: Box::new(|style| {
                    if style.transform_origin.x == Dimension::Percent(50.0)
                        && style.transform_origin.y == Dimension::Percent(50.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 50%/50%, got {:?}",
                            style.transform_origin
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_origin_top_left() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-origin: top left",
        css: "div { transform-origin: top left; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform-origin top left",
                check: Box::new(|style| {
                    if style.transform_origin.x == Dimension::Percent(0.0)
                        && style.transform_origin.y == Dimension::Percent(0.0)
                    {
                        Ok(())
                    } else {
                        Err(format!("expected 0%/0%, got {:?}", style.transform_origin))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_origin_bottom_right() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-origin: bottom right",
        css: "div { transform-origin: bottom right; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform-origin bottom right",
                check: Box::new(|style| {
                    if style.transform_origin.x == Dimension::Percent(100.0)
                        && style.transform_origin.y == Dimension::Percent(100.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 100%/100%, got {:?}",
                            style.transform_origin
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_origin_px() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-origin: 10px 20px",
        css: "div { transform-origin: 10px 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform-origin px values",
                check: Box::new(|style| {
                    if style.transform_origin.x == Dimension::Px(10.0)
                        && style.transform_origin.y == Dimension::Px(20.0)
                    {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 10px/20px, got {:?}",
                            style.transform_origin
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn perspective_length() {
    CssTestRunner::run(&CssTestCase {
        name: "perspective: 800px",
        css: "div { perspective: 800px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "perspective 800px",
                check: Box::new(|style| {
                    if style.perspective == Perspective::Length(800.0) {
                        Ok(())
                    } else {
                        Err(format!("expected Length(800), got {:?}", style.perspective))
                    }
                }),
            },
        )],
    });
}

#[test]
fn perspective_none() {
    CssTestRunner::run(&CssTestCase {
        name: "perspective: none",
        css: "div { perspective: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "perspective none",
                check: Box::new(|style| {
                    if style.perspective == Perspective::None {
                        Ok(())
                    } else {
                        Err(format!("expected None, got {:?}", style.perspective))
                    }
                }),
            },
        )],
    });
}

#[test]
fn backface_visibility_visible() {
    CssTestRunner::run(&CssTestCase {
        name: "backface-visibility: visible",
        css: "div { backface-visibility: visible; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "backface-visibility",
                backface_visibility == BackfaceVisibility::Visible
            ),
        )],
    });
}

#[test]
fn transform_style_flat() {
    CssTestRunner::run(&CssTestCase {
        name: "transform-style: flat",
        css: "div { transform-style: flat; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("transform-style", transform_style == TransformStyle::Flat),
        )],
    });
}

#[test]
fn transform_none() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: none",
        css: "div { transform: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transform none",
                check: Box::new(|style| {
                    if style.transform.is_empty() {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected empty, got {} transforms",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_skew_single_axis() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: skew(30deg)",
        css: "div { transform: skew(30deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "skew single",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 1 transform, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_three_functions() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: translate(10px, 10px) rotate(45deg) scale(1.5)",
        css: "div { transform: translate(10px, 10px) rotate(45deg) scale(1.5); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "three transforms",
                check: Box::new(|style| {
                    if style.transform.len() == 3 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 3 transforms, got {}",
                            style.transform.len()
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_scale_uniform() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: scale(3)",
        css: "div { transform: scale(3); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "scale uniform",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Scale(x, y)
                                if (*x - 3.0).abs() < 0.01 && (*y - 3.0).abs() < 0.01 =>
                            {
                                Ok(())
                            }
                            other => Err(format!("expected scale(3,3), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_translate_single_value() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: translate(25px)",
        css: "div { transform: translate(25px); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "translate single",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Translate(x, _) if (*x - 25.0).abs() < 0.01 => Ok(()),
                            other => Err(format!("expected translate(25, 0), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_rotate_negative() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate(-45deg)",
        css: "div { transform: rotate(-45deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "rotate negative",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Rotate(v) if (*v - (-45.0)).abs() < 0.01 => Ok(()),
                            other => Err(format!("expected rotate(-45), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transform_rotate_360() {
    CssTestRunner::run(&CssTestCase {
        name: "transform: rotate(360deg)",
        css: "div { transform: rotate(360deg); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "rotate 360",
                check: Box::new(|style| {
                    if style.transform.len() == 1 {
                        match &style.transform[0] {
                            Transform::Rotate(v) if (*v - 360.0).abs() < 0.01 => Ok(()),
                            other => Err(format!("expected rotate(360), got {:?}", other)),
                        }
                    } else {
                        Err(format!("expected 1, got {}", style.transform.len()))
                    }
                }),
            },
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 24. ANIMATION & TRANSITION TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn transition_property_all() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-property: all",
        css: "div { transition-property: all; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-property",
                check: Box::new(|style| {
                    if style.transition_property.as_deref() == Some("all") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'all', got {:?}",
                            style.transition_property
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_duration_ms() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-duration: 300ms",
        css: "div { transition-duration: 300ms; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-duration",
                check: Box::new(|style| {
                    if style.transition_duration.as_deref() == Some("300ms") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected '300ms', got {:?}",
                            style.transition_duration
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_timing_function_ease() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-timing-function: ease",
        css: "div { transition-timing-function: ease; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-timing-function",
                check: Box::new(|style| {
                    if style.transition_timing_function.as_deref() == Some("ease") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'ease', got {:?}",
                            style.transition_timing_function
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_delay_s() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-delay: 0.5s",
        css: "div { transition-delay: 0.5s; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-delay",
                check: Box::new(|style| {
                    if style.transition_delay.as_deref() == Some("0.5s") {
                        Ok(())
                    } else {
                        Err(format!("expected '0.5s', got {:?}", style.transition_delay))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_shorthand() {
    CssTestRunner::run(&CssTestCase {
        name: "transition: opacity 200ms ease-in",
        css: "div { transition: opacity 200ms ease-in; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition shorthand parsed",
                check: Box::new(|style| {
                    if !style.transition.is_empty() {
                        let t = &style.transition[0];
                        if t.property == "opacity"
                            && (t.duration_ms - 200.0).abs() < 0.01
                            && t.timing_function == TimingFunction::EaseIn
                        {
                            Ok(())
                        } else {
                            Err(format!("got {:?}", t))
                        }
                    } else {
                        Err("empty transitions".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_shorthand_with_delay() {
    CssTestRunner::run(&CssTestCase {
        name: "transition: color 500ms ease-out 100ms",
        css: "div { transition: color 500ms ease-out 100ms; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition with delay",
                check: Box::new(|style| {
                    if !style.transition.is_empty() {
                        let t = &style.transition[0];
                        if t.property == "color"
                            && (t.duration_ms - 500.0).abs() < 0.01
                            && (t.delay_ms - 100.0).abs() < 0.01
                        {
                            Ok(())
                        } else {
                            Err(format!("got {:?}", t))
                        }
                    } else {
                        Err("empty transitions".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_timing_linear() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-timing-function: linear",
        css: "div { transition-timing-function: linear; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "timing linear",
                check: Box::new(|style| {
                    if style.transition_timing_function.as_deref() == Some("linear") {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.transition_timing_function))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_timing_ease_in_out() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-timing-function: ease-in-out",
        css: "div { transition-timing-function: ease-in-out; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "timing ease-in-out",
                check: Box::new(|style| {
                    if style.transition_timing_function.as_deref() == Some("ease-in-out") {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.transition_timing_function))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_name() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-name: fadeIn",
        css: "div { animation-name: fadeIn; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-name",
                check: Box::new(|style| {
                    if style.animation_name.as_deref() == Some("fadeIn") {
                        Ok(())
                    } else {
                        Err(format!("expected 'fadeIn', got {:?}", style.animation_name))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_duration() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-duration: 1s",
        css: "div { animation-duration: 1s; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-duration",
                check: Box::new(|style| {
                    if style.animation_duration.as_deref() == Some("1s") {
                        Ok(())
                    } else {
                        Err(format!("expected '1s', got {:?}", style.animation_duration))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_iteration_count_infinite() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-iteration-count: infinite",
        css: "div { animation-iteration-count: infinite; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-iteration-count",
                check: Box::new(|style| {
                    if style.animation_iteration_count == AnimationIterationCount::Infinite {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected Infinite, got {:?}",
                            style.animation_iteration_count
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_iteration_count_finite() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-iteration-count: 3",
        css: "div { animation-iteration-count: 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-iteration-count 3",
                check: Box::new(|style| {
                    if style.animation_iteration_count == AnimationIterationCount::Finite(3.0) {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected Finite(3), got {:?}",
                            style.animation_iteration_count
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_direction_reverse() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-direction: reverse",
        css: "div { animation-direction: reverse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-direction",
                animation_direction == AnimationDirection::Reverse
            ),
        )],
    });
}

#[test]
fn animation_direction_alternate() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-direction: alternate",
        css: "div { animation-direction: alternate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-direction",
                animation_direction == AnimationDirection::Alternate
            ),
        )],
    });
}

#[test]
fn animation_direction_alternate_reverse() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-direction: alternate-reverse",
        css: "div { animation-direction: alternate-reverse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-direction",
                animation_direction == AnimationDirection::AlternateReverse
            ),
        )],
    });
}

#[test]
fn animation_fill_mode_forwards() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-fill-mode: forwards",
        css: "div { animation-fill-mode: forwards; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-fill-mode",
                animation_fill_mode == AnimationFillMode::Forwards
            ),
        )],
    });
}

#[test]
fn animation_fill_mode_backwards() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-fill-mode: backwards",
        css: "div { animation-fill-mode: backwards; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-fill-mode",
                animation_fill_mode == AnimationFillMode::Backwards
            ),
        )],
    });
}

#[test]
fn animation_fill_mode_both() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-fill-mode: both",
        css: "div { animation-fill-mode: both; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-fill-mode",
                animation_fill_mode == AnimationFillMode::Both
            ),
        )],
    });
}

#[test]
fn animation_fill_mode_none() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-fill-mode: none",
        css: "div { animation-fill-mode: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-fill-mode",
                animation_fill_mode == AnimationFillMode::None
            ),
        )],
    });
}

#[test]
fn animation_play_state_paused() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-play-state: paused",
        css: "div { animation-play-state: paused; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-play-state",
                animation_play_state == AnimationPlayState::Paused
            ),
        )],
    });
}

#[test]
fn animation_play_state_running() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-play-state: running",
        css: "div { animation-play-state: running; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-play-state",
                animation_play_state == AnimationPlayState::Running
            ),
        )],
    });
}

#[test]
fn animation_delay() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-delay: 500ms",
        css: "div { animation-delay: 500ms; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-delay",
                check: Box::new(|style| {
                    if style.animation_delay.as_deref() == Some("500ms") {
                        Ok(())
                    } else {
                        Err(format!("expected '500ms', got {:?}", style.animation_delay))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_timing_function() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-timing-function: ease-in",
        css: "div { animation-timing-function: ease-in; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation-timing-function",
                check: Box::new(|style| {
                    if style.animation_timing_function.as_deref() == Some("ease-in") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'ease-in', got {:?}",
                            style.animation_timing_function
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_shorthand() {
    CssTestRunner::run(&CssTestCase {
        name: "animation: slide 1s ease-in-out 200ms infinite alternate",
        css: "div { animation: slide 1s ease-in-out 200ms infinite alternate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "animation shorthand",
                check: Box::new(|style| {
                    if !style.animation.is_empty() {
                        let a = &style.animation[0];
                        if a.name == "slide"
                            && (a.duration_ms - 1000.0).abs() < 0.01
                            && a.timing_function == TimingFunction::EaseInOut
                            && a.iteration_count == AnimationIterationCount::Infinite
                            && a.direction == AnimationDirection::Alternate
                        {
                            Ok(())
                        } else {
                            Err(format!("got {:?}", a))
                        }
                    } else {
                        Err("empty animation".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_shorthand_simple() {
    CssTestRunner::run(&CssTestCase {
        name: "animation: fadeIn 300ms",
        css: "div { animation: fadeIn 300ms; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "simple animation shorthand",
                check: Box::new(|style| {
                    if !style.animation.is_empty()
                        && style.animation[0].name == "fadeIn"
                        && (style.animation[0].duration_ms - 300.0).abs() < 0.01
                    {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.animation))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_property_opacity() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-property: opacity",
        css: "div { transition-property: opacity; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-property opacity",
                check: Box::new(|style| {
                    if style.transition_property.as_deref() == Some("opacity") {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.transition_property))
                    }
                }),
            },
        )],
    });
}

#[test]
fn transition_duration_seconds() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-duration: 1s",
        css: "div { transition-duration: 1s; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "transition-duration 1s",
                check: Box::new(|style| {
                    if style.transition_duration.as_deref() == Some("1s") {
                        Ok(())
                    } else {
                        Err(format!("got {:?}", style.transition_duration))
                    }
                }),
            },
        )],
    });
}

#[test]
fn animation_direction_normal() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-direction: normal",
        css: "div { animation-direction: normal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-direction",
                animation_direction == AnimationDirection::Normal
            ),
        )],
    });
}

#[test]
fn animation_composition_add() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-composition: add",
        css: "div { animation-composition: add; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-composition",
                animation_composition == AnimationComposition::Add
            ),
        )],
    });
}

#[test]
fn animation_composition_accumulate() {
    CssTestRunner::run(&CssTestCase {
        name: "animation-composition: accumulate",
        css: "div { animation-composition: accumulate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "animation-composition",
                animation_composition == AnimationComposition::Accumulate
            ),
        )],
    });
}

#[test]
fn transition_behavior_allow_discrete() {
    CssTestRunner::run(&CssTestCase {
        name: "transition-behavior: allow-discrete",
        css: "div { transition-behavior: allow-discrete; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "transition-behavior",
                transition_behavior == TransitionBehavior::AllowDiscrete
            ),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 25. EXTENDED MEDIA QUERY & @SUPPORTS & @CONTAINER TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn media_query_min_width() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(1024.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-width: 800px) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_max_width() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(600.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (max-width: 800px) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
}

#[test]
fn media_query_min_width_not_matching() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(600.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-width: 800px) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.r, 255);
}

#[test]
fn media_query_min_height() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_height(900.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-height: 600px) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_max_height() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_height(400.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (max-height: 600px) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
}

#[test]
fn media_query_orientation_landscape() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(1024.0);
    engine.set_viewport_height(768.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (orientation: landscape) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_orientation_portrait() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(768.0);
    engine.set_viewport_height(1024.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (orientation: portrait) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
}

#[test]
fn media_query_and_compound() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(1024.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-width: 800px) and (max-width: 1200px) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_not() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(1024.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media not (max-width: 600px) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_prefers_reduced_motion() {
    let mut engine = StyleEngine::default();
    engine.set_prefers_reduced_motion(true);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (prefers-reduced-motion: reduce) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
}

#[test]
fn media_query_prefers_contrast_high() {
    let mut engine = StyleEngine::default();
    engine.set_prefers_contrast("high");
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (prefers-contrast: high) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_hover_hover() {
    let mut engine = StyleEngine::default();
    engine.set_hover_available(true);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (hover: hover) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_pointer_fine() {
    let mut engine = StyleEngine::default();
    engine.set_pointer_type("fine");
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (pointer: fine) { div { color: blue; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.b, 255);
}

#[test]
fn supports_opacity() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (opacity: 0.5)",
        css: r#"
            div { color: red; }
            @supports (opacity: 0.5) { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports opacity", color == (0, 128, 0)))],
    });
}

#[test]
fn supports_transform() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (transform: rotate(45deg))",
        css: r#"
            div { color: red; }
            @supports (transform: rotate(45deg)) { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_color!("@supports transform", color == (0, 128, 0)),
        )],
    });
}

#[test]
fn supports_not() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports not (nonexistent: value)",
        css: r#"
            div { color: red; }
            @supports not (nonexistent: value) { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports not", color == (0, 128, 0)))],
    });
}

#[test]
fn supports_and() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (display: flex) and (gap: 10px)",
        css: r#"
            div { color: red; }
            @supports (display: flex) and (gap: 10px) { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports and", color == (0, 128, 0)))],
    });
}

#[test]
fn supports_or() {
    CssTestRunner::run(&CssTestCase {
        name: "@supports (display: flex) or (display: grid)",
        css: r#"
            div { color: red; }
            @supports (display: flex) or (display: grid) { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("@supports or", color == (0, 128, 0)))],
    });
}

#[test]
fn container_type_normal() {
    CssTestRunner::run(&CssTestCase {
        name: "container-type: normal",
        css: "div { container-type: normal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("container-type", container_type == ContainerType::Normal),
        )],
    });
}

#[test]
fn container_name_value() {
    CssTestRunner::run(&CssTestCase {
        name: "container-name: sidebar",
        css: "div { container-name: sidebar; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "container-name",
                check: Box::new(|style| {
                    if style.container_name.as_deref() == Some("sidebar") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'sidebar', got {:?}",
                            style.container_name
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn media_query_display_mode_standalone() {
    let mut engine = StyleEngine::default();
    engine.set_display_mode("standalone");
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (display-mode: standalone) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_aspect_ratio() {
    let mut engine = StyleEngine::default();
    engine.set_viewport_width(1920.0);
    engine.set_viewport_height(1080.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-aspect-ratio: 16/9) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_resolution() {
    let mut engine = StyleEngine::default();
    engine.set_resolution_dpi(192.0);
    engine.add_stylesheet(
        r#"
        div { color: red; }
        @media (min-resolution: 2dppx) { div { color: green; } }
    "#,
    );
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);
    let map = engine.restyle_all(&doc);
    let style = map.get(div).unwrap();
    assert_eq!(style.color.g, 128);
}

#[test]
fn media_query_screen() {
    CssTestRunner::run(&CssTestCase {
        name: "@media screen",
        css: r#"
            div { color: red; }
            @media screen { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("media screen", color == (0, 128, 0)))],
    });
}

#[test]
fn media_query_all() {
    CssTestRunner::run(&CssTestCase {
        name: "@media all",
        css: r#"
            div { color: red; }
            @media all { div { color: green; } }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("media all", color == (0, 128, 0)))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 26. MISCELLANEOUS TESTS (50+)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn aspect_ratio_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "aspect-ratio: auto",
        css: "div { aspect-ratio: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("aspect-ratio", aspect_ratio == AspectRatio::Auto),
        )],
    });
}

#[test]
fn aspect_ratio_value() {
    CssTestRunner::run(&CssTestCase {
        name: "aspect-ratio: 16 / 9",
        css: "div { aspect-ratio: 16 / 9; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "aspect-ratio",
                check: Box::new(|style| match &style.aspect_ratio {
                    AspectRatio::Ratio(w, h)
                        if (*w - 16.0).abs() < 0.01 && (*h - 9.0).abs() < 0.01 =>
                    {
                        Ok(())
                    }
                    other => Err(format!("expected 16/9, got {:?}", other)),
                }),
            },
        )],
    });
}

#[test]
fn column_count_value() {
    CssTestRunner::run(&CssTestCase {
        name: "column-count: 3",
        css: "div { column-count: 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "column-count",
                check: Box::new(|style| {
                    if style.column_count == Some(3) {
                        Ok(())
                    } else {
                        Err(format!("expected Some(3), got {:?}", style.column_count))
                    }
                }),
            },
        )],
    });
}

#[test]
fn column_width_px() {
    CssTestRunner::run(&CssTestCase {
        name: "column-width: 200px",
        css: "div { column-width: 200px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("column-width", column_width == Dimension::Px(200.0)),
        )],
    });
}

#[test]
fn column_gap_px() {
    CssTestRunner::run(&CssTestCase {
        name: "column-gap: 20px",
        css: "div { column-gap: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("column-gap", column_gap == Dimension::Px(20.0)),
        )],
    });
}

#[test]
fn object_fit_fill() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: fill",
        css: "img { object-fit: fill; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::Fill),
        )],
    });
}

#[test]
fn object_fit_none() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: none",
        css: "img { object-fit: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::None),
        )],
    });
}

#[test]
fn object_fit_scale_down() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: scale-down",
        css: "img { object-fit: scale-down; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::ScaleDown),
        )],
    });
}

#[test]
fn white_space_nowrap() {
    CssTestRunner::run(&CssTestCase {
        name: "white-space: nowrap",
        css: "div { white-space: nowrap; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("white-space", white_space == WhiteSpace::NoWrap),
        )],
    });
}

#[test]
fn white_space_pre() {
    CssTestRunner::run(&CssTestCase {
        name: "white-space: pre",
        css: "div { white-space: pre; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("white-space", white_space == WhiteSpace::Pre),
        )],
    });
}

#[test]
fn white_space_pre_wrap() {
    CssTestRunner::run(&CssTestCase {
        name: "white-space: pre-wrap",
        css: "div { white-space: pre-wrap; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("white-space", white_space == WhiteSpace::PreWrap),
        )],
    });
}

#[test]
fn white_space_pre_line() {
    CssTestRunner::run(&CssTestCase {
        name: "white-space: pre-line",
        css: "div { white-space: pre-line; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("white-space", white_space == WhiteSpace::PreLine),
        )],
    });
}

#[test]
fn word_break_break_all() {
    CssTestRunner::run(&CssTestCase {
        name: "word-break: break-all",
        css: "div { word-break: break-all; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("word-break", word_break == WordBreak::BreakAll),
        )],
    });
}

#[test]
fn word_break_keep_all() {
    CssTestRunner::run(&CssTestCase {
        name: "word-break: keep-all",
        css: "div { word-break: keep-all; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("word-break", word_break == WordBreak::KeepAll),
        )],
    });
}

#[test]
fn overflow_wrap_anywhere() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow-wrap: anywhere",
        css: "div { overflow-wrap: anywhere; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("overflow-wrap", overflow_wrap == OverflowWrap::Anywhere),
        )],
    });
}

#[test]
fn overflow_wrap_break_word() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow-wrap: break-word",
        css: "div { overflow-wrap: break-word; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("overflow-wrap", overflow_wrap == OverflowWrap::BreakWord),
        )],
    });
}

#[test]
fn cursor_grab() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: grab",
        css: "div { cursor: grab; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Grab))],
    });
}

#[test]
fn cursor_crosshair() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: crosshair",
        css: "div { cursor: crosshair; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Crosshair))],
    });
}

#[test]
fn cursor_move() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: move",
        css: "div { cursor: move; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Move))],
    });
}

#[test]
fn cursor_wait() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: wait",
        css: "div { cursor: wait; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Wait))],
    });
}

#[test]
fn resize_horizontal() {
    CssTestRunner::run(&CssTestCase {
        name: "resize: horizontal",
        css: "div { resize: horizontal; overflow: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("resize", resize == Resize::Horizontal))],
    });
}

#[test]
fn resize_vertical() {
    CssTestRunner::run(&CssTestCase {
        name: "resize: vertical",
        css: "div { resize: vertical; overflow: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("resize", resize == Resize::Vertical))],
    });
}

#[test]
fn resize_none() {
    CssTestRunner::run(&CssTestCase {
        name: "resize: none",
        css: "div { resize: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("resize", resize == Resize::None))],
    });
}

#[test]
fn user_select_text() {
    CssTestRunner::run(&CssTestCase {
        name: "user-select: text",
        css: "div { user-select: text; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("user-select", user_select == UserSelect::Text),
        )],
    });
}

#[test]
fn user_select_all() {
    CssTestRunner::run(&CssTestCase {
        name: "user-select: all",
        css: "div { user-select: all; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("user-select", user_select == UserSelect::All),
        )],
    });
}

#[test]
fn scroll_behavior_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "scroll-behavior: auto",
        css: "div { scroll-behavior: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("scroll-behavior", scroll_behavior == ScrollBehavior::Auto),
        )],
    });
}

#[test]
fn content_visibility_visible() {
    CssTestRunner::run(&CssTestCase {
        name: "content-visibility: visible",
        css: "div { content-visibility: visible; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "content-visibility",
                content_visibility == ContentVisibility::Visible
            ),
        )],
    });
}

#[test]
fn border_collapse_separate() {
    CssTestRunner::run(&CssTestCase {
        name: "border-collapse: separate",
        css: "table { border-collapse: separate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let table = doc.create_element("table");
            doc.append_child(root, table);
            vec![table]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "border-collapse",
                border_collapse == BorderCollapse::Separate
            ),
        )],
    });
}

#[test]
fn list_style_type_decimal() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-type: decimal",
        css: "li { list-style-type: decimal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!("list-style-type", list_style_type == ListStyleType::Decimal),
        )],
    });
}

#[test]
fn list_style_type_circle() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-type: circle",
        css: "li { list-style-type: circle; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!("list-style-type", list_style_type == ListStyleType::Circle),
        )],
    });
}

#[test]
fn list_style_type_square() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-type: square",
        css: "li { list-style-type: square; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!("list-style-type", list_style_type == ListStyleType::Square),
        )],
    });
}

#[test]
fn list_style_position_outside() {
    CssTestRunner::run(&CssTestCase {
        name: "list-style-position: outside",
        css: "li { list-style-position: outside; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let li = doc.create_element("li");
            doc.append_child(root, li);
            vec![li]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "list-style-position",
                list_style_position == ListStylePosition::Outside
            ),
        )],
    });
}

#[test]
fn color_hsl() {
    CssTestRunner::run(&CssTestCase {
        name: "color: hsl(0, 100%, 50%)",
        css: "div { color: hsl(0, 100%, 50%); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("hsl red", color == (255, 0, 0)))],
    });
}

#[test]
fn color_hex_8digit_alpha() {
    CssTestRunner::run(&CssTestCase {
        name: "color: #ff000080 (50% alpha)",
        css: "div { color: #ff000080; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "8-digit hex color",
                check: Box::new(|style| {
                    if style.color.r == 255
                        && style.color.g == 0
                        && style.color.b == 0
                        && style.color.a == 128
                    {
                        Ok(())
                    } else {
                        Err(format!("expected rgba(255,0,0,128), got {:?}", style.color))
                    }
                }),
            },
        )],
    });
}

#[test]
fn color_rgba_function() {
    CssTestRunner::run(&CssTestCase {
        name: "color: rgba(0, 0, 255, 0.5)",
        css: "div { color: rgba(0, 0, 255, 0.5); }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "rgba color",
                check: Box::new(|style| {
                    if style.color.r == 0 && style.color.b == 255 && style.color.a == 128 {
                        Ok(())
                    } else {
                        Err(format!("expected rgba(0,0,255,128), got {:?}", style.color))
                    }
                }),
            },
        )],
    });
}

#[test]
fn border_radius_individual() {
    CssTestRunner::run(&CssTestCase {
        name: "border-radius individual corners",
        css: "div { border-top-left-radius: 4px; border-top-right-radius: 8px; border-bottom-right-radius: 12px; border-bottom-left-radius: 16px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_corner_f32!("border-top-left-radius", border_radius.top_left == 4.0),
            ),
            (
                0,
                assert_corner_f32!("border-top-right-radius", border_radius.top_right == 8.0),
            ),
            (
                0,
                assert_corner_f32!(
                    "border-bottom-right-radius",
                    border_radius.bottom_right == 12.0
                ),
            ),
            (
                0,
                assert_corner_f32!(
                    "border-bottom-left-radius",
                    border_radius.bottom_left == 16.0
                ),
            ),
        ],
    });
}

#[test]
fn border_width_individual() {
    CssTestRunner::run(&CssTestCase {
        name: "border-width individual sides",
        css: "div { border-top-width: 1px; border-right-width: 2px; border-bottom-width: 3px; border-left-width: 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side_f32!("border-width-top", border_width.top == 1.0),
            ),
            (
                0,
                assert_side_f32!("border-width-right", border_width.right == 2.0),
            ),
            (
                0,
                assert_side_f32!("border-width-bottom", border_width.bottom == 3.0),
            ),
            (
                0,
                assert_side_f32!("border-width-left", border_width.left == 4.0),
            ),
        ],
    });
}

#[test]
fn opacity_clamp_above_one() {
    CssTestRunner::run(&CssTestCase {
        name: "opacity: 2 clamped to 1",
        css: "div { opacity: 2; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("opacity", opacity == 1.0))],
    });
}

#[test]
fn opacity_clamp_below_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "opacity: -1 clamped to 0",
        css: "div { opacity: -1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("opacity", opacity == 0.0))],
    });
}

#[test]
fn text_align_start() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: start",
        css: "div { text-align: start; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Start),
        )],
    });
}

#[test]
fn text_align_end() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: end",
        css: "div { text-align: end; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("text-align", text_align == TextAlign::End))],
    });
}

#[test]
fn font_weight_normal() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: normal (400)",
        css: "div { font-weight: normal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight normal",
                check: Box::new(|style| {
                    if style.font_weight == 400 {
                        Ok(())
                    } else {
                        Err(format!("expected 400, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_weight_900() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: 900",
        css: "div { font-weight: 900; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight 900",
                check: Box::new(|style| {
                    if style.font_weight == 900 {
                        Ok(())
                    } else {
                        Err(format!("expected 900, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_style_normal() {
    CssTestRunner::run(&CssTestCase {
        name: "font-style: normal",
        css: "div { font-style: normal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("font-style", font_style == FontStyle::Normal),
        )],
    });
}

#[test]
fn font_style_oblique() {
    CssTestRunner::run(&CssTestCase {
        name: "font-style: oblique",
        css: "div { font-style: oblique; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("font-style", font_style == FontStyle::Oblique),
        )],
    });
}

#[test]
fn overflow_xy_individual() {
    CssTestRunner::run(&CssTestCase {
        name: "overflow-x: scroll; overflow-y: hidden",
        css: "div { overflow-x: scroll; overflow-y: hidden; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_style!(
                    "overflow-x",
                    overflow_x == liquide_compositor::scene::Overflow::Scroll
                ),
            ),
            (
                0,
                assert_style!(
                    "overflow-y",
                    overflow_y == liquide_compositor::scene::Overflow::Hidden
                ),
            ),
        ],
    });
}

#[test]
fn z_index_negative() {
    CssTestRunner::run(&CssTestCase {
        name: "z-index: -1",
        css: "div { z-index: -1; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("z-index", z_index == Some(-1)))],
    });
}

#[test]
fn z_index_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "z-index: auto",
        css: "div { z-index: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("z-index", z_index == None))],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 27. LOGICAL PROPERTIES EXTENDED
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn padding_inline_end() {
    CssTestRunner::run(&CssTestCase {
        name: "padding-inline-end: 12px",
        css: "div { padding-inline-end: 12px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "padding-inline-end",
                padding_inline_end == Dimension::Px(12.0)
            ),
        )],
    });
}

#[test]
fn padding_block_end() {
    CssTestRunner::run(&CssTestCase {
        name: "padding-block-end: 16px",
        css: "div { padding-block-end: 16px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "padding-block-end",
                padding_block_end == Dimension::Px(16.0)
            ),
        )],
    });
}

#[test]
fn margin_block_start() {
    CssTestRunner::run(&CssTestCase {
        name: "margin-block-start: 24px",
        css: "div { margin-block-start: 24px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "margin-block-start",
                margin_block_start == Dimension::Px(24.0)
            ),
        )],
    });
}

#[test]
fn margin_block_end() {
    CssTestRunner::run(&CssTestCase {
        name: "margin-block-end: 32px",
        css: "div { margin-block-end: 32px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("margin-block-end", margin_block_end == Dimension::Px(32.0)),
        )],
    });
}

#[test]
fn inset_inline_end() {
    CssTestRunner::run(&CssTestCase {
        name: "inset-inline-end: 10px",
        css: "div { position: relative; inset-inline-end: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("inset-inline-end", inset_inline_end == Dimension::Px(10.0)),
        )],
    });
}

#[test]
fn inset_block_start() {
    CssTestRunner::run(&CssTestCase {
        name: "inset-block-start: 15px",
        css: "div { position: relative; inset-block-start: 15px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!(
                "inset-block-start",
                inset_block_start == Dimension::Px(15.0)
            ),
        )],
    });
}

#[test]
fn inset_block_end() {
    CssTestRunner::run(&CssTestCase {
        name: "inset-block-end: 20px",
        css: "div { position: relative; inset-block-end: 20px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("inset-block-end", inset_block_end == Dimension::Px(20.0)),
        )],
    });
}

#[test]
fn min_inline_size() {
    CssTestRunner::run(&CssTestCase {
        name: "min-inline-size: 50px",
        css: "div { min-inline-size: 50px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("min-inline-size", min_inline_size == Dimension::Px(50.0)),
        )],
    });
}

#[test]
fn max_inline_size() {
    CssTestRunner::run(&CssTestCase {
        name: "max-inline-size: 600px",
        css: "div { max-inline-size: 600px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-inline-size", max_inline_size == Dimension::Px(600.0)),
        )],
    });
}

#[test]
fn min_block_size() {
    CssTestRunner::run(&CssTestCase {
        name: "min-block-size: 100px",
        css: "div { min-block-size: 100px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("min-block-size", min_block_size == Dimension::Px(100.0)),
        )],
    });
}

#[test]
fn max_block_size() {
    CssTestRunner::run(&CssTestCase {
        name: "max-block-size: 400px",
        css: "div { max-block-size: 400px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-block-size", max_block_size == Dimension::Px(400.0)),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 28. SCROLL SNAP & OVERSCROLL
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn overscroll_behavior_x_contain() {
    CssTestRunner::run(&CssTestCase {
        name: "overscroll-behavior-x: contain",
        css: "div { overscroll-behavior-x: contain; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "overscroll-behavior-x",
                overscroll_behavior_x == OverscrollBehavior::Contain
            ),
        )],
    });
}

#[test]
fn overscroll_behavior_y_none() {
    CssTestRunner::run(&CssTestCase {
        name: "overscroll-behavior-y: none",
        css: "div { overscroll-behavior-y: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "overscroll-behavior-y",
                overscroll_behavior_y == OverscrollBehavior::None
            ),
        )],
    });
}

#[test]
fn scroll_padding_all() {
    CssTestRunner::run(&CssTestCase {
        name: "scroll-padding: 10px",
        css: "div { scroll-padding: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!(
                    "scroll-padding-top",
                    scroll_padding.top == Dimension::Px(10.0)
                ),
            ),
            (
                0,
                assert_side!(
                    "scroll-padding-right",
                    scroll_padding.right == Dimension::Px(10.0)
                ),
            ),
        ],
    });
}

#[test]
fn scroll_margin_all() {
    CssTestRunner::run(&CssTestCase {
        name: "scroll-margin: 5px",
        css: "div { scroll-margin: 5px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!("scroll-margin-top", scroll_margin.top == Dimension::Px(5.0)),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 29. FRAGMENTATION & COLUMNS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn break_before_page() {
    CssTestRunner::run(&CssTestCase {
        name: "break-before: page",
        css: "div { break-before: page; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("break-before", break_before == BreakValue::Page),
        )],
    });
}

#[test]
fn break_after_avoid() {
    CssTestRunner::run(&CssTestCase {
        name: "break-after: avoid",
        css: "div { break-after: avoid; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("break-after", break_after == BreakValue::Avoid),
        )],
    });
}

#[test]
fn break_inside_avoid() {
    CssTestRunner::run(&CssTestCase {
        name: "break-inside: avoid",
        css: "div { break-inside: avoid; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("break-inside", break_inside == BreakValue::Avoid),
        )],
    });
}

#[test]
fn orphans_value() {
    CssTestRunner::run(&CssTestCase {
        name: "orphans: 3",
        css: "div { orphans: 3; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "orphans",
                check: Box::new(|style| {
                    if style.orphans == 3 {
                        Ok(())
                    } else {
                        Err(format!("expected 3, got {}", style.orphans))
                    }
                }),
            },
        )],
    });
}

#[test]
fn widows_value() {
    CssTestRunner::run(&CssTestCase {
        name: "widows: 2",
        css: "div { widows: 2; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "widows",
                check: Box::new(|style| {
                    if style.widows == 2 {
                        Ok(())
                    } else {
                        Err(format!("expected 2, got {}", style.widows))
                    }
                }),
            },
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 30. TYPOGRAPHY EXTENDED
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn font_size_rem() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size: 1.5rem",
        css: "div { font-size: 1.5rem; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("font-size", font_size == 24.0))],
    });
}

#[test]
fn text_decoration_underline() {
    CssTestRunner::run(&CssTestCase {
        name: "text-decoration-line: underline",
        css: "div { text-decoration-line: underline; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "text-decoration-line",
                check: Box::new(|style| {
                    if style.text_decoration_line.as_deref() == Some("underline") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'underline', got {:?}",
                            style.text_decoration_line
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn text_decoration_line_through() {
    CssTestRunner::run(&CssTestCase {
        name: "text-decoration-line: line-through",
        css: "div { text-decoration-line: line-through; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "text-decoration-line",
                check: Box::new(|style| {
                    if style.text_decoration_line.as_deref() == Some("line-through") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'line-through', got {:?}",
                            style.text_decoration_line
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn text_overflow_clip() {
    CssTestRunner::run(&CssTestCase {
        name: "text-overflow: clip",
        css: "div { text-overflow: clip; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-overflow", text_overflow == TextOverflow::Clip),
        )],
    });
}

#[test]
fn vertical_align_middle() {
    CssTestRunner::run(&CssTestCase {
        name: "vertical-align: middle",
        css: "span { vertical-align: middle; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(
            0,
            assert_style!("vertical-align", vertical_align == VerticalAlign::Middle),
        )],
    });
}

#[test]
fn hyphens_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "hyphens: auto",
        css: "div { hyphens: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("hyphens", hyphens == Hyphens::Auto))],
    });
}

#[test]
fn hyphens_none() {
    CssTestRunner::run(&CssTestCase {
        name: "hyphens: none",
        css: "div { hyphens: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("hyphens", hyphens == Hyphens::None))],
    });
}

#[test]
fn font_feature_settings() {
    CssTestRunner::run(&CssTestCase {
        name: "font-feature-settings: 'liga' 1",
        css: r#"div { font-feature-settings: "liga" 1; }"#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-feature-settings",
                check: Box::new(|style| {
                    if style.font_feature_settings.is_some() {
                        Ok(())
                    } else {
                        Err("expected Some font-feature-settings".into())
                    }
                }),
            },
        )],
    });
}

#[test]
fn line_height_normal() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height: normal",
        css: "div { line-height: normal; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("line-height", line_height == LineHeight::Normal),
        )],
    });
}

#[test]
fn text_transform_none() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: none",
        css: "div { text-transform: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-transform", text_transform == TextTransform::None),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 31. COMPLEX COMPOUND TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn card_component_styles() {
    CssTestRunner::run(&CssTestCase {
        name: "card component compound test",
        css: r#"
            .card {
                display: flex;
                flex-direction: column;
                width: 320px;
                padding: 16px;
                border-radius: 8px;
                box-sizing: border-box;
                overflow: hidden;
                background-color: #ffffff;
                color: #333333;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let card = doc.create_element("div");
            doc.add_class(card, "card");
            doc.append_child(root, card);
            vec![card]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Flex)),
            (
                0,
                assert_style!("flex-direction", flex_direction == FlexDirection::Column),
            ),
            (0, assert_dimension!("width", width == Dimension::Px(320.0))),
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(16.0)),
            ),
            (
                0,
                assert_corner_f32!("border-radius", border_radius.top_left == 8.0),
            ),
            (
                0,
                assert_style!("box-sizing", box_sizing == BoxSizing::BorderBox),
            ),
            (
                0,
                assert_style!(
                    "overflow-x",
                    overflow_x == liquide_compositor::scene::Overflow::Hidden
                ),
            ),
            (0, assert_color!("bg", background_color == (255, 255, 255))),
        ],
    });
}

#[test]
fn modal_overlay_styles() {
    CssTestRunner::run(&CssTestCase {
        name: "modal overlay compound test",
        css: r#"
            .overlay {
                position: fixed;
                top: 0;
                right: 0;
                bottom: 0;
                left: 0;
                z-index: 1000;
                display: flex;
                justify-content: center;
                align-items: center;
                opacity: 0.8;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let overlay = doc.create_element("div");
            doc.add_class(overlay, "overlay");
            doc.append_child(root, overlay);
            vec![overlay]
        }),
        assertions: vec![
            (0, assert_style!("position", position == Position::Fixed)),
            (0, assert_dimension!("top", top == Dimension::Px(0.0))),
            (0, assert_style!("z-index", z_index == Some(1000))),
            (0, assert_style!("display", display == Display::Flex)),
            (
                0,
                assert_style!("justify-content", justify_content == JustifyContent::Center),
            ),
            (
                0,
                assert_style!("align-items", align_items == AlignItems::Center),
            ),
            (0, assert_style_f32!("opacity", opacity == 0.8)),
        ],
    });
}

#[test]
fn responsive_grid_with_items() {
    CssTestRunner::run(&CssTestCase {
        name: "responsive grid with items",
        css: r#"
            .grid { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 16px; padding: 24px; }
            .cell { background-color: #eeeeee; padding: 8px; border-radius: 4px; }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let grid = doc.create_element("div");
            doc.add_class(grid, "grid");
            doc.append_child(root, grid);
            let c1 = doc.create_element("div");
            doc.add_class(c1, "cell");
            doc.append_child(grid, c1);
            let c2 = doc.create_element("div");
            doc.add_class(c2, "cell");
            doc.append_child(grid, c2);
            vec![grid, c1, c2]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Grid)),
            (
                0,
                StyleAssertion {
                    description: "3 columns",
                    check: Box::new(|style| {
                        if style.grid_template_columns.len() == 3 {
                            Ok(())
                        } else {
                            Err(format!(
                                "expected 3, got {}",
                                style.grid_template_columns.len()
                            ))
                        }
                    }),
                },
            ),
            (
                0,
                assert_side!("grid padding", padding.top == Dimension::Px(24.0)),
            ),
            (
                1,
                assert_side!("cell padding", padding.top == Dimension::Px(8.0)),
            ),
            (
                1,
                assert_corner_f32!("cell radius", border_radius.top_left == 4.0),
            ),
        ],
    });
}

#[test]
fn sidebar_nav_styles() {
    CssTestRunner::run(&CssTestCase {
        name: "sidebar nav component",
        css: r#"
            .nav {
                display: flex;
                flex-direction: column;
                width: 260px;
                min-height: 100vh;
                padding: 16px 8px;
                background-color: #1a1a2e;
                color: #e0e0e0;
                overflow-y: auto;
            }
        "#,
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let nav = doc.create_element("nav");
            doc.add_class(nav, "nav");
            doc.append_child(root, nav);
            vec![nav]
        }),
        assertions: vec![
            (0, assert_style!("display", display == Display::Flex)),
            (
                0,
                assert_style!("flex-direction", flex_direction == FlexDirection::Column),
            ),
            (0, assert_dimension!("width", width == Dimension::Px(260.0))),
            (
                0,
                assert_dimension!("min-height", min_height == Dimension::Vh(100.0)),
            ),
            (
                0,
                assert_style!(
                    "overflow-y",
                    overflow_y == liquide_compositor::scene::Overflow::Auto
                ),
            ),
        ],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 32. ANCHOR POSITIONING & MISC SPEC
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn anchor_name_property() {
    CssTestRunner::run(&CssTestCase {
        name: "anchor-name: --tooltip-anchor",
        css: "div { anchor-name: --tooltip-anchor; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "anchor-name",
                check: Box::new(|style| {
                    if style.anchor_name.as_deref() == Some("--tooltip-anchor") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected '--tooltip-anchor', got {:?}",
                            style.anchor_name
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn position_anchor_property() {
    CssTestRunner::run(&CssTestCase {
        name: "position-anchor: --tooltip-anchor",
        css: "div { position-anchor: --tooltip-anchor; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "position-anchor",
                check: Box::new(|style| {
                    if style.position_anchor.as_deref() == Some("--tooltip-anchor") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected '--tooltip-anchor', got {:?}",
                            style.position_anchor
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn view_transition_name_property() {
    CssTestRunner::run(&CssTestCase {
        name: "view-transition-name: hero",
        css: "div { view-transition-name: hero; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "view-transition-name",
                check: Box::new(|style| {
                    if style.view_transition_name.as_deref() == Some("hero") {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected 'hero', got {:?}",
                            style.view_transition_name
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn zoom_property() {
    CssTestRunner::run(&CssTestCase {
        name: "zoom: 1.5",
        css: "div { zoom: 1.5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style_f32!("zoom", zoom == 1.5))],
    });
}

#[test]
fn contain_size_only() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: size",
        css: "div { contain: size; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain size",
                check: Box::new(|style| {
                    if style.contain.size {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected size containment, got {:?}",
                            style.contain
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn contain_style_only() {
    CssTestRunner::run(&CssTestCase {
        name: "contain: style",
        css: "div { contain: style; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "contain style",
                check: Box::new(|style| {
                    if style.contain.style {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected style containment, got {:?}",
                            style.contain
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn will_change_transform() {
    CssTestRunner::run(&CssTestCase {
        name: "will-change: transform",
        css: "div { will-change: transform; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "will-change",
                check: Box::new(|style| {
                    if style.will_change.contains(&"transform".to_string()) {
                        Ok(())
                    } else {
                        Err(format!("expected 'transform', got {:?}", style.will_change))
                    }
                }),
            },
        )],
    });
}

#[test]
fn will_change_opacity() {
    CssTestRunner::run(&CssTestCase {
        name: "will-change: opacity",
        css: "div { will-change: opacity; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "will-change opacity",
                check: Box::new(|style| {
                    if style.will_change.contains(&"opacity".to_string()) {
                        Ok(())
                    } else {
                        Err(format!("expected 'opacity', got {:?}", style.will_change))
                    }
                }),
            },
        )],
    });
}

#[test]
fn pointer_events_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "pointer-events: auto",
        css: "div { pointer-events: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("pointer-events", pointer_events == PointerEvents::Auto),
        )],
    });
}

#[test]
fn table_layout_fixed() {
    CssTestRunner::run(&CssTestCase {
        name: "table-layout: fixed",
        css: "table { table-layout: fixed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let table = doc.create_element("table");
            doc.append_child(root, table);
            vec![table]
        }),
        assertions: vec![(
            0,
            assert_style!("table-layout", table_layout == TableLayout::Fixed),
        )],
    });
}

#[test]
fn empty_cells_hide() {
    CssTestRunner::run(&CssTestCase {
        name: "empty-cells: hide",
        css: "td { empty-cells: hide; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let td = doc.create_element("td");
            doc.append_child(root, td);
            vec![td]
        }),
        assertions: vec![(
            0,
            assert_style!("empty-cells", empty_cells == EmptyCells::Hide),
        )],
    });
}

#[test]
fn caption_side_bottom() {
    CssTestRunner::run(&CssTestCase {
        name: "caption-side: bottom",
        css: "caption { caption-side: bottom; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let cap = doc.create_element("caption");
            doc.append_child(root, cap);
            vec![cap]
        }),
        assertions: vec![(
            0,
            assert_style!("caption-side", caption_side == CaptionSide::Bottom),
        )],
    });
}

#[test]
fn text_align_left() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: left",
        css: "div { text-align: left; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Left),
        )],
    });
}

#[test]
fn color_named_cyan() {
    CssTestRunner::run(&CssTestCase {
        name: "color: cyan",
        css: "div { color: cyan; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color cyan", color == (0, 255, 255)))],
    });
}

#[test]
fn color_named_magenta() {
    CssTestRunner::run(&CssTestCase {
        name: "color: magenta",
        css: "div { color: magenta; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color magenta", color == (255, 0, 255)))],
    });
}

#[test]
fn color_named_yellow() {
    CssTestRunner::run(&CssTestCase {
        name: "color: yellow",
        css: "div { color: yellow; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("color yellow", color == (255, 255, 0)))],
    });
}

#[test]
fn height_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "height: 50%",
        css: "div { height: 50%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("height", height == Dimension::Percent(50.0)),
        )],
    });
}

#[test]
fn width_fit_content() {
    CssTestRunner::run(&CssTestCase {
        name: "width: fit-content",
        css: "div { width: fit-content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::FitContent),
        )],
    });
}

#[test]
fn visibility_collapse() {
    CssTestRunner::run(&CssTestCase {
        name: "visibility: collapse",
        css: "div { visibility: collapse; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("visibility", visibility == Visibility::Collapse),
        )],
    });
}

#[test]
fn visibility_visible() {
    CssTestRunner::run(&CssTestCase {
        name: "visibility: visible",
        css: "div { visibility: visible; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("visibility", visibility == Visibility::Visible),
        )],
    });
}

#[test]
fn float_none() {
    CssTestRunner::run(&CssTestCase {
        name: "float: none",
        css: "div { float: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("float", float == Float::None))],
    });
}

#[test]
fn clear_left() {
    CssTestRunner::run(&CssTestCase {
        name: "clear: left",
        css: "div { clear: left; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("clear", clear == Clear::Left))],
    });
}

#[test]
fn clear_right() {
    CssTestRunner::run(&CssTestCase {
        name: "clear: right",
        css: "div { clear: right; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("clear", clear == Clear::Right))],
    });
}

#[test]
fn clear_none() {
    CssTestRunner::run(&CssTestCase {
        name: "clear: none",
        css: "div { clear: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("clear", clear == Clear::None))],
    });
}

#[test]
fn border_style_none() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: none",
        css: "div { border-style: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::None
            ),
        )],
    });
}

#[test]
fn border_style_double() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: double",
        css: "div { border-style: double; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::Double
            ),
        )],
    });
}

#[test]
fn border_radius_two_values() {
    CssTestRunner::run(&CssTestCase {
        name: "border-radius: 4px 8px",
        css: "div { border-radius: 4px 8px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_corner_f32!("top-left", border_radius.top_left == 4.0),
            ),
            (
                0,
                assert_corner_f32!("top-right", border_radius.top_right == 8.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-right", border_radius.bottom_right == 4.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-left", border_radius.bottom_left == 8.0),
            ),
        ],
    });
}

#[test]
fn margin_percent() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 5%",
        css: "div { margin: 5%; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!("margin-top", margin.top == Dimension::Percent(5.0)),
        )],
    });
}

#[test]
fn inset_shorthand() {
    CssTestRunner::run(&CssTestCase {
        name: "inset: 10px",
        css: "div { position: absolute; inset: 10px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_dimension!("top", top == Dimension::Px(10.0))),
            (0, assert_dimension!("right", right == Dimension::Px(10.0))),
            (
                0,
                assert_dimension!("bottom", bottom == Dimension::Px(10.0)),
            ),
            (0, assert_dimension!("left", left == Dimension::Px(10.0))),
        ],
    });
}

#[test]
fn height_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "height: auto",
        css: "div { height: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("height", height == Dimension::Auto))],
    });
}

#[test]
fn min_width_zero() {
    CssTestRunner::run(&CssTestCase {
        name: "min-width: 0",
        css: "div { min-width: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("min-width", min_width == Dimension::Px(0.0)),
        )],
    });
}

#[test]
fn max_height_none() {
    CssTestRunner::run(&CssTestCase {
        name: "max-height: none",
        css: "div { max-height: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("max-height", max_height == Dimension::None),
        )],
    });
}

// ═══════════════════════════════════════════════════════════════════════════
// 33. ADDITIONAL TESTS TO REACH 300+ NEW
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn display_inline_block() {
    CssTestRunner::run(&CssTestCase {
        name: "display: inline-block",
        css: "div { display: inline-block; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::InlineBlock))],
    });
}

#[test]
fn display_table() {
    CssTestRunner::run(&CssTestCase {
        name: "display: table",
        css: "div { display: table; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Table))],
    });
}

#[test]
fn display_table_cell() {
    CssTestRunner::run(&CssTestCase {
        name: "display: table-cell",
        css: "div { display: table-cell; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::TableCell))],
    });
}

#[test]
fn display_table_row() {
    CssTestRunner::run(&CssTestCase {
        name: "display: table-row",
        css: "div { display: table-row; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::TableRow))],
    });
}

#[test]
fn display_contents() {
    CssTestRunner::run(&CssTestCase {
        name: "display: contents",
        css: "div { display: contents; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("display", display == Display::Contents))],
    });
}

#[test]
fn position_sticky() {
    CssTestRunner::run(&CssTestCase {
        name: "position: sticky",
        css: "div { position: sticky; top: 0; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_style!("position", position == Position::Sticky)),
            (0, assert_dimension!("top", top == Dimension::Px(0.0))),
        ],
    });
}

#[test]
fn width_min_content() {
    CssTestRunner::run(&CssTestCase {
        name: "width: min-content",
        css: "div { width: min-content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::MinContent),
        )],
    });
}

#[test]
fn width_max_content() {
    CssTestRunner::run(&CssTestCase {
        name: "width: max-content",
        css: "div { width: max-content; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("width", width == Dimension::MaxContent),
        )],
    });
}

#[test]
fn padding_three_values() {
    CssTestRunner::run(&CssTestCase {
        name: "padding: 10px 20px 30px",
        css: "div { padding: 10px 20px 30px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("padding-top", padding.top == Dimension::Px(10.0)),
            ),
            (
                0,
                assert_side!("padding-right", padding.right == Dimension::Px(20.0)),
            ),
            (
                0,
                assert_side!("padding-bottom", padding.bottom == Dimension::Px(30.0)),
            ),
            (
                0,
                assert_side!("padding-left", padding.left == Dimension::Px(20.0)),
            ),
        ],
    });
}

#[test]
fn margin_four_values() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 1px 2px 3px 4px",
        css: "div { margin: 1px 2px 3px 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(1.0)),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Px(2.0)),
            ),
            (
                0,
                assert_side!("margin-bottom", margin.bottom == Dimension::Px(3.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Px(4.0)),
            ),
        ],
    });
}

#[test]
fn margin_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: auto",
        css: "div { margin: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (0, assert_side!("margin-top", margin.top == Dimension::Auto)),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Auto),
            ),
        ],
    });
}

#[test]
fn margin_auto_horizontal() {
    CssTestRunner::run(&CssTestCase {
        name: "margin: 0 auto",
        css: "div { margin: 0 auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side!("margin-top", margin.top == Dimension::Px(0.0)),
            ),
            (
                0,
                assert_side!("margin-left", margin.left == Dimension::Auto),
            ),
            (
                0,
                assert_side!("margin-right", margin.right == Dimension::Auto),
            ),
        ],
    });
}

#[test]
fn text_indent_px() {
    CssTestRunner::run(&CssTestCase {
        name: "text-indent: 2em",
        css: "div { text-indent: 2em; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("text-indent", text_indent == Dimension::Em(2.0)),
        )],
    });
}

#[test]
fn letter_spacing_px() {
    CssTestRunner::run(&CssTestCase {
        name: "letter-spacing: 2px",
        css: "div { letter-spacing: 2px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("letter-spacing", letter_spacing == Dimension::Px(2.0)),
        )],
    });
}

#[test]
fn word_spacing_px() {
    CssTestRunner::run(&CssTestCase {
        name: "word-spacing: 4px",
        css: "div { word-spacing: 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("word-spacing", word_spacing == Dimension::Px(4.0)),
        )],
    });
}

#[test]
fn tab_size_4() {
    CssTestRunner::run(&CssTestCase {
        name: "tab-size: 4",
        css: "pre { tab-size: 4; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let pre = doc.create_element("pre");
            doc.append_child(root, pre);
            vec![pre]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "tab-size",
                check: Box::new(|style| {
                    if style.tab_size == 4.0 {
                        Ok(())
                    } else {
                        Err(format!("expected 4, got {}", style.tab_size))
                    }
                }),
            },
        )],
    });
}

#[test]
fn background_color_transparent() {
    CssTestRunner::run(&CssTestCase {
        name: "background-color: transparent",
        css: "div { background-color: transparent; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "bg transparent",
                check: Box::new(|style| {
                    if style.background_color.a == 0 {
                        Ok(())
                    } else {
                        Err(format!(
                            "expected alpha 0, got {}",
                            style.background_color.a
                        ))
                    }
                }),
            },
        )],
    });
}

#[test]
fn color_named_white() {
    CssTestRunner::run(&CssTestCase {
        name: "color: white",
        css: "div { color: white; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("white", color == (255, 255, 255)))],
    });
}

#[test]
fn color_named_black() {
    CssTestRunner::run(&CssTestCase {
        name: "color: black",
        css: "div { color: black; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("black", color == (0, 0, 0)))],
    });
}

#[test]
fn color_named_red() {
    CssTestRunner::run(&CssTestCase {
        name: "color: red",
        css: "div { color: red; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("red", color == (255, 0, 0)))],
    });
}

#[test]
fn color_named_green() {
    CssTestRunner::run(&CssTestCase {
        name: "color: green",
        css: "div { color: green; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("green", color == (0, 128, 0)))],
    });
}

#[test]
fn color_named_blue() {
    CssTestRunner::run(&CssTestCase {
        name: "color: blue",
        css: "div { color: blue; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("blue", color == (0, 0, 255)))],
    });
}

#[test]
fn color_hex_3digit() {
    CssTestRunner::run(&CssTestCase {
        name: "color: #f00",
        css: "div { color: #f00; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_color!("hex 3 digit", color == (255, 0, 0)))],
    });
}

#[test]
fn font_size_keyword_small() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size: small",
        css: "div { font-size: small; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-size small",
                check: Box::new(|style| {
                    if (style.font_size - 13.0).abs() < 1.0 {
                        Ok(())
                    } else {
                        Err(format!("expected ~13px, got {}", style.font_size))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_size_keyword_large() {
    CssTestRunner::run(&CssTestCase {
        name: "font-size: large",
        css: "div { font-size: large; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-size large",
                check: Box::new(|style| {
                    if (style.font_size - 18.0).abs() < 1.0 {
                        Ok(())
                    } else {
                        Err(format!("expected ~18px, got {}", style.font_size))
                    }
                }),
            },
        )],
    });
}

#[test]
fn text_transform_capitalize() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: capitalize",
        css: "div { text-transform: capitalize; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!(
                "text-transform",
                text_transform == TextTransform::Capitalize
            ),
        )],
    });
}

#[test]
fn text_transform_lowercase() {
    CssTestRunner::run(&CssTestCase {
        name: "text-transform: lowercase",
        css: "div { text-transform: lowercase; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-transform", text_transform == TextTransform::Lowercase),
        )],
    });
}

#[test]
fn text_align_center() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: center",
        css: "div { text-align: center; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Center),
        )],
    });
}

#[test]
fn text_align_justify() {
    CssTestRunner::run(&CssTestCase {
        name: "text-align: justify",
        css: "div { text-align: justify; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("text-align", text_align == TextAlign::Justify),
        )],
    });
}

#[test]
fn vertical_align_top() {
    CssTestRunner::run(&CssTestCase {
        name: "vertical-align: top",
        css: "span { vertical-align: top; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(
            0,
            assert_style!("vertical-align", vertical_align == VerticalAlign::Top),
        )],
    });
}

#[test]
fn vertical_align_bottom() {
    CssTestRunner::run(&CssTestCase {
        name: "vertical-align: bottom",
        css: "span { vertical-align: bottom; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let span = doc.create_element("span");
            doc.append_child(root, span);
            vec![span]
        }),
        assertions: vec![(
            0,
            assert_style!("vertical-align", vertical_align == VerticalAlign::Bottom),
        )],
    });
}

#[test]
fn cursor_text() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: text",
        css: "div { cursor: text; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::Text))],
    });
}

#[test]
fn cursor_not_allowed() {
    CssTestRunner::run(&CssTestCase {
        name: "cursor: not-allowed",
        css: "div { cursor: not-allowed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("cursor", cursor == Cursor::NotAllowed))],
    });
}

#[test]
fn outline_none() {
    CssTestRunner::run(&CssTestCase {
        name: "outline: none",
        css: "div { outline: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "outline none",
                check: Box::new(|style| {
                    if style.outline.style == BorderLineStyle::None {
                        Ok(())
                    } else {
                        Err(format!("expected none, got {:?}", style.outline.style))
                    }
                }),
            },
        )],
    });
}

#[test]
fn touch_action_none() {
    CssTestRunner::run(&CssTestCase {
        name: "touch-action: none",
        css: "div { touch-action: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("touch-action", touch_action == TouchAction::None),
        )],
    });
}

#[test]
fn touch_action_manipulation() {
    CssTestRunner::run(&CssTestCase {
        name: "touch-action: manipulation",
        css: "div { touch-action: manipulation; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("touch-action", touch_action == TouchAction::Manipulation),
        )],
    });
}

#[test]
fn appearance_none() {
    CssTestRunner::run(&CssTestCase {
        name: "appearance: none",
        css: "div { appearance: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("appearance", appearance == Appearance::None),
        )],
    });
}

#[test]
fn isolation_isolate() {
    CssTestRunner::run(&CssTestCase {
        name: "isolation: isolate",
        css: "div { isolation: isolate; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("isolation", isolation == Isolation::Isolate),
        )],
    });
}

#[test]
fn isolation_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "isolation: auto",
        css: "div { isolation: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("isolation", isolation == Isolation::Auto))],
    });
}

#[test]
fn object_fit_contain() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: contain",
        css: "img { object-fit: contain; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::Contain),
        )],
    });
}

#[test]
fn object_fit_cover() {
    CssTestRunner::run(&CssTestCase {
        name: "object-fit: cover",
        css: "img { object-fit: cover; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let img = doc.create_element("img");
            doc.append_child(root, img);
            vec![img]
        }),
        assertions: vec![(
            0,
            assert_style!("object-fit", object_fit == ObjectFit::Cover),
        )],
    });
}

#[test]
fn font_weight_bold() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: bold (700)",
        css: "div { font-weight: bold; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight bold",
                check: Box::new(|style| {
                    if style.font_weight == 700 {
                        Ok(())
                    } else {
                        Err(format!("expected 700, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn font_weight_100() {
    CssTestRunner::run(&CssTestCase {
        name: "font-weight: 100",
        css: "div { font-weight: 100; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            StyleAssertion {
                description: "font-weight 100",
                check: Box::new(|style| {
                    if style.font_weight == 100 {
                        Ok(())
                    } else {
                        Err(format!("expected 100, got {}", style.font_weight))
                    }
                }),
            },
        )],
    });
}

#[test]
fn border_shorthand_parsing() {
    CssTestRunner::run(&CssTestCase {
        name: "border: 2px solid #333",
        css: "div { border: 2px solid #333333; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_side_f32!("border-top-width", border_width.top == 2.0),
            ),
            (
                0,
                assert_side!(
                    "border-top-style",
                    border_style.top == BorderLineStyle::Solid
                ),
            ),
        ],
    });
}

#[test]
fn border_style_dashed() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: dashed",
        css: "div { border-style: dashed; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::Dashed
            ),
        )],
    });
}

#[test]
fn border_style_dotted() {
    CssTestRunner::run(&CssTestCase {
        name: "border-style: dotted",
        css: "div { border-style: dotted; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_side!(
                "border-style-top",
                border_style.top == BorderLineStyle::Dotted
            ),
        )],
    });
}

#[test]
fn z_index_large_positive() {
    CssTestRunner::run(&CssTestCase {
        name: "z-index: 9999",
        css: "div { z-index: 9999; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_style!("z-index", z_index == Some(9999)))],
    });
}

#[test]
fn box_sizing_content_box() {
    CssTestRunner::run(&CssTestCase {
        name: "box-sizing: content-box",
        css: "div { box-sizing: content-box; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("box-sizing", box_sizing == BoxSizing::ContentBox),
        )],
    });
}

#[test]
fn height_vh() {
    CssTestRunner::run(&CssTestCase {
        name: "height: 100vh",
        css: "div { height: 100vh; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_dimension!("height", height == Dimension::Vh(100.0)),
        )],
    });
}

#[test]
fn width_vw() {
    CssTestRunner::run(&CssTestCase {
        name: "width: 50vw",
        css: "div { width: 50vw; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(0, assert_dimension!("width", width == Dimension::Vw(50.0)))],
    });
}

#[test]
fn flex_combined_shorthand_auto() {
    CssTestRunner::run(&CssTestCase {
        name: "flex: auto",
        css: ".item { flex: auto; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (0, assert_style_f32!("flex-grow", flex_grow == 1.0)),
            (0, assert_style_f32!("flex-shrink", flex_shrink == 1.0)),
            (
                0,
                assert_dimension!("flex-basis", flex_basis == Dimension::Auto),
            ),
        ],
    });
}

#[test]
fn flex_combined_shorthand_none() {
    CssTestRunner::run(&CssTestCase {
        name: "flex: none",
        css: ".item { flex: none; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let item = doc.create_element("div");
            doc.add_class(item, "item");
            doc.append_child(root, item);
            vec![item]
        }),
        assertions: vec![
            (0, assert_style_f32!("flex-grow", flex_grow == 0.0)),
            (0, assert_style_f32!("flex-shrink", flex_shrink == 0.0)),
            (
                0,
                assert_dimension!("flex-basis", flex_basis == Dimension::Auto),
            ),
        ],
    });
}

#[test]
fn line_height_number() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height: 1.5",
        css: "div { line-height: 1.5; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("line-height", line_height == LineHeight::Number(1.5)),
        )],
    });
}

#[test]
fn line_height_px() {
    CssTestRunner::run(&CssTestCase {
        name: "line-height: 24px",
        css: "div { line-height: 24px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("line-height", line_height == LineHeight::Length(24.0)),
        )],
    });
}

#[test]
fn grid_auto_flow_column() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: column",
        css: "div { display: grid; grid-auto-flow: column; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::Column),
        )],
    });
}

#[test]
fn grid_auto_flow_row() {
    CssTestRunner::run(&CssTestCase {
        name: "grid-auto-flow: row",
        css: "div { display: grid; grid-auto-flow: row; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![(
            0,
            assert_style!("grid-auto-flow", grid_auto_flow == GridAutoFlow::Row),
        )],
    });
}

#[test]
fn border_radius_three_values() {
    CssTestRunner::run(&CssTestCase {
        name: "border-radius: 4px 8px 12px",
        css: "div { border-radius: 4px 8px 12px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_corner_f32!("top-left", border_radius.top_left == 4.0),
            ),
            (
                0,
                assert_corner_f32!("top-right", border_radius.top_right == 8.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-right", border_radius.bottom_right == 12.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-left", border_radius.bottom_left == 8.0),
            ),
        ],
    });
}

#[test]
fn border_radius_four_values() {
    CssTestRunner::run(&CssTestCase {
        name: "border-radius: 1px 2px 3px 4px",
        css: "div { border-radius: 1px 2px 3px 4px; }",
        build_dom: Box::new(|doc| {
            let root = doc.root();
            let div = doc.create_element("div");
            doc.append_child(root, div);
            vec![div]
        }),
        assertions: vec![
            (
                0,
                assert_corner_f32!("top-left", border_radius.top_left == 1.0),
            ),
            (
                0,
                assert_corner_f32!("top-right", border_radius.top_right == 2.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-right", border_radius.bottom_right == 3.0),
            ),
            (
                0,
                assert_corner_f32!("bottom-left", border_radius.bottom_left == 4.0),
            ),
        ],
    });
}
