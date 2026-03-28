//! Scoped profiling with RAII guards and flame graph export.
//!
//! Provides [`ProfileScope`] RAII guards that automatically measure duration,
//! a per-frame [`Profiler`] that aggregates scope timings, and a
//! [`FlameEntry`] export format for visualization.

use std::collections::HashMap;
use std::time::Instant;

/// Well-known profiling scope identifiers used throughout the rendering pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeId {
    Layout,
    Style,
    Paint,
    Composite,
    HitTest,
    EventDispatch,
    DomSync,
    AnimationTick,
    /// Application-defined scope (arbitrary name via string).
    Custom(u32),
}

impl ScopeId {
    /// Human-readable label for the scope.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::Style => "Style",
            Self::Paint => "Paint",
            Self::Composite => "Composite",
            Self::HitTest => "HitTest",
            Self::EventDispatch => "EventDispatch",
            Self::DomSync => "DomSync",
            Self::AnimationTick => "AnimationTick",
            Self::Custom(_) => "Custom",
        }
    }
}

/// A single measurement recorded by a [`ProfileScope`].
#[derive(Debug, Clone, Copy)]
pub struct ScopeMeasurement {
    pub scope: ScopeId,
    /// Start time relative to frame begin (microseconds).
    pub start_us: u64,
    /// Duration of this scope (microseconds).
    pub duration_us: u64,
    /// Depth in the call stack (0 = top level).
    pub depth: u32,
}

/// Aggregated statistics for a single scope across one frame.
#[derive(Debug, Clone)]
pub struct ScopeReport {
    pub scope: ScopeId,
    /// Sum of all durations for this scope in the frame (microseconds).
    pub total_time_us: u64,
    /// Self time = total time minus time in child scopes (microseconds).
    pub self_time_us: u64,
    /// Number of times this scope was entered.
    pub call_count: u32,
}

/// An entry in a flame graph / flame chart.
#[derive(Debug, Clone)]
pub struct FlameEntry {
    /// Human-readable name.
    pub name: String,
    /// Start offset from frame begin (microseconds).
    pub start_us: u64,
    /// Duration (microseconds).
    pub duration_us: u64,
    /// Stack depth (0 = root).
    pub depth: u32,
}

/// RAII guard that records a scope's duration when dropped.
///
/// Created via [`Profiler::scope`]. The scope measurement is pushed to the
/// profiler when the guard goes out of scope.
pub struct ProfileScope<'a> {
    profiler: &'a mut Profiler,
    scope_id: ScopeId,
    start: Instant,
    depth: u32,
}

impl<'a> Drop for ProfileScope<'a> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let start_us = self
            .profiler
            .frame_start
            .map(|fs| self.start.duration_since(fs).as_micros() as u64)
            .unwrap_or(0);
        self.profiler.measurements.push(ScopeMeasurement {
            scope: self.scope_id,
            start_us,
            duration_us: elapsed.as_micros() as u64,
            depth: self.depth,
        });
        self.profiler.current_depth -= 1;
    }
}

/// Per-frame profiler that collects scope measurements.
///
/// Usage:
/// ```ignore
/// let mut profiler = Profiler::new();
/// profiler.begin_frame();
///
/// {
///     let _scope = profiler.scope(ScopeId::Style);
///     // ... do style work ...
/// }
/// {
///     let _scope = profiler.scope(ScopeId::Layout);
///     // ... do layout work ...
/// }
///
/// profiler.end_frame();
/// let report = profiler.report();
/// ```
pub struct Profiler {
    measurements: Vec<ScopeMeasurement>,
    frame_start: Option<Instant>,
    current_depth: u32,
    /// Completed frame reports (last N frames).
    history: Vec<Vec<ScopeMeasurement>>,
    history_capacity: usize,
}

