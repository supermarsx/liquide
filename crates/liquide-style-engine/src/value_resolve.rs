//! Value resolution — CSS PropertyValue → ComputedStyle field mapping.
//!
//! Bridges `liquide_theme_css::value::PropertyValue` to our computed types.

use liquide_compositor::pixel::Color;
use liquide_theme_css::value::PropertyValue;

use crate::computed::*;
use crate::dimension::{CalcExpr, Dimension};

/// Parse an inline style string value into a PropertyValue.
///
/// Supports common inline patterns like "100", "100px", "auto", "#rgb".
pub fn parse_inline_value(value: &str) -> PropertyValue {
    let value = value.trim();

    // Try numeric (with optional unit)
    if let Some(px) = try_parse_px(value) {
        return PropertyValue::Number(px);
    }

    // Try color
    if let Some(c) = try_parse_color(value) {
        return PropertyValue::Color(liquide_theme_css::value::Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        });
    }

    // Keyword fallback
    PropertyValue::Keyword(value.to_string())
}

/// Try to parse a px value like "100" or "100px" or "50.5".
fn try_parse_px(value: &str) -> Option<f32> {
    let v = value.strip_suffix("px").unwrap_or(value);
    v.parse::<f32>().ok()
}

/// Try to parse a color value.
fn try_parse_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    // `currentColor` depends on the resolved `color` value of the same element
    // and needs per-property context, so we do not resolve it here.
    if trimmed.eq_ignore_ascii_case("currentcolor") {
        return None;
    }

    liquide_theme_css::value::Color::parse_css(trimmed)
        .ok()
        .map(|c| Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        })
}

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
            "fit-content" => Dimension::FitContent(Box::new(Dimension::Auto)),
            _ => Dimension::Auto,
        },
        PropertyValue::MathExpr(expr) => {
            // Convert to deferred CalcExpr for lazy resolution at layout time
            Dimension::Calc(Box::new(math_expr_to_calc(expr)))
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
        // Dynamic viewport units — map to distinct Dimension variants
        LengthUnit::Dvw(v) => Dimension::Dvw(*v),
        LengthUnit::Dvh(v) => Dimension::Dvh(*v),
        LengthUnit::Svw(v) => Dimension::Svw(*v),
        LengthUnit::Svh(v) => Dimension::Svh(*v),
        LengthUnit::Lvw(v) => Dimension::Lvw(*v),
        LengthUnit::Lvh(v) => Dimension::Lvh(*v),
        // Container query units — approximate as percentage of parent
        LengthUnit::Cqw(v) | LengthUnit::Cqi(v) => Dimension::Percent(*v),
        LengthUnit::Cqh(v) | LengthUnit::Cqb(v) => Dimension::Percent(*v),
        LengthUnit::Cqmin(v) => Dimension::Percent(*v),
        LengthUnit::Cqmax(v) => Dimension::Percent(*v),
        // Line-height units — approximate as 1.2× font-size
        LengthUnit::Lh(v) => Dimension::Em(*v * 1.2),
        LengthUnit::Rlh(v) => Dimension::Rem(*v * 1.2),
    }
}

fn length_unit_to_calc(lu: &liquide_theme_css::value::LengthUnit) -> CalcExpr {
    use liquide_theme_css::value::LengthUnit;
    match lu {
        LengthUnit::Px(v) | LengthUnit::Pt(v) => CalcExpr::Px(if matches!(lu, LengthUnit::Pt(_)) {
            *v * 1.333
        } else {
            *v
        }),
        LengthUnit::Em(v) => CalcExpr::Em(*v),
        LengthUnit::Rem(v) => CalcExpr::Rem(*v),
        LengthUnit::Percent(v) => CalcExpr::Percent(*v),
        LengthUnit::Vw(v) => CalcExpr::Vw(*v),
        LengthUnit::Vh(v) => CalcExpr::Vh(*v),
        LengthUnit::Vmin(v) => CalcExpr::Vmin(*v),
        LengthUnit::Vmax(v) => CalcExpr::Vmax(*v),
        LengthUnit::Ch(v) | LengthUnit::Ex(v) => CalcExpr::Em(if matches!(lu, LengthUnit::Ex(_)) {
            *v * 0.5
        } else {
            *v
        }),
        // Dynamic viewport units → distinct CalcExpr variants
        LengthUnit::Dvw(v) => CalcExpr::Dvw(*v),
        LengthUnit::Dvh(v) => CalcExpr::Dvh(*v),
        LengthUnit::Svw(v) => CalcExpr::Svw(*v),
        LengthUnit::Svh(v) => CalcExpr::Svh(*v),
        LengthUnit::Lvw(v) => CalcExpr::Lvw(*v),
        LengthUnit::Lvh(v) => CalcExpr::Lvh(*v),
        // Container query units → percentage approximation
        LengthUnit::Cqw(v)
        | LengthUnit::Cqi(v)
        | LengthUnit::Cqh(v)
        | LengthUnit::Cqb(v)
        | LengthUnit::Cqmin(v)
        | LengthUnit::Cqmax(v) => CalcExpr::Percent(*v),
        // Line-height units → em/rem × 1.2
        LengthUnit::Lh(v) => CalcExpr::Em(*v * 1.2),
        LengthUnit::Rlh(v) => CalcExpr::Rem(*v * 1.2),
    }
}

