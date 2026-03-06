//! CSS rule processing — dispatches at-rules and style rules, handles nesting.
//!
//! Processes the top-level rule tree from lightningcss, converting each rule type
//! (@media, @supports, @keyframes, @font-face, @layer, @container, @scope, etc.)
//! into our `StyleSheet` representation. Supports CSS Nesting Level 1 with `&`
//! parent-selector resolution.

use crate::error::Result;
use crate::selector::Selector;
use crate::stylesheet::StyleSheet;
use crate::value::{FontFaceRule, FontSource, Keyframe, KeyframesRule};

use lightningcss::rules::CssRule;

use super::ThemeParser;

impl ThemeParser {
    /// Process a CSS rule recursively.
    pub(crate) fn process_rule(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                // Convert selector list to our format
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;

                    // Convert declarations to properties
                    let properties = self.convert_declarations(&style_rule.declarations)?;

                    stylesheet.add_rule_with_conditions(
                        our_selector,
                        properties,
                        None,
                        None,
                        None,
                    );
                }
                // CSS Nesting Level 1: process nested rules within this style rule.
                // Nested rules inherit the parent selector context — `&` references
                // are resolved by substituting the parent selector string.
                if !style_rule.rules.0.is_empty() {
                    let parent_selectors: Vec<String> = style_rule
                        .selectors
                        .0
                        .iter()
                        .filter_map(|s| self.selector_to_string(s).ok())
                        .collect();
                    for nested_rule in &style_rule.rules.0 {
                        self.process_nested_rule(
                            nested_rule,
                            &parent_selectors,
                            stylesheet,
                            None,
                            None,
                            None,
                        )?;
                    }
                }
            }
            CssRule::Media(media) => {
                // Serialize the media query condition for later evaluation
                let condition = self.to_css_string(&media.query);
                // Process nested rules, tagging each with the media condition
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(
                        nested_rule,
                        stylesheet,
                        Some(&condition),
                        None,
                        None,
                    )?;
                }
            }
            CssRule::Supports(supports) => {
                // Serialize the @supports condition
                let condition_str = self.to_css_string(&supports.condition);
                // Preserve supports condition for runtime evaluation.
                for nested_rule in &supports.rules.0 {
                    self.process_rule_with_supports(
                        nested_rule,
                        stylesheet,
                        Some(&condition_str),
                        None,
                        None,
                    )?;
                }
            }
            CssRule::Keyframes(keyframes) => {
                let name = match &keyframes.name {
                    lightningcss::rules::keyframes::KeyframesName::Ident(ident) => {
                        ident.0.to_string()
                    }
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
                    let declarations = self.convert_declarations_to_pairs(&kf.declarations)?;
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
                self.process_font_face(font_face, stylesheet);
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
                let layer_name = layer_block
                    .name
                    .as_ref()
                    .map(|n| self.to_css_string(n))
                    .unwrap_or_default();
                if !layer_name.is_empty() {
                    stylesheet.add_layer(&layer_name);
                }
                for nested_rule in &layer_block.rules.0 {
                    self.process_rule_in_layer(
                        nested_rule,
                        stylesheet,
                        &layer_name,
                        None,
                        None,
                    )?;
                }
            }
            CssRule::Container(container) => {
                // @container (condition) { ... } — container queries
                let condition = self.to_css_string(&container.condition);
                let name = container.name.as_ref().map(|n| self.to_css_string(n));
                let mut container_rules = Vec::new();
                for nested_rule in &container.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties =
                                self.convert_declarations(&style_rule.declarations)?;
                            container_rules.push(crate::stylesheet::StyleRule::new(
                                our_selector,
                                properties,
                            ));
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
                let initial_value = property
                    .initial_value
                    .as_ref()
                    .map(|v| self.to_css_string(v));
                stylesheet.add_registered_property(crate::stylesheet::RegisteredProperty {
                    name,
                    syntax,
                    inherits,
                    initial_value,
                });
            }
            CssRule::Nesting(nesting) => {
                // @nest (deprecated) — process the inner style rule
                let style_rule = &nesting.style;
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;
                    let properties = self.convert_declarations(&style_rule.declarations)?;
                    stylesheet.add_rule(our_selector, properties);
                }
            }
            CssRule::Page(page) => {
                // @page [:first | :left | :right | name] { ... }
                let selectors: Vec<String> = page
                    .selectors
                    .iter()
                    .map(|s| self.to_css_string(s))
                    .collect();
                let properties = self.convert_declarations(&page.declarations)?;
                stylesheet.add_page_rule(crate::stylesheet::PageRule {
                    selectors,
                    properties,
                });
            }
            CssRule::Namespace(ns) => {
                // @namespace [prefix] url(...)
                let prefix = ns.prefix.as_ref().map(|p| p.0.to_string());
                let url = ns.url.to_string();
                stylesheet.add_namespace(crate::stylesheet::NamespaceRule { prefix, url });
            }
            CssRule::CounterStyle(cs) => {
                self.process_counter_style(cs, stylesheet);
            }
            CssRule::Scope(scope) => {
                // @scope (.start) to (.end) { ... }
                let scope_start = scope.scope_start.as_ref().map(|s| self.to_css_string(s));
                let scope_end = scope.scope_end.as_ref().map(|s| self.to_css_string(s));
                let mut scope_style_rules = Vec::new();
                for nested_rule in &scope.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties =
                                self.convert_declarations(&style_rule.declarations)?;
                            scope_style_rules.push(crate::stylesheet::StyleRule::new(
                                our_selector,
                                properties,
                            ));
                        }
                    }
                }
                stylesheet.add_scope_rule(crate::stylesheet::ScopeRule {
                    scope_start,
                    scope_end,
                    rules: scope_style_rules,
                });
            }
            CssRule::StartingStyle(starting) => {
                // @starting-style { ... } — initial styles for transition origins
                for nested_rule in &starting.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties =
                                self.convert_declarations(&style_rule.declarations)?;
                            stylesheet.add_starting_style_rule(
                                crate::stylesheet::StyleRule::new(our_selector, properties),
                            );
                        }
                    }
                }
            }
            // Ignored, deprecated, or niche at-rules that don't affect layout:
            // @viewport (deprecated), @-moz-document (non-standard), @custom-media (draft),
            // @font-palette-values, @font-feature-values, @view-transition, unknown rules.
            _ => {}
        }

        Ok(())
    }

    /// Process a `@font-face` rule.
    fn process_font_face(
        &self,
        font_face: &lightningcss::rules::font_face::FontFaceRule<'_>,
        stylesheet: &mut StyleSheet,
    ) {
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
                    let range_strs: Vec<String> = ranges
                        .iter()
                        .map(|r| {
                            if r.start == r.end {
                                format!("U+{:X}", r.start)
                            } else {
                                format!("U+{:X}-{:X}", r.start, r.end)
                            }
                        })
                        .collect();
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

    /// Process a `@counter-style` rule.
    fn process_counter_style(
        &self,
        cs: &lightningcss::rules::counter_style::CounterStyleRule<'_>,
        stylesheet: &mut StyleSheet,
    ) {
        let name = cs.name.0.to_string();
        let mut system = None;
        let mut symbols = None;
        let mut suffix = None;
        let mut prefix = None;
        let mut negative = None;
        let mut range = None;
        let mut pad = None;
        let mut fallback = None;
        let mut speak_as = None;
        let mut additive_symbols = None;

        // Serialize the entire rule to extract descriptors
        let rule_str = self.to_css_string(cs);
        // Parse "name { system: ...; symbols: ...; }" by extracting
        // the block contents between { and }.
        if let Some(start) = rule_str.find('{') {
            let block = rule_str[start + 1..].trim_end_matches('}').trim();
            for decl_str in block.split(';') {
                let decl_str = decl_str.trim();
                if decl_str.is_empty() {
                    continue;
                }
                if let Some((key, val)) = decl_str.split_once(':') {
                    let key = key.trim();
                    let val = val.trim().to_string();
                    match key {
                        "system" => system = Some(val),
                        "symbols" => symbols = Some(val),
                        "suffix" => suffix = Some(val),
                        "prefix" => prefix = Some(val),
                        "negative" => negative = Some(val),
                        "range" => range = Some(val),
                        "pad" => pad = Some(val),
                        "fallback" => fallback = Some(val),
                        "speak-as" => speak_as = Some(val),
                        "additive-symbols" => additive_symbols = Some(val),
                        _ => {}
                    }
                }
            }
        }

        stylesheet.add_counter_style(crate::stylesheet::CounterStyleRule {
            name,
            system,
            symbols,
            suffix,
            prefix,
            negative,
            range,
            pad,
            fallback,
            speak_as,
            additive_symbols,
        });
    }

    // ── Conditional rule processing ─────────────────────────────────

    /// Process a CSS rule inside an `@media` block, tagging output rules with conditions.
    pub(crate) fn process_rule_with_media(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        media_condition: Option<&str>,
        supports_condition: Option<&str>,
        layer_name: Option<&str>,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                self.process_conditional_style_rule(
                    style_rule,
                    stylesheet,
                    media_condition,
                    supports_condition,
                    layer_name,
                )?;
            }
            CssRule::Media(media) => {
                let inner_condition = self.to_css_string(&media.query);
                let combined = match media_condition {
                    Some(outer) => format!("{} and {}", outer, inner_condition),
                    None => inner_condition,
                };
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        supports_condition,
                        layer_name,
                    )?;
                }
            }
            CssRule::Supports(supports) => {
                let inner = self.to_css_string(&supports.condition);
                let combined = match supports_condition {
                    Some(outer) => format!("({}) and ({})", outer, inner),
                    None => inner,
                };
                for nested_rule in &supports.rules.0 {
                    self.process_rule_with_supports(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        media_condition,
                        layer_name,
                    )?;
                }
            }
            CssRule::LayerBlock(layer_block) => {
                let inner_layer = layer_block
                    .name
                    .as_ref()
                    .map(|n| self.to_css_string(n))
                    .unwrap_or_default();
                if !inner_layer.is_empty() {
                    stylesheet.add_layer(&inner_layer);
                }
                let effective_layer = if inner_layer.is_empty() {
                    layer_name
                } else {
                    Some(inner_layer.as_str())
                };
                for nested_rule in &layer_block.rules.0 {
                    self.process_rule_in_layer(
                        nested_rule,
                        stylesheet,
                        effective_layer.unwrap_or(""),
                        media_condition,
                        supports_condition,
                    )?;
                }
            }
            CssRule::Container(container) => {
                self.process_conditional_container(
                    container,
                    stylesheet,
                    media_condition,
                    supports_condition,
                    layer_name,
                )?;
            }
            _ => {
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    /// Process a CSS rule inside an `@supports` block, tagging output rules with conditions.
    pub(crate) fn process_rule_with_supports(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        supports_condition: Option<&str>,
        media_condition: Option<&str>,
        layer_name: Option<&str>,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                self.process_conditional_style_rule(
                    style_rule,
                    stylesheet,
                    media_condition,
                    supports_condition,
                    layer_name,
                )?;
            }
            CssRule::Supports(supports) => {
                let inner = self.to_css_string(&supports.condition);
                let combined = match supports_condition {
                    Some(outer) => format!("({}) and ({})", outer, inner),
                    None => inner,
                };
                for nested_rule in &supports.rules.0 {
                    self.process_rule_with_supports(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        media_condition,
                        layer_name,
                    )?;
                }
            }
            CssRule::Media(media) => {
                let inner_condition = self.to_css_string(&media.query);
                let combined = match media_condition {
                    Some(outer) => format!("{} and {}", outer, inner_condition),
                    None => inner_condition,
                };
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        supports_condition,
                        layer_name,
                    )?;
                }
            }
            CssRule::LayerBlock(layer_block) => {
                let inner_layer = layer_block
                    .name
                    .as_ref()
                    .map(|n| self.to_css_string(n))
                    .unwrap_or_default();
                if !inner_layer.is_empty() {
                    stylesheet.add_layer(&inner_layer);
                }
                let effective_layer = if inner_layer.is_empty() {
                    layer_name
                } else {
                    Some(inner_layer.as_str())
                };
                for nested_rule in &layer_block.rules.0 {
                    self.process_rule_in_layer(
                        nested_rule,
                        stylesheet,
                        effective_layer.unwrap_or(""),
                        media_condition,
                        supports_condition,
                    )?;
                }
            }
            CssRule::Container(container) => {
                self.process_conditional_container(
                    container,
                    stylesheet,
                    media_condition,
                    supports_condition,
                    layer_name,
                )?;
            }
            _ => {
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    /// Process a CSS rule inside a `@layer` block.
    pub(crate) fn process_rule_in_layer(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        layer_name: &str,
        media_condition: Option<&str>,
        supports_condition: Option<&str>,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                let layer = if layer_name.is_empty() {
                    None
                } else {
                    Some(layer_name.to_string())
                };
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;
                    let properties = self.convert_declarations(&style_rule.declarations)?;
                    stylesheet.add_rule_with_conditions(
                        our_selector,
                        properties,
                        media_condition.map(|s| s.to_string()),
                        supports_condition.map(|s| s.to_string()),
                        layer.clone(),
                    );
                }
                if !style_rule.rules.0.is_empty() {
                    let parent_selectors: Vec<String> = style_rule
                        .selectors
                        .0
                        .iter()
                        .filter_map(|s| self.selector_to_string(s).ok())
                        .collect();
                    for nested_rule in &style_rule.rules.0 {
                        self.process_nested_rule(
                            nested_rule,
                            &parent_selectors,
                            stylesheet,
                            media_condition,
                            supports_condition,
                            layer.as_deref(),
                        )?;
                    }
                }
            }
            CssRule::Media(media) => {
                let inner_condition = self.to_css_string(&media.query);
                let combined = match media_condition {
                    Some(outer) => format!("{} and {}", outer, inner_condition),
                    None => inner_condition,
                };
                for nested_rule in &media.rules.0 {
                    self.process_rule_with_media(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        supports_condition,
                        Some(layer_name),
                    )?;
                }
            }
            CssRule::Supports(supports) => {
                let inner = self.to_css_string(&supports.condition);
                let combined = match supports_condition {
                    Some(outer) => format!("({}) and ({})", outer, inner),
                    None => inner,
                };
                for nested_rule in &supports.rules.0 {
                    self.process_rule_with_supports(
                        nested_rule,
                        stylesheet,
                        Some(&combined),
                        media_condition,
                        Some(layer_name),
                    )?;
                }
            }
            CssRule::Container(container) => {
                let condition = self.to_css_string(&container.condition);
                let name = container.name.as_ref().map(|n| self.to_css_string(n));
                let mut container_rules = Vec::new();
                for nested_rule in &container.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties =
                                self.convert_declarations(&style_rule.declarations)?;
                            let mut rule =
                                crate::stylesheet::StyleRule::new(our_selector, properties);
                            rule.media_condition = media_condition.map(|s| s.to_string());
                            rule.supports_condition = supports_condition.map(|s| s.to_string());
                            rule.layer = if layer_name.is_empty() {
                                None
                            } else {
                                Some(layer_name.to_string())
                            };
                            container_rules.push(rule);
                        }
                    }
                }
                stylesheet.add_container_rule(crate::stylesheet::ContainerRule {
                    name,
                    condition,
                    rules: container_rules,
                });
            }
            _ => {
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    // ── CSS Nesting ─────────────────────────────────────────────────

    /// Process a nested CSS rule (CSS Nesting Level 1).
    ///
    /// Resolves `&` references in nested selectors by substituting the parent
    /// selector string. If the nested selector doesn't start with `&`, it is
    /// implicitly treated as `& <nested-selector>` (descendant combinator).
    pub(crate) fn process_nested_rule(
        &self,
        rule: &CssRule,
        parent_selectors: &[String],
        stylesheet: &mut StyleSheet,
        media_condition: Option<&str>,
        supports_condition: Option<&str>,
        layer_name: Option<&str>,
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                for nested_sel in &style_rule.selectors.0 {
                    let nested_str = self.selector_to_string(nested_sel)?;
                    for parent_str in parent_selectors {
                        let resolved = Self::resolve_nesting_selector(&nested_str, parent_str);
                        let our_selector = Selector::parse(&resolved)?;
                        let properties = self.convert_declarations(&style_rule.declarations)?;
                        stylesheet.add_rule_with_conditions(
                            our_selector,
                            properties,
                            media_condition.map(|s| s.to_string()),
                            supports_condition.map(|s| s.to_string()),
                            layer_name.map(|s| s.to_string()),
                        );
                    }
                }
                if !style_rule.rules.0.is_empty() {
                    let mut new_parents = Vec::new();
                    for nested_sel in &style_rule.selectors.0 {
                        let nested_str = self.selector_to_string(nested_sel)?;
                        for parent_str in parent_selectors {
                            new_parents
                                .push(Self::resolve_nesting_selector(&nested_str, parent_str));
                        }
                    }
                    for sub_rule in &style_rule.rules.0 {
                        self.process_nested_rule(
                            sub_rule,
                            &new_parents,
                            stylesheet,
                            media_condition,
                            supports_condition,
                            layer_name,
                        )?;
                    }
                }
            }
            CssRule::Nesting(nesting) => {
                for nested_sel in &nesting.style.selectors.0 {
                    let nested_str = self.selector_to_string(nested_sel)?;
                    for parent_str in parent_selectors {
                        let resolved = Self::resolve_nesting_selector(&nested_str, parent_str);
                        let our_selector = Selector::parse(&resolved)?;
                        let properties =
                            self.convert_declarations(&nesting.style.declarations)?;
                        stylesheet.add_rule_with_conditions(
                            our_selector,
                            properties,
                            media_condition.map(|s| s.to_string()),
                            supports_condition.map(|s| s.to_string()),
                            layer_name.map(|s| s.to_string()),
                        );
                    }
                }
                if !nesting.style.rules.0.is_empty() {
                    let mut new_parents = Vec::new();
                    for nested_sel in &nesting.style.selectors.0 {
                        let nested_str = self.selector_to_string(nested_sel)?;
                        for parent_str in parent_selectors {
                            new_parents
                                .push(Self::resolve_nesting_selector(&nested_str, parent_str));
                        }
                    }
                    for sub_rule in &nesting.style.rules.0 {
                        self.process_nested_rule(
                            sub_rule,
                            &new_parents,
                            stylesheet,
                            media_condition,
                            supports_condition,
                            layer_name,
                        )?;
                    }
                }
            }
            CssRule::Media(media) => {
                let condition = self.to_css_string(&media.query);
                let combined_media = match media_condition {
                    Some(outer) => format!("{} and {}", outer, condition),
                    None => condition,
                };
                for nested_rule in &media.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for nested_sel in &style_rule.selectors.0 {
                            let nested_str = self.selector_to_string(nested_sel)?;
                            for parent_str in parent_selectors {
                                let resolved =
                                    Self::resolve_nesting_selector(&nested_str, parent_str);
                                let our_selector = Selector::parse(&resolved)?;
                                let properties =
                                    self.convert_declarations(&style_rule.declarations)?;
                                stylesheet.add_rule_with_conditions(
                                    our_selector,
                                    properties,
                                    Some(combined_media.clone()),
                                    supports_condition.map(|s| s.to_string()),
                                    layer_name.map(|s| s.to_string()),
                                );
                            }
                        }
                    }
                }
            }
            _ => {
                self.process_rule(rule, stylesheet)?;
            }
        }
        Ok(())
    }

    /// Resolve `&` in a nested selector by substituting the parent selector.
    ///
    /// If the nested selector contains `&`, replace it with the parent.
    /// Otherwise, prepend the parent with a descendant combinator.
    fn resolve_nesting_selector(nested: &str, parent: &str) -> String {
        if nested.contains('&') {
            nested.replace('&', parent)
        } else {
            // Implicit `& <nested>` — descendant combinator
            format!("{} {}", parent, nested)
        }
    }

    // ── Shared conditional helpers ──────────────────────────────────

    /// Process a style rule with conditional context (shared by media/supports/layer).
    fn process_conditional_style_rule(
        &self,
        style_rule: &lightningcss::rules::style::StyleRule<'_>,
        stylesheet: &mut StyleSheet,
        media_condition: Option<&str>,
        supports_condition: Option<&str>,
        layer_name: Option<&str>,
    ) -> Result<()> {
        for selector in &style_rule.selectors.0 {
            let selector_str = self.selector_to_string(selector)?;
            let our_selector = Selector::parse(&selector_str)?;
            let properties = self.convert_declarations(&style_rule.declarations)?;
            stylesheet.add_rule_with_conditions(
                our_selector,
                properties,
                media_condition.map(|s| s.to_string()),
                supports_condition.map(|s| s.to_string()),
                layer_name.map(|s| s.to_string()),
            );
        }
        if !style_rule.rules.0.is_empty() {
            let parent_selectors: Vec<String> = style_rule
                .selectors
                .0
                .iter()
                .filter_map(|s| self.selector_to_string(s).ok())
                .collect();
            for nested_rule in &style_rule.rules.0 {
                self.process_nested_rule(
                    nested_rule,
                    &parent_selectors,
                    stylesheet,
                    media_condition,
                    supports_condition,
                    layer_name,
                )?;
            }
        }
        Ok(())
    }

    /// Process a `@container` rule with conditional context.
    fn process_conditional_container(
        &self,
        container: &lightningcss::rules::container::ContainerRule<'_>,
        stylesheet: &mut StyleSheet,
        media_condition: Option<&str>,
        supports_condition: Option<&str>,
        layer_name: Option<&str>,
    ) -> Result<()> {
        let condition = self.to_css_string(&container.condition);
        let name = container.name.as_ref().map(|n| self.to_css_string(n));
        let mut container_rules = Vec::new();
        for nested_rule in &container.rules.0 {
            if let CssRule::Style(style_rule) = nested_rule {
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;
                    let properties = self.convert_declarations(&style_rule.declarations)?;
                    let mut rule =
                        crate::stylesheet::StyleRule::new(our_selector, properties);
                    rule.media_condition = media_condition.map(|s| s.to_string());
                    rule.supports_condition = supports_condition.map(|s| s.to_string());
                    rule.layer = layer_name.map(|s| s.to_string());
                    container_rules.push(rule);
                }
            }
        }
        stylesheet.add_container_rule(crate::stylesheet::ContainerRule {
            name,
            condition,
            rules: container_rules,
        });
        Ok(())
    }
}
