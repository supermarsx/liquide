use super::*;
use crate::value::Color;

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
