//! Value resolution — CSS PropertyValue → ComputedStyle field mapping.
//!
//! Bridges `liquide_theme_css::value::PropertyValue` to our computed types.

use liquide_compositor::pixel::Color;
use liquide_theme_css::value::PropertyValue;

use crate::computed::*;
use crate::dimension::Dimension;

/// Resolve a PropertyValue to a Dimension.
pub fn resolve_dimension(val: &PropertyValue) -> Dimension {
    match val {
        PropertyValue::Length(lu) => length_unit_to_dimension(lu),
        PropertyValue::Number(n) => Dimension::Px(*n),
        PropertyValue::Keyword(kw) => match kw.as_str() {
            "auto" => Dimension::Auto,
            "none" => Dimension::None,
            "min-content" => Dimension::MinContent,
            "max-content" => Dimension::MaxContent,
            _ => Dimension::Auto,
        },
        PropertyValue::MathExpr(expr) => {
            // Resolve statically
            let px = expr.resolve(16.0, 1920.0, 1080.0);
            Dimension::Px(px)
        }
        _ => Dimension::Auto,
    }
}

fn length_unit_to_dimension(lu: &liquide_theme_css::value::LengthUnit) -> Dimension {
    use liquide_theme_css::value::LengthUnit;
    match lu {
        LengthUnit::Px(v) => Dimension::Px(*v),
        LengthUnit::Pt(v) => Dimension::Px(*v * 1.333),
        LengthUnit::Em(v) => Dimension::Em(*v),
        LengthUnit::Rem(v) => Dimension::Rem(*v),
        LengthUnit::Percent(v) => Dimension::Percent(*v),
        LengthUnit::Vw(v) => Dimension::Vw(*v),
        LengthUnit::Vh(v) => Dimension::Vh(*v),
        LengthUnit::Vmin(v) => Dimension::Vmin(*v),
        LengthUnit::Vmax(v) => Dimension::Vmax(*v),
        LengthUnit::Ch(v) => Dimension::Ch(*v),
        LengthUnit::Ex(v) => Dimension::Em(*v * 0.5), // approximate
    }
}

/// Resolve a PropertyValue to a number (f32).
pub fn resolve_number(val: &PropertyValue) -> f32 {
    match val {
        PropertyValue::Number(n) => *n,
        PropertyValue::Length(lu) => lu.to_px(16.0),
        _ => 0.0,
    }
}

/// Resolve a PropertyValue to a Color.
pub fn resolve_color(val: &PropertyValue) -> Option<Color> {
    match val {
        PropertyValue::Color(c) => Some(Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }),
        _ => None,
    }
}

/// Resolve a keyword to a Display value.
pub fn resolve_display(val: &PropertyValue) -> Display {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "block" => Display::Block,
            "inline" => Display::Inline,
            "inline-block" => Display::InlineBlock,
            "flex" => Display::Flex,
            "inline-flex" => Display::InlineFlex,
            "grid" => Display::Grid,
            "inline-grid" => Display::InlineGrid,
            "none" => Display::None,
            "contents" => Display::Contents,
            _ => Display::Block,
        }
    } else {
        Display::Block
    }
}

/// Resolve a keyword to a Position value.
pub fn resolve_position(val: &PropertyValue) -> Position {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "static" => Position::Static,
            "relative" => Position::Relative,
            "absolute" => Position::Absolute,
            "fixed" => Position::Fixed,
            "sticky" => Position::Sticky,
            _ => Position::Static,
        }
    } else {
        Position::Static
    }
}

/// Resolve a keyword to FlexDirection.
pub fn resolve_flex_direction(val: &PropertyValue) -> FlexDirection {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "row" => FlexDirection::Row,
            "row-reverse" => FlexDirection::RowReverse,
            "column" => FlexDirection::Column,
            "column-reverse" => FlexDirection::ColumnReverse,
            _ => FlexDirection::Row,
        }
    } else {
        FlexDirection::Row
    }
}

/// Resolve a keyword to FlexWrap.
pub fn resolve_flex_wrap(val: &PropertyValue) -> FlexWrap {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "nowrap" => FlexWrap::NoWrap,
            "wrap" => FlexWrap::Wrap,
            "wrap-reverse" => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        }
    } else {
        FlexWrap::NoWrap
    }
}

/// Resolve a keyword to JustifyContent.
pub fn resolve_justify_content(val: &PropertyValue) -> JustifyContent {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "flex-start" | "start" => JustifyContent::FlexStart,
            "flex-end" | "end" => JustifyContent::FlexEnd,
            "center" => JustifyContent::Center,
            "space-between" => JustifyContent::SpaceBetween,
            "space-around" => JustifyContent::SpaceAround,
            "space-evenly" => JustifyContent::SpaceEvenly,
            _ => JustifyContent::FlexStart,
        }
    } else {
        JustifyContent::FlexStart
    }
}

/// Resolve a keyword to AlignItems.
pub fn resolve_align_items(val: &PropertyValue) -> AlignItems {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "flex-start" | "start" => AlignItems::FlexStart,
            "flex-end" | "end" => AlignItems::FlexEnd,
            "center" => AlignItems::Center,
            "baseline" => AlignItems::Baseline,
            "stretch" => AlignItems::Stretch,
            _ => AlignItems::Stretch,
        }
    } else {
        AlignItems::Stretch
    }
}

