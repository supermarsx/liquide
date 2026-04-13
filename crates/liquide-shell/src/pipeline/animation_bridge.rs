//! Animation & transition bridge — detects triggers, ticks engines,
//! and applies interpolated values back onto the `StyleMap`.

use std::collections::HashMap;
use std::sync::Arc;

use liquide_animation::{CubicBezier, EasingFunction, RunningAnimation};
use liquide_animation::scheduler::{
    AnimationState, Direction, FillMode, IterationCount,
};
use liquide_dom::NodeId;
use liquide_style_engine::computed::{
    AnimationDirection, AnimationFillMode, AnimationIterationCount,
    AnimationPlayState, ComputedStyle, LineHeight,
};
use liquide_style_engine::Dimension;
use liquide_style_engine::StyleMap;

use super::DesktopPipeline;

// ── Parsing helpers ─────────────────────────────────────────────────────

/// Parse a CSS timing-function string into an `EasingFunction`.
fn parse_timing_function(s: &str) -> EasingFunction {
    match s.trim() {
        "linear" => EasingFunction::Linear,
        "ease" => EasingFunction::Ease,
        "ease-in" => EasingFunction::EaseIn,
        "ease-out" => EasingFunction::EaseOut,
        "ease-in-out" => EasingFunction::EaseInOut,
        s if s.starts_with("cubic-bezier(") => {
            let inner = s.trim_start_matches("cubic-bezier(").trim_end_matches(')');
            let parts: Vec<f32> = inner
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if parts.len() == 4 {
                EasingFunction::CubicBezier(CubicBezier::new(
                    parts[0], parts[1], parts[2], parts[3],
                ))
            } else {
                EasingFunction::Ease
            }
        }
        s if s.starts_with("steps(") => {
            let inner = s.trim_start_matches("steps(").trim_end_matches(')');
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            let count = parts.first().and_then(|p| p.parse().ok()).unwrap_or(1);
            let jump_start = parts
                .get(1)
                .is_some_and(|p| *p == "start" || *p == "jump-start");
            EasingFunction::Steps { count, jump_start }
        }
        _ => EasingFunction::Ease,
    }
}

/// Parse a CSS duration string (e.g. "300ms", "0.5s") to milliseconds.
fn parse_duration_ms(s: &str) -> f32 {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        ms.trim().parse().unwrap_or(0.0)
    } else if let Some(secs) = s.strip_suffix('s') {
        secs.trim().parse::<f32>().unwrap_or(0.0) * 1000.0
    } else {
        s.parse::<f32>().unwrap_or(0.0) * 1000.0
    }
}

/// Convert a `liquide_style_engine` `AnimationDirection` to the `liquide_animation` `Direction`.
fn convert_direction(d: &AnimationDirection) -> Direction {
    match d {
        AnimationDirection::Normal => Direction::Normal,
        AnimationDirection::Reverse => Direction::Reverse,
        AnimationDirection::Alternate => Direction::Alternate,
        AnimationDirection::AlternateReverse => Direction::AlternateReverse,
    }
}

/// Convert a `liquide_style_engine` `AnimationFillMode` to `liquide_animation` `FillMode`.
fn convert_fill_mode(f: &AnimationFillMode) -> FillMode {
    match f {
        AnimationFillMode::None => FillMode::None,
        AnimationFillMode::Forwards => FillMode::Forwards,
        AnimationFillMode::Backwards => FillMode::Backwards,
        AnimationFillMode::Both => FillMode::Both,
    }
}

/// Convert an `AnimationIterationCount` from the style engine to `liquide_animation`.
fn convert_iteration_count(c: &AnimationIterationCount) -> IterationCount {
    match c {
        AnimationIterationCount::Finite(n) => IterationCount::Finite(*n),
        AnimationIterationCount::Infinite => IterationCount::Infinite,
    }
}

// ── Transitionable property float extraction ────────────────────────────

/// CSS properties that support color transitions.
const COLOR_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "outline-color",
];

