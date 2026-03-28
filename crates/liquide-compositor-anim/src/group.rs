//! Animation grouping and sequencing.
//!
//! Provides mechanisms to coordinate multiple animations:
//!
//! - **AnimationGroup**: runs animations in parallel (all start together).
//! - **AnimationSequence**: runs animations one after another.
//! - **AnimationTimeline**: combines groups and sequences with named markers.
//!
//! # Stagger
//!
//! Groups support staggered starts where each successive animation begins
//! after a fixed delay, similar to CSS stagger patterns or GSAP's stagger.

use std::collections::HashMap;

use crate::animation::AnimationId;

/// State of a group or sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupState {
    /// Not yet started.
    Idle,
    /// Actively playing.
    Playing,
    /// Paused (time not advancing).
    Paused,
    /// All animations have completed.
    Finished,
}

/// An entry in a group or sequence, tracking per-animation state.
#[derive(Debug, Clone)]
struct GroupEntry {
    /// The animation identifier.
    id: AnimationId,
    /// Delay before this entry starts (used for stagger).
    delay: f64,
    /// Elapsed time for this entry (relative to its start).
    elapsed: f64,
    /// Duration of this entry's animation (provided at add time).
    duration: f64,
    /// Whether this entry has started (delay elapsed).
    started: bool,
    /// Whether this entry has finished.
    finished: bool,
}

/// Run multiple animations in parallel.
///
/// All animations start simultaneously (modulo stagger delay). The group
/// finishes when every animation has completed.
pub struct AnimationGroup {
    entries: Vec<GroupEntry>,
    state: GroupState,
    elapsed: f64,
    stagger_delay: f64,
}

impl AnimationGroup {
    /// Create a new empty group.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            state: GroupState::Idle,
            elapsed: 0.0,
            stagger_delay: 0.0,
        }
    }

    /// Add an animation to the group.
    ///
    /// `duration` is the expected duration of the animation in seconds. The
    /// group uses this to determine when the animation finishes (the actual
    /// animation is driven externally via the scheduler).
    pub fn add(&mut self, id: AnimationId, duration: f64) {
        let index = self.entries.len();
        let delay = self.stagger_delay * index as f64;
        self.entries.push(GroupEntry {
            id,
            delay,
            elapsed: 0.0,
            duration,
            started: false,
            finished: false,
        });
    }

    /// Set the stagger delay in seconds.
    ///
    /// Each successive animation will start `delay` seconds after the previous
    /// one. This is applied retroactively to all entries.
    pub fn stagger(&mut self, delay: f64) {
        self.stagger_delay = delay;
        // Recompute delays for existing entries.
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.delay = delay * i as f64;
        }
    }

    /// Advance the group by `dt` seconds.
    ///
    /// Returns a list of animation IDs that should be started this frame
    /// (their stagger delay has elapsed).
    pub fn tick(&mut self, dt: f64) -> Vec<AnimationId> {
        if self.state == GroupState::Finished || self.state == GroupState::Paused {
            return Vec::new();
        }

        if self.state == GroupState::Idle {
            self.state = GroupState::Playing;
        }

        self.elapsed += dt;
        let mut newly_started = Vec::new();

        for entry in &mut self.entries {
            if entry.finished {
                continue;
            }

            if !entry.started && self.elapsed >= entry.delay {
                entry.started = true;
                newly_started.push(entry.id);
            }

            if entry.started {
                entry.elapsed += dt;
                if entry.elapsed >= entry.duration {
                    entry.finished = true;
                }
            }
        }

        // Check if all entries are finished.
        if self.entries.iter().all(|e| e.finished) {
            self.state = GroupState::Finished;
        }

        newly_started
    }

    /// Whether all animations in the group have completed.
    pub fn is_complete(&self) -> bool {
        self.state == GroupState::Finished
    }

    /// Get the current group state.
    pub fn state(&self) -> GroupState {
        self.state
    }

    /// Pause the group.
    pub fn pause(&mut self) {
        if self.state == GroupState::Playing {
            self.state = GroupState::Paused;
        }
    }

    /// Resume a paused group.
    pub fn resume(&mut self) {
        if self.state == GroupState::Paused {
            self.state = GroupState::Playing;
        }
    }

    /// Get the list of animation IDs in this group.
    pub fn animation_ids(&self) -> Vec<AnimationId> {
        self.entries.iter().map(|e| e.id).collect()
    }

    /// Get the total elapsed time.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Number of entries in the group.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the group is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AnimationGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Run animations one after another in sequence.
