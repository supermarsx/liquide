//! CSS stylesheet representation

use crate::error::Result as ThemeResult;
use crate::parser::ThemeParser;
use crate::property::PropertySet;
use crate::selector::Selector;
use crate::value::{FontFaceRule, KeyframesRule, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_IMPORT_DEPTH: usize = 10;
const ANONYMOUS_LAYER_PREFIX: &str = "__liquide_anon_layer__";
const STRUCTURAL_RECORD_SEPARATOR: char = '\u{1f}';
const STRUCTURAL_FIELD_SEPARATOR: char = '\u{1e}';

pub const STRUCTURAL_CONDITION_SENTINEL: &str = "__liquide_structural__";

static NEXT_ANONYMOUS_LAYER_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportLayer {
    Named(String),
    Anonymous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportRule {
    pub url: String,
    pub layer: Option<ImportLayer>,
    pub supports_condition: Option<String>,
    pub media_condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuralContainerConstraint {
    pub name: Option<String>,
    pub condition: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuralCondition {
    pub containers: Vec<StructuralContainerConstraint>,
    pub scope_start: Option<String>,
    pub scope_end: Option<String>,
}

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

    /// `@import` rules and qualifiers (resolved externally when desired).
    imports: Vec<ImportRule>,

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

    /// Load one or more stylesheet paths and resolve their local `@import` trees.
    pub fn load_paths_with_imports(paths: &[PathBuf]) -> ThemeResult<Self> {
        let parser = ThemeParser::new();
        let candidates = Self::collect_stylesheet_candidates(paths)?;
        let root_files = Self::detect_root_stylesheets(&parser, &candidates, paths)?;

        let mut combined = StyleSheet::new();
        for root in root_files {
            let mut visiting = HashSet::new();
            let sheet = Self::load_root_with_imports(&parser, &root, &mut visiting, 0)?;
            combined.merge(&sheet);
        }

        Ok(combined)
    }

    /// Load a single stylesheet path and resolve its local `@import` tree.
    pub fn load_path_with_imports<P: AsRef<Path>>(path: P) -> ThemeResult<Self> {
        Self::load_paths_with_imports(&[path.as_ref().to_path_buf()])
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
        self.add_rule_with_conditions(selector, properties, Some(media_condition), None, None);
    }

    /// Add a rule that is gated on a `@supports` condition string.
    pub fn add_supports_rule(
        &mut self,
        selector: Selector,
        properties: PropertySet,
        supports_condition: String,
    ) {
        self.add_rule_with_conditions(selector, properties, None, Some(supports_condition), None);
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
        for layer_name in &other.layer_order {
            self.add_layer(layer_name);
        }
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
        self.add_import_rule(ImportRule {
            url,
            layer: None,
            supports_condition: None,
            media_condition: None,
        });
    }

    /// Add an `@import` rule with full qualifier metadata.
    pub fn add_import_rule(&mut self, import: ImportRule) {
        self.imports.push(import);
    }

    /// All `@import` rules.
    pub fn imports(&self) -> &[ImportRule] {
        &self.imports
    }

    // ── @layer ─────────────────────────────────────────────────────────
    /// Declare a cascade layer. Layers are ordered by first declaration.
    pub fn add_layer(&mut self, name: &str) {
        if !self.layer_order.contains(&name.to_string()) {
            self.layer_order.push(name.to_string());
        }
    }

    /// Allocate a stable internal name for an anonymous layer.
    pub fn allocate_anonymous_layer(&mut self) -> String {
        let name = Self::next_anonymous_layer_name();
        self.add_layer(&name);
        name
    }

    /// Generate a stable internal name for an anonymous layer without registering it.
    pub fn fresh_anonymous_layer_name() -> String {
        Self::next_anonymous_layer_name()
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

    pub fn encode_structural_condition(condition: &StructuralCondition) -> String {
        let mut records = Vec::new();

        if let Some(scope_start) = &condition.scope_start {
            records.push(format!(
                "scope-start{STRUCTURAL_FIELD_SEPARATOR}{scope_start}"
            ));
        }
        if let Some(scope_end) = &condition.scope_end {
            records.push(format!("scope-end{STRUCTURAL_FIELD_SEPARATOR}{scope_end}"));
        }
        for container in &condition.containers {
            records.push(format!(
                "container{STRUCTURAL_FIELD_SEPARATOR}{}{STRUCTURAL_FIELD_SEPARATOR}{}",
                container.name.as_deref().unwrap_or(""),
                container.condition
            ));
        }

        records.join(&STRUCTURAL_RECORD_SEPARATOR.to_string())
    }

    pub fn decode_structural_condition(
        name: Option<&str>,
        condition: &str,
    ) -> Option<StructuralCondition> {
        if name != Some(STRUCTURAL_CONDITION_SENTINEL) {
            return None;
        }

        let mut decoded = StructuralCondition::default();
        for record in condition.split(STRUCTURAL_RECORD_SEPARATOR) {
            let mut fields = record.split(STRUCTURAL_FIELD_SEPARATOR);
            match fields.next()? {
                "scope-start" => decoded.scope_start = fields.next().map(|value| value.to_string()),
                "scope-end" => decoded.scope_end = fields.next().map(|value| value.to_string()),
                "container" => {
                    let raw_name = fields.next().unwrap_or("");
                    let raw_condition = fields.next().unwrap_or("");
                    decoded.containers.push(StructuralContainerConstraint {
                        name: if raw_name.is_empty() {
                            None
                        } else {
                            Some(raw_name.to_string())
                        },
                        condition: raw_condition.to_string(),
                    });
                }
                _ => return None,
            }
        }

        Some(decoded)
    }

    fn next_anonymous_layer_name() -> String {
        format!(
            "{ANONYMOUS_LAYER_PREFIX}{}",
            NEXT_ANONYMOUS_LAYER_ID.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn detect_root_stylesheets(
        parser: &ThemeParser,
        candidates: &[PathBuf],
        watched_paths: &[PathBuf],
    ) -> ThemeResult<Vec<PathBuf>> {
        let candidate_set: HashSet<PathBuf> = candidates.iter().cloned().collect();
        let explicit_files: HashSet<PathBuf> = watched_paths
            .iter()
            .filter(|path| path.is_file() && Self::is_css_file(path))
            .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
            .collect();
        let mut imported = HashSet::new();

        for candidate in candidates {
            let sheet = parser.parse_file(candidate)?;
            let base_dir = candidate.parent().unwrap_or_else(|| Path::new("."));
            for import in sheet.imports() {
                let import_path = base_dir.join(Self::strip_import_url(&import.url));
                if let Ok(canonical) = import_path.canonicalize() {
                    if candidate_set.contains(&canonical) {
                        imported.insert(canonical);
                    }
                }
            }
        }

        let mut roots: Vec<PathBuf> = candidates
            .iter()
            .filter(|path| explicit_files.contains(*path) || !imported.contains(*path))
            .cloned()
            .collect();
        if roots.is_empty() {
            roots = candidates.to_vec();
        }
        roots.sort();
        Ok(roots)
    }

    fn collect_stylesheet_candidates(paths: &[PathBuf]) -> ThemeResult<Vec<PathBuf>> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        for path in paths {
            if path.is_file() && Self::is_css_file(path) {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical.clone()) {
                    candidates.push(canonical);
                }
            } else if path.is_dir() {
                Self::collect_stylesheet_candidates_in_dir(path, &mut candidates, &mut seen)?;
            }
        }

        candidates.sort();
        Ok(candidates)
    }

    fn collect_stylesheet_candidates_in_dir(
        dir: &Path,
        candidates: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) -> ThemeResult<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::result::Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                Self::collect_stylesheet_candidates_in_dir(&path, candidates, seen)?;
            } else if path.is_file() && Self::is_css_file(&path) {
                let canonical = path.canonicalize().unwrap_or(path);
                if seen.insert(canonical.clone()) {
                    candidates.push(canonical);
                }
            }
        }

        Ok(())
    }

    fn load_root_with_imports(
        parser: &ThemeParser,
        path: &Path,
        visiting: &mut HashSet<PathBuf>,
        depth: usize,
    ) -> ThemeResult<Self> {
        if depth > MAX_IMPORT_DEPTH {
            return Ok(StyleSheet::new());
        }

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if !visiting.insert(canonical.clone()) {
            return Ok(StyleSheet::new());
        }

        let stylesheet = parser.parse_file(&canonical)?;
        let imports = stylesheet.imports.clone();
        let mut combined = StyleSheet::new();
        let base_dir = canonical.parent().unwrap_or_else(|| Path::new("."));

        for import in imports {
            let import_path = base_dir.join(Self::strip_import_url(&import.url));
            let mut imported =
                Self::load_root_with_imports(parser, &import_path, visiting, depth + 1)?;
            imported.apply_import_context(&import);
            combined.merge(&imported);
        }

        visiting.remove(&canonical);
        combined.merge(&stylesheet);
        Ok(combined)
    }

    fn apply_import_context(&mut self, import: &ImportRule) {
        if let Some(layer) = &import.layer {
            self.apply_import_layer(layer);
        }
        if let Some(condition) = import.media_condition.as_deref() {
            self.apply_media_gate(condition);
        }
        if let Some(condition) = import.supports_condition.as_deref() {
            self.apply_supports_gate(condition);
        }
    }

    fn apply_import_layer(&mut self, layer: &ImportLayer) {
        let base_layer = match layer {
            ImportLayer::Named(name) => name.clone(),
            ImportLayer::Anonymous => Self::next_anonymous_layer_name(),
        };

        let existing_layers = std::mem::take(&mut self.layer_order);
        let mut renamed_layers = HashMap::new();
        self.add_layer(&base_layer);
        for layer_name in existing_layers {
            let qualified = Self::qualify_layer_name(Some(&base_layer), &layer_name);
            renamed_layers.insert(layer_name, qualified.clone());
            self.add_layer(&qualified);
        }

        self.update_rule_layers(&base_layer, &renamed_layers);
    }

    fn update_rule_layers(&mut self, base_layer: &str, renamed_layers: &HashMap<String, String>) {
        for rule in &mut self.rules {
            Self::reassign_rule_layer(rule, base_layer, renamed_layers);
        }
        for container_rule in &mut self.container_rules {
            for rule in &mut container_rule.rules {
                Self::reassign_rule_layer(rule, base_layer, renamed_layers);
            }
        }
        for scope_rule in &mut self.scope_rules {
            for rule in &mut scope_rule.rules {
                Self::reassign_rule_layer(rule, base_layer, renamed_layers);
            }
        }
        for rule in &mut self.starting_style_rules {
            Self::reassign_rule_layer(rule, base_layer, renamed_layers);
        }
    }

    fn reassign_rule_layer(
        rule: &mut StyleRule,
        base_layer: &str,
        renamed_layers: &HashMap<String, String>,
    ) {
        rule.layer = Some(match rule.layer.as_ref() {
            Some(existing) => renamed_layers
                .get(existing)
                .cloned()
                .unwrap_or_else(|| Self::qualify_layer_name(Some(base_layer), existing)),
            None => base_layer.to_string(),
        });
    }

    fn apply_media_gate(&mut self, outer_condition: &str) {
        for rule in &mut self.rules {
            rule.media_condition =
                Self::combine_gate_conditions(outer_condition, rule.media_condition.as_deref());
        }
        for container_rule in &mut self.container_rules {
            for rule in &mut container_rule.rules {
                rule.media_condition =
                    Self::combine_gate_conditions(outer_condition, rule.media_condition.as_deref());
            }
        }
        for scope_rule in &mut self.scope_rules {
            for rule in &mut scope_rule.rules {
                rule.media_condition =
                    Self::combine_gate_conditions(outer_condition, rule.media_condition.as_deref());
            }
        }
        for rule in &mut self.starting_style_rules {
            rule.media_condition =
                Self::combine_gate_conditions(outer_condition, rule.media_condition.as_deref());
        }
    }

    fn apply_supports_gate(&mut self, outer_condition: &str) {
        for rule in &mut self.rules {
            rule.supports_condition =
                Self::combine_gate_conditions(outer_condition, rule.supports_condition.as_deref());
        }
        for container_rule in &mut self.container_rules {
            for rule in &mut container_rule.rules {
                rule.supports_condition = Self::combine_gate_conditions(
                    outer_condition,
                    rule.supports_condition.as_deref(),
                );
            }
        }
        for scope_rule in &mut self.scope_rules {
            for rule in &mut scope_rule.rules {
                rule.supports_condition = Self::combine_gate_conditions(
                    outer_condition,
                    rule.supports_condition.as_deref(),
                );
            }
        }
        for rule in &mut self.starting_style_rules {
            rule.supports_condition =
                Self::combine_gate_conditions(outer_condition, rule.supports_condition.as_deref());
        }
    }

    fn combine_gate_conditions(outer: &str, inner: Option<&str>) -> Option<String> {
        let outer = outer.trim();
        if outer.is_empty() {
            return inner.map(|value| value.to_string());
        }

        match inner.map(str::trim).filter(|value| !value.is_empty()) {
            Some(inner) => Some(format!("({outer}) and ({inner})")),
            None => Some(outer.to_string()),
        }
    }

    fn qualify_layer_name(parent: Option<&str>, layer_name: &str) -> String {
        match parent.filter(|name| !name.is_empty()) {
            Some(parent) => format!("{parent}.{layer_name}"),
            None => layer_name.to_string(),
        }
    }

    fn is_css_file(path: &Path) -> bool {
        path.extension().and_then(|ext| ext.to_str()) == Some("css")
    }

    fn strip_import_url(raw: &str) -> String {
        let mut url = raw.trim();
        if let Some(inner) = url.strip_prefix("url(") {
            url = inner.strip_suffix(')').unwrap_or(inner).trim();
        }
        if (url.starts_with('"') && url.ends_with('"'))
            || (url.starts_with('\'') && url.ends_with('\''))
        {
            url = &url[1..url.len() - 1];
        }
        url.to_string()
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

        if let Some(parts) = Self::split_top_level_keyword(condition, "or") {
            return parts
                .iter()
                .any(|part| self.evaluate_media_condition(part, env));
        }
        if let Some(parts) = Self::split_top_level_keyword(condition, "and") {
            return parts
                .iter()
                .all(|part| self.evaluate_media_condition(part, env));
        }
        if let Some(parts) = Self::split_top_level_commas(condition) {
            return parts
                .iter()
                .any(|part| self.evaluate_media_condition(part, env));
        }

        match condition {
            "all" | "screen" => true,
            "print" | "not all" => false,
            _ => Self::strip_wrapping_parens(condition)
                .map(|inner| self.evaluate_media_feature_or_comparison(inner.trim(), env))
                .unwrap_or(false),
        }
    }

    fn evaluate_media_feature_or_comparison(&self, inner: &str, env: &QueryEnvironment) -> bool {
        if let Some(result) = self.evaluate_media_comparison(inner, env) {
            return result;
        }

        if let Some((feature, value)) = inner.split_once(':') {
            let feature = feature.trim();
            let value = value.trim();
            match feature {
                "min-width" => Self::parse_px_value(value)
                    .map(|v| env.viewport_width >= v)
                    .unwrap_or(false),
                "max-width" => Self::parse_px_value(value)
                    .map(|v| env.viewport_width <= v)
                    .unwrap_or(false),
                "min-height" => Self::parse_px_value(value)
                    .map(|v| env.viewport_height >= v)
                    .unwrap_or(false),
                "max-height" => Self::parse_px_value(value)
                    .map(|v| env.viewport_height <= v)
                    .unwrap_or(false),
                "width" => Self::parse_px_value(value)
                    .map(|v| (env.viewport_width - v).abs() < 1.0)
                    .unwrap_or(false),
                "height" => Self::parse_px_value(value)
                    .map(|v| (env.viewport_height - v).abs() < 1.0)
                    .unwrap_or(false),
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
                _ => false,
            }
        } else {
            false
        }
    }

    fn evaluate_media_comparison(&self, inner: &str, env: &QueryEnvironment) -> Option<bool> {
        let tokens: Vec<&str> = inner.split_whitespace().collect();
        match tokens.as_slice() {
            [lhs, op, rhs] if Self::is_media_comparison_operator(op) => {
                Self::evaluate_media_comparison_pair(lhs, op, rhs, env)
            }
            [lhs, op1, mid, op2, rhs]
                if Self::is_media_comparison_operator(op1)
                    && Self::is_media_comparison_operator(op2) =>
            {
                Some(
                    Self::evaluate_media_comparison_pair(lhs, op1, mid, env)?
                        && Self::evaluate_media_comparison_pair(mid, op2, rhs, env)?,
                )
            }
            _ => None,
        }
    }

    fn evaluate_media_comparison_pair(
        lhs: &str,
        op: &str,
        rhs: &str,
        env: &QueryEnvironment,
    ) -> Option<bool> {
        let lhs_val = Self::media_dimension_value(lhs.trim(), env)?;
        let rhs_val = Self::media_dimension_value(rhs.trim(), env)?;
        Some(match op {
            "<=" => lhs_val <= rhs_val,
            ">=" => lhs_val >= rhs_val,
            "<" => lhs_val < rhs_val,
            ">" => lhs_val > rhs_val,
            _ => return None,
        })
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

        if let Some(parts) = Self::split_top_level_keyword(condition, "or") {
            return parts
                .iter()
                .any(|part| self.evaluate_supports_condition(part, env));
        }
        if let Some(parts) = Self::split_top_level_keyword(condition, "and") {
            return parts
                .iter()
                .all(|part| self.evaluate_supports_condition(part, env));
        }

        let inner = Self::strip_wrapping_parens(condition)
            .unwrap_or(condition)
            .trim();

        if let Some(colon_index) = Self::find_top_level_delimiter(inner, ':') {
            let property = inner[..colon_index].trim();
            let value = inner[colon_index + 1..].trim();
            let property = property.trim();
            let property_supported = if env.supported_properties.is_empty() {
                Self::is_supported_css_property(property)
            } else {
                env.supported_properties.contains(property)
            };
            property_supported && Self::is_supported_css_value(property, value)
        } else {
            false
        }
    }

    fn split_top_level_keyword<'a>(input: &'a str, keyword: &str) -> Option<Vec<&'a str>> {
        let pattern = format!(" {keyword} ");
        let mut depth = 0i32;
        let mut start = 0usize;
        let mut skip_until = 0usize;
        let mut parts = Vec::new();

        for (idx, ch) in input.char_indices() {
            if idx < skip_until {
                continue;
            }

            match ch {
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                _ => {}
            }

            if depth == 0 && input[idx..].starts_with(&pattern) {
                parts.push(input[start..idx].trim());
                start = idx + pattern.len();
                skip_until = start;
            }
        }

        if parts.is_empty() {
            None
        } else {
            parts.push(input[start..].trim());
            Some(parts)
        }
    }

    fn split_top_level_commas(input: &str) -> Option<Vec<&str>> {
        let mut depth = 0i32;
        let mut start = 0usize;
        let mut parts = Vec::new();

        for (idx, ch) in input.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(input[start..idx].trim());
                    start = idx + 1;
                }
                _ => {}
            }
        }

        if parts.is_empty() {
            None
        } else {
            parts.push(input[start..].trim());
            Some(parts)
        }
    }

    fn strip_wrapping_parens(input: &str) -> Option<&str> {
        if !(input.starts_with('(') && input.ends_with(')')) {
            return None;
        }

        let mut depth = 0i32;
        for (idx, ch) in input.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 && idx != input.len() - 1 {
                        return None;
                    }
                }
                _ => {}
            }
        }

        if depth == 0 {
            Some(&input[1..input.len() - 1])
        } else {
            None
        }
    }

    fn find_top_level_delimiter(input: &str, delimiter: char) -> Option<usize> {
        let mut depth = 0i32;
        for (idx, ch) in input.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                _ if ch == delimiter && depth == 0 => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn is_media_comparison_operator(value: &str) -> bool {
        matches!(value, "<" | ">" | "<=" | ">=")
    }

    pub fn parse_px_value(value: &str) -> Option<f32> {
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

    pub fn is_supported_css_property(property: &str) -> bool {
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

    pub fn is_supported_css_value(property: &str, value: &str) -> bool {
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
            "position" => matches!(
                value,
                "static" | "relative" | "absolute" | "fixed" | "sticky"
            ),
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
            | "border-radius" | "border-width" | "font-size" | "line-height" | "letter-spacing"
            | "word-spacing" | "text-indent" | "flex-basis" => {
                matches!(
                    value,
                    "auto"
                        | "none"
                        | "0"
                        | "inherit"
                        | "initial"
                        | "unset"
                        | "min-content"
                        | "max-content"
                        | "fit-content"
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
                    "auto"
                        | "normal"
                        | "bold"
                        | "bolder"
                        | "lighter"
                        | "inherit"
                        | "initial"
                        | "unset"
                ) || value.parse::<f32>().is_ok()
            }
            _ => Self::is_supported_css_property(property),
        }
    }
}

#[cfg(test)]
#[path = "tests/stylesheet_tests.rs"]
mod tests;
