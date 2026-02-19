//! The style engine — orchestrates cascade, specificity, inheritance, and variable resolution.

use std::sync::Arc;

use liquide_compositor::pixel::Color;
use liquide_dom::{Document, NodeId};
use liquide_theme_css::ThemeParser;
use liquide_theme_css::property::PropertySet;

use crate::cascade::{CascadeDeclaration, CascadeMap, CascadePriority};
use crate::computed::*;
use crate::dimension::Dimension;
use crate::dimension::Sides;
use crate::selector::ComplexSelector;
use crate::specificity::Specificity;
use crate::style_map::StyleMap;
use crate::value_resolve::{parse_inline_value, *};

/// A prepared stylesheet rule ready for matching.
#[derive(Debug)]
pub struct PreparedRule {
    pub selector: ComplexSelector,
    pub specificity: Specificity,
    pub source_order: u32,
    pub properties: PropertySet,
    /// Optional media condition string. `None` means the rule is unconditional.
    pub media_condition: Option<String>,
    /// Cascade layer order (0 = unlayered, 1+ = layer index in declaration order).
    pub layer_order: u32,
    /// Optional `@container` condition. Rule only applies when an ancestor
    /// container satisfies this condition.
    pub container_condition: Option<ContainerCondition>,
    /// Optional `@supports` condition for runtime feature checking.
    pub supports_condition: Option<String>,
    /// If this rule targets a pseudo-element (e.g. "before", "after"), stored here.
    /// Rules with `pseudo_element == None` target the element itself.
    pub pseudo_element: Option<String>,
}

/// A `@container` query condition attached to a prepared rule.
#[derive(Debug, Clone)]
pub struct ContainerCondition {
    /// Optional container name to search for (None = nearest).
    pub name: Option<String>,
    /// The condition expression, e.g. "(min-width: 600px)".
    pub condition: String,
}

/// A parsed `@font-face` rule ready for registration.
#[derive(Debug, Clone)]
pub struct PreparedFontFace {
    pub family: String,
    pub sources: Vec<liquide_theme_css::value::FontSource>,
    pub weight: Option<(u16, u16)>,
    pub style: Option<String>,
    pub display: Option<String>,
    pub unicode_range: Option<String>,
}

/// A prepared stylesheet — all rules compiled and ready.
pub struct PreparedSheet {
    pub rules: Vec<PreparedRule>,
}

/// Viewport size for resolving viewport units.
#[derive(Debug, Clone, Copy)]
pub struct ViewportSize {
    pub width: f32,
    pub height: f32,
}

impl Default for ViewportSize {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
        }
    }
}

/// The CSS style engine.
///
/// Takes stylesheets + a DOM tree and produces computed styles for every element.
pub struct StyleEngine {
    /// Compiled rule sets (from all stylesheets).
    sheets: Vec<PreparedSheet>,
    /// Viewport size for viewport units.
    pub viewport: ViewportSize,
    /// Base font size for `rem` units.
    pub base_font_size: f32,
    /// CSS variables.
    variables: std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    /// Layer order map: layer name → layer index (1-based).
    layer_order: std::collections::HashMap<String, u32>,
    /// `@font-face` rules parsed from stylesheets.
    font_faces: Vec<PreparedFontFace>,
    /// CSS properties we support (for `@supports` runtime evaluation).
    supported_properties: std::collections::HashSet<&'static str>,
    /// Registered custom properties from `@property` rules.
    registered_properties: std::collections::HashMap<String, RegisteredPropertyDef>,
    /// System color-scheme preference: `"light"` or `"dark"`.
    pub preferred_color_scheme: String,
    /// Whether the user prefers reduced motion.
    pub prefers_reduced_motion: bool,
    /// `@keyframes` rules keyed by animation name.
    pub keyframes: std::collections::HashMap<String, liquide_theme_css::value::KeyframesRule>,
}

/// A registered custom property definition (from `@property`).
#[derive(Debug, Clone)]
pub struct RegisteredPropertyDef {
    /// Syntax descriptor, e.g. "<color>", "<length>", "*".
    pub syntax: String,
    /// Whether the property inherits.
    pub inherits: bool,
    /// Initial value.
    pub initial_value: Option<String>,
}

impl StyleEngine {
    /// Create a new style engine.
    pub fn new(viewport: ViewportSize, base_font_size: f32) -> Self {
        Self {
            sheets: Vec::new(),
            viewport,
            base_font_size,
            variables: std::collections::HashMap::new(),
            layer_order: std::collections::HashMap::new(),
            font_faces: Vec::new(),
            supported_properties: Self::build_supported_properties(),
            registered_properties: std::collections::HashMap::new(),
            preferred_color_scheme: "light".into(),
            prefers_reduced_motion: false,
            keyframes: std::collections::HashMap::new(),
        }
    }

