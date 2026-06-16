use super::*;
use crate::parser::ThemeParser;
use crate::value::Color;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_stylesheet() {
    let mut sheet = StyleSheet::new();

    let selector = Selector::element("button");
    let mut properties = PropertySet::new();
    properties.insert(
        "background".to_string(),
        PropertyValue::Color(Color::rgb(255, 0, 0)),
    );

    sheet.add_rule(selector, properties);

    assert_eq!(sheet.rule_count(), 1);
}

#[test]
fn test_cascade() {
    let mut sheet = StyleSheet::new();

    let selector1 = Selector::element("button");
    let mut props1 = PropertySet::new();
    props1.insert(
        "background".to_string(),
        PropertyValue::Color(Color::rgb(255, 0, 0)),
    );
    sheet.add_rule(selector1, props1);

    let selector2 = Selector::element("button").with_class("primary");
    let mut props2 = PropertySet::new();
    props2.insert(
        "background".to_string(),
        PropertyValue::Color(Color::rgb(0, 255, 0)),
    );
    sheet.add_rule(selector2, props2);

    let styles = sheet.compute_styles("button", &vec!["primary".to_string()], None, &[]);

    let color = styles.get("background").unwrap().as_color().unwrap();
    assert_eq!(color.g, 255);
}

#[test]
fn test_layer_and_conditions_cascade() {
    let mut sheet = StyleSheet::new();
    sheet.add_layer("base");
    sheet.add_layer("components");

    let mut base_props = PropertySet::new();
    base_props.insert(
        "color".to_string(),
        PropertyValue::Color(Color::rgb(255, 0, 0)),
    );
    sheet.add_rule_with_conditions(
        Selector::element("button"),
        base_props,
        None,
        None,
        Some("base".to_string()),
    );

    let mut component_props = PropertySet::new();
    component_props.insert(
        "color".to_string(),
        PropertyValue::Color(Color::rgb(0, 255, 0)),
    );
    sheet.add_rule_with_conditions(
        Selector::element("button"),
        component_props,
        Some("(max-width: 600px)".to_string()),
        Some("(display: grid)".to_string()),
        Some("components".to_string()),
    );

    let env = QueryEnvironment {
        viewport_width: 500.0,
        ..QueryEnvironment::default()
    };
    let styles = sheet.compute_styles_with_environment("button", &[], None, &[], &env);
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.g, 255);

    let env_desktop = QueryEnvironment {
        viewport_width: 1200.0,
        ..QueryEnvironment::default()
    };
    let desktop = sheet.compute_styles_with_environment("button", &[], None, &[], &env_desktop);
    let desktop_color = desktop.get("color").unwrap().as_color().unwrap();
    assert_eq!(desktop_color.r, 255);
}