/// List of CSS properties we support float transitions for.
const TRANSITIONABLE_PROPERTIES: &[&str] = &[
    // Opacity
    "opacity",
    // Dimensions
    "width", "height", "min-width", "min-height", "max-width", "max-height",
    // Spacing
    "margin-top", "margin-right", "margin-bottom", "margin-left",
    "padding-top", "padding-right", "padding-bottom", "padding-left",
    // Position
    "top", "right", "bottom", "left",
    // Border widths
    "border-top-width", "border-right-width", "border-bottom-width", "border-left-width",
    // Border radius
    "border-top-left-radius", "border-top-right-radius",
    "border-bottom-left-radius", "border-bottom-right-radius",
    // Typography
    "font-size", "letter-spacing", "word-spacing", "text-indent", "line-height",
    // Flex
    "flex-grow", "flex-shrink", "flex-basis",
    // Grid/gap
    "row-gap", "column-gap", "gap",
    // Outline
    "outline-width", "outline-offset",
    // Other numeric
    "z-index", "order",
];

/// Extract a float value for a known transitionable property from a computed style.
/// Returns `None` if the property is not a simple float or is not supported.
fn get_float_property(style: &ComputedStyle, property: &str) -> Option<f32> {
    match property {
        "opacity" => Some(style.opacity),
        "font-size" => Some(style.font_size),
        "flex-grow" => Some(style.flex_grow),
        "flex-shrink" => Some(style.flex_shrink),
        "border-top-width" => Some(style.border_width.top),
        "border-right-width" => Some(style.border_width.right),
        "border-bottom-width" => Some(style.border_width.bottom),
        "border-left-width" => Some(style.border_width.left),
        "letter-spacing" => Some(style.letter_spacing),
        "word-spacing" => Some(style.word_spacing),
        "text-indent" => Some(style.text_indent),
        "order" => Some(style.order as f32),
        "z-index" => style.z_index.map(|z| z as f32),
        // Border radius
        "border-top-left-radius" => Some(style.border_radius.top_left),
        "border-top-right-radius" => Some(style.border_radius.top_right),
        "border-bottom-left-radius" => Some(style.border_radius.bottom_left),
        "border-bottom-right-radius" => Some(style.border_radius.bottom_right),
        // Line-height
        "line-height" => match &style.line_height {
            LineHeight::Px(v) => Some(*v),
            LineHeight::Number(v) => Some(*v * style.font_size),
            LineHeight::Normal => None,
        },
        // Outline
        "outline-width" => style.outline.as_ref().map(|o| o.width),
        "outline-offset" => style.outline.as_ref().map(|o| o.offset),
        // Dimension properties — extract Px value only
        "width" => dimension_px(&style.width),
        "height" => dimension_px(&style.height),
        "min-width" => dimension_px(&style.min_width),
        "min-height" => dimension_px(&style.min_height),
        "max-width" => dimension_px(&style.max_width),
        "max-height" => dimension_px(&style.max_height),
        "margin-top" => dimension_px(&style.margin.top),
        "margin-right" => dimension_px(&style.margin.right),
        "margin-bottom" => dimension_px(&style.margin.bottom),
        "margin-left" => dimension_px(&style.margin.left),
        "padding-top" => dimension_px(&style.padding.top),
        "padding-right" => dimension_px(&style.padding.right),
        "padding-bottom" => dimension_px(&style.padding.bottom),
        "padding-left" => dimension_px(&style.padding.left),
        "top" => dimension_px(&style.top),
        "right" => dimension_px(&style.right),
        "bottom" => dimension_px(&style.bottom),
        "left" => dimension_px(&style.left),
        "flex-basis" => dimension_px(&style.flex_basis),
        "gap" => dimension_px(&style.gap.width),
        "row-gap" => dimension_px(&style.row_gap),
        "column-gap" => dimension_px(&style.column_gap),
        _ => None,
    }
}