///
/// Each animation starts only after the previous one finishes. The sequence
/// completes when the last animation finishes.
pub struct AnimationSequence {
    entries: Vec<GroupEntry>,
    state: GroupState,
    current_index: usize,
    elapsed: f64,
}

impl AnimationSequence {
    /// Create a new empty sequence.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            state: GroupState::Idle,
            current_index: 0,
            elapsed: 0.0,
        }
    }

    /// Add an animation to the end of the sequence.
    pub fn add(&mut self, id: AnimationId, duration: f64) {
        self.entries.push(GroupEntry {
            id,
            delay: 0.0,
            elapsed: 0.0,
            duration,
            started: false,
            finished: false,
        });
    }

    /// Advance the sequence by `dt` seconds.
    ///
    /// Returns `Some(id)` if a new animation should be started this frame.
    pub fn tick(&mut self, dt: f64) -> Option<AnimationId> {
        if self.state == GroupState::Finished || self.state == GroupState::Paused {
            return None;
        }

        if self.entries.is_empty() {
            self.state = GroupState::Finished;
            return None;
        }

        if self.state == GroupState::Idle {
            self.state = GroupState::Playing;
        }

        self.elapsed += dt;

        if self.current_index >= self.entries.len() {
            self.state = GroupState::Finished;
            return None;
        }

        let mut started_id = None;

        let entry = &mut self.entries[self.current_index];
        if !entry.started {
            entry.started = true;
            started_id = Some(entry.id);
        }

        entry.elapsed += dt;
        if entry.elapsed >= entry.duration {
            entry.finished = true;
            self.current_index += 1;

            if self.current_index >= self.entries.len() {
                self.state = GroupState::Finished;
            }
        }

        started_id
    }

    /// Whether the sequence has completed.
    pub fn is_complete(&self) -> bool {
        self.state == GroupState::Finished
    }

    /// Get the current state.
    pub fn state(&self) -> GroupState {
        self.state
    }

    /// Get the currently playing animation ID, if any.
    pub fn current_animation(&self) -> Option<AnimationId> {
        if self.state != GroupState::Playing {
            return None;
        }
        self.entries.get(self.current_index).map(|e| e.id)
    }

    /// Pause the sequence.
    pub fn pause(&mut self) {
        if self.state == GroupState::Playing {
            self.state = GroupState::Paused;
        }
    }

    /// Resume a paused sequence.
    pub fn resume(&mut self) {
        if self.state == GroupState::Paused {
            self.state = GroupState::Playing;
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AnimationSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// A timeline combining groups, sequences, and markers for coordinating
/// complex multi-stage animations.
pub struct AnimationTimeline {
    /// Named groups.
    groups: HashMap<String, AnimationGroup>,
    /// Named sequences.
    sequences: HashMap<String, AnimationSequence>,
    /// Named time markers (label -> elapsed seconds).
    markers: HashMap<String, f64>,
    /// Total elapsed time.
    elapsed: f64,
    /// Whether the timeline is running.
    running: bool,
}

impl AnimationTimeline {
    /// Create a new empty timeline.
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            sequences: HashMap::new(),
            markers: HashMap::new(),
            elapsed: 0.0,
            running: false,
        }
    }

    /// Add a named group to the timeline.
    pub fn add_group(&mut self, name: impl Into<String>, group: AnimationGroup) {
        self.groups.insert(name.into(), group);
    }

    /// Add a named sequence to the timeline.
    pub fn add_sequence(&mut self, name: impl Into<String>, seq: AnimationSequence) {
        self.sequences.insert(name.into(), seq);
    }

    /// Add a named marker at a specific time (seconds).
    pub fn add_marker(&mut self, name: impl Into<String>, time: f64) {
        self.markers.insert(name.into(), time);
    }

    /// Start the timeline.
    pub fn start(&mut self) {
        self.running = true;
        self.elapsed = 0.0;
    }

    /// Advance the timeline by `dt` seconds.
    ///
    /// Returns a list of markers that were reached this frame.
    pub fn tick(&mut self, dt: f64) -> Vec<String> {
        if !self.running {
            return Vec::new();
        }

        let prev_elapsed = self.elapsed;
        self.elapsed += dt;

        // Tick all groups.
        for group in self.groups.values_mut() {
            group.tick(dt);
        }

        // Tick all sequences.
        for seq in self.sequences.values_mut() {
            seq.tick(dt);
        }

        // Check which markers were crossed.
        let mut reached = Vec::new();
        for (name, &time) in &self.markers {
            if prev_elapsed < time && self.elapsed >= time {
                reached.push(name.clone());
            }
        }

        // Check if everything is done.
        let all_groups_done = self.groups.values().all(|g| g.is_complete());
        let all_seqs_done = self.sequences.values().all(|s| s.is_complete());
        if all_groups_done && all_seqs_done {
            self.running = false;
        }

        reached
    }

    /// Whether the timeline is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Whether all groups and sequences have completed.
    pub fn is_complete(&self) -> bool {
        !self.running
            && self.groups.values().all(|g| g.is_complete())
            && self.sequences.values().all(|s| s.is_complete())
    }

    /// Get the total elapsed time.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Get a group by name.
    pub fn group(&self, name: &str) -> Option<&AnimationGroup> {
        self.groups.get(name)
    }

    /// Get a mutable group by name.
    pub fn group_mut(&mut self, name: &str) -> Option<&mut AnimationGroup> {
        self.groups.get_mut(name)
    }

    /// Get a sequence by name.
    pub fn sequence(&self, name: &str) -> Option<&AnimationSequence> {
        self.sequences.get(name)
    }

    /// Get a mutable sequence by name.
    pub fn sequence_mut(&mut self, name: &str) -> Option<&mut AnimationSequence> {
        self.sequences.get_mut(name)
    }

    /// Get the time of a named marker.
    pub fn marker_time(&self, name: &str) -> Option<f64> {
        self.markers.get(name).copied()
    }
}