#[test]
fn test_invalid_supports_and_media_fail_closed() {
    let css = r#"
            button { color: #ff0000; }
            @supports (display: definitely-not-a-real-value) {
                button { color: #0000ff; }
            }
            @media (totally-unknown: 1) {
                button { background-color: #0000ff; }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "button",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );

    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.r, 255);
    assert!(styles.get("background-color").is_none());
}

#[test]
fn test_textual_or_media_query_evaluates() {
    let css = r#"
            button { color: #ff0000; }
            @media (max-width: 100px) or (prefers-color-scheme: light) {
                button { color: #00ff00; }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "button",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );

    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.g, 255);
}

#[test]
fn test_load_path_with_imports_applies_import_qualifiers() {
    let dir = tempdir().unwrap();
    let import_path = dir.path().join("imported.css");
    let root_true = dir.path().join("root-true.css");
    let root_false = dir.path().join("root-false.css");

    fs::write(&import_path, "button { background-color: #0000ff; }").unwrap();
    fs::write(
        &root_true,
        "@import \"imported.css\" supports(display: grid) screen; button { color: #ff0000; }",
    )
    .unwrap();
    fs::write(
        &root_false,
        "@import \"imported.css\" supports(display: definitely-not-real) screen; button { color: #ff0000; }",
    )
    .unwrap();

    let true_sheet = StyleSheet::load_path_with_imports(&root_true).unwrap();
    let true_styles = true_sheet.compute_styles_with_environment(
        "button",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    assert_eq!(
        true_styles
            .get("background-color")
            .unwrap()
            .as_color()
            .unwrap()
            .b,
        255
    );

    let false_sheet = StyleSheet::load_path_with_imports(&root_false).unwrap();
    let false_styles = false_sheet.compute_styles_with_environment(
        "button",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    assert!(false_styles.get("background-color").is_none());
}

#[test]
fn test_important_beats_higher_specificity_normal() {
    // TODO 13: a low-specificity element rule with !important beats a
    // higher-specificity (class) normal rule.
    let css = r#"
            button { color: #ff0000 !important; }
            button.primary { color: #00ff00; }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "button",
        &["primary".to_string()],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.r, 255, "!important element rule should win");
    assert_eq!(color.g, 0);
    assert!(styles.is_important("color"));
}

#[test]
fn test_important_same_specificity_later_wins() {
    // TODO 13: two same-specificity !important rules — later source order wins.
    let css = r#"
            button { color: #ff0000 !important; }
            button { color: #00ff00 !important; }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "button",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.g, 255, "later !important should win on equal specificity");
}

#[test]
fn test_important_not_clobbered_by_later_normal() {
    // TODO 13: a later higher-specificity NORMAL declaration must not clear or
    // override an earlier !important one.
    let css = r#"
            #id-rule { color: #00ff00; }
            button { color: #ff0000 !important; }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "button",
        &[],
        Some("id-rule"),
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.r, 255, "!important must survive a higher-specificity normal rule");
    assert!(styles.is_important("color"));
}

// ── var() custom property resolution ────────────────────────────────────────

#[test]
fn test_var_resolves_root_custom_property() {
    // A custom property declared on :root must be substituted into a consuming
    // declaration during the cascade. The computed `color` must be the
    // substituted value (#ff0000), not the literal `var(--accent)` token.
    let css = r#"
            :root { --accent: #ff0000; }
            foo { color: var(--accent); }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles
        .get("color")
        .unwrap_or_else(|| panic!("color must be present and resolved"))
        .as_color()
        .unwrap_or_else(|| panic!("color must resolve to a Color, got {:?}", styles.get("color")));
    assert_eq!(color.r, 255, "var(--accent) should resolve to #ff0000");
    assert_eq!(color.g, 0);
    assert_eq!(color.b, 0);
}

#[test]
fn test_var_fallback_used_when_missing() {
    // var(--missing, #00ff00) must resolve to the fallback when the custom
    // property is undefined.
    let css = r#"
            foo { color: var(--missing, #00ff00); }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles
        .get("color")
        .unwrap_or_else(|| panic!("color must be present"))
        .as_color()
        .unwrap_or_else(|| panic!("fallback must resolve to a Color, got {:?}", styles.get("color")));
    assert_eq!(color.g, 255, "fallback #00ff00 should be used");
    assert_eq!(color.r, 0);
    assert_eq!(color.b, 0);
}

#[test]
fn test_var_defined_value_wins_over_fallback() {
    // When the custom property IS defined, the fallback must be ignored.
    let css = r#"
            :root { --accent: #ff0000; }
            foo { color: var(--accent, #00ff00); }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.r, 255, "defined --accent should win over fallback");
    assert_eq!(color.g, 0);
}

#[test]
fn test_var_unresolved_no_fallback_leaves_property_unset() {
    // var(--missing) with no fallback and no definition is an invalid value:
    // the consuming declaration must NOT yield a usable color (it should be
    // dropped / left unresolved rather than presented as a literal token).
    let css = r#"
            foo { color: var(--missing); }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    // The property must not surface as a parseable color, and must never be the
    // literal `var(--missing)` token.
    if let Some(value) = styles.get("color") {
        assert!(
            value.as_color().is_none(),
            "unresolved var with no fallback must not produce a color"
        );
        assert_ne!(
            value.to_css_string(),
            "var(--missing)",
            "unresolved var token must not leak through as a literal value"
        );
    }
}

#[test]
fn test_var_with_important_resolves_and_keeps_importance() {
    // Theme-customization seam: an `!important` declaration whose value is a
    // var() reference must (a) resolve the custom property AND (b) keep its
    // `!important` flag through the cascade so it still beats normal rules.
    let css = r#"
            :root { --accent: #ff0000; }
            foo { color: #00ff00; }
            foo { color: var(--accent) !important; }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles
        .get("color")
        .unwrap_or_else(|| panic!("color must be present"))
        .as_color()
        .unwrap_or_else(|| {
            panic!(
                "var(--accent) !important must resolve to a Color, got {:?}",
                styles.get("color")
            )
        });
    assert_eq!(color.r, 255, "var(--accent) !important should resolve to #ff0000");
    assert_eq!(color.g, 0);
    assert!(
        styles.is_important("color"),
        "importance must survive var() substitution"
    );
}

#[test]
fn test_var_resolves_to_custom_property_on_same_element() {
    // A custom property declared on the SAME matched rule scope is usable by a
    // sibling declaration via var().
    let css = r#"
            foo { --local: #0000ff; background-color: var(--local); }
        "#;
    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let styles = sheet.compute_styles_with_environment(
        "foo",
        &[],
        None,
        &[],
        &QueryEnvironment::default(),
    );
    let color = styles
        .get("background-color")
        .unwrap_or_else(|| panic!("background-color must be present"))
        .as_color()
        .unwrap_or_else(|| {
            panic!(
                "var(--local) must resolve to a Color, got {:?}",
                styles.get("background-color")
            )
        });
    assert_eq!(color.b, 255, "var(--local) should resolve to #0000ff");
}