/// Set a float property on a mutable `ComputedStyle`.
fn set_float_property(style: &mut ComputedStyle, property: &str, value: f32) {
    match property {
        "opacity" => style.opacity = value,
        "font-size" => style.font_size = value,
        "flex-grow" => style.flex_grow = value,
        "flex-shrink" => style.flex_shrink = value,
        "border-top-width" => style.border_width.top = value,
        "border-right-width" => style.border_width.right = value,
        "border-bottom-width" => style.border_width.bottom = value,
        "border-left-width" => style.border_width.left = value,
        "letter-spacing" => style.letter_spacing = value,
        "word-spacing" => style.word_spacing = value,
        "text-indent" => style.text_indent = value,
        "order" => style.order = value as i32,
        "z-index" => style.z_index = Some(value as i32),
        // Border radius
        "border-top-left-radius" => style.border_radius.top_left = value,
        "border-top-right-radius" => style.border_radius.top_right = value,
        "border-bottom-left-radius" => style.border_radius.bottom_left = value,
        "border-bottom-right-radius" => style.border_radius.bottom_right = value,
        // Line-height
        "line-height" => style.line_height = LineHeight::Px(value),
        // Outline
        "outline-width" => {
            if let Some(ref mut o) = style.outline {
                o.width = value;
            }
        }
        "outline-offset" => {
            if let Some(ref mut o) = style.outline {
                o.offset = value;
            }
        }
        // Dimension properties
        "width" => style.width = Dimension::Px(value),
        "height" => style.height = Dimension::Px(value),
        "min-width" => style.min_width = Dimension::Px(value),
        "min-height" => style.min_height = Dimension::Px(value),
        "max-width" => style.max_width = Dimension::Px(value),
        "max-height" => style.max_height = Dimension::Px(value),
        "margin-top" => style.margin.top = Dimension::Px(value),
        "margin-right" => style.margin.right = Dimension::Px(value),
        "margin-bottom" => style.margin.bottom = Dimension::Px(value),
        "margin-left" => style.margin.left = Dimension::Px(value),
        "padding-top" => style.padding.top = Dimension::Px(value),
        "padding-right" => style.padding.right = Dimension::Px(value),
        "padding-bottom" => style.padding.bottom = Dimension::Px(value),
        "padding-left" => style.padding.left = Dimension::Px(value),
        "top" => style.top = Dimension::Px(value),
        "right" => style.right = Dimension::Px(value),
        "bottom" => style.bottom = Dimension::Px(value),
        "left" => style.left = Dimension::Px(value),
        "flex-basis" => style.flex_basis = Dimension::Px(value),
        "gap" => style.gap.width = Dimension::Px(value),
        "row-gap" => style.row_gap = Dimension::Px(value),
        "column-gap" => style.column_gap = Dimension::Px(value),
        _ => {}
    }
}

/// Extract a color for a known color property from a computed style.
fn get_color_property(style: &ComputedStyle, property: &str) -> Option<[u8; 4]> {
    match property {
        "color" => {
            let c = &style.color;
            Some([c.r, c.g, c.b, c.a])
        }
        "background-color" => {
            let c = &style.background_color;
            Some([c.r, c.g, c.b, c.a])
        }
        "border-top-color" => {
            let c = &style.border_color.top;
            Some([c.r, c.g, c.b, c.a])
        }
        "border-right-color" => {
            let c = &style.border_color.right;
            Some([c.r, c.g, c.b, c.a])
        }
        "border-bottom-color" => {
            let c = &style.border_color.bottom;
            Some([c.r, c.g, c.b, c.a])
        }
        "border-left-color" => {
            let c = &style.border_color.left;
            Some([c.r, c.g, c.b, c.a])
        }
        "outline-color" => style.outline.as_ref().map(|o| [o.color.r, o.color.g, o.color.b, o.color.a]),
        _ => None,
    }
}

/// Set a color property on a mutable `ComputedStyle`.
fn set_color_property(style: &mut ComputedStyle, property: &str, rgba: [u8; 4]) {
    use liquide_compositor::pixel::Color as CssColor;
    let c = CssColor::new(rgba[0], rgba[1], rgba[2], rgba[3]);
    match property {
        "color" => style.color = c,
        "background-color" => style.background_color = c,
        "border-top-color" => style.border_color.top = c,
        "border-right-color" => style.border_color.right = c,
        "border-bottom-color" => style.border_color.bottom = c,
        "border-left-color" => style.border_color.left = c,
        "outline-color" => {
            if let Some(ref mut o) = style.outline {
                o.color = c;
            }
        }
        _ => {}
    }
}

fn dimension_px(d: &Dimension) -> Option<f32> {
    match d {
        Dimension::Px(v) => Some(*v),
        _ => None,
    }
}

// ── Pipeline integration ────────────────────────────────────────────────

