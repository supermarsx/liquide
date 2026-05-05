use liquide_dom::Document;
use liquide_dom::dirty::DirtySet;
use liquide_style_engine::cascade::CascadeDeclaration;
use liquide_style_engine::computed::{ComputedStyle, TimingFunction, TransitionDef};
use liquide_style_engine::{
    CascadeMap, CascadePriority, Dimension, PseudoKind, Specificity, StyleEngine,
};

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
    let mut dirty = DirtySet::new();
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

#[ignore = "public pseudo-rule ingestion still depends on selector/stylesheet routing outside this validation target"]
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
}

#[allow(deprecated)]
#[test]
fn transition_property_all_tracks_supported_numeric_changes() {
    let mut manager = liquide_style_engine::transition::TransitionManager::new();
    let node_id = 1;

    let mut style = ComputedStyle::default();
    style.transition = vec![TransitionDef {
        property: "all".into(),
        duration_ms: 150.0,
        delay_ms: 0.0,
        timing_function: TimingFunction::Linear,
    }];
    style.width = Dimension::Px(10.0);
    style.opacity = 0.5;

    manager.update_node(node_id, &style);

    let mut changed = style.clone();
    changed.width = Dimension::Px(30.0);

    manager.update_node(node_id, &changed);

    assert_eq!(manager.get_value(node_id, "width"), Some(10.0));
    assert!(manager.has_running_transitions());
    assert!(manager.get_value(node_id, "opacity").is_none());
}

#[test]
fn revert_layer_falls_back_across_lower_origins_when_no_lower_layer_exists() {
    let mut map = CascadeMap::new();
    map.add(CascadeDeclaration {
        property: "color".into(),
        value: liquide_theme_css::value::PropertyValue::Keyword("ua-red".into()),
        priority: CascadePriority::ua(0),
    });
    map.add(CascadeDeclaration {
        property: "color".into(),
        value: liquide_theme_css::value::PropertyValue::Keyword("user-green".into()),
        priority: CascadePriority::user(Specificity::ZERO, 1),
    });
    let mut author = CascadePriority::author(Specificity::ZERO, 2);
    author.layer_order = 2;
    map.add(CascadeDeclaration {
        property: "color".into(),
        value: liquide_theme_css::value::PropertyValue::Keyword("revert-layer".into()),
        priority: author,
    });

    let resolved = map.resolve();
    let color = resolved.iter().find(|(key, _)| key == "color").unwrap();
    assert!(matches!(
        &color.1,
        liquide_theme_css::value::PropertyValue::Keyword(kw) if kw == "user-green"
    ));
}
