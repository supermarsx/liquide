//! CSS property conversion — maps lightningcss `Property` variants to `PropertySet` entries.
//!
//! Handles all CSS property shorthands and longhands: background, border, margin,
//! padding, flex, grid, font, text, transform, and custom properties.

use crate::property::PropertySet;
use crate::value::{Color, LengthUnit, PropertyValue};

use lightningcss::properties::Property;
use lightningcss::stylesheet::PrinterOptions;
use lightningcss::traits::ToCss;
use lightningcss::values::color::CssColor;
use lightningcss::values::length::LengthPercentageOrAuto;

use super::ThemeParser;

impl ThemeParser {
    /// Insert converted properties from a single declaration into the property set.
    /// A single declaration can expand into multiple properties (e.g. shorthand `background`
    /// produces both "background" and "background-color").
    pub(crate) fn insert_property(&self, prop: &Property, properties: &mut PropertySet) {
        match prop {
            // ── Background ──────────────────────────────────────────────
            Property::BackgroundColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("background-color".into(), v.clone());
                    properties.insert("background".into(), v);
                }
            }
            Property::Background(backgrounds) => {
                let background_css = self.to_css_string(backgrounds);
                let background_value = match self.parse_value_string(&background_css) {
                    PropertyValue::Color(color) => PropertyValue::Color(color),
                    _ => PropertyValue::Keyword(background_css),
                };
                properties.insert("background".into(), background_value.clone());

                let layer_count = backgrounds.len();
                let mut image_values = Vec::with_capacity(layer_count);
                let mut position_values = Vec::with_capacity(layer_count);
                let mut size_values = Vec::with_capacity(layer_count);
                let mut repeat_values = Vec::with_capacity(layer_count);
                let mut attachment_values = Vec::with_capacity(layer_count);
                let mut origin_values = Vec::with_capacity(layer_count);
                let mut clip_values = Vec::with_capacity(layer_count);

                for bg in backgrounds.iter() {
                    image_values.push(self.to_css_string(&bg.image));
                    position_values.push(self.to_css_string(&bg.position));
                    size_values.push(self.to_css_string(&bg.size));
                    repeat_values.push(self.to_css_string(&bg.repeat));
                    attachment_values.push(self.to_css_string(&bg.attachment));
                    origin_values.push(self.to_css_string(&bg.origin));
                    clip_values.push(self.to_css_string(&bg.clip));
                }

                if matches!(background_value, PropertyValue::Color(_)) {
                    properties.insert("background-color".into(), background_value);
                }

                if layer_count == 1 && image_values.first().map(|value| value.as_str()) == Some("none") {
                    if let Some(color) = backgrounds
                        .first()
                        .and_then(|layer| self.convert_color(&layer.color))
                    {
                        properties.insert("background-color".into(), color);
                    }
                }

                if layer_count > 1 || image_values.iter().any(|value| value != "none") {
                    properties.insert(
                        "background-image".into(),
                        PropertyValue::Keyword(image_values.join(", ")),
                    );
                }

                if layer_count > 1
                    || position_values
                        .iter()
                        .any(|value| value != "0% 0%" && value != "0 0")
                {
                    properties.insert(
                        "background-position".into(),
                        PropertyValue::Keyword(position_values.join(", ")),
                    );
                }

                if layer_count > 1
                    || size_values
                        .iter()
                        .any(|value| value != "auto" && value != "auto auto")
                {
                    properties.insert(
                        "background-size".into(),
                        PropertyValue::Keyword(size_values.join(", ")),
                    );
                }

                if layer_count > 1
                    || repeat_values
                        .iter()
                        .any(|value| value != "repeat" && value != "repeat repeat")
                {
                    properties.insert(
                        "background-repeat".into(),
                        PropertyValue::Keyword(repeat_values.join(", ")),
                    );
                }

                if layer_count > 1 || attachment_values.iter().any(|value| value != "scroll") {
                    properties.insert(
                        "background-attachment".into(),
                        PropertyValue::Keyword(attachment_values.join(", ")),
                    );
                }

                if layer_count > 1 || origin_values.iter().any(|value| value != "padding-box") {
                    properties.insert(
                        "background-origin".into(),
                        PropertyValue::Keyword(origin_values.join(", ")),
                    );
                }

                if layer_count > 1 || clip_values.iter().any(|value| value != "border-box") {
                    properties.insert(
                        "background-clip".into(),
                        PropertyValue::Keyword(clip_values.join(", ")),
                    );
                }
            }