impl DesktopPipeline {
    /// Detect and start CSS transitions by comparing old and new computed styles.
    pub(super) fn detect_transitions(&mut self, styles: &StyleMap) {
        let mut dur_cache: HashMap<String, f32> = HashMap::new();
        let mut timing_cache: HashMap<String, EasingFunction> = HashMap::new();

        for (node_id, new_style) in styles.iter() {
            let old_style = match self.prev_styles.get(node_id) {
                Some(s) => s,
                None => continue,
            };

            // Check if this element has transition-property set
            let transition_property = match &new_style.transition_property {
                Some(p) => p.clone(),
                None => continue,
            };

            let duration_ms = match new_style.transition_duration.as_deref() {
                Some(s) => *dur_cache.entry(s.to_owned()).or_insert_with(|| parse_duration_ms(s)),
                None => 0.0,
            };

            if duration_ms <= 0.0 {
                continue;
            }

            let delay_ms = match new_style.transition_delay.as_deref() {
                Some(s) => *dur_cache.entry(s.to_owned()).or_insert_with(|| parse_duration_ms(s)),
                None => 0.0,
            };

            let easing = match new_style.transition_timing_function.as_deref() {
                Some(s) => *timing_cache.entry(s.to_owned()).or_insert_with(|| parse_timing_function(s)),
                None => EasingFunction::Ease,
            };

            // Determine which properties to transition
            let props: Vec<&str> = if transition_property.trim() == "all" {
                // All supported float and color properties
                let mut all = TRANSITIONABLE_PROPERTIES.to_vec();
                all.extend_from_slice(COLOR_PROPERTIES);
                all
            } else {
                transition_property.split(',').map(|s| s.trim()).collect()
            };

            for prop in &props {
                let old_val = match get_float_property(old_style, prop) {
                    Some(v) => v,
                    None => continue,
                };
                let new_val = match get_float_property(new_style, prop) {
                    Some(v) => v,
                    None => continue,
                };

                // Only start transition if value actually changed
                if (old_val - new_val).abs() < f32::EPSILON {
                    continue;
                }

                // If already transitioning, retarget from current value if
                // the destination changed; skip if same target.
                if self
                    .transition_engine
                    .is_transitioning(*node_id, prop)
                {
                    let same_target = self
                        .transition_engine
                        .get_target(*node_id, prop)
                        .is_some_and(|t| (t - new_val).abs() < f32::EPSILON);
                    if same_target {
                        continue;
                    }
                    // Retarget: start from the current interpolated value
                    let from = self
                        .transition_engine
                        .get(*node_id, prop)
                        .unwrap_or(old_val);
                    self.transition_engine.start(
                        *node_id, prop, from, new_val, duration_ms, delay_ms, easing,
                    );
                    continue;
                }

                self.transition_engine.start(
                    *node_id, prop, old_val, new_val, duration_ms, delay_ms, easing,
                );
            }

            // Color transitions
            for prop in &props {
                if !COLOR_PROPERTIES.contains(prop) {
                    continue;
                }
                let old_c = match get_color_property(old_style, prop) {
                    Some(c) => c,
                    None => continue,
                };
                let new_c = match get_color_property(new_style, prop) {
                    Some(c) => c,
                    None => continue,
                };
                if old_c == new_c {
                    continue;
                }
                // Encode RGBA as 4 separate float transitions
                let suffixes = ["_r", "_g", "_b", "_a"];
                for (i, suffix) in suffixes.iter().enumerate() {
                    let key = format!("{}{}", prop, suffix);
                    let from = old_c[i] as f32;
                    let to = new_c[i] as f32;
                    if (from - to).abs() < 0.5 {
                        continue;
                    }
                    self.transition_engine.start(
                        *node_id, &key, from, to, duration_ms, delay_ms, easing,
                    );
                }
            }
        }
    }

