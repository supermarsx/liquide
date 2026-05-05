use super::*;
use crate::error::ThemeError;
use crate::value::{
    CssMathExpr, Gradient, GradientPositionComponent, GradientStopPosition, HorizontalGradientSide,
    LengthUnit, PropertyValue, RadialGradientExtent, RadialGradientShape, VerticalGradientSide,
};

fn parse_background_gradient(css_value: &str) -> lightningcss::values::gradient::Gradient {
    use lightningcss::properties::Property;
    use lightningcss::rules::CssRule;
    use lightningcss::stylesheet::{ParserFlags, ParserOptions, StyleSheet as LightningStyleSheet};
    use lightningcss::values::image::Image;

    let css = format!("window {{ background: {}; }}", css_value);
    let sheet = LightningStyleSheet::parse(
        &css,
        ParserOptions {
            filename: "test.css".into(),
            flags: ParserFlags::NESTING,
            ..ParserOptions::default()
        },
    )
    .unwrap();

    let style_rule = match &sheet.rules.0[0] {
        CssRule::Style(rule) => rule,
        other => panic!("expected style rule, got {:?}", other),
    };

    match &style_rule.declarations.declarations[0] {
        Property::Background(backgrounds) => match &backgrounds[0].image {
            Image::Gradient(gradient) => *gradient.clone(),
            other => panic!("expected gradient background image, got {:?}", other),
        },
        other => panic!("expected background property, got {:?}", other),
    }
}

fn serialize_custom_property_value(css_value: &str) -> String {
    use lightningcss::properties::Property;
    use lightningcss::rules::CssRule;
    use lightningcss::stylesheet::{ParserFlags, ParserOptions, StyleSheet as LightningStyleSheet};

    let css = format!(":root {{ --probe: {}; }}", css_value);
    let sheet = LightningStyleSheet::parse(
        &css,
        ParserOptions {
            filename: "test.css".into(),
            flags: ParserFlags::NESTING,
            ..ParserOptions::default()
        },
    )
    .unwrap();

    let style_rule = match &sheet.rules.0[0] {
        CssRule::Style(rule) => rule,
        other => panic!("expected style rule, got {:?}", other),
    };

    let parser = ThemeParser::new();
    match &style_rule.declarations.declarations[0] {
        Property::Custom(custom) => parser.to_css_string_from_token_list(&custom.value),
        other => panic!("expected custom property, got {:?}", other),
    }
}

#[test]
fn test_parse_simple() {
    let css = r#"
            button {
                background: #ff0000;
                width: 100px;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.rule_count(), 1);
}

#[test]
fn test_parse_multiple_rules() {
    let css = r#"
            button {
                background: #ff0000;
            }

            window {
                border: 1px;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.rule_count(), 2);
}

#[test]
fn test_parse_with_comments() {
    let css = r#"
            /* This is a comment */
            button {
                background: #ff0000;
                /* Another comment */
                width: 100px;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.rule_count(), 1);
}

#[test]
fn test_parse_pseudo_classes() {
    let css = r#"
            button:hover {
                background: #00ff00;
            }
        "#;

    let parser = ThemeParser::new();
    let result = parser.parse_str(css);

    assert!(result.is_ok());
}

#[test]
fn test_parse_rgba_colors() {
    let css = r#"
            window {
                background: rgba(255, 0, 0, 0.5);
            }
        "#;

    let parser = ThemeParser::new();
    let result = parser.parse_str(css);

    assert!(result.is_ok());
}

#[test]
fn test_parse_css_variables() {
    let css = r#"
            :root {
                --primary: #5e81ac;
            }

            button {
                background: var(--primary);
            }
        "#;

    let parser = ThemeParser::new();
    let result = parser.parse_str(css);

    assert!(result.is_ok());
}

