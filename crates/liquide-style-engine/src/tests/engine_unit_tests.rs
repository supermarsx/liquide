use super::*;
use liquide_dom::Document;

#[test]
fn empty_engine() {
    let engine = StyleEngine::default();
    let doc = Document::new();
    let style = engine.compute_style(&doc, doc.root());
    assert_eq!(style.display, Display::Block);
}

#[test]
fn basic_style_computation() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            div {
                display: flex;
                width: 100px;
                color: red;
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let style = engine.compute_style(&doc, div);
    assert_eq!(style.display, Display::Flex);
}

#[test]
fn restyle_all() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            statusbar {
                display: flex;
                position: fixed;
                height: 28px;
            }
            dock {
                display: flex;
                gap: 4px;
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let dock = doc.create_element("dock");
    doc.append_child(root, bar);
    doc.append_child(root, dock);

    let map = engine.restyle_all(&doc);
    let bar_style = map.get(bar).unwrap();
    assert_eq!(bar_style.display, Display::Flex);
    assert_eq!(bar_style.position, Position::Fixed);

    let dock_style = map.get(dock).unwrap();
    assert_eq!(dock_style.display, Display::Flex);
}

#[test]
fn prefers_color_scheme_media_query_follows_setting() {
    let mut engine = StyleEngine::default();
    assert!(engine.evaluate_media_condition("(prefers-color-scheme: light)"));
    assert!(!engine.evaluate_media_condition("(prefers-color-scheme: dark)"));

    engine.set_preferred_color_scheme("dark");
    assert!(engine.evaluate_media_condition("(prefers-color-scheme: dark)"));
    assert!(!engine.evaluate_media_condition("(prefers-color-scheme: light)"));
}

#[test]
fn supports_rules_are_respected() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            button { color: #ff0000; }
            @supports (display: grid) {
                button { color: #00ff00; }
            }
            @supports (nonexistent-prop: foo) {
                button { color: #0000ff; }
            }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();
    let button = doc.create_element("button");
    doc.append_child(root, button);

    let style = engine.compute_style(&doc, button);
    assert_eq!(style.color.g, 255);
    assert_eq!(style.color.r, 0);
}

#[test]
fn scope_rules_match_dom_descendants() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            @scope (.panel) {
                button { color: #00ff00; }
            }
            button { color: #ff0000; }
            "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let panel = doc.create_element("div");
    doc.add_class(panel, "panel");
    doc.append_child(root, panel);

    let scoped_button = doc.create_element("button");
    doc.append_child(panel, scoped_button);

    let unscoped_button = doc.create_element("button");
    doc.append_child(root, unscoped_button);

    let scoped_style = engine.compute_style(&doc, scoped_button);
    assert_eq!(scoped_style.color.g, 255);

    let plain_style = engine.compute_style(&doc, unscoped_button);
    assert_eq!(plain_style.color.r, 255);
}
