//! CSS rule processing — dispatches at-rules and style rules, handles nesting.
//!
//! Processes the top-level rule tree from lightningcss, converting each rule type
//! (@media, @supports, @keyframes, @font-face, @layer, @container, @scope, etc.)
//! into our `StyleSheet` representation. Supports CSS Nesting Level 1 with `&`
//! parent-selector resolution.

use crate::error::Result;
use crate::selector::Selector;
use crate::stylesheet::{
    ImportLayer, ImportRule, StructuralCondition, StructuralContainerConstraint, StyleSheet,
    STRUCTURAL_CONDITION_SENTINEL,
};
use crate::value::{FontFaceRule, FontSource, Keyframe, KeyframesRule};

use lightningcss::rules::CssRule;

use super::ThemeParser;

#[derive(Clone, Default)]
struct RuleContext {
    media_condition: Option<String>,
    supports_condition: Option<String>,
    layer_name: Option<String>,
    containers: Vec<StructuralContainerConstraint>,
    scope_start: Option<String>,
    scope_end: Option<String>,
}

impl ThemeParser {
    /// Process a CSS rule recursively.
    pub(crate) fn process_rule(&self, rule: &CssRule, stylesheet: &mut StyleSheet) -> Result<()> {
        match rule {
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
                stylesheet.add_import_rule(ImportRule {
                    url: import.url.to_string(),
                    layer: import.layer.as_ref().map(|layer| match layer {
                        Some(name) => ImportLayer::Named(self.to_css_string(name)),
                        None => ImportLayer::Anonymous,
                    }),
                    supports_condition: import.supports.as_ref().map(|cond| self.to_css_string(cond)),
                    media_condition: if import.media.media_queries.is_empty() {
                        None
                    } else {
                        Some(self.to_css_string(&import.media))
                    },
                });
            }
            CssRule::LayerStatement(layer_stmt) => {
                // @layer declaration (ordering): @layer reset, base, components;
                for name in &layer_stmt.names {
                    let layer_name = self.to_css_string(name);
                    stylesheet.add_layer(&layer_name);
                }
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
            CssRule::StartingStyle(starting) => {
                // @starting-style { ... } — initial styles for transition origins
                for nested_rule in &starting.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        for selector in &style_rule.selectors.0 {
                            let selector_str = self.selector_to_string(selector)?;
                            let our_selector = Selector::parse(&selector_str)?;
                            let properties = self.convert_declarations(&style_rule.declarations)?;
                            stylesheet.add_starting_style_rule(crate::stylesheet::StyleRule::new(
                                our_selector,
                                properties,
                            ));
                        }
                    }
                }
            }
            // Ignored, deprecated, or niche at-rules that don't affect layout:
            // @viewport (deprecated), @-moz-document (non-standard), @custom-media (draft),
            // @font-palette-values, @font-feature-values, @view-transition, unknown rules.
            _ => {
                self.process_rule_in_context(rule, stylesheet, &RuleContext::default(), &[])?;
            }
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

    // ── Context-aware rule walking ─────────────────────────────────