            // ── Foreground color ────────────────────────────────────────
            Property::Color(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("color".into(), v);
                }
            }

            // ── Border colors (longhand) ────────────────────────────────
            Property::BorderTopColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-top-color".into(), v);
                }
            }
            Property::BorderRightColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-right-color".into(), v);
                }
            }
            Property::BorderBottomColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-bottom-color".into(), v);
                }
            }
            Property::BorderLeftColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-left-color".into(), v);
                }
            }

            // ── Border color shorthand ──────────────────────────────────
            Property::BorderColor(bc) => {
                let top = self.convert_color(&bc.top);
                let right = self.convert_color(&bc.right);
                let bottom = self.convert_color(&bc.bottom);
                let left = self.convert_color(&bc.left);

                if let Some(v) = &top {
                    properties.insert("border-top-color".into(), v.clone());
                }
                if let Some(v) = &right {
                    properties.insert("border-right-color".into(), v.clone());
                }
                if let Some(v) = &bottom {
                    properties.insert("border-bottom-color".into(), v.clone());
                }
                if let Some(v) = &left {
                    properties.insert("border-left-color".into(), v.clone());
                }

                if let (Some(top), Some(right), Some(bottom), Some(left)) = (top, right, bottom, left)
                {
                    properties.insert(
                        "border-color".into(),
                        compress_box_shorthand([top, right, bottom, left]),
                    );
                }
            }

            // ── Border shorthand (border: 1px solid red) ────────────────
            Property::Border(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-width".into(), v);
                }
                properties.insert(
                    "border-style".into(),
                    self.convert_line_style(&border.style),
                );
            }
            Property::BorderTop(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-top-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-top-width".into(), v);
                }
                properties.insert(
                    "border-top-style".into(),
                    self.convert_line_style(&border.style),
                );
            }
            Property::BorderRight(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-right-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-right-width".into(), v);
                }
                properties.insert(
                    "border-right-style".into(),
                    self.convert_line_style(&border.style),
                );
            }
            Property::BorderBottom(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-bottom-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-bottom-width".into(), v);
                }
                properties.insert(
                    "border-bottom-style".into(),
                    self.convert_line_style(&border.style),
                );
            }
            Property::BorderLeft(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-left-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-left-width".into(), v);
                }
                properties.insert(
                    "border-left-style".into(),
                    self.convert_line_style(&border.style),
                );
            }

            // ── Border widths ───────────────────────────────────────────
            Property::BorderTopWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-top-width".into(), v);
                }
            }
            Property::BorderRightWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-right-width".into(), v);
                }
            }
            Property::BorderBottomWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-bottom-width".into(), v);
                }
            }
            Property::BorderLeftWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-left-width".into(), v);
                }
            }

            // ── Border radius ───────────────────────────────────────────
            Property::BorderRadius(radius, _prefix) => {
                let css_str = self.to_css_string(radius);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("border-radius".into(), v);
                }
            }

            // ── Dimensions ──────────────────────────────────────────────
            Property::Width(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("width".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties.insert("width".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }
            Property::Height(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("height".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties.insert("height".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }

            // ── Padding ─────────────────────────────────────────────────
            Property::PaddingTop(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("padding-top".into(), v);
                }
            }
            Property::PaddingRight(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("padding-right".into(), v);
                }
            }
            Property::PaddingBottom(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("padding-bottom".into(), v);
                }
            }
            Property::PaddingLeft(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("padding-left".into(), v);
                }
            }
            Property::Padding(padding) => {
                let top = self.convert_length_percentage_or_auto(&padding.top);
                let right = self.convert_length_percentage_or_auto(&padding.right);
                let bottom = self.convert_length_percentage_or_auto(&padding.bottom);
                let left = self.convert_length_percentage_or_auto(&padding.left);

                if let Some(v) = &top {
                    properties.insert("padding-top".into(), v.clone());
                }
                if let Some(v) = &right {
                    properties.insert("padding-right".into(), v.clone());
                }
                if let Some(v) = &bottom {
                    properties.insert("padding-bottom".into(), v.clone());
                }
                if let Some(v) = &left {
                    properties.insert("padding-left".into(), v.clone());
                }

                if let (Some(top), Some(right), Some(bottom), Some(left)) = (top, right, bottom, left)
                {
                    properties.insert(
                        "padding".into(),
                        compress_box_shorthand([top, right, bottom, left]),
                    );
                }
            }

            // ── Margin ──────────────────────────────────────────────────
            Property::MarginTop(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("margin-top".into(), v);
                }
            }
            Property::MarginRight(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("margin-right".into(), v);
                }
            }
            Property::MarginBottom(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("margin-bottom".into(), v);
                }
            }
            Property::MarginLeft(val) => {
                if let Some(v) = self.convert_length_percentage_or_auto(val) {
                    properties.insert("margin-left".into(), v);
                }
            }
            Property::Margin(margin) => {
                let top = self.convert_length_percentage_or_auto(&margin.top);
                let right = self.convert_length_percentage_or_auto(&margin.right);
                let bottom = self.convert_length_percentage_or_auto(&margin.bottom);
                let left = self.convert_length_percentage_or_auto(&margin.left);

                if let Some(v) = &top {
                    properties.insert("margin-top".into(), v.clone());
                }
                if let Some(v) = &right {
                    properties.insert("margin-right".into(), v.clone());
                }
                if let Some(v) = &bottom {
                    properties.insert("margin-bottom".into(), v.clone());
                }
                if let Some(v) = &left {
                    properties.insert("margin-left".into(), v.clone());
                }

                if let (Some(top), Some(right), Some(bottom), Some(left)) = (top, right, bottom, left)
                {
                    properties.insert(
                        "margin".into(),
                        compress_box_shorthand([top, right, bottom, left]),
                    );
                }
            }

            // ── Opacity ─────────────────────────────────────────────────
            Property::Opacity(alpha) => {
                let css_str = self.to_css_string(alpha);
                if let Ok(n) = css_str.trim().parse::<f32>() {
                    properties.insert("opacity".into(), PropertyValue::Number(n));
                }
            }

            // ── Font properties ─────────────────────────────────────────
            Property::FontSize(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("font-size".into(), v);
                }
            }
            Property::FontWeight(weight) => {
                let css_str = self.to_css_string(weight);
                let val = match css_str.trim() {
                    "bold" => Some(700.0),
                    "normal" => Some(400.0),
                    "lighter" => Some(300.0),
                    "bolder" => Some(800.0),
                    other => other.parse::<f32>().ok(),
                };
                if let Some(n) = val {
                    properties.insert("font-weight".into(), PropertyValue::Number(n));
                }
            }
            Property::LineHeight(lh) => {
                let css_str = self.to_css_string(lh);
                // Check for bare number FIRST (unitless line-height multiplier)
                // before parse_length_value which treats bare numbers as px
                let trimmed = css_str.trim();
                if let Ok(n) = trimmed.parse::<f32>() {
                    // Only treat as unitless number if it has no unit suffix
                    if !trimmed.ends_with("px")
                        && !trimmed.ends_with("em")
                        && !trimmed.ends_with("rem")
                        && !trimmed.ends_with('%')
                    {
                        properties.insert("line-height".into(), PropertyValue::Number(n));
                    } else if let Some(v) = self.parse_length_value(&css_str) {
                        properties.insert("line-height".into(), v);
                    }
                } else if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("line-height".into(), v);
                }
            }
            Property::FontFamily(families) => {
                let css_str = self.to_css_string(families);
                properties.insert("font-family".into(), PropertyValue::String(css_str));
            }

            // ── Box shadow ──────────────────────────────────────────────
            Property::BoxShadow(shadows, _prefix) => {
                let mut shadow_values = Vec::new();
                for shadow in shadows.iter() {
                    let offset_x = self.length_to_px(&self.to_css_string(&shadow.x_offset));
                    let offset_y = self.length_to_px(&self.to_css_string(&shadow.y_offset));
                    let blur = self.length_to_px(&self.to_css_string(&shadow.blur));
                    let spread = self.length_to_px(&self.to_css_string(&shadow.spread));
                    let color = self
                        .convert_color(&shadow.color)
                        .and_then(|v| {
                            if let PropertyValue::Color(c) = v {
                                Some(c)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Color::new(0, 0, 0, 255));
                    shadow_values.push(crate::value::BoxShadow {
                        offset_x,
                        offset_y,
                        blur_radius: blur,
                        spread_radius: spread,
                        color,
                        inset: shadow.inset,
                    });
                    // Also set individual shadow properties for easy querying
                    properties.insert(
                        "shadow-offset-x".into(),
                        PropertyValue::Length(LengthUnit::Px(offset_x)),
                    );
                    properties.insert(
                        "shadow-offset-y".into(),
                        PropertyValue::Length(LengthUnit::Px(offset_y)),
                    );
                    properties.insert(
                        "shadow-blur".into(),
                        PropertyValue::Length(LengthUnit::Px(blur)),
                    );
                    properties.insert("shadow-color".into(), PropertyValue::Color(color));
                    properties.insert("box-shadow-color".into(), PropertyValue::Color(color));
                }
                if !shadow_values.is_empty() {
                    properties.insert("box-shadow".into(), PropertyValue::BoxShadow(shadow_values));
                }
            }

            // ── Z-index ─────────────────────────────────────────────────
            Property::ZIndex(z) => {
                let css_str = self.to_css_string(z);
                if let Ok(n) = css_str.trim().parse::<f32>() {
                    properties.insert("z-index".into(), PropertyValue::Number(n));
                }
            }

            // ── Visibility ──────────────────────────────────────────────
            Property::Visibility(vis) => {
                let css_str = self.to_css_string(vis);
                properties.insert("visibility".into(), PropertyValue::Keyword(css_str));
            }

            // ── Display ─────────────────────────────────────────────────
            Property::Display(display) => {
                let css_str = self.to_css_string(display);
                properties.insert("display".into(), PropertyValue::Keyword(css_str));
            }

            // ── Position ────────────────────────────────────────────────
            Property::Position(pos) => {
                let css_str = self.to_css_string(pos);
                properties.insert("position".into(), PropertyValue::Keyword(css_str));
            }

            // ── Overflow ────────────────────────────────────────────────
            Property::Overflow(overflow) => {
                let x = self.to_css_string(&overflow.x);
                let y = self.to_css_string(&overflow.y);
                properties.insert("overflow-x".into(), PropertyValue::Keyword(x.clone()));
                properties.insert("overflow-y".into(), PropertyValue::Keyword(y.clone()));
                properties.insert(
                    "overflow".into(),
                    compress_pair_shorthand(PropertyValue::Keyword(x), PropertyValue::Keyword(y)),
                );
            }
            Property::OverflowX(kw) => {
                let css_str = self.to_css_string(kw);
                properties.insert("overflow-x".into(), PropertyValue::Keyword(css_str));
            }
            Property::OverflowY(kw) => {
                let css_str = self.to_css_string(kw);
                properties.insert("overflow-y".into(), PropertyValue::Keyword(css_str));
            }

            // ── Flex ────────────────────────────────────────────────────
            Property::FlexDirection(dir, _prefix) => {
                let css_str = self.to_css_string(dir);
                properties.insert("flex-direction".into(), PropertyValue::Keyword(css_str));
            }
            Property::FlexWrap(wrap, _prefix) => {
                let css_str = self.to_css_string(wrap);
                properties.insert("flex-wrap".into(), PropertyValue::Keyword(css_str));
            }
            Property::FlexGrow(grow, _prefix) => {
                properties.insert("flex-grow".into(), PropertyValue::Number(*grow));
            }
            Property::FlexShrink(shrink, _prefix) => {
                properties.insert("flex-shrink".into(), PropertyValue::Number(*shrink));
            }
            Property::JustifyContent(jc, _prefix) => {
                let css_str = self.to_css_string(jc);
                properties.insert("justify-content".into(), PropertyValue::Keyword(css_str));
            }
            Property::AlignItems(ai, _prefix) => {
                let css_str = self.to_css_string(ai);
                properties.insert("align-items".into(), PropertyValue::Keyword(css_str));
            }
            Property::AlignSelf(a, _prefix) => {
                let css_str = self.to_css_string(a);
                properties.insert("align-self".into(), PropertyValue::Keyword(css_str));
            }
            Property::AlignContent(ac, _prefix) => {
                let css_str = self.to_css_string(ac);
                properties.insert("align-content".into(), PropertyValue::Keyword(css_str));
            }
            Property::Gap(gap) => {
                let row_str = self.to_css_string(&gap.row);
                let col_str = self.to_css_string(&gap.column);
                let row = if let Some(v) = self.parse_length_value(&row_str) {
                    Some(v)
                } else if row_str.trim() == "normal" {
                    Some(PropertyValue::Keyword("normal".into()))
                } else {
                    None
                };
                let column = if let Some(v) = self.parse_length_value(&col_str) {
                    Some(v)
                } else if col_str.trim() == "normal" {
                    Some(PropertyValue::Keyword("normal".into()))
                } else {
                    None
                };

                if let Some(v) = &row {
                    properties.insert("row-gap".into(), v.clone());
                }
                if let Some(v) = &column {
                    properties.insert("column-gap".into(), v.clone());
                }

                if let (Some(row), Some(column)) = (row, column) {
                    properties.insert("gap".into(), compress_pair_shorthand(row, column));
                }
            }
            Property::RowGap(gap) => {
                let css_str = self.to_css_string(gap);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("row-gap".into(), v);
                } else if css_str.trim() == "normal" {
                    properties.insert("row-gap".into(), PropertyValue::Keyword("normal".into()));
                }
            }
            Property::ColumnGap(gap) => {
                let css_str = self.to_css_string(gap);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("column-gap".into(), v);
                } else if css_str.trim() == "normal" {
                    properties.insert("column-gap".into(), PropertyValue::Keyword("normal".into()));
                }
            }

            // ── Inset properties (top/right/bottom/left) ───────────────
            Property::Top(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("top".into(), v);
                } else if css_str.trim() == "auto" {
                    properties.insert("top".into(), PropertyValue::Keyword("auto".into()));
                }
            }
            Property::Right(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("right".into(), v);
                } else if css_str.trim() == "auto" {
                    properties.insert("right".into(), PropertyValue::Keyword("auto".into()));
                }
            }
            Property::Bottom(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("bottom".into(), v);
                } else if css_str.trim() == "auto" {
                    properties.insert("bottom".into(), PropertyValue::Keyword("auto".into()));
                }
            }
            Property::Left(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("left".into(), v);
                } else if css_str.trim() == "auto" {
                    properties.insert("left".into(), PropertyValue::Keyword("auto".into()));
                }
            }

            // ── Min/max dimensions ──────────────────────────────────────
            Property::MinWidth(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("min-width".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("min-width".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }
            Property::MaxWidth(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("max-width".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "none" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("max-width".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }
            Property::MinHeight(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("min-height".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("min-height".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }
            Property::MaxHeight(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("max-height".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "none" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("max-height".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }

            // ── Text ────────────────────────────────────────────────────
            Property::TextAlign(align) => {
                let css_str = self.to_css_string(align);
                properties.insert("text-align".into(), PropertyValue::Keyword(css_str));
            }
            Property::WhiteSpace(ws) => {
                let css_str = self.to_css_string(ws);
                properties.insert("white-space".into(), PropertyValue::Keyword(css_str));
            }
            Property::WordBreak(wb) => {
                let css_str = self.to_css_string(wb);
                properties.insert("word-break".into(), PropertyValue::Keyword(css_str));
            }
            Property::TextOverflow(to, _prefix) => {
                let css_str = self.to_css_string(to);
                properties.insert("text-overflow".into(), PropertyValue::Keyword(css_str));
            }
            Property::TextTransform(tt) => {
                let css_str = self.to_css_string(tt);
                properties.insert("text-transform".into(), PropertyValue::Keyword(css_str));
            }
            Property::LetterSpacing(ls) => {
                let css_str = self.to_css_string(ls);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("letter-spacing".into(), v);
                }
            }
            Property::WordSpacing(ws) => {
                let css_str = self.to_css_string(ws);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("word-spacing".into(), v);
                }
            }
            Property::TextIndent(ti) => {
                let css_str = self.to_css_string(ti);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("text-indent".into(), v);
                }
            }
            Property::FontStyle(fs) => {
                let css_str = self.to_css_string(fs);
                properties.insert("font-style".into(), PropertyValue::Keyword(css_str));
            }
            Property::Cursor(cursor) => {
                let css_str = self.to_css_string(cursor);
                properties.insert("cursor".into(), PropertyValue::Keyword(css_str));
            }

            // ── Border styles (per-side) ────────────────────────────────
            Property::BorderTopStyle(style) => {
                properties.insert("border-top-style".into(), self.convert_line_style(style));
            }
            Property::BorderRightStyle(style) => {
                properties.insert("border-right-style".into(), self.convert_line_style(style));
            }
            Property::BorderBottomStyle(style) => {
                properties.insert("border-bottom-style".into(), self.convert_line_style(style));
            }
            Property::BorderLeftStyle(style) => {
                properties.insert("border-left-style".into(), self.convert_line_style(style));
            }
            Property::BorderStyle(bs) => {
                let top = self.convert_line_style(&bs.top);
                let right = self.convert_line_style(&bs.right);
                let bottom = self.convert_line_style(&bs.bottom);
                let left = self.convert_line_style(&bs.left);

                properties.insert("border-top-style".into(), top.clone());
                properties.insert("border-right-style".into(), right.clone());
                properties.insert("border-bottom-style".into(), bottom.clone());
                properties.insert("border-left-style".into(), left.clone());
                properties.insert(
                    "border-style".into(),
                    compress_box_shorthand([top, right, bottom, left]),
                );
            }

            // ── Border radius per-corner ────────────────────────────────
            Property::BorderTopLeftRadius(r, _prefix) => {
                let css_str = self.to_css_string(r);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("border-top-left-radius".into(), v);
                }
            }
            Property::BorderTopRightRadius(r, _prefix) => {
                let css_str = self.to_css_string(r);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("border-top-right-radius".into(), v);
                }
            }
            Property::BorderBottomLeftRadius(r, _prefix) => {
                let css_str = self.to_css_string(r);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("border-bottom-left-radius".into(), v);
                }
            }
            Property::BorderBottomRightRadius(r, _prefix) => {
                let css_str = self.to_css_string(r);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("border-bottom-right-radius".into(), v);
                }
            }

            // ── Border width shorthand ──────────────────────────────────
            Property::BorderWidth(bw) => {
                let top = self.convert_border_width(&bw.top);
                let right = self.convert_border_width(&bw.right);
                let bottom = self.convert_border_width(&bw.bottom);
                let left = self.convert_border_width(&bw.left);

                if let Some(v) = &top {
                    properties.insert("border-top-width".into(), v.clone());
                }
                if let Some(v) = &right {
                    properties.insert("border-right-width".into(), v.clone());
                }
                if let Some(v) = &bottom {
                    properties.insert("border-bottom-width".into(), v.clone());
                }
                if let Some(v) = &left {
                    properties.insert("border-left-width".into(), v.clone());
                }

                if let (Some(top), Some(right), Some(bottom), Some(left)) = (top, right, bottom, left)
                {
                    properties.insert(
                        "border-width".into(),
                        compress_box_shorthand([top, right, bottom, left]),
                    );
                }
            }

            // ── Flex extras ─────────────────────────────────────────────
            Property::FlexBasis(fb, _prefix) => {
                let css_str = self.to_css_string(fb);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("flex-basis".into(), v);
                } else {
                    // Keywords: auto, min-content, max-content, fit-content, content
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "content" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("flex-basis".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }
            Property::Order(order, _) => {
                properties.insert("order".into(), PropertyValue::Number(*order as f32));
            }
            Property::Flex(flex, _prefix) => {
                properties.insert("flex-grow".into(), PropertyValue::Number(flex.grow));
                properties.insert("flex-shrink".into(), PropertyValue::Number(flex.shrink));
                let css_str = self.to_css_string(&flex.basis);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("flex-basis".into(), v);
                } else {
                    let trimmed = css_str.trim();
                    if matches!(
                        trimmed,
                        "auto" | "content" | "min-content" | "max-content" | "fit-content"
                    ) {
                        properties
                            .insert("flex-basis".into(), PropertyValue::Keyword(trimmed.into()));
                    }
                }
            }

            // ── Grid ────────────────────────────────────────────────────
            Property::GridTemplateColumns(tracks) => {
                let css_str = self.to_css_string(tracks);
                properties.insert(
                    "grid-template-columns".into(),
                    PropertyValue::Keyword(css_str),
                );
            }
            Property::GridTemplateRows(tracks) => {
                let css_str = self.to_css_string(tracks);
                properties.insert("grid-template-rows".into(), PropertyValue::Keyword(css_str));
            }
            Property::GridAutoFlow(flow) => {
                let css_str = self.to_css_string(flow);
                properties.insert("grid-auto-flow".into(), PropertyValue::Keyword(css_str));
            }
            Property::GridColumn(gc) => {
                let css_str = self.to_css_string(gc);
                properties.insert("grid-column".into(), PropertyValue::Keyword(css_str));
            }
            Property::GridRow(gr) => {
                let css_str = self.to_css_string(gr);
                properties.insert("grid-row".into(), PropertyValue::Keyword(css_str));
            }

            // ── Transform ───────────────────────────────────────────────
            Property::Transform(transforms, _prefix) => {
                let css_str = self.to_css_string(transforms);
                properties.insert("transform".into(), PropertyValue::Keyword(css_str));
            }

            // ── Transition ──────────────────────────────────────────────
            Property::Transition(transitions, _prefix) => {
                let css_str = self.to_css_string(transitions);
                properties.insert("transition".into(), PropertyValue::Keyword(css_str));
            }

            // ── Box sizing ──────────────────────────────────────────────
            Property::BoxSizing(bs, _prefix) => {
                let css_str = self.to_css_string(bs);
                properties.insert("box-sizing".into(), PropertyValue::Keyword(css_str));
            }

            // ── Custom properties (--var-name: value) ───────────────────
            Property::Custom(custom) => {
                let name = self.to_css_string(&custom.name);
                if let Some(value_str) = self.serialize_property_value(prop) {
                    properties.insert(name, self.parse_value_string(&value_str));
                }
            }

            // ── Unparsed properties (non-standard names like glass-tint) ─
            Property::Unparsed(unparsed) => {
                let name = self.to_css_string(&unparsed.property_id);
                if let Some(value_str) = self.serialize_property_value(prop) {
                    properties.insert(name, self.parse_value_string(&value_str));
                }
            }

            // ── Catch-all: store property name so we know it was declared ──
            _ => {
                let prop_id = prop.property_id();
                let name = self.to_css_string(&prop_id);
                if !name.is_empty() {
                    if let Some(value_str) = self.serialize_property_value(prop) {
                        properties.insert(name, self.parse_value_string(&value_str));
                    }
                }
            }
        }
    }

    /// Convert lightningcss color to our Color type.
    pub(crate) fn convert_color(&self, css_color: &CssColor) -> Option<PropertyValue> {
        match css_color {
            CssColor::RGBA(rgba) => Some(PropertyValue::Color(Color::new(
                rgba.red, rgba.green, rgba.blue, rgba.alpha,
            ))),
            _ => {
                // For other color types, try to serialize and parse
                let css_str = self.to_css_string(css_color);
                // Try oklch/oklab/color-mix first, then fall back
                if let Ok(color) = Color::parse_css(&css_str) {
                    return Some(PropertyValue::Color(color));
                }
                None
            }
        }
    }

    /// Convert `LengthPercentageOrAuto` to `PropertyValue`.
    fn convert_length_percentage_or_auto(
        &self,
        val: &LengthPercentageOrAuto,
    ) -> Option<PropertyValue> {
        match val {
            LengthPercentageOrAuto::Auto => Some(PropertyValue::Keyword("auto".into())),
            LengthPercentageOrAuto::LengthPercentage(lp) => {
                let css_str = self.to_css_string(lp);
                self.parse_length_value(&css_str)
            }
        }
    }

    /// Convert a CSS line style to `PropertyValue`.
    fn convert_line_style<S: ToCss>(&self, style: &S) -> PropertyValue {
        let css_str = self.to_css_string(style);
        PropertyValue::Keyword(css_str)
    }

    /// Convert border width.
    fn convert_border_width(
        &self,
        width: &lightningcss::properties::border::BorderSideWidth,
    ) -> Option<PropertyValue> {
        let width_str = self.to_css_string(width);
        match width_str.as_str() {
            "thin" => Some(PropertyValue::Length(LengthUnit::Px(1.0))),
            "medium" => Some(PropertyValue::Length(LengthUnit::Px(3.0))),
            "thick" => Some(PropertyValue::Length(LengthUnit::Px(5.0))),
            _ => self.parse_length_value(&width_str),
        }
    }

    fn serialize_property_value(&self, prop: &Property<'_>) -> Option<String> {
        prop.value_to_css_string(PrinterOptions::default())
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }
}

fn compress_box_shorthand(values: [PropertyValue; 4]) -> PropertyValue {
    let [top, right, bottom, left] = values;
    if top == right && top == bottom && top == left {
        top
    } else if top == bottom && right == left {
        PropertyValue::List(vec![top, right])
    } else if right == left {
        PropertyValue::List(vec![top, right, bottom])
    } else {
        PropertyValue::List(vec![top, right, bottom, left])
    }
}

fn compress_pair_shorthand(first: PropertyValue, second: PropertyValue) -> PropertyValue {
    if first == second {
        first
    } else {
        PropertyValue::List(vec![first, second])
    }
}
