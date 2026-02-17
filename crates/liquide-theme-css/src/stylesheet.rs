//! CSS stylesheet representation

use crate::property::PropertySet;
use crate::selector::Selector;
use crate::value::{FontFaceRule, KeyframesRule, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
            layer: None,
        }
    }

    /// Create a style rule gated on a media condition.
    pub fn with_media_condition(mut self, condition: String) -> Self {
        self.media_condition = Some(condition);
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

    /// Add a rule that is gated on a media condition string.
    pub fn add_conditional_rule(
        &mut self,
        selector: Selector,
        properties: PropertySet,
        media_condition: String,
    ) {
        self.rules
            .push(StyleRule::new(selector, properties).with_media_condition(media_condition));
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
        let mut matching = Vec::new();

        for rule in &self.rules {
            if rule.selector.matches(element, classes, id, pseudo_classes) {
                matching.push(rule);
            }
        }

        // Sort by specificity (higher specificity = higher priority)
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
        let matching = self.find_matching_rules(element, classes, id, pseudo_classes);

        let mut final_properties = PropertySet::new();

        // Apply rules in reverse specificity order (lowest first)
        for rule in matching.iter().rev() {
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
}