    /// Detect and start CSS animations based on `animation-name` in computed styles.
    pub(super) fn detect_animations(&mut self, styles: &StyleMap) {
        let mut dur_cache: HashMap<String, f32> = HashMap::new();
        let mut timing_cache: HashMap<String, EasingFunction> = HashMap::new();

        for (node_id, style) in styles.iter() {
            let anim_name = match &style.animation_name {
                Some(name) if !name.is_empty() && name != "none" => name.clone(),
                _ => continue,
            };

            // Check if an animation is already running for this name+node
            let already_running = self
                .animation_scheduler
                .animations_for(*node_id)
                .iter()
                .any(|a| {
                    a.keyframes_name == anim_name && a.state != AnimationState::Finished
                });
            if already_running {
                continue;
            }

            let duration_ms = match style.animation_duration.as_deref() {
                Some(s) => *dur_cache.entry(s.to_owned()).or_insert_with(|| parse_duration_ms(s)),
                None => 0.0,
            };

            // Skip zero-duration animations
            if duration_ms <= 0.0 {
                continue;
            }

            let delay_ms = match style.animation_delay.as_deref() {
                Some(s) => *dur_cache.entry(s.to_owned()).or_insert_with(|| parse_duration_ms(s)),
                None => 0.0,
            };

            let easing = match style.animation_timing_function.as_deref() {
                Some(s) => *timing_cache.entry(s.to_owned()).or_insert_with(|| parse_timing_function(s)),
                None => EasingFunction::Ease,
            };

            let direction = convert_direction(&style.animation_direction);
            let fill_mode = convert_fill_mode(&style.animation_fill_mode);
            let iteration_count = convert_iteration_count(&style.animation_iteration_count);

            let initial_state = match style.animation_play_state {
                AnimationPlayState::Running => {
                    if delay_ms > 0.0 {
                        AnimationState::Pending
                    } else {
                        AnimationState::Running
                    }
                }
                AnimationPlayState::Paused => AnimationState::Paused,
            };

            let anim = RunningAnimation {
                node_id: *node_id,
                keyframes_name: anim_name,
                duration_ms,
                delay_ms,
                easing,
                iteration_count,
                direction,
                fill_mode,
                state: initial_state,
                elapsed_ms: 0.0,
                iterations_done: 0.0,
            };

            self.animation_scheduler.start(anim);
        }
    }

    /// Tick all animations and transitions, then apply interpolated values
    /// to the style map. Returns `true` if any animations or transitions
    /// are active (caller should request a re-render).
    pub(super) fn tick_and_apply(&mut self, dt_ms: f32, styles: &mut StyleMap) -> bool {
        self.animation_scheduler.tick_all(dt_ms);
        self.transition_engine.tick_all(dt_ms);

        let anim_active = self.animation_scheduler.active_count() > 0;
        let trans_active = self.transition_engine.active_count() > 0;

        if !anim_active && !trans_active {
            return false;
        }

        // Apply transition overrides
        self.apply_transitions(styles);

        // Apply animation overrides
        self.apply_animations(styles);

        // Prune finished entries
        self.animation_scheduler.prune_finished();
        self.transition_engine.prune_finished();

        true
    }

    /// Apply active transition values back onto the style map.
    fn apply_transitions(&self, styles: &mut StyleMap) {
        // Collect overrides grouped by node — only iterates active transitions (typically 0-5).
        let mut overrides: HashMap<NodeId, Vec<(&str, f32)>> = HashMap::new();
        for (node_id, prop, val) in self.transition_engine.active_overrides() {
            overrides.entry(node_id).or_default().push((prop, val));
        }

        for (node_id, props) in &overrides {
            if let Some(existing) = styles.get(*node_id) {
                let mut patched = (**existing).clone();
                for &(prop, val) in props {
                    set_float_property(&mut patched, prop, val);
                }
                // Apply color channel overrides
                for color_prop in COLOR_PROPERTIES {
                    let r = self.transition_engine.get(*node_id, &format!("{}_r", color_prop));
                    let g = self.transition_engine.get(*node_id, &format!("{}_g", color_prop));
                    let b = self.transition_engine.get(*node_id, &format!("{}_b", color_prop));
                    let a = self.transition_engine.get(*node_id, &format!("{}_a", color_prop));
                    if r.is_some() || g.is_some() || b.is_some() || a.is_some() {
                        let base = get_color_property(&patched, color_prop).unwrap_or([0, 0, 0, 255]);
                        let rgba = [
                            r.map(|v| v.clamp(0.0, 255.0) as u8).unwrap_or(base[0]),
                            g.map(|v| v.clamp(0.0, 255.0) as u8).unwrap_or(base[1]),
                            b.map(|v| v.clamp(0.0, 255.0) as u8).unwrap_or(base[2]),
                            a.map(|v| v.clamp(0.0, 255.0) as u8).unwrap_or(base[3]),
                        ];
                        set_color_property(&mut patched, color_prop, rgba);
                    }
                }
                styles.insert(*node_id, patched);
            }
        }
    }

