//! CSS stylesheet representation

use crate::property::PropertySet;
use crate::selector::Selector;
use crate::value::{FontFaceRule, KeyframesRule, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A parsed CSS stylesheet
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleSheet {
    /// Rules indexed by selector
    rules: Vec<StyleRule>,

    /// CSS variables (custom properties)
    variables: HashMap<String, PropertyValue>,

    /// `@keyframes` rules by name.
    keyframes: HashMap<String, KeyframesRule>,

    /// `@font-face` rules.
    font_faces: Vec<FontFaceRule>,

    /// `@import` URLs (resolved externally).
    imports: Vec<String>,

    /// `@layer` ordering — layer names in cascade order.
    layer_order: Vec<String>,

    /// `@container` query rules.
    container_rules: Vec<ContainerRule>,

    /// `@property` custom property registrations.
    registered_properties: Vec<RegisteredProperty>,

    /// `@namespace` declarations.
    namespaces: Vec<NamespaceRule>,

    /// `@page` rules (print styling).
    page_rules: Vec<PageRule>,

    /// `@counter-style` custom counter definitions.
    counter_styles: Vec<CounterStyleRule>,

    /// `@scope` rules (CSS Cascading and Inheritance Level 6).
    scope_rules: Vec<ScopeRule>,

    /// `@starting-style` rules (CSS Transitions Level 2).
    starting_style_rules: Vec<StyleRule>,
}

/// A `@container` query rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRule {
    /// Optional container name (None = any nearest container).
    pub name: Option<String>,
    /// The container query condition string (e.g., "(min-width: 600px)").
    pub condition: String,
    /// Nested style rules.
    pub rules: Vec<StyleRule>,
}

/// A `@property` custom property registration (CSS Houdini).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredProperty {
    /// Property name (e.g., "--my-color").
    pub name: String,
    /// Syntax descriptor (e.g., "<color>", "<length>", "*").
    pub syntax: String,
    /// Whether the property inherits.
    pub inherits: bool,
    /// Initial value string.
    pub initial_value: Option<String>,
}

/// A `@namespace` declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceRule {
    /// Optional namespace prefix (e.g., `svg` in `@namespace svg url(…)`).
    pub prefix: Option<String>,
    /// The namespace URL.
    pub url: String,
}

/// A `@page` rule for print styling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRule {
    /// Page selectors (e.g., `:first`, `:left`, `:right`, or named pages).
    pub selectors: Vec<String>,
    /// Declarations inside the `@page` block.
    pub properties: PropertySet,
}

/// A `@counter-style` custom counter definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterStyleRule {
    /// Counter style name.
    pub name: String,
    /// The `system` descriptor value.
    pub system: Option<String>,
    /// The `symbols` descriptor value.
    pub symbols: Option<String>,
    /// The `suffix` descriptor value.
    pub suffix: Option<String>,
    /// The `prefix` descriptor value.
    pub prefix: Option<String>,
    /// The `negative` descriptor value.
    pub negative: Option<String>,
    /// The `range` descriptor value.
    pub range: Option<String>,
    /// The `pad` descriptor value.
    pub pad: Option<String>,
    /// The `fallback` descriptor value.
    pub fallback: Option<String>,
    /// The `speak-as` descriptor value.
    pub speak_as: Option<String>,
    /// The `additive-symbols` descriptor value.
    pub additive_symbols: Option<String>,
}

/// A `@scope` rule (CSS Cascading and Inheritance Level 6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeRule {
    /// Scope root selector (e.g., `.card` in `@scope (.card)`).
    pub scope_start: Option<String>,
    /// Scope limit selector (e.g., `.content` in `@scope (.card) to (.content)`).
    pub scope_end: Option<String>,
    /// Nested style rules.
    pub rules: Vec<StyleRule>,
}

/// Environment used when evaluating conditional CSS rules.
#[derive(Debug, Clone)]
pub struct QueryEnvironment {
    /// Viewport width in CSS pixels.
    pub viewport_width: f32,
    /// Viewport height in CSS pixels.
    pub viewport_height: f32,
    /// Preferred color scheme (`"light"` / `"dark"`).
    pub preferred_color_scheme: String,
    /// Whether reduced motion is preferred.
    pub prefers_reduced_motion: bool,
    /// Explicitly supported property names for `@supports` checks.
    /// If empty, the stylesheet's built-in support table is used.
    pub supported_properties: HashSet<String>,
}