impl Profiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            measurements: Vec::with_capacity(64),
            frame_start: None,
            current_depth: 0,
            history: Vec::new(),
            history_capacity: 60,
        }
    }

    /// Signal the start of a new frame.
    pub fn begin_frame(&mut self) {
        self.measurements.clear();
        self.current_depth = 0;
        self.frame_start = Some(Instant::now());
    }

    /// Signal the end of the current frame and archive measurements.
    pub fn end_frame(&mut self) {
        let completed = std::mem::replace(&mut self.measurements, Vec::with_capacity(64));
        if self.history.len() >= self.history_capacity {
            self.history.remove(0);
        }
        self.history.push(completed);
        self.frame_start = None;
    }

    /// Begin a profiling scope. The returned guard will record the scope's
    /// duration when dropped.
    pub fn scope(&mut self, scope_id: ScopeId) -> ProfileScope<'_> {
        let depth = self.current_depth;
        self.current_depth += 1;
        ProfileScope {
            profiler: self,
            scope_id,
            start: Instant::now(),
            depth,
        }
    }

    /// Record a scope measurement directly (without using an RAII guard).
    pub fn record(&mut self, scope_id: ScopeId, duration_us: u64) {
        let start_us = self
            .frame_start
            .map(|fs| Instant::now().duration_since(fs).as_micros() as u64)
            .unwrap_or(0);
        self.measurements.push(ScopeMeasurement {
            scope: scope_id,
            start_us,
            duration_us,
            depth: self.current_depth,
        });
    }

    /// Access raw measurements for the current (in-progress) frame.
    pub fn current_measurements(&self) -> &[ScopeMeasurement] {
        &self.measurements
    }

    /// Generate a report of the most recent completed frame, sorted by total time descending.
    pub fn report(&self) -> Vec<ScopeReport> {
        let frame = match self.history.last() {
            Some(f) => f,
            None => return Vec::new(),
        };
        Self::build_report(frame)
    }

    /// Generate a report for a specific frame index in history (0 = oldest).
    pub fn report_frame(&self, index: usize) -> Vec<ScopeReport> {
        match self.history.get(index) {
            Some(f) => Self::build_report(f),
            None => Vec::new(),
        }
    }

    /// Number of completed frames in history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Export the most recent frame as flame graph data.
    pub fn to_flame_graph(&self) -> Vec<FlameEntry> {
        let frame = match self.history.last() {
            Some(f) => f,
            None => return Vec::new(),
        };
        frame
            .iter()
            .map(|m| FlameEntry {
                name: m.scope.label().to_string(),
                start_us: m.start_us,
                duration_us: m.duration_us,
                depth: m.depth,
            })
            .collect()
    }

    /// Export a specific frame as flame graph data.
    pub fn to_flame_graph_frame(&self, index: usize) -> Vec<FlameEntry> {
        match self.history.get(index) {
            Some(frame) => frame
                .iter()
                .map(|m| FlameEntry {
                    name: m.scope.label().to_string(),
                    start_us: m.start_us,
                    duration_us: m.duration_us,
                    depth: m.depth,
                })
                .collect(),
            None => Vec::new(),
        }
    }

    // --- internal ---

    fn build_report(measurements: &[ScopeMeasurement]) -> Vec<ScopeReport> {
        let mut totals: HashMap<ScopeId, (u64, u32)> = HashMap::new();

        for m in measurements {
            let entry = totals.entry(m.scope).or_insert((0, 0));
            entry.0 += m.duration_us;
            entry.1 += 1;
        }

        // Compute self-time: total_time minus time of direct children.
        // Simplified: for each scope instance, children are measurements at depth+1
        // that start within its time range.
        let mut self_times: HashMap<ScopeId, u64> = HashMap::new();
        for (scope, (total, _)) in &totals {
            self_times.insert(*scope, *total);
        }

        // Subtract child durations from parent scopes
        for i in 0..measurements.len() {
            let parent = &measurements[i];
            let parent_end = parent.start_us + parent.duration_us;
            for j in (i + 1)..measurements.len() {
                let child = &measurements[j];
                if child.depth == parent.depth + 1
                    && child.start_us >= parent.start_us
                    && child.start_us + child.duration_us <= parent_end
                {
                    if let Some(st) = self_times.get_mut(&parent.scope) {
                        *st = st.saturating_sub(child.duration_us);
                    }
                }
            }
        }

        let mut reports: Vec<ScopeReport> = totals
            .into_iter()
            .map(|(scope, (total, count))| ScopeReport {
                scope,
                total_time_us: total,
                self_time_us: *self_times.get(&scope).unwrap_or(&total),
                call_count: count,
            })
            .collect();

        reports.sort_by(|a, b| b.total_time_us.cmp(&a.total_time_us));
        reports
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_id_labels() {
        assert_eq!(ScopeId::Layout.label(), "Layout");
        assert_eq!(ScopeId::Style.label(), "Style");
        assert_eq!(ScopeId::Paint.label(), "Paint");
        assert_eq!(ScopeId::Composite.label(), "Composite");
        assert_eq!(ScopeId::HitTest.label(), "HitTest");
        assert_eq!(ScopeId::EventDispatch.label(), "EventDispatch");
        assert_eq!(ScopeId::DomSync.label(), "DomSync");
        assert_eq!(ScopeId::AnimationTick.label(), "AnimationTick");
        assert_eq!(ScopeId::Custom(42).label(), "Custom");
    }

    #[test]
    fn empty_profiler_report() {
        let profiler = Profiler::new();
        assert!(profiler.report().is_empty());
        assert!(profiler.to_flame_graph().is_empty());
        assert_eq!(profiler.history_len(), 0);
    }

    #[test]
    fn record_direct_measurement() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Layout, 5000);
        profiler.record(ScopeId::Paint, 3000);
        profiler.end_frame();

        let report = profiler.report();
        assert_eq!(report.len(), 2);
        assert_eq!(report[0].scope, ScopeId::Layout);
        assert_eq!(report[0].total_time_us, 5000);
        assert_eq!(report[1].scope, ScopeId::Paint);
        assert_eq!(report[1].total_time_us, 3000);
    }

    #[test]
    fn report_sorted_by_total_time() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Paint, 1000);
        profiler.record(ScopeId::Layout, 5000);
        profiler.record(ScopeId::Style, 3000);
        profiler.end_frame();

        let report = profiler.report();
        assert_eq!(report[0].scope, ScopeId::Layout);
        assert_eq!(report[1].scope, ScopeId::Style);
        assert_eq!(report[2].scope, ScopeId::Paint);
    }

    #[test]
    fn multiple_calls_same_scope() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Layout, 1000);
        profiler.record(ScopeId::Layout, 2000);
        profiler.record(ScopeId::Layout, 3000);
        profiler.end_frame();

        let report = profiler.report();
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].total_time_us, 6000);
        assert_eq!(report[0].call_count, 3);
    }

    #[test]
    fn flame_graph_export() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Style, 2000);
        profiler.record(ScopeId::Layout, 4000);
        profiler.end_frame();

        let flame = profiler.to_flame_graph();
        assert_eq!(flame.len(), 2);
        assert_eq!(flame[0].name, "Style");
        assert_eq!(flame[0].duration_us, 2000);
        assert_eq!(flame[1].name, "Layout");
    }

    #[test]
    fn begin_frame_clears_measurements() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Layout, 1000);
        profiler.begin_frame();
        assert!(profiler.current_measurements().is_empty());
    }

    #[test]
    fn history_accumulates() {
        let mut profiler = Profiler::new();

        profiler.begin_frame();
        profiler.record(ScopeId::Layout, 1000);
        profiler.end_frame();

        profiler.begin_frame();
        profiler.record(ScopeId::Paint, 2000);
        profiler.end_frame();

        assert_eq!(profiler.history_len(), 2);

        let r0 = profiler.report_frame(0);
        assert_eq!(r0[0].scope, ScopeId::Layout);

        let r1 = profiler.report_frame(1);
        assert_eq!(r1[0].scope, ScopeId::Paint);
    }

    #[test]
    fn history_capacity_eviction() {
        let mut profiler = Profiler::new();
        // Default capacity is 60
        for i in 0..70 {
            profiler.begin_frame();
            profiler.record(ScopeId::Layout, i as u64 * 100);
            profiler.end_frame();
        }
        assert_eq!(profiler.history_len(), 60);
        // Oldest should be frame 10 (indices 0..9 evicted)
        let first = profiler.report_frame(0);
        assert_eq!(first[0].total_time_us, 1000); // frame 10 * 100
    }

    #[test]
    fn flame_graph_frame_index() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::Style, 500);
        profiler.end_frame();
        profiler.begin_frame();
        profiler.record(ScopeId::Composite, 700);
        profiler.end_frame();

        let fg0 = profiler.to_flame_graph_frame(0);
        assert_eq!(fg0[0].name, "Style");
        let fg1 = profiler.to_flame_graph_frame(1);
        assert_eq!(fg1[0].name, "Composite");
        assert!(profiler.to_flame_graph_frame(99).is_empty());
    }

    #[test]
    fn report_frame_out_of_bounds() {
        let profiler = Profiler::new();
        assert!(profiler.report_frame(0).is_empty());
    }

    #[test]
    fn scope_id_equality() {
        assert_eq!(ScopeId::Layout, ScopeId::Layout);
        assert_ne!(ScopeId::Layout, ScopeId::Paint);
        assert_eq!(ScopeId::Custom(1), ScopeId::Custom(1));
        assert_ne!(ScopeId::Custom(1), ScopeId::Custom(2));
    }

    #[test]
    fn scope_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ScopeId::Layout);
        set.insert(ScopeId::Style);
        set.insert(ScopeId::Layout); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn default_profiler() {
        let p = Profiler::default();
        assert_eq!(p.history_len(), 0);
    }

    #[test]
    fn current_measurements_during_frame() {
        let mut profiler = Profiler::new();
        profiler.begin_frame();
        profiler.record(ScopeId::HitTest, 100);
        profiler.record(ScopeId::EventDispatch, 200);
        assert_eq!(profiler.current_measurements().len(), 2);
        profiler.end_frame();
        // After end_frame, current measurements are moved to history
        assert_eq!(profiler.current_measurements().len(), 0);
    }

    #[test]
    fn flame_entry_fields() {
        let entry = FlameEntry {
            name: "TestScope".into(),
            start_us: 100,
            duration_us: 500,
            depth: 2,
        };
        assert_eq!(entry.name, "TestScope");
        assert_eq!(entry.start_us, 100);
        assert_eq!(entry.duration_us, 500);
        assert_eq!(entry.depth, 2);
    }
}