/// Convert a `CssMathExpr` (parser type) to a `CalcExpr` (style-engine type)
/// so that calc() values can be lazily resolved with actual context at layout time.
fn math_expr_to_calc(expr: &liquide_theme_css::value::CssMathExpr) -> CalcExpr {
    use liquide_theme_css::value::CssMathExpr;
    match expr {
        CssMathExpr::Value(lu) => length_unit_to_calc(lu),
        CssMathExpr::Number(n) => CalcExpr::Number(*n),
        CssMathExpr::Add(a, b) => CalcExpr::Add(
            Box::new(math_expr_to_calc(a)),
            Box::new(math_expr_to_calc(b)),
        ),
        CssMathExpr::Sub(a, b) => CalcExpr::Sub(
            Box::new(math_expr_to_calc(a)),
            Box::new(math_expr_to_calc(b)),
        ),
        CssMathExpr::Mul(a, b) => CalcExpr::Mul(
            Box::new(math_expr_to_calc(a)),
            Box::new(math_expr_to_calc(b)),
        ),
        CssMathExpr::Div(a, b) => CalcExpr::Div(
            Box::new(math_expr_to_calc(a)),
            Box::new(math_expr_to_calc(b)),
        ),
        CssMathExpr::Min(args) => CalcExpr::Min(args.iter().map(math_expr_to_calc).collect()),
        CssMathExpr::Max(args) => CalcExpr::Max(args.iter().map(math_expr_to_calc).collect()),
        CssMathExpr::Clamp {
            min,
            preferred,
            max,
        } => CalcExpr::Clamp {
            min: Box::new(math_expr_to_calc(min)),
            preferred: Box::new(math_expr_to_calc(preferred)),
            max: Box::new(math_expr_to_calc(max)),
        },
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
        PropertyValue::Keyword(kw) => try_parse_color(kw),
        PropertyValue::String(s) => try_parse_color(s),
        _ => None,
    }
}

