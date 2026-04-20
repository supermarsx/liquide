//! The style engine — orchestrates cascade, specificity, inheritance, and variable resolution.

mod apply;
mod apply_ext;
mod assemble;
mod cascade;
pub(crate) mod content;
mod filters;
mod logical;
mod media;
mod stylesheet;
mod variables;

pub use cascade::RestyleResult;

use liquide_theme_css::property::PropertySet;

/// A prepared stylesheet rule ready for matching.
#[derive(Debug)]
pub struct PreparedRule {
    pub selector: crate::selector::ComplexSelector,
    pub specificity: crate::specificity::Specificity,
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
    /// Index: tag name → rule indices. Rules with no tag filter (universal
    /// selector, class-only, etc.) are stored in `universal_rule_indices`.
    tag_index: std::collections::HashMap<String, Vec<usize>>,
    /// Rule indices that have no tag filter and must be checked against every node.
    universal_rule_indices: Vec<usize>,
}

impl PreparedSheet {
    /// Build a new PreparedSheet with a tag-name index for fast lookup.
    pub fn new(rules: Vec<PreparedRule>) -> Self {
        let mut tag_index: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut universal_rule_indices = Vec::new();

        for (i, rule) in rules.iter().enumerate() {
            // The key selector is compounds[0] (rightmost in CSS, first in our array)
            if let Some(tag) = rule.selector.compounds[0].tag.as_ref() {
                tag_index.entry(tag.to_ascii_lowercase()).or_default().push(i);
            } else {
                universal_rule_indices.push(i);
            }
        }

        Self {
            rules,
            tag_index,
            universal_rule_indices,
        }
    }

    /// Iterate rule indices that could potentially match a node with the given tag name.
    pub fn candidate_indices(&self, tag_name: &str) -> impl Iterator<Item = usize> + '_ {
        let lower = tag_name.to_ascii_lowercase();
        let tag_iter = self
            .tag_index
            .get(&lower)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .iter()
            .copied();
        let universal_iter = self.universal_rule_indices.iter().copied();
        tag_iter.chain(universal_iter)
    }
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
    pub(crate) sheets: Vec<PreparedSheet>,
    /// Viewport size for viewport units.
    pub viewport: ViewportSize,
    /// Base font size for `rem` units.
    pub base_font_size: f32,
    /// CSS variables.
    pub(crate) variables: std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    /// Layer order map: layer name → layer index (1-based).
    pub(crate) layer_order: std::collections::HashMap<String, u32>,
    /// `@font-face` rules parsed from stylesheets.
    pub(crate) font_faces: Vec<PreparedFontFace>,
    /// CSS properties we support (for `@supports` runtime evaluation).
    pub(crate) supported_properties: std::collections::HashSet<&'static str>,
    /// Registered custom properties from `@property` rules.
    pub(crate) registered_properties: std::collections::HashMap<String, RegisteredPropertyDef>,
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

    /// Get parsed @font-face rules for external font loading.
    pub fn font_faces(&self) -> &[PreparedFontFace] {
        &self.font_faces
    }

    /// Get registered custom properties.
    pub fn registered_property(&self, name: &str) -> Option<&RegisteredPropertyDef> {
        self.registered_properties.get(name)
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

    /// Invalidate all cached style data, forcing styles to be recomputed.
    ///
    /// Currently styles are recomputed on demand each time
    /// [`compute_style`](Self::compute_style) is called, so this method
    /// clears the compiled sheet and variable state to allow a full
    /// re-evaluation after stylesheet changes.
    pub fn invalidate_all(&mut self) {
        self.sheets.clear();
        self.variables.clear();
        self.font_faces.clear();
        self.registered_properties.clear();
        self.keyframes.clear();
        self.layer_order.clear();
    }
}

impl Default for StyleEngine {
    fn default() -> Self {
        Self::new(ViewportSize::default(), 16.0)
    }
}

#[cfg(test)]
use crate::computed::*;

#[cfg(test)]
#[path = "../tests/engine_unit_tests.rs"]
mod tests;