impl Default for QueryEnvironment {
    fn default() -> Self {
        Self {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            preferred_color_scheme: "light".to_string(),
            prefers_reduced_motion: false,
            supported_properties: HashSet::new(),
        }
    }
}

/// A single style rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRule {
    /// Selector for this rule
    pub selector: Selector,

    /// Properties in this rule
    pub properties: PropertySet,

    /// Rule specificity (cached)
    specificity: (u32, u32, u32),

    /// Optional media condition string (e.g. "(prefers-color-scheme: dark)").
    /// When `Some`, the rule only applies if the condition matches the viewport.
    pub media_condition: Option<String>,

    /// Optional `@supports` condition string.
    /// When `Some`, the rule only applies if the support expression evaluates to true.
    pub supports_condition: Option<String>,

    /// Optional cascade layer name.
    pub layer: Option<String>,
}

impl StyleRule {
    /// Create a new style rule
    pub fn new(selector: Selector, properties: PropertySet) -> Self {
        let specificity = selector.specificity();
        Self {
            selector,
            properties,
            specificity,
            media_condition: None,
            supports_condition: None,
            layer: None,
        }
    }

    /// Create a style rule gated on a media condition.
    pub fn with_media_condition(mut self, condition: String) -> Self {
        self.media_condition = Some(condition);
        self
    }

    /// Create a style rule gated on a `@supports` condition.
    pub fn with_supports_condition(mut self, condition: String) -> Self {
        self.supports_condition = Some(condition);
        self
    }

    /// Get specificity
    pub fn specificity(&self) -> (u32, u32, u32) {
        self.specificity
    }
}

impl StyleSheet {
    /// Create a new empty stylesheet
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule
    pub fn add_rule(&mut self, selector: Selector, properties: PropertySet) {
        self.rules.push(StyleRule::new(selector, properties));
    }

    /// Add a rule with optional media/supports/layer conditions.
    pub fn add_rule_with_conditions(
        &mut self,
        selector: Selector,
        properties: PropertySet,
        media_condition: Option<String>,
        supports_condition: Option<String>,
        layer: Option<String>,
    ) {
        let mut rule = StyleRule::new(selector, properties);
        rule.media_condition = media_condition;
        rule.supports_condition = supports_condition;
        rule.layer = layer;
        self.rules.push(rule);
    }

    /// Add a rule that is gated on a media condition string.
    pub fn add_conditional_rule(
        &mut self,
        selector: Selector,
        properties: PropertySet,
        media_condition: String,
    ) {
        self.add_rule_with_conditions(
            selector,
            properties,
            Some(media_condition),
            None,
            None,
        );
    }

    /// Add a rule that is gated on a `@supports` condition string.
    pub fn add_supports_rule(
        &mut self,
        selector: Selector,
        properties: PropertySet,
        supports_condition: String,
    ) {
        self.add_rule_with_conditions(
            selector,
            properties,
            None,
            Some(supports_condition),
            None,
        );
    }

    /// Set a CSS variable
    pub fn set_variable(&mut self, name: String, value: PropertyValue) {
        self.variables.insert(name, value);
    }

    /// Get a CSS variable
    pub fn get_variable(&self, name: &str) -> Option<&PropertyValue> {
        self.variables.get(name)
    }

