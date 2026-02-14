//! CSS parser for themes using lightningcss

use crate::error::{Result, ThemeError};
use crate::property::PropertySet;
use crate::selector::Selector;
use crate::stylesheet::StyleSheet;
use crate::value::{
    Color, FontFaceRule, FontSource, Keyframe, KeyframesRule, LengthUnit, PropertyValue,
};
use std::path::Path;

use lightningcss::printer::Printer;
use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet as LightningStyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::color::CssColor;
use lightningcss::values::length::LengthPercentageOrAuto;

/// CSS theme parser using lightningcss for full CSS3 support
pub struct ThemeParser {}

impl Default for ThemeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeParser {
    /// Create a new theme parser
    pub fn new() -> Self {
        Self {}
    }

    /// Parse CSS from a string
    pub fn parse_str(&self, css: &str) -> Result<StyleSheet> {
        // Parse with lightningcss - use default options with static lifetime
        let options = ParserOptions::default();
        let lightning_sheet =
            LightningStyleSheet::parse(css, options).map_err(|e| ThemeError::ParseError {
                message: format!("lightningcss parse error: {:?}", e),
                location: "unknown".to_string(),
            })?;

        // Convert to our stylesheet format
        self.convert_stylesheet(lightning_sheet)
    }

    /// Parse CSS from a file
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<StyleSheet> {
        let css = std::fs::read_to_string(path)?;
        self.parse_str(&css)
    }

    /// Convert lightningcss StyleSheet to our StyleSheet format
    fn convert_stylesheet(&self, lightning: LightningStyleSheet) -> Result<StyleSheet> {
        let mut stylesheet = StyleSheet::new();

        // Process all rules
        for rule in lightning.rules.0.iter() {
            self.process_rule(rule, &mut stylesheet)?;
        }

        Ok(stylesheet)
    }

    /// Process a CSS rule recursively
    fn process_rule(&self, rule: &CssRule, stylesheet: &mut StyleSheet) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                // Convert selector list to our format
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;

                    // Convert declarations to properties
                    let properties = self.convert_declarations(&style_rule.declarations)?;