/// Resolve a PropertyValue to a Color, with `currentcolor` support.
/// When `currentcolor` is encountered, returns the provided `current` color.
pub fn resolve_color_with_current(val: &PropertyValue, current: Color) -> Option<Color> {
    match val {
        PropertyValue::Color(c) => Some(Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }),
        PropertyValue::Keyword(kw) => {
            if kw.trim().eq_ignore_ascii_case("currentcolor") {
                Some(current)
            } else {
                try_parse_color(kw)
            }
        }
        PropertyValue::String(s) => {
            if s.trim().eq_ignore_ascii_case("currentcolor") {
                Some(current)
            } else {
                try_parse_color(s)
            }
        }
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
            "table" => Display::Table,
            "table-row" => Display::TableRow,
            "table-cell" => Display::TableCell,
            "table-row-group" => Display::TableRowGroup,
            "table-header-group" => Display::TableHeaderGroup,
            "table-footer-group" => Display::TableFooterGroup,
            "table-column" => Display::TableColumn,
            "table-column-group" => Display::TableColumnGroup,
            "table-caption" => Display::TableCaption,
            "none" => Display::None,
            "contents" => Display::Contents,
            "flow-root" => Display::FlowRoot,
            "list-item" => Display::ListItem,
            "ruby" => Display::Ruby,
            "ruby-text" => Display::RubyText,
            "run-in" => Display::RunIn,
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

/// Parse an origin keyword (center/left/right/top/bottom/percentage/px) into a Dimension.
pub fn parse_origin_keyword(s: &str) -> Dimension {
    match s.trim() {
        "center" => Dimension::Percent(50.0),
        "left" | "top" => Dimension::Percent(0.0),
        "right" | "bottom" => Dimension::Percent(100.0),
        other => {
            if let Some(v) = other.strip_suffix('%').and_then(|n| n.parse::<f32>().ok()) {
                Dimension::Percent(v)
            } else if let Some(v) = other.strip_suffix("px").and_then(|n| n.parse::<f32>().ok()) {
                Dimension::Px(v)
            } else if let Ok(v) = other.parse::<f32>() {
                Dimension::Px(v)
            } else {
                Dimension::Percent(50.0) // default center
            }
        }
    }
}

/// Resolve a scroll-snap-type keyword like "x mandatory", "y proximity", etc.
pub fn parse_scroll_snap_type(kw: &str) -> ScrollSnapType {
    let parts: Vec<&str> = kw.split_whitespace().collect();
    let strictness = match parts.get(1).map(|s| *s) {
        Some("mandatory") => ScrollSnapStrictness::Mandatory,
        Some("proximity") => ScrollSnapStrictness::Proximity,
        _ => ScrollSnapStrictness::Proximity,
    };
    match parts.first().map(|s| *s) {
        Some("x") => ScrollSnapType::X(strictness),
        Some("y") => ScrollSnapType::Y(strictness),
        Some("block") => ScrollSnapType::Block(strictness),
        Some("inline") => ScrollSnapType::Inline(strictness),
        Some("both") => ScrollSnapType::Both(strictness),
        _ => ScrollSnapType::None,
    }
}

/// Resolve a break/page-break keyword into BreakValue.
pub fn resolve_break_value(kw: &str) -> BreakValue {
    match kw {
        "auto" => BreakValue::Auto,
        "avoid" => BreakValue::Avoid,
        "always" | "page" => BreakValue::Always,
        "left" => BreakValue::Left,
        "right" => BreakValue::Right,
        "column" => BreakValue::Column,
        "avoid-page" => BreakValue::AvoidPage,
        "avoid-column" => BreakValue::AvoidColumn,
        _ => BreakValue::Auto,
    }
}

/// Parse a CSS blend mode keyword into BlendMode.
pub fn resolve_blend_mode(kw: &str) -> liquide_compositor::pixel::BlendMode {
    use liquide_compositor::pixel::BlendMode;
    match kw {
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::ColorBlend,
        "luminosity" => BlendMode::Luminosity,
        _ => BlendMode::SrcOver,
    }
}

/// Parse a grid line value from PropertyValue.
pub fn parse_grid_line_value(val: &PropertyValue) -> GridLine {
    match val {
        PropertyValue::Number(n) => GridLine::Line(*n as i32),
        PropertyValue::Keyword(kw) => parse_grid_line_str(kw),
        _ => GridLine::Auto,
    }
}

/// Parse a grid line from a string like "auto", "1", "span 2".
pub fn parse_grid_line_str(s: &str) -> GridLine {
    let s = s.trim();
    if s == "auto" {
        GridLine::Auto
    } else if let Some(rest) = s.strip_prefix("span") {
        let n = rest.trim().parse::<u32>().unwrap_or(1);
        GridLine::Span(n)
    } else if let Ok(n) = s.parse::<i32>() {
        GridLine::Line(n)
    } else {
        // Treat as a named grid line / grid-area name (e.g. "header")
        GridLine::Named(s.to_string())
    }
}

/// Parse a CSS transform list string like "translateX(10px) rotate(45deg)" into Transform values.
pub fn parse_transform_list(css: &str) -> Vec<Transform> {
    let mut result = Vec::new();
    let mut rest = css.trim();
    while !rest.is_empty() {
        if let Some(open) = rest.find('(') {
            let func = rest[..open].trim();
            if let Some(close) = rest.find(')') {
                let args = rest[open + 1..close].trim();
                match func {
                    "translateX" => {
                        if let Some(px) = parse_px(args) {
                            result.push(Transform::Translate(px, 0.0));
                        }
                    }
                    "translateY" => {
                        if let Some(px) = parse_px(args) {
                            result.push(Transform::Translate(0.0, px));
                        }
                    }
                    "translate" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts
                            .first()
                            .and_then(|s| parse_px(s.trim()))
                            .unwrap_or(0.0);
                        let y = parts.get(1).and_then(|s| parse_px(s.trim())).unwrap_or(0.0);
                        result.push(Transform::Translate(x, y));
                    }
                    "scale" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts
                            .first()
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .unwrap_or(1.0);
                        let y = parts
                            .get(1)
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .unwrap_or(x);
                        result.push(Transform::Scale(x, y));
                    }
                    "scaleX" => {
                        if let Ok(v) = args.parse::<f32>() {
                            result.push(Transform::Scale(v, 1.0));
                        }
                    }
                    "scaleY" => {
                        if let Ok(v) = args.parse::<f32>() {
                            result.push(Transform::Scale(1.0, v));
                        }
                    }
                    "rotate" => {
                        if let Some(deg) = parse_degrees(args) {
                            result.push(Transform::Rotate(deg));
                        }
                    }
                    "skew" | "skewX" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts
                            .first()
                            .and_then(|s| parse_degrees(s.trim()))
                            .unwrap_or(0.0);
                        let y = parts
                            .get(1)
                            .and_then(|s| parse_degrees(s.trim()))
                            .unwrap_or(0.0);
                        result.push(Transform::Skew(x, y));
                    }
                    "skewY" => {
                        if let Some(deg) = parse_degrees(args) {
                            result.push(Transform::Skew(0.0, deg));
                        }
                    }
                    "translate3d" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts.first().and_then(|s| parse_px(s.trim())).unwrap_or(0.0);
                        let y = parts.get(1).and_then(|s| parse_px(s.trim())).unwrap_or(0.0);
                        let z = parts.get(2).and_then(|s| parse_px(s.trim())).unwrap_or(0.0);
                        result.push(Transform::Translate3d(x, y, z));
                    }
                    "translateZ" => {
                        if let Some(px) = parse_px(args) {
                            result.push(Transform::Translate3d(0.0, 0.0, px));
                        }
                    }
                    "rotate3d" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts.first().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
                        let y = parts.get(1).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
                        let z = parts.get(2).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(0.0);
                        let angle = parts.get(3).and_then(|s| parse_degrees(s.trim())).unwrap_or(0.0);
                        result.push(Transform::Rotate3d(x, y, z, angle));
                    }
                    "rotateX" => {
                        if let Some(deg) = parse_degrees(args) {
                            result.push(Transform::Rotate3d(1.0, 0.0, 0.0, deg));
                        }
                    }
                    "rotateY" => {
                        if let Some(deg) = parse_degrees(args) {
                            result.push(Transform::Rotate3d(0.0, 1.0, 0.0, deg));
                        }
                    }
                    "rotateZ" => {
                        if let Some(deg) = parse_degrees(args) {
                            result.push(Transform::Rotate3d(0.0, 0.0, 1.0, deg));
                        }
                    }
                    "scale3d" => {
                        let parts: Vec<&str> = args.split(',').collect();
                        let x = parts.first().and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(1.0);
                        let y = parts.get(1).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(1.0);
                        let z = parts.get(2).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(1.0);
                        result.push(Transform::Scale3d(x, y, z));
                    }
                    "scaleZ" => {
                        if let Ok(v) = args.parse::<f32>() {
                            result.push(Transform::Scale3d(1.0, 1.0, v));
                        }
                    }
                    "matrix3d" => {
                        let parts: Vec<f32> = args
                            .split(',')
                            .filter_map(|s| s.trim().parse::<f32>().ok())
                            .collect();
                        if parts.len() == 16 {
                            let mut m = [0.0f32; 16];
                            m.copy_from_slice(&parts);
                            result.push(Transform::Matrix3d(m));
                        }
                    }
                    "perspective" => {
                        if let Some(px) = parse_px(args) {
                            if px > 0.0 {
                                result.push(Transform::PerspectiveFn(px));
                            }
                        }
                    }
                    _ => {}
                }
                rest = rest[close + 1..].trim();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}

