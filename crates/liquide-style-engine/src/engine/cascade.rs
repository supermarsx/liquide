//! Cascade resolution and style computation for DOM trees.

use std::sync::Arc;

use liquide_dom::{Document, NodeId};

use super::StyleEngine;
use super::content::consume_remaining_properties;
use crate::cascade::{CascadeDeclaration, CascadeMap, CascadePriority};
use crate::computed::*;
use crate::style_map::StyleMap;
use crate::value_resolve::parse_inline_value;

impl StyleEngine {
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

        // ── Populate structured transition/animation defs from longhands ──
        style.transition = parse_transition_defs(&style);
        style.animation = parse_animation_defs(&style);

        style
    }

    pub fn restyle_all(&self, doc: &Document) -> StyleMap {
        let mut map = StyleMap::new();
        let scope = std::collections::HashMap::new();
        self.restyle_node(doc, doc.root(), None, &mut map, &scope);
        map
    }

    pub fn restyle_subtree(&self, doc: &Document, node_id: NodeId, map: &mut StyleMap) {
        let parent_style = doc.parent(node_id).and_then(|pid| map.get(pid).cloned());
        let scope = std::collections::HashMap::new();
        self.restyle_node(doc, node_id, parent_style.as_deref(), map, &scope);
    }

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
            //
            // Defer cloning scope_vars until we actually need to modify it
            // (i.e. when custom properties or @property rules are present).
            let has_custom_props = resolved.iter().any(|(p, _)| p.starts_with("--"));
            let needs_property_overrides = !self.registered_properties.is_empty();
            let needs_local_vars = has_custom_props || needs_property_overrides;

            // Use Cow-like pattern: only clone when modifications are needed.
            let mut owned_vars: Option<std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>> = None;

            if needs_local_vars {
                let local_vars = owned_vars.insert(scope_vars.clone());

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
            }

            let effective_vars = owned_vars.as_ref().unwrap_or(scope_vars);

            for (prop, val) in &resolved {
                self.apply_single_property(prop, val, &mut style, effective_vars);
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
                self.restyle_node(doc, child_id, Some(&style), map, child_vars);
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

        let tag_name = doc.get(node_id).map(|n| n.tag_name()).unwrap_or_default();
        for (pseudo_name, kind) in [("before", PseudoKind::Before), ("after", PseudoKind::After)] {
            let mut cascade = CascadeMap::new();
            let mut has_rules = false;

            for sheet in &self.sheets {
                for rule_idx in sheet.candidate_indices(&tag_name) {
                    let rule = &sheet.rules[rule_idx];
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
}

/// Parse transition longhand strings into structured TransitionDef vec.
fn parse_transition_defs(style: &ComputedStyle) -> Vec<TransitionDef> {
    let properties: Vec<&str> = style.transition_property.as_deref()
        .unwrap_or("all").split(',').map(str::trim).collect();
    let durations: Vec<&str> = style.transition_duration.as_deref()
        .unwrap_or("0s").split(',').map(str::trim).collect();
    let timings: Vec<&str> = style.transition_timing_function.as_deref()
        .unwrap_or("ease").split(',').map(str::trim).collect();
    let delays: Vec<&str> = style.transition_delay.as_deref()
        .unwrap_or("0s").split(',').map(str::trim).collect();

    let count = properties.len();
    let mut defs = Vec::with_capacity(count);
    for i in 0..count {
        if properties[i] == "none" { continue; }
        defs.push(TransitionDef {
            property: properties[i].to_string(),
            duration_ms: parse_time_ms(durations.get(i).copied().unwrap_or(durations.last().copied().unwrap_or("0s"))),
            timing_function: parse_timing_function(timings.get(i).copied().unwrap_or(timings.last().copied().unwrap_or("ease"))),
            delay_ms: parse_time_ms(delays.get(i).copied().unwrap_or(delays.last().copied().unwrap_or("0s"))),
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
    let durations: Vec<&str> = style.animation_duration.as_deref()
        .unwrap_or("0s").split(',').map(str::trim).collect();
    let timings: Vec<&str> = style.animation_timing_function.as_deref()
        .unwrap_or("ease").split(',').map(str::trim).collect();
    let delays: Vec<&str> = style.animation_delay.as_deref()
        .unwrap_or("0s").split(',').map(str::trim).collect();

    let count = names.len();
    let mut defs = Vec::with_capacity(count);
    for i in 0..count {
        let name = names[i];
        if name == "none" { continue; }
        defs.push(AnimationDef {
            name: name.to_string(),
            duration_ms: parse_time_ms(durations.get(i).copied().unwrap_or(durations.last().copied().unwrap_or("0s"))),
            timing_function: parse_timing_function(timings.get(i).copied().unwrap_or(timings.last().copied().unwrap_or("ease"))),
            delay_ms: parse_time_ms(delays.get(i).copied().unwrap_or(delays.last().copied().unwrap_or("0s"))),
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
            if let Some(inner) = other.strip_prefix("cubic-bezier(").and_then(|s| s.strip_suffix(')')) {
                let nums: Vec<f32> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if nums.len() == 4 {
                    return TimingFunction::CubicBezier(nums[0], nums[1], nums[2], nums[3]);
                }
            }
            // Try steps(n, position)
            if let Some(inner) = other.strip_prefix("steps(").and_then(|s| s.strip_suffix(')')) {
                let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                let n = parts.first().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
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