impl Default for AnimationTimeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn id(n: u64) -> AnimationId {
        AnimationId(n)
    }

    // --- AnimationGroup tests ---

    #[test]
    fn group_starts_idle() {
        let g = AnimationGroup::new();
        assert_eq!(g.state(), GroupState::Idle);
        assert!(g.is_empty());
    }

    #[test]
    fn group_add_increases_len() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        g.add(id(2), 2.0);
        assert_eq!(g.len(), 2);
        assert!(!g.is_empty());
    }

    #[test]
    fn group_tick_starts_all() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        g.add(id(2), 1.0);
        let started = g.tick(DT);
        assert_eq!(started.len(), 2);
        assert!(started.contains(&id(1)));
        assert!(started.contains(&id(2)));
        assert_eq!(g.state(), GroupState::Playing);
    }

    #[test]
    fn group_completes_when_all_done() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 0.5);
        g.add(id(2), 1.0);
        g.tick(0.6); // id(1) finishes
        assert!(!g.is_complete());
        g.tick(0.5); // id(2) finishes
        assert!(g.is_complete());
        assert_eq!(g.state(), GroupState::Finished);
    }

    #[test]
    fn group_stagger() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        g.add(id(2), 1.0);
        g.add(id(3), 1.0);
        g.stagger(0.1);

        let started = g.tick(DT);
        assert_eq!(started, vec![id(1)]); // only first starts at t=0

        let started = g.tick(0.1);
        assert!(started.contains(&id(2))); // second starts at t~0.1

        let started = g.tick(0.1);
        assert!(started.contains(&id(3))); // third at t~0.2
    }

    #[test]
    fn group_stagger_retroactive() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        g.add(id(2), 1.0);
        // Set stagger after adding — should recompute delays.
        g.stagger(0.5);

        let started = g.tick(DT);
        assert_eq!(started, vec![id(1)]);

        let started = g.tick(0.3);
        assert!(started.is_empty()); // id(2) delay not yet met

        let started = g.tick(0.3);
        assert_eq!(started, vec![id(2)]); // now at ~0.6s, past 0.5 delay
    }

    #[test]
    fn group_pause_resume() {
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        g.tick(DT);
        assert_eq!(g.state(), GroupState::Playing);

        g.pause();
        assert_eq!(g.state(), GroupState::Paused);

        let started = g.tick(0.5);
        assert!(started.is_empty()); // paused, nothing happens

        g.resume();
        assert_eq!(g.state(), GroupState::Playing);
    }

    #[test]
    fn group_animation_ids() {
        let mut g = AnimationGroup::new();
        g.add(id(10), 1.0);
        g.add(id(20), 1.0);
        let ids = g.animation_ids();
        assert_eq!(ids, vec![id(10), id(20)]);
    }

    // --- AnimationSequence tests ---

    #[test]
    fn sequence_starts_idle() {
        let s = AnimationSequence::new();
        assert_eq!(s.state(), GroupState::Idle);
        assert!(s.is_empty());
    }

    #[test]
    fn sequence_plays_in_order() {
        let mut s = AnimationSequence::new();
        s.add(id(1), 0.5);
        s.add(id(2), 0.5);
        s.add(id(3), 0.5);

        let started = s.tick(DT);
        assert_eq!(started, Some(id(1)));
        assert_eq!(s.current_animation(), Some(id(1)));

        let started = s.tick(0.5);
        // id(1) finishes, id(2) might start same frame.
        assert_eq!(started, None); // entry.started was already true for id(1)

        let started = s.tick(DT);
        assert_eq!(started, Some(id(2)));
        assert_eq!(s.current_animation(), Some(id(2)));
    }

    #[test]
    fn sequence_completes_after_last() {
        let mut s = AnimationSequence::new();
        s.add(id(1), 0.3);
        s.add(id(2), 0.3);

        // Play through both.
        s.tick(DT);
        s.tick(0.3);
        s.tick(DT);
        s.tick(0.3);

        assert!(s.is_complete());
        assert_eq!(s.state(), GroupState::Finished);
    }

    #[test]
    fn sequence_pause_resume() {
        let mut s = AnimationSequence::new();
        s.add(id(1), 1.0);
        s.tick(DT);
        s.pause();
        assert_eq!(s.state(), GroupState::Paused);

        assert!(s.tick(0.5).is_none()); // paused

        s.resume();
        assert_eq!(s.state(), GroupState::Playing);
    }

    #[test]
    fn sequence_empty_finishes_immediately() {
        let mut s = AnimationSequence::new();
        s.tick(DT);
        assert!(s.is_complete());
    }

    #[test]
    fn sequence_len() {
        let mut s = AnimationSequence::new();
        s.add(id(1), 1.0);
        s.add(id(2), 1.0);
        assert_eq!(s.len(), 2);
        assert!(!s.is_empty());
    }

    // --- AnimationTimeline tests ---

    #[test]
    fn timeline_starts_not_running() {
        let t = AnimationTimeline::new();
        assert!(!t.is_running());
    }

    #[test]
    fn timeline_start() {
        let mut t = AnimationTimeline::new();
        t.start();
        assert!(t.is_running());
    }

    #[test]
    fn timeline_ticks_groups() {
        let mut t = AnimationTimeline::new();
        let mut g = AnimationGroup::new();
        g.add(id(1), 0.5);
        t.add_group("fade", g);
        t.start();

        t.tick(DT);
        assert!(t.is_running());

        t.tick(1.0); // group completes
        assert!(!t.is_running());
        assert!(t.is_complete());
    }

    #[test]
    fn timeline_ticks_sequences() {
        let mut t = AnimationTimeline::new();
        let mut s = AnimationSequence::new();
        s.add(id(1), 0.3);
        s.add(id(2), 0.3);
        t.add_sequence("chain", s);
        t.start();

        t.tick(0.4); // first entry done
        assert!(t.is_running());
        t.tick(0.1); // second starts
        t.tick(0.3); // second done
        assert!(!t.is_running());
    }

    #[test]
    fn timeline_markers() {
        let mut t = AnimationTimeline::new();
        let mut g = AnimationGroup::new();
        g.add(id(1), 2.0);
        t.add_group("anim", g);
        t.add_marker("halfway", 1.0);
        t.add_marker("quarter", 0.5);
        t.start();

        let reached = t.tick(0.3);
        assert!(reached.is_empty());

        let reached = t.tick(0.3); // at 0.6, crosses 0.5
        assert!(reached.contains(&"quarter".to_string()));
        assert!(!reached.contains(&"halfway".to_string()));

        let reached = t.tick(0.5); // at 1.1, crosses 1.0
        assert!(reached.contains(&"halfway".to_string()));
    }

    #[test]
    fn timeline_marker_time() {
        let mut t = AnimationTimeline::new();
        t.add_marker("start", 0.0);
        t.add_marker("end", 5.0);
        assert_eq!(t.marker_time("start"), Some(0.0));
        assert_eq!(t.marker_time("end"), Some(5.0));
        assert_eq!(t.marker_time("missing"), None);
    }

    #[test]
    fn timeline_group_access() {
        let mut t = AnimationTimeline::new();
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        t.add_group("test", g);

        assert!(t.group("test").is_some());
        assert!(t.group("missing").is_none());
        assert!(t.group_mut("test").is_some());
    }

    #[test]
    fn timeline_sequence_access() {
        let mut t = AnimationTimeline::new();
        let mut s = AnimationSequence::new();
        s.add(id(1), 1.0);
        t.add_sequence("chain", s);

        assert!(t.sequence("chain").is_some());
        assert!(t.sequence("missing").is_none());
        assert!(t.sequence_mut("chain").is_some());
    }

    #[test]
    fn timeline_elapsed() {
        let mut t = AnimationTimeline::new();
        let mut g = AnimationGroup::new();
        g.add(id(1), 5.0);
        t.add_group("long", g);
        t.start();
        t.tick(1.0);
        t.tick(0.5);
        assert!((t.elapsed() - 1.5).abs() < 0.001);
    }

    #[test]
    fn timeline_not_started_does_nothing() {
        let mut t = AnimationTimeline::new();
        let mut g = AnimationGroup::new();
        g.add(id(1), 1.0);
        t.add_group("test", g);
        // Don't call start.
        let reached = t.tick(2.0);
        assert!(reached.is_empty());
        assert!(!t.is_running());
    }

    #[test]
    fn timeline_default_trait() {
        let t = AnimationTimeline::default();
        assert!(!t.is_running());
    }

    #[test]
    fn group_default_trait() {
        let g = AnimationGroup::default();
        assert_eq!(g.state(), GroupState::Idle);
    }

    #[test]
    fn sequence_default_trait() {
        let s = AnimationSequence::default();
        assert_eq!(s.state(), GroupState::Idle);
    }
}
