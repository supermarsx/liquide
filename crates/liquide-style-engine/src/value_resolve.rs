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
    if value.starts_with('#') || value.starts_with("rgb") || value.starts_with("rgba") {
        if let Some(c) = try_parse_color(value) {
            return PropertyValue::Color(liquide_theme_css::value::Color {
                r: c.r,
                g: c.g,
                b: c.b,
                a: c.a,
            });
        }
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
    // #rrggbb or #rgb
    if value.starts_with('#') {
        let hex = &value[1..];
        let (r, g, b, a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                (r, g, b, 255)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                (r, g, b, a)
            }
            _ => return None,
        };
        return Some(Color { r, g, b, a });
    }
    None
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
        // Dynamic viewport units — map to regular viewport (no dynamic chrome distinction yet)
        LengthUnit::Dvw(v) | LengthUnit::Svw(v) | LengthUnit::Lvw(v) => Dimension::Vw(*v),
        LengthUnit::Dvh(v) | LengthUnit::Svh(v) | LengthUnit::Lvh(v) => Dimension::Vh(*v),
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
        // Dynamic viewport units → regular viewport
        LengthUnit::Dvw(v) | LengthUnit::Svw(v) | LengthUnit::Lvw(v) => CalcExpr::Vw(*v),
        LengthUnit::Dvh(v) | LengthUnit::Svh(v) | LengthUnit::Lvh(v) => CalcExpr::Vh(*v),
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
        GridLine::Auto
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

/// Parse a CSS grid track list string like "100px 200px" or "1fr 2fr" into TrackSize values.
pub fn parse_track_list(css: &str) -> Vec<TrackSize> {
    css.split_whitespace()
        .filter_map(|token| {
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
        })
        .collect()
}
