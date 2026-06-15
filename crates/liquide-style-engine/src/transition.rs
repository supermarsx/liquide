//! CSS Transitions runtime — tracks running transitions and interpolates
//! property values between old and new computed styles.

#![allow(deprecated)] // TODO: migrate to liquide_animation::TransitionEngine

use std::collections::HashMap;

use crate::computed::{ComputedStyle, TimingFunction};
use liquide_dom::NodeId;

/// A single running transition for one property on one node.
#[deprecated(
    note = "Use liquide_animation::FloatTransition instead. This type duplicates TransitionEngine functionality."
)]
#[derive(Debug, Clone)]
pub struct RunningTransition {
    /// CSS property being transitioned (e.g. "width", "opacity", "background-color").
    pub property: String,
    /// Start value (f32 representation — for color we use individual channels).
    pub from: f32,
    /// End value.
    pub to: f32,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Delay in milliseconds.
    pub delay_ms: f32,
    /// Elapsed time in milliseconds (starts negative if there's a delay).
    pub elapsed_ms: f32,
    /// Timing function.
    pub timing_function: TimingFunction,
}

impl RunningTransition {
    /// The interpolated value at the current elapsed time.
    pub fn current_value(&self) -> f32 {
        if self.elapsed_ms < 0.0 {
            return self.from; // Still in delay
        }
        if self.duration_ms <= 0.0 || self.elapsed_ms >= self.duration_ms {
            return self.to;
        }
        let t = self.elapsed_ms / self.duration_ms;
        let eased = ease(t, &self.timing_function);
        self.from + (self.to - self.from) * eased
    }

    /// Whether the transition has finished.
    pub fn is_finished(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Advance by `dt_ms` milliseconds.
    pub fn tick(&mut self, dt_ms: f32) {
        self.elapsed_ms += dt_ms;
    }
}

/// Manages all running CSS transitions across the document.
///
/// **⚠️ DEPRECATED:** This is a duplicate of [`liquide_animation::TransitionEngine`].
/// New code should use `TransitionEngine` from `liquide-animation` instead.
#[deprecated(
    note = "Use liquide_animation::TransitionEngine instead. This type will be removed in a future release."
)]
#[derive(Debug, Default)]
pub struct TransitionManager {
    /// Active transitions: node → property → transition.
    transitions: HashMap<NodeId, HashMap<String, RunningTransition>>,
    /// Previous frame's extracted property values for change detection.
    previous_values: HashMap<NodeId, HashMap<String, f32>>,
}

impl TransitionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect property changes and start/update transitions for a node.
    ///
    /// Call this after computing a new style for `node_id`. Pass the old
    /// extracted values and the new computed style.
    pub fn update_node(&mut self, node_id: NodeId, new_style: &ComputedStyle) {
        let defs = &new_style.transition;
        if defs.is_empty() {
            // No transition definitions — clean up any running ones
            self.transitions.remove(&node_id);
            return;
        }

        let prev = self.previous_values.entry(node_id).or_default();
        let running = self.transitions.entry(node_id).or_default();

        for def in defs {
            if def.property.eq_ignore_ascii_case("all") {
                for &property in TRANSITIONABLE_PROPERTIES {
                    let Some(new_val) = extract_numeric_property(new_style, property) else {
                        continue;
                    };
                    let old_val = prev.get(property).copied();

                    if let Some(old) = old_val {
                        if (old - new_val).abs() > f32::EPSILON && !running.contains_key(property) {
                            running.insert(
                                property.to_string(),
                                RunningTransition {
                                    property: property.to_string(),
                                    from: old,
                                    to: new_val,
                                    duration_ms: def.duration_ms,
                                    delay_ms: def.delay_ms,
                                    elapsed_ms: -def.delay_ms,
                                    timing_function: def.timing_function.clone(),
                                },
                            );
                        }
                    }

                    prev.insert(property.to_string(), new_val);
                }
                continue;
            }

            let property = def.property.as_str();
            let Some(new_val) = extract_numeric_property(new_style, property) else {
                continue;
            };
            let old_val = prev.get(property).copied();

            if let Some(old) = old_val {
                if (old - new_val).abs() > f32::EPSILON && !running.contains_key(property) {
                    running.insert(
                        property.to_string(),
                        RunningTransition {
                            property: property.to_string(),
                            from: old,
                            to: new_val,
                            duration_ms: def.duration_ms,
                            delay_ms: def.delay_ms,
                            elapsed_ms: -def.delay_ms,
                            timing_function: def.timing_function.clone(),
                        },
                    );
                }
            }

            prev.insert(property.to_string(), new_val);
        }
    }

    /// Advance all running transitions by `dt_ms` milliseconds. Call once per frame.
    pub fn tick_all(&mut self, dt_ms: f32) {
        for transitions in self.transitions.values_mut() {
            for t in transitions.values_mut() {
                t.tick(dt_ms);
            }
            // Remove finished transitions
            transitions.retain(|_, t| !t.is_finished());
        }
        // Remove empty node entries
        self.transitions.retain(|_, m| !m.is_empty());
    }

    /// Get the interpolated value for a property on a node, if a transition is running.
    pub fn get_value(&self, node_id: NodeId, property: &str) -> Option<f32> {
        self.transitions
            .get(&node_id)?
            .get(property)
            .map(|t| t.current_value())
    }

    /// Check if any transitions are currently running.
    pub fn has_running_transitions(&self) -> bool {
        !self.transitions.is_empty()
    }

    /// Remove all transitions for a node (e.g. when the node is removed from the DOM).
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.transitions.remove(&node_id);
        self.previous_values.remove(&node_id);
    }

    /// Clear all running transitions and previous values.
    pub fn clear(&mut self) {
        self.transitions.clear();
        self.previous_values.clear();
    }

    /// Drop all in-flight transitions while **keeping** the recorded baseline
    /// (`previous_values`) for change detection.
    ///
    /// Use this to treat the current frame as the established baseline state
    /// rather than a user-visible transition: any transitions started during
    /// the initial style computation are discarded, but the per-property
    /// baseline values are retained so the *next* change is still detected
    /// (and animated) correctly.
    pub fn clear_running(&mut self) {
        self.transitions.clear();
    }
}