    /// Apply active animation keyframe values back onto the style map.
    fn apply_animations(&self, styles: &mut StyleMap) {
        // Collect all (node_id, property, value) triples first to avoid borrow issues
        let mut animation_overrides: Vec<(NodeId, String, f32)> = Vec::new();

        // Only iterate nodes with active animations to avoid O(n*m) iteration
        let mut animated_set: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        for (node_id, style) in styles.iter() {
            if style.animation_name.is_some() {
                animated_set.insert(*node_id);
            }
        }
        for node_id in &animated_set {
            let anims = self.animation_scheduler.animations_for(*node_id);
            for anim in anims {
                if anim.state != AnimationState::Running {
                    continue;
                }
                for prop in TRANSITIONABLE_PROPERTIES
                    .iter()
                {
                    if let Some(pv) = self.animation_scheduler.resolve_property(anim, prop) {
                        let vw = self.style_engine.viewport.width;
                        let vh = self.style_engine.viewport.height;
                        let fs = self.style_engine.base_font_size;
                        if let Some(val) = property_value_to_float(&pv, vw, vh, fs) {
                            animation_overrides.push((*node_id, prop.to_string(), val));
                        }
                    }
                }
            }
        }

        // Group overrides by node to avoid repeated clone+insert per property
        let mut grouped: HashMap<NodeId, Vec<(&str, f32)>> = HashMap::new();
        for (node_id, prop, val) in &animation_overrides {
            grouped.entry(*node_id).or_default().push((prop.as_str(), *val));
        }
        for (node_id, props) in &grouped {
            if let Some(existing) = styles.get(*node_id) {
                let mut patched = (**existing).clone();
                for &(prop, val) in props {
                    set_float_property(&mut patched, prop, val);
                }
                styles.insert(*node_id, patched);
            }
        }
    }

    /// Snapshot current styles for next-frame transition detection.
    pub(super) fn snapshot_styles(&mut self, styles: &StyleMap) {
        self.prev_styles.clear();
        for (node_id, style) in styles.iter() {
            self.prev_styles.insert(*node_id, Arc::clone(style));
        }
    }
}

/// Try to extract a float value from a `PropertyValue`.
fn property_value_to_float(
    pv: &liquide_theme_css::value::PropertyValue,
    viewport_width: f32,
    viewport_height: f32,
    base_font_size: f32,
) -> Option<f32> {
    use liquide_theme_css::value::{PropertyValue, LengthUnit};
    match pv {
        PropertyValue::Length(lu) => match lu {
            LengthUnit::Px(v) => Some(*v),
            LengthUnit::Em(v) => Some(*v * base_font_size),
            LengthUnit::Rem(v) => Some(*v * base_font_size),
            LengthUnit::Pt(v) => Some(*v * 1.333),
            LengthUnit::Vw(v) => Some(*v * viewport_width / 100.0),
            LengthUnit::Vh(v) => Some(*v * viewport_height / 100.0),
            LengthUnit::Percent(v) => Some(*v),
            _ => None,
        },
        PropertyValue::Number(v) => Some(*v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds() {
        assert!((parse_duration_ms("0.5s") - 500.0).abs() < 0.1);
        assert!((parse_duration_ms("1s") - 1000.0).abs() < 0.1);
    }

    #[test]
    fn parse_duration_millis() {
        assert!((parse_duration_ms("300ms") - 300.0).abs() < 0.1);
        assert!((parse_duration_ms("0ms") - 0.0).abs() < 0.1);
    }

    #[test]
    fn parse_timing_linear() {
        assert_eq!(parse_timing_function("linear"), EasingFunction::Linear);
    }

    #[test]
    fn parse_timing_ease() {
        assert_eq!(parse_timing_function("ease"), EasingFunction::Ease);
        assert_eq!(parse_timing_function("ease-in"), EasingFunction::EaseIn);
        assert_eq!(parse_timing_function("ease-out"), EasingFunction::EaseOut);
        assert_eq!(
            parse_timing_function("ease-in-out"),
            EasingFunction::EaseInOut
        );
    }

    #[test]
    fn parse_timing_cubic_bezier() {
        let f = parse_timing_function("cubic-bezier(0.25, 0.1, 0.25, 1.0)");
        match f {
            EasingFunction::CubicBezier(cb) => {
                assert!((cb.x1 - 0.25).abs() < 0.01);
                assert!((cb.y1 - 0.1).abs() < 0.01);
            }
            _ => panic!("expected CubicBezier"),
        }
    }

    #[test]
    fn parse_timing_steps() {
        let f = parse_timing_function("steps(4, start)");
        assert_eq!(
            f,
            EasingFunction::Steps {
                count: 4,
                jump_start: true
            }
        );

        let f2 = parse_timing_function("steps(3, end)");
        assert_eq!(
            f2,
            EasingFunction::Steps {
                count: 3,
                jump_start: false
            }
        );
    }
}