    fn process_rule_in_context(
        &self,
        rule: &CssRule,
        stylesheet: &mut StyleSheet,
        context: &RuleContext,
        parent_selectors: &[String],
    ) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                self.emit_style_rule(style_rule, stylesheet, context, parent_selectors)
            }
            CssRule::Nesting(nesting) => {
                self.emit_style_rule(&nesting.style, stylesheet, context, parent_selectors)
            }
            CssRule::Media(media) => {
                let mut next = context.clone();
                next.media_condition = Some(Self::combine_condition(
                    context.media_condition.as_deref(),
                    &self.to_css_string(&media.query),
                ));
                for nested_rule in &media.rules.0 {
                    self.process_rule_in_context(nested_rule, stylesheet, &next, parent_selectors)?;
                }
                Ok(())
            }
            CssRule::Supports(supports) => {
                let mut next = context.clone();
                next.supports_condition = Some(Self::combine_condition(
                    context.supports_condition.as_deref(),
                    &self.to_css_string(&supports.condition),
                ));
                for nested_rule in &supports.rules.0 {
                    self.process_rule_in_context(nested_rule, stylesheet, &next, parent_selectors)?;
                }
                Ok(())
            }
            CssRule::LayerBlock(layer_block) => {
                let resolved_layer = self.resolve_layer_name(
                    stylesheet,
                    context.layer_name.as_deref(),
                    layer_block.name.as_ref().map(|name| self.to_css_string(name)),
                );
                let mut next = context.clone();
                next.layer_name = Some(resolved_layer);
                for nested_rule in &layer_block.rules.0 {
                    self.process_rule_in_context(nested_rule, stylesheet, &next, parent_selectors)?;
                }
                Ok(())
            }
            CssRule::LayerStatement(layer_stmt) => {
                for name in &layer_stmt.names {
                    let qualified = Self::qualify_layer_name(
                        context.layer_name.as_deref(),
                        &self.to_css_string(name),
                    );
                    stylesheet.add_layer(&qualified);
                }
                Ok(())
            }
            CssRule::Container(container) => {
                let mut next = context.clone();
                next.containers.push(StructuralContainerConstraint {
                    name: container.name.as_ref().map(|name| self.to_css_string(name)),
                    condition: self.to_css_string(&container.condition),
                });
                for nested_rule in &container.rules.0 {
                    self.process_rule_in_context(nested_rule, stylesheet, &next, parent_selectors)?;
                }
                Ok(())
            }
            CssRule::Scope(scope) => {
                let mut next = context.clone();
                next.scope_start = scope.scope_start.as_ref().map(|selector| self.to_css_string(selector));
                next.scope_end = scope.scope_end.as_ref().map(|selector| self.to_css_string(selector));
                for nested_rule in &scope.rules.0 {
                    self.process_rule_in_context(nested_rule, stylesheet, &next, parent_selectors)?;
                }
                Ok(())
            }
            CssRule::StartingStyle(starting) => {
                for nested_rule in &starting.rules.0 {
                    if let CssRule::Style(style_rule) = nested_rule {
                        self.emit_starting_style_rule(
                            style_rule,
                            stylesheet,
                            context,
                            parent_selectors,
                        )?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn emit_style_rule(
        &self,
        style_rule: &lightningcss::rules::style::StyleRule<'_>,
        stylesheet: &mut StyleSheet,
        context: &RuleContext,
        parent_selectors: &[String],
    ) -> Result<()> {
        let resolved_selectors = self.resolve_style_selectors(style_rule, parent_selectors)?;
        let properties = self.convert_declarations(&style_rule.declarations)?;

        for selector_str in &resolved_selectors {
            let our_selector = Selector::parse(selector_str)?;
            let mut rule = crate::stylesheet::StyleRule::new(our_selector, properties.clone());
            rule.media_condition = context.media_condition.clone();
            rule.supports_condition = context.supports_condition.clone();
            rule.layer = context.layer_name.clone();
            self.push_contextual_style_rule(stylesheet, rule, context);
        }

        if !style_rule.rules.0.is_empty() {
            for nested_rule in &style_rule.rules.0 {
                self.process_rule_in_context(
                    nested_rule,
                    stylesheet,
                    context,
                    &resolved_selectors,
                )?;
            }
        }

        Ok(())
    }

    fn emit_starting_style_rule(
        &self,
        style_rule: &lightningcss::rules::style::StyleRule<'_>,
        stylesheet: &mut StyleSheet,
        context: &RuleContext,
        parent_selectors: &[String],
    ) -> Result<()> {
        let resolved_selectors = self.resolve_style_selectors(style_rule, parent_selectors)?;
        let properties = self.convert_declarations(&style_rule.declarations)?;

        for selector_str in &resolved_selectors {
            let our_selector = Selector::parse(selector_str)?;
            let mut rule = crate::stylesheet::StyleRule::new(our_selector, properties.clone());
            rule.media_condition = context.media_condition.clone();
            rule.supports_condition = context.supports_condition.clone();
            rule.layer = context.layer_name.clone();
            stylesheet.add_starting_style_rule(rule);
        }

        Ok(())
    }

    fn resolve_style_selectors(
        &self,
        style_rule: &lightningcss::rules::style::StyleRule<'_>,
        parent_selectors: &[String],
    ) -> Result<Vec<String>> {
        let mut resolved = Vec::new();

        for selector in &style_rule.selectors.0 {
            let selector_str = self.selector_to_string(selector)?;
            if parent_selectors.is_empty() {
                resolved.push(selector_str);
            } else {
                for parent_selector in parent_selectors {
                    resolved.push(Self::resolve_nesting_selector(&selector_str, parent_selector));
                }
            }
        }

        Ok(resolved)
    }

    fn push_contextual_style_rule(
        &self,
        stylesheet: &mut StyleSheet,
        rule: crate::stylesheet::StyleRule,
        context: &RuleContext,
    ) {
        if !context.containers.is_empty() {
            let (name, condition) = if context.containers.len() == 1
                && context.scope_start.is_none()
                && context.scope_end.is_none()
            {
                (
                    context.containers[0].name.clone(),
                    context.containers[0].condition.clone(),
                )
            } else {
                (
                    Some(STRUCTURAL_CONDITION_SENTINEL.to_string()),
                    StyleSheet::encode_structural_condition(&StructuralCondition {
                        containers: context.containers.clone(),
                        scope_start: context.scope_start.clone(),
                        scope_end: context.scope_end.clone(),
                    }),
                )
            };

            stylesheet.add_container_rule(crate::stylesheet::ContainerRule {
                name,
                condition,
                rules: vec![rule],
            });
            return;
        }

        if context.scope_start.is_some() || context.scope_end.is_some() {
            stylesheet.add_scope_rule(crate::stylesheet::ScopeRule {
                scope_start: context.scope_start.clone(),
                scope_end: context.scope_end.clone(),
                rules: vec![rule],
            });
            return;
        }

        stylesheet.add_rule_with_conditions(
            rule.selector,
            rule.properties,
            rule.media_condition,
            rule.supports_condition,
            rule.layer,
        );
    }

    fn resolve_layer_name(
        &self,
        stylesheet: &mut StyleSheet,
        parent_layer: Option<&str>,
        raw_layer_name: Option<String>,
    ) -> String {
        let layer_name = raw_layer_name.unwrap_or_else(StyleSheet::fresh_anonymous_layer_name);
        let qualified = Self::qualify_layer_name(parent_layer, &layer_name);
        stylesheet.add_layer(&qualified);
        qualified
    }

    fn qualify_layer_name(parent_layer: Option<&str>, layer_name: &str) -> String {
        match parent_layer.filter(|name| !name.is_empty()) {
            Some(parent_layer) => format!("{parent_layer}.{layer_name}"),
            None => layer_name.to_string(),
        }
    }

    fn combine_condition(outer: Option<&str>, inner: &str) -> String {
        match outer.filter(|value| !value.trim().is_empty()) {
            Some(outer) => format!("({outer}) and ({inner})"),
            None => inner.to_string(),
        }
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
}
