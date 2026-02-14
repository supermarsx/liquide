//! CSS transition engine.
//!
//! Tracks property changes and interpolates values over `transition-duration`.

use std::collections::HashMap;

use liquide_compositor::scene::NodeId;
use serde::{Deserialize, Serialize};

use crate::easing::EasingFunction;
use crate::interpolate::Interpolatable;

/// State of a running transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionState {
    /// Currently interpolating.
    Running,
    /// Completed.
    Finished,
}

/// A concrete in-flight transition on a single float property.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatTransition {
    pub property: String,
    pub from: f32,
    pub to: f32,
    pub duration_ms: f32,
    pub delay_ms: f32,
    pub easing: EasingFunction,
    pub elapsed_ms: f32,
    pub state: TransitionState,
}

impl FloatTransition {
    /// Current interpolated value.
    pub fn current(&self) -> f32 {
        if self.state == TransitionState::Finished {
            return self.to;
        }
        let active = (self.elapsed_ms - self.delay_ms).max(0.0);
        let raw_t = if self.duration_ms > 0.0 {
            (active / self.duration_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased = self.easing.evaluate(raw_t);
        self.from.interpolate(&self.to, eased)
    }

    /// Advance by `dt` ms.
    pub fn tick(&mut self, dt_ms: f32) {
        if self.state == TransitionState::Finished {
            return;
        }
        self.elapsed_ms += dt_ms;
        if self.elapsed_ms >= self.delay_ms + self.duration_ms {
            self.state = TransitionState::Finished;
        }
    }
}

/// Manages all running CSS transitions.
#[derive(Debug, Default)]
pub struct TransitionEngine {
    /// Node → (property → transition).
    transitions: HashMap<NodeId, HashMap<String, FloatTransition>>,
}

impl TransitionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start or replace a transition on a property of a node.
    pub fn start(
        &mut self,
        node_id: NodeId,
        property: &str,
        from: f32,
        to: f32,
        duration_ms: f32,
        delay_ms: f32,
        easing: EasingFunction,
    ) {
        let transition = FloatTransition {
            property: property.to_string(),
            from,
            to,
            duration_ms,
            delay_ms,
            easing,
            elapsed_ms: 0.0,
            state: TransitionState::Running,
        };
        self.transitions
            .entry(node_id)
            .or_default()
            .insert(property.to_string(), transition);
    }

    /// Tick all transitions by `dt_ms` milliseconds.
    pub fn tick_all(&mut self, dt_ms: f32) {
        for props in self.transitions.values_mut() {
            for t in props.values_mut() {
                t.tick(dt_ms);
            }
        }
    }

    /// Get current value of a transitioning property.
    pub fn get(&self, node_id: NodeId, property: &str) -> Option<f32> {
        self.transitions
            .get(&node_id)?
            .get(property)
            .map(|t| t.current())
    }

    /// Check if a property is actively transitioning.
    pub fn is_transitioning(&self, node_id: NodeId, property: &str) -> bool {
        self.transitions
            .get(&node_id)
            .and_then(|props| props.get(property))
            .map_or(false, |t| t.state == TransitionState::Running)
    }

    /// Remove all finished transitions.
    pub fn prune_finished(&mut self) {
        for props in self.transitions.values_mut() {
            props.retain(|_, t| t.state != TransitionState::Finished);
        }
        self.transitions.retain(|_, props| !props.is_empty());
    }

    /// Total number of active transitions.
    pub fn active_count(&self) -> usize {
        self.transitions
            .values()
            .flat_map(|m| m.values())
            .filter(|t| t.state == TransitionState::Running)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_transition() {
        let mut engine = TransitionEngine::new();
        engine.start(1, "opacity", 0.0, 1.0, 1000.0, 0.0, EasingFunction::Linear);
        engine.tick_all(500.0);
        let val = engine.get(1, "opacity").unwrap();
        assert!((val - 0.5).abs() < 0.01);
    }

    #[test]
    fn finishes_at_end() {
        let mut engine = TransitionEngine::new();
        engine.start(2, "width", 100.0, 200.0, 500.0, 0.0, EasingFunction::Linear);
        engine.tick_all(600.0);
        let val = engine.get(2, "width").unwrap();
        assert!((val - 200.0).abs() < 0.1);
    }

    #[test]
    fn delay_works() {
        let mut engine = TransitionEngine::new();
        engine.start(3, "height", 0.0, 100.0, 1000.0, 500.0, EasingFunction::Linear);
        engine.tick_all(250.0);
        let val = engine.get(3, "height").unwrap();
        // Still in delay — should be at 0.0
        assert!((val - 0.0).abs() < 0.1);

        engine.tick_all(750.0);
        // 1000ms total, 500ms delay, 500ms into transition = 50%
        let val = engine.get(3, "height").unwrap();
        assert!((val - 50.0).abs() < 1.0);
    }
}
