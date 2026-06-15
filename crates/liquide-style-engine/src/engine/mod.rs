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
pub use variables::EnvironmentValues;

use std::cell::RefCell;

use liquide_dom::NodeId;
use liquide_theme_css::property::PropertySet;

#[allow(deprecated)]
use crate::transition::TransitionManager;

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
                tag_index
                    .entry(tag.to_ascii_lowercase())
                    .or_default()
                    .push(i);
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
    /// Dynamic viewport size (accounts for dynamic UI chrome like virtual keyboards).
    /// Defaults to the standard viewport size.
    pub dynamic_viewport: Option<(f32, f32)>,
    /// Small viewport size (smallest possible viewport when all UI chrome is visible).
    /// Defaults to the standard viewport size.
    pub small_viewport: Option<(f32, f32)>,
    /// Large viewport size (largest possible viewport when all UI chrome is hidden).
    /// Defaults to the standard viewport size.
    pub large_viewport: Option<(f32, f32)>,
    /// Base font size for `rem` units.
    pub base_font_size: f32,
    /// CSS variables.
    pub(crate) variables:
        std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
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
    /// Platform-provided environment values for CSS `env()` resolution.
    pub env_values: EnvironmentValues,
    /// CSS transition manager — detects property changes and interpolates values.
    #[allow(deprecated)]
    pub transition_manager: RefCell<TransitionManager>,
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
    /// Create a new style engine with the default `"light"` color scheme.
    pub fn new(viewport: ViewportSize, base_font_size: f32) -> Self {
        Self::new_with_color_scheme(viewport, base_font_size, "light")
    }

    /// Create a new style engine with an explicit initial color scheme.
    ///
    /// `color_scheme` should be `"light"` or `"dark"`. Any other value is
    /// normalised to `"light"`.
    pub fn new_with_color_scheme(
        viewport: ViewportSize,
        base_font_size: f32,
        color_scheme: &str,
    ) -> Self {
        let preferred = if color_scheme.trim().eq_ignore_ascii_case("dark") {
            "dark".to_string()
        } else {
            "light".to_string()
        };
        Self {
            sheets: Vec::new(),
            viewport,
            dynamic_viewport: None,
            small_viewport: None,
            large_viewport: None,
            base_font_size,
            variables: std::collections::HashMap::new(),
            layer_order: std::collections::HashMap::new(),
            font_faces: Vec::new(),
            supported_properties: Self::build_supported_properties(),
            registered_properties: std::collections::HashMap::new(),
            preferred_color_scheme: preferred,
            prefers_reduced_motion: false,
            keyframes: std::collections::HashMap::new(),
            env_values: EnvironmentValues::default(),
            #[allow(deprecated)] // TODO: migrate to liquide_animation::TransitionEngine
            transition_manager: RefCell::new(TransitionManager::new()),
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

    /// Set the dynamic viewport size.
    ///
    /// The dynamic viewport reflects the current state of dynamic UI chrome
    /// (e.g. virtual keyboards, expanding/collapsing URL bars).
    pub fn set_dynamic_viewport(&mut self, width: f32, height: f32) {
        self.dynamic_viewport = Some((width, height));
    }

    /// Set the small viewport size.
    ///
    /// The small viewport is the smallest possible viewport when all dynamic
    /// UI chrome (virtual keyboard, URL bar, etc.) is visible.
    pub fn set_small_viewport(&mut self, width: f32, height: f32) {
        self.small_viewport = Some((width, height));
    }

    /// Set the large viewport size.
    ///
    /// The large viewport is the largest possible viewport when all dynamic
    /// UI chrome is hidden.
    pub fn set_large_viewport(&mut self, width: f32, height: f32) {
        self.large_viewport = Some((width, height));
    }

    /// Build a [`ViewportSizes`](crate::dimension::ViewportSizes) from the
    /// configured viewport tiers, falling back to the standard viewport for
    /// any tier that has not been explicitly set.
    pub fn viewport_sizes(&self) -> crate::dimension::ViewportSizes {
        let (dw, dh) = self
            .dynamic_viewport
            .unwrap_or((self.viewport.width, self.viewport.height));
        let (sw, sh) = self
            .small_viewport
            .unwrap_or((self.viewport.width, self.viewport.height));
        let (lw, lh) = self
            .large_viewport
            .unwrap_or((self.viewport.width, self.viewport.height));
        crate::dimension::ViewportSizes {
            width: self.viewport.width,
            height: self.viewport.height,
            dynamic_width: dw,
            dynamic_height: dh,
            small_width: sw,
            small_height: sh,
            large_width: lw,
            large_height: lh,
        }
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
        self.transition_manager.borrow_mut().clear();
    }

    /// Feed the transition manager with updated computed styles from the style map.
    ///
    /// Call this **after** `restyle_all()` or `restyle_dirty()`. For each node
    /// in the map, the transition manager compares the new style against its
    /// stored previous values and starts `RunningTransition`s when transitionable
    /// properties change.
    ///
    /// Then, for any nodes with active transitions, the interpolated values
    /// are written back into the style map — overriding the cascade result
    /// with the in-flight transition value.
    #[allow(deprecated)]
    pub fn apply_transitions(&self, map: &mut crate::style_map::StyleMap) {
        use std::sync::Arc;

        let mut tm = self.transition_manager.borrow_mut();

        // Phase 1: Feed new styles into the transition manager.
        let node_ids: Vec<NodeId> = map.iter().map(|(&nid, _)| nid).collect();
        for &node_id in &node_ids {
            if let Some(style) = map.get(node_id) {
                tm.update_node(node_id, style);
            }
        }

        // Phase 2: Override computed style for nodes with running transitions.
        if !tm.has_running_transitions() {
            return;
        }

        for &node_id in &node_ids {
            let style_arc = match map.get(node_id) {
                Some(s) => s.clone(),
                None => continue,
            };

            // Collect overrides for this node.
            let overrides = collect_transition_overrides(&tm, node_id, &style_arc);
            if overrides.is_empty() {
                continue;
            }

            // Clone the style, apply overrides, and replace in the map.
            let mut style = (*style_arc).clone();
            for (prop, val) in &overrides {
                apply_numeric_override(&mut style, prop, *val);
            }
            map.insert_shared(node_id, Arc::new(style));
        }
    }

    /// Advance all running CSS transitions by `dt_ms` milliseconds.
    ///
    /// Call this once per frame (typically from the compositor frame callback).
    /// After ticking, call [`apply_transitions`](Self::apply_transitions) to
    /// write interpolated values into the style map.
    #[allow(deprecated)]
    pub fn tick_transitions(&self, dt_ms: f32) {
        self.transition_manager.borrow_mut().tick_all(dt_ms);
    }

    /// Check if any CSS transitions are currently running.
    #[allow(deprecated)]
    pub fn has_running_transitions(&self) -> bool {
        self.transition_manager.borrow().has_running_transitions()
    }

    /// Remove transition tracking for a node (e.g. when the node is removed from the DOM).
    #[allow(deprecated)]
    pub fn remove_transition_node(&self, node_id: NodeId) {
        self.transition_manager.borrow_mut().remove_node(node_id);
    }
}

/// Collect interpolated transition overrides for a single node.
#[allow(deprecated)]
fn collect_transition_overrides(
    tm: &TransitionManager,
    node_id: NodeId,
    style: &crate::computed::ComputedStyle,
) -> Vec<(String, f32)> {
    let mut overrides = Vec::new();
    for def in &style.transition {
        if let Some(val) = tm.get_value(node_id, &def.property) {
            overrides.push((def.property.clone(), val));
        }
    }
    overrides
}

/// Write a numeric override into a `ComputedStyle`.
///
/// Mirrors the property set recognized by `extract_numeric_property` in
/// `transition.rs` so that every property the transition engine can detect
/// can also be written back.
fn apply_numeric_override(style: &mut crate::computed::ComputedStyle, property: &str, val: f32) {
    use crate::dimension::Dimension;
    match property {
        "opacity" => style.opacity = val,
        "width" => style.width = Dimension::Px(val),
        "height" => style.height = Dimension::Px(val),
        "min-width" => style.min_width = Dimension::Px(val),
        "min-height" => style.min_height = Dimension::Px(val),
        "max-width" => style.max_width = Dimension::Px(val),
        "max-height" => style.max_height = Dimension::Px(val),
        "top" => style.top = Dimension::Px(val),
        "right" => style.right = Dimension::Px(val),
        "bottom" => style.bottom = Dimension::Px(val),
        "left" => style.left = Dimension::Px(val),
        "margin-top" => style.margin.top = Dimension::Px(val),
        "margin-right" => style.margin.right = Dimension::Px(val),
        "margin-bottom" => style.margin.bottom = Dimension::Px(val),
        "margin-left" => style.margin.left = Dimension::Px(val),
        "padding-top" => style.padding.top = Dimension::Px(val),
        "padding-right" => style.padding.right = Dimension::Px(val),
        "padding-bottom" => style.padding.bottom = Dimension::Px(val),
        "padding-left" => style.padding.left = Dimension::Px(val),
        "font-size" => style.font_size = val,
        "letter-spacing" => style.letter_spacing = val,
        "word-spacing" => style.word_spacing = val,
        "border-top-width" => style.border_width.top = val,
        "border-right-width" => style.border_width.right = val,
        "border-bottom-width" => style.border_width.bottom = val,
        "border-left-width" => style.border_width.left = val,
        "flex-grow" => style.flex_grow = val,
        "flex-shrink" => style.flex_shrink = val,
        "gap" => {
            style.gap.width = Dimension::Px(val);
            style.gap.height = Dimension::Px(val);
        }
        "column-gap" => style.column_gap = Dimension::Px(val),
        "row-gap" => style.row_gap = Dimension::Px(val),
        _ => {} // Unsupported property — silently skip
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

#[cfg(test)]
mod transition_integration_tests {
    use super::*;
    use liquide_dom::Document;

    #[test]
    fn apply_transitions_detects_opacity_change() {
        let mut engine = StyleEngine::default();
        engine.add_stylesheet(
            r#"
            div {
                opacity: 1.0;
                transition-property: opacity;
                transition-duration: 300ms;
                transition-timing-function: linear;
            }
            "#,
        );

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        // First restyle: establish initial opacity = 1.0.
        let mut map = engine.restyle_all(&doc);
        engine.apply_transitions(&mut map);

        // Treat the first frame as baseline state, not a user-visible transition:
        // drop any first-frame transitions but KEEP the recorded baseline values
        // so the subsequent opacity change is still detected.
        engine.transition_manager.borrow_mut().clear_running();

        assert!(
            !engine.has_running_transitions(),
            "no transition yet — first frame"
        );

        // Change opacity via inline style.
        doc.set_inline_style(div, "opacity", "0.5");
        let mut map = engine.restyle_all(&doc);
        engine.apply_transitions(&mut map);

        assert!(
            engine.has_running_transitions(),
            "transition should have started after opacity change"
        );

        // The transition should override the style map value.
        let style = map.get(div).expect("style exists");
        // At t=0 with linear timing, the interpolated value should be the old value (1.0).
        assert!(
            (style.opacity - 1.0).abs() < f32::EPSILON,
            "at t=0, opacity should still be 1.0 (from), got {}",
            style.opacity
        );

        // Tick halfway and re-apply.
        engine.tick_transitions(150.0);
        engine.apply_transitions(&mut map);
        let style = map.get(div).expect("style exists");
        assert!(
            (style.opacity - 0.75).abs() < 0.02,
            "at 150ms, opacity should be ~0.75, got {}",
            style.opacity
        );
    }
}