#[test]
fn test_parse_supports_rules_preserved() {
    let css = r#"
            @supports (display: grid) {
                button { color: #00ff00; }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    assert_eq!(sheet.rule_count(), 1);
    assert!(
        sheet.rules()[0]
            .supports_condition
            .as_deref()
            .unwrap_or("")
            .contains("display")
    );
}

#[test]
fn test_parse_media_supports_combined() {
    let css = r#"
            @media (max-width: 600px) {
                @supports (display: grid) {
                    button { color: #00ff00; }
                }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    assert_eq!(sheet.rule_count(), 1);
    let rule = &sheet.rules()[0];
    assert!(rule.media_condition.is_some());
    assert!(
        rule.supports_condition
            .as_deref()
            .unwrap_or("")
            .contains("display")
    );
}

#[test]
fn test_parse_import_metadata_preserved() {
    let css = r#"
            @import "shared.css" layer(theme) supports(display: grid) screen and (min-width: 600px);
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.imports().len(), 1);
    let import = &sheet.imports()[0];
    assert_eq!(import.url, "shared.css");
    assert_eq!(
        import.layer,
        Some(crate::stylesheet::ImportLayer::Named("theme".to_string()))
    );
    assert!(
        import
            .supports_condition
            .as_deref()
            .unwrap_or("")
            .contains("display")
    );
    assert!(import.media_condition.is_some());
}

#[test]
fn test_parse_anonymous_layer_gets_internal_identity() {
    let css = r#"
            @layer {
                button { color: #ff0000; }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.layer_order().len(), 1);
    let layer_name = &sheet.layer_order()[0];
    assert!(layer_name.starts_with("__liquide_anon_layer__"));
    assert_eq!(sheet.rules()[0].layer.as_deref(), Some(layer_name.as_str()));
}

#[test]
fn test_parse_nested_container_contents_preserved() {
    let css = r#"
            @container (min-width: 10px) {
                @supports (display: flex) {
                    button { color: #00ff00; }
                }
                @container sidebar (min-width: 20px) {
                    button { background-color: #0000ff; }
                }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();

    assert_eq!(sheet.container_rules().len(), 2);
    assert!(sheet.container_rules().iter().any(|rule| {
        rule.rules.iter().any(|style_rule| {
            style_rule
                .supports_condition
                .as_deref()
                .unwrap_or("")
                .contains("display")
        })
    }));
    assert!(sheet.container_rules().iter().any(|rule| {
        rule.name.as_deref() == Some(crate::stylesheet::STRUCTURAL_CONDITION_SENTINEL)
    }));
}

#[test]
fn test_custom_property_tokens_round_trip_as_css() {
    let css = r#"
            :root {
                --safe: env(safe-area-inset-top, 10px);
                --icon: url("foo bar.svg");
                --accent: rgb(255 0 0 / var(--alpha));
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let rule = &sheet.rules()[0];

    let safe = rule.properties.get("--safe").unwrap();
    assert!(matches!(safe, PropertyValue::Env(inner) if inner == "safe-area-inset-top, 10px"));
    assert_eq!(safe.to_css_string(), "env(safe-area-inset-top, 10px)");

    let icon = rule.properties.get("--icon").unwrap();
    assert!(matches!(icon, PropertyValue::Url(inner) if inner == "\"foo bar.svg\""));
    assert_eq!(icon.to_css_string(), "url(\"foo bar.svg\")");

    let accent = rule.properties.get("--accent").unwrap();
    assert!(matches!(accent, PropertyValue::Keyword(raw) if raw == "rgb(255 0 0 / var(--alpha))"));
    assert_eq!(accent.to_css_string(), "rgb(255 0 0 / var(--alpha))");

    assert_eq!(
        serialize_custom_property_value("rgb(255 0 0 / var(--alpha))"),
        "rgb(255 0 0 / var(--alpha))"
    );
}

#[test]
fn test_unitless_numbers_and_nested_math_parse_correctly() {
    let parser = ThemeParser::new();

    assert_eq!(
        parser.parse_value_string("1.25"),
        PropertyValue::Number(1.25)
    );

    let division_value = parser.parse_value_string("calc(10px / 2)");
    assert_eq!(division_value.resolve_px(16.0, 1000.0, 800.0), Some(5.0));

    let min_value = parser.parse_value_string("min(100% - 2rem, 50vw)");
    assert_eq!(min_value.to_css_string(), "min(100% - 2rem, 50vw)");
    match min_value {
        PropertyValue::MathExpr(CssMathExpr::Min(exprs)) => {
            assert_eq!(exprs.len(), 2);
            assert!(matches!(
                &exprs[0],
                CssMathExpr::Sub(left, right)
                    if matches!(
                        (&**left, &**right),
                        (
                            CssMathExpr::Value(LengthUnit::Percent(100.0)),
                            CssMathExpr::Value(LengthUnit::Rem(2.0))
                        )
                    )
            ));
            assert!(matches!(exprs[1], CssMathExpr::Value(LengthUnit::Vw(50.0))));
        }
        other => panic!("expected min() expression, got {:?}", other),
    }

    let clamp_value = parser.parse_value_string("clamp(1, 50% + 2px, 20rem)");
    assert_eq!(clamp_value.to_css_string(), "clamp(1, 50% + 2px, 20rem)");
    match clamp_value {
        PropertyValue::MathExpr(CssMathExpr::Clamp {
            min,
            preferred,
            max,
        }) => {
            assert!(matches!(*min, CssMathExpr::Number(value) if value == 1.0));
            assert!(matches!(
                *preferred,
                CssMathExpr::Add(left, right)
                    if matches!(
                        (&*left, &*right),
                        (
                            CssMathExpr::Value(LengthUnit::Percent(50.0)),
                            CssMathExpr::Value(LengthUnit::Px(2.0))
                        )
                    )
            ));
            assert!(matches!(*max, CssMathExpr::Value(LengthUnit::Rem(20.0))));
        }
        other => panic!("expected clamp expression, got {:?}", other),
    }
}

#[test]
fn test_empty_math_functions_are_rejected() {
    let parser = ThemeParser::new();

    assert!(parser.parse_math_expr("min()").is_none());
    assert!(parser.parse_math_expr("max(, 1px)").is_none());
    assert_eq!(CssMathExpr::Min(vec![]).resolve(16.0, 1920.0, 1080.0), 0.0);
    assert_eq!(CssMathExpr::Max(vec![]).resolve(16.0, 1920.0, 1080.0), 0.0);
}

#[test]
fn test_radial_gradient_geometry_and_stop_units_are_preserved() {
    let parser = ThemeParser::new();
    let lightning_gradient = parse_background_gradient(
        "radial-gradient(circle closest-side at right 10px bottom 20%, #ff0000 10%, #0000ff 24px)",
    );
    let value = parser.convert_gradient(&lightning_gradient).unwrap();

    match &value {
        Gradient::Radial {
            shape,
            position,
            stops,
        } => {
            assert_eq!(
                shape,
                &RadialGradientShape::Circle {
                    radius: None,
                    extent: Some(RadialGradientExtent::ClosestSide),
                }
            );
            assert_eq!(
                position.x,
                GradientPositionComponent::Side {
                    side: HorizontalGradientSide::Right,
                    offset: Some(LengthUnit::Px(10.0)),
                }
            );
            assert_eq!(
                position.y,
                GradientPositionComponent::Side {
                    side: VerticalGradientSide::Bottom,
                    offset: Some(LengthUnit::Percent(20.0)),
                }
            );
            assert_eq!(
                stops[0].position,
                Some(GradientStopPosition::Length(LengthUnit::Percent(10.0)))
            );
            assert_eq!(
                stops[1].position,
                Some(GradientStopPosition::Length(LengthUnit::Px(24.0)))
            );
        }
        other => panic!("expected radial gradient, got {:?}", other),
    }

    assert_eq!(
        value.to_string(),
        "radial-gradient(circle closest-side at right 10px bottom 20%, #ff0000 10%, #0000ff 24px)"
    );
}

#[test]
fn test_conic_gradients_normalize_angles_and_stop_units() {
    let parser = ThemeParser::new();
    let lightning_gradient = parse_background_gradient(
        "conic-gradient(from 0.5turn at 30% 70%, #ff0000 25%, #0000ff 200grad)",
    );
    let value = parser.convert_gradient(&lightning_gradient).unwrap();

    match &value {
        Gradient::Conic {
            from_angle,
            position,
            stops,
        } => {
            assert_eq!(*from_angle, 180.0);
            assert_eq!(
                position.x,
                GradientPositionComponent::Value(LengthUnit::Percent(30.0))
            );
            assert_eq!(
                position.y,
                GradientPositionComponent::Value(LengthUnit::Percent(70.0))
            );
            assert_eq!(stops[0].position, Some(GradientStopPosition::Angle(90.0)));
            assert_eq!(stops[1].position, Some(GradientStopPosition::Angle(180.0)));
        }
        other => panic!("expected conic gradient, got {:?}", other),
    }

    assert_eq!(ThemeParser::parse_angle_string("200grad"), 180.0);
    assert_eq!(
        value.to_string(),
        "conic-gradient(from 180deg at 30% 70%, #ff0000 90deg, #0000ff 180deg)"
    );
}

#[test]
fn test_parse_errors_preserve_source_location() {
    let parser = ThemeParser::new();
    let err = parser.parse_str("button {\n  color: red;\n}}").unwrap_err();

    match err {
        ThemeError::ParseError { location, .. } => {
            assert_ne!(location, "unknown");
            assert!(
                location.starts_with("<inline>:"),
                "unexpected location: {location}"
            );
        }
        other => panic!("expected parse error, got {:?}", other),
    }
}

#[test]
fn test_shorthand_aliases_preserve_multi_value_structure() {
    let css = r#"
        button {
            padding: 1px 2px;
            overflow: hidden scroll;
            gap: 4px 8px;
            border-color: red blue;
            border-top: 1px solid red;
        }
    "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let props = &sheet.rules()[0].properties;

    assert_eq!(
        props.get("padding"),
        Some(&PropertyValue::List(vec![
            PropertyValue::Length(LengthUnit::Px(1.0)),
            PropertyValue::Length(LengthUnit::Px(2.0)),
        ]))
    );
    assert_eq!(
        props.get("overflow"),
        Some(&PropertyValue::List(vec![
            PropertyValue::Keyword("hidden".into()),
            PropertyValue::Keyword("scroll".into()),
        ]))
    );
    assert_eq!(
        props.get("gap"),
        Some(&PropertyValue::List(vec![
            PropertyValue::Length(LengthUnit::Px(4.0)),
            PropertyValue::Length(LengthUnit::Px(8.0)),
        ]))
    );

    match props.get("border-color") {
        Some(PropertyValue::List(items)) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].as_color().unwrap().r, 255);
            assert_eq!(items[1].as_color().unwrap().b, 255);
        }
        other => panic!("expected canonical border-color list, got {other:?}"),
    }

    assert_eq!(
        props
            .get("border-top-style")
            .and_then(PropertyValue::as_string),
        Some("solid")
    );
}

#[test]
fn test_parser_preserves_layered_background_and_raw_shorthand_text() {
    let css = r#"
        button {
            background: url(bg.png) center/cover no-repeat, linear-gradient(red, blue);
            font: italic 700 16px/1.4 "Fira Sans", sans-serif;
            animation: fade 1s steps(4, end);
        }
    "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let props = &sheet.rules()[0].properties;

    let background = props
        .get("background")
        .and_then(PropertyValue::as_string)
        .unwrap();
    assert!(background.contains("url("));
    assert!(background.contains("linear-gradient("));

    let background_image = props
        .get("background-image")
        .and_then(PropertyValue::as_string)
        .unwrap();
    assert!(background_image.contains("url("));
    assert!(background_image.contains("linear-gradient("));

    let font = props
        .get("font")
        .and_then(PropertyValue::as_string)
        .unwrap();
    assert!(font.contains("16px"));
    assert!(font.contains("Fira Sans"));
    assert!(!font.contains("Font("));

    let animation = props
        .get("animation")
        .and_then(PropertyValue::as_string)
        .unwrap();
    assert!(animation.contains("steps(4, end)"));
    assert!(!animation.contains("Animation("));
}

#[test]
fn test_parser_converts_single_gradient_background_to_structured_value() {
    let css = r#"
        statusbar {
            background: linear-gradient(180deg, rgba(18, 22, 48, 0.88), rgba(12, 16, 38, 0.82));
        }
    "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let props = &sheet.rules()[0].properties;

    assert!(matches!(
        props.get("background"),
        Some(PropertyValue::Gradient(Gradient::Linear { stops, .. })) if stops.len() == 2
    ));
    assert!(matches!(
        props.get("background-image"),
        Some(PropertyValue::Gradient(Gradient::Linear { stops, .. })) if stops.len() == 2
    ));
}