fn parse_px(s: &str) -> Option<f32> {
    let s = s.trim();
    s.strip_suffix("px")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .or_else(|| s.parse::<f32>().ok())
}

fn parse_degrees(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("deg") {
        v.trim().parse::<f32>().ok()
    } else if let Some(v) = s.strip_suffix("rad") {
        v.trim().parse::<f32>().ok().map(|r| r.to_degrees())
    } else if let Some(v) = s.strip_suffix("turn") {
        v.trim().parse::<f32>().ok().map(|t| t * 360.0)
    } else {
        s.parse::<f32>().ok()
    }
}

/// Parse a single track size token.
fn parse_single_track(token: &str) -> Option<TrackSize> {
    let token = token.trim();
    if let Some(v) = token.strip_suffix("fr") {
        v.parse::<f32>().ok().map(TrackSize::Fr)
    } else if let Some(v) = token.strip_suffix("px") {
        v.parse::<f32>().ok().map(TrackSize::Px)
    } else if let Some(v) = token.strip_suffix('%') {
        v.parse::<f32>().ok().map(TrackSize::Percent)
    } else if token == "auto" {
        Some(TrackSize::Auto)
    } else if token == "min-content" {
        Some(TrackSize::MinContent)
    } else if token == "max-content" {
        Some(TrackSize::MaxContent)
    } else {
        token.parse::<f32>().ok().map(TrackSize::Px)
    }
}

