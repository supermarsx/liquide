use super::*;
use crate::parser::ThemeParser;

#[test]
fn test_query() {
    let css = r#"
            button {
                background: #ff0000;
                width: 100px;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let engine = ThemeEngine::new(sheet);

    let styles = engine.query("button", &[], &[]).unwrap();
    assert!(styles.has("background"));
    assert!(styles.has("width"));
}

#[test]
fn test_get_property() {
    let css = r#"
            button {
                background: #ff0000;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let engine = ThemeEngine::new(sheet);

    let bg = engine.get_property("button", &[], &[], "background").unwrap();
    assert!(bg.is_some());

    if let Some(PropertyValue::Color(color)) = bg {
        assert_eq!(color.r, 255);
    } else {
        panic!("Expected color");
    }
}

#[test]
fn test_cascade() {
    let css = r#"
            button {
                background: #ff0000;
            }

            button.primary {
                background: #00ff00;
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let engine = ThemeEngine::new(sheet);

    let bg1 = engine
        .get_property("button", &[], &[], "background")
        .unwrap()
        .unwrap();
    if let PropertyValue::Color(color) = bg1 {
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
    }

    let bg2 = engine
        .get_property("button", &vec!["primary".to_string()], &[], "background")
        .unwrap()
        .unwrap();
    if let PropertyValue::Color(color) = bg2 {
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
    }
}

#[test]
fn test_query_with_environment_media_supports() {
    let css = r#"
            button { color: #ff0000; }
            @media (max-width: 600px) {
                button { color: #00ff00; }
            }
            @supports (display: grid) {
                button { background: #111111; }
            }
            @supports (nonexistent-property: 1) {
                button { background: #ffffff; }
            }
        "#;

    let parser = ThemeParser::new();
    let sheet = parser.parse_str(css).unwrap();
    let engine = ThemeEngine::new(sheet);

    let mut env = QueryEnvironment::default();
    env.viewport_width = 500.0;
    let styles = engine
        .query_with_environment("button", &[], &[], &env)
        .unwrap();
    let color = styles.get("color").unwrap().as_color().unwrap();
    assert_eq!(color.g, 255);
    let bg = styles.get("background").unwrap().as_color().unwrap();
    assert_eq!(bg.r, 17);

    env.viewport_width = 1200.0;
    let desktop = engine
        .query_with_environment("button", &[], &[], &env)
        .unwrap();
    let desktop_color = desktop.get("color").unwrap().as_color().unwrap();
    assert_eq!(desktop_color.r, 255);
}