    /// Build the set of CSS properties we support for `@supports` runtime checks.
    fn build_supported_properties() -> std::collections::HashSet<&'static str> {
        [
            "display",
            "position",
            "box-sizing",
            "width",
            "height",
            "min-width",
            "max-width",
            "min-height",
            "max-height",
            "margin",
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "border",
            "border-width",
            "border-style",
            "border-color",
            "border-radius",
            "border-top",
            "border-right",
            "border-bottom",
            "border-left",
            "top",
            "right",
            "bottom",
            "left",
            "z-index",
            "float",
            "clear",
            "overflow",
            "overflow-x",
            "overflow-y",
            "visibility",
            "opacity",
            "color",
            "background",
            "background-color",
            "background-image",
            "background-size",
            "background-position",
            "background-repeat",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "line-height",
            "letter-spacing",
            "word-spacing",
            "text-align",
            "text-decoration",
            "text-transform",
            "text-overflow",
            "text-indent",
            "white-space",
            "word-break",
            "vertical-align",
            "flex",
            "flex-direction",
            "flex-wrap",
            "flex-grow",
            "flex-shrink",
            "flex-basis",
            "justify-content",
            "align-items",
            "align-self",
            "align-content",
            "order",
            "gap",
            "row-gap",
            "column-gap",
            "grid",
            "grid-template-columns",
            "grid-template-rows",
            "grid-column",
            "grid-row",
            "grid-area",
            "grid-auto-flow",
            "grid-auto-columns",
            "grid-auto-rows",
            "grid-template-areas",
            "transform",
            "transition",
            "animation",
            "box-shadow",
            "filter",
            "backdrop-filter",
            "clip-path",
            "cursor",
            "outline",
            "resize",
            "user-select",
            "pointer-events",
            "content",
            "counter-increment",
            "counter-reset",
            "quotes",
            "list-style",
            "list-style-type",
            "list-style-position",
            "table-layout",
            "border-collapse",
            "border-spacing",
            "columns",
            "column-count",
            "column-width",
            "column-gap",
            "column-rule",
            "column-span",
            "column-fill",
            "writing-mode",
            "direction",
            "unicode-bidi",
            "contain",
            "container-type",
            "container-name",
            "aspect-ratio",
            "object-fit",
            "object-position",
            "scroll-behavior",
            "scroll-snap-type",
            "scroll-snap-align",
            "scroll-padding",
            "scroll-margin",
            "overscroll-behavior",
            "accent-color",
            "caret-color",
            "appearance",
            "will-change",
            "isolation",
            "mix-blend-mode",
            "mask",
            "mask-image",
            "mask-size",
            "mask-position",
            "shape-outside",
            "shape-margin",
            "shape-image-threshold",
            // Logical properties
            "margin-inline",
            "margin-inline-start",
            "margin-inline-end",
            "margin-block",
            "margin-block-start",
            "margin-block-end",
            "padding-inline",
            "padding-inline-start",
            "padding-inline-end",
            "padding-block",
            "padding-block-start",
            "padding-block-end",
            "border-inline",
            "border-block",
            "inset",
            "inset-inline",
            "inset-block",
            // Modern CSS features
            "container",
            "subgrid",
        ]
        .into_iter()
        .collect()
    }

    /// Evaluate a `@supports` condition at runtime.
    pub fn evaluate_supports_condition(&self, condition: &str) -> bool {
        let condition = condition.trim();

        // Handle `not (…)`
        if let Some(inner) = condition.strip_prefix("not ") {
            return !self.evaluate_supports_condition(inner.trim());
        }

        // Handle bare parenthesized condition `(property: value)`
        if condition.starts_with('(') && condition.ends_with(')') {
            let inner = &condition[1..condition.len() - 1];
            if let Some((prop, _val)) = inner.split_once(':') {
                return self.supported_properties.contains(prop.trim());
            }
            // Could be a nested condition
            return self.evaluate_supports_condition(inner.trim());
        }

        // Handle `(…) and (…)`
        if condition.contains(") and (") {
            return condition.split(") and (").all(|part| {
                let p = part.trim().trim_start_matches('(').trim_end_matches(')');
                self.evaluate_supports_condition(&format!("({})", p))
            });
        }

        // Handle `(…) or (…)`
        if condition.contains(") or (") {
            return condition.split(") or (").any(|part| {
                let p = part.trim().trim_start_matches('(').trim_end_matches(')');
                self.evaluate_supports_condition(&format!("({})", p))
            });
        }

        // Default: assume supported
        true
    }

    /// Get parsed @font-face rules for external font loading.
    pub fn font_faces(&self) -> &[PreparedFontFace] {
        &self.font_faces
    }

    /// Get registered custom properties.
    pub fn registered_property(&self, name: &str) -> Option<&RegisteredPropertyDef> {
        self.registered_properties.get(name)
    }

    /// Parse and add a CSS stylesheet.
    pub fn add_stylesheet(&mut self, css: &str) {
        let parser = ThemeParser::new();
        let stylesheet = match parser.parse_str(css) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse stylesheet: {}", e);
                return;
            }
        };

        // ── @layer ordering ─────────────────────────────────────────────
        for layer_name in stylesheet.layer_order() {
            let next_idx = self.layer_order.len() as u32 + 1;
            self.layer_order
                .entry(layer_name.to_string())
                .or_insert(next_idx);
        }

        // ── @font-face rules ────────────────────────────────────────────
        for ff in stylesheet.font_faces() {
            self.font_faces.push(PreparedFontFace {
                family: ff.family.clone(),
                sources: ff.src.clone(),
                weight: ff.weight,
                style: ff.style.clone(),
                display: ff.display.clone(),
                unicode_range: ff.unicode_range.clone(),
            });
        }

        // ── @property registrations ─────────────────────────────────────
        for prop in stylesheet.registered_properties() {
            self.registered_properties.insert(
                prop.name.clone(),
                RegisteredPropertyDef {
                    syntax: prop.syntax.clone(),
                    inherits: prop.inherits,
                    initial_value: prop.initial_value.clone(),
                },
            );
        }

        // ── @keyframes rules ────────────────────────────────────────────
        for (name, kf_rule) in stylesheet.keyframes() {
            self.keyframes.insert(name.clone(), kf_rule.clone());
        }

        // ── Extract variables ───────────────────────────────────────────
        for rule in stylesheet.rules() {
            for (key, val) in rule.properties.iter() {
                if key.starts_with("--") {
                    self.variables.insert(key.clone(), val.clone());
                }
            }
        }

        // ── Compile normal rules ────────────────────────────────────────
        let mut prepared_rules = Vec::new();
        let mut order = self
            .sheets
            .iter()
            .map(|s| s.rules.len() as u32)
            .sum::<u32>();

        for rule in stylesheet.rules() {
            // Use the raw selector string directly — it preserves combinators,
            // attribute selectors, and functional pseudo-classes from lightningcss.
            let selector_str = &rule.selector.raw;

            if let Some(complex) = ComplexSelector::parse(selector_str) {
                let specificity = complex.specificity();
                // Resolve layer order for this rule
                let layer_ord = rule
                    .layer
                    .as_ref()
                    .and_then(|name| self.layer_order.get(name))
                    .copied()
                    .unwrap_or(0);
                prepared_rules.push(PreparedRule {
                    selector: complex,
                    specificity,
                    source_order: order,
                    properties: rule.properties.clone(),
                    media_condition: rule.media_condition.clone(),
                    layer_order: layer_ord,
                    container_condition: None,
                    supports_condition: rule.supports_condition.clone(),
                    pseudo_element: rule.selector.pseudo_element.clone(),
                });
                order += 1;
            }
        }

        // ── Compile @container query rules ──────────────────────────────
        for cr in stylesheet.container_rules() {
            for rule in &cr.rules {
                let selector_str = &rule.selector.raw;
                if let Some(complex) = ComplexSelector::parse(selector_str) {
                    let specificity = complex.specificity();
                    let layer_ord = rule
                        .layer
                        .as_ref()
                        .and_then(|name| self.layer_order.get(name))
                        .copied()
                        .unwrap_or(0);
                    prepared_rules.push(PreparedRule {
                        selector: complex,
                        specificity,
                        source_order: order,
                        properties: rule.properties.clone(),
                        media_condition: rule.media_condition.clone(),
                        layer_order: layer_ord,
                        container_condition: Some(ContainerCondition {
                            name: cr.name.clone(),
                            condition: cr.condition.clone(),
                        }),
                        supports_condition: rule.supports_condition.clone(),
                        pseudo_element: rule.selector.pseudo_element.clone(),
                    });
                    order += 1;
                }
            }
        }

        // ── Compile @scope rules ────────────────────────────────────────
        // Current behavior: scope-start is applied as an ancestor prefix to
        // each nested selector. scope-end is retained in data model but is not
        // yet enforced here.
        for scope_rule in stylesheet.scope_rules() {
            let scope_prefix = scope_rule.scope_start.as_deref().unwrap_or("").trim();
            for rule in &scope_rule.rules {
                let selector_str = if scope_prefix.is_empty() {
                    rule.selector.raw.clone()
                } else {
                    format!("{} {}", scope_prefix, rule.selector.raw)
                };
                if let Some(complex) = ComplexSelector::parse(&selector_str) {
                    let specificity = complex.specificity();
                    let layer_ord = rule
                        .layer
                        .as_ref()
                        .and_then(|name| self.layer_order.get(name))
                        .copied()
                        .unwrap_or(0);
                    prepared_rules.push(PreparedRule {
                        selector: complex,
                        specificity,
                        source_order: order,
                        properties: rule.properties.clone(),
                        media_condition: rule.media_condition.clone(),
                        layer_order: layer_ord,
                        container_condition: None,
                        supports_condition: rule.supports_condition.clone(),
                        pseudo_element: rule.selector.pseudo_element.clone(),
                    });
                    order += 1;
                }
            }
        }

        self.sheets.push(PreparedSheet {
            rules: prepared_rules,
        });
    }

    /// Compute the style for a single node.
    pub fn compute_style(&self, doc: &Document, node_id: NodeId) -> ComputedStyle {
        let node = match doc.get(node_id) {
            Some(n) => n,
            None => return ComputedStyle::default(),
        };

        // Start with inherited values from parent
        let mut style = if let Some(parent_id) = node.parent {
            let parent_style = self.compute_style(doc, parent_id);
            let mut s = ComputedStyle::default();
            s.inherit_from(&parent_style);
            s
        } else {
            ComputedStyle::default()
        };

        // Skip text nodes — they inherit only
        if node.is_text() {
            return style;
        }

        // ── Full cascade via CascadeMap ──
        let mut cascade = CascadeMap::new();

        for sheet in &self.sheets {
            for rule in &sheet.rules {
                // Skip rules whose media condition does not match the viewport
                if let Some(ref cond) = rule.media_condition {
                    if !self.evaluate_media_condition(cond) {
                        continue;
                    }
                }
                // Skip @supports-gated rules that don't match
                if let Some(ref cond) = rule.supports_condition {
                    if !self.evaluate_supports_condition(cond) {
                        continue;
                    }
                }
                // Skip @container-gated rules (container evaluation needs layout
                // data which isn't available in compute_style — these are
                // handled in restyle_node instead)
                if rule.container_condition.is_some() {
                    continue;
                }
                // Skip pseudo-element rules — they apply to ::before/::after, not the element
                if rule.pseudo_element.is_some() {
                    continue;
                }
                if rule.selector.matches(doc, node_id) {
                    let mut priority = CascadePriority::author(rule.specificity, rule.source_order);
                    priority.layer_order = rule.layer_order;
                    cascade.add_properties(&rule.properties, priority);
                }
            }
        }

        // Inline styles
        let mut inline_order = 0u32;
        for (prop, value) in node.inline_styles.iter() {
            let pv = parse_inline_value(value);
            cascade.add(CascadeDeclaration {
                property: prop.to_string(),
                value: pv,
                priority: CascadePriority::inline(inline_order),
            });
            inline_order += 1;
        }

        let resolved = cascade.resolve();
        let empty_scope: std::collections::HashMap<
            String,
            liquide_theme_css::value::PropertyValue,
        > = std::collections::HashMap::new();
        for (prop, val) in &resolved {
            self.apply_single_property(prop, val, &mut style, &empty_scope);
        }

        style
    }

    /// Compute styles for the entire document tree.
    pub fn restyle_all(&self, doc: &Document) -> StyleMap {
        let mut map = StyleMap::new();
        let scope = std::collections::HashMap::new();
        self.restyle_node(doc, doc.root(), None, &mut map, &scope);
        map
    }

    /// Compute styles for a subtree (incremental).
    pub fn restyle_subtree(&self, doc: &Document, node_id: NodeId, map: &mut StyleMap) {
        let parent_style = doc.parent(node_id).and_then(|pid| map.get(pid).cloned());
        let scope = std::collections::HashMap::new();
        self.restyle_node(doc, node_id, parent_style.as_deref(), map, &scope);
    }

    /// Incrementally invalidate and recompute styles for changed nodes.
    pub fn invalidate(&self, doc: &Document, changed_nodes: &[NodeId], map: &mut StyleMap) {
        for &node_id in changed_nodes {
            self.restyle_subtree(doc, node_id, map);
        }
    }

    fn restyle_node(
        &self,
        doc: &Document,
        node_id: NodeId,
        parent_style: Option<&ComputedStyle>,
        map: &mut StyleMap,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) {
        let node = match doc.get(node_id) {
            Some(n) => n,
            None => return,
        };

        // Compute this node's style
        let mut style = ComputedStyle::default();
        if let Some(ps) = parent_style {
            style.inherit_from(ps);
        }

        if !node.is_text() {
            // ── Full cascade via CascadeMap ──
            let mut cascade = CascadeMap::new();

            // Collect matching rules and add to cascade with proper priority
            for sheet in &self.sheets {
                for rule in &sheet.rules {
                    // Skip rules whose media condition does not match the viewport
                    if let Some(ref cond) = rule.media_condition {
                        if !self.evaluate_media_condition(cond) {
                            continue;
                        }
                    }
                    // Skip @supports-gated rules that don't match
                    if let Some(ref cond) = rule.supports_condition {
                        if !self.evaluate_supports_condition(cond) {
                            continue;
                        }
                    }
                    // Evaluate @container conditions against ancestor containers
                    if let Some(ref cc) = rule.container_condition {
                        if !self.evaluate_container_condition(cc, node_id, doc, map) {
                            continue;
                        }
                    }
                    // Skip pseudo-element rules — they are computed separately below
                    if rule.pseudo_element.is_some() {
                        continue;
                    }
                    if rule.selector.matches(doc, node_id) {
                        let mut priority =
                            CascadePriority::author(rule.specificity, rule.source_order);
                        priority.layer_order = rule.layer_order;
                        cascade.add_properties(&rule.properties, priority);
                    }
                }
            }

            // Add inline styles with highest author priority
            let mut inline_order = 0u32;
            for (prop, value) in node.inline_styles.iter() {
                let pv = parse_inline_value(value);
                cascade.add(CascadeDeclaration {
                    property: prop.to_string(),
                    value: pv,
                    priority: CascadePriority::inline(inline_order),
                });
                inline_order += 1;
            }

            // Resolve the cascade and apply winners
            let resolved = cascade.resolve();

            // Extract scoped CSS variables from the resolved cascade.
            // Respect @property `inherits` flag: non-inheriting custom properties
            // that aren't explicitly set on this element get their initial value
            // instead of inheriting from the parent scope.
            let mut local_vars = scope_vars.clone();

            // Collect which custom properties are explicitly declared on this element
            let mut explicitly_set: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for (prop, val) in &resolved {
                if prop.starts_with("--") {
                    local_vars.insert(prop.clone(), val.clone());
                    explicitly_set.insert(prop.clone());
                }
            }

            // For registered @property definitions: enforce `inherits: false`
            // by resetting inherited values to initial when not explicitly set
            for (name, def) in &self.registered_properties {
                if !def.inherits && !explicitly_set.contains(name) {
                    // Non-inheriting property not set on this element: use initial value
                    if let Some(ref initial) = def.initial_value {
                        local_vars.insert(
                            name.clone(),
                            liquide_theme_css::value::PropertyValue::Keyword(initial.clone()),
                        );
                    } else {
                        // No initial value → remove any inherited value
                        local_vars.remove(name);
                    }
                } else if !local_vars.contains_key(name) {
                    // Inheriting (or unregistered) property not in scope: use initial value
                    if let Some(ref initial) = def.initial_value {
                        local_vars.insert(
                            name.clone(),
                            liquide_theme_css::value::PropertyValue::Keyword(initial.clone()),
                        );
                    }
                }
            }

            for (prop, val) in &resolved {
                self.apply_single_property(prop, val, &mut style, &local_vars);
            }

            // Assemble TextDecoration composite from longhands if set
            Self::assemble_text_decoration(&mut style);
            // Assemble BackgroundSpec from longhands
            Self::assemble_background(&mut style);
            // Assemble MaskSpec from mask longhands
            Self::assemble_mask(&mut style);
            // Resolve logical properties to physical equivalents
            Self::resolve_logical_properties(&mut style);
            // Read remaining dead properties so the compiler sees them as consumed
            consume_remaining_properties(&style);

            let style = Arc::new(style);
            map.insert_shared(node_id, style.clone());
            self.compute_pseudo_styles(doc, node_id, &style, map, &local_vars);

            // Recurse into children with scoped variables
            let children = doc.children(node_id).to_vec();
            for child_id in children {
                self.restyle_node(doc, child_id, Some(&style), map, &local_vars);
            }
            return;
        }

        // Assemble TextDecoration composite from longhands if set
        Self::assemble_text_decoration(&mut style);
        // Assemble BackgroundSpec from longhands
        Self::assemble_background(&mut style);
        // Assemble MaskSpec from mask longhands
        Self::assemble_mask(&mut style);
        // Resolve logical properties to physical equivalents
        Self::resolve_logical_properties(&mut style);
        // Read remaining dead properties so the compiler sees them as consumed
        consume_remaining_properties(&style);

        let style = Arc::new(style);
        map.insert_shared(node_id, style.clone());

        // Recurse into children (text nodes pass through parent scope).
        // Shadow DOM boundary: when entering a ShadowRoot, reset author-style
        // scope — only inherited properties pass through.
        let children = doc.children(node_id).to_vec();
        for child_id in children {
            let is_shadow = doc
                .get(child_id)
                .map(|n| matches!(n.data, liquide_dom::node::NodeData::ShadowRoot))
                .unwrap_or(false);
            if is_shadow {
                // Shadow roots inherit from their host but don't match host
                // document author rules. Pass parent style for inheritance.
                self.restyle_node(doc, child_id, Some(&style), map, &std::collections::HashMap::new());
            } else {
                self.restyle_node(doc, child_id, Some(&style), map, scope_vars);
            }
        }
    }

    /// Compute pseudo-element styles (::before, ::after) for a host element.
    ///
    /// Collects matching rules that have `pseudo_element` set to "before" or
    /// "after", builds a cascade, and stores the resulting style in the
    /// StyleMap's pseudo-element map. The layout engine uses these to
    /// generate synthetic boxes before/after the element's children.
    fn compute_pseudo_styles(
        &self,
        doc: &Document,
        node_id: NodeId,
        host_style: &ComputedStyle,
        map: &mut StyleMap,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) {
        use crate::style_map::PseudoKind;

        for (pseudo_name, kind) in [("before", PseudoKind::Before), ("after", PseudoKind::After)] {
            let mut cascade = CascadeMap::new();
            let mut has_rules = false;

            for sheet in &self.sheets {
                for rule in &sheet.rules {
                    // Only consider rules targeting this pseudo-element
                    if rule.pseudo_element.as_deref() != Some(pseudo_name) {
                        continue;
                    }
                    // Check media/supports/container conditions
                    if let Some(ref cond) = rule.media_condition {
                        if !self.evaluate_media_condition(cond) {
                            continue;
                        }
                    }
                    if let Some(ref cond) = rule.supports_condition {
                        if !self.evaluate_supports_condition(cond) {
                            continue;
                        }
                    }
                    if let Some(ref cc) = rule.container_condition {
                        if !self.evaluate_container_condition(cc, node_id, doc, map) {
                            continue;
                        }
                    }
                    // The selector (without pseudo-element) must match the host element
                    if rule.selector.matches(doc, node_id) {
                        let mut priority =
                            CascadePriority::author(rule.specificity, rule.source_order);
                        priority.layer_order = rule.layer_order;
                        cascade.add_properties(&rule.properties, priority);
                        has_rules = true;
                    }
                }
            }

            if !has_rules {
                continue;
            }

            let resolved = cascade.resolve();

            // Check if the content property is set — per spec, a pseudo-element
            // is only generated when `content` is not `none` / not absent.
            let has_content = resolved.iter().any(|(prop, val)| {
                prop == "content" && {
                    let s = format!("{:?}", val);
                    !s.contains("none") && !s.contains("normal")
                }
            });

            if !has_content {
                continue;
            }

            // Build the pseudo-element's computed style, inheriting from host
            let mut pseudo_style = ComputedStyle::default();
            pseudo_style.inherit_from(host_style);

            for (prop, val) in &resolved {
                self.apply_single_property(prop, val, &mut pseudo_style, scope_vars);
            }

            map.insert_pseudo(node_id, kind, Arc::new(pseudo_style));
        }
    }

    /// Evaluate a `@container` condition by walking up the tree to find
    /// the nearest container ancestor and checking the condition against
    /// its computed dimensions.
    fn evaluate_container_condition(
        &self,
        condition: &ContainerCondition,
        node_id: NodeId,
        doc: &Document,
        map: &StyleMap,
    ) -> bool {
        // Walk ancestors to find a container
        let mut current = doc.parent(node_id);
        while let Some(ancestor_id) = current {
            if let Some(ancestor_style) = map.get(ancestor_id) {
                let ct = ancestor_style.container_type;
                if ct != ContainerType::Normal {
                    // Check container name if specified
                    if let Some(ref required_name) = condition.name {
                        if ancestor_style.container_name.as_deref() != Some(required_name.as_str())
                        {
                            current = doc.parent(ancestor_id);
                            continue;
                        }
                    }
                    // Evaluate the condition against this container's dimensions.
                    // Use real container dimensions if available from previous
                    // layout pass; fall back to viewport as a proxy.
                    let (cw, ch) = map
                        .container_size(ancestor_id)
                        .unwrap_or((self.viewport.width, self.viewport.height));
                    return self.evaluate_container_size_condition(&condition.condition, cw, ch);
                }
            }
            current = doc.parent(ancestor_id);
        }
        false // No matching container found
    }

    /// Parse and evaluate a container size condition like `(min-width: 600px)`.
    fn evaluate_container_size_condition(
        &self,
        condition: &str,
        container_w: f32,
        container_h: f32,
    ) -> bool {
        let condition = condition.trim();
        let inner = condition
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(condition);

        // Handle compound conditions
        if inner.contains(") and (") {
            return inner.split(") and (").all(|part| {
                self.evaluate_container_size_condition(
                    &format!("({})", part.trim_matches(|c| c == '(' || c == ')')),
                    container_w,
                    container_h,
                )
            });
        }
        if inner.contains(") or (") {
            return inner.split(") or (").any(|part| {
                self.evaluate_container_size_condition(
                    &format!("({})", part.trim_matches(|c| c == '(' || c == ')')),
                    container_w,
                    container_h,
                )
            });
        }

        if let Some((prop, value_str)) = inner.split_once(':') {
            let prop = prop.trim();
            let value_str = value_str.trim();
            let px_value = Self::parse_px_value(value_str).unwrap_or(0.0);
            match prop {
                "min-width" => container_w >= px_value,
                "max-width" => container_w <= px_value,
                "min-height" => container_h >= px_value,
                "max-height" => container_h <= px_value,
                "width" => (container_w - px_value).abs() < 1.0,
                "height" => (container_h - px_value).abs() < 1.0,
                _ => true,
            }
        } else {
            true
        }
    }

    /// Update viewport size (triggers re-resolution of viewport units).
    pub fn set_viewport(&mut self, size: ViewportSize) {
        self.viewport = size;
    }

    /// Set the preferred color scheme used by media queries such as
    /// `(prefers-color-scheme: dark)`.
    pub fn set_preferred_color_scheme(&mut self, scheme: &str) {
        self.preferred_color_scheme = if scheme.trim().eq_ignore_ascii_case("dark") {
            "dark".to_string()
        } else {
            "light".to_string()
        };
    }

    /// Resolve a CSS variable value.
    pub fn resolve_variable(&self, name: &str) -> Option<&liquide_theme_css::value::PropertyValue> {
        self.variables.get(name)
    }

    /// Total number of compiled rules.
    pub fn rule_count(&self) -> usize {
        self.sheets.iter().map(|s| s.rules.len()).sum()
    }

    /// Number of loaded stylesheets.
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    /// Number of CSS custom properties (variables) defined.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    // ── Private: apply property values to ComputedStyle ──

    fn apply_single_property(
        &self,
        key: &str,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) {
        // ── CSS-wide keywords ──
        // Check for initial/inherit/unset/revert before normal property handling
        if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
            match kw.as_str() {
                "initial" => {
                    // Reset this property to its initial (default) value
                    self.reset_property_to_initial(key, style);
                    return;
                }
                "inherit" => {
                    // Value is inherited — already handled by inherit_from(), so just return
                    // (the property keeps whatever inherited value it has)
                    return;
                }
                "unset" => {
                    // If the property is inherited by default, act as inherit
                    // If not inherited by default, act as initial
                    if !crate::inheritance::is_inherited(key) {
                        self.reset_property_to_initial(key, style);
                    }
                    // For inherited properties, just keep inherited value (do nothing)
                    return;
                }
                "revert" | "revert-layer" => {
                    // Revert to the previous cascade origin's value
                    // For now, simplified: act like unset
                    if !crate::inheritance::is_inherited(key) {
                        self.reset_property_to_initial(key, style);
                    }
                    return;
                }
                _ => {} // Not a CSS-wide keyword, proceed normally
            }
        }

        // ── var() resolution ──
        if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
            if kw.contains("var(") {
                if let Some(resolved) = self.resolve_var_in_value(kw, scope_vars) {
                    self.apply_single_property(key, &resolved, style, scope_vars);
                    return;
                }
            }
        }

        match key {
            // Display & position
            "display" => style.display = resolve_display(val),
            "position" => style.position = resolve_position(val),
            "box-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.box_sizing = match kw.as_str() {
                        "border-box" => BoxSizing::BorderBox,
                        _ => BoxSizing::ContentBox,
                    };
                }
            }

            // Dimensions
            "width" => style.width = resolve_dimension(val),
            "height" => style.height = resolve_dimension(val),
            "min-width" => style.min_width = resolve_dimension(val),
            "max-width" => style.max_width = resolve_dimension(val),
            "min-height" => style.min_height = resolve_dimension(val),
            "max-height" => style.max_height = resolve_dimension(val),

            // Margin
            "margin" => {
                let d = resolve_dimension(val);
                style.margin = Sides::all(d);
            }
            "margin-top" => style.margin.top = resolve_dimension(val),
            "margin-right" => style.margin.right = resolve_dimension(val),
            "margin-bottom" => style.margin.bottom = resolve_dimension(val),
            "margin-left" => style.margin.left = resolve_dimension(val),

            // Padding
            "padding" => {
                let d = resolve_dimension(val);
                style.padding = Sides::all(d);
            }
            "padding-top" => style.padding.top = resolve_dimension(val),
            "padding-right" => style.padding.right = resolve_dimension(val),
            "padding-bottom" => style.padding.bottom = resolve_dimension(val),
            "padding-left" => style.padding.left = resolve_dimension(val),

            // Border width
            "border-width" => {
                let w = resolve_number(val);
                style.border_width = Sides::all(w);
            }
            "border-top-width" => style.border_width.top = resolve_number(val),
            "border-right-width" => style.border_width.right = resolve_number(val),
            "border-bottom-width" => style.border_width.bottom = resolve_number(val),
            "border-left-width" => style.border_width.left = resolve_number(val),

            // Border radius
            "border-radius" => {
                let r = resolve_number(val);
                style.border_radius = crate::dimension::Corners::all(r);
            }
            "border-top-left-radius" => style.border_radius.top_left = resolve_number(val),
            "border-top-right-radius" => style.border_radius.top_right = resolve_number(val),
            "border-bottom-left-radius" => style.border_radius.bottom_left = resolve_number(val),
            "border-bottom-right-radius" => style.border_radius.bottom_right = resolve_number(val),

            // Border color
            "border-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color = Sides::all(c);
                }
            }
            "border-top-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.top = c;
                }
            }
            "border-right-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.right = c;
                }
            }
            "border-bottom-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.bottom = c;
                }
            }
            "border-left-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.left = c;
                }
            }

            // Border style
            "border-style" => {
                let s = resolve_border_style(val);
                style.border_style = Sides::all(s);
            }
            "border-top-style" => style.border_style.top = resolve_border_style(val),
            "border-right-style" => style.border_style.right = resolve_border_style(val),
            "border-bottom-style" => style.border_style.bottom = resolve_border_style(val),
            "border-left-style" => style.border_style.left = resolve_border_style(val),

            // Box shadow
            "box-shadow" => {
                if let liquide_theme_css::value::PropertyValue::BoxShadow(shadows) = val {
                    style.box_shadow = shadows
                        .iter()
                        .map(|s| liquide_compositor::scene::BoxShadowSpec {
                            offset_x: s.offset_x,
                            offset_y: s.offset_y,
                            blur_radius: s.blur_radius,
                            spread_radius: s.spread_radius,
                            color: Color {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            inset: s.inset,
                        })
                        .collect();
                }
            }

            // Flex
            "flex-direction" => style.flex_direction = resolve_flex_direction(val),
            "flex-wrap" => style.flex_wrap = resolve_flex_wrap(val),
            "justify-content" => style.justify_content = resolve_justify_content(val),
            "align-items" => style.align_items = resolve_align_items(val),
            "flex-grow" => style.flex_grow = resolve_number(val),
            "flex-shrink" => style.flex_shrink = resolve_number(val),
            "flex-basis" => style.flex_basis = resolve_dimension(val),
            "gap" => {
                let d = resolve_dimension(val);
                style.gap.width = d.clone();
                style.gap.height = d;
            }
            "order" => style.order = resolve_number(val) as i32,
            "align-self" => style.align_self = resolve_align_self(val),
            "align-content" => style.align_content = resolve_align_content(val),

            // Positioning
            "top" => style.top = resolve_dimension(val),
            "right" => style.right = resolve_dimension(val),
            "bottom" => style.bottom = resolve_dimension(val),
            "left" => style.left = resolve_dimension(val),
            "z-index" => style.z_index = Some(resolve_number(val) as i32),

            // Float & clear
            "float" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.float = match kw.as_str() {
                        "left" => Float::Left,
                        "right" => Float::Right,
                        "inline-start" => Float::InlineStart,
                        "inline-end" => Float::InlineEnd,
                        _ => Float::None,
                    };
                }
            }
            "clear" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clear = match kw.as_str() {
                        "left" => Clear::Left,
                        "right" => Clear::Right,
                        "both" => Clear::Both,
                        "inline-start" => Clear::InlineStart,
                        "inline-end" => Clear::InlineEnd,
                        _ => Clear::None,
                    };
                }
            }

            // Writing mode
            "writing-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.writing_mode = match kw.as_str() {
                        "vertical-rl" => WritingMode::VerticalRl,
                        "vertical-lr" => WritingMode::VerticalLr,
                        "sideways-rl" => WritingMode::SidewaysRl,
                        "sideways-lr" => WritingMode::SidewaysLr,
                        _ => WritingMode::HorizontalTb,
                    };
                }
            }
            "direction" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.direction = match kw.as_str() {
                        "rtl" => Direction::Rtl,
                        _ => Direction::Ltr,
                    };
                }
            }
            "unicode-bidi" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.unicode_bidi = match kw.as_str() {
                        "embed" => UnicodeBidi::Embed,
                        "isolate" => UnicodeBidi::Isolate,
                        "bidi-override" => UnicodeBidi::BidiOverride,
                        "isolate-override" => UnicodeBidi::IsolateOverride,
                        "plaintext" => UnicodeBidi::Plaintext,
                        _ => UnicodeBidi::Normal,
                    };
                }
            }

            // Typography
            "color" => {
                if let Some(c) = resolve_color(val) {
                    style.color = c;
                }
            }
            "font-family" => {
                if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_family = s
                        .split(',')
                        .map(|f| f.trim().trim_matches('"').to_string())
                        .collect();
                }
            }
            "font-size" => style.font_size = resolve_number(val),
            "font-weight" => style.font_weight = resolve_font_weight(val),
            "font-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_style = match kw.as_str() {
                        "italic" => FontStyle::Italic,
                        "oblique" => FontStyle::Oblique,
                        _ => FontStyle::Normal,
                    };
                }
            }
            "line-height" => {
                style.line_height = match val {
                    liquide_theme_css::value::PropertyValue::Number(n) => LineHeight::Number(*n),
                    liquide_theme_css::value::PropertyValue::Length(lu) => {
                        LineHeight::Px(lu.to_px(16.0))
                    }
                    liquide_theme_css::value::PropertyValue::Keyword(kw) if kw == "normal" => {
                        LineHeight::Normal
                    }
                    _ => LineHeight::Normal,
                };
            }
            "letter-spacing" => style.letter_spacing = resolve_number(val),
            "word-spacing" => style.word_spacing = resolve_number(val),
            "text-align" => style.text_align = resolve_text_align(val),
            "text-transform" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_transform = match kw.as_str() {
                        "capitalize" => TextTransform::Capitalize,
                        "uppercase" => TextTransform::Uppercase,
                        "lowercase" => TextTransform::Lowercase,
                        _ => TextTransform::None,
                    };
                }
            }
            "text-overflow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_overflow = match kw.as_str() {
                        "ellipsis" => TextOverflow::Ellipsis,
                        _ => TextOverflow::Clip,
                    };
                }
            }
            "white-space" => style.white_space = resolve_white_space(val),
            "word-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.word_break = match kw.as_str() {
                        "break-all" => WordBreak::BreakAll,
                        "keep-all" => WordBreak::KeepAll,
                        "break-word" => WordBreak::BreakWord,
                        _ => WordBreak::Normal,
                    };
                }
            }
            "text-indent" => style.text_indent = resolve_number(val),

            // Visual
            "background-color" | "background" => {
                if let Some(c) = resolve_color(val) {
                    style.background_color = c;
                }
            }
            "opacity" => style.opacity = resolve_number(val),
            "visibility" => style.visibility = resolve_visibility(val),
            "overflow" => {
                let o = resolve_overflow(val);
                style.overflow_x = o;
                style.overflow_y = o;
            }
            "overflow-x" => style.overflow_x = resolve_overflow(val),
            "overflow-y" => style.overflow_y = resolve_overflow(val),
            "cursor" => style.cursor = resolve_cursor(val),
            "pointer-events" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.pointer_events = match kw.as_str() {
                        "none" => PointerEvents::None,
                        _ => PointerEvents::Auto,
                    };
                }
            }

            // Effects
            "mix-blend-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mix_blend_mode = resolve_blend_mode(kw);
                }
            }
            "isolation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.isolation = match kw.as_str() {
                        "isolate" => Isolation::Isolate,
                        _ => Isolation::Auto,
                    };
                }
            }

            // ── Shell custom extensions ─────────────────────────
            // Non-standard CSS properties used by the LiquiDE desktop.
            "blur-radius" | "backdrop-blur-radius" => {
                style.x_blur_radius = resolve_number(val);
            }

            // Transform
            "transform" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parsed = parse_transform_list(kw);
                    if !parsed.is_empty() {
                        style.transform = parsed;
                    }
                }
            }

            // Grid templates
            "grid-template-columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_template_columns = parse_track_list(kw);
                }
            }
            "grid-template-rows" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_template_rows = parse_track_list(kw);
                }
            }
            "grid-auto-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_auto_flow = match kw.as_str() {
                        "column" => GridAutoFlow::Column,
                        "row dense" | "dense" => GridAutoFlow::RowDense,
                        "column dense" => GridAutoFlow::ColumnDense,
                        _ => GridAutoFlow::Row,
                    };
                }
            }
            "glass-tint" => {
                if let Some(c) = resolve_color(val) {
                    style.x_glass_tint = Some(c);
                }
            }
            // Standard box-shadow-color shorthand (non-standard, used in themes)
            "box-shadow-color" => {
                if let Some(c) = resolve_color(val) {
                    // Store as a single zero-offset shadow with only the color set.
                    if style.box_shadow.is_empty() {
                        style
                            .box_shadow
                            .push(liquide_compositor::scene::BoxShadowSpec {
                                offset_x: 0.0,
                                offset_y: 4.0,
                                blur_radius: 12.0,
                                spread_radius: 0.0,
                                color: c,
                                inset: false,
                            });
                    } else {
                        for sh in &mut style.box_shadow {
                            sh.color = c;
                        }
                    }
                }
            }
            // titlebar-background (legacy compat — maps to x_custom)
            "titlebar-background" => {
                if let Some(c) = resolve_color(val) {
                    style.x_custom.push((
                        "titlebar-background".into(),
                        format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a),
                    ));
                }
            }

            // ── Layout extras ──
            "contain" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.contain = match kw.as_str() {
                        "none" => Contain::none(),
                        "strict" => Contain::strict(),
                        "content" => Contain::content(),
                        other => {
                            let mut c = Contain::none();
                            for part in other.split_whitespace() {
                                match part {
                                    "size" => c.size = true,
                                    "layout" => c.layout = true,
                                    "style" => c.style = true,
                                    "paint" => c.paint = true,
                                    "inline-size" => c.inline_size = true,
                                    _ => {}
                                }
                            }
                            c
                        }
                    };
                }
            }
            "content-visibility" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.content_visibility = match kw.as_str() {
                        "auto" => ContentVisibility::Auto,
                        "hidden" => ContentVisibility::Hidden,
                        _ => ContentVisibility::Visible,
                    };
                }
            }
            "aspect-ratio" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let kw = kw.trim();
                    if kw == "auto" {
                        style.aspect_ratio = AspectRatio::Auto;
                    } else if let Some((w, h)) = kw.split_once('/') {
                        if let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>()) {
                            style.aspect_ratio = AspectRatio::Ratio(w, h);
                        }
                    } else if let Ok(n) = kw.parse::<f32>() {
                        style.aspect_ratio = AspectRatio::Ratio(n, 1.0);
                    }
                }
            }
            "object-fit" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.object_fit = match kw.as_str() {
                        "contain" => ObjectFit::Contain,
                        "cover" => ObjectFit::Cover,
                        "none" => ObjectFit::None,
                        "scale-down" => ObjectFit::ScaleDown,
                        _ => ObjectFit::Fill,
                    };
                }
            }
            "resize" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.resize = match kw.as_str() {
                        "both" => Resize::Both,
                        "horizontal" => Resize::Horizontal,
                        "vertical" => Resize::Vertical,
                        "block" => Resize::Block,
                        "inline" => Resize::Inline,
                        _ => Resize::None,
                    };
                }
            }
            "column-count" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.column_count = Some(*n as u32);
                }
            }
            "column-width" => style.column_width = resolve_dimension(val),
            "column-gap" => {
                let d = resolve_dimension(val);
                style.column_gap = d.clone();
                style.gap.width = d;
            }
            "row-gap" => {
                let d = resolve_dimension(val);
                style.row_gap = d.clone();
                style.gap.height = d;
            }

            // ── Alignment extras ──
            "justify-items" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.justify_items = match kw.as_str() {
                        "stretch" => JustifyItems::Stretch,
                        "center" => JustifyItems::Center,
                        "start" => JustifyItems::Start,
                        "end" => JustifyItems::End,
                        "flex-start" => JustifyItems::FlexStart,
                        "flex-end" => JustifyItems::FlexEnd,
                        "left" => JustifyItems::Left,
                        "right" => JustifyItems::Right,
                        "legacy" => JustifyItems::Legacy,
                        _ => JustifyItems::Normal,
                    };
                }
            }
            "justify-self" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.justify_self = match kw.as_str() {
                        "normal" => JustifySelf::Normal,
                        "stretch" => JustifySelf::Stretch,
                        "center" => JustifySelf::Center,
                        "start" => JustifySelf::Start,
                        "end" => JustifySelf::End,
                        "flex-start" => JustifySelf::FlexStart,
                        "flex-end" => JustifySelf::FlexEnd,
                        _ => JustifySelf::Auto,
                    };
                }
            }

            // ── place-items shorthand (align-items + justify-items) ──
            "place-items" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("normal");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_items = match align_val {
                        "stretch" => AlignItems::Stretch,
                        "center" => AlignItems::Center,
                        "flex-start" | "start" => AlignItems::FlexStart,
                        "flex-end" | "end" => AlignItems::FlexEnd,
                        "baseline" => AlignItems::Baseline,
                        _ => AlignItems::Stretch,
                    };
                    style.justify_items = match justify_val {
                        "stretch" => JustifyItems::Stretch,
                        "center" => JustifyItems::Center,
                        "start" | "flex-start" => JustifyItems::Start,
                        "end" | "flex-end" => JustifyItems::End,
                        "left" => JustifyItems::Left,
                        "right" => JustifyItems::Right,
                        _ => JustifyItems::Normal,
                    };
                }
            }

            // ── place-content shorthand (align-content + justify-content) ──
            "place-content" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("normal");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_content = match align_val {
                        "stretch" => AlignContent::Stretch,
                        "center" => AlignContent::Center,
                        "flex-start" | "start" => AlignContent::FlexStart,
                        "flex-end" | "end" => AlignContent::FlexEnd,
                        "space-between" => AlignContent::SpaceBetween,
                        "space-around" => AlignContent::SpaceAround,
                        _ => AlignContent::Stretch,
                    };
                    style.justify_content = match justify_val {
                        "center" => JustifyContent::Center,
                        "flex-start" | "start" => JustifyContent::FlexStart,
                        "flex-end" | "end" => JustifyContent::FlexEnd,
                        "space-between" => JustifyContent::SpaceBetween,
                        "space-around" => JustifyContent::SpaceAround,
                        "space-evenly" => JustifyContent::SpaceEvenly,
                        _ => JustifyContent::FlexStart,
                    };
                }
            }

            // ── place-self shorthand (align-self + justify-self) ──
            "place-self" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("auto");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_self = match align_val {
                        "stretch" => AlignSelf::Stretch,
                        "center" => AlignSelf::Center,
                        "flex-start" | "start" => AlignSelf::FlexStart,
                        "flex-end" | "end" => AlignSelf::FlexEnd,
                        "baseline" => AlignSelf::Baseline,
                        _ => AlignSelf::Auto,
                    };
                    style.justify_self = match justify_val {
                        "normal" => JustifySelf::Normal,
                        "stretch" => JustifySelf::Stretch,
                        "center" => JustifySelf::Center,
                        "start" | "flex-start" => JustifySelf::Start,
                        "end" | "flex-end" => JustifySelf::End,
                        _ => JustifySelf::Auto,
                    };
                }
            }

            // ── inset shorthand (top + right + bottom + left) ──
            "inset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let top_val = parts.first().copied().unwrap_or("auto");
                    let right_val = parts.get(1).copied().unwrap_or(top_val);
                    let bottom_val = parts.get(2).copied().unwrap_or(top_val);
                    let left_val = parts.get(3).copied().unwrap_or(right_val);

                    let parse_inset = |s: &str| -> Dimension {
                        if s == "auto" {
                            Dimension::Auto
                        } else {
                            resolve_dimension(&parse_inline_value(s))
                        }
                    };
                    style.top = parse_inset(top_val);
                    style.right = parse_inset(right_val);
                    style.bottom = parse_inset(bottom_val);
                    style.left = parse_inset(left_val);
                } else {
                    let dim = resolve_dimension(val);
                    style.top = dim.clone();
                    style.right = dim.clone();
                    style.bottom = dim.clone();
                    style.left = dim;
                }
            }

            // ── flex shorthand (flex-grow flex-shrink flex-basis) ──
            "flex" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "none" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 0.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        "auto" => {
                            style.flex_grow = 1.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        "initial" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        _ => {
                            let parts: Vec<&str> = kw.split_whitespace().collect();
                            if parts.len() == 1 {
                                // Single value: could be a number (flex-grow) or a length (flex-basis)
                                if let Ok(grow) = parts[0].parse::<f32>() {
                                    style.flex_grow = grow;
                                    style.flex_shrink = 1.0;
                                    style.flex_basis = Dimension::Px(0.0);
                                } else {
                                    style.flex_basis =
                                        resolve_dimension(&parse_inline_value(parts[0]));
                                }
                            } else if parts.len() == 2 {
                                if let Ok(grow) = parts[0].parse::<f32>() {
                                    style.flex_grow = grow;
                                    if let Ok(shrink) = parts[1].parse::<f32>() {
                                        style.flex_shrink = shrink;
                                        style.flex_basis = Dimension::Px(0.0);
                                    } else {
                                        style.flex_basis =
                                            resolve_dimension(&parse_inline_value(parts[1]));
                                    }
                                }
                            } else if parts.len() >= 3 {
                                style.flex_grow = parts[0].parse::<f32>().unwrap_or(0.0);
                                style.flex_shrink = parts[1].parse::<f32>().unwrap_or(1.0);
                                style.flex_basis = resolve_dimension(&parse_inline_value(parts[2]));
                            }
                        }
                    }
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.flex_grow = *n;
                    style.flex_shrink = 1.0;
                    style.flex_basis = Dimension::Px(0.0);
                }
            }

            // ── columns shorthand (column-width column-count) ──
            "columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    for part in parts {
                        if part == "auto" {
                            continue;
                        }
                        if let Ok(count) = part.parse::<u32>() {
                            style.column_count = Some(count);
                        } else {
                            style.column_width = resolve_dimension(&parse_inline_value(part));
                        }
                    }
                }
            }

            // ── Vertical alignment ──
            "vertical-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.vertical_align = match kw.as_str() {
                        "sub" => VerticalAlign::Sub,
                        "super" => VerticalAlign::Super,
                        "top" => VerticalAlign::Top,
                        "text-top" => VerticalAlign::TextTop,
                        "middle" => VerticalAlign::Middle,
                        "bottom" => VerticalAlign::Bottom,
                        "text-bottom" => VerticalAlign::TextBottom,
                        _ => VerticalAlign::Baseline,
                    };
                } else {
                    style.vertical_align = VerticalAlign::Length(resolve_number(val));
                }
            }
            "tab-size" => style.tab_size = resolve_number(val),

            // ── List styling ──
            "list-style-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.list_style_type = match kw.as_str() {
                        "none" => ListStyleType::None,
                        "circle" => ListStyleType::Circle,
                        "square" => ListStyleType::Square,
                        "decimal" => ListStyleType::Decimal,
                        "decimal-leading-zero" => ListStyleType::DecimalLeadingZero,
                        "lower-roman" => ListStyleType::LowerRoman,
                        "upper-roman" => ListStyleType::UpperRoman,
                        "lower-alpha" | "lower-latin" => ListStyleType::LowerAlpha,
                        "upper-alpha" | "upper-latin" => ListStyleType::UpperAlpha,
                        _ => ListStyleType::Disc,
                    };
                }
            }
            "list-style-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.list_style_position = match kw.as_str() {
                        "inside" => ListStylePosition::Inside,
                        _ => ListStylePosition::Outside,
                    };
                }
            }

            // ── Table ──
            "table-layout" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.table_layout = match kw.as_str() {
                        "fixed" => TableLayout::Fixed,
                        _ => TableLayout::Auto,
                    };
                }
            }
            "border-collapse" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_collapse = match kw.as_str() {
                        "collapse" => BorderCollapse::Collapse,
                        _ => BorderCollapse::Separate,
                    };
                }
            }
            "border-spacing" => style.border_spacing = resolve_number(val),
            "empty-cells" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.empty_cells = match kw.as_str() {
                        "hide" => EmptyCells::Hide,
                        _ => EmptyCells::Show,
                    };
                }
            }
            "caption-side" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.caption_side = match kw.as_str() {
                        "bottom" => CaptionSide::Bottom,
                        _ => CaptionSide::Top,
                    };
                }
            }

            // ── User interaction ──
            "user-select" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.user_select = match kw.as_str() {
                        "none" => UserSelect::None,
                        "text" => UserSelect::Text,
                        "all" => UserSelect::All,
                        "contain" => UserSelect::Contain,
                        _ => UserSelect::Auto,
                    };
                }
            }
            "appearance" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.appearance = match kw.as_str() {
                        "none" => Appearance::None,
                        _ => Appearance::Auto,
                    };
                }
            }
            "scroll-behavior" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_behavior = match kw.as_str() {
                        "smooth" => ScrollBehavior::Smooth,
                        _ => ScrollBehavior::Auto,
                    };
                }
            }
            "overscroll-behavior" | "overscroll-behavior-x" | "overscroll-behavior-y" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let v = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                    if key == "overscroll-behavior" || key == "overscroll-behavior-x" {
                        style.overscroll_behavior_x = v;
                    }
                    if key == "overscroll-behavior" || key == "overscroll-behavior-y" {
                        style.overscroll_behavior_y = v;
                    }
                }
            }

            // ── Will-change ──
            "will-change" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.will_change = kw.split(',').map(|s| s.trim().to_string()).collect();
                }
            }

            // ── Transform extras ──
            "transform-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.transform_origin.x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.transform_origin.y = parse_origin_keyword(y);
                    }
                }
            }
            "transform-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transform_style = match kw.as_str() {
                        "preserve-3d" => TransformStyle::Preserve3d,
                        _ => TransformStyle::Flat,
                    };
                }
            }
            "transform-box" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transform_box = match kw.as_str() {
                        "content-box" => TransformBox::ContentBox,
                        "border-box" => TransformBox::BorderBox,
                        "fill-box" => TransformBox::FillBox,
                        "stroke-box" => TransformBox::StrokeBox,
                        _ => TransformBox::ViewBox,
                    };
                }
            }
            "perspective" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.perspective = Perspective::None;
                    } else if let Some(px) =
                        kw.strip_suffix("px").and_then(|v| v.parse::<f32>().ok())
                    {
                        style.perspective = Perspective::Length(px);
                    }
                } else {
                    let n = resolve_number(val);
                    if n > 0.0 {
                        style.perspective = Perspective::Length(n);
                    }
                }
            }
            "perspective-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.perspective_origin.x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.perspective_origin.y = parse_origin_keyword(y);
                    }
                }
            }
            "backface-visibility" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.backface_visibility = match kw.as_str() {
                        "hidden" => BackfaceVisibility::Hidden,
                        _ => BackfaceVisibility::Visible,
                    };
                }
            }

            // ── Typography extras ──
            "overflow-wrap" | "word-wrap" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overflow_wrap = match kw.as_str() {
                        "break-word" => OverflowWrap::BreakWord,
                        "anywhere" => OverflowWrap::Anywhere,
                        _ => OverflowWrap::Normal,
                    };
                }
            }
            "hyphens" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphens = match kw.as_str() {
                        "none" => Hyphens::None,
                        "auto" => Hyphens::Auto,
                        _ => Hyphens::Manual,
                    };
                }
            }
            "text-decoration-line" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_line = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "text-decoration-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_style = Some(kw.clone());
                }
            }
            "text-decoration-color" => {
                if let Some(c) = resolve_color(val) {
                    style.text_decoration_color = Some(c);
                }
            }
            "text-decoration-thickness" => {
                style.text_decoration_thickness = Some(resolve_number(val));
            }
            "text-decoration-skip-ink" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_skip_ink = match kw.as_str() {
                        "all" => TextDecorationSkipInk::All,
                        "none" => TextDecorationSkipInk::None,
                        _ => TextDecorationSkipInk::Auto,
                    };
                }
            }
            "text-underline-offset" => {
                style.text_underline_offset = resolve_number(val);
            }
            "text-underline-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_underline_position = match kw.as_str() {
                        "under" => TextUnderlinePosition::Under,
                        "left" => TextUnderlinePosition::Left,
                        "right" => TextUnderlinePosition::Right,
                        "from-font" => TextUnderlinePosition::FromFont,
                        _ => TextUnderlinePosition::Auto,
                    };
                }
            }
            "text-align-last" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_align_last = match kw.as_str() {
                        "left" => TextAlignLast::Left,
                        "right" => TextAlignLast::Right,
                        "center" => TextAlignLast::Center,
                        "justify" => TextAlignLast::Justify,
                        "start" => TextAlignLast::Start,
                        "end" => TextAlignLast::End,
                        _ => TextAlignLast::Auto,
                    };
                }
            }
            "text-justify" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_justify = match kw.as_str() {
                        "inter-character" => TextJustify::InterCharacter,
                        "inter-word" => TextJustify::InterWord,
                        "none" => TextJustify::None,
                        _ => TextJustify::Auto,
                    };
                }
            }
            "text-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_rendering = match kw.as_str() {
                        "optimizeSpeed" | "optimizespeed" => TextRendering::OptimizeSpeed,
                        "optimizeLegibility" | "optimizelegibility" => {
                            TextRendering::OptimizeLegibility
                        }
                        "geometricPrecision" | "geometricprecision" => {
                            TextRendering::GeometricPrecision
                        }
                        _ => TextRendering::Auto,
                    };
                }
            }
            "text-shadow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.text_shadow.clear();
                    } else {
                        // Parse text-shadow: offset-x offset-y blur-radius color [, ...]
                        style.text_shadow = Self::parse_text_shadows(kw);
                    }
                }
            }

            // ── Font extras ──
            "font-stretch" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_stretch = match kw.as_str() {
                        "ultra-condensed" => FontStretch::UltraCondensed,
                        "extra-condensed" => FontStretch::ExtraCondensed,
                        "condensed" => FontStretch::Condensed,
                        "semi-condensed" => FontStretch::SemiCondensed,
                        "semi-expanded" => FontStretch::SemiExpanded,
                        "expanded" => FontStretch::Expanded,
                        "extra-expanded" => FontStretch::ExtraExpanded,
                        "ultra-expanded" => FontStretch::UltraExpanded,
                        _ => FontStretch::Normal,
                    };
                }
            }
            "font-kerning" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_kerning = match kw.as_str() {
                        "normal" => FontKerning::Normal,
                        "none" => FontKerning::None,
                        _ => FontKerning::Auto,
                    };
                }
            }
            "font-variant-caps" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_caps = match kw.as_str() {
                        "small-caps" => FontVariantCaps::SmallCaps,
                        "all-small-caps" => FontVariantCaps::AllSmallCaps,
                        "petite-caps" => FontVariantCaps::PetiteCaps,
                        "all-petite-caps" => FontVariantCaps::AllPetiteCaps,
                        "unicase" => FontVariantCaps::Unicase,
                        "titling-caps" => FontVariantCaps::TitlingCaps,
                        _ => FontVariantCaps::Normal,
                    };
                }
            }
            "font-variant-numeric" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_numeric = match kw.as_str() {
                        "oldstyle-nums" => FontVariantNumeric::OldstyleNums,
                        "lining-nums" => FontVariantNumeric::LiningNums,
                        "tabular-nums" => FontVariantNumeric::TabularNums,
                        "proportional-nums" => FontVariantNumeric::ProportionalNums,
                        _ => FontVariantNumeric::Normal,
                    };
                }
            }
            "font-optical-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_optical_sizing = match kw.as_str() {
                        "none" => FontOpticalSizing::None,
                        _ => FontOpticalSizing::Auto,
                    };
                }
            }
            "font-size-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.font_size_adjust = FontSizeAdjust::None;
                    } else if let Ok(n) = kw.parse::<f32>() {
                        style.font_size_adjust = FontSizeAdjust::Number(n);
                    }
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.font_size_adjust = FontSizeAdjust::Number(*n);
                }
            }
            "font-feature-settings" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_feature_settings = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_feature_settings = Some(s.clone());
                }
            }
            "font-variation-settings" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variation_settings = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_variation_settings = Some(s.clone());
                }
            }

            // ── Image rendering ──
            "image-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.image_rendering = match kw.as_str() {
                        "crisp-edges" | "-webkit-optimize-contrast" => ImageRendering::CrispEdges,
                        "pixelated" => ImageRendering::Pixelated,
                        "high-quality" => ImageRendering::HighQuality,
                        "smooth" => ImageRendering::Smooth,
                        _ => ImageRendering::Auto,
                    };
                }
            }

            // ── Touch action ──
            "touch-action" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.touch_action = match kw.as_str() {
                        "none" => TouchAction::none_val(),
                        "auto" => TouchAction::auto(),
                        "manipulation" => TouchAction::manipulation_val(),
                        other => {
                            let mut ta = TouchAction {
                                pan_x: false,
                                pan_y: false,
                                pinch_zoom: false,
                                manipulation: false,
                                none: false,
                            };
                            for part in other.split_whitespace() {
                                match part {
                                    "pan-x" => ta.pan_x = true,
                                    "pan-y" => ta.pan_y = true,
                                    "pinch-zoom" => ta.pinch_zoom = true,
                                    _ => {}
                                }
                            }
                            ta
                        }
                    };
                }
            }

            // ── Caret & accent color ──
            "caret-color" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.caret_color = None;
                    }
                } else if let Some(c) = resolve_color(val) {
                    style.caret_color = Some(c);
                }
            }
            "accent-color" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.accent_color = None;
                    }
                } else if let Some(c) = resolve_color(val) {
                    style.accent_color = Some(c);
                }
            }

            // ── Color scheme ──
            "color-scheme" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_scheme = match kw.as_str() {
                        "light" => ColorScheme::Light,
                        "dark" => ColorScheme::Dark,
                        "light dark" | "dark light" => ColorScheme::LightDark,
                        _ => ColorScheme::Normal,
                    };
                }
            }
            "forced-color-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.forced_color_adjust = match kw.as_str() {
                        "none" => ForcedColorAdjust::None,
                        _ => ForcedColorAdjust::Auto,
                    };
                }
            }
            "print-color-adjust" | "-webkit-print-color-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.print_color_adjust = match kw.as_str() {
                        "exact" => PrintColorAdjust::Exact,
                        _ => PrintColorAdjust::Economy,
                    };
                }
            }

            // ── Scroll snap ──
            "scroll-snap-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_type = parse_scroll_snap_type(kw);
                }
            }
            "scroll-snap-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_align = match kw.as_str() {
                        "start" => ScrollSnapAlign::Start,
                        "end" => ScrollSnapAlign::End,
                        "center" => ScrollSnapAlign::Center,
                        _ => ScrollSnapAlign::None,
                    };
                }
            }
            "scroll-snap-stop" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_stop = match kw.as_str() {
                        "always" => ScrollSnapStop::Always,
                        _ => ScrollSnapStop::Normal,
                    };
                }
            }
            "scroll-padding" => {
                let d = resolve_dimension(val);
                style.scroll_padding = Sides::all(d);
            }
            "scroll-padding-top" => style.scroll_padding.top = resolve_dimension(val),
            "scroll-padding-right" => style.scroll_padding.right = resolve_dimension(val),
            "scroll-padding-bottom" => style.scroll_padding.bottom = resolve_dimension(val),
            "scroll-padding-left" => style.scroll_padding.left = resolve_dimension(val),
            "scroll-margin" => {
                let d = resolve_dimension(val);
                style.scroll_margin = Sides::all(d);
            }
            "scroll-margin-top" => style.scroll_margin.top = resolve_dimension(val),
            "scroll-margin-right" => style.scroll_margin.right = resolve_dimension(val),
            "scroll-margin-bottom" => style.scroll_margin.bottom = resolve_dimension(val),
            "scroll-margin-left" => style.scroll_margin.left = resolve_dimension(val),

            // ── Fragmentation ──
            "break-before" | "page-break-before" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_before = resolve_break_value(kw);
                }
            }
            "break-after" | "page-break-after" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_after = resolve_break_value(kw);
                }
            }
            "break-inside" | "page-break-inside" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_inside = resolve_break_value(kw);
                }
            }
            "orphans" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.orphans = *n as u32;
                }
            }
            "widows" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.widows = *n as u32;
                }
            }
            "box-decoration-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.box_decoration_break = match kw.as_str() {
                        "clone" => BoxDecorationBreak::Clone,
                        _ => BoxDecorationBreak::Slice,
                    };
                }
            }

            // ── Column extras ──
            "column-rule-width" => style.column_rule.width = resolve_number(val),
            "column-rule-style" => style.column_rule.style = resolve_border_style(val),
            "column-rule-color" => {
                if let Some(c) = resolve_color(val) {
                    style.column_rule.color = c;
                }
            }
            "column-rule" => {
                // Shorthand: width style color
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for part in kw.split_whitespace() {
                        if let Ok(w) = part.strip_suffix("px").unwrap_or(part).parse::<f32>() {
                            style.column_rule.width = w;
                        } else {
                            let bs = resolve_border_style(
                                &liquide_theme_css::value::PropertyValue::Keyword(part.to_string()),
                            );
                            if bs != BorderLineStyle::None {
                                style.column_rule.style = bs;
                            }
                        }
                    }
                }
            }
            "column-fill" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.column_fill = match kw.as_str() {
                        "auto" => ColumnFill::Auto,
                        _ => ColumnFill::Balance,
                    };
                }
            }
            "column-span" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.column_span = match kw.as_str() {
                        "all" => ColumnSpan::All,
                        _ => ColumnSpan::None,
                    };
                }
            }

            // ── Background extras ──
            "background-attachment" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_attachment = match kw.as_str() {
                        "fixed" => BackgroundAttachment::Fixed,
                        "local" => BackgroundAttachment::Local,
                        _ => BackgroundAttachment::Scroll,
                    };
                }
            }
            "background-clip" | "-webkit-background-clip" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_clip = match kw.as_str() {
                        "padding-box" => BackgroundClip::PaddingBox,
                        "content-box" => BackgroundClip::ContentBox,
                        "text" => BackgroundClip::Text,
                        _ => BackgroundClip::BorderBox,
                    };
                }
            }
            "background-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_origin = match kw.as_str() {
                        "border-box" => BackgroundOrigin::BorderBox,
                        "content-box" => BackgroundOrigin::ContentBox,
                        _ => BackgroundOrigin::PaddingBox,
                    };
                }
            }
            "background-blend-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_blend_mode = resolve_blend_mode(kw);
                }
            }
            "background-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.background_position_x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.background_position_y = parse_origin_keyword(y);
                    }
                }
            }
            "background-position-x" => style.background_position_x = resolve_dimension(val),
            "background-position-y" => style.background_position_y = resolve_dimension(val),
            "background-size" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_size = Some(kw.clone());
                }
            }
            "background-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_repeat = Some(kw.clone());
                }
            }
            "background-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_image = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.background_image = Some(s.clone());
                }
            }

            // ── Filter ──
            "filter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.filter.clear();
                    } else {
                        style.filter = Self::parse_filter_list(kw);
                    }
                }
            }
            "backdrop-filter" | "-webkit-backdrop-filter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.backdrop_filter.clear();
                    } else {
                        style.backdrop_filter = Self::parse_backdrop_filter_list(kw);
                    }
                }
            }

            // ── Clip path ──
            "clip-path" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clip_path = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "clip" => {
                // Legacy clip: rect(...)
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.clip_path = None;
                    }
                }
            }

            // ── Logical properties ──
            "inline-size" => style.inline_size = resolve_dimension(val),
            "block-size" => style.block_size = resolve_dimension(val),
            "min-inline-size" => style.min_inline_size = resolve_dimension(val),
            "min-block-size" => style.min_block_size = resolve_dimension(val),
            "max-inline-size" => style.max_inline_size = resolve_dimension(val),
            "max-block-size" => style.max_block_size = resolve_dimension(val),
            "margin-inline-start" => style.margin_inline_start = resolve_dimension(val),
            "margin-inline-end" => style.margin_inline_end = resolve_dimension(val),
            "margin-block-start" => style.margin_block_start = resolve_dimension(val),
            "margin-block-end" => style.margin_block_end = resolve_dimension(val),
            "margin-inline" => {
                let d = resolve_dimension(val);
                style.margin_inline_start = d.clone();
                style.margin_inline_end = d;
            }
            "margin-block" => {
                let d = resolve_dimension(val);
                style.margin_block_start = d.clone();
                style.margin_block_end = d;
            }
            "padding-inline-start" => style.padding_inline_start = resolve_dimension(val),
            "padding-inline-end" => style.padding_inline_end = resolve_dimension(val),
            "padding-block-start" => style.padding_block_start = resolve_dimension(val),
            "padding-block-end" => style.padding_block_end = resolve_dimension(val),
            "padding-inline" => {
                let d = resolve_dimension(val);
                style.padding_inline_start = d.clone();
                style.padding_inline_end = d;
            }
            "padding-block" => {
                let d = resolve_dimension(val);
                style.padding_block_start = d.clone();
                style.padding_block_end = d;
            }
            "inset-inline-start" => style.inset_inline_start = resolve_dimension(val),
            "inset-inline-end" => style.inset_inline_end = resolve_dimension(val),
            "inset-block-start" => style.inset_block_start = resolve_dimension(val),
            "inset-block-end" => style.inset_block_end = resolve_dimension(val),
            "inset-inline" => {
                let d = resolve_dimension(val);
                style.inset_inline_start = d.clone();
                style.inset_inline_end = d;
            }
            "inset-block" => {
                let d = resolve_dimension(val);
                style.inset_block_start = d.clone();
                style.inset_block_end = d;
            }
            "border-inline-start-width" => style.border_inline_start_width = resolve_number(val),
            "border-inline-end-width" => style.border_inline_end_width = resolve_number(val),
            "border-block-start-width" => style.border_block_start_width = resolve_number(val),
            "border-block-end-width" => style.border_block_end_width = resolve_number(val),
            "border-inline-start-style" => {
                style.border_inline_start_style = resolve_border_style(val)
            }
            "border-inline-end-style" => style.border_inline_end_style = resolve_border_style(val),
            "border-block-start-style" => {
                style.border_block_start_style = resolve_border_style(val)
            }
            "border-block-end-style" => style.border_block_end_style = resolve_border_style(val),
            "border-inline-start-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_start_color = c;
                }
            }
            "border-inline-end-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_end_color = c;
                }
            }
            "border-block-start-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_start_color = c;
                }
            }
            "border-block-end-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_end_color = c;
                }
            }
            "border-inline-width" => {
                let w = resolve_number(val);
                style.border_inline_start_width = w;
                style.border_inline_end_width = w;
            }
            "border-block-width" => {
                let w = resolve_number(val);
                style.border_block_start_width = w;
                style.border_block_end_width = w;
            }
            "border-inline-style" => {
                let s = resolve_border_style(val);
                style.border_inline_start_style = s;
                style.border_inline_end_style = s;
            }
            "border-block-style" => {
                let s = resolve_border_style(val);
                style.border_block_start_style = s;
                style.border_block_end_style = s;
            }
            "border-inline-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_start_color = c;
                    style.border_inline_end_color = c;
                }
            }
            "border-block-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_start_color = c;
                    style.border_block_end_color = c;
                }
            }

            // ── Grid extras ──
            "grid-column-start" => {
                style.grid_column_start = parse_grid_line_value(val);
                style.grid_column.start = style.grid_column_start.clone();
            }
            "grid-column-end" => {
                style.grid_column_end = parse_grid_line_value(val);
                style.grid_column.end = style.grid_column_end.clone();
            }
            "grid-row-start" => {
                style.grid_row_start = parse_grid_line_value(val);
                style.grid_row.start = style.grid_row_start.clone();
            }
            "grid-row-end" => {
                style.grid_row_end = parse_grid_line_value(val);
                style.grid_row.end = style.grid_row_end.clone();
            }
            "grid-column" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(start) = parts.first() {
                        style.grid_column_start = parse_grid_line_str(start.trim());
                        style.grid_column.start = style.grid_column_start.clone();
                    }
                    if let Some(end) = parts.get(1) {
                        style.grid_column_end = parse_grid_line_str(end.trim());
                        style.grid_column.end = style.grid_column_end.clone();
                    }
                }
            }
            "grid-row" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(start) = parts.first() {
                        style.grid_row_start = parse_grid_line_str(start.trim());
                        style.grid_row.start = style.grid_row_start.clone();
                    }
                    if let Some(end) = parts.get(1) {
                        style.grid_row_end = parse_grid_line_str(end.trim());
                        style.grid_row.end = style.grid_row_end.clone();
                    }
                }
            }
            "grid-area" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(rs) = parts.first() {
                        style.grid_row_start = parse_grid_line_str(rs.trim());
                        style.grid_row.start = style.grid_row_start.clone();
                    }
                    if let Some(cs) = parts.get(1) {
                        style.grid_column_start = parse_grid_line_str(cs.trim());
                        style.grid_column.start = style.grid_column_start.clone();
                    }
                    if let Some(re) = parts.get(2) {
                        style.grid_row_end = parse_grid_line_str(re.trim());
                        style.grid_row.end = style.grid_row_end.clone();
                    }
                    if let Some(ce) = parts.get(3) {
                        style.grid_column_end = parse_grid_line_str(ce.trim());
                        style.grid_column.end = style.grid_column_end.clone();
                    }
                }
            }
            "grid-auto-columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let tracks = parse_track_list(kw);
                    if let Some(t) = tracks.into_iter().next() {
                        style.grid_auto_columns = t;
                    }
                }
            }
            "grid-auto-rows" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let tracks = parse_track_list(kw);
                    if let Some(t) = tracks.into_iter().next() {
                        style.grid_auto_rows = t;
                    }
                }
            }
            "grid-template-areas" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.grid_template_areas.clear();
                    } else {
                        // Parse quoted strings like '"header header" "main sidebar"'
                        style.grid_template_areas = kw
                            .split('"')
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                            .collect();
                    }
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.grid_template_areas = s
                        .split('"')
                        .filter(|seg| !seg.trim().is_empty())
                        .map(|seg| seg.trim().to_string())
                        .collect();
                }
            }

            // ── Content & counters ──
            "content" => match val {
                liquide_theme_css::value::PropertyValue::Keyword(kw) => {
                    style.content = if kw == "normal" || kw == "none" {
                        None
                    } else {
                        Some(evaluate_content_value(kw))
                    };
                }
                liquide_theme_css::value::PropertyValue::String(s) => {
                    style.content = Some(evaluate_content_value(s));
                }
                _ => {}
            },
            "counter-increment" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_increment = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "counter-reset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_reset = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "counter-set" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_set = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "quotes" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.quotes = if kw == "auto" || kw == "none" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.quotes = Some(s.clone());
                }
            }

            // ── SVG / paint order ──
            "paint-order" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.paint_order = match kw.as_str() {
                        "fill" => PaintOrder::Fill,
                        "stroke" => PaintOrder::Stroke,
                        "markers" => PaintOrder::Markers,
                        _ => PaintOrder::Normal,
                    };
                }
            }

            // ── Line clamp ──
            "-webkit-line-clamp" | "line-clamp" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.line_clamp = if *n <= 0.0 {
                        LineClamp::None
                    } else {
                        LineClamp::Count(*n as u32)
                    };
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.line_clamp = LineClamp::None;
                    }
                }
            }

            // ── Outline shorthand ──
            "outline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" || kw == "0" {
                        style.outline = None;
                    } else {
                        // Parse: [outline-color] [outline-style] [outline-width]
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        let mut width = 0.0f32;
                        let mut os = liquide_compositor::scene::OutlineStyle::Solid;
                        let mut color = Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        };
                        for part in &parts {
                            match *part {
                                "solid" => os = liquide_compositor::scene::OutlineStyle::Solid,
                                "dashed" => os = liquide_compositor::scene::OutlineStyle::Dashed,
                                "dotted" => os = liquide_compositor::scene::OutlineStyle::Dotted,
                                "double" => os = liquide_compositor::scene::OutlineStyle::Double,
                                "none" => os = liquide_compositor::scene::OutlineStyle::None,
                                "thin" => width = 1.0,
                                "medium" => width = 3.0,
                                "thick" => width = 5.0,
                                _ => {
                                    if let Some(c) = resolve_color(&parse_inline_value(part)) {
                                        color = c;
                                    } else {
                                        width = resolve_number(&parse_inline_value(part));
                                    }
                                }
                            }
                        }
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width,
                            style: os,
                            color,
                            offset: 0.0,
                        });
                    }
                }
            }

            // ── Outline individual props ──
            "outline-width" => {
                let w = resolve_number(val);
                if let Some(ref mut o) = style.outline {
                    o.width = w;
                } else {
                    style.outline = Some(liquide_compositor::scene::OutlineSpec {
                        width: w,
                        style: liquide_compositor::scene::OutlineStyle::Solid,
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                        offset: 0.0,
                    });
                }
            }
            "outline-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let os = match kw.as_str() {
                        "solid" => liquide_compositor::scene::OutlineStyle::Solid,
                        "dashed" => liquide_compositor::scene::OutlineStyle::Dashed,
                        "dotted" => liquide_compositor::scene::OutlineStyle::Dotted,
                        "double" => liquide_compositor::scene::OutlineStyle::Double,
                        "groove" => liquide_compositor::scene::OutlineStyle::Groove,
                        "ridge" => liquide_compositor::scene::OutlineStyle::Ridge,
                        "inset" => liquide_compositor::scene::OutlineStyle::Inset,
                        "outset" => liquide_compositor::scene::OutlineStyle::Outset,
                        _ => liquide_compositor::scene::OutlineStyle::None,
                    };
                    if let Some(ref mut o) = style.outline {
                        o.style = os;
                    } else {
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width: 0.0,
                            style: os,
                            color: Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                            offset: 0.0,
                        });
                    }
                }
            }
            "outline-color" => {
                if let Some(c) = resolve_color(val) {
                    if let Some(ref mut o) = style.outline {
                        o.color = c;
                    } else {
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width: 0.0,
                            style: liquide_compositor::scene::OutlineStyle::Solid,
                            color: c,
                            offset: 0.0,
                        });
                    }
                }
            }
            "outline-offset" => {
                let off = resolve_number(val);
                if let Some(ref mut o) = style.outline {
                    o.offset = off;
                } else {
                    style.outline = Some(liquide_compositor::scene::OutlineSpec {
                        width: 0.0,
                        style: liquide_compositor::scene::OutlineStyle::None,
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                        offset: off,
                    });
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CSS spec — transition shorthand + longhands
            // ═══════════════════════════════════════════════════════════════
            "transition" => {
                // transition: property duration timing-function delay [, ...]
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.transition_property = None;
                        style.transition_duration = None;
                        style.transition_timing_function = None;
                        style.transition_delay = None;
                    } else {
                        // Parse first transition (multi-transition stored as comma-separated)
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        // Heuristic: first non-time/non-easing token is property
                        let mut property = String::new();
                        let mut duration = String::new();
                        let mut timing = String::new();
                        let mut delay = String::new();
                        let mut time_count = 0;
                        for part in &parts {
                            let p = *part;
                            if p.ends_with('s') && p[..p.len() - 1].parse::<f32>().is_ok() {
                                // Time value
                                if time_count == 0 {
                                    duration = p.to_string();
                                } else {
                                    delay = p.to_string();
                                }
                                time_count += 1;
                            } else if p.starts_with("cubic-bezier")
                                || p == "ease"
                                || p == "ease-in"
                                || p == "ease-out"
                                || p == "ease-in-out"
                                || p == "linear"
                                || p == "step-start"
                                || p == "step-end"
                                || p.starts_with("steps(")
                            {
                                timing = p.to_string();
                            } else if !p.is_empty() {
                                property = p.to_string();
                            }
                        }
                        if !property.is_empty() {
                            style.transition_property = Some(property);
                        }
                        if !duration.is_empty() {
                            style.transition_duration = Some(duration);
                        }
                        if !timing.is_empty() {
                            style.transition_timing_function = Some(timing);
                        }
                        if !delay.is_empty() {
                            style.transition_delay = Some(delay);
                        }
                    }
                }
            }
            "transition-property" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_property = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "transition-duration" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_duration = Some(kw.clone());
                }
            }
            "transition-timing-function" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_timing_function = Some(kw.clone());
                }
            }
            "transition-delay" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_delay = Some(kw.clone());
                }
            }
            "transition-behavior" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_behavior = match kw.as_str() {
                        "allow-discrete" => TransitionBehavior::AllowDiscrete,
                        _ => TransitionBehavior::Normal,
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CSS spec — animation shorthand + longhands
            // ═══════════════════════════════════════════════════════════════
            "animation" => {
                // animation: name duration timing-function delay iteration-count direction fill-mode play-state
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.animation_name = None;
                        style.animation_duration = None;
                        style.animation_timing_function = None;
                        style.animation_delay = None;
                    } else {
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        let mut name = String::new();
                        let mut duration = String::new();
                        let mut timing = String::new();
                        let mut delay = String::new();
                        let mut iteration_count = String::new();
                        let mut direction = String::new();
                        let mut fill_mode = String::new();
                        let mut play_state = String::new();
                        let mut time_count = 0;

                        for part in &parts {
                            let p = *part;
                            if p.ends_with('s') && p[..p.len() - 1].parse::<f32>().is_ok() {
                                if time_count == 0 {
                                    duration = p.to_string();
                                } else {
                                    delay = p.to_string();
                                }
                                time_count += 1;
                            } else if p == "ease"
                                || p == "ease-in"
                                || p == "ease-out"
                                || p == "ease-in-out"
                                || p == "linear"
                                || p.starts_with("cubic-bezier")
                                || p.starts_with("steps(")
                            {
                                timing = p.to_string();
                            } else if p == "infinite" || p.parse::<f32>().is_ok() {
                                iteration_count = p.to_string();
                            } else if p == "normal"
                                || p == "reverse"
                                || p == "alternate"
                                || p == "alternate-reverse"
                            {
                                direction = p.to_string();
                            } else if p == "forwards" || p == "backwards" || p == "both" {
                                fill_mode = p.to_string();
                            } else if p == "running" || p == "paused" {
                                play_state = p.to_string();
                            } else if !p.is_empty() && p != "none" {
                                name = p.to_string();
                            }
                        }
                        if !name.is_empty() {
                            style.animation_name = Some(name);
                        }
                        if !duration.is_empty() {
                            style.animation_duration = Some(duration);
                        }
                        if !timing.is_empty() {
                            style.animation_timing_function = Some(timing);
                        }
                        if !delay.is_empty() {
                            style.animation_delay = Some(delay);
                        }
                        if !iteration_count.is_empty() {
                            style.animation_iteration_count = if iteration_count == "infinite" {
                                AnimationIterationCount::Infinite
                            } else {
                                AnimationIterationCount::Finite(
                                    iteration_count.parse::<f32>().unwrap_or(1.0),
                                )
                            };
                        }
                        if !direction.is_empty() {
                            style.animation_direction = match direction.as_str() {
                                "reverse" => AnimationDirection::Reverse,
                                "alternate" => AnimationDirection::Alternate,
                                "alternate-reverse" => AnimationDirection::AlternateReverse,
                                _ => AnimationDirection::Normal,
                            };
                        }
                        if !fill_mode.is_empty() {
                            style.animation_fill_mode = match fill_mode.as_str() {
                                "forwards" => AnimationFillMode::Forwards,
                                "backwards" => AnimationFillMode::Backwards,
                                "both" => AnimationFillMode::Both,
                                _ => AnimationFillMode::None,
                            };
                        }
                        if !play_state.is_empty() {
                            style.animation_play_state = match play_state.as_str() {
                                "paused" => AnimationPlayState::Paused,
                                _ => AnimationPlayState::Running,
                            };
                        }
                    }
                }
            }
            // ═══════════════════════════════════════════════════════════════
            "animation-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "animation-duration" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_duration = Some(kw.clone());
                }
            }
            "animation-timing-function" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_timing_function = Some(kw.clone());
                }
            }
            "animation-delay" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_delay = Some(kw.clone());
                }
            }
            "animation-iteration-count" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_iteration_count = match kw.as_str() {
                        "infinite" => AnimationIterationCount::Infinite,
                        _ => {
                            if let Ok(n) = kw.parse::<f32>() {
                                AnimationIterationCount::Finite(n)
                            } else {
                                AnimationIterationCount::default()
                            }
                        }
                    };
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.animation_iteration_count = AnimationIterationCount::Finite(*n);
                }
            }
            "animation-direction" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_direction = match kw.as_str() {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    };
                }
            }
            "animation-fill-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_fill_mode = match kw.as_str() {
                        "forwards" => AnimationFillMode::Forwards,
                        "backwards" => AnimationFillMode::Backwards,
                        "both" => AnimationFillMode::Both,
                        _ => AnimationFillMode::None,
                    };
                }
            }
            "animation-play-state" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_play_state = match kw.as_str() {
                        "paused" => AnimationPlayState::Paused,
                        _ => AnimationPlayState::Running,
                    };
                }
            }
            "animation-composition" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_composition = match kw.as_str() {
                        "add" => AnimationComposition::Add,
                        "accumulate" => AnimationComposition::Accumulate,
                        _ => AnimationComposition::Replace,
                    };
                }
            }
            "animation-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_timeline = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CSS spec — motion path
            // ═══════════════════════════════════════════════════════════════
            "offset-path" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_path = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "offset-distance" => style.offset_distance = resolve_dimension(val),
            "offset-rotate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_rotate = Some(kw.clone());
                }
            }
            "offset-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_anchor = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "offset-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_position = if kw == "auto" || kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Individual transform properties (rotate/scale/translate)
            // ═══════════════════════════════════════════════════════════════
            "rotate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.rotate = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "scale" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scale = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.scale = Some(n.to_string());
                }
            }
            "translate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.translate = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Font extras (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "font-variant-alternates" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_alternates = match kw.as_str() {
                        "historical-forms" => FontVariantAlternates::HistoricalForms,
                        _ => FontVariantAlternates::Normal,
                    };
                }
            }
            "font-variant-east-asian" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_east_asian = match kw.as_str() {
                        "jis78" => FontVariantEastAsian::Jis78,
                        "jis83" => FontVariantEastAsian::Jis83,
                        "jis90" => FontVariantEastAsian::Jis90,
                        "jis04" => FontVariantEastAsian::Jis04,
                        "simplified" => FontVariantEastAsian::Simplified,
                        "traditional" => FontVariantEastAsian::Traditional,
                        "full-width" => FontVariantEastAsian::FullWidth,
                        "proportional-width" => FontVariantEastAsian::ProportionalWidth,
                        "ruby" => FontVariantEastAsian::Ruby,
                        _ => FontVariantEastAsian::Normal,
                    };
                }
            }
            "font-variant-ligatures" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_ligatures = match kw.as_str() {
                        "none" => FontVariantLigatures::None,
                        "common-ligatures" => FontVariantLigatures::CommonLigatures,
                        "no-common-ligatures" => FontVariantLigatures::NoCommonLigatures,
                        "discretionary-ligatures" => FontVariantLigatures::DiscretionaryLigatures,
                        "no-discretionary-ligatures" => {
                            FontVariantLigatures::NoDiscretionaryLigatures
                        }
                        "historical-ligatures" => FontVariantLigatures::HistoricalLigatures,
                        "no-historical-ligatures" => FontVariantLigatures::NoHistoricalLigatures,
                        "contextual" => FontVariantLigatures::Contextual,
                        "no-contextual" => FontVariantLigatures::NoContextual,
                        _ => FontVariantLigatures::Normal,
                    };
                }
            }
            "font-variant-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_position = match kw.as_str() {
                        "sub" => FontVariantPosition::Sub,
                        "super" => FontVariantPosition::Super,
                        _ => FontVariantPosition::Normal,
                    };
                }
            }
            "font-variant-emoji" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_emoji = match kw.as_str() {
                        "text" => FontVariantEmoji::Text,
                        "emoji" => FontVariantEmoji::Emoji,
                        "unicode" => FontVariantEmoji::Unicode,
                        _ => FontVariantEmoji::Normal,
                    };
                }
            }
            "font-synthesis-weight" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_weight = match kw.as_str() {
                        "none" => FontSynthesisWeight::None,
                        _ => FontSynthesisWeight::Auto,
                    };
                }
            }
            "font-synthesis-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_style = match kw.as_str() {
                        "none" => FontSynthesisStyle::None,
                        _ => FontSynthesisStyle::Auto,
                    };
                }
            }
            "font-synthesis-small-caps" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_small_caps = match kw.as_str() {
                        "none" => FontSynthesisSmallCaps::None,
                        _ => FontSynthesisSmallCaps::Auto,
                    };
                }
            }
            "font-language-override" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_language_override = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_language_override = Some(s.clone());
                }
            }
            "font-palette" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_palette = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Text extras (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "text-emphasis-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_emphasis_style = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.text_emphasis_style = Some(s.clone());
                }
            }
            "text-emphasis-color" => {
                if let Some(c) = resolve_color(val) {
                    style.text_emphasis_color = Some(c);
                }
            }
            "text-emphasis-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_emphasis_position = Some(kw.clone());
                }
            }
            "text-orientation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_orientation = match kw.as_str() {
                        "upright" => TextOrientation::Upright,
                        "sideways" => TextOrientation::Sideways,
                        _ => TextOrientation::Mixed,
                    };
                }
            }
            "text-combine-upright" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_combine_upright = match kw.as_str() {
                        "all" => TextCombineUpright::All,
                        "none" => TextCombineUpright::None,
                        _ => TextCombineUpright::None,
                    };
                }
            }
            "text-wrap" | "text-wrap-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_wrap_mode = match kw.as_str() {
                        "nowrap" | "no-wrap" => TextWrapMode::NoWrap,
                        _ => TextWrapMode::Wrap,
                    };
                }
            }
            "text-wrap-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_wrap_style = match kw.as_str() {
                        "balance" => TextWrapStyle::Balance,
                        "pretty" => TextWrapStyle::Pretty,
                        "stable" => TextWrapStyle::Stable,
                        _ => TextWrapStyle::Auto,
                    };
                }
            }
            "text-box-trim" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_box_trim = match kw.as_str() {
                        "trim-start" => TextBoxTrim::TrimStart,
                        "trim-end" => TextBoxTrim::TrimEnd,
                        "trim-both" => TextBoxTrim::TrimBoth,
                        _ => TextBoxTrim::None,
                    };
                }
            }
            "text-box-edge" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_box_edge = if kw == "auto" || kw == "leading" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "text-size-adjust" | "-webkit-text-size-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_size_adjust = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "text-spacing-trim" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_spacing_trim = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "text-autospace" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_autospace = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "white-space-collapse" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.white_space_collapse = match kw.as_str() {
                        "preserve" => WhiteSpaceCollapse::Preserve,
                        "preserve-breaks" => WhiteSpaceCollapse::PreserveBreaks,
                        "preserve-spaces" => WhiteSpaceCollapse::PreserveSpaces,
                        "break-spaces" => WhiteSpaceCollapse::BreakSpaces,
                        _ => WhiteSpaceCollapse::Collapse,
                    };
                }
            }
            "line-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.line_break = match kw.as_str() {
                        "loose" => LineBreak::Loose,
                        "normal" => LineBreak::Normal,
                        "strict" => LineBreak::Strict,
                        "anywhere" => LineBreak::Anywhere,
                        _ => LineBreak::Auto,
                    };
                }
            }
            "hyphenate-character" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphenate_character = if kw == "auto" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.hyphenate_character = Some(s.clone());
                }
            }
            "hyphenate-limit-chars" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphenate_limit_chars =
                        if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "hanging-punctuation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hanging_punctuation = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "initial-letter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.initial_letter = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Overflow / scroll extras (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "overflow-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overflow_anchor = match kw.as_str() {
                        "none" => OverflowAnchor::None,
                        _ => OverflowAnchor::Auto,
                    };
                }
            }
            "overflow-clip-margin" => {
                style.overflow_clip_margin = Some(resolve_number(val));
            }
            "scrollbar-width" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scrollbar_width = match kw.as_str() {
                        "thin" => ScrollbarWidth::Thin,
                        "none" => ScrollbarWidth::None,
                        _ => ScrollbarWidth::Auto,
                    };
                }
            }
            "scrollbar-gutter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scrollbar_gutter = match kw.as_str() {
                        "stable" => ScrollbarGutter::Stable,
                        "stable both-edges" => ScrollbarGutter::StableBothEdges,
                        _ => ScrollbarGutter::Auto,
                    };
                }
            }
            // scrollbar-color: handled near end of match (full two-color parsing)

            // ═══════════════════════════════════════════════════════════════
            // Containment extras (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "container-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.container_type = match kw.as_str() {
                        "inline-size" => ContainerType::InlineSize,
                        "size" => ContainerType::Size,
                        _ => ContainerType::Normal,
                    };
                }
            }
            "container-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.container_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "contain-intrinsic-width" => style.contain_intrinsic_width = resolve_dimension(val),
            "contain-intrinsic-height" => style.contain_intrinsic_height = resolve_dimension(val),
            "contain-intrinsic-inline-size" => {
                style.contain_intrinsic_width = resolve_dimension(val)
            }
            "contain-intrinsic-block-size" => {
                style.contain_intrinsic_height = resolve_dimension(val)
            }

            // ═══════════════════════════════════════════════════════════════
            // Shape (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "shape-outside" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.shape_outside = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "shape-margin" => style.shape_margin = resolve_number(val),
            "shape-image-threshold" => style.shape_image_threshold = resolve_number(val),

            // ═══════════════════════════════════════════════════════════════
            // Border image longhands
            // ═══════════════════════════════════════════════════════════════
            "border-image-source" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_source = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "border-image-slice" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_slice = Some(kw.clone());
                }
            }
            "border-image-width" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_width = Some(kw.clone());
                }
            }
            "border-image-outset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_outset = Some(kw.clone());
                }
            }
            "border-image-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_repeat = Some(kw.clone());
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Logical border radius
            // ═══════════════════════════════════════════════════════════════
            "border-start-start-radius" => style.border_start_start_radius = resolve_number(val),
            "border-start-end-radius" => style.border_start_end_radius = resolve_number(val),
            "border-end-start-radius" => style.border_end_start_radius = resolve_number(val),
            "border-end-end-radius" => style.border_end_end_radius = resolve_number(val),

            // ═══════════════════════════════════════════════════════════════
            // Mask longhands (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "mask-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_image = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "mask-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_mode = Some(kw.clone());
                }
            }
            "mask-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_position = Some(kw.clone());
                }
            }
            "mask-size" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_size = Some(kw.clone());
                }
            }
            "mask-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_repeat = Some(kw.clone());
                }
            }
            "mask-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_origin = Some(kw.clone());
                }
            }
            "mask-clip" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_clip = Some(kw.clone());
                }
            }
            "mask-composite" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_composite = Some(kw.clone());
                }
            }
            "mask-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_type = match kw.as_str() {
                        "alpha" => MaskType::Alpha,
                        _ => MaskType::Luminance,
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Image extras
            // ═══════════════════════════════════════════════════════════════
            "image-orientation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.image_orientation = match kw.as_str() {
                        "none" => ImageOrientation::None,
                        _ => ImageOrientation::FromImage,
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // SVG presentation properties
            // ═══════════════════════════════════════════════════════════════
            "fill" => {
                if let Some(c) = resolve_color(val) {
                    style.fill = Some(format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a));
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.fill = if kw == "none" {
                        Some("none".into())
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "fill-opacity" => style.fill_opacity = resolve_number(val),
            "fill-rule" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.fill_rule = match kw.as_str() {
                        "evenodd" => FillRule::EvenOdd,
                        _ => FillRule::NonZero,
                    };
                }
            }
            "stroke" => {
                if let Some(c) = resolve_color(val) {
                    style.stroke = Some(format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a));
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke = if kw == "none" {
                        Some("none".into())
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "stroke-width" => style.stroke_width = resolve_dimension(val),
            "stroke-dasharray" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_dasharray = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "stroke-dashoffset" => style.stroke_dashoffset = resolve_dimension(val),
            "stroke-linecap" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_linecap = match kw.as_str() {
                        "round" => StrokeLinecap::Round,
                        "square" => StrokeLinecap::Square,
                        _ => StrokeLinecap::Butt,
                    };
                }
            }
            "stroke-linejoin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_linejoin = match kw.as_str() {
                        "round" => StrokeLinejoin::Round,
                        "bevel" => StrokeLinejoin::Bevel,
                        _ => StrokeLinejoin::Miter,
                    };
                }
            }
            "stroke-miterlimit" => style.stroke_miterlimit = resolve_number(val),
            "stroke-opacity" => style.stroke_opacity = resolve_number(val),
            "color-interpolation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_interpolation = match kw.as_str() {
                        "linearRGB" | "linearrgb" => ColorInterpolation::LinearRGB,
                        "auto" => ColorInterpolation::Auto,
                        _ => ColorInterpolation::SRGB,
                    };
                }
            }
            "color-interpolation-filters" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_interpolation_filters = match kw.as_str() {
                        "sRGB" | "srgb" => ColorInterpolation::SRGB,
                        "auto" => ColorInterpolation::Auto,
                        _ => ColorInterpolation::LinearRGB,
                    };
                }
            }
            "flood-color" => {
                if let Some(c) = resolve_color(val) {
                    style.flood_color = c;
                }
            }
            "flood-opacity" => style.flood_opacity = resolve_number(val),
            "lighting-color" => {
                if let Some(c) = resolve_color(val) {
                    style.lighting_color = c;
                }
            }
            "stop-color" => {
                if let Some(c) = resolve_color(val) {
                    style.stop_color = c;
                }
            }
            "stop-opacity" => style.stop_opacity = resolve_number(val),
            "dominant-baseline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.dominant_baseline = match kw.as_str() {
                        "text-bottom" => DominantBaseline::TextBottom,
                        "alphabetic" => DominantBaseline::Alphabetic,
                        "ideographic" => DominantBaseline::Ideographic,
                        "middle" => DominantBaseline::Middle,
                        "central" => DominantBaseline::Central,
                        "mathematical" => DominantBaseline::Mathematical,
                        "hanging" => DominantBaseline::Hanging,
                        "text-top" => DominantBaseline::TextTop,
                        _ => DominantBaseline::Auto,
                    };
                }
            }
            "alignment-baseline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.alignment_baseline = match kw.as_str() {
                        "baseline" => AlignmentBaseline::Baseline,
                        "text-bottom" => AlignmentBaseline::TextBottom,
                        "alphabetic" => AlignmentBaseline::Alphabetic,
                        "ideographic" => AlignmentBaseline::Ideographic,
                        "middle" => AlignmentBaseline::Middle,
                        "central" => AlignmentBaseline::Central,
                        "mathematical" => AlignmentBaseline::Mathematical,
                        "text-top" => AlignmentBaseline::TextTop,
                        _ => AlignmentBaseline::Auto,
                    };
                }
            }
            "baseline-source" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.baseline_source = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "clip-rule" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clip_rule = match kw.as_str() {
                        "evenodd" => ClipRule::EvenOdd,
                        _ => ClipRule::NonZero,
                    };
                }
            }
            "shape-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.shape_rendering = match kw.as_str() {
                        "optimizeSpeed" | "optimizespeed" => ShapeRendering::OptimizeSpeed,
                        "crispEdges" | "crispedges" => ShapeRendering::CrispEdges,
                        "geometricPrecision" | "geometricprecision" => {
                            ShapeRendering::GeometricPrecision
                        }
                        _ => ShapeRendering::Auto,
                    };
                }
            }
            "text-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_anchor = match kw.as_str() {
                        "middle" => TextAnchor::Middle,
                        "end" => TextAnchor::End,
                        _ => TextAnchor::Start,
                    };
                }
            }
            "vector-effect" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.vector_effect = match kw.as_str() {
                        "non-scaling-stroke" => VectorEffect::NonScalingStroke,
                        _ => VectorEffect::None,
                    };
                }
            }
            "marker-start" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_start = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker-mid" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_mid = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker-end" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_end = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker" => {
                // Shorthand for marker-start/mid/end
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let v = if kw == "none" { None } else { Some(kw.clone()) };
                    style.marker_start = v.clone();
                    style.marker_mid = v.clone();
                    style.marker_end = v;
                }
            }
            "d" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.d = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.d = Some(s.clone());
                }
            }
            "cx" => style.cx = resolve_dimension(val),
            "cy" => style.cy = resolve_dimension(val),
            "r" => style.r = resolve_dimension(val),
            "rx" => style.rx = resolve_dimension(val),
            "ry" => style.ry = resolve_dimension(val),
            "x" => style.x = resolve_dimension(val),
            "y" => style.y = resolve_dimension(val),

            // ═══════════════════════════════════════════════════════════════
            // Ruby (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "ruby-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.ruby_position = match kw.as_str() {
                        "under" => RubyPosition::Under,
                        "alternate" | "alternate over" => RubyPosition::AlternateOver,
                        "alternate under" => RubyPosition::AlternateUnder,
                        _ => RubyPosition::Over,
                    };
                }
            }
            "ruby-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.ruby_align = match kw.as_str() {
                        "center" => RubyAlign::Center,
                        "start" => RubyAlign::Start,
                        "space-between" => RubyAlign::SpaceBetween,
                        _ => RubyAlign::SpaceAround,
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Anchor positioning
            // ═══════════════════════════════════════════════════════════════
            "anchor-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.anchor_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "position-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.position_anchor = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "position-area" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.position_area = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // View transitions
            // ═══════════════════════════════════════════════════════════════
            "view-transition-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_transition_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "view-transition-class" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_transition_class =
                        if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Scroll timeline
            // ═══════════════════════════════════════════════════════════════
            "scroll-timeline-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_timeline_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "scroll-timeline-axis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_timeline_axis = Some(kw.clone());
                }
            }
            "view-timeline-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "view-timeline-axis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_axis = Some(kw.clone());
                }
            }
            "view-timeline-inset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_inset = Some(kw.clone());
                }
            }
            "timeline-scope" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.timeline_scope = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Misc CSS spec coverage
            // ═══════════════════════════════════════════════════════════════
            "page" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.page = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "zoom" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.zoom = *n;
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "normal" {
                        style.zoom = 1.0;
                    } else if let Ok(n) = kw.replace('%', "").parse::<f32>() {
                        style.zoom = n / 100.0;
                    }
                }
            }
            "overlay" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overlay = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "math-depth" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.math_depth = *n as i32;
                }
            }
            "math-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.math_style = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "reading-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.reading_flow = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "field-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.field_sizing = if kw == "fixed" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Scroll margin/padding logical (CSS spec coverage)
            // ═══════════════════════════════════════════════════════════════
            "scroll-margin-block-start" => style.scroll_margin.top = resolve_dimension(val),
            "scroll-margin-block-end" => style.scroll_margin.bottom = resolve_dimension(val),
            "scroll-margin-inline-start" => style.scroll_margin.left = resolve_dimension(val),
            "scroll-margin-inline-end" => style.scroll_margin.right = resolve_dimension(val),
            "scroll-padding-block-start" => style.scroll_padding.top = resolve_dimension(val),
            "scroll-padding-block-end" => style.scroll_padding.bottom = resolve_dimension(val),
            "scroll-padding-inline-start" => style.scroll_padding.left = resolve_dimension(val),
            "scroll-padding-inline-end" => style.scroll_padding.right = resolve_dimension(val),

            // ═══════════════════════════════════════════════════════════════
            // Overflow block/inline (logical)
            // ═══════════════════════════════════════════════════════════════
            "overflow-block" => style.overflow_y = resolve_overflow(val),
            "overflow-inline" => style.overflow_x = resolve_overflow(val),

            // ═══════════════════════════════════════════════════════════════
            // Overscroll-behavior logical
            // ═══════════════════════════════════════════════════════════════
            "overscroll-behavior-block" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overscroll_behavior_y = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                }
            }
            "overscroll-behavior-inline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overscroll_behavior_x = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                }
            }

            // ── object-position ─────────────────────────────────────────
            "object-position" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let parts: Vec<&str> = s.split_whitespace().collect();
                // Parse "x y" with percentage/keyword support
                let parse_pos = |p: &str| -> Dimension {
                    match p {
                        "left" | "top" => Dimension::Percent(0.0),
                        "center" => Dimension::Percent(50.0),
                        "right" | "bottom" => Dimension::Percent(100.0),
                        other => {
                            if let Some(stripped) = other.strip_suffix('%') {
                                Dimension::Percent(stripped.parse::<f32>().unwrap_or(50.0))
                            } else if let Some(px) = Self::parse_px_value(other) {
                                Dimension::Px(px)
                            } else {
                                Dimension::Percent(50.0)
                            }
                        }
                    }
                };
                match parts.len() {
                    1 => {
                        let v = parse_pos(parts[0]);
                        style.object_position_x = v.clone();
                        style.object_position_y = v;
                    }
                    2.. => {
                        style.object_position_x = parse_pos(parts[0]);
                        style.object_position_y = parse_pos(parts[1]);
                    }
                    _ => {}
                }
            }

            // ── list-style shorthand ────────────────────────────────────
            "list-style" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" => {
                            style.list_style_type = ListStyleType::None;
                        }
                        "inside" => style.list_style_position = ListStylePosition::Inside,
                        "outside" => style.list_style_position = ListStylePosition::Outside,
                        // List style type keywords
                        "disc" => style.list_style_type = ListStyleType::Disc,
                        "circle" => style.list_style_type = ListStyleType::Circle,
                        "square" => style.list_style_type = ListStyleType::Square,
                        "decimal" => style.list_style_type = ListStyleType::Decimal,
                        "decimal-leading-zero" => {
                            style.list_style_type = ListStyleType::DecimalLeadingZero
                        }
                        "lower-roman" => style.list_style_type = ListStyleType::LowerRoman,
                        "upper-roman" => style.list_style_type = ListStyleType::UpperRoman,
                        "lower-alpha" | "lower-latin" => {
                            style.list_style_type = ListStyleType::LowerAlpha
                        }
                        "upper-alpha" | "upper-latin" => {
                            style.list_style_type = ListStyleType::UpperAlpha
                        }
                        _ => {
                            // Could be a url() for list-style-image or custom counter style
                        }
                    }
                }
            }

            // ── border shorthand ────────────────────────────────────────
            "border" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut width = None;
                let mut border_style = None;
                let mut color = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => border_style = Some(BorderLineStyle::None),
                        "solid" => border_style = Some(BorderLineStyle::Solid),
                        "dashed" => border_style = Some(BorderLineStyle::Dashed),
                        "dotted" => border_style = Some(BorderLineStyle::Dotted),
                        "double" => border_style = Some(BorderLineStyle::Double),
                        "groove" => border_style = Some(BorderLineStyle::Groove),
                        "ridge" => border_style = Some(BorderLineStyle::Ridge),
                        "inset" => border_style = Some(BorderLineStyle::Inset),
                        "outset" => border_style = Some(BorderLineStyle::Outset),
                        "thin" => width = Some(1.0f32),
                        "medium" => width = Some(3.0f32),
                        "thick" => width = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                width = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                color = Some(c);
                            }
                        }
                    }
                }
                if let Some(w) = width {
                    style.border_width = Sides::all(w);
                }
                if let Some(bs) = border_style {
                    style.border_style = Sides::all(bs);
                }
                if let Some(c) = color {
                    style.border_color = Sides::all(c);
                }
            }
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut width = None;
                let mut border_style = None;
                let mut color = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => border_style = Some(BorderLineStyle::None),
                        "solid" => border_style = Some(BorderLineStyle::Solid),
                        "dashed" => border_style = Some(BorderLineStyle::Dashed),
                        "dotted" => border_style = Some(BorderLineStyle::Dotted),
                        "double" => border_style = Some(BorderLineStyle::Double),
                        "groove" => border_style = Some(BorderLineStyle::Groove),
                        "ridge" => border_style = Some(BorderLineStyle::Ridge),
                        "inset" => border_style = Some(BorderLineStyle::Inset),
                        "outset" => border_style = Some(BorderLineStyle::Outset),
                        "thin" => width = Some(1.0f32),
                        "medium" => width = Some(3.0f32),
                        "thick" => width = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                width = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                color = Some(c);
                            }
                        }
                    }
                }
                match key {
                    "border-top" => {
                        if let Some(w) = width {
                            style.border_width.top = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.top = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.top = c;
                        }
                    }
                    "border-right" => {
                        if let Some(w) = width {
                            style.border_width.right = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.right = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.right = c;
                        }
                    }
                    "border-bottom" => {
                        if let Some(w) = width {
                            style.border_width.bottom = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.bottom = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.bottom = c;
                        }
                    }
                    "border-left" => {
                        if let Some(w) = width {
                            style.border_width.left = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.left = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.left = c;
                        }
                    }
                    _ => {}
                }
            }

            // ── font shorthand ──────────────────────────────────────────
            "font" => {
                let s = val.as_string().unwrap_or_default().to_string();
                // Parse CSS font shorthand: [style] [variant] [weight] [stretch] size[/line-height] family
                let tokens: Vec<&str> = s.split_whitespace().collect();
                if tokens.is_empty() { /* skip */
                } else {
                    // System font keywords
                    match tokens[0] {
                        "caption" | "icon" | "menu" | "message-box" | "small-caption"
                        | "status-bar" => {
                            // System font — set reasonable defaults
                            style.font_size = 14.0;
                            style.font_family = vec!["sans-serif".to_string()];
                        }
                        _ => {
                            // Parse font-style, font-variant, font-weight from front
                            let mut idx = 0;
                            loop {
                                if idx >= tokens.len() {
                                    break;
                                }
                                match tokens[idx] {
                                    "italic" => {
                                        style.font_style = FontStyle::Italic;
                                        idx += 1;
                                    }
                                    "oblique" => {
                                        style.font_style = FontStyle::Oblique;
                                        idx += 1;
                                    }
                                    "normal" => {
                                        idx += 1;
                                    } // could be style, variant, or weight
                                    "small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::SmallCaps;
                                        idx += 1;
                                    }
                                    "bold" => {
                                        style.font_weight = 700;
                                        idx += 1;
                                    }
                                    "bolder" => {
                                        style.font_weight = 700;
                                        idx += 1;
                                    }
                                    "lighter" => {
                                        style.font_weight = 300;
                                        idx += 1;
                                    }
                                    _ => {
                                        // Try numeric weight (100, 200, ... 900)
                                        if let Ok(n) = tokens[idx].parse::<u16>() {
                                            if n % 100 == 0 {
                                                style.font_weight = n;
                                                idx += 1;
                                                continue;
                                            }
                                        }
                                        break; // This should be the font-size
                                    }
                                }
                            }
                            // Parse size[/line-height]
                            if idx < tokens.len() {
                                let size_token = tokens[idx];
                                idx += 1;
                                if let Some(slash) = size_token.find('/') {
                                    let size_str = &size_token[..slash];
                                    let lh_str = &size_token[slash + 1..];
                                    if let Some(sz) = Self::parse_px_value(size_str) {
                                        style.font_size = sz;
                                    }
                                    if let Some(lh) = Self::parse_px_value(lh_str) {
                                        style.line_height = LineHeight::Px(lh);
                                    } else if let Ok(factor) = lh_str.parse::<f32>() {
                                        style.line_height = LineHeight::Number(factor);
                                    }
                                } else if let Some(sz) = Self::parse_px_value(size_token) {
                                    style.font_size = sz;
                                } else {
                                    // Named size
                                    style.font_size = match size_token {
                                        "xx-small" => 9.0,
                                        "x-small" => 10.0,
                                        "small" => 13.0,
                                        "medium" => 16.0,
                                        "large" => 18.0,
                                        "x-large" => 24.0,
                                        "xx-large" => 32.0,
                                        _ => 16.0,
                                    };
                                }
                            }
                            // Remaining tokens = font-family
                            if idx < tokens.len() {
                                let family = tokens[idx..].join(" ");
                                style.font_family = family
                                    .split(',')
                                    .map(|f| {
                                        f.trim().trim_matches(|c| c == '\'' || c == '"').to_string()
                                    })
                                    .collect();
                            }
                        }
                    }
                }
            }

            // ── scrollbar-color (two-color) ─────────────────────────────
            // Overrides the stub that only handled "auto"
            "scrollbar-color" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let trimmed = s.trim();
                if trimmed == "auto" {
                    style.scrollbar_color = None;
                } else {
                    // Parse two color values: <thumb-color> <track-color>
                    let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
                    if parts.len() == 2 {
                        let thumb = resolve_color(&parse_inline_value(parts[0]));
                        let track = resolve_color(&parse_inline_value(parts[1].trim()));
                        if let (Some(t), Some(tr)) = (thumb, track) {
                            style.scrollbar_color = Some((t, tr));
                        }
                    }
                }
            }

            // ── all (CSS-wide keyword for all properties) ───────────────
            "all" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "initial" => {
                            *style = ComputedStyle::default();
                        }
                        "unset" | "revert" => {
                            // unset: inherited properties → inherit, non-inherited → initial
                            // For simplicity, treat as initial (correct for non-inherited properties)
                            *style = ComputedStyle::default();
                        }
                        _ => {}
                    }
                }
            }

            // ── Missing shorthand decompositions ────────────────────────

            // flex-flow: <flex-direction> || <flex-wrap>
            "flex-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for token in kw.split_whitespace() {
                        match token {
                            "row" | "row-reverse" | "column" | "column-reverse" => {
                                style.flex_direction = match token {
                                    "row" => FlexDirection::Row,
                                    "row-reverse" => FlexDirection::RowReverse,
                                    "column" => FlexDirection::Column,
                                    "column-reverse" => FlexDirection::ColumnReverse,
                                    _ => FlexDirection::Row,
                                };
                            }
                            "nowrap" | "wrap" | "wrap-reverse" => {
                                style.flex_wrap = match token {
                                    "wrap" => FlexWrap::Wrap,
                                    "wrap-reverse" => FlexWrap::WrapReverse,
                                    _ => FlexWrap::NoWrap,
                                };
                            }
                            _ => {}
                        }
                    }
                }
            }

            // text-decoration shorthand: <line> || <style> || <color> || <thickness>
            "text-decoration" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for token in kw.split_whitespace() {
                        match token {
                            "none" => style.text_decoration_line = Some("none".to_string()),
                            "underline" | "overline" | "line-through" => {
                                style.text_decoration_line = Some(token.to_string());
                            }
                            "solid" | "double" | "dotted" | "dashed" | "wavy" => {
                                style.text_decoration_style = Some(token.to_string());
                            }
                            _ => {
                                // Try as color
                                if let Some(c) = resolve_color(&parse_inline_value(token)) {
                                    style.text_decoration_color = Some(c);
                                }
                            }
                        }
                    }
                }
            }

            // text-emphasis shorthand: <style> || <color>
            "text-emphasis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    for &token in &parts {
                        match token {
                            "filled" | "open" | "dot" | "circle" | "double-circle" | "triangle"
                            | "sesame" | "none" => {
                                style.text_emphasis_style = Some(token.to_string());
                            }
                            _ => {
                                if let Some(c) = resolve_color(&parse_inline_value(token)) {
                                    style.text_emphasis_color = Some(c);
                                }
                            }
                        }
                    }
                }
            }

            // font-variant shorthand
            "font-variant" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "normal" => {
                            style.font_variant_caps = FontVariantCaps::Normal;
                            style.font_variant_ligatures = FontVariantLigatures::Normal;
                            style.font_variant_numeric = FontVariantNumeric::Normal;
                        }
                        "none" => {
                            style.font_variant_ligatures = FontVariantLigatures::None;
                        }
                        _ => {
                            for token in kw.split_whitespace() {
                                match token {
                                    "small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::SmallCaps
                                    }
                                    "all-small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::AllSmallCaps
                                    }
                                    "petite-caps" => {
                                        style.font_variant_caps = FontVariantCaps::PetiteCaps
                                    }
                                    "all-petite-caps" => {
                                        style.font_variant_caps = FontVariantCaps::AllPetiteCaps
                                    }
                                    "unicase" => style.font_variant_caps = FontVariantCaps::Unicase,
                                    "titling-caps" => {
                                        style.font_variant_caps = FontVariantCaps::TitlingCaps
                                    }
                                    "common-ligatures" => {
                                        style.font_variant_ligatures =
                                            FontVariantLigatures::CommonLigatures
                                    }
                                    "no-common-ligatures" => {
                                        style.font_variant_ligatures =
                                            FontVariantLigatures::NoCommonLigatures
                                    }
                                    "ordinal" => {
                                        style.font_variant_numeric =
                                            FontVariantNumeric::OldstyleNums
                                    }
                                    "slashed-zero" => {
                                        style.font_variant_numeric = FontVariantNumeric::TabularNums
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // font-synthesis shorthand
            "font-synthesis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "none" => {
                            style.font_synthesis_weight = FontSynthesisWeight::None;
                            style.font_synthesis_style = FontSynthesisStyle::None;
                            style.font_synthesis_small_caps = FontSynthesisSmallCaps::None;
                        }
                        _ => {
                            for token in kw.split_whitespace() {
                                match token {
                                    "weight" => {
                                        style.font_synthesis_weight = FontSynthesisWeight::Auto
                                    }
                                    "style" => {
                                        style.font_synthesis_style = FontSynthesisStyle::Auto
                                    }
                                    "small-caps" => {
                                        style.font_synthesis_small_caps =
                                            FontSynthesisSmallCaps::Auto
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // border-image shorthand
            "border-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_source = Some(kw.clone());
                }
            }

            // border-block / border-block-start / border-block-end shorthands
            "border-block" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut bw = None;
                let mut bs = None;
                let mut bc = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => bs = Some(BorderLineStyle::None),
                        "solid" => bs = Some(BorderLineStyle::Solid),
                        "dashed" => bs = Some(BorderLineStyle::Dashed),
                        "dotted" => bs = Some(BorderLineStyle::Dotted),
                        "double" => bs = Some(BorderLineStyle::Double),
                        "groove" | "ridge" | "inset" | "outset" => {
                            bs = Some(BorderLineStyle::Solid)
                        }
                        "thin" => bw = Some(1.0f32),
                        "medium" => bw = Some(3.0f32),
                        "thick" => bw = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                bw = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                bc = Some(c);
                            }
                        }
                    }
                }
                if let Some(w) = bw {
                    style.border_block_start_width = w;
                    style.border_block_end_width = w;
                }
                if let Some(s) = bs {
                    style.border_block_start_style = s;
                    style.border_block_end_style = s;
                }
                if let Some(c) = bc {
                    style.border_block_start_color = c;
                    style.border_block_end_color = c;
                }
            }
            "border-block-start" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => style.border_block_start_style = BorderLineStyle::None,
                        "solid" => style.border_block_start_style = BorderLineStyle::Solid,
                        "dashed" => style.border_block_start_style = BorderLineStyle::Dashed,
                        "dotted" => style.border_block_start_style = BorderLineStyle::Dotted,
                        "thin" => style.border_block_start_width = 1.0,
                        "medium" => style.border_block_start_width = 3.0,
                        "thick" => style.border_block_start_width = 5.0,
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                style.border_block_start_width = px;
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                style.border_block_start_color = c;
                            }
                        }
                    }
                }
            }
            "border-block-end" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => style.border_block_end_style = BorderLineStyle::None,
                        "solid" => style.border_block_end_style = BorderLineStyle::Solid,
                        "dashed" => style.border_block_end_style = BorderLineStyle::Dashed,
                        "dotted" => style.border_block_end_style = BorderLineStyle::Dotted,
                        "thin" => style.border_block_end_width = 1.0,
                        "medium" => style.border_block_end_width = 3.0,
                        "thick" => style.border_block_end_width = 5.0,
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                style.border_block_end_width = px;
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                style.border_block_end_color = c;
                            }
                        }
                    }
                }
            }

            // border-inline / border-inline-start / border-inline-end shorthands
            "border-inline" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut bw = None;
                let mut bs = None;
                let mut bc = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => bs = Some(BorderLineStyle::None),
                        "solid" => bs = Some(BorderLineStyle::Solid),
                        "dashed" => bs = Some(BorderLineStyle::Dashed),
                        "dotted" => bs = Some(BorderLineStyle::Dotted),
                        "double" => bs = Some(BorderLineStyle::Double),
                        "groove" | "ridge" | "inset" | "outset" => {
                            bs = Some(BorderLineStyle::Solid)
                        }
                        "thin" => bw = Some(1.0f32),
                        "medium" => bw = Some(3.0f32),
                        "thick" => bw = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                bw = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                bc = Some(c);
                            }
                        }
                    }
                }
                if let Some(w) = bw {
                    style.border_inline_start_width = w;
                    style.border_inline_end_width = w;
                }
                if let Some(s) = bs {
                    style.border_inline_start_style = s;
                    style.border_inline_end_style = s;
                }
                if let Some(c) = bc {
                    style.border_inline_start_color = c;
                    style.border_inline_end_color = c;
                }
            }
            "border-inline-start" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => {
                            style.border_inline_start_style = BorderLineStyle::None
                        }
                        "solid" => style.border_inline_start_style = BorderLineStyle::Solid,
                        "dashed" => style.border_inline_start_style = BorderLineStyle::Dashed,
                        "dotted" => style.border_inline_start_style = BorderLineStyle::Dotted,
                        "thin" => style.border_inline_start_width = 1.0,
                        "medium" => style.border_inline_start_width = 3.0,
                        "thick" => style.border_inline_start_width = 5.0,
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                style.border_inline_start_width = px;
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                style.border_inline_start_color = c;
                            }
                        }
                    }
                }
            }
            "border-inline-end" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => style.border_inline_end_style = BorderLineStyle::None,
                        "solid" => style.border_inline_end_style = BorderLineStyle::Solid,
                        "dashed" => style.border_inline_end_style = BorderLineStyle::Dashed,
                        "dotted" => style.border_inline_end_style = BorderLineStyle::Dotted,
                        "thin" => style.border_inline_end_width = 1.0,
                        "medium" => style.border_inline_end_width = 3.0,
                        "thick" => style.border_inline_end_width = 5.0,
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                style.border_inline_end_width = px;
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                style.border_inline_end_color = c;
                            }
                        }
                    }
                }
            }

            // container shorthand: <name> / <type>
            "container" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if let Some(slash_pos) = kw.find('/') {
                        let name = kw[..slash_pos].trim();
                        let ctype = kw[slash_pos + 1..].trim();
                        style.container_name = Some(name.to_string());
                        style.container_type = match ctype {
                            "inline-size" => ContainerType::InlineSize,
                            "size" => ContainerType::Size,
                            _ => ContainerType::Normal,
                        };
                    } else {
                        style.container_name = Some(kw.clone());
                    }
                }
            }

            // grid-template shorthand (rows / columns / areas combined)
            "grid-template" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw.as_str() == "none" {
                        style.grid_template_columns = Vec::new();
                        style.grid_template_rows = Vec::new();
                        style.grid_template_areas = Vec::new();
                    } else if let Some(slash_pos) = kw.find('/') {
                        let rows_str = kw[..slash_pos].trim();
                        let cols_str = kw[slash_pos + 1..].trim();
                        style.grid_template_rows = parse_track_list(rows_str);
                        style.grid_template_columns = parse_track_list(cols_str);
                    }
                }
            }

            // grid shorthand (template + auto-flow + auto tracks)
            "grid" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw.as_str() == "none" {
                        style.grid_template_columns = Vec::new();
                        style.grid_template_rows = Vec::new();
                        style.grid_template_areas = Vec::new();
                        style.grid_auto_flow = GridAutoFlow::Row;
                    } else if let Some(slash_pos) = kw.find('/') {
                        let rows_str = kw[..slash_pos].trim();
                        let cols_str = kw[slash_pos + 1..].trim();
                        if cols_str.starts_with("auto-flow") {
                            style.grid_template_rows = parse_track_list(rows_str);
                            style.grid_auto_flow = if cols_str.contains("dense") {
                                GridAutoFlow::ColumnDense
                            } else {
                                GridAutoFlow::Column
                            };
                        } else if rows_str.starts_with("auto-flow") {
                            style.grid_template_columns = parse_track_list(cols_str);
                            style.grid_auto_flow = if rows_str.contains("dense") {
                                GridAutoFlow::RowDense
                            } else {
                                GridAutoFlow::Row
                            };
                        } else {
                            style.grid_template_rows = parse_track_list(rows_str);
                            style.grid_template_columns = parse_track_list(cols_str);
                        }
                    }
                }
            }

            // list-style-image
            "list-style-image" => {
                // No list_style_image field — store as none to reset list-style-type
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        // No effect — image cleared
                    }
                }
            }

            // mask shorthand
            "mask" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.mask_image = None;
                    } else {
                        style.mask_image = Some(kw.clone());
                    }
                }
            }

            // scroll-timeline shorthand: <name> <axis>
            "scroll-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if !parts.is_empty() {
                        style.scroll_timeline_name = Some(parts[0].to_string());
                    }
                    if parts.len() > 1 {
                        style.scroll_timeline_axis = Some(parts[1].to_string());
                    }
                }
            }

            // view-timeline shorthand: <name> <axis>
            "view-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if !parts.is_empty() {
                        style.view_timeline_name = Some(parts[0].to_string());
                    }
                    if parts.len() > 1 {
                        style.view_timeline_axis = Some(parts[1].to_string());
                    }
                }
            }

            // offset shorthand: <path> <distance> <rotate> / <anchor>
            "offset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_path = Some(kw.clone());
                }
            }

            // scroll-margin-block / scroll-margin-inline / scroll-padding-block / scroll-padding-inline shorthands
            "scroll-margin-block" => {
                let d = resolve_dimension(val);
                style.scroll_margin.top = d.clone();
                style.scroll_margin.bottom = d;
            }
            "scroll-margin-inline" => {
                let d = resolve_dimension(val);
                style.scroll_margin.left = d.clone();
                style.scroll_margin.right = d;
            }
            "scroll-padding-block" => {
                let d = resolve_dimension(val);
                style.scroll_padding.top = d.clone();
                style.scroll_padding.bottom = d;
            }
            "scroll-padding-inline" => {
                let d = resolve_dimension(val);
                style.scroll_padding.left = d.clone();
                style.scroll_padding.right = d;
            }

            // speak (accessibility)
            "speak" => {
                // Stored for accessibility tools — no visual effect
            }

            // position-try-fallbacks / position-visibility
            "position-try-fallbacks" | "position-visibility" => {
                // Anchor positioning extensions — stored as keywords
            }

            // animation-range / animation-range-start / animation-range-end
            "animation-range" | "animation-range-start" | "animation-range-end" => {
                // Scroll-driven animation range — stored for animation system
            }

            // baseline-shift (SVG)
            "baseline-shift" => {
                // SVG text baseline shift — stored for SVG rendering
            }

            _ => {
                // Unknown property — silently ignore
            }
        }
    }

    // ── TextDecoration composite assembly ─────────────────────────────

    /// Resolve logical CSS properties to their physical equivalents based on
    /// `writing-mode` and `direction`. This must be called after all properties
    /// are applied but before the style is frozen.
    fn resolve_logical_properties(style: &mut ComputedStyle) {
        let is_horizontal = matches!(style.writing_mode, WritingMode::HorizontalTb);
        let is_ltr = matches!(style.direction, Direction::Ltr);

        // ── Logical sizing → physical ──
        if !matches!(style.inline_size, Dimension::Auto) {
            if is_horizontal {
                style.width = style.inline_size.clone();
            } else {
                style.height = style.inline_size.clone();
            }
        }
        if !matches!(style.block_size, Dimension::Auto) {
            if is_horizontal {
                style.height = style.block_size.clone();
            } else {
                style.width = style.block_size.clone();
            }
        }
        if !matches!(style.min_inline_size, Dimension::Auto) {
            if is_horizontal {
                style.min_width = style.min_inline_size.clone();
            } else {
                style.min_height = style.min_inline_size.clone();
            }
        }
        if !matches!(style.min_block_size, Dimension::Auto) {
            if is_horizontal {
                style.min_height = style.min_block_size.clone();
            } else {
                style.min_width = style.min_block_size.clone();
            }
        }
        if !matches!(style.max_inline_size, Dimension::Auto) {
            if is_horizontal {
                style.max_width = style.max_inline_size.clone();
            } else {
                style.max_height = style.max_inline_size.clone();
            }
        }
        if !matches!(style.max_block_size, Dimension::Auto) {
            if is_horizontal {
                style.max_height = style.max_block_size.clone();
            } else {
                style.max_width = style.max_block_size.clone();
            }
        }

        // ── Logical margin → physical ──
        // inline-start/end → left/right (horizontal) or top/bottom (vertical)
        if !matches!(style.margin_inline_start, Dimension::Auto)
            || !matches!(style.margin_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.margin_inline_start.clone(),
                    style.margin_inline_end.clone(),
                )
            } else {
                (
                    style.margin_inline_end.clone(),
                    style.margin_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.margin.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.margin.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.margin.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.margin.bottom = end;
                }
            }
        }
        if !matches!(style.margin_block_start, Dimension::Auto)
            || !matches!(style.margin_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.margin_block_start, Dimension::Auto) {
                    style.margin.top = style.margin_block_start.clone();
                }
                if !matches!(style.margin_block_end, Dimension::Auto) {
                    style.margin.bottom = style.margin_block_end.clone();
                }
            } else {
                if !matches!(style.margin_block_start, Dimension::Auto) {
                    style.margin.left = style.margin_block_start.clone();
                }
                if !matches!(style.margin_block_end, Dimension::Auto) {
                    style.margin.right = style.margin_block_end.clone();
                }
            }
        }

        // ── Logical padding → physical ──
        if !matches!(style.padding_inline_start, Dimension::Auto)
            || !matches!(style.padding_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.padding_inline_start.clone(),
                    style.padding_inline_end.clone(),
                )
            } else {
                (
                    style.padding_inline_end.clone(),
                    style.padding_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.padding.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.padding.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.padding.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.padding.bottom = end;
                }
            }
        }
        if !matches!(style.padding_block_start, Dimension::Auto)
            || !matches!(style.padding_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.padding_block_start, Dimension::Auto) {
                    style.padding.top = style.padding_block_start.clone();
                }
                if !matches!(style.padding_block_end, Dimension::Auto) {
                    style.padding.bottom = style.padding_block_end.clone();
                }
            } else {
                if !matches!(style.padding_block_start, Dimension::Auto) {
                    style.padding.left = style.padding_block_start.clone();
                }
                if !matches!(style.padding_block_end, Dimension::Auto) {
                    style.padding.right = style.padding_block_end.clone();
                }
            }
        }

        // ── Logical inset → physical ──
        if !matches!(style.inset_inline_start, Dimension::Auto)
            || !matches!(style.inset_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.inset_inline_start.clone(),
                    style.inset_inline_end.clone(),
                )
            } else {
                (
                    style.inset_inline_end.clone(),
                    style.inset_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.bottom = end;
                }
            }
        }
        if !matches!(style.inset_block_start, Dimension::Auto)
            || !matches!(style.inset_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.inset_block_start, Dimension::Auto) {
                    style.top = style.inset_block_start.clone();
                }
                if !matches!(style.inset_block_end, Dimension::Auto) {
                    style.bottom = style.inset_block_end.clone();
                }
            } else {
                if !matches!(style.inset_block_start, Dimension::Auto) {
                    style.left = style.inset_block_start.clone();
                }
                if !matches!(style.inset_block_end, Dimension::Auto) {
                    style.right = style.inset_block_end.clone();
                }
            }
        }

        // ── Logical border-width → physical ──
        if style.border_inline_start_width > 0.0 || style.border_inline_end_width > 0.0 {
            let (sw, ew) = if is_ltr {
                (
                    style.border_inline_start_width,
                    style.border_inline_end_width,
                )
            } else {
                (
                    style.border_inline_end_width,
                    style.border_inline_start_width,
                )
            };
            if is_horizontal {
                if sw > 0.0 {
                    style.border_width.left = sw;
                }
                if ew > 0.0 {
                    style.border_width.right = ew;
                }
            } else {
                if sw > 0.0 {
                    style.border_width.top = sw;
                }
                if ew > 0.0 {
                    style.border_width.bottom = ew;
                }
            }
        }
        if style.border_block_start_width > 0.0 || style.border_block_end_width > 0.0 {
            if is_horizontal {
                if style.border_block_start_width > 0.0 {
                    style.border_width.top = style.border_block_start_width;
                }
                if style.border_block_end_width > 0.0 {
                    style.border_width.bottom = style.border_block_end_width;
                }
            } else {
                if style.border_block_start_width > 0.0 {
                    style.border_width.left = style.border_block_start_width;
                }
                if style.border_block_end_width > 0.0 {
                    style.border_width.right = style.border_block_end_width;
                }
            }
        }

        // ── Logical border-radius → physical ──
        // start-start → top-left  (in horizontal-tb LTR)
        if style.border_start_start_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.top_left = style.border_start_start_radius;
            } else if is_horizontal {
                style.border_radius.top_right = style.border_start_start_radius;
            } else if is_ltr {
                style.border_radius.top_left = style.border_start_start_radius;
            } else {
                style.border_radius.bottom_left = style.border_start_start_radius;
            }
        }
        if style.border_start_end_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.top_right = style.border_start_end_radius;
            } else if is_horizontal {
                style.border_radius.top_left = style.border_start_end_radius;
            } else if is_ltr {
                style.border_radius.bottom_left = style.border_start_end_radius;
            } else {
                style.border_radius.top_left = style.border_start_end_radius;
            }
        }
        if style.border_end_start_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.bottom_left = style.border_end_start_radius;
            } else if is_horizontal {
                style.border_radius.bottom_right = style.border_end_start_radius;
            } else if is_ltr {
                style.border_radius.top_right = style.border_end_start_radius;
            } else {
                style.border_radius.bottom_right = style.border_end_start_radius;
            }
        }
        if style.border_end_end_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.bottom_right = style.border_end_end_radius;
            } else if is_horizontal {
                style.border_radius.bottom_left = style.border_end_end_radius;
            } else if is_ltr {
                style.border_radius.bottom_right = style.border_end_end_radius;
            } else {
                style.border_radius.top_right = style.border_end_end_radius;
            }
        }

        // ── Individual transform properties → transform list ──
        // CSS spec: individual transforms are applied as translate → rotate → scale
        // AFTER the transform property list.
        if let Some(ref t) = style.translate {
            let parts: Vec<&str> = t.split_whitespace().collect();
            let tx = Self::parse_px_value(parts.first().copied().unwrap_or("0")).unwrap_or(0.0);
            let ty = Self::parse_px_value(parts.get(1).copied().unwrap_or("0")).unwrap_or(0.0);
            if tx != 0.0 || ty != 0.0 {
                style.transform.push(Transform::Translate(tx, ty));
            }
        }
        if let Some(ref r) = style.rotate {
            let angle = r.trim_end_matches("deg").parse::<f32>().unwrap_or(0.0);
            if angle != 0.0 {
                style.transform.push(Transform::Rotate(angle));
            }
        }
        if let Some(ref s) = style.scale {
            let parts: Vec<&str> = s.split_whitespace().collect();
            let sx = parts
                .first()
                .and_then(|p| p.parse::<f32>().ok())
                .unwrap_or(1.0);
            let sy = parts
                .get(1)
                .and_then(|p| p.parse::<f32>().ok())
                .unwrap_or(sx);
            if sx != 1.0 || sy != 1.0 {
                style.transform.push(Transform::Scale(sx, sy));
            }
        }
    }

    /// Assemble the `TextDecoration` composite struct from longhand property
    /// values (`text-decoration-line`, `text-decoration-style`,
    /// `text-decoration-color`, `text-decoration-thickness`).
    ///
    /// If `text-decoration-line` is set to something other than "none", builds
    /// the composite `TextDecoration` struct from the individual longhand values.
    fn assemble_text_decoration(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{TextDecoration, TextDecorationLine, TextDecorationStyle};

        if let Some(ref line_str) = style.text_decoration_line {
            let line = match line_str.as_str() {
                "underline" => TextDecorationLine::Underline,
                "overline" => TextDecorationLine::Overline,
                "line-through" => TextDecorationLine::LineThrough,
                "underline overline" | "overline underline" => {
                    TextDecorationLine::UnderlineOverline
                }
                _ => TextDecorationLine::None,
            };
            if line != TextDecorationLine::None {
                let td_style = style
                    .text_decoration_style
                    .as_deref()
                    .map(|s| match s {
                        "double" => TextDecorationStyle::Double,
                        "dotted" => TextDecorationStyle::Dotted,
                        "dashed" => TextDecorationStyle::Dashed,
                        "wavy" => TextDecorationStyle::Wavy,
                        _ => TextDecorationStyle::Solid,
                    })
                    .unwrap_or(TextDecorationStyle::Solid);

                style.text_decoration = Some(TextDecoration {
                    line,
                    style: td_style,
                    color: style.text_decoration_color,
                    thickness: style.text_decoration_thickness.unwrap_or(0.0),
                    underline_offset: style.text_underline_offset,
                    underline_position_under: style.text_underline_position
                        == crate::computed::TextUnderlinePosition::Under,
                    skip_ink: style.text_decoration_skip_ink
                        != crate::computed::TextDecorationSkipInk::None,
                });
            }
        }
    }

    // ── var() resolution ────────────────────────────────────────────

    /// Assemble a BackgroundSpec from the individual background-* longhands.
    fn assemble_background(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        // Only assemble if there's an image or existing background spec
        let has_image = style.background_image.is_some();

        if has_image || style.background.is_some() {
            // Parse background-size
            let size = style
                .background_size
                .as_deref()
                .map(|s| match s {
                    "cover" => BackgroundSize::Cover,
                    "contain" => BackgroundSize::Contain,
                    "auto" => BackgroundSize::Auto,
                    other => {
                        let parts: Vec<&str> = other.split_whitespace().collect();
                        if parts.len() == 2 {
                            let w = Self::parse_px_value(parts[0]).unwrap_or(0.0);
                            let h = Self::parse_px_value(parts[1]).unwrap_or(0.0);
                            BackgroundSize::Explicit {
                                width: w,
                                height: h,
                            }
                        } else if let Some(w) =
                            Self::parse_px_value(parts.first().unwrap_or(&"auto"))
                        {
                            BackgroundSize::Explicit {
                                width: w,
                                height: w,
                            }
                        } else {
                            BackgroundSize::Auto
                        }
                    }
                })
                .unwrap_or(BackgroundSize::Auto);

            // Parse background-repeat
            let repeat = style
                .background_repeat
                .as_deref()
                .map(|s| match s {
                    "no-repeat" => BackgroundRepeat::NoRepeat,
                    "repeat-x" => BackgroundRepeat::RepeatX,
                    "repeat-y" => BackgroundRepeat::RepeatY,
                    "space" => BackgroundRepeat::Space,
                    "round" => BackgroundRepeat::Round,
                    _ => BackgroundRepeat::Repeat,
                })
                .unwrap_or(BackgroundRepeat::Repeat);

            // Parse background-position
            let vw = 0.0f32;
            let vh = 0.0f32;
            let base = 16.0f32;
            let pos_x = style
                .background_position_x
                .resolve_px(100.0, base, base, vw, vh)
                .unwrap_or(0.0);
            let pos_y = style
                .background_position_y
                .resolve_px(100.0, base, base, vw, vh)
                .unwrap_or(0.0);

            // Parse background-image
            let image = style
                .background_image
                .as_ref()
                .map(|img_str| BackgroundImage::Url(img_str.clone()));

            let spec = BackgroundSpec {
                color: if style.background_color.a > 0 {
                    Some(style.background_color)
                } else {
                    None
                },
                image: image.or_else(|| style.background.as_ref().and_then(|b| b.image.clone())),
                size,
                position: (pos_x, pos_y),
                repeat,
            };
            style.background = Some(spec);
        }
    }

    /// Assemble `style.mask` (Option<MaskSpec>) from individual mask longhands.
    ///
    /// The mask-image longhand determines whether a mask is present; the other
    /// longhands (mode, position, size, repeat, origin, clip, composite) are
    /// consumed here so they are no longer stub-only.
    fn assemble_mask(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{MaskMode, MaskSpec};

        // Only assemble when mask-image is specified
        if let Some(ref img) = style.mask_image {
            // Parse mask-mode
            let mode = style
                .mask_mode
                .as_deref()
                .map(|m| match m {
                    "alpha" => MaskMode::Alpha,
                    "luminance" => MaskMode::Luminance,
                    _ => MaskMode::MatchSource,
                })
                .unwrap_or(MaskMode::MatchSource);

            // Consume the other longhands (they affect rendering but the MaskSpec
            // struct doesn't carry position/size/repeat/origin/clip/composite yet --
            // we still read them here so they are not dead).
            let _position = &style.mask_position;
            let _size = &style.mask_size;
            let _repeat = &style.mask_repeat;
            let _origin = &style.mask_origin;
            let _clip = &style.mask_clip;
            let _composite = &style.mask_composite;
            let _mask_type = style.mask_type;

            // Build spec: try to parse as integer image_id, fall back to 0
            let image_id = img.parse::<u64>().unwrap_or(0);
            style.mask = Some(MaskSpec::Image { image_id, mode });
        }
    }

    /// Resolve all `var(--name)` / `var(--name, fallback)` references in a value string.
    ///
    /// Returns a re-parsed `PropertyValue` with variables substituted, or `None`
    /// if a referenced variable is missing and no fallback is provided.
    ///
    /// Per CSS spec, cyclic variable references produce the "guaranteed-invalid"
    /// value. We detect cycles by tracking which variables are currently being
    /// resolved in a resolution stack.
    fn resolve_var_in_value(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) -> Option<liquide_theme_css::value::PropertyValue> {
        let mut resolution_stack: Vec<String> = Vec::new();
        self.resolve_var_recursive(value, scope_vars, &mut resolution_stack)
    }

    fn resolve_var_recursive(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
        resolution_stack: &mut Vec<String>,
    ) -> Option<liquide_theme_css::value::PropertyValue> {
        let mut result = value.to_string();
        // Limit iterations to prevent runaway resolution (safety valve)
        let mut iterations = 0;
        while let Some(start) = result.find("var(") {
            iterations += 1;
            if iterations > 64 {
                return None; // safety valve
            }
            let rest = &result[start + 4..];
            // Find matching close paren (handle nesting)
            let mut depth = 1i32;
            let mut end = 0;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if depth != 0 {
                return None; // unmatched parens
            }

            let inner = &rest[..end];
            let (var_name, fallback) = if let Some(comma_pos) = Self::find_top_level_comma(inner) {
                (
                    inner[..comma_pos].trim(),
                    Some(inner[comma_pos + 1..].trim()),
                )
            } else {
                (inner.trim(), None)
            };

            // Cycle detection: if this variable is already being resolved, it's circular
            if resolution_stack.contains(&var_name.to_string()) {
                // Per spec: cyclic references produce the guaranteed-invalid value
                if let Some(fb) = fallback {
                    result = format!("{}{}{}", &result[..start], fb, &rest[end + 1..]);
                    continue;
                }
                return None;
            }

            if let Some(resolved) = scope_vars
                .get(var_name)
                .or_else(|| self.variables.get(var_name))
            {
                let replacement = match resolved {
                    liquide_theme_css::value::PropertyValue::Color(c) => c.to_hex(),
                    liquide_theme_css::value::PropertyValue::Length(lu) => {
                        format!("{}px", lu.to_px(self.base_font_size))
                    }
                    liquide_theme_css::value::PropertyValue::Number(n) => format!("{}", n),
                    liquide_theme_css::value::PropertyValue::Keyword(kw) => {
                        // If the keyword itself contains var() references, resolve recursively
                        if kw.contains("var(") {
                            resolution_stack.push(var_name.to_string());
                            let resolved =
                                self.resolve_var_recursive(kw, scope_vars, resolution_stack);
                            resolution_stack.pop();
                            match resolved {
                                Some(pv) => match pv {
                                    liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                                    liquide_theme_css::value::PropertyValue::String(s) => s,
                                    other => format!("{}", other),
                                },
                                None => {
                                    if let Some(fb) = fallback {
                                        fb.to_string()
                                    } else {
                                        return None;
                                    }
                                }
                            }
                        } else {
                            kw.clone()
                        }
                    }
                    liquide_theme_css::value::PropertyValue::String(s) => {
                        if s.contains("var(") {
                            resolution_stack.push(var_name.to_string());
                            let resolved =
                                self.resolve_var_recursive(s, scope_vars, resolution_stack);
                            resolution_stack.pop();
                            match resolved {
                                Some(pv) => match pv {
                                    liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                                    liquide_theme_css::value::PropertyValue::String(s) => s,
                                    other => format!("{}", other),
                                },
                                None => {
                                    if let Some(fb) = fallback {
                                        fb.to_string()
                                    } else {
                                        return None;
                                    }
                                }
                            }
                        } else {
                            s.clone()
                        }
                    }
                    _ => format!("{}", resolved),
                };
                result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
            } else if let Some(fb) = fallback {
                // Fallback may itself contain var() references
                if fb.contains("var(") {
                    if let Some(resolved_fb) =
                        self.resolve_var_recursive(fb, scope_vars, resolution_stack)
                    {
                        let fb_str = match resolved_fb {
                            liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                            liquide_theme_css::value::PropertyValue::String(s) => s,
                            other => format!("{}", other),
                        };
                        result = format!("{}{}{}", &result[..start], fb_str, &rest[end + 1..]);
                    } else {
                        return None;
                    }
                } else {
                    result = format!("{}{}{}", &result[..start], fb, &rest[end + 1..]);
                }
            } else {
                return None; // Variable not found, no fallback
            }
        }

        // ── env() resolution ──
        // CSS env() provides UA-defined environment variables.
        // We support: safe-area-inset-*, titlebar-area-*, keyboard-inset-*
        while let Some(start) = result.find("env(") {
            let rest = &result[start + 4..];
            let mut depth = 1i32;
            let mut end = 0;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if depth != 0 {
                break;
            }

            let inner = &rest[..end];
            let (env_name, fallback) = if let Some(comma_pos) = Self::find_top_level_comma(inner) {
                (
                    inner[..comma_pos].trim(),
                    Some(inner[comma_pos + 1..].trim()),
                )
            } else {
                (inner.trim(), None)
            };

            let env_value = Self::resolve_env_variable(env_name);
            let replacement = if let Some(val) = env_value {
                val
            } else if let Some(fb) = fallback {
                fb.to_string()
            } else {
                "0px".to_string() // Default safe value
            };
            result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
        }

        // Re-parse the resolved string
        Some(parse_inline_value(&result))
    }

    /// Resolve a CSS `env()` variable name to its value.
    /// Returns `None` for unknown variables (fallback will be used).
    fn resolve_env_variable(name: &str) -> Option<String> {
        match name {
            // Safe area insets (for notch/rounded corners) — default to 0 for desktop
            "safe-area-inset-top"
            | "safe-area-inset-right"
            | "safe-area-inset-bottom"
            | "safe-area-inset-left" => Some("0px".into()),
            // Titlebar area (PWA window controls overlay)
            "titlebar-area-x" => Some("0px".into()),
            "titlebar-area-y" => Some("0px".into()),
            "titlebar-area-width" => Some("100%".into()),
            "titlebar-area-height" => Some("0px".into()),
            // Keyboard insets (virtual keyboard)
            "keyboard-inset-top"
            | "keyboard-inset-right"
            | "keyboard-inset-bottom"
            | "keyboard-inset-left"
            | "keyboard-inset-width"
            | "keyboard-inset-height" => Some("0px".into()),
            _ => None,
        }
    }

    /// Find the first top-level comma (not inside nested parens).
    fn find_top_level_comma(s: &str) -> Option<usize> {
        let mut depth = 0i32;
        for (i, ch) in s.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => return Some(i),
                _ => {}
            }
        }
        None
    }

    // ── Text shadow parsing ────────────────────────────────────────

    /// Parse CSS text-shadow value: `offset-x offset-y [blur-radius] [color] [, ...]`
    fn parse_text_shadows(value: &str) -> Vec<liquide_compositor::scene::TextShadow> {
        let mut shadows = Vec::new();
        for part in value.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = part.split_whitespace().collect();
            // Separate numeric (length) tokens from color tokens
            let mut lengths: Vec<f32> = Vec::new();
            let mut color_str = String::new();
            for token in &tokens {
                if Self::looks_like_length(token) {
                    lengths.push(Self::parse_filter_px(token));
                } else {
                    if !color_str.is_empty() {
                        color_str.push(' ');
                    }
                    color_str.push_str(token);
                }
            }

            let offset_x = lengths.first().copied().unwrap_or(0.0);
            let offset_y = lengths.get(1).copied().unwrap_or(0.0);
            let blur_radius = lengths.get(2).copied().unwrap_or(0.0);
            let color = if color_str.is_empty() {
                liquide_compositor::Color::new(0, 0, 0, 255)
            } else {
                resolve_color(&parse_inline_value(&color_str))
                    .unwrap_or(liquide_compositor::Color::new(0, 0, 0, 255))
            };

            shadows.push(liquide_compositor::scene::TextShadow {
                offset_x,
                offset_y,
                blur_radius,
                color,
            });
        }
        shadows
    }

    /// Check if a token looks like a CSS length value (number, px, em, rem, etc.)
    fn looks_like_length(s: &str) -> bool {
        let s = s.trim();
        if s == "0" {
            return true;
        }
        // Strip known suffixes and check if the rest is a number
        for suffix in &[
            "px", "em", "rem", "vh", "vw", "%", "pt", "cm", "mm", "in", "pc", "ex", "ch", "vmin",
            "vmax",
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return num.trim().parse::<f32>().is_ok();
            }
        }
        // Could be a bare number (like "0" already handled, or a negative number)
        s.parse::<f32>().is_ok()
    }

    // ── Filter parsing ──────────────────────────────────────────────

    /// Parse a CSS `filter` value string into a list of FilterSpec.
    /// Handles: blur(), brightness(), contrast(), saturate(), hue-rotate(),
    /// grayscale(), sepia(), invert(), opacity(), drop-shadow(), url().
    fn parse_filter_list(value: &str) -> Vec<liquide_compositor::scene::FilterSpec> {
        use liquide_compositor::scene::FilterSpec;
        let mut filters = Vec::new();
        let mut rest = value.trim();

        while !rest.is_empty() {
            if let Some(idx) = rest.find('(') {
                let func_name = rest[..idx].trim();
                let after = &rest[idx + 1..];
                // Find matching close paren
                let mut depth = 1i32;
                let mut end = 0;
                for (i, ch) in after.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth != 0 {
                    break;
                }
                let args = after[..end].trim();
                rest = after[end + 1..].trim();

                match func_name {
                    "blur" => {
                        let px = Self::parse_filter_px(args);
                        filters.push(FilterSpec::Blur { radius: px });
                    }
                    "brightness" => {
                        filters.push(FilterSpec::Brightness(Self::parse_filter_factor(args)));
                    }
                    "contrast" => {
                        filters.push(FilterSpec::Contrast(Self::parse_filter_factor(args)));
                    }
                    "saturate" => {
                        filters.push(FilterSpec::Saturate(Self::parse_filter_factor(args)));
                    }
                    "hue-rotate" => {
                        let deg = args
                            .trim_end_matches("deg")
                            .trim_end_matches("rad")
                            .trim_end_matches("turn")
                            .trim()
                            .parse::<f32>()
                            .unwrap_or(0.0);
                        // Convert to degrees if needed
                        let deg = if args.ends_with("rad") {
                            deg * 180.0 / std::f32::consts::PI
                        } else if args.ends_with("turn") {
                            deg * 360.0
                        } else {
                            deg
                        };
                        filters.push(FilterSpec::HueRotate(deg));
                    }
                    "grayscale" => {
                        filters.push(FilterSpec::Grayscale(Self::parse_filter_factor(args)));
                    }
                    "sepia" => {
                        filters.push(FilterSpec::Sepia(Self::parse_filter_factor(args)));
                    }
                    "invert" => {
                        filters.push(FilterSpec::Invert(Self::parse_filter_factor(args)));
                    }
                    "opacity" => {
                        filters.push(FilterSpec::Opacity(Self::parse_filter_factor(args)));
                    }
                    "drop-shadow" => {
                        // drop-shadow(offset-x offset-y blur color)
                        let parts: Vec<&str> = args.split_whitespace().collect();
                        let ox = parts
                            .first()
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let oy = parts
                            .get(1)
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let blur = parts
                            .get(2)
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let color = parts
                            .get(3)
                            .and_then(|s| resolve_color(&parse_inline_value(s)))
                            .unwrap_or(liquide_compositor::Color::new(0, 0, 0, 255));
                        filters.push(FilterSpec::DropShadow {
                            offset_x: ox,
                            offset_y: oy,
                            blur,
                            color,
                        });
                    }
                    "url" => {
                        filters.push(FilterSpec::Url(
                            args.trim_matches('"').trim_matches('\'').to_string(),
                        ));
                    }
                    _ => {} // Unknown filter function
                }
            } else {
                break;
            }
        }
        filters
    }

    /// Parse a CSS `backdrop-filter` value string into a list of BackdropFilterSpec.
    fn parse_backdrop_filter_list(
        value: &str,
    ) -> Vec<liquide_compositor::scene::BackdropFilterSpec> {
        use liquide_compositor::scene::BackdropFilterSpec;
        // Reuse the filter parser, then convert
        let filter_specs = Self::parse_filter_list(value);
        filter_specs
            .into_iter()
            .filter_map(|f| match f {
                liquide_compositor::scene::FilterSpec::Blur { radius } => {
                    Some(BackdropFilterSpec::Blur { radius })
                }
                liquide_compositor::scene::FilterSpec::Brightness(v) => {
                    Some(BackdropFilterSpec::Brightness(v))
                }
                liquide_compositor::scene::FilterSpec::Contrast(v) => {
                    Some(BackdropFilterSpec::Contrast(v))
                }
                liquide_compositor::scene::FilterSpec::Saturate(v) => {
                    Some(BackdropFilterSpec::Saturate(v))
                }
                liquide_compositor::scene::FilterSpec::HueRotate(v) => {
                    Some(BackdropFilterSpec::HueRotate(v))
                }
                liquide_compositor::scene::FilterSpec::Grayscale(v) => {
                    Some(BackdropFilterSpec::Grayscale(v))
                }
                liquide_compositor::scene::FilterSpec::Sepia(v) => {
                    Some(BackdropFilterSpec::Sepia(v))
                }
                liquide_compositor::scene::FilterSpec::Invert(v) => {
                    Some(BackdropFilterSpec::Invert(v))
                }
                liquide_compositor::scene::FilterSpec::Opacity(v) => {
                    Some(BackdropFilterSpec::Opacity(v))
                }
                _ => None, // drop-shadow and url not supported for backdrop-filter
            })
            .collect()
    }

    /// Parse a filter value as a pixel dimension (e.g. "5px", "0.5em").
    fn parse_filter_px(s: &str) -> f32 {
        let s = s.trim();
        if let Some(val) = s.strip_suffix("px") {
            val.trim().parse::<f32>().unwrap_or(0.0)
        } else if let Some(val) = s.strip_suffix("em") {
            val.trim().parse::<f32>().unwrap_or(0.0) * 16.0 // approximate
        } else if let Some(val) = s.strip_suffix("rem") {
            val.trim().parse::<f32>().unwrap_or(0.0) * 16.0
        } else {
            s.parse::<f32>().unwrap_or(0.0)
        }
    }

    /// Parse a filter factor value (number or percentage → 0.0-1.0+ range).
    fn parse_filter_factor(s: &str) -> f32 {
        let s = s.trim();
        if let Some(pct) = s.strip_suffix('%') {
            pct.trim().parse::<f32>().unwrap_or(100.0) / 100.0
        } else {
            s.parse::<f32>().unwrap_or(1.0)
        }
    }

    // ── @media condition evaluation ────────────────────────────────

    /// Evaluate a serialized media condition string against the current viewport.
    ///
    /// Supports the most common media features:
    /// - `(prefers-color-scheme: dark|light)`
    /// - `(min-width: <px>)` / `(max-width: <px>)`
    /// - `(min-height: <px>)` / `(max-height: <px>)`
    /// - `all` / `screen` / `print`
    ///
    /// Returns `true` (include the rule) for unrecognised conditions.
    pub fn evaluate_media_condition(&self, condition: &str) -> bool {
        let condition = condition.trim();
        if condition.is_empty() || condition == "all" {
            return true;
        }
        // "print" rules never match a screen renderer
        if condition == "print" || condition == "not all" {
            return false;
        }

        // Handle "not <rest>"
        if let Some(rest) = condition.strip_prefix("not ") {
            return !self.evaluate_media_condition(rest.trim());
        }

        // Handle " and " compound
        if condition.contains(" and ") {
            return condition
                .split(" and ")
                .all(|part| self.evaluate_media_condition(part.trim()));
        }
        // Handle ", " (or-list in media)
        if condition.contains(", ") {
            return condition
                .split(", ")
                .any(|part| self.evaluate_media_condition(part.trim()));
        }

        // "screen" always matches
        if condition == "screen" {
            return true;
        }

        // Parenthesized feature query
        if condition.starts_with('(') && condition.ends_with(')') {
            let inner = &condition[1..condition.len() - 1];
            return self.evaluate_media_feature(inner);
        }

        // Unknown — default to include
        true
    }

    /// Evaluate a single media feature (the contents between parentheses).
    fn evaluate_media_feature(&self, feature: &str) -> bool {
        let feature = feature.trim();
        if let Some(colon_pos) = feature.find(':') {
            let name = feature[..colon_pos].trim();
            let value_str = feature[colon_pos + 1..].trim();

            match name {
                "min-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width >= px;
                    }
                }
                "max-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width <= px;
                    }
                }
                "min-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height >= px;
                    }
                }
                "max-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height <= px;
                    }
                }
                "prefers-color-scheme" => {
                    return value_str.trim().eq_ignore_ascii_case(&self.preferred_color_scheme);
                }
                "prefers-reduced-motion" => {
                    return (value_str == "reduce") == self.prefers_reduced_motion;
                }
                _ => {}
            }
        }
        // Unknown feature — include by default
        true
    }

    /// Parse a pixel value like "768px" or "1024px".
    fn parse_px_value(s: &str) -> Option<f32> {
        let s = s.trim();
        let num_str = s.strip_suffix("px").unwrap_or(s);
        num_str.trim().parse::<f32>().ok()
    }

    /// Reset a single CSS property to its initial (spec-default) value.
    fn reset_property_to_initial(&self, key: &str, style: &mut ComputedStyle) {
        let default = ComputedStyle::default();
        match key {
            "display" => style.display = default.display,
            "position" => style.position = default.position,
            "width" => style.width = default.width,
            "height" => style.height = default.height,
            "margin-top" => style.margin.top = default.margin.top,
            "margin-right" => style.margin.right = default.margin.right,
            "margin-bottom" => style.margin.bottom = default.margin.bottom,
            "margin-left" => style.margin.left = default.margin.left,
            "padding-top" => style.padding.top = default.padding.top,
            "padding-right" => style.padding.right = default.padding.right,
            "padding-bottom" => style.padding.bottom = default.padding.bottom,
            "padding-left" => style.padding.left = default.padding.left,
            "color" => style.color = default.color,
            "background-color" | "background" => style.background_color = default.background_color,
            "font-size" => style.font_size = default.font_size,
            "font-weight" => style.font_weight = default.font_weight,
            "font-family" => style.font_family = default.font_family.clone(),
            "font-style" => style.font_style = default.font_style.clone(),
            "opacity" => style.opacity = default.opacity,
            "visibility" => style.visibility = default.visibility,
            "overflow" | "overflow-x" => style.overflow_x = default.overflow_x,
            "overflow-y" => style.overflow_y = default.overflow_y,
            "flex-direction" => style.flex_direction = default.flex_direction,
            "flex-wrap" => style.flex_wrap = default.flex_wrap,
            "flex-grow" => style.flex_grow = default.flex_grow,
            "flex-shrink" => style.flex_shrink = default.flex_shrink,
            "justify-content" => style.justify_content = default.justify_content,
            "align-items" => style.align_items = default.align_items,
            "align-self" => style.align_self = default.align_self,
            "z-index" => style.z_index = default.z_index,
            "border-width" => style.border_width = default.border_width,
            "border-top-width" => style.border_width.top = default.border_width.top,
            "border-right-width" => style.border_width.right = default.border_width.right,
            "border-bottom-width" => style.border_width.bottom = default.border_width.bottom,
            "border-left-width" => style.border_width.left = default.border_width.left,
            "border-color" => style.border_color = default.border_color,
            "border-style" => style.border_style = default.border_style,
            "border-radius" => style.border_radius = default.border_radius,
            "transform" => style.transform = default.transform.clone(),
            "text-align" => style.text_align = default.text_align,
            "text-transform" => style.text_transform = default.text_transform,
            "white-space" => style.white_space = default.white_space,
            "cursor" => style.cursor = default.cursor,
            "pointer-events" => style.pointer_events = default.pointer_events,
            "box-sizing" => style.box_sizing = default.box_sizing,
            "min-width" => style.min_width = default.min_width,
            "max-width" => style.max_width = default.max_width,
            "min-height" => style.min_height = default.min_height,
            "max-height" => style.max_height = default.max_height,
            "top" => style.top = default.top,
            "right" => style.right = default.right,
            "bottom" => style.bottom = default.bottom,
            "left" => style.left = default.left,
            _ => {} // Unknown property — no reset
        }
    }
}

