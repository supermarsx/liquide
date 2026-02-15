//! The style engine — orchestrates cascade, specificity, inheritance, and variable resolution.

use std::sync::Arc;

use liquide_compositor::pixel::Color;
use liquide_dom::{Document, NodeId};
use liquide_theme_css::property::PropertySet;
use liquide_theme_css::ThemeParser;

use crate::computed::*;
use crate::dimension::{Dimension, Sides};
use crate::inheritance;
use crate::selector::ComplexSelector;
use crate::specificity::Specificity;
use crate::style_map::StyleMap;
use crate::value_resolve::*;

/// A prepared stylesheet rule ready for matching.
#[derive(Debug)]
pub struct PreparedRule {
    pub selector: ComplexSelector,
    pub specificity: Specificity,
    pub source_order: u32,
    pub properties: PropertySet,
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
}

impl StyleEngine {
    /// Create a new style engine.
    pub fn new(viewport: ViewportSize, base_font_size: f32) -> Self {
        Self {
            sheets: Vec::new(),
            viewport,
            base_font_size,
            variables: std::collections::HashMap::new(),
        }
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

        // Extract variables
        for rule in stylesheet.rules() {
            for (key, val) in rule.properties.iter() {
                if key.starts_with("--") {
                    self.variables.insert(key.clone(), val.clone());
                }
            }
        }

        // Compile rules
        let mut prepared_rules = Vec::new();
        let mut order = self.sheets.iter().map(|s| s.rules.len() as u32).sum::<u32>();

        for rule in stylesheet.rules() {
            let selector_str = format!(
                "{}{}{}{}",
                &rule.selector.element,
                rule.selector
                    .classes
                    .iter()
                    .map(|c| format!(".{}", c))
                    .collect::<String>(),
                rule.selector
                    .id
                    .as_ref()
                    .map(|id| format!("#{}", id))
                    .unwrap_or_default(),
                rule.selector
                    .pseudo_classes
                    .iter()
                    .map(|p| format!(":{}", p))
                    .collect::<String>(),
            );

            if let Some(complex) = ComplexSelector::parse(&selector_str) {
                let specificity = complex.specificity();
                prepared_rules.push(PreparedRule {
                    selector: complex,
                    specificity,
                    source_order: order,
                    properties: rule.properties.clone(),
                });
                order += 1;
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

        // Collect matching rules sorted by (specificity, source_order)
        let mut matching: Vec<&PreparedRule> = Vec::new();
        for sheet in &self.sheets {
            for rule in &sheet.rules {
                if rule.selector.matches(doc, node_id) {
                    matching.push(rule);
                }
            }
        }

        // Sort: lower specificity first, so later (higher) overwrites
        matching.sort_by(|a, b| {
            a.specificity
                .cmp(&b.specificity)
                .then(a.source_order.cmp(&b.source_order))
        });

        // Apply rules in cascade order (lowest specificity first)
        for rule in &matching {
            self.apply_properties(&rule.properties, &mut style);
        }

        style
    }

    /// Compute styles for the entire document tree.
    pub fn restyle_all(&self, doc: &Document) -> StyleMap {
        let mut map = StyleMap::new();
        self.restyle_node(doc, doc.root(), None, &mut map);
        map
    }

    /// Compute styles for a subtree (incremental).
    pub fn restyle_subtree(&self, doc: &Document, node_id: NodeId, map: &mut StyleMap) {
        let parent_style = doc
            .parent(node_id)
            .and_then(|pid| map.get(pid).cloned());
        self.restyle_node(doc, node_id, parent_style.as_deref(), map);
    }

    fn restyle_node(
        &self,
        doc: &Document,
        node_id: NodeId,
        parent_style: Option<&ComputedStyle>,
        map: &mut StyleMap,
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
            // Collect matching rules
            let mut matching: Vec<&PreparedRule> = Vec::new();
            for sheet in &self.sheets {
                for rule in &sheet.rules {
                    if rule.selector.matches(doc, node_id) {
                        matching.push(rule);
                    }
                }
            }
            matching.sort_by(|a, b| {
                a.specificity
                    .cmp(&b.specificity)
                    .then(a.source_order.cmp(&b.source_order))
            });
            for rule in &matching {
                self.apply_properties(&rule.properties, &mut style);
            }
        }

        let style = Arc::new(style);
        map.insert_shared(node_id, style.clone());

        // Recurse into children
        let children = doc.children(node_id).to_vec();
        for child_id in children {
            self.restyle_node(doc, child_id, Some(&style), map);
        }
    }

    /// Update viewport size (triggers re-resolution of viewport units).
    pub fn set_viewport(&mut self, size: ViewportSize) {
        self.viewport = size;
    }

    /// Resolve a CSS variable value.
    pub fn resolve_variable(
        &self,
        name: &str,
    ) -> Option<&liquide_theme_css::value::PropertyValue> {
        self.variables.get(name)
    }

    /// Total number of compiled rules.
    pub fn rule_count(&self) -> usize {
        self.sheets.iter().map(|s| s.rules.len()).sum()
    }

    // ── Private: apply property values to ComputedStyle ──

    fn apply_properties(&self, properties: &PropertySet, style: &mut ComputedStyle) {
        for (key, val) in properties.iter() {
            self.apply_single_property(key, val, style);
        }
    }

    fn apply_single_property(
        &self,
        key: &str,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
    ) {
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
            "border-top-color" => { if let Some(c) = resolve_color(val) { style.border_color.top = c; } }
            "border-right-color" => { if let Some(c) = resolve_color(val) { style.border_color.right = c; } }
            "border-bottom-color" => { if let Some(c) = resolve_color(val) { style.border_color.bottom = c; } }
            "border-left-color" => { if let Some(c) = resolve_color(val) { style.border_color.left = c; } }

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
                    style.box_shadow = shadows.iter().map(|s| {
                        liquide_compositor::scene::BoxShadowSpec {
                            offset_x: s.offset_x,
                            offset_y: s.offset_y,
                            blur_radius: s.blur_radius,
                            spread_radius: s.spread_radius,
                            color: Color { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a },
                            inset: s.inset,
                        }
                    }).collect();
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

            // Typography
            "color" => {
                if let Some(c) = resolve_color(val) {
                    style.color = c;
                }
            }
            "font-family" => {
                if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_family = s.split(',').map(|f| f.trim().trim_matches('"').to_string()).collect();
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
                    liquide_theme_css::value::PropertyValue::Length(lu) => LineHeight::Px(lu.to_px(16.0)),
                    liquide_theme_css::value::PropertyValue::Keyword(kw) if kw == "normal" => LineHeight::Normal,
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
                    style.mix_blend_mode = match kw.as_str() {
                        "multiply" => liquide_compositor::pixel::BlendMode::Multiply,
                        "screen" => liquide_compositor::pixel::BlendMode::Screen,
                        "overlay" => liquide_compositor::pixel::BlendMode::Overlay,
                        "darken" => liquide_compositor::pixel::BlendMode::Darken,
                        "lighten" => liquide_compositor::pixel::BlendMode::Lighten,
                        _ => liquide_compositor::pixel::BlendMode::SrcOver,
                    };
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
                        style.box_shadow.push(liquide_compositor::scene::BoxShadowSpec {
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
                    style.x_custom.push(("titlebar-background".into(), format!(
                        "rgba({},{},{},{})", c.r, c.g, c.b, c.a
                    )));
                }
            }

            _ => {
                // Unknown property — silently ignore
            }
        }
    }
}

impl Default for StyleEngine {
    fn default() -> Self {
        Self::new(ViewportSize::default(), 16.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;

    #[test]
    fn empty_engine() {
        let engine = StyleEngine::default();
        let doc = Document::new();
        let style = engine.compute_style(&doc, doc.root());
        assert_eq!(style.display, Display::Block);
    }

    #[test]
    fn basic_style_computation() {
        let mut engine = StyleEngine::default();
        engine.add_stylesheet(
            r#"
            div {
                display: flex;
                width: 100px;
                color: red;
            }
            "#,
        );

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let style = engine.compute_style(&doc, div);
        assert_eq!(style.display, Display::Flex);
    }

    #[test]
    fn restyle_all() {
        let mut engine = StyleEngine::default();
        engine.add_stylesheet(
            r#"
            statusbar {
                display: flex;
                position: fixed;
                height: 28px;
            }
            dock {
                display: flex;
                gap: 4px;
            }
            "#,
        );

        let mut doc = Document::new();
        let root = doc.root();
        let bar = doc.create_element("statusbar");
        let dock = doc.create_element("dock");
        doc.append_child(root, bar);
        doc.append_child(root, dock);

        let map = engine.restyle_all(&doc);
        let bar_style = map.get(bar).unwrap();
        assert_eq!(bar_style.display, Display::Flex);
        assert_eq!(bar_style.position, Position::Fixed);

        let dock_style = map.get(dock).unwrap();
        assert_eq!(dock_style.display, Display::Flex);
    }
}
