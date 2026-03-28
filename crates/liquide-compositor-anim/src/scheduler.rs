use std::collections::HashMap;

use crate::animation::{Animation, AnimationId, AnimationState};
use crate::easing::EasingFunction;
use crate::keyframe::{AnimValue, Keyframe, KeyframeTrack};
use crate::transition::Transition;

/// Events emitted by the scheduler for the main thread to consume.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEvent {
    /// An animation has started playing (after delay).
    Started(AnimationId),
    /// An animation has finished all iterations.
    Completed(AnimationId),
    /// An animation was explicitly cancelled.
    Cancelled(AnimationId),
    /// An animation completed one iteration.
    IterationEnd(AnimationId, u32),
}

/// Scheduler that manages all compositor-driven animations and transitions.
///
/// This runs on the compositor thread. Each frame, `tick_all()` is called
/// with the frame delta, and the scheduler advances all active animations,
/// collects events, and provides sampled values for layer state application.
pub struct CompositorAnimScheduler {
    /// Active animations keyed by their ID.
    animations: HashMap<AnimationId, Animation>,
    /// Active transitions keyed by (layer_id, property_name).
    transitions: HashMap<(u64, String), Transition>,
    /// Mapping from AnimationId to the transition key, for transitions
    /// that were created via `add_transition`.
    transition_ids: HashMap<AnimationId, (u64, String)>,
    /// Events generated during the last `tick_all()` call.
    pending_events: Vec<AnimationEvent>,
    /// Counter for generating unique IDs.
    next_id: u64,
}

impl CompositorAnimScheduler {
    /// Create a new empty scheduler.
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            transitions: HashMap::new(),
            transition_ids: HashMap::new(),
            pending_events: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a fully configured animation and return its ID.
    pub fn add_animation(&mut self, anim: Animation) -> AnimationId {
        let id = anim.id;
        self.animations.insert(id, anim);
        id
    }

    /// Create and add a simple transition for a layer property.
    ///
    /// If a transition already exists for the same (layer_id, property), it
    /// is retargeted instead of replaced.
    pub fn add_transition(
        &mut self,
        layer_id: u64,
        property: String,
        from: AnimValue,
        to: AnimValue,
        duration_ms: f32,
        easing: EasingFunction,
    ) -> AnimationId {
        let key = (layer_id, property.clone());

        if let Some(existing) = self.transitions.get_mut(&key) {
            if !existing.is_complete() {
                existing.retarget(to);
                // Find the existing ID for this transition.
                for (id, k) in &self.transition_ids {
                    if *k == key {
                        return *id;
                    }
                }
            }
        }

        let id = AnimationId(self.next_id);
        self.next_id += 1;

        let transition = Transition::new(property, from, to, duration_ms, easing);
        self.transitions.insert(key.clone(), transition);
        self.transition_ids.insert(id, key);
        id
    }

    /// Cancel an animation or transition by ID.
    pub fn cancel(&mut self, id: AnimationId) {
        if let Some(mut anim) = self.animations.remove(&id) {
            anim.cancel();
            self.pending_events.push(AnimationEvent::Cancelled(id));
        }

        if let Some(key) = self.transition_ids.remove(&id) {
            self.transitions.remove(&key);
            self.pending_events.push(AnimationEvent::Cancelled(id));
        }
    }