/// Resolve a keyword to Overflow.
pub fn resolve_overflow(val: &PropertyValue) -> liquide_compositor::scene::Overflow {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "visible" => liquide_compositor::scene::Overflow::Visible,
            "hidden" => liquide_compositor::scene::Overflow::Hidden,
            "scroll" => liquide_compositor::scene::Overflow::Scroll,
            "auto" => liquide_compositor::scene::Overflow::Auto,
            _ => liquide_compositor::scene::Overflow::Visible,
        }
    } else {
        liquide_compositor::scene::Overflow::Visible
    }
}

/// Resolve a keyword to Visibility.
pub fn resolve_visibility(val: &PropertyValue) -> Visibility {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "visible" => Visibility::Visible,
            "hidden" => Visibility::Hidden,
            "collapse" => Visibility::Collapse,
            _ => Visibility::Visible,
        }
    } else {
        Visibility::Visible
    }
}

/// Resolve font-weight from a number or keyword.
pub fn resolve_font_weight(val: &PropertyValue) -> u16 {
    match val {
        PropertyValue::Number(n) => *n as u16,
        PropertyValue::Keyword(kw) => match kw.as_str() {
            "normal" => 400,
            "bold" => 700,
            "lighter" => 300,
            "bolder" => 700,
            _ => 400,
        },
        _ => 400,
    }
}

/// Resolve a keyword to TextAlign.
pub fn resolve_text_align(val: &PropertyValue) -> TextAlign {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            "center" => TextAlign::Center,
            "justify" => TextAlign::Justify,
            "start" => TextAlign::Start,
            "end" => TextAlign::End,
            _ => TextAlign::Start,
        }
    } else {
        TextAlign::Start
    }
}

/// Resolve a keyword to WhiteSpace.
pub fn resolve_white_space(val: &PropertyValue) -> WhiteSpace {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "normal" => WhiteSpace::Normal,
            "nowrap" => WhiteSpace::NoWrap,
            "pre" => WhiteSpace::Pre,
            "pre-wrap" => WhiteSpace::PreWrap,
            "pre-line" => WhiteSpace::PreLine,
            "break-spaces" => WhiteSpace::BreakSpaces,
            _ => WhiteSpace::Normal,
        }
    } else {
        WhiteSpace::Normal
    }
}

/// Resolve a keyword to Cursor.
pub fn resolve_cursor(val: &PropertyValue) -> Cursor {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "auto" => Cursor::Auto,
            "default" => Cursor::Default,
            "pointer" => Cursor::Pointer,
            "text" => Cursor::Text,
            "move" => Cursor::Move,
            "crosshair" => Cursor::Crosshair,
            "wait" => Cursor::Wait,
            "help" => Cursor::Help,
            "not-allowed" => Cursor::NotAllowed,
            "grab" => Cursor::Grab,
            "grabbing" => Cursor::Grabbing,
            _ => Cursor::Auto,
        }
    } else {
        Cursor::Auto
    }
}

/// Resolve a keyword to AlignSelf.
pub fn resolve_align_self(val: &PropertyValue) -> AlignSelf {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "auto" => AlignSelf::Auto,
            "flex-start" | "start" => AlignSelf::FlexStart,
            "flex-end" | "end" => AlignSelf::FlexEnd,
            "center" => AlignSelf::Center,
            "baseline" => AlignSelf::Baseline,
            "stretch" => AlignSelf::Stretch,
            _ => AlignSelf::Auto,
        }
    } else {
        AlignSelf::Auto
    }
}

/// Resolve a keyword to AlignContent.
pub fn resolve_align_content(val: &PropertyValue) -> AlignContent {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "flex-start" | "start" => AlignContent::FlexStart,
            "flex-end" | "end" => AlignContent::FlexEnd,
            "center" => AlignContent::Center,
            "stretch" => AlignContent::Stretch,
            "space-between" => AlignContent::SpaceBetween,
            "space-around" => AlignContent::SpaceAround,
            _ => AlignContent::Stretch,
        }
    } else {
        AlignContent::Stretch
    }
}

/// Resolve a keyword/value to BorderLineStyle.
pub fn resolve_border_style(val: &PropertyValue) -> BorderLineStyle {
    if let PropertyValue::Keyword(kw) = val {
        match kw.as_str() {
            "none" => BorderLineStyle::None,
            "solid" => BorderLineStyle::Solid,
            "dashed" => BorderLineStyle::Dashed,
            "dotted" => BorderLineStyle::Dotted,
            "double" => BorderLineStyle::Double,
            "groove" => BorderLineStyle::Groove,
            "ridge" => BorderLineStyle::Ridge,
            "inset" => BorderLineStyle::Inset,
            "outset" => BorderLineStyle::Outset,
            "hidden" => BorderLineStyle::Hidden,
            _ => BorderLineStyle::None,
        }
    } else if let PropertyValue::BorderStyle(bs) = val {
        match bs {
            liquide_theme_css::value::BorderStyle::None => BorderLineStyle::None,
            liquide_theme_css::value::BorderStyle::Solid => BorderLineStyle::Solid,
            liquide_theme_css::value::BorderStyle::Dashed => BorderLineStyle::Dashed,
            liquide_theme_css::value::BorderStyle::Dotted => BorderLineStyle::Dotted,
            liquide_theme_css::value::BorderStyle::Double => BorderLineStyle::Double,
            liquide_theme_css::value::BorderStyle::Groove => BorderLineStyle::Groove,
            liquide_theme_css::value::BorderStyle::Ridge => BorderLineStyle::Ridge,
            liquide_theme_css::value::BorderStyle::Inset => BorderLineStyle::Inset,
            liquide_theme_css::value::BorderStyle::Outset => BorderLineStyle::Outset,
            liquide_theme_css::value::BorderStyle::Hidden => BorderLineStyle::Hidden,
        }
    } else {
        BorderLineStyle::None
    }
}