/// Parse a CSS grid track list string like "100px 200px" or "1fr 2fr" into TrackSize values.
/// Also handles repeat() functions: repeat(3, 100px), repeat(auto-fill, 100px), repeat(auto-fit, minmax(100px, 1fr))
pub fn parse_track_list(css: &str) -> Vec<TrackSize> {
    use crate::computed::RepeatMode;
    
    let css = css.trim();
    if css.is_empty() {
        return Vec::new();
    }
    
    let mut result = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = css.chars().collect();
    
    while i < chars.len() {
        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        
        if i >= chars.len() {
            break;
        }
        
        // Check for repeat() function
        if css[i..].to_lowercase().starts_with("repeat(") {
            // Find matching closing paren
            let start = i + 7; // after "repeat("
            let mut depth = 1;
            let mut end = start;
            while end < chars.len() && depth > 0 {
                match chars[end] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    end += 1;
                }
            }
            
            if depth == 0 {
                let inner: String = chars[start..end].iter().collect();
                // Parse repeat() arguments: mode, tracks
                if let Some(comma_idx) = find_first_comma(&inner) {
                    let mode_str = inner[..comma_idx].trim();
                    let tracks_str = inner[comma_idx + 1..].trim();
                    
                    let mode = match mode_str.to_lowercase().as_str() {
                        "auto-fill" => Some(RepeatMode::AutoFill),
                        "auto-fit" => Some(RepeatMode::AutoFit),
                        _ => mode_str.parse::<u32>().ok().map(RepeatMode::Count),
                    };
                    
                    if let Some(mode) = mode {
                        // Parse the tracks inside repeat()
                        let inner_tracks = if tracks_str.starts_with("minmax(") {
                            // Handle minmax() inside repeat()
                            vec![parse_minmax(tracks_str).unwrap_or(TrackSize::Auto)]
                        } else {
                            parse_track_list_simple(tracks_str)
                        };
                        
                        if !inner_tracks.is_empty() {
                            result.push(TrackSize::Repeat { mode, tracks: inner_tracks });
                        }
                    }
                }
                i = end + 1;
                continue;
            }
        }
        
        // Check for minmax() function
        if css[i..].to_lowercase().starts_with("minmax(") {
            let start = i + 7;
            let mut depth = 1;
            let mut end = start;
            while end < chars.len() && depth > 0 {
                match chars[end] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    end += 1;
                }
            }
            
            if depth == 0 {
                let inner: String = chars[i..=end].iter().collect();
                if let Some(track) = parse_minmax(&inner) {
                    result.push(track);
                }
                i = end + 1;
                continue;
            }
        }
        
        // Regular token - find end
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' {
            i += 1;
        }
        
        if i > start {
            let token: String = chars[start..i].iter().collect();
            if let Some(track) = parse_single_track(&token) {
                result.push(track);
            }
        }
    }
    
    result
}

/// Find the first comma at depth 0 (not inside parentheses).
fn find_first_comma(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Parse minmax(min, max) into TrackSize::MinMax.
fn parse_minmax(s: &str) -> Option<TrackSize> {
    let s = s.trim();
    if !s.to_lowercase().starts_with("minmax(") || !s.ends_with(')') {
        return None;
    }
    
    let inner = &s[7..s.len() - 1];
    let comma_idx = find_first_comma(inner)?;
    
    let min_str = inner[..comma_idx].trim();
    let max_str = inner[comma_idx + 1..].trim();
    
    let min = parse_single_track(min_str)?;
    let max = parse_single_track(max_str)?;
    
    Some(TrackSize::MinMax(Box::new(min), Box::new(max)))
}

/// Simple track list parser for space-separated tokens (no functions).
fn parse_track_list_simple(css: &str) -> Vec<TrackSize> {
    css.split_whitespace()
        .filter_map(|token| parse_single_track(token))        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inline_named_color() {
        let value = parse_inline_value("red");
        assert!(matches!(value, PropertyValue::Color(_)));
    }

    #[test]
    fn resolve_color_from_keyword() {
        let value = PropertyValue::Keyword("rgba(0, 128, 255, 0.5)".to_string());
        let color = resolve_color(&value).expect("expected rgba keyword to parse");
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 255);
        assert_eq!(color.a, 127);
    }

    #[test]
    fn resolve_color_transparent_keyword() {
        let value = PropertyValue::Keyword("transparent".to_string());
        let color = resolve_color(&value).expect("expected transparent keyword to parse");
        assert_eq!(color.a, 0);
    }
}