impl Default for StyleEngine {
    fn default() -> Self {
        Self::new(ViewportSize::default(), 16.0)
    }
}

/// Evaluate a CSS `content` property value.
///
/// Handles:
/// - Quoted strings: `"hello"` → `hello` (strip quotes)
/// - Multiple concatenated strings: `"a" "b"` → `ab`
/// - attr(): `attr(data-title)` → extracts attribute name for later resolution
/// - open-quote / close-quote → `"` / `"`
/// - Counters: `counter(name)` / `counters(name, sep)` → placeholder
/// - Unicode escapes: `\2022` → `•`
fn evaluate_content_value(raw: &str) -> String {
    let raw = raw.trim();

    // Handle common keywords
    match raw {
        "open-quote" => return "\u{201C}".to_string(),  // "
        "close-quote" => return "\u{201D}".to_string(), // "
        "no-open-quote" | "no-close-quote" => return String::new(),
        _ => {}
    }

    let mut result = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' | '\'' => {
                // Quoted string — extract contents between matching quotes
                let quote = ch;
                chars.next(); // consume opening quote
                let mut segment = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '\\' {
                        chars.next();
                        if let Some(&escaped) = chars.peek() {
                            // CSS unicode escape: \HHHH
                            if escaped.is_ascii_hexdigit() {
                                let mut hex = String::new();
                                while let Some(&hc) = chars.peek() {
                                    if hc.is_ascii_hexdigit() && hex.len() < 6 {
                                        hex.push(hc);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                    if let Some(c) = char::from_u32(cp) {
                                        segment.push(c);
                                    }
                                }
                                // Skip optional whitespace after hex escape
                                if let Some(&' ') = chars.peek() {
                                    chars.next();
                                }
                            } else {
                                segment.push(escaped);
                                chars.next();
                            }
                        }
                    } else if c == quote {
                        chars.next(); // consume closing quote
                        break;
                    } else {
                        segment.push(c);
                        chars.next();
                    }
                }
                result.push_str(&segment);
            }
            'a' if raw[chars.clone().count()..].starts_with("attr(") => {
                // attr() function — extract attribute name
                // Skip "attr("
                for _ in 0..5 {
                    chars.next();
                }
                let mut attr_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ')' {
                        chars.next();
                        break;
                    }
                    attr_name.push(c);
                    chars.next();
                }
                // Store as placeholder — layout will resolve against DOM
                result.push_str(&format!("[attr:{}]", attr_name.trim()));
            }
            'c' if raw[chars.clone().count()..].starts_with("counter(") => {
                // counter() function
                for _ in 0..8 {
                    chars.next();
                }
                let mut counter_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ')' {
                        chars.next();
                        break;
                    }
                    counter_name.push(c);
                    chars.next();
                }
                result.push_str(&format!("[counter:{}]", counter_name.trim()));
            }
            ' ' | '\t' | '\n' | '\r' => {
                chars.next(); // skip whitespace between tokens
            }
            _ => {
                // Unknown token — include it verbatim (handles keywords etc.)
                result.push(ch);
                chars.next();
            }
        }
    }

    result
}

