use std::fs;

use crate::error::ThemeError;
use crate::parser::ThemeParser;
use crate::stylesheet::{QueryEnvironment, StyleSheet};
use crate::value::{CssMathExpr, LengthUnit, PropertyValue};

#[path = "../../../liquide-conformance/src/css.rs"]
mod shared_css;

use shared_css::{
    THEME_CSS_PARSER_FIXTURES, THEME_CSS_STYLESHEET_FIXTURES, ThemeCssParserScenario,
    ThemeCssStylesheetScenario,
};

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
fn css_parser_conformance_fixtures() {
    let parser = ThemeParser::new();

    for fixture in THEME_CSS_PARSER_FIXTURES {
        match fixture.scenario {
            ThemeCssParserScenario::CustomPropertyRoundTrip => {
                let serialized = serialize_custom_property_value(fixture.source);
                assert!(
                    serialized.starts_with("env(titlebar-area-x, 0"),
                    "{} {} -> {}",
                    fixture.meta.id,
                    fixture.meta.title,
                    serialized
                );
                assert!(
                    serialized.contains("url(\"foo bar.svg\")"),
                    "{} {} -> {}",
                    fixture.meta.id,
                    fixture.meta.title,
                    serialized
                );
                assert!(
                    serialized.contains("rgb(255 0 0 / var(--alpha))"),
                    "{} {} -> {}",
                    fixture.meta.id,
                    fixture.meta.title,
                    serialized
                );
            }
            ThemeCssParserScenario::NestedMathPreservesStructure => {
                assert_eq!(
                    parser.parse_value_string("1.25"),
                    PropertyValue::Number(1.25),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                let value = parser.parse_value_string(fixture.source);
                assert_eq!(
                    value.to_css_string(),
                    fixture.source,
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                match value {
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
                    other => panic!(
                        "{} {} expected clamp expression, got {:?}",
                        fixture.meta.id, fixture.meta.title, other
                    ),
                }
            }
            ThemeCssParserScenario::EmptyMathRejected => {
                assert!(
                    parser.parse_math_expr(fixture.source).is_none(),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    parser.parse_math_expr("max(, 1px)").is_none(),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
            }
            ThemeCssParserScenario::ShorthandTokensPreserved => {
                let sheet = parser.parse_str(fixture.source).unwrap();
                let props = &sheet.rules()[0].properties;

                let background = props
                    .get("background")
                    .and_then(PropertyValue::as_string)
                    .unwrap();
                assert!(
                    background.contains("url("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    background.contains("linear-gradient("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                let background_image = props
                    .get("background-image")
                    .and_then(PropertyValue::as_string)
                    .unwrap();
                assert!(
                    background_image.contains("url("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    background_image.contains("linear-gradient("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                let font = props
                    .get("font")
                    .and_then(PropertyValue::as_string)
                    .unwrap();
                assert!(
                    font.contains("16px"),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    font.contains("Fira Sans"),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    !font.contains("Font("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                let animation = props
                    .get("animation")
                    .and_then(PropertyValue::as_string)
                    .unwrap();
                assert!(
                    animation.contains("steps(4, end)"),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    !animation.contains("Animation("),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
            }
        }
    }
}

#[test]
fn css_stylesheet_conformance_fixtures() {
    let parser = ThemeParser::new();

    for fixture in THEME_CSS_STYLESHEET_FIXTURES {
        match fixture.scenario {
            ThemeCssStylesheetScenario::InvalidSupportsAndMediaFailClosed => {
                let sheet = parser.parse_str(fixture.source).unwrap();
                let styles = sheet.compute_styles_with_environment(
                    "button",
                    &[],
                    None,
                    &[],
                    &QueryEnvironment::default(),
                );

                let color = styles.get("color").unwrap().as_color().unwrap();
                assert_eq!(
                    (color.r, color.g, color.b),
                    (255, 0, 0),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
                assert!(
                    styles.get("background-color").is_none(),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
            }
            ThemeCssStylesheetScenario::ImportQualifiersRespected => {
                let dir = tempfile::tempdir().unwrap();
                let import_path = dir.path().join("imported.css");
                let root_true = dir.path().join("root-true.css");
                let root_false = dir.path().join("root-false.css");

                fs::write(&import_path, fixture.source).unwrap();
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
                    255,
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );

                let false_sheet = StyleSheet::load_path_with_imports(&root_false).unwrap();
                let false_styles = false_sheet.compute_styles_with_environment(
                    "button",
                    &[],
                    None,
                    &[],
                    &QueryEnvironment::default(),
                );
                assert!(
                    false_styles.get("background-color").is_none(),
                    "{} {}",
                    fixture.meta.id,
                    fixture.meta.title
                );
            }
        }
    }
}

#[test]
fn css_fixture_parse_errors_keep_source_locations() {
    let parser = ThemeParser::new();
    let err = parser.parse_str("button {\n  color: red;\n}}").unwrap_err();

    match err {
        ThemeError::ParseError { location, .. } => {
            assert_ne!(location, "unknown");
            assert!(location.starts_with("<inline>:"));
        }
        other => panic!("expected parse error, got {:?}", other),
    }
}