    /// Advance all animations and transitions by `dt_ms` milliseconds.
    ///
    /// Collects events for animations that started, completed iterations,
    /// or finished.
    pub fn tick_all(&mut self, dt_ms: f32) {
        let mut finished_anims = Vec::new();

        for (id, anim) in &mut self.animations {
            let prev_state = anim.state;
            let prev_iter = anim.current_iteration;

            let still_active = anim.tick(dt_ms);

            // Detect started (state may go Pending → Running → Finished in one tick).
            if prev_state == AnimationState::Pending && anim.state != AnimationState::Pending {
                self.pending_events.push(AnimationEvent::Started(*id));
            }

            // Detect iteration boundary.
            if anim.current_iteration > prev_iter && anim.state == AnimationState::Running {
                self.pending_events.push(AnimationEvent::IterationEnd(*id, anim.current_iteration));
            }

            if !still_active {
                self.pending_events.push(AnimationEvent::Completed(*id));
                finished_anims.push(*id);
            }
        }

        // Remove finished animations.
        for id in finished_anims {
            self.animations.remove(&id);
        }

        // Tick transitions.
        let mut finished_transitions = Vec::new();
        for (key, transition) in &mut self.transitions {
            if !transition.tick(dt_ms) {
                finished_transitions.push(key.clone());
            }
        }

        // Clean up finished transitions and their ID mappings.
        for key in finished_transitions {
            self.transitions.remove(&key);
            self.transition_ids.retain(|_, k| *k != key);
        }
    }

    /// Drain all pending events, returning them as a Vec.
    pub fn drain_events(&mut self) -> Vec<AnimationEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Sample a specific property from an animation.
    pub fn sample_animation(&self, id: AnimationId, property: &str) -> Option<AnimValue> {
        self.animations.get(&id)?.sample(property)
    }

    /// Sample a transition for a specific layer and property.
    pub fn sample_transition(&self, layer_id: u64, property: &str) -> Option<AnimValue> {
        let key = (layer_id, property.to_string());
        self.transitions.get(&key).map(|t| t.current_value())
    }

    /// Return the number of active animations and transitions.
    pub fn active_count(&self) -> usize {
        self.animations.len() + self.transitions.len()
    }

    /// Whether any animations or transitions are currently active.
    pub fn is_animating(&self) -> bool {
        !self.animations.is_empty() || !self.transitions.is_empty()
    }

