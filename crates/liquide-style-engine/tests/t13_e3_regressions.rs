use liquide_style_engine::expand_shorthand;
use liquide_theme_css::value::{LengthUnit, PropertyValue};

fn keyword(value: &str) -> PropertyValue {
    PropertyValue::Keyword(value.to_string())
}

#[test]
fn transition_and_animation_splitting_stays_token_aware() {
    let transition = expand_shorthand(
        "transition",
        &keyword("opacity 200ms cubic-bezier(0.1, 0.2, 0.3, 0.4), transform 300ms steps(4, end)"),
    )
    .unwrap();
    assert_eq!(
        transition[0],
        ("transition-property", keyword("opacity, transform"))
    );
    assert_eq!(
        transition[1],
        ("transition-duration", keyword("200ms, 300ms"))
    );
    assert_eq!(
        transition[2],
        (
            "transition-timing-function",
            keyword("cubic-bezier(0.1, 0.2, 0.3, 0.4), steps(4, end)"),
        )
    );

    let animation = expand_shorthand(
        "animation",
        &keyword("fade 1s steps(4, end), slide 2s cubic-bezier(0.2, 0.4, 0.6, 1)"),
    )
    .unwrap();
    assert_eq!(animation[0], ("animation-name", keyword("fade, slide")));
    assert_eq!(animation[1], ("animation-duration", keyword("1s, 2s")));
    assert_eq!(
        animation[2],
        (
            "animation-timing-function",
            keyword("steps(4, end), cubic-bezier(0.2, 0.4, 0.6, 1)"),
        )
    );
}

#[test]
fn font_shorthand_and_background_layers_are_preserved() {
    let font = expand_shorthand(
        "font",
        &keyword("italic 700 16px/1.4 \"Fira Sans\", sans-serif"),
    )
    .unwrap();
    assert!(font.contains(&("font-style", keyword("italic"))));
    assert!(font.contains(&("font-weight", PropertyValue::Number(700.0))));
    assert!(font.contains(&("font-size", PropertyValue::Length(LengthUnit::Px(16.0)),)));
    assert!(font.contains(&("line-height", PropertyValue::Number(1.4))));
    assert!(font.contains(&(
        "font-family",
        PropertyValue::String("\"Fira Sans\", sans-serif".into()),
    )));

    let background = expand_shorthand(
        "background",
        &keyword("url(bg.png) center/cover no-repeat, linear-gradient(red, blue)"),
    )
    .unwrap();
    assert_eq!(background.len(), 1);
    assert_eq!(
        background[0],
        (
            "background-image",
            keyword("url(bg.png), linear-gradient(red, blue)"),
        )
    );
}

#[test]
fn slash_sensitive_shorthands_ignore_nested_url_contents() {
    let mask = expand_shorthand(
        "mask",
        &keyword("url(data:image/svg+xml;base64,AAAA) center / contain no-repeat"),
    )
    .unwrap();
    assert_eq!(
        mask[0],
        ("mask-image", keyword("url(data:image/svg+xml;base64,AAAA)"),)
    );
    assert_eq!(mask[3], ("mask-size", keyword("contain")));
    assert_eq!(mask[4], ("mask-repeat", keyword("no-repeat")));
}
