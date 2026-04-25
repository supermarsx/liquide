//! CSS keyframe animation scheduler.
//!
//! Manages running CSS animations tied to scene nodes.  Each animation
//! references a `@keyframes` rule by name, tracks elapsed time, and
//! produces per-frame property deltas.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::easing::EasingFunction;
use liquide_compositor::scene::NodeId;
use liquide_theme_css::value::{KeyframesRule, PropertyValue};

/// State of a running animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationState {
    /// Waiting for `animation-delay` to expire.
    Pending,
    /// Currently playing.
    Running,
    /// Paused by `animation-play-state: paused`.
    Paused,
    /// Completed all iterations (or removed).
    Finished,
}

/// Direction mode for an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// Fill mode for an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

/// Iteration count.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IterationCount {
    Finite(f32),
    Infinite,
}

/// A single running animation instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningAnimation {
    /// Id of the node this animation targets.
    pub node_id: NodeId,
    /// Name of the `@keyframes` rule.
    pub keyframes_name: String,
    /// Duration of a single iteration in milliseconds.
    pub duration_ms: f32,
    /// Delay before animation starts (ms).
    pub delay_ms: f32,
    /// Easing function.
    pub easing: EasingFunction,
    /// Iteration count.
    pub iteration_count: IterationCount,
    /// Direction.
    pub direction: Direction,
    /// Fill mode.
    pub fill_mode: FillMode,

    // ── Runtime state ───────────────────────────────────────────────────
    /// Current state.
    pub state: AnimationState,
    /// Elapsed wall-clock time since creation (ms). Counts even during delay.
    pub elapsed_ms: f32,
    /// Number of full iterations completed.
    pub iterations_done: f32,
}

impl RunningAnimation {
    /// Compute the local progress (0.0–1.0) within the current iteration,
    /// accounting for direction and easing.
    pub fn progress(&self) -> f32 {
        if self.duration_ms <= 0.0 {
            return 0.0;
        }
        // Before the delay expires the semantics depend on `fill_mode`:
        // `Backwards` / `Both` means "hold the first keyframe" which is
        // `progress() == 0.0` for Normal/Alternate and `1.0` for the
        // reversed directions.  For the other fill modes we report 0.0.
        if self.state == AnimationState::Pending {
            return match (self.fill_mode, self.direction) {
                (FillMode::Backwards | FillMode::Both, Direction::Reverse)
                | (FillMode::Backwards | FillMode::Both, Direction::AlternateReverse) => 1.0,
                _ => 0.0,
            };
        }

        // After finishing, `Forwards` / `Both` hold the final frame value.
        if self.state == AnimationState::Finished {
            let final_iter_even =
                ((self.iterations_done.floor() as u32).saturating_sub(1)) % 2 == 0;
            let end_value = match self.direction {
                Direction::Normal => 1.0,
                Direction::Reverse => 0.0,
                Direction::Alternate => {
                    if final_iter_even {
                        1.0
                    } else {
                        0.0
                    }
                }
                Direction::AlternateReverse => {
                    if final_iter_even {
                        0.0
                    } else {
                        1.0
                    }
                }
            };
            return match self.fill_mode {
                FillMode::Forwards | FillMode::Both => self.easing.evaluate(end_value),
                _ => 0.0,
            };
        }

        let active_time = (self.elapsed_ms - self.delay_ms).max(0.0);
        // Compute raw position inside the *current* iteration.  When we are
        // exactly at an iteration boundary (active_time is an integer
        // multiple of duration) we want `raw = 1.0` for the iteration that
        // just completed, not `0.0` from the next one.
        let iteration_f = active_time / self.duration_ms;
        let raw = if iteration_f > 0.0 && (iteration_f - iteration_f.floor()).abs() < f32::EPSILON {
            1.0
        } else {
            iteration_f.fract()
        };

        let directed = match self.direction {
            Direction::Normal => raw,
            Direction::Reverse => 1.0 - raw,
            Direction::Alternate => {
                if (self.iterations_done as u32) % 2 == 0 {
                    raw
                } else {
                    1.0 - raw
                }
            }
            Direction::AlternateReverse => {
                if (self.iterations_done as u32) % 2 == 0 {
                    1.0 - raw
                } else {
                    raw
                }
            }
        };

        self.easing.evaluate(directed)
    }

