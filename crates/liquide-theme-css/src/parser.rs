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
                // Serialize the media query condition for later evaluation
                let condition = self.to_css_string(&media.query);
                // Process nested rules, tagging each with the media condition
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(nested_rule, stylesheet, Some(&condition))?;
                }
            }
            CssRule::Supports(supports) => {
                // Serialize the @supports condition
                let condition_str = self.to_css_string(&supports.condition);
                // Evaluate simple @supports conditions at parse time
                if self.evaluate_supports_condition(&condition_str) {
                    for nested_rule in &supports.rules.0 {
                        self.process_rule(nested_rule, stylesheet)?;
                    }
                }
                // If the condition doesn't match, the rules are dropped.
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

                let mut weight: Option<(u16, u16)> = None;
                let mut style: Option<String> = None;
                let mut unicode_range: Option<String> = None;

                for prop in &font_face.properties {
                    match prop {
                        lightningcss::rules::font_face::FontFaceProperty::FontWeight(w) => {
                            let w0 = self.to_css_string(&w.0);
                            let w1 = self.to_css_string(&w.1);
                            let v0 = match w0.trim() {
                                "normal" => 400u16,
                                "bold" => 700,
                                other => other.parse::<f32>().unwrap_or(400.0) as u16,
                            };
                            let v1 = match w1.trim() {
                                "normal" => 400u16,
                                "bold" => 700,
                                other => other.parse::<f32>().unwrap_or(v0 as f32) as u16,
                            };
                            weight = Some((v0, v1));
                        }
                        lightningcss::rules::font_face::FontFaceProperty::FontStyle(fs) => {
                            style = Some(self.to_css_string(fs));
                        }
                        lightningcss::rules::font_face::FontFaceProperty::UnicodeRange(ranges) => {
                            let range_strs: Vec<String> = ranges.iter().map(|r| {
                                if r.start == r.end {
                                    format!("U+{:X}", r.start)
                                } else {
                                    format!("U+{:X}-{:X}", r.start, r.end)
                                }
                            }).collect();
                            unicode_range = Some(range_strs.join(", "));
                        }
                        _ => {}
                    }
                }

                stylesheet.add_font_face(FontFaceRule {
                    family,
                    src: sources,
                    weight,
                    style,
                    display: None,
                    unicode_range,
                });
            }
            CssRule::Import(import) => {
                stylesheet.add_import(import.url.to_string());
            }
            CssRule::LayerStatement(layer_stmt) => {
                // @layer declaration (ordering): @layer reset, base, components;
                for name in &layer_stmt.names {
                    let layer_name = self.to_css_string(name);
                    stylesheet.add_layer(&layer_name);
                }
            }
            CssRule::LayerBlock(layer_block) => {
                // @layer name { ... } — rules inside a named cascade layer
                let layer_name = layer_block.name.as_ref()
                    .map(|n| self.to_css_string(n))
                    .unwrap_or_default();
                if !layer_name.is_empty() {
                    stylesheet.add_layer(&layer_name);
                }
                for nested_rule in &layer_block.rules.0 {
                    self.process_rule_in_layer(nested_rule, stylesheet, &layer_name)?;
                }
            }
            CssRule::Container(container) => {
                // @container (condition) { ... } — container queries
                let condition = self.to_css_string(&container.condition);
                let name = container.name.as_ref().map(|n| {
                    self.to_css_string(n)
                });
                let mut container_rules = Vec::new();
                for nested_rule in &container.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties = self.convert_declarations(&style_rule.declarations)?;
                            container_rules.push(crate::stylesheet::StyleRule::new(our_selector, properties));
                        }
                    }
                }
                stylesheet.add_container_rule(crate::stylesheet::ContainerRule {
                    name,
                    condition,
                    rules: container_rules,
                });
            }
            CssRule::Property(property) => {
                // @property --name { syntax: "<color>"; inherits: false; initial-value: red; }
                let name = self.to_css_string(&property.name);
                let syntax = match &property.syntax {
                    lightningcss::values::syntax::SyntaxString::Universal => "*".to_string(),
                    lightningcss::values::syntax::SyntaxString::Components(c) => {
                        self.to_css_string(c)
                    }
                };
                let inherits = property.inherits;
                let initial_value = property.initial_value.as_ref().map(|v| {
                    self.to_css_string(v)
                });
                stylesheet.add_registered_property(crate::stylesheet::RegisteredProperty {
                    name,
                    syntax,
                    inherits,
                    initial_value,
                });
            }
            // Ignore rule types we don't yet handle
            _ => {}
        }

        Ok(())
    }

    /// Process a CSS rule that lives inside an @media block, tagging output rules with the condition.
    fn process_rule_with_media(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        media_condition: Option<&str>,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;
                    let properties = self.convert_declarations(&style_rule.declarations)?;
                    if let Some(condition) = media_condition {
                        stylesheet.add_conditional_rule(our_selector, properties, condition.to_string());
                    } else {
                        stylesheet.add_rule(our_selector, properties);
                    }
                }
            }
            CssRule::Media(media) => {
                // Nested @media: combine conditions with "and"
                let inner_condition = self.to_css_string(&media.query);
                let combined = match media_condition {
                    Some(outer) => format!("{} and {}", outer, inner_condition),
                    None => inner_condition,
                };
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(nested_rule, stylesheet, Some(&combined))?;
                }
            }
            // For any other rule type inside @media, delegate to normal processing
            _ => {
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    /// Process a CSS rule that lives inside a @layer block.
    fn process_rule_in_layer(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        layer_name: &str,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;
                    let properties = self.convert_declarations(&style_rule.declarations)?;
                    stylesheet.add_layer_rule(layer_name, our_selector, properties);
                }
            }
            _ => {
                // For other rule types inside @layer, delegate to normal processing
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    /// Evaluate a serialized @supports condition.
    ///
    /// Returns `true` if the condition is satisfied by our engine's property set.
    /// Unknown conditions default to `true` so that the rules are included.
    fn evaluate_supports_condition(&self, condition: &str) -> bool {
        let condition = condition.trim();

        // Handle "not <condition>"
        if let Some(inner) = condition.strip_prefix("not ") {
            return !self.evaluate_supports_condition(inner.trim());
        }

        // Handle parenthesized declaration: "(property: value)"
        if condition.starts_with('(') && condition.ends_with(')') {
            let inner = &condition[1..condition.len() - 1].trim();
            // Check for and/or inside parens (compound condition)
            if inner.contains(") and (") || inner.contains(") or (") {
                // Compound — split on " and " / " or "
                if let Some(_) = condition.find(") and (") {
                    return condition
                        .split(" and ")
                        .all(|part| self.evaluate_supports_condition(part.trim()));
                }
                if let Some(_) = condition.find(") or (") {
                    return condition
                        .split(" or ")
                        .any(|part| self.evaluate_supports_condition(part.trim()));
                }
            }
            // Simple property: value declaration check
            if let Some(colon_pos) = inner.find(':') {
                let property = inner[..colon_pos].trim();
                return Self::is_supported_css_property(property);
            }
            // Might be a nested condition
            return self.evaluate_supports_condition(inner);
        }

        // If we see " and " at the top level
        if condition.contains(" and ") {
            return condition
                .split(" and ")
                .all(|part| self.evaluate_supports_condition(part.trim()));
        }
        // If we see " or "
        if condition.contains(" or ") {
            return condition
                .split(" or ")
                .any(|part| self.evaluate_supports_condition(part.trim()));
        }

        // Default: assume supported
        true
    }

    /// Check if a CSS property name is supported by our engine.
    fn is_supported_css_property(property: &str) -> bool {
        matches!(
            property.trim(),
            "display"
                | "position"
                | "width"
                | "height"
                | "min-width"
                | "max-width"
                | "min-height"
                | "max-height"
                | "margin"
                | "margin-top"
                | "margin-right"
                | "margin-bottom"
                | "margin-left"
                | "padding"
                | "padding-top"
                | "padding-right"
                | "padding-bottom"
                | "padding-left"
                | "color"
                | "background"
                | "background-color"
                | "border"
                | "border-color"
                | "border-width"
                | "border-style"
                | "border-radius"
                | "font-size"
                | "font-weight"
                | "font-family"
                | "font-style"
                | "line-height"
                | "text-align"
                | "text-transform"
                | "text-overflow"
                | "white-space"
                | "opacity"
                | "visibility"
                | "overflow"
                | "overflow-x"
                | "overflow-y"
                | "flex"
                | "flex-direction"
                | "flex-wrap"
                | "flex-grow"
                | "flex-shrink"
                | "flex-basis"
                | "justify-content"
                | "align-items"
                | "align-self"
                | "align-content"
                | "gap"
                | "row-gap"
                | "column-gap"
                | "grid-template-columns"
                | "grid-template-rows"
                | "grid-auto-flow"
                | "grid-column"
                | "grid-row"
                | "z-index"
                | "cursor"
                | "pointer-events"
                | "box-shadow"
                | "transform"
                | "transition"
                | "box-sizing"
                | "top"
                | "right"
                | "bottom"
                | "left"
                | "order"
                | "letter-spacing"
                | "word-spacing"
                | "text-indent"
                | "word-break"
        )
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
                    if !trimmed.ends_with("px") && !trimmed.ends_with("em") && !trimmed.ends_with("rem") && !trimmed.ends_with('%') {
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
                properties.insert("border-top-style".into(), self.convert_line_style(&bs.top));
                properties.insert("border-right-style".into(), self.convert_line_style(&bs.right));
                properties.insert("border-bottom-style".into(), self.convert_line_style(&bs.bottom));
                properties.insert("border-left-style".into(), self.convert_line_style(&bs.left));
                properties.insert("border-style".into(), self.convert_line_style(&bs.top));
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
                if let Some(v) = self.convert_border_width(&bw.top) {
                    properties.insert("border-top-width".into(), v);
                }
                if let Some(v) = self.convert_border_width(&bw.right) {
                    properties.insert("border-right-width".into(), v);
                }
                if let Some(v) = self.convert_border_width(&bw.bottom) {
                    properties.insert("border-bottom-width".into(), v);
                }
                if let Some(v) = self.convert_border_width(&bw.left) {
                    properties.insert("border-left-width".into(), v);
                }
                if let Some(v) = self.convert_border_width(&bw.top) {
                    properties.insert("border-width".into(), v);
                }
            }

            // ── Flex extras ─────────────────────────────────────────────
            Property::FlexBasis(fb, _prefix) => {
                let css_str = self.to_css_string(fb);
                if let Some(v) = self.parse_length_value(&css_str) {
                    properties.insert("flex-basis".into(), v);
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
                }
            }

            // ── Grid ────────────────────────────────────────────────────
            Property::GridTemplateColumns(tracks) => {
                let css_str = self.to_css_string(tracks);
                properties.insert("grid-template-columns".into(), PropertyValue::Keyword(css_str));
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

            // ── Pointer events ───────────────────────────────────────────
            // pointer-events is not a lightningcss Property variant; handled via custom parsing if needed

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
                // Try oklch/oklab/color-mix first, then fall back
                if let Ok(color) = Color::parse_css(&css_str) {
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
                TokenOrValue::Var(var) => {
                    // Serialize var() properly: var(--name) or var(--name, fallback)
                    result.push_str("var(");
                    result.push_str(&var.name.ident.0);
                    if let Some(fallback) = &var.fallback {
                        result.push_str(", ");
                        result.push_str(&self.to_css_string_from_token_list(fallback));
                    }
                    result.push(')');
                }
                TokenOrValue::Env(env) => {
                    result.push_str("env(");
                    result.push_str(&self.to_css_string(&env.name));
                    if let Some(fallback) = &env.fallback {
                        result.push_str(", ");
                        result.push_str(&self.to_css_string_from_token_list(fallback));
                    }
                    result.push(')');
                }
                TokenOrValue::Function(func) => {
                    result.push_str(&func.name);
                    result.push('(');
                    result.push_str(&self.to_css_string_from_token_list(&func.arguments));
                    result.push(')');
                }
                TokenOrValue::DashedIdent(ident) => {
                    result.push_str(&ident.0);
                }
                _ => {
                    // UnresolvedColor, Url, AnimationName, etc.
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
        } else if let Some(v) = s.strip_suffix("rem") {
            // Must check rem before em to avoid false match
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Rem(n)))
        } else if let Some(v) = s.strip_suffix("em") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Em(n)))
        } else if let Some(v) = s.strip_suffix("vmin") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Vmin(n)))
        } else if let Some(v) = s.strip_suffix("vmax") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Vmax(n)))
        } else if let Some(v) = s.strip_suffix("vw") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Vw(n)))
        } else if let Some(v) = s.strip_suffix("vh") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Vh(n)))
        } else if let Some(v) = s.strip_suffix("ch") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Ch(n)))
        } else if let Some(v) = s.strip_suffix("ex") {
            v.trim().parse::<f32>().ok().map(|n| PropertyValue::Length(LengthUnit::Ex(n)))
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

        // Try calc() / min() / max() / clamp() math expressions
        if s.starts_with("calc(")
            || s.starts_with("min(")
            || s.starts_with("max(")
            || s.starts_with("clamp(")
        {
            if let Some(expr) = self.parse_math_expr(s) {
                return PropertyValue::MathExpr(expr);
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

    // ── calc() / min() / max() / clamp() parsing ────────────────────

    /// Parse a CSS math expression string into a `CssMathExpr`.
    fn parse_math_expr(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        if let Some(inner) = Self::strip_function(s, "calc") {
            return self.parse_calc_expr(inner);
        }
        if let Some(inner) = Self::strip_function(s, "min") {
            let args = Self::split_function_args(inner);
            let exprs: Option<Vec<_>> = args.iter().map(|a| self.parse_calc_atom(a.trim())).collect();
            return exprs.map(crate::value::CssMathExpr::Min);
        }
        if let Some(inner) = Self::strip_function(s, "max") {
            let args = Self::split_function_args(inner);
            let exprs: Option<Vec<_>> = args.iter().map(|a| self.parse_calc_atom(a.trim())).collect();
            return exprs.map(crate::value::CssMathExpr::Max);
        }
        if let Some(inner) = Self::strip_function(s, "clamp") {
            let args = Self::split_function_args(inner);
            if args.len() == 3 {
                let min = self.parse_calc_atom(args[0].trim())?;
                let pref = self.parse_calc_atom(args[1].trim())?;
                let max = self.parse_calc_atom(args[2].trim())?;
                return Some(crate::value::CssMathExpr::Clamp {
                    min: Box::new(min),
                    preferred: Box::new(pref),
                    max: Box::new(max),
                });
            }
        }
        None
    }

    /// Parse the inside of a `calc(...)` expression (supports +, -, *, /).
    fn parse_calc_expr(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        // Try to split on + or - at the top level (outside parens).
        // Addition/subtraction are the lowest precedence operators.
        if let Some((left, op, right)) = Self::split_additive(s) {
            let lhs = self.parse_calc_term(left.trim())?;
            let rhs = self.parse_calc_term(right.trim())?;
            return Some(if op == '+' {
                crate::value::CssMathExpr::Add(Box::new(lhs), Box::new(rhs))
            } else {
                crate::value::CssMathExpr::Sub(Box::new(lhs), Box::new(rhs))
            });
        }
        self.parse_calc_term(s)
    }

    /// Parse a multiplicative term (handles * and /).
    fn parse_calc_term(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        if let Some((left, op, right)) = Self::split_multiplicative(s) {
            let lhs = self.parse_calc_atom(left.trim())?;
            let rhs = self.parse_calc_atom(right.trim())?;
            return Some(if op == '*' {
                crate::value::CssMathExpr::Mul(Box::new(lhs), Box::new(rhs))
            } else {
                crate::value::CssMathExpr::Div(Box::new(lhs), Box::new(rhs))
            });
        }
        self.parse_calc_atom(s)
    }

    /// Parse a calc atom: a number, length, parenthesized sub-expression, or nested function.
    fn parse_calc_atom(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        // Nested function (calc, min, max, clamp)
        if s.starts_with("calc(") || s.starts_with("min(") || s.starts_with("max(") || s.starts_with("clamp(") {
            return self.parse_math_expr(s);
        }
        // Parenthesized sub-expression
        if s.starts_with('(') && s.ends_with(')') {
            return self.parse_calc_expr(&s[1..s.len() - 1]);
        }
        // Try as length
        if let Some(pv) = self.parse_length_value(s) {
            if let PropertyValue::Length(lu) = pv {
                return Some(crate::value::CssMathExpr::Value(lu));
            }
        }
        // Try as bare number
        if let Ok(n) = s.parse::<f32>() {
            return Some(crate::value::CssMathExpr::Number(n));
        }
        None
    }

    /// Strip a function wrapper: e.g. `calc(100% - 20px)` → `100% - 20px`.
    fn strip_function<'a>(s: &'a str, name: &str) -> Option<&'a str> {
        let s = s.trim();
        if s.starts_with(name)
            && s[name.len()..].starts_with('(')
            && s.ends_with(')')
        {
            Some(&s[name.len() + 1..s.len() - 1])
        } else {
            None
        }
    }

    /// Split function arguments by commas at the top level (respecting nested parens).
    fn split_function_args(s: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        for ch in s.chars() {
            match ch {
                '(' => { depth += 1; current.push(ch); }
                ')' => { depth -= 1; current.push(ch); }
                ',' if depth == 0 => {
                    args.push(std::mem::take(&mut current));
                }
                _ => current.push(ch),
            }
        }
        if !current.is_empty() {
            args.push(current);
        }
        args
    }

    /// Split on the *last* top-level `+` or `-` (lowest precedence, left-associative).
    /// We scan right-to-left, but the `-` must be preceded by a space to differentiate
    /// from negative numbers (e.g. `-20px`).
    fn split_additive(s: &str) -> Option<(&str, char, &str)> {
        let bytes = s.as_bytes();
        let mut depth: i32 = 0;
        // Scan right to left
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b')' => depth += 1,
                b'(' => depth -= 1,
                b'+' if depth == 0 && i > 0 => {
                    // Require whitespace around operator for calc
                    return Some((&s[..i].trim_end(), '+', &s[i + 1..]));
                }
                b'-' if depth == 0 && i > 0 && bytes[i - 1] == b' ' => {
                    return Some((&s[..i].trim_end(), '-', &s[i + 1..]));
                }
                _ => {}
            }
        }
        None
    }

    /// Split on the *last* top-level `*` or `/`.
    fn split_multiplicative(s: &str) -> Option<(&str, char, &str)> {
        let bytes = s.as_bytes();
        let mut depth: i32 = 0;
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b')' => depth += 1,
                b'(' => depth -= 1,
                b'*' if depth == 0 => {
                    return Some((&s[..i], '*', &s[i + 1..]));
                }
                b'/' if depth == 0 => {
                    return Some((&s[..i], '/', &s[i + 1..]));
                }
                _ => {}
            }
        }
        None
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