    /// Allocate a new unique AnimationId.
    pub fn next_animation_id(&mut self) -> AnimationId {
        let id = AnimationId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Default for CompositorAnimScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyframe::KeyframeTrack;

    fn make_anim(scheduler: &mut CompositorAnimScheduler, duration_ms: f32) -> AnimationId {
        let id = scheduler.next_animation_id();
        let mut tracks = HashMap::new();
        tracks.insert("opacity".to_string(), KeyframeTrack::new(vec![
            Keyframe { offset: 0.0, value: AnimValue::Float(0.0), easing: EasingFunction::Linear },
            Keyframe { offset: 1.0, value: AnimValue::Float(1.0), easing: EasingFunction::Linear },
        ]));
        let anim = Animation::new(id, tracks, duration_ms);
        scheduler.add_animation(anim);
        id
    }

    #[test]
    fn new_scheduler_empty() {
        let s = CompositorAnimScheduler::new();
        assert_eq!(s.active_count(), 0);
        assert!(!s.is_animating());
    }

    #[test]
    fn add_animation_increases_count() {
        let mut s = CompositorAnimScheduler::new();
        make_anim(&mut s, 100.0);
        assert_eq!(s.active_count(), 1);
        assert!(s.is_animating());
    }

    #[test]
    fn animation_lifecycle() {
        let mut s = CompositorAnimScheduler::new();
        let id = make_anim(&mut s, 100.0);

        // Tick to start.
        s.tick_all(10.0);
        let events = s.drain_events();
        assert!(events.contains(&AnimationEvent::Started(id)));

        // Sample mid-animation.
        let val = s.sample_animation(id, "opacity");
        assert!(val.is_some());

        // Complete.
        s.tick_all(200.0);
        let events = s.drain_events();
        assert!(events.contains(&AnimationEvent::Completed(id)));
        assert_eq!(s.active_count(), 0);
    }

    #[test]
    fn cancel_animation() {
        let mut s = CompositorAnimScheduler::new();
        let id = make_anim(&mut s, 1000.0);
        s.tick_all(10.0);
        s.drain_events();

        s.cancel(id);
        let events = s.drain_events();
        assert!(events.contains(&AnimationEvent::Cancelled(id)));
        assert_eq!(s.active_count(), 0);
    }

    #[test]
    fn transition_lifecycle() {
        let mut s = CompositorAnimScheduler::new();
        let _id = s.add_transition(
            42, "opacity".to_string(),
            AnimValue::Float(0.0), AnimValue::Float(1.0),
            200.0, EasingFunction::Linear,
        );
        assert_eq!(s.active_count(), 1);

        s.tick_all(100.0);
        let val = s.sample_transition(42, "opacity");
        match val {
            Some(AnimValue::Float(v)) => assert!((v - 0.5).abs() < 0.05, "midpoint: {v}"),
            other => panic!("expected Float, got {other:?}"),
        }

        // Complete.
        s.tick_all(150.0);
        assert_eq!(s.active_count(), 0);
    }

    #[test]
    fn transition_retarget_on_add() {
        let mut s = CompositorAnimScheduler::new();
        let id1 = s.add_transition(
            1, "opacity".to_string(),
            AnimValue::Float(0.0), AnimValue::Float(1.0),
            200.0, EasingFunction::Linear,
        );
        s.tick_all(100.0); // midpoint → 0.5

        // Adding same layer+property should retarget, not create new.
        let id2 = s.add_transition(
            1, "opacity".to_string(),
            AnimValue::Float(0.0), AnimValue::Float(0.0),
            200.0, EasingFunction::Linear,
        );
        // Should return the same ID since it was retargeted.
        assert_eq!(id1, id2);
        assert_eq!(s.active_count(), 1);
    }

    #[test]
    fn multiple_concurrent_animations() {
        let mut s = CompositorAnimScheduler::new();
        let id1 = make_anim(&mut s, 100.0);
        let id2 = make_anim(&mut s, 200.0);
        assert_eq!(s.active_count(), 2);

        s.tick_all(150.0);
        let events = s.drain_events();
        // id1 should have started and completed; id2 should have started.
        assert!(events.contains(&AnimationEvent::Started(id1)));
        assert!(events.contains(&AnimationEvent::Completed(id1)));
        assert!(events.contains(&AnimationEvent::Started(id2)));
        assert_eq!(s.active_count(), 1); // only id2 left

        s.tick_all(100.0);
        let events = s.drain_events();
        assert!(events.contains(&AnimationEvent::Completed(id2)));
        assert_eq!(s.active_count(), 0);
    }

    #[test]
    fn iteration_events() {
        let mut s = CompositorAnimScheduler::new();
        let id = s.next_animation_id();
        let mut tracks = HashMap::new();
        tracks.insert("opacity".to_string(), KeyframeTrack::new(vec![
            Keyframe { offset: 0.0, value: AnimValue::Float(0.0), easing: EasingFunction::Linear },
            Keyframe { offset: 1.0, value: AnimValue::Float(1.0), easing: EasingFunction::Linear },
        ]));
        let mut anim = Animation::new(id, tracks, 100.0);
        anim.iteration_count = 3.0;
        s.add_animation(anim);

        // Tick past first iteration.
        s.tick_all(150.0);
        let events = s.drain_events();
        assert!(events.contains(&AnimationEvent::Started(id)));
        assert!(events.contains(&AnimationEvent::IterationEnd(id, 1)));
    }

    #[test]
    fn drain_events_clears() {
        let mut s = CompositorAnimScheduler::new();
        make_anim(&mut s, 100.0);
        s.tick_all(10.0);
        let events = s.drain_events();
        assert!(!events.is_empty());
        // Second drain should be empty.
        let events = s.drain_events();
        assert!(events.is_empty());
    }

    #[test]
    fn sample_nonexistent() {
        let s = CompositorAnimScheduler::new();
        assert!(s.sample_animation(AnimationId(999), "opacity").is_none());
        assert!(s.sample_transition(999, "opacity").is_none());
    }

    #[test]
    fn default_trait() {
        let s = CompositorAnimScheduler::default();
        assert_eq!(s.active_count(), 0);
    }
}