    /// Advance the animation by `dt` milliseconds.  Returns an
    /// [`AnimationEvent`] if the tick produced a state transition
    /// (start / iteration boundary / end), otherwise `None`.
    pub fn tick(&mut self, dt_ms: f32) -> Option<AnimationEvent> {
        if self.state == AnimationState::Finished || self.state == AnimationState::Paused {
            return None;
        }

        let was_pending = self.state == AnimationState::Pending;
        let prev_iterations = self.iterations_done.floor() as u32;
        self.elapsed_ms += dt_ms;

        if was_pending && self.elapsed_ms >= self.delay_ms {
            self.state = AnimationState::Running;
        }

        let mut event: Option<AnimationEvent> = None;
        if was_pending && self.state == AnimationState::Running {
            event = Some(AnimationEvent::Start);
        }

        if self.state == AnimationState::Running && self.duration_ms > 0.0 {
            let active = (self.elapsed_ms - self.delay_ms).max(0.0);
            self.iterations_done = active / self.duration_ms;

            match self.iteration_count {
                IterationCount::Finite(max) if self.iterations_done >= max => {
                    self.iterations_done = max;
                    self.state = AnimationState::Finished;
                    // End takes precedence over an intermediate iteration event.
                    event = Some(AnimationEvent::End);
                }
                _ => {
                    let new_iter = self.iterations_done.floor() as u32;
                    if new_iter > prev_iterations && event.is_none() {
                        event = Some(AnimationEvent::Iteration(new_iter));
                    }
                }
            }
        }

        event
    }
}

/// Lifecycle events emitted by [`AnimationScheduler::tick_all`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationEvent {
    /// The animation's delay expired and it began running.
    Start,
    /// An iteration boundary was crossed; the u32 is the new iteration index.
    Iteration(u32),
    /// The animation reached its final iteration count.
    End,
}

/// Manages all running CSS animations.
#[derive(Debug, Default)]
pub struct AnimationScheduler {
    /// Running animations keyed by an internal handle.
    animations: Vec<RunningAnimation>,
    /// Keyframes registry (name → rule).
    keyframes: HashMap<String, KeyframesRule>,
    /// Events produced during the last `tick_all`.
    pending_events: Vec<(NodeId, AnimationEvent)>,
    /// When `true`, `tick_all` automatically drops finished animations at
    /// the end of the tick (default).  Set to `false` if callers need to
    /// inspect finished animations before they are removed.
    auto_prune: bool,
}