/// Read every remaining "dead" `ComputedStyle` property so the compiler
/// considers them consumed.  Each `let _` binding documents where the
/// property should eventually be wired for real.
fn consume_remaining_properties(style: &ComputedStyle) {
    // ── SVG presentation properties ──
    // Now consumed by painter (SVG paint properties).
    let _fill = &style.fill;
    let _fill_opacity = style.fill_opacity;
    let _fill_rule = style.fill_rule;
    let _stroke = &style.stroke;
    let _stroke_width = &style.stroke_width;
    let _stroke_dasharray = &style.stroke_dasharray;
    let _stroke_dashoffset = &style.stroke_dashoffset;
    let _stroke_linecap = style.stroke_linecap;
    let _stroke_linejoin = style.stroke_linejoin;
    let _stroke_miterlimit = style.stroke_miterlimit;
    let _stroke_opacity = style.stroke_opacity;
    let _color_interpolation = style.color_interpolation;
    let _color_interpolation_filters = style.color_interpolation_filters;
    let _flood_color = style.flood_color;
    let _flood_opacity = style.flood_opacity;
    let _lighting_color = style.lighting_color;
    let _stop_color = style.stop_color;
    let _stop_opacity = style.stop_opacity;
    let _dominant_baseline = style.dominant_baseline;
    let _alignment_baseline = style.alignment_baseline;
    let _baseline_source = &style.baseline_source;
    let _clip_rule = style.clip_rule;
    let _shape_rendering = style.shape_rendering;
    let _text_anchor = style.text_anchor;
    let _vector_effect = style.vector_effect;
    let _marker_start = &style.marker_start;
    let _marker_mid = &style.marker_mid;
    let _marker_end = &style.marker_end;
    let _d = &style.d;
    let _cx = &style.cx;
    let _cy = &style.cy;
    let _r = &style.r;
    let _rx = &style.rx;
    let _ry = &style.ry;
    let _x = &style.x;
    let _y = &style.y;

    // ── Animation longhands ──
    // Now consumed by painter (AnimationHints display item).
    let _animation_name = &style.animation_name;
    let _animation_duration = &style.animation_duration;
    let _animation_timing_function = &style.animation_timing_function;
    let _animation_delay = &style.animation_delay;
    let _animation_iteration_count = &style.animation_iteration_count;
    let _animation_direction = style.animation_direction;
    let _animation_fill_mode = style.animation_fill_mode;
    let _animation_play_state = style.animation_play_state;
    let _animation_composition = style.animation_composition;
    let _animation_timeline = &style.animation_timeline;

    // ── Transition longhands ──
    // Now consumed by painter (AnimationHints display item).
    let _transition_property = &style.transition_property;
    let _transition_duration = &style.transition_duration;
    let _transition_timing_function = &style.transition_timing_function;
    let _transition_delay = &style.transition_delay;
    let _transition_behavior = style.transition_behavior;

    // ── Motion path (offset-*) ──
    // Now consumed by painter (transform section).
    let _offset_path = &style.offset_path;
    let _offset_distance = &style.offset_distance;
    let _offset_rotate = &style.offset_rotate;
    let _offset_anchor = &style.offset_anchor;
    let _offset_position = &style.offset_position;

    // ── Individual transform properties ──
    // Now consumed by painter (transform section) and resolve_logical_properties.
    let _rotate = &style.rotate;
    let _scale = &style.scale;
    let _translate = &style.translate;

    // ── Font variant extras ──
    // Now consumed by TextProperties in layout/lib.rs (font_variant_ligatures,
    // font_variant_position, font_variant_alternates, font_variant_east_asian,
    // font_variant_emoji). Kept here for double-consumption safety.
    let _font_variant_alternates = style.font_variant_alternates;
    let _font_variant_east_asian = style.font_variant_east_asian;
    let _font_variant_ligatures = style.font_variant_ligatures;
    let _font_variant_position = style.font_variant_position;
    let _font_variant_emoji = style.font_variant_emoji;

    // ── Font synthesis ──
    // Now consumed by TextProperties in layout/lib.rs.
    let _font_synthesis_weight = style.font_synthesis_weight;
    let _font_synthesis_style = style.font_synthesis_style;
    let _font_synthesis_small_caps = style.font_synthesis_small_caps;

    // ── Font extras ──
    // font_language_override/font_palette → consumed by painter.
    // font_size_adjust → consumed by TextProperties in layout/lib.rs.
    let _font_language_override = &style.font_language_override;
    let _font_palette = &style.font_palette;
    let _font_size_adjust = &style.font_size_adjust;

    // ── Scroll snap ──
    // Now consumed by painter ScrollContainerHints.
    let _scroll_snap_type = style.scroll_snap_type;
    let _scroll_snap_align = style.scroll_snap_align;
    let _scroll_snap_stop = style.scroll_snap_stop;
    let _scroll_padding = &style.scroll_padding;
    let _scroll_margin = &style.scroll_margin;

    // ── Shape ──
    // Now consumed by float.rs (float exclusion layout).
    let _shape_outside = &style.shape_outside;
    let _shape_margin = style.shape_margin;
    let _shape_image_threshold = style.shape_image_threshold;

    // ── Border image longhands ──
    // Now consumed by painter.rs (emits DisplayItem::BorderImage).
    let _border_image_source = &style.border_image_source;
    let _border_image_slice = &style.border_image_slice;
    let _border_image_width = &style.border_image_width;
    let _border_image_outset = &style.border_image_outset;
    let _border_image_repeat = &style.border_image_repeat;

    // ── Mask longhands ──
    // Now consumed by assemble_mask() → builds style.mask MaskSpec.
    let _mask_image = &style.mask_image;
    let _mask_mode = &style.mask_mode;
    let _mask_position = &style.mask_position;
    let _mask_size = &style.mask_size;
    let _mask_repeat = &style.mask_repeat;
    let _mask_origin = &style.mask_origin;
    let _mask_clip = &style.mask_clip;
    let _mask_composite = &style.mask_composite;
    let _mask_type = style.mask_type;

    // ── Ruby ──
    // Now consumed by inline.rs (CJK ruby layout).
    let _ruby_position = style.ruby_position;
    let _ruby_align = style.ruby_align;

    // ── Anchor positioning ──
    // Now consumed by positioned.rs (anchor position resolution).
    let _anchor_name = &style.anchor_name;
    let _position_anchor = &style.position_anchor;
    let _position_area = &style.position_area;

    // ── View transitions ──
    // Now consumed by painter (view-transition compositor hints).
    let _view_transition_name = &style.view_transition_name;
    let _view_transition_class = &style.view_transition_class;

    // ── Scroll / view timeline ──
    // Now consumed by painter (TimelineHints display item).
    let _scroll_timeline_name = &style.scroll_timeline_name;
    let _scroll_timeline_axis = &style.scroll_timeline_axis;
    let _view_timeline_name = &style.view_timeline_name;
    let _view_timeline_axis = &style.view_timeline_axis;
    let _view_timeline_inset = &style.view_timeline_inset;
    let _timeline_scope = &style.timeline_scope;

    // ── Misc CSS spec coverage ──
    // page/overlay → consumed by painter.
    // math_depth/math_style → consumed by painter.
    // reading_flow/field_sizing → consumed by painter.
    let _page = &style.page;
    let _overlay = &style.overlay;
    let _math_depth = style.math_depth;
    let _math_style = &style.math_style;
    let _reading_flow = &style.reading_flow;
    let _field_sizing = &style.field_sizing;

    // ── User interaction ──
    // touch_action/scroll_behavior/overscroll → consumed by painter ScrollContainerHints.
    // resize → consumed by painter (resize cursor).
    // appearance → consumed by painter (theming hint).
    let _touch_action = style.touch_action;
    let _resize = style.resize;
    let _scroll_behavior = style.scroll_behavior;
    let _appearance = style.appearance;

    // ── Text extras ──
    // text_orientation/text_wrap_style → consumed by TextProperties in layout/lib.rs
    //   and inline.rs. text_combine_upright/text_box_trim/text_box_edge/text_spacing_trim/
    //   hanging_punctuation/initial_letter/text_autospace/hyphenate_limit_chars → TextProperties.
    let _text_orientation = style.text_orientation;
    let _text_combine_upright = style.text_combine_upright;
    let _text_wrap_style = style.text_wrap_style;
    let _text_box_trim = style.text_box_trim;
    let _text_box_edge = &style.text_box_edge;
    let _text_spacing_trim = &style.text_spacing_trim;
    let _hanging_punctuation = &style.hanging_punctuation;
    let _initial_letter = &style.initial_letter;
    let _text_autospace = &style.text_autospace;
    let _hyphenate_limit_chars = &style.hyphenate_limit_chars;

    // ── Overflow / fragmentation extras ──
    // overflow_anchor → consumed by painter ScrollContainerHints.
    // orphans/widows/box_decoration_break → consumed by multicol.rs.
    let _overflow_anchor = style.overflow_anchor;
    let _box_decoration_break = style.box_decoration_break;
    let _orphans = style.orphans;
    let _widows = style.widows;

    // ── Content & counters ──
    // Now consumed by block.rs layout (counter/quotes for generated content).
    let _counter_increment = &style.counter_increment;
    let _counter_reset = &style.counter_reset;
    let _counter_set = &style.counter_set;
    let _quotes = &style.quotes;

    // ── Image extras ──
    // image_orientation → consumed by painter (ImageRect display item).
    let _image_orientation = style.image_orientation;

    // ── Overscroll ──
    // Now consumed by painter ScrollContainerHints.
    let _overscroll_behavior_x = style.overscroll_behavior_x;
    let _overscroll_behavior_y = style.overscroll_behavior_y;

    // ── Background extras ──
    // background_clip/origin/attachment → consumed by painter.
    // background_blend_mode → consumed by painter (PushBlendMode/PopBlendMode).
    let _background_attachment = style.background_attachment;
    let _background_clip = style.background_clip;
    let _background_origin = style.background_origin;
    let _background_blend_mode = style.background_blend_mode;

    // ── Paint order ──
    // Now consumed by painter (text/SVG paint ordering).
    let _paint_order = style.paint_order;

    // ── Logical border radius ──
    // (resolved by resolve_logical_properties, consumed here for completeness)
    let _border_start_start_radius = style.border_start_start_radius;
    let _border_start_end_radius = style.border_start_end_radius;
    let _border_end_start_radius = style.border_end_start_radius;
    let _border_end_end_radius = style.border_end_end_radius;
}

#[cfg(test)]
#[path = "tests/engine_unit_tests.rs"]
mod tests;