                    stylesheet.add_rule(our_selector, properties);
                }
            }
            CssRule::Media(media) => {
                // Process nested rules in media queries
                for nested_rule in &media.rules.0 {
                    self.process_rule(nested_rule, stylesheet)?;
                }
            }
            CssRule::Supports(supports) => {
                // Process nested rules in @supports
                for nested_rule in &supports.rules.0 {
                    self.process_rule(nested_rule, stylesheet)?;
                }
            }
            CssRule::Keyframes(keyframes) => {
                let name = match &keyframes.name {
                    lightningcss::rules::keyframes::KeyframesName::Ident(ident) => ident.0.to_string(),
                    lightningcss::rules::keyframes::KeyframesName::Custom(s) => s.to_string(),
                };
                let mut frames = Vec::new();
                for kf in &keyframes.keyframes {
                    let mut selectors = Vec::new();
                    for sel in &kf.selectors {
                        match sel {
                            lightningcss::rules::keyframes::KeyframeSelector::Percentage(p) => {
                                selectors.push(p.0);
                            }
                            lightningcss::rules::keyframes::KeyframeSelector::From => {
                                selectors.push(0.0);
                            }
                            lightningcss::rules::keyframes::KeyframeSelector::To => {
                                selectors.push(1.0);
                            }
                            _ => {
                                // TimelineRangePercentage and future variants — skip
                            }
                        }
                    }
                    let declarations =
                        self.convert_declarations_to_pairs(&kf.declarations)?;
                    frames.push(Keyframe {
                        selectors,
                        declarations,
                    });
                }
                stylesheet.add_keyframes(KeyframesRule {
                    name,
                    keyframes: frames,
                });
            }
            CssRule::FontFace(font_face) => {
                let family = font_face
                    .properties
                    .iter()
                    .find_map(|p| {
                        if let lightningcss::rules::font_face::FontFaceProperty::FontFamily(f) = p {
                            Some(format!("{:?}", f))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let mut sources = Vec::new();
                for prop in &font_face.properties {
                    if let lightningcss::rules::font_face::FontFaceProperty::Source(src_list) = prop {
                        for src in src_list.iter() {
                            match src {
                                lightningcss::rules::font_face::Source::Url(url_src) => {
                                    sources.push(FontSource::Url {
                                        url: url_src.url.url.to_string(),
                                        format: url_src.format.as_ref().map(|f| format!("{:?}", f)),
                                    });
                                }
                                lightningcss::rules::font_face::Source::Local(local) => {
                                    sources.push(FontSource::Local(format!("{:?}", local)));
                                }
                            }
                        }
                    }
                }

                stylesheet.add_font_face(FontFaceRule {
                    family,
                    src: sources,
                    weight: None,
                    style: None,
                    display: None,
                    unicode_range: None,
                });
            }
            CssRule::Import(import) => {
                stylesheet.add_import(import.url.to_string());
            }
            // Ignore rule types we don't yet handle
            _ => {}
        }

        Ok(())
    }

    /// Convert lightningcss selector to string
    fn selector_to_string(
        &self,
        selector: &lightningcss::selector::Selector<'_>,
    ) -> Result<String> {
        let mut css_string = String::new();
        let mut printer = Printer::new(&mut css_string, PrinterOptions::default());
        selector
            .to_css(&mut printer)
            .map_err(|e| ThemeError::ParseError {
                message: format!("Failed to serialize selector: {:?}", e),
                location: "selector".to_string(),
            })?;
        Ok(css_string)
    }

    /// Convert lightningcss declarations to our PropertySet
    fn convert_declarations(
        &self,
        decls: &lightningcss::declaration::DeclarationBlock,
    ) -> Result<PropertySet> {
        let mut properties = PropertySet::new();

        // Process normal declarations
        for decl in &decls.declarations {
            self.insert_property(decl, &mut properties);
        }

        // Process !important declarations (these override normal ones)
        for decl in &decls.important_declarations {
            self.insert_property(decl, &mut properties);
        }

        Ok(properties)
    }

    /// Convert declarations to (name, value) pairs — used for @keyframes.
    fn convert_declarations_to_pairs(
        &self,
        decls: &lightningcss::declaration::DeclarationBlock,
    ) -> Result<Vec<(String, PropertyValue)>> {
        let props = self.convert_declarations(decls)?;
        Ok(props
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect())
    }

    /// Insert converted properties from a single declaration into the property set.
    /// A single declaration can expand into multiple properties (e.g. shorthand `background`
    /// produces both "background" and "background-color").
    fn insert_property(&self, prop: &Property, properties: &mut PropertySet) {
        match prop {
            // ── Background ──────────────────────────────────────────────
            Property::BackgroundColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("background-color".into(), v.clone());
                    properties.insert("background".into(), v);
                }
            }
            Property::Background(backgrounds) => {
                // Extract color from first background layer
                if let Some(bg) = backgrounds.first() {
                    if let Some(v) = self.convert_color(&bg.color) {
                        properties.insert("background".into(), v.clone());
                        properties.insert("background-color".into(), v);
                    }
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
                    properties.insert("border-top-color".into(), v.clone());
                    // Also set generic border-color if not set yet
                    if !properties.has("border-color") {
                        properties.insert("border-color".into(), v);
                    }
                }
            }
            Property::BorderRightColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-right-color".into(), v);
                }
            }
            Property::BorderBottomColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-bottom-color".into(), v.clone());
                    if !properties.has("border-color") {
                        properties.insert("border-color".into(), v);
                    }
                }
            }
            Property::BorderLeftColor(color) => {
                if let Some(v) = self.convert_color(color) {
                    properties.insert("border-left-color".into(), v);
                }
            }

            // ── Border color shorthand ──────────────────────────────────
            Property::BorderColor(bc) => {
                if let Some(v) = self.convert_color(&bc.top) {
                    properties.insert("border-color".into(), v.clone());
                    properties.insert("border-top-color".into(), v);
                }
                if let Some(v) = self.convert_color(&bc.right) {
                    properties.insert("border-right-color".into(), v);
                }
                if let Some(v) = self.convert_color(&bc.bottom) {
                    properties.insert("border-bottom-color".into(), v);
                }
                if let Some(v) = self.convert_color(&bc.left) {
                    properties.insert("border-left-color".into(), v);
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
            }
            Property::BorderRight(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-right-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-right-width".into(), v);
                }
            }
            Property::BorderBottom(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-bottom-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-bottom-width".into(), v);
                }
            }
            Property::BorderLeft(border) => {
                if let Some(v) = self.convert_color(&border.color) {
                    properties.insert("border-left-color".into(), v);
                }
                if let Some(v) = self.convert_border_width(&border.width) {
                    properties.insert("border-left-width".into(), v);
                }
            }

            // ── Border widths ───────────────────────────────────────────
            Property::BorderTopWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-top-width".into(), v.clone());
                    if !properties.has("border-width") {
                        properties.insert("border-width".into(), v);
                    }
                }
            }
            Property::BorderRightWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-right-width".into(), v);
                }
            }
            Property::BorderBottomWidth(width) => {
                if let Some(v) = self.convert_border_width(width) {
                    properties.insert("border-bottom-width".into(), v.clone());
                    if !properties.has("border-width") {
                        properties.insert("border-width".into(), v);
                    }
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
                }
            }
            Property::Height(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("height".into(), v);
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
                if let Some(v) = self.convert_length_percentage_or_auto(&padding.top) {
                    properties.insert("padding-top".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&padding.right) {
                    properties.insert("padding-right".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&padding.bottom) {
                    properties.insert("padding-bottom".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&padding.left) {
                    properties.insert("padding-left".into(), v);
                }
                // Also set shorthand "padding" to top value for simple cases
                if let Some(v) = self.convert_length_percentage_or_auto(&padding.top) {
                    properties.insert("padding".into(), v);
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
                if let Some(v) = self.convert_length_percentage_or_auto(&margin.top) {
                    properties.insert("margin-top".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&margin.right) {
                    properties.insert("margin-right".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&margin.bottom) {
                    properties.insert("margin-bottom".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&margin.left) {
                    properties.insert("margin-left".into(), v);
                }
                if let Some(v) = self.convert_length_percentage_or_auto(&margin.top) {
                    properties.insert("margin".into(), v);
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
                if let Ok(n) = css_str.trim().parse::<f32>() {
                    properties.insert("font-weight".into(), PropertyValue::Number(n));
                }
            }
            Property::LineHeight(lh) => {
                let css_str = self.to_css_string(lh);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("line-height".into(), v);
                } else if let Ok(n) = css_str.trim().parse::<f32>() {
                    properties.insert("line-height".into(), PropertyValue::Number(n));
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
                    let color = self.convert_color(&shadow.color)
                        .and_then(|v| if let PropertyValue::Color(c) = v { Some(c) } else { None })
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
                    properties.insert("shadow-offset-x".into(), PropertyValue::Length(LengthUnit::Px(offset_x)));
                    properties.insert("shadow-offset-y".into(), PropertyValue::Length(LengthUnit::Px(offset_y)));
                    properties.insert("shadow-blur".into(), PropertyValue::Length(LengthUnit::Px(blur)));
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
                properties.insert("overflow-y".into(), PropertyValue::Keyword(y));
                properties.insert("overflow".into(), PropertyValue::Keyword(x));
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
                if let Some(v) = self.parse_length_value(&row_str) {
                    properties.insert("row-gap".into(), v.clone());
                    properties.insert("gap".into(), v);
                }
                if let Some(v) = self.parse_length_value(&col_str) {
                    properties.insert("column-gap".into(), v);
                }
            }
            Property::RowGap(gap) => {
                let css_str = self.to_css_string(gap);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("row-gap".into(), v);
                }
            }
            Property::ColumnGap(gap) => {
                let css_str = self.to_css_string(gap);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("column-gap".into(), v);
                }
            }

            // ── Inset properties (top/right/bottom/left) ───────────────
            Property::Top(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("top".into(), v);
                }
            }
            Property::Right(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("right".into(), v);
                }
            }
            Property::Bottom(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("bottom".into(), v);
                }
            }
            Property::Left(val) => {
                let css_str = self.to_css_string(val);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("left".into(), v);
                }
            }

            // ── Min/max dimensions ──────────────────────────────────────
            Property::MinWidth(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("min-width".into(), v);
                }
            }
            Property::MaxWidth(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("max-width".into(), v);
                }
            }
            Property::MinHeight(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("min-height".into(), v);
                }
            }
            Property::MaxHeight(size) => {
                let css_str = self.to_css_string(size);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("max-height".into(), v);
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
            Property::Cursor(cursor) => {
                let css_str = self.to_css_string(cursor);
                properties.insert("cursor".into(), PropertyValue::Keyword(css_str));
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
                let value_str = self.to_css_string_from_token_list(&custom.value);
                // Parse value as color, length, or string
                let pv = self.parse_value_string(&value_str);
                properties.insert(name, pv);
            }

            // ── Unparsed properties (non-standard names like glass-tint) ─
            Property::Unparsed(unparsed) => {
                let name = self.to_css_string(&unparsed.property_id);
                let value_str = self.to_css_string_from_token_list(&unparsed.value);
                let pv = self.parse_value_string(&value_str);
                properties.insert(name, pv);
            }

            // ── Catch-all: store property name so we know it was declared ──
            _ => {
                // We can serialize the property_id but not the Property itself.
                // Record the property name with a debug representation.
                let prop_id = prop.property_id();
                let name = self.to_css_string(&prop_id);
                if !name.is_empty() {
                    // Use Debug format as best-effort value
                    let debug_val = format!("{:?}", prop);
                    let pv = self.parse_value_string(&debug_val);
                    properties.insert(name, pv);
                }
            }
        }
    }

    /// Convert lightningcss color to our Color type
    fn convert_color(&self, css_color: &CssColor) -> Option<PropertyValue> {
        match css_color {
            CssColor::RGBA(rgba) => Some(PropertyValue::Color(Color::new(
                rgba.red, rgba.green, rgba.blue, rgba.alpha,
            ))),
            _ => {
                // For other color types, try to serialize and parse
                let css_str = self.to_css_string(css_color);
                if let Ok(color) = Color::from_hex(&css_str) {
                    return Some(PropertyValue::Color(color));
                }
                None
            }
        }
    }

    /// Convert LengthPercentageOrAuto to PropertyValue
    fn convert_length_percentage_or_auto(
        &self,
        val: &LengthPercentageOrAuto,
    ) -> Option<PropertyValue> {
        match val {
            LengthPercentageOrAuto::Auto => None,
            LengthPercentageOrAuto::LengthPercentage(lp) => {
                let css_str = self.to_css_string(lp);
                self.parse_length_value(&css_str)
            }
        }
    }

    /// Convert a CSS line style to PropertyValue
    fn convert_line_style<S: ToCss>(&self, style: &S) -> PropertyValue {
        let css_str = self.to_css_string(style);
        PropertyValue::Keyword(css_str)
    }

    /// Convert border width
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

    /// Serialize any ToCss value to string
    fn to_css_string<T: ToCss>(&self, value: &T) -> String {
        let mut s = String::new();
        let mut printer = Printer::new(&mut s, PrinterOptions::default());
        let _ = value.to_css(&mut printer);
        s
    }

    /// Serialize a TokenList to string by iterating its public token vector.
    fn to_css_string_from_token_list(
        &self,
        tokens: &lightningcss::properties::custom::TokenList,
    ) -> String {
        use lightningcss::properties::custom::TokenOrValue;
        let mut result = String::new();
        for token_or_value in &tokens.0 {
            match token_or_value {
                TokenOrValue::Color(color) => {
                    result.push_str(&self.to_css_string(color));
                }
                TokenOrValue::Length(length) => {
                    result.push_str(&self.to_css_string(length));
                }
                TokenOrValue::Angle(angle) => {
                    result.push_str(&self.to_css_string(angle));
                }
                TokenOrValue::Time(time) => {
                    result.push_str(&self.to_css_string(time));
                }
                TokenOrValue::Resolution(res) => {
                    result.push_str(&self.to_css_string(res));
                }
                TokenOrValue::Token(token) => {
                    result.push_str(&self.to_css_string(token));
                }
                _ => {
                    // Var, Env, Function, DashedIdent, etc.
                    result.push_str(&format!("{:?}", token_or_value));
                }
            }
        }
        result.trim().to_string()
    }

    /// Parse a length string like "10px", "1.5em", "50%", "12pt", "1rem"
    fn parse_length_value(&self, s: &str) -> Option<PropertyValue> {
        let s = s.trim();
        if let Some(v) = s.strip_suffix("px") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Px(n)))
        } else if let Some(v) = s.strip_suffix("em") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Em(n)))
        } else if let Some(v) = s.strip_suffix("rem") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Rem(n)))
        } else if let Some(v) = s.strip_suffix("pt") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Pt(n)))
        } else if let Some(v) = s.strip_suffix('%') {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Percent(n)))
        } else {
            // Try as plain number → pixels
            s.parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Px(n)))
        }
    }

    /// Extract px value from a serialized length string
    fn length_to_px(&self, s: &str) -> f32 {
        self.parse_length_value(s)
            .and_then(|v| v.as_length())
            .map(|l| l.to_px(16.0))
            .unwrap_or(0.0)
    }

    /// Attempt to parse a raw value string as color, length, number, or keyword
    fn parse_value_string(&self, s: &str) -> PropertyValue {
        let s = s.trim();

        // Try as color first (hex, rgb(), rgba(), named)
        if let Ok(color) = Color::from_hex(s) {
            return PropertyValue::Color(color);
        }
        // csscolorparser handles rgb()/rgba()/hsl()/named too
        if s.starts_with("rgb") || s.starts_with("hsl") || s.starts_with("hwb") {
            if let Ok(c) = csscolorparser::parse(s) {
                return PropertyValue::Color(Color::new(
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                    (c.a * 255.0) as u8,
                ));
            }
        }

        // Try as length
        if let Some(v) = self.parse_length_value(s) {
            return v;
        }

        // Try as plain number
        if let Ok(n) = s.parse::<f32>() {
            return PropertyValue::Number(n);
        }

        // Fall back to keyword / string
        PropertyValue::Keyword(s.to_string())
    }
}

#[cfg(test)]
mod tests {
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

        // Should parse successfully with lightningcss
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

        // Should parse successfully with lightningcss
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

        // Should parse successfully with lightningcss (full CSS3 support)
        assert!(result.is_ok());
    }
}
