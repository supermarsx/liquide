use super::*;

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
