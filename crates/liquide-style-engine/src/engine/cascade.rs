//! Cascade resolution and style computation for DOM trees.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use liquide_dom::{Document, NodeId};

use super::StyleEngine;
use super::content::consume_remaining_properties;
use crate::cascade::{CascadeDeclaration, CascadeMap, CascadePriority};
use crate::computed::*;
use crate::style_map::StyleMap;
use crate::value_resolve::parse_inline_value;

type ScopeVars = HashMap<String, liquide_theme_css::value::PropertyValue>;

/// Result of an incremental restyle operation.
///
/// Returned by [`StyleEngine::restyle_dirty`] to allow the caller to decide
/// whether downstream work (layout, paint) can be skipped.
#[derive(Debug, Clone)]
pub struct RestyleResult {
    /// Number of nodes whose computed style was recomputed.
    pub restyled_count: usize,
    /// Number of nodes that were skipped because they were clean.
    pub skipped_count: usize,
    /// `true` if at least one node's computed style was added or replaced.
    /// When `false`, the caller can safely skip layout — the style map is
    /// unchanged.
    pub had_changes: bool,
}

/// Maximum recursion depth for style computation to prevent stack overflow.
const MAX_STYLE_DEPTH: usize = 512;

thread_local! {
    static STYLE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// RAII guard that increments depth on creation and decrements on drop.
struct StyleDepthGuard;

impl StyleDepthGuard {
    fn try_enter() -> Option<Self> {
        STYLE_DEPTH.with(|d| {
            let current = d.get();
            if current >= MAX_STYLE_DEPTH {
                None
            } else {
                d.set(current + 1);
                Some(StyleDepthGuard)
            }
        })
    }
}

impl Drop for StyleDepthGuard {
    fn drop(&mut self) {
        STYLE_DEPTH.with(|d| d.set(d.get() - 1));
    }
}

thread_local! {
    /// Memoizes the full-grammar parse of an inline `prop: value` declaration.
    ///
    /// Inline styles are parsed per element that carries them, and chart/widget
    /// nodes can rewrite inline geometry/gradients every frame, so the
    /// lightningcss `parse_declaration` cost is paid repeatedly for identical
    /// `(prop, value)` strings. Cache the parsed `PropertySet` (or its absence)
    /// keyed by the declaration text. `None` records a value that has no
    /// full-grammar parse (so the single-value inline fallback should run).
    static INLINE_PARSE_CACHE: std::cell::RefCell<
        HashMap<(String, String), Option<liquide_theme_css::property::PropertySet>>,
    > = std::cell::RefCell::new(HashMap::new());
}

/// Maximum number of distinct `(prop, value)` inline declarations to remember.
/// Bounds memory for pathological churn (e.g. continuously unique inline values);
/// past the cap the cache is cleared and rebuilt rather than growing unbounded.
const INLINE_PARSE_CACHE_CAP: usize = 4096;

/// Add a node's inline styles to the cascade, parsing each declaration with the
/// REAL CSS property grammar (the same path stylesheet rules and `var()`
/// re-parsing use) so inline values produce the same typed `PropertyValue`s as
/// stylesheet values — i.e. inline `%`, `em`, `calc()`, gradients (linear/
/// radial/conic), box-shadow, etc. all resolve instead of collapsing to a
/// `Keyword` (and thence `Dimension::Auto`).
///
/// Mirrors the cascade priority of the previous single-value path
/// (`CascadePriority::inline(order)`) and preserves `!important` via
/// `add_properties`. Two fallbacks keep prior behaviour intact:
/// * Values containing `var(`/`env(` are NOT pre-expanded — they are kept as a
///   single `parse_inline_value` keyword so the existing apply-time var()
///   substitution + re-parse path (`apply.rs`) owns them (CSS pending-
///   substitution semantics, matching `CascadeMap::add_properties`).
/// * Declarations with no full-grammar parse fall back to the single-value
///   inline parser (numbers, colors, plain keywords).
fn add_inline_styles_to_cascade(
    inline_styles: &liquide_dom::attrs::AttributeMap,
    cascade: &mut CascadeMap,
) {
    let mut inline_order = 0u32;
    for (prop, value) in inline_styles.iter() {
        let priority = CascadePriority::inline(inline_order);
        inline_order += 1;

        // var()/env() must keep flowing through the apply-time resolution path.
        let trimmed = value.trim();
        if trimmed.contains("var(") || trimmed.contains("env(") {
            cascade.add(CascadeDeclaration {
                property: prop.to_string(),
                value: parse_inline_value(value),
                priority,
            });
            continue;
        }

        if let Some(props) = parse_inline_declaration_cached(prop, value) {
            cascade.add_properties(&props, priority);
        } else {
            // No full-grammar parse — keep the cheap single-value fallback.
            cascade.add(CascadeDeclaration {
                property: prop.to_string(),
                value: parse_inline_value(value),
                priority,
            });
        }
    }
}

/// Cached full-grammar parse of one inline declaration. Returns the parsed
/// `PropertySet` (cloned out of the cache) or `None` when the declaration has no
/// recognized full-grammar parse.
fn parse_inline_declaration_cached(
    prop: &str,
    value: &str,
) -> Option<liquide_theme_css::property::PropertySet> {
    INLINE_PARSE_CACHE.with(|cache| {
        let key = (prop.to_string(), value.to_string());
        if let Some(cached) = cache.borrow().get(&key) {
            return cached.clone();
        }
        let parsed = liquide_theme_css::ThemeParser::new().parse_declaration(prop, value);
        let mut map = cache.borrow_mut();
        if map.len() >= INLINE_PARSE_CACHE_CAP {
            map.clear();
        }
        map.insert(key, parsed.clone());
        parsed
    })
}

impl StyleEngine {
    /// Compute the style of a single node in isolation.
    ///
    /// This is a **limited helper**, not the full pipeline contract. It runs the
    /// same cascade, property application, post-cascade assembly
    /// (`assemble_text_decoration`/`assemble_background`/`assemble_mask`),
    /// logical→physical resolution, and transition/animation parsing as the tree
    /// path (`restyle_all`/`restyle_node`). It deliberately differs in two ways
    /// that require whole-tree/layout context the caller does not provide here:
    ///
    /// * **`@container` rules are skipped** — container condition evaluation
    ///   needs post-layout container sizes recorded on the `StyleMap`, which only
    ///   exists on the tree path.
    /// * **`var()` resolves against an empty scope** — scoped custom properties
    ///   are collected per-subtree during `restyle_node`; this helper has no
    ///   ancestor scope to draw from.
    ///
    /// Tests that depend on `@container` matching or scoped variables MUST use
    /// `restyle_all` and read the node's entry from the returned `StyleMap`.
    pub fn compute_style(&self, doc: &Document, node_id: NodeId) -> ComputedStyle {
        let _guard = match StyleDepthGuard::try_enter() {
            Some(g) => g,
            None => {
                tracing::warn!("style recursion depth limit reached at node {:?}", node_id);
                return ComputedStyle::default();
            }
        };

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
        let tag_name = node.tag_name();

        for sheet in &self.sheets {
            for rule_idx in sheet.candidate_indices(&tag_name) {
                let rule = &sheet.rules[rule_idx];
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
                // Skip pseudo-element rules — they apply to synthetic boxes,
                // not the element's own computed style.
                if prepared_rule_pseudo_element(rule).is_some() {
                    continue;
                }
                if rule.selector.matches(doc, node_id) {
                    let mut priority = CascadePriority::author(rule.specificity, rule.source_order);
                    priority.layer_order = rule.layer_order;
                    cascade.add_properties(&rule.properties, priority);
                }
            }
        }

        // Inline styles — parsed with the real CSS grammar (see
        // `add_inline_styles_to_cascade`).
        add_inline_styles_to_cascade(&node.inline_styles, &mut cascade);

        let resolved = cascade.resolve();
        let empty_scope = ScopeVars::new();
        let inherited_style = style.clone();
        for (prop, val) in &resolved {
            self.apply_cascaded_property(prop, val, &mut style, &inherited_style, &empty_scope);
        }

        // ── Post-cascade assembly — identical to the tree path so this helper
        //    matches `restyle_node` semantics for everything except @container /
        //    scoped var() (documented above). ──
        Self::assemble_text_decoration(&mut style);
        Self::assemble_background(&mut style);
        Self::assemble_mask(&mut style);
        Self::resolve_logical_properties(&mut style);
        consume_remaining_properties(&style);

        // ── Populate structured transition/animation defs from longhands ──
        style.transition = parse_transition_defs(&style);
        style.animation = parse_animation_defs(&style);

        style
    }

    pub fn restyle_all(&self, doc: &Document) -> StyleMap {
        let mut map = StyleMap::new();
        let scope = ScopeVars::new();
        self.restyle_node(doc, doc.root(), None, &mut map, &scope);
        map
    }

    pub fn restyle_subtree(&self, doc: &Document, node_id: NodeId, map: &mut StyleMap) {
        let parent_style = doc.parent(node_id).and_then(|pid| map.get(pid).cloned());
        let scope = self.inherited_scope_for_node(doc, node_id, map);
        self.restyle_node(doc, node_id, parent_style.as_deref(), map, &scope);
    }

    pub fn invalidate(&self, doc: &Document, changed_nodes: &[NodeId], map: &mut StyleMap) {
        for &node_id in changed_nodes {
            self.restyle_subtree(doc, node_id, map);
        }
    }

    /// Incrementally restyle only the nodes in the dirty set, plus any
    /// children whose inherited styles are affected by a parent change.
    ///
    /// For nodes not in `dirty.style` whose ancestors are also clean,
    /// the existing computed style in `map` is preserved, avoiding a
    /// full cascade recomputation.
    ///
    /// Returns a [`RestyleResult`] indicating how much work was done and
    /// whether any computed style actually changed, which the caller can
    /// use to skip layout when the result is unchanged.
    pub fn restyle_dirty(
        &self,
        doc: &Document,
        dirty: &liquide_dom::dirty::DirtySet,
        map: &mut StyleMap,
    ) -> RestyleResult {
        if dirty.style.is_empty() {
            return RestyleResult {
                restyled_count: 0,
                skipped_count: map.len(),
                had_changes: false,
            };
        }

        let mut result = RestyleResult {
            restyled_count: 0,
            skipped_count: 0,
            had_changes: false,
        };

        let scope = ScopeVars::new();
        self.restyle_dirty_walk(
            doc,
            doc.root(),
            None,
            map,
            &scope,
            &dirty.style,
            false,
            &mut result,
        );
        result
    }

    /// Recursive tree walk for incremental restyle.
    ///
    /// `force_restyle` is `true` when an ancestor was dirty and had its style
    /// recomputed — all descendants must be re-cascaded because inherited
    /// values may have changed.
    fn restyle_dirty_walk(
        &self,
        doc: &Document,
        node_id: NodeId,
        parent_style: Option<&ComputedStyle>,
        map: &mut StyleMap,
        scope_vars: &ScopeVars,
        dirty_nodes: &HashSet<NodeId>,
        force_restyle: bool,
        result: &mut RestyleResult,
    ) {
        let needs_restyle = force_restyle || dirty_nodes.contains(&node_id);

        // ── Fast path: node is clean and no ancestor forced a restyle ──
        if !needs_restyle {
            result.skipped_count += 1;
            // Still need to recurse into children — some descendants may be dirty.
            let existing_style = map.get(node_id).cloned();
            let children = doc.children(node_id).to_vec();
            for child_id in children {
                let is_shadow = doc
                    .get(child_id)
                    .map(|n| matches!(n.data, liquide_dom::node::NodeData::ShadowRoot))
                    .unwrap_or(false);
                let empty_scope = ScopeVars::new();
                let child_scope = if is_shadow { &empty_scope } else { scope_vars };
                self.restyle_dirty_walk(
                    doc,
                    child_id,
                    existing_style.as_deref(),
                    map,
                    child_scope,
                    dirty_nodes,
                    false,
                    result,
                );
            }
            return;
        }

        // ── Slow path: recompute this node's style via full cascade ──
        // This mirrors restyle_node() but tracks whether anything changed.
        let _guard = match StyleDepthGuard::try_enter() {
            Some(g) => g,
            None => {
                tracing::warn!(
                    "restyle_dirty recursion depth limit reached at node {:?}",
                    node_id
                );
                return;
            }
        };

        let node = match doc.get(node_id) {
            Some(n) => n,
            None => return,
        };

        let mut style = ComputedStyle::default();
        if let Some(ps) = parent_style {
            style.inherit_from(ps);
        }

        if !node.is_text() {
            let inherited_scope_storage;
            let inherited_scope = if force_restyle {
                scope_vars
            } else {
                inherited_scope_storage = self.inherited_scope_for_node(doc, node_id, map);
                &inherited_scope_storage
            };

            let mut cascade = CascadeMap::new();
            let tag_name = node.tag_name();

            for sheet in &self.sheets {
                for rule_idx in sheet.candidate_indices(&tag_name) {
                    let rule = &sheet.rules[rule_idx];
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
                    if prepared_rule_pseudo_element(rule).is_some() {
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

            add_inline_styles_to_cascade(&node.inline_styles, &mut cascade);

            let resolved = cascade.resolve();

            let owned_vars = self.build_local_variable_scope(&resolved, inherited_scope);
            let effective_vars = owned_vars.as_ref().unwrap_or(inherited_scope);
            let inherited_style = style.clone();

            for (prop, val) in &resolved {
                self.apply_cascaded_property(
                    prop,
                    val,
                    &mut style,
                    &inherited_style,
                    effective_vars,
                );
            }

            Self::assemble_text_decoration(&mut style);
            Self::assemble_background(&mut style);
            Self::assemble_mask(&mut style);
            Self::resolve_logical_properties(&mut style);
            consume_remaining_properties(&style);

            style.transition = parse_transition_defs(&style);
            style.animation = parse_animation_defs(&style);

            let style = Arc::new(style);
            result.restyled_count += 1;
            result.had_changes = true;
            map.insert_shared(node_id, style.clone());
            let child_vars = owned_vars.as_ref().unwrap_or(inherited_scope);
            self.compute_pseudo_styles(doc, node_id, &style, map, child_vars);

            // All children must be restyled since this node's style changed.
            let children = doc.children(node_id).to_vec();
            for child_id in children {
                let empty_scope = ScopeVars::new();
                let is_shadow = doc
                    .get(child_id)
                    .map(|n| matches!(n.data, liquide_dom::node::NodeData::ShadowRoot))
                    .unwrap_or(false);
                let child_scope = if is_shadow { &empty_scope } else { child_vars };
                self.restyle_dirty_walk(
                    doc,
                    child_id,
                    Some(&style),
                    map,
                    child_scope,
                    dirty_nodes,
                    true,
                    result,
                );
            }
            return;
        }

        // Text node path
        Self::assemble_text_decoration(&mut style);
        Self::assemble_background(&mut style);
        Self::assemble_mask(&mut style);
        Self::resolve_logical_properties(&mut style);
        consume_remaining_properties(&style);

        style.transition = parse_transition_defs(&style);
        style.animation = parse_animation_defs(&style);

        let style = Arc::new(style);
        result.restyled_count += 1;
        result.had_changes = true;
        map.insert_shared(node_id, style.clone());

        let children = doc.children(node_id).to_vec();
        for child_id in children {
            let is_shadow = doc
                .get(child_id)
                .map(|n| matches!(n.data, liquide_dom::node::NodeData::ShadowRoot))
                .unwrap_or(false);
            if is_shadow {
                self.restyle_dirty_walk(
                    doc,
                    child_id,
                    Some(&style),
                    map,
                    &ScopeVars::new(),
                    dirty_nodes,
                    true,
                    result,
                );
            } else {
                self.restyle_dirty_walk(
                    doc,
                    child_id,
                    Some(&style),
                    map,
                    scope_vars,
                    dirty_nodes,
                    true,
                    result,
                );
            }
        }
    }

    fn restyle_node(
        &self,
        doc: &Document,
        node_id: NodeId,
        parent_style: Option<&ComputedStyle>,
        map: &mut StyleMap,
        scope_vars: &ScopeVars,
    ) {
        let _guard = match StyleDepthGuard::try_enter() {
            Some(g) => g,
            None => {
                tracing::warn!(
                    "restyle recursion depth limit reached at node {:?}",
                    node_id
                );
                return;
            }
        };

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
            let tag_name = node.tag_name();

            // Collect matching rules and add to cascade with proper priority.
            // Only check rules whose key selector tag matches this node's tag
            // (plus universal/class-only rules), skipping all others.
            for sheet in &self.sheets {
                for rule_idx in sheet.candidate_indices(&tag_name) {
                    let rule = &sheet.rules[rule_idx];
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
                    if prepared_rule_pseudo_element(rule).is_some() {
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

            // Add inline styles with highest author priority, parsed with the
            // real CSS grammar (see `add_inline_styles_to_cascade`).
            add_inline_styles_to_cascade(&node.inline_styles, &mut cascade);

            // Resolve the cascade and apply winners
            let resolved = cascade.resolve();

            // Extract scoped CSS variables from the resolved cascade.
            // Respect @property `inherits` flag: non-inheriting custom properties
            // that aren't explicitly set on this element get their initial value
            // instead of inheriting from the parent scope.
            //
            // Defer cloning scope_vars until we actually need to modify it
            // (i.e. when custom properties or @property rules are present).
            let owned_vars = self.build_local_variable_scope(&resolved, scope_vars);
            let effective_vars = owned_vars.as_ref().unwrap_or(scope_vars);
            let inherited_style = style.clone();

            for (prop, val) in &resolved {
                self.apply_cascaded_property(
                    prop,
                    val,
                    &mut style,
                    &inherited_style,
                    effective_vars,
                );
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

            // Populate structured transition/animation defs from longhands
            style.transition = parse_transition_defs(&style);
            style.animation = parse_animation_defs(&style);

            let style = Arc::new(style);
            map.insert_shared(node_id, style.clone());
            let child_vars = owned_vars.as_ref().unwrap_or(scope_vars);
            self.compute_pseudo_styles(doc, node_id, &style, map, child_vars);

            // Recurse into children with scoped variables
            let children = doc.children(node_id).to_vec();
            for child_id in children {
                let empty_scope = ScopeVars::new();
                let is_shadow = doc
                    .get(child_id)
                    .map(|n| matches!(n.data, liquide_dom::node::NodeData::ShadowRoot))
                    .unwrap_or(false);
                let child_scope = if is_shadow { &empty_scope } else { child_vars };
                self.restyle_node(doc, child_id, Some(&style), map, child_scope);
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

        // Populate structured transition/animation defs from longhands
        style.transition = parse_transition_defs(&style);
        style.animation = parse_animation_defs(&style);

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
                self.restyle_node(doc, child_id, Some(&style), map, &ScopeVars::new());
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
        scope_vars: &ScopeVars,
    ) {
        use crate::style_map::PseudoKind;

        let tag_name = doc.get(node_id).map(|n| n.tag_name()).unwrap_or_default();
        for (pseudo_name, kind) in [("before", PseudoKind::Before), ("after", PseudoKind::After)] {
            let mut cascade = CascadeMap::new();
            let mut has_rules = false;

            for sheet in &self.sheets {
                for rule_idx in sheet.candidate_indices(&tag_name) {
                    let rule = &sheet.rules[rule_idx];
                    // Only consider rules targeting this pseudo-element.
                    if prepared_rule_pseudo_element(rule) != Some(pseudo_name) {
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
            let owned_vars = self.build_local_variable_scope(&resolved, scope_vars);
            let effective_vars = owned_vars.as_ref().unwrap_or(scope_vars);

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
            let inherited_style = pseudo_style.clone();

            for (prop, val) in &resolved {
                self.apply_cascaded_property(
                    prop,
                    val,
                    &mut pseudo_style,
                    &inherited_style,
                    effective_vars,
                );
            }

            map.insert_pseudo(node_id, kind, Arc::new(pseudo_style));
        }

        // ── ::first-line and ::first-letter pseudo-elements ─────────────
        //
        // Unlike ::before/::after, these do not require a `content` property.
        // ::first-line: allowed properties — font, color, background,
        //   text-decoration, letter-spacing, word-spacing, line-height,
        //   text-transform.
        // ::first-letter: all ::first-line properties plus margin, padding,
        //   border, float.
        for (pseudo_name, kind) in [
            ("first-line", PseudoKind::FirstLine),
            ("first-letter", PseudoKind::FirstLetter),
        ] {
            let mut cascade = CascadeMap::new();
            let mut has_rules = false;

            for sheet in &self.sheets {
                for rule_idx in sheet.candidate_indices(&tag_name) {
                    let rule = &sheet.rules[rule_idx];
                    if prepared_rule_pseudo_element(rule) != Some(pseudo_name) {
                        continue;
                    }
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

            let mut resolved = cascade.resolve();

            // Build the local variable scope from the FULL resolved set BEFORE
            // filtering, so custom properties declared on the pseudo-element
            // (e.g. `--accent`) are available to `var()` in allowed properties.
            // (TODO 19)
            let owned_vars = self.build_local_variable_scope(&resolved, scope_vars);
            let effective_vars = owned_vars.as_ref().unwrap_or(scope_vars);

            // Filter to spec-allowed properties for each pseudo-element. Custom
            // properties are kept in the scope above but not applied directly.
            resolved.retain(|(prop, _)| is_allowed_pseudo_property(pseudo_name, prop));

            if resolved.is_empty() {
                continue;
            }

            let mut pseudo_style = ComputedStyle::default();
            pseudo_style.inherit_from(host_style);
            let inherited_style = pseudo_style.clone();

            for (prop, val) in &resolved {
                self.apply_cascaded_property(
                    prop,
                    val,
                    &mut pseudo_style,
                    &inherited_style,
                    effective_vars,
                );
            }

            map.insert_pseudo(node_id, kind, Arc::new(pseudo_style));
        }
    }

    fn build_local_variable_scope(
        &self,
        resolved: &[(String, liquide_theme_css::value::PropertyValue)],
        inherited_scope: &ScopeVars,
    ) -> Option<ScopeVars> {
        let has_custom_props = resolved
            .iter()
            .any(|(property, _)| property.starts_with("--"));
        let needs_property_overrides = !self.registered_properties.is_empty();
        if !has_custom_props && !needs_property_overrides {
            return None;
        }

        let mut local_vars = inherited_scope.clone();
        let mut explicitly_set = HashSet::new();
        for (property, value) in resolved {
            if property.starts_with("--") {
                local_vars.insert(property.clone(), value.clone());
                explicitly_set.insert(property.clone());
            }
        }

        for (name, def) in &self.registered_properties {
            if !def.inherits && !explicitly_set.contains(name) {
                if let Some(ref initial) = def.initial_value {
                    local_vars.insert(
                        name.clone(),
                        liquide_theme_css::value::PropertyValue::Keyword(initial.clone()),
                    );
                } else {
                    local_vars.remove(name);
                }
            } else if !local_vars.contains_key(name) {
                if let Some(ref initial) = def.initial_value {
                    local_vars.insert(
                        name.clone(),
                        liquide_theme_css::value::PropertyValue::Keyword(initial.clone()),
                    );
                }
            }
        }

        Some(local_vars)
    }

    fn resolve_scope_for_node(
        &self,
        doc: &Document,
        node_id: NodeId,
        map: &StyleMap,
        inherited_scope: &ScopeVars,
    ) -> ScopeVars {
        let node = match doc.get(node_id) {
            Some(node) if !node.is_text() => node,
            _ => return inherited_scope.clone(),
        };

        let mut cascade = CascadeMap::new();
        let tag_name = node.tag_name();

        for sheet in &self.sheets {
            for rule_idx in sheet.candidate_indices(&tag_name) {
                let rule = &sheet.rules[rule_idx];
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
                if prepared_rule_pseudo_element(rule).is_some() {
                    continue;
                }
                if rule.selector.matches(doc, node_id) {
                    let mut priority = CascadePriority::author(rule.specificity, rule.source_order);
                    priority.layer_order = rule.layer_order;
                    cascade.add_properties(&rule.properties, priority);
                }
            }
        }

        add_inline_styles_to_cascade(&node.inline_styles, &mut cascade);

        let resolved = cascade.resolve();
        self.build_local_variable_scope(&resolved, inherited_scope)
            .unwrap_or_else(|| inherited_scope.clone())
    }

    fn inherited_scope_for_node(
        &self,
        doc: &Document,
        node_id: NodeId,
        map: &StyleMap,
    ) -> ScopeVars {
        if doc
            .get(node_id)
            .map(|node| matches!(node.data, liquide_dom::node::NodeData::ShadowRoot))
            .unwrap_or(false)
        {
            return ScopeVars::new();
        }

        let mut scope = ScopeVars::new();
        let mut ancestors = doc.ancestors(node_id);
        ancestors.reverse();
        for ancestor_id in ancestors {
            if doc
                .get(ancestor_id)
                .map(|node| matches!(node.data, liquide_dom::node::NodeData::ShadowRoot))
                .unwrap_or(false)
            {
                scope.clear();
            }
            scope = self.resolve_scope_for_node(doc, ancestor_id, map, &scope);
        }
        scope
    }
}

/// Check whether a CSS property is allowed on `::first-line` or `::first-letter`.
///
/// Per CSS Pseudo-Elements Module Level 4:
/// - `::first-line` allowed: font-*, color, background-*, text-decoration-*,
///   letter-spacing, word-spacing, line-height, text-transform, vertical-align.
/// - `::first-letter` allowed: everything in `::first-line` plus margin-*,
///   padding-*, border-*, float.
fn is_allowed_pseudo_property(pseudo_name: &str, property: &str) -> bool {
    // Properties allowed for both ::first-line and ::first-letter
    let first_line_ok = property.starts_with("--")
        || property.starts_with("font")
        || property == "color"
        || property.starts_with("background")
        || property.starts_with("text-decoration")
        || property == "letter-spacing"
        || property == "word-spacing"
        || property == "line-height"
        || property == "text-transform"
        || property == "vertical-align";

    match pseudo_name {
        "first-line" => first_line_ok,
        "first-letter" => {
            first_line_ok
                || property.starts_with("margin")
                || property.starts_with("padding")
                || property.starts_with("border")
                || property == "float"
        }
        _ => true,
    }
}

fn prepared_rule_pseudo_element(rule: &super::PreparedRule) -> Option<&str> {
    rule.pseudo_element.as_deref().or_else(|| {
        rule.selector
            .compounds
            .first()
            .and_then(|compound| compound.pseudo_element.as_ref())
            .map(|pseudo| match pseudo {
                crate::selector::PseudoElement::Before => "before",
                crate::selector::PseudoElement::After => "after",
                crate::selector::PseudoElement::FirstLine => "first-line",
                crate::selector::PseudoElement::FirstLetter => "first-letter",
                crate::selector::PseudoElement::Placeholder => "placeholder",
                crate::selector::PseudoElement::Selection => "selection",
            })
    })
}

/// Parse transition longhand strings into structured TransitionDef vec.
fn parse_transition_defs(style: &ComputedStyle) -> Vec<TransitionDef> {
    let properties: Vec<&str> = style
        .transition_property
        .as_deref()
        .unwrap_or("all")
        .split(',')
        .map(str::trim)
        .collect();
    let durations: Vec<&str> = style
        .transition_duration
        .as_deref()
        .unwrap_or("0s")
        .split(',')
        .map(str::trim)
        .collect();
    let timings: Vec<&str> = style
        .transition_timing_function
        .as_deref()
        .unwrap_or("ease")
        .split(',')
        .map(str::trim)
        .collect();
    let delays: Vec<&str> = style
        .transition_delay
        .as_deref()
        .unwrap_or("0s")
        .split(',')
        .map(str::trim)
        .collect();

    let count = properties.len();
    let mut defs = Vec::with_capacity(count);
    for i in 0..count {
        if properties[i] == "none" {
            continue;
        }
        defs.push(TransitionDef {
            property: properties[i].to_string(),
            duration_ms: parse_time_ms(
                durations
                    .get(i)
                    .copied()
                    .unwrap_or(durations.last().copied().unwrap_or("0s")),
            ),
            timing_function: parse_timing_function(
                timings
                    .get(i)
                    .copied()
                    .unwrap_or(timings.last().copied().unwrap_or("ease")),
            ),
            delay_ms: parse_time_ms(
                delays
                    .get(i)
                    .copied()
                    .unwrap_or(delays.last().copied().unwrap_or("0s")),
            ),
        });
    }
    defs
}

/// Parse animation longhand strings into structured AnimationDef vec.
fn parse_animation_defs(style: &ComputedStyle) -> Vec<AnimationDef> {
    let names_str = style.animation_name.as_deref().unwrap_or("none");
    if names_str == "none" || names_str.is_empty() {
        return Vec::new();
    }

    let names: Vec<&str> = names_str.split(',').map(str::trim).collect();
    let durations: Vec<&str> = style
        .animation_duration
        .as_deref()
        .unwrap_or("0s")
        .split(',')
        .map(str::trim)
        .collect();
    let timings: Vec<&str> = style
        .animation_timing_function
        .as_deref()
        .unwrap_or("ease")
        .split(',')
        .map(str::trim)
        .collect();
    let delays: Vec<&str> = style
        .animation_delay
        .as_deref()
        .unwrap_or("0s")
        .split(',')
        .map(str::trim)
        .collect();

    let count = names.len();
    let mut defs = Vec::with_capacity(count);
    for i in 0..count {
        let name = names[i];
        if name == "none" {
            continue;
        }
        defs.push(AnimationDef {
            name: name.to_string(),
            duration_ms: parse_time_ms(
                durations
                    .get(i)
                    .copied()
                    .unwrap_or(durations.last().copied().unwrap_or("0s")),
            ),
            timing_function: parse_timing_function(
                timings
                    .get(i)
                    .copied()
                    .unwrap_or(timings.last().copied().unwrap_or("ease")),
            ),
            delay_ms: parse_time_ms(
                delays
                    .get(i)
                    .copied()
                    .unwrap_or(delays.last().copied().unwrap_or("0s")),
            ),
            iteration_count: style.animation_iteration_count.clone(),
            direction: style.animation_direction,
            fill_mode: style.animation_fill_mode,
            play_state: style.animation_play_state,
        });
    }
    defs
}

/// Parse a CSS time string like "0.3s" or "300ms" to milliseconds.
fn parse_time_ms(s: &str) -> f32 {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        ms.trim().parse::<f32>().unwrap_or(0.0)
    } else if let Some(sec) = s.strip_suffix('s') {
        sec.trim().parse::<f32>().unwrap_or(0.0) * 1000.0
    } else {
        s.parse::<f32>().unwrap_or(0.0)
    }
}

/// Parse a CSS timing function keyword to the enum.
fn parse_timing_function(s: &str) -> TimingFunction {
    match s.trim() {
        "linear" => TimingFunction::Linear,
        "ease" => TimingFunction::Ease,
        "ease-in" => TimingFunction::EaseIn,
        "ease-out" => TimingFunction::EaseOut,
        "ease-in-out" => TimingFunction::EaseInOut,
        "step-start" => TimingFunction::Steps(1, StepPosition::JumpStart),
        "step-end" => TimingFunction::Steps(1, StepPosition::JumpEnd),
        other => {
            // Try cubic-bezier(x1, y1, x2, y2)
            if let Some(inner) = other
                .strip_prefix("cubic-bezier(")
                .and_then(|s| s.strip_suffix(')'))
            {
                let nums: Vec<f32> = inner
                    .split(',')
                    .filter_map(|n| n.trim().parse().ok())
                    .collect();
                if nums.len() == 4 {
                    return TimingFunction::CubicBezier(nums[0], nums[1], nums[2], nums[3]);
                }
            }
            // Try steps(n, position)
            if let Some(inner) = other
                .strip_prefix("steps(")
                .and_then(|s| s.strip_suffix(')'))
            {
                let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                let n = parts
                    .first()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let pos = match parts.get(1).map(|s| *s) {
                    Some("jump-start" | "start") => StepPosition::JumpStart,
                    Some("jump-end" | "end") => StepPosition::JumpEnd,
                    Some("jump-none") => StepPosition::JumpNone,
                    Some("jump-both") => StepPosition::JumpBoth,
                    _ => StepPosition::JumpEnd,
                };
                return TimingFunction::Steps(n, pos);
            }
            TimingFunction::Ease
        }
    }
}