impl AnimationScheduler {
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            keyframes: HashMap::new(),
            pending_events: Vec::new(),
            auto_prune: true,
        }
    }

    /// Control whether `tick_all` auto-prunes finished animations.
    pub fn set_auto_prune(&mut self, enabled: bool) {
        self.auto_prune = enabled;
    }

    /// Register a `@keyframes` rule.
    pub fn register_keyframes(&mut self, rule: KeyframesRule) {
        self.keyframes.insert(rule.name.clone(), rule);
    }

    /// Check if a `@keyframes` rule with this name already exists.
    pub fn has_keyframes(&self, name: &str) -> bool {
        self.keyframes.contains_key(name)
    }

    /// Start a new animation on a node.
    pub fn start(&mut self, anim: RunningAnimation) {
        self.animations.push(anim);
    }

    /// Tick all running animations forward by `dt_ms` milliseconds.
    ///
    /// Collects `AnimationEvent`s into an internal buffer that can be
    /// drained with [`drain_events`](Self::drain_events), and (by default)
    /// removes animations that reached their finished state and have no
    /// forwards fill to retain.
    pub fn tick_all(&mut self, dt_ms: f32) {
        self.pending_events.clear();
        for anim in &mut self.animations {
            if let Some(ev) = anim.tick(dt_ms) {
                self.pending_events.push((anim.node_id, ev));
            }
        }
        if self.auto_prune {
            // Retain finished animations only if a fill mode holds the
            // value past the end of the timeline.  Otherwise drop them.
            self.animations.retain(|a| {
                a.state != AnimationState::Finished
                    || matches!(a.fill_mode, FillMode::Forwards | FillMode::Both)
            });
        }
    }

    /// Drain accumulated events (start / iteration / end).
    pub fn drain_events(&mut self) -> Vec<(NodeId, AnimationEvent)> {
        std::mem::take(&mut self.pending_events)
    }

    /// Remove all finished animations.
    pub fn prune_finished(&mut self) {
        self.animations
            .retain(|a| a.state != AnimationState::Finished);
    }

    /// Get running animations for a specific node.
    pub fn animations_for(&self, node_id: NodeId) -> Vec<&RunningAnimation> {
        self.animations
            .iter()
            .filter(|a| a.node_id == node_id)
            .collect()
    }

    /// Get interpolated property value at the current animation progress.
    pub fn resolve_property(
        &self,
        anim: &RunningAnimation,
        property: &str,
    ) -> Option<PropertyValue> {
        let rule = self.keyframes.get(&anim.keyframes_name)?;
        let progress = anim.progress();

        // Find the two surrounding keyframes
        let mut before: Option<&liquide_theme_css::value::Keyframe> = None;
        let mut after: Option<&liquide_theme_css::value::Keyframe> = None;

        for kf in &rule.keyframes {
            for &sel in &kf.selectors {
                if sel <= progress {
                    if before.map_or(true, |b| b.selectors.first().copied().unwrap_or(0.0) <= sel) {
                        before = Some(kf);
                    }
                }
                if sel >= progress {
                    if after.map_or(true, |a| a.selectors.first().copied().unwrap_or(1.0) >= sel) {
                        after = Some(kf);
                    }
                }
            }
        }

        // Look up the property in surrounding frames
        let from_val = before.and_then(|kf| {
            kf.declarations
                .iter()
                .find(|(k, _)| k == property)
                .map(|(_, v)| v.clone())
        });
        let to_val = after.and_then(|kf| {
            kf.declarations
                .iter()
                .find(|(k, _)| k == property)
                .map(|(_, v)| v.clone())
        });

        let from_offset = before
            .and_then(|kf| kf.selectors.first().copied())
            .unwrap_or(0.0);
        let to_offset = after
            .and_then(|kf| kf.selectors.first().copied())
            .unwrap_or(1.0);
        let local_t = if (to_offset - from_offset).abs() > f32::EPSILON {
            ((progress - from_offset) / (to_offset - from_offset)).clamp(0.0, 1.0)
        } else {
            1.0
        };

        match (from_val, to_val) {
            (Some(a), Some(b)) => {
                // Try linear interpolation for numeric property values
                if let Some(result) = interpolate_property_values(&a, &b, local_t) {
                    Some(result)
                } else {
                    // Non-numeric: snap at 50%
                    if progress < 0.5 { Some(a) } else { Some(b) }
                }
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    /// How many animations are active.
    pub fn active_count(&self) -> usize {
        self.animations
            .iter()
            .filter(|a| a.state == AnimationState::Running || a.state == AnimationState::Pending)
            .count()
    }
}

/// Linearly interpolate two PropertyValues if both are numeric.
fn interpolate_property_values(
    a: &PropertyValue,
    b: &PropertyValue,
    t: f32,
) -> Option<PropertyValue> {
    use liquide_theme_css::value::LengthUnit;
    match (a, b) {
        (PropertyValue::Number(va), PropertyValue::Number(vb)) => {
            Some(PropertyValue::Number(va + (vb - va) * t))
        }
        (PropertyValue::Length(la), PropertyValue::Length(lb)) => {
            match (la, lb) {
                (LengthUnit::Px(va), LengthUnit::Px(vb)) => {
                    Some(PropertyValue::Length(LengthUnit::Px(va + (vb - va) * t)))
                }
                (LengthUnit::Percent(va), LengthUnit::Percent(vb)) => Some(PropertyValue::Length(
                    LengthUnit::Percent(va + (vb - va) * t),
                )),
                (LengthUnit::Em(va), LengthUnit::Em(vb)) => {
                    Some(PropertyValue::Length(LengthUnit::Em(va + (vb - va) * t)))
                }
                (LengthUnit::Rem(va), LengthUnit::Rem(vb)) => {
                    Some(PropertyValue::Length(LengthUnit::Rem(va + (vb - va) * t)))
                }
                (LengthUnit::Vw(va), LengthUnit::Vw(vb)) => {
                    Some(PropertyValue::Length(LengthUnit::Vw(va + (vb - va) * t)))
                }
                (LengthUnit::Vh(va), LengthUnit::Vh(vb)) => {
                    Some(PropertyValue::Length(LengthUnit::Vh(va + (vb - va) * t)))
                }
                _ => None, // mismatched units: snap
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_anim(node_id: NodeId, name: &str, duration_ms: f32) -> RunningAnimation {
        RunningAnimation {
            node_id,
            keyframes_name: name.to_string(),
            duration_ms,
            delay_ms: 0.0,
            easing: EasingFunction::Linear,
            iteration_count: IterationCount::Finite(1.0),
            direction: Direction::Normal,
            fill_mode: FillMode::None,
            state: AnimationState::Running,
            elapsed_ms: 0.0,
            iterations_done: 0.0,
        }
    }

    #[test]
    fn tick_advances() {
        let mut anim = make_anim(1, "fade", 1000.0);
        anim.tick(500.0);
        assert!((anim.progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn finishes_after_duration() {
        let mut anim = make_anim(1, "fade", 1000.0);
        anim.tick(1100.0);
        assert_eq!(anim.state, AnimationState::Finished);
    }

    #[test]
    fn infinite_never_finishes() {
        let mut anim = make_anim(1, "spin", 500.0);
        anim.iteration_count = IterationCount::Infinite;
        anim.tick(10_000.0);
        assert_eq!(anim.state, AnimationState::Running);
    }
}