/// Extract a numeric (f32) representation of a CSS property from computed style.
/// For dimensions, extracts only Px values (viewport/percentage resolution
/// requires context not available here).
const TRANSITIONABLE_PROPERTIES: &[&str] = &[
    "opacity",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "top",
    "right",
    "bottom",
    "left",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "font-size",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "flex-grow",
    "flex-shrink",
    "gap",
    "column-gap",
    "row-gap",
];

fn extract_numeric_property(style: &ComputedStyle, property: &str) -> Option<f32> {
    use crate::dimension::Dimension;
    fn dim_px(d: &Dimension) -> f32 {
        match d {
            Dimension::Px(v) => *v,
            Dimension::Zero => 0.0,
            _ => 0.0,
        }
    }
    Some(match property {
        "opacity" => style.opacity,
        "width" => dim_px(&style.width),
        "height" => dim_px(&style.height),
        "min-width" => dim_px(&style.min_width),
        "min-height" => dim_px(&style.min_height),
        "max-width" => dim_px(&style.max_width),
        "max-height" => dim_px(&style.max_height),
        "top" => dim_px(&style.top),
        "right" => dim_px(&style.right),
        "bottom" => dim_px(&style.bottom),
        "left" => dim_px(&style.left),
        "margin-top" => dim_px(&style.margin.top),
        "margin-right" => dim_px(&style.margin.right),
        "margin-bottom" => dim_px(&style.margin.bottom),
        "margin-left" => dim_px(&style.margin.left),
        "padding-top" => dim_px(&style.padding.top),
        "padding-right" => dim_px(&style.padding.right),
        "padding-bottom" => dim_px(&style.padding.bottom),
        "padding-left" => dim_px(&style.padding.left),
        "font-size" => style.font_size,
        "line-height" => match style.line_height {
            crate::computed::LineHeight::Px(v) => v,
            crate::computed::LineHeight::Number(v) => v * style.font_size,
            crate::computed::LineHeight::Normal => style.font_size * 1.2,
        },
        "letter-spacing" => style.letter_spacing,
        "word-spacing" => style.word_spacing,
        "border-top-width" => style.border_width.top,
        "border-right-width" => style.border_width.right,
        "border-bottom-width" => style.border_width.bottom,
        "border-left-width" => style.border_width.left,
        "flex-grow" => style.flex_grow,
        "flex-shrink" => style.flex_shrink,
        "gap" => dim_px(&style.gap.width),
        "column-gap" => dim_px(&style.column_gap),
        "row-gap" => dim_px(&style.row_gap),
        _ => return None,
    })
}

/// Apply a CSS timing function to a normalized progress `t` (0.0 → 1.0).
fn ease(t: f32, tf: &TimingFunction) -> f32 {
    match tf {
        TimingFunction::Linear => t,
        TimingFunction::Ease => cubic_bezier(t, 0.25, 0.1, 0.25, 1.0),
        TimingFunction::EaseIn => cubic_bezier(t, 0.42, 0.0, 1.0, 1.0),
        TimingFunction::EaseOut => cubic_bezier(t, 0.0, 0.0, 0.58, 1.0),
        TimingFunction::EaseInOut => cubic_bezier(t, 0.42, 0.0, 0.58, 1.0),
        TimingFunction::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, *x1, *y1, *x2, *y2),
        TimingFunction::Steps(steps, _pos) => {
            let s = *steps as f32;
            (t * s).floor() / s
        }
    }
}

/// Approximate cubic-bezier evaluation using binary search for the t parameter.
fn cubic_bezier(progress: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Find the parametric t that corresponds to `progress` on the X axis
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for _ in 0..16 {
        let mid = (lo + hi) / 2.0;
        let x = bezier_component(mid, x1, x2);
        if x < progress {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = (lo + hi) / 2.0;
    bezier_component(t, y1, y2)
}

/// Evaluate one component of a cubic bezier at parameter t.
/// B(t) = 3(1-t)²t·P1 + 3(1-t)t²·P2 + t³
fn bezier_component(t: f32, p1: f32, p2: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * t * p1 + 3.0 * mt * t * t * p2 + t * t * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed::{TimingFunction, TransitionDef};
    use crate::dimension::Dimension;

    #[test]
    fn transition_property_all_tracks_supported_numeric_changes() {
        let node_id = 1;
        let mut manager = TransitionManager::new();

        let mut style = ComputedStyle::default();
        style.transition = vec![TransitionDef {
            property: "all".into(),
            duration_ms: 150.0,
            delay_ms: 0.0,
            timing_function: TimingFunction::Linear,
        }];
        style.width = Dimension::Px(10.0);
        style.opacity = 0.5;

        manager.update_node(node_id, &style);

        let mut changed = style.clone();
        changed.width = Dimension::Px(30.0);

        manager.update_node(node_id, &changed);

        assert_eq!(manager.get_value(node_id, "width"), Some(10.0));
        assert!(manager.has_running_transitions());
        assert!(manager.get_value(node_id, "opacity").is_none());
    }
}