    /// Find matching rules for an element
    pub fn find_matching_rules(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> Vec<&StyleRule> {
        self.find_matching_rules_with_environment(
            element,
            classes,
            id,
            pseudo_classes,
            &QueryEnvironment::default(),
        )
    }

    /// Find matching rules for an element in a specific query environment.
    pub fn find_matching_rules_with_environment(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
        env: &QueryEnvironment,
    ) -> Vec<&StyleRule> {
        let mut matching = Vec::new();

        for rule in &self.rules {
            if let Some(ref condition) = rule.media_condition {
                if !self.evaluate_media_condition(condition, env) {
                    continue;
                }
            }
            if let Some(ref condition) = rule.supports_condition {
                if !self.evaluate_supports_condition(condition, env) {
                    continue;
                }
            }
            if rule.selector.matches(element, classes, id, pseudo_classes) {
                matching.push(rule);
            }
        }

        // Keep deterministic fallback ordering when callers use this API directly.
        matching.sort_by(|a, b| b.specificity.cmp(&a.specificity));

        matching
    }

    /// Compute final styles for an element (cascade resolution)
    pub fn compute_styles(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> PropertySet {
        self.compute_styles_with_environment(
            element,
            classes,
            id,
            pseudo_classes,
            &QueryEnvironment::default(),
        )
    }

    /// Compute final styles for an element (cascade resolution) using a query environment.
    pub fn compute_styles_with_environment(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
        env: &QueryEnvironment,
    ) -> PropertySet {
        let mut final_properties = PropertySet::new();

        let mut matching: Vec<(usize, &StyleRule)> = Vec::new();
        for (source_order, rule) in self.rules.iter().enumerate() {
            if let Some(ref condition) = rule.media_condition {
                if !self.evaluate_media_condition(condition, env) {
                    continue;
                }
            }
            if let Some(ref condition) = rule.supports_condition {
                if !self.evaluate_supports_condition(condition, env) {
                    continue;
                }
            }
            if rule.selector.matches(element, classes, id, pseudo_classes) {
                matching.push((source_order, rule));
            }
        }

        // Cascade order: lower layer first, then lower specificity, then source order.
        // Unlayered author rules are treated as highest-precedence layer.
        matching.sort_by(|(a_idx, a_rule), (b_idx, b_rule)| {
            let a_layer = self.layer_rank(a_rule);
            let b_layer = self.layer_rank(b_rule);
            a_layer
                .cmp(&b_layer)
                .then(a_rule.specificity().cmp(&b_rule.specificity()))
                .then(a_idx.cmp(b_idx))
        });

        for (_, rule) in matching {
            final_properties.merge(&rule.properties);
        }

        final_properties
    }

    /// Get all rules
    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }

    /// Get number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Merge another stylesheet
    pub fn merge(&mut self, other: &StyleSheet) {
        self.rules.extend(other.rules.clone());
        self.variables.extend(other.variables.clone());
        self.keyframes.extend(other.keyframes.clone());
        self.font_faces.extend(other.font_faces.clone());
        self.imports.extend(other.imports.clone());
        self.container_rules.extend(other.container_rules.clone());
        self.registered_properties
            .extend(other.registered_properties.clone());
        self.namespaces.extend(other.namespaces.clone());
        self.page_rules.extend(other.page_rules.clone());
        self.counter_styles.extend(other.counter_styles.clone());
        self.scope_rules.extend(other.scope_rules.clone());
        self.starting_style_rules
            .extend(other.starting_style_rules.clone());
    }

    // ── @keyframes ─────────────────────────────────────────────────────
    /// Add a `@keyframes` rule.
    pub fn add_keyframes(&mut self, rule: KeyframesRule) {
        self.keyframes.insert(rule.name.clone(), rule);
    }

    /// Get a `@keyframes` rule by name.
    pub fn get_keyframes(&self, name: &str) -> Option<&KeyframesRule> {
        self.keyframes.get(name)
    }

    /// All `@keyframes` rules.
    pub fn keyframes(&self) -> &HashMap<String, KeyframesRule> {
        &self.keyframes
    }

    // ── @font-face ─────────────────────────────────────────────────────
    /// Add a `@font-face` rule.
    pub fn add_font_face(&mut self, rule: FontFaceRule) {
        self.font_faces.push(rule);
    }

    /// All `@font-face` rules.
    pub fn font_faces(&self) -> &[FontFaceRule] {
        &self.font_faces
    }

    // ── @import ────────────────────────────────────────────────────────
    /// Add an `@import` URL.
    pub fn add_import(&mut self, url: String) {
        self.imports.push(url);
    }

    /// All `@import` URLs.
    pub fn imports(&self) -> &[String] {
        &self.imports
    }

    // ── @layer ─────────────────────────────────────────────────────────
    /// Declare a cascade layer. Layers are ordered by first declaration.
    pub fn add_layer(&mut self, name: &str) {
        if !self.layer_order.contains(&name.to_string()) {
            self.layer_order.push(name.to_string());
        }
    }

    /// Add a style rule to a named layer.
    pub fn add_layer_rule(
        &mut self,
        layer_name: &str,
        selector: Selector,
        properties: PropertySet,
    ) {
        self.add_layer(layer_name);
        let mut rule = StyleRule::new(selector, properties);
        rule.layer = Some(layer_name.to_string());
        self.rules.push(rule);
    }

    /// Get cascade layer ordering.
    pub fn layer_order(&self) -> &[String] {
        &self.layer_order
    }

    // ── @container ─────────────────────────────────────────────────────
    /// Add a `@container` query rule.
    pub fn add_container_rule(&mut self, rule: ContainerRule) {
        self.container_rules.push(rule);
    }

    /// All `@container` rules.
    pub fn container_rules(&self) -> &[ContainerRule] {
        &self.container_rules
    }

    // ── @property ──────────────────────────────────────────────────────
    /// Register a custom property.
    pub fn add_registered_property(&mut self, prop: RegisteredProperty) {
        self.registered_properties.push(prop);
    }

    /// All registered custom properties.
    pub fn registered_properties(&self) -> &[RegisteredProperty] {
        &self.registered_properties
    }

    // ── @namespace ─────────────────────────────────────────────────────
    /// Add a `@namespace` declaration.
    pub fn add_namespace(&mut self, rule: NamespaceRule) {
        self.namespaces.push(rule);
    }

    /// All `@namespace` declarations.
    pub fn namespaces(&self) -> &[NamespaceRule] {
        &self.namespaces
    }

    // ── @page ──────────────────────────────────────────────────────────
    /// Add a `@page` rule.
    pub fn add_page_rule(&mut self, rule: PageRule) {
        self.page_rules.push(rule);
    }

    /// All `@page` rules.
    pub fn page_rules(&self) -> &[PageRule] {
        &self.page_rules
    }

    // ── @counter-style ─────────────────────────────────────────────────
    /// Add a `@counter-style` rule.
    pub fn add_counter_style(&mut self, rule: CounterStyleRule) {
        self.counter_styles.push(rule);
    }

    /// All `@counter-style` rules.
    pub fn counter_styles(&self) -> &[CounterStyleRule] {
        &self.counter_styles
    }

    // ── @scope ─────────────────────────────────────────────────────────
    /// Add a `@scope` rule.
    pub fn add_scope_rule(&mut self, rule: ScopeRule) {
        self.scope_rules.push(rule);
    }

    /// All `@scope` rules.
    pub fn scope_rules(&self) -> &[ScopeRule] {
        &self.scope_rules
    }

    // ── @starting-style ────────────────────────────────────────────────
    /// Add a `@starting-style` rule.
    pub fn add_starting_style_rule(&mut self, rule: StyleRule) {
        self.starting_style_rules.push(rule);
    }

    /// All `@starting-style` rules.
    pub fn starting_style_rules(&self) -> &[StyleRule] {
        &self.starting_style_rules
    }

    fn layer_rank(&self, rule: &StyleRule) -> u32 {
        match rule
            .layer
            .as_ref()
            .and_then(|name| self.layer_order.iter().position(|n| n == name))
        {
            Some(idx) => idx as u32 + 1,
            None => u32::MAX,
        }
    }

    fn evaluate_media_condition(&self, condition: &str, env: &QueryEnvironment) -> bool {
        let condition = condition.trim();
        if condition.is_empty() {
            return true;
        }

        if let Some(rest) = condition.strip_prefix("not ") {
            return !self.evaluate_media_condition(rest.trim(), env);
        }
        if condition.contains(" and ") {
            return condition
                .split(" and ")
                .all(|part| self.evaluate_media_condition(part.trim(), env));
        }
        if condition.contains(" or ") {
            return condition
                .split(" or ")
                .any(|part| self.evaluate_media_condition(part.trim(), env));
        }

        let inner = condition
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(condition)
            .trim();

        if let Some(result) = self.evaluate_media_comparison(inner, env) {
            return result;
        }

        if let Some((feature, value)) = inner.split_once(':') {
            let feature = feature.trim();
            let value = value.trim();
            match feature {
                "min-width" => Self::parse_px_value(value)
                    .map(|v| env.viewport_width >= v)
                    .unwrap_or(true),
                "max-width" => Self::parse_px_value(value)
                    .map(|v| env.viewport_width <= v)
                    .unwrap_or(true),
                "min-height" => Self::parse_px_value(value)
                    .map(|v| env.viewport_height >= v)
                    .unwrap_or(true),
                "max-height" => Self::parse_px_value(value)
                    .map(|v| env.viewport_height <= v)
                    .unwrap_or(true),
                "width" => Self::parse_px_value(value)
                    .map(|v| (env.viewport_width - v).abs() < 1.0)
                    .unwrap_or(true),
                "height" => Self::parse_px_value(value)
                    .map(|v| (env.viewport_height - v).abs() < 1.0)
                    .unwrap_or(true),
                "prefers-color-scheme" => env.preferred_color_scheme.eq_ignore_ascii_case(value),
                "prefers-reduced-motion" => {
                    let v = value.eq_ignore_ascii_case("reduce");
                    env.prefers_reduced_motion == v
                }
                "orientation" => {
                    let actual = if env.viewport_width >= env.viewport_height {
                        "landscape"
                    } else {
                        "portrait"
                    };
                    actual.eq_ignore_ascii_case(value)
                }
                _ => true,
            }
        } else {
            true
        }
    }

    fn evaluate_media_comparison(
        &self,
        inner: &str,
        env: &QueryEnvironment,
    ) -> Option<bool> {
        for op in ["<=", ">=", "<", ">"] {
            if let Some((lhs_raw, rhs_raw)) = inner.split_once(op) {
                let lhs = lhs_raw.trim();
                let rhs = rhs_raw.trim();
                let lhs_val = Self::media_dimension_value(lhs, env)?;
                let rhs_val = Self::media_dimension_value(rhs, env)?;
                let result = match op {
                    "<=" => lhs_val <= rhs_val,
                    ">=" => lhs_val >= rhs_val,
                    "<" => lhs_val < rhs_val,
                    ">" => lhs_val > rhs_val,
                    _ => return None,
                };
                return Some(result);
            }
        }
        None
    }

    fn media_dimension_value(token: &str, env: &QueryEnvironment) -> Option<f32> {
        match token {
            "width" => Some(env.viewport_width),
            "height" => Some(env.viewport_height),
            _ => Self::parse_px_value(token),
        }
    }

    fn evaluate_supports_condition(&self, condition: &str, env: &QueryEnvironment) -> bool {
        let condition = condition.trim();
        if condition.is_empty() {
            return true;
        }

        if let Some(rest) = condition.strip_prefix("not ") {
            return !self.evaluate_supports_condition(rest.trim(), env);
        }
        if condition.contains(" and ") {
            return condition
                .split(" and ")
                .all(|part| self.evaluate_supports_condition(part.trim(), env));
        }
        if condition.contains(" or ") {
            return condition
                .split(" or ")
                .any(|part| self.evaluate_supports_condition(part.trim(), env));
        }

        let inner = condition
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(condition)
            .trim();

        if let Some((property, value)) = inner.split_once(':') {
            let property = property.trim();
            let value = value.trim();
            let property_supported = if env.supported_properties.is_empty() {
                Self::is_supported_css_property(property)
            } else {
                env.supported_properties.contains(property)
            };
            property_supported && Self::is_supported_css_value(property, value)
        } else {
            true
        }
    }

    fn parse_px_value(value: &str) -> Option<f32> {
        let value = value.trim();
        if let Some(px) = value.strip_suffix("px") {
            px.trim().parse::<f32>().ok()
        } else if let Some(rem) = value.strip_suffix("rem") {
            rem.trim().parse::<f32>().ok().map(|v| v * 16.0)
        } else if let Some(em) = value.strip_suffix("em") {
            em.trim().parse::<f32>().ok().map(|v| v * 16.0)
        } else {
            value.parse::<f32>().ok()
        }
    }

    fn is_supported_css_property(property: &str) -> bool {
        matches!(
            property,
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

    fn is_supported_css_value(property: &str, value: &str) -> bool {
        let value = value.trim();
        match property {
            "display" => matches!(
                value,
                "block"
                    | "inline"
                    | "inline-block"
                    | "flex"
                    | "inline-flex"
                    | "grid"
                    | "inline-grid"
                    | "none"
                    | "contents"
                    | "table"
                    | "table-row"
                    | "table-cell"
                    | "list-item"
                    | "flow-root"
            ),
            "position" => matches!(value, "static" | "relative" | "absolute" | "fixed" | "sticky"),
            "visibility" => matches!(value, "visible" | "hidden" | "collapse"),
            "overflow" | "overflow-x" | "overflow-y" => {
                matches!(value, "visible" | "hidden" | "scroll" | "auto" | "clip")
            }
            "box-sizing" => matches!(value, "border-box" | "content-box"),
            "text-align" => matches!(
                value,
                "left" | "right" | "center" | "justify" | "start" | "end"
            ),
            "text-transform" => {
                matches!(value, "none" | "uppercase" | "lowercase" | "capitalize")
            }
            "white-space" => matches!(
                value,
                "normal" | "nowrap" | "pre" | "pre-wrap" | "pre-line" | "break-spaces"
            ),
            "cursor" => matches!(
                value,
                "auto"
                    | "default"
                    | "pointer"
                    | "crosshair"
                    | "text"
                    | "move"
                    | "grab"
                    | "grabbing"
                    | "not-allowed"
                    | "wait"
                    | "progress"
                    | "help"
                    | "none"
            ),
            "pointer-events" => matches!(value, "auto" | "none"),
            "border-style" => matches!(
                value,
                "none"
                    | "solid"
                    | "dashed"
                    | "dotted"
                    | "double"
                    | "groove"
                    | "ridge"
                    | "inset"
                    | "outset"
                    | "hidden"
            ),
            "width" | "height" | "min-width" | "max-width" | "min-height" | "max-height"
            | "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left"
            | "padding" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left"
            | "top" | "right" | "bottom" | "left" | "gap" | "row-gap" | "column-gap"
            | "border-radius" | "border-width" | "font-size" | "line-height"
            | "letter-spacing" | "word-spacing" | "text-indent" | "flex-basis" => {
                matches!(
                    value,
                    "auto" | "none" | "0" | "inherit" | "initial" | "unset"
                        | "min-content" | "max-content" | "fit-content"
                ) || value.ends_with("px")
                    || value.ends_with("em")
                    || value.ends_with("rem")
                    || value.ends_with('%')
                    || value.ends_with("vw")
                    || value.ends_with("vh")
                    || value.ends_with("pt")
                    || value.ends_with("ch")
                    || value.starts_with("calc(")
                    || value.parse::<f32>().is_ok()
            }
            "color" | "background-color" | "background" => {
                value.starts_with('#')
                    || value.starts_with("rgb")
                    || value.starts_with("hsl")
                    || value.starts_with("oklch")
                    || value.starts_with("oklab")
                    || value.starts_with("color(")
                    || value.starts_with("hwb")
                    || matches!(
                        value,
                        "transparent" | "currentColor" | "inherit" | "initial" | "unset" | "none"
                    )
                    || crate::value::Color::from_hex(value).is_ok()
                    || csscolorparser::parse(value).is_ok()
            }
            "opacity" | "flex-grow" | "flex-shrink" | "z-index" | "order" | "font-weight" => {
                matches!(
                    value,
                    "auto" | "normal" | "bold" | "bolder" | "lighter"
                        | "inherit" | "initial" | "unset"
                ) || value.parse::<f32>().is_ok()
            }
            _ => Self::is_supported_css_property(property),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Color;

    #[test]
    fn test_stylesheet() {
        let mut sheet = StyleSheet::new();

        let selector = Selector::element("button");
        let mut properties = PropertySet::new();
        properties.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );

        sheet.add_rule(selector, properties);

        assert_eq!(sheet.rule_count(), 1);
    }

    #[test]
    fn test_cascade() {
        let mut sheet = StyleSheet::new();

        // Less specific rule
        let selector1 = Selector::element("button");
        let mut props1 = PropertySet::new();
        props1.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        sheet.add_rule(selector1, props1);

        // More specific rule
        let selector2 = Selector::element("button").with_class("primary");
        let mut props2 = PropertySet::new();
        props2.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(0, 255, 0)),
        );
        sheet.add_rule(selector2, props2);

        // Query with class
        let styles = sheet.compute_styles("button", &vec!["primary".to_string()], None, &[]);

        // Should get green background (more specific)
        let color = styles.get("background").unwrap().as_color().unwrap();
        assert_eq!(color.g, 255);
    }

    #[test]
    fn test_layer_and_conditions_cascade() {
        let mut sheet = StyleSheet::new();
        sheet.add_layer("base");
        sheet.add_layer("components");

        let mut base_props = PropertySet::new();
        base_props.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        sheet.add_rule_with_conditions(
            Selector::element("button"),
            base_props,
            None,
            None,
            Some("base".to_string()),
        );

        let mut component_props = PropertySet::new();
        component_props.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(0, 255, 0)),
        );
        sheet.add_rule_with_conditions(
            Selector::element("button"),
            component_props,
            Some("(max-width: 600px)".to_string()),
            Some("(display: grid)".to_string()),
            Some("components".to_string()),
        );

        let env = QueryEnvironment {
            viewport_width: 500.0,
            ..QueryEnvironment::default()
        };
        let styles = sheet.compute_styles_with_environment("button", &[], None, &[], &env);
        let color = styles.get("color").unwrap().as_color().unwrap();
        assert_eq!(color.g, 255);

        let env_desktop = QueryEnvironment {
            viewport_width: 1200.0,
            ..QueryEnvironment::default()
        };
        let desktop = sheet.compute_styles_with_environment("button", &[], None, &[], &env_desktop);
        let desktop_color = desktop.get("color").unwrap().as_color().unwrap();
        assert_eq!(desktop_color.r, 255);
    }
}
