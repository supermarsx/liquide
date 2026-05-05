use super::*;
use crate::PseudoKind;
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

#[test]
fn current_color_on_color_property_inherits_parent_value() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: red; }
            .child  { color: currentColor; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "child");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    // `color: currentColor` on the `color` property must resolve to the
    // inherited (parent) value per CSS Color Level 4.
    assert_eq!(child_style.color.r, 255);
    assert_eq!(child_style.color.g, 0);
    assert_eq!(child_style.color.b, 0);
}

#[test]
fn invalidate_preserves_inherited_custom_property_scope() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .red-scope { --accent: #ff0000; }
            .blue-scope { --accent: #0000ff; }
            .target { color: var(--accent); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let red_scope = doc.create_element("div");
    doc.add_class(red_scope, "red-scope");
    doc.append_child(root, red_scope);

    let target = doc.create_element("span");
    doc.add_class(target, "target");
    doc.append_child(red_scope, target);

    let blue_scope = doc.create_element("div");
    doc.add_class(blue_scope, "blue-scope");
    doc.append_child(root, blue_scope);

    let mut styles = engine.restyle_all(&doc);
    let initial = styles.get(target).unwrap();
    assert_eq!(
        (initial.color.r, initial.color.g, initial.color.b),
        (255, 0, 0)
    );

    engine.invalidate(&doc, &[target], &mut styles);

    let updated = styles.get(target).unwrap();
    assert_eq!(
        (updated.color.r, updated.color.g, updated.color.b),
        (255, 0, 0)
    );
}

#[test]
fn restyle_dirty_rebuilds_ancestor_custom_property_scope() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .red-scope { --accent: #ff0000; }
            .blue-scope { --accent: #0000ff; }
            .target { color: var(--accent); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let red_scope = doc.create_element("div");
    doc.add_class(red_scope, "red-scope");
    doc.append_child(root, red_scope);

    let target = doc.create_element("span");
    doc.add_class(target, "target");
    doc.append_child(red_scope, target);

    let blue_scope = doc.create_element("div");
    doc.add_class(blue_scope, "blue-scope");
    doc.append_child(root, blue_scope);

    let mut styles = engine.restyle_all(&doc);
    let mut dirty = liquide_dom::dirty::DirtySet::new();
    dirty.mark_style(target);
    engine.restyle_dirty(&doc, &dirty, &mut styles);

    let updated = styles.get(target).unwrap();
    assert_eq!(
        (updated.color.r, updated.color.g, updated.color.b),
        (255, 0, 0)
    );
}

#[test]
fn shadow_root_custom_property_scope_stays_isolated() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .host { --accent: #ff0000; }
            .other { --accent: #0000ff; }
            .inner { color: var(--accent, #00ff00); }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let host = doc.create_element("div");
    doc.add_class(host, "host");
    doc.append_child(root, host);

    let shadow_root = doc.create_shadow_root();
    doc.append_child(host, shadow_root);

    let inner = doc.create_element("span");
    doc.add_class(inner, "inner");
    doc.append_child(shadow_root, inner);

    let other = doc.create_element("div");
    doc.add_class(other, "other");
    doc.append_child(root, other);

    let mut styles = engine.restyle_all(&doc);
    let full = styles.get(inner).unwrap();
    assert_eq!((full.color.r, full.color.g, full.color.b), (0, 255, 0));

    engine.invalidate(&doc, &[inner], &mut styles);
    let incremental = styles.get(inner).unwrap();
    assert_eq!(
        (
            incremental.color.r,
            incremental.color.g,
            incremental.color.b
        ),
        (0, 255, 0)
    );
}

#[ignore = "public pseudo-rule ingestion still depends on selector/stylesheet routing outside this validation path"]
#[test]
fn pseudo_elements_use_local_custom_properties() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            div::first-line {
                --accent: #ff0000;
                color: var(--accent);
            }
            .other { --accent: #0000ff; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let host = doc.create_element("div");
    doc.append_child(root, host);

    let other = doc.create_element("div");
    doc.add_class(other, "other");
    doc.append_child(root, other);

    let styles = engine.restyle_all(&doc);
    let first_line = styles.get_pseudo(host, PseudoKind::FirstLine).unwrap();
    assert_eq!(
        (first_line.color.r, first_line.color.g, first_line.color.b),
        (255, 0, 0)
    );
}

#[test]
fn all_initial_resets_inherited_properties_to_initial_values() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: #ff0000; }
            .reset { all: initial; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "reset");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    assert_eq!(
        (
            child_style.color.r,
            child_style.color.g,
            child_style.color.b
        ),
        (0, 0, 0)
    );
}

#[test]
fn all_revert_restores_parent_inherited_values() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet(
        r#"
            .parent { color: #ff0000; }
            .reset { color: #0000ff; }
            .reset { all: revert; }
        "#,
    );

    let mut doc = Document::new();
    let root = doc.root();

    let parent = doc.create_element("div");
    doc.add_class(parent, "parent");
    doc.append_child(root, parent);

    let child = doc.create_element("span");
    doc.add_class(child, "reset");
    doc.append_child(parent, child);

    let styles = engine.restyle_all(&doc);
    let child_style = styles.get(child).unwrap();
    assert_eq!(
        (
            child_style.color.r,
            child_style.color.g,
            child_style.color.b
        ),
        (255, 0, 0)
    );
}

#[test]
fn transition_duration_defaults_transition_definitions_to_all() {
    let mut engine = StyleEngine::default();
    engine.add_stylesheet("div { transition-duration: 120ms; }");

    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    doc.append_child(root, div);

    let styles = engine.restyle_all(&doc);
    let style = styles.get(div).unwrap();
    assert_eq!(style.transition.len(), 1);
    assert_eq!(style.transition[0].property, "all");
    assert!((style.transition[0].duration_ms - 120.0).abs() < 0.01);
}
