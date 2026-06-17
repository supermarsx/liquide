//! Runtime coordinator for the task manager.
//!
//! Owns the data pipeline: collectors → aggregator → renderers, plus the
//! action dispatcher for privileged operations.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};

use crate::aggregator::Aggregator;
use crate::collector::{CpuTracker, NativeProcessCollector, SystemMetrics};
use crate::config::TaskManagerConfig;
use crate::event::TaskManagerEvent;
use crate::process::ProcessInfo;
use crate::ui::{SortOrder, TabId};

/// Which process-table column the widget UI is sorted by.
///
/// This is the synchronous sort state that backs the widget seam
/// ([`crate::app_view`]); it is independent of the async `active_tab` and the
/// CPU-descending order `refresh` always applies to the raw snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSortColumn {
    /// Sort by the process display name (lexicographic).
    Name,
    /// Sort by process id.
    Pid,
    /// Sort by current CPU percentage.
    Cpu,
    /// Sort by working-set memory.
    Memory,
}

impl ProcessSortColumn {
    /// The stable column index used by the widget table.
    #[must_use]
    pub fn index(self) -> u32 {
        match self {
            Self::Name => 0,
            Self::Pid => 1,
            Self::Cpu => 2,
            Self::Memory => 3,
        }
    }

    /// Map a widget table column index back to a sort column.
    #[must_use]
    pub fn from_index(index: u32) -> Option<Self> {
        match index {
            0 => Some(Self::Name),
            1 => Some(Self::Pid),
            2 => Some(Self::Cpu),
            3 => Some(Self::Memory),
            _ => None,
        }
    }
}

/// Central runtime that owns all task-manager subsystems.
#[derive(Debug)]
pub struct TaskManagerRuntime {
    /// Active configuration.
    config: Arc<RwLock<TaskManagerConfig>>,
    /// Live data aggregator (ring buffers for each metric).
    aggregator: Arc<RwLock<Aggregator>>,
    /// Which tab is currently visible.
    active_tab: Arc<RwLock<TabId>>,
    /// Broadcast channel for internal events.
    event_tx: broadcast::Sender<TaskManagerEvent>,
    /// Whether the elevated helper daemon is connected.
    elevated: Arc<RwLock<bool>>,
    /// Sampling interval override (None = use config).
    sampling_override_ms: Arc<RwLock<Option<u64>>>,
    /// Per-tab pause state.
    paused_tabs: Arc<RwLock<HashMap<TabId, bool>>>,
    /// Native process collector.
    native_collector: NativeProcessCollector,
    /// Delta-based CPU percentage tracker.
    cpu_tracker: CpuTracker,
    /// Most recent process list (sorted by CPU% descending).
    processes: Vec<ProcessInfo>,
    /// Most recent system-wide metrics.
    system_metrics: SystemMetrics,
    /// Monotonic timestamp of the last refresh (ms).
    last_refresh_ms: u64,
    /// Free-text filter applied to the process list (synchronous UI state).
    filter_query: String,
    /// Synchronous mirror of the visible tab, used by the widget seam (the
    /// canonical `active_tab` lives behind an async `RwLock`; the widget model
    /// is built synchronously, so it tracks its own copy here).
    widget_tab: TabId,
    /// Column the widget process table is sorted by.
    sort_column: ProcessSortColumn,
    /// Direction of the widget process-table sort.
    sort_order: SortOrder,
    /// PID of the selected process row, if any.
    selected_pid: Option<u32>,
}

impl TaskManagerRuntime {
    /// Create a new runtime with the given configuration.
    pub fn new(config: TaskManagerConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            config: Arc::new(RwLock::new(config)),
            aggregator: Arc::new(RwLock::new(Aggregator::new(300))),
            active_tab: Arc::new(RwLock::new(TabId::Processes)),
            event_tx,
            elevated: Arc::new(RwLock::new(false)),
            sampling_override_ms: Arc::new(RwLock::new(None)),
            paused_tabs: Arc::new(RwLock::new(HashMap::new())),
            native_collector: NativeProcessCollector::new(),
            cpu_tracker: CpuTracker::new(),
            processes: Vec::new(),
            system_metrics: SystemMetrics::default(),
            last_refresh_ms: 0,
            filter_query: String::new(),
            widget_tab: TabId::Processes,
            sort_column: ProcessSortColumn::Cpu,
            sort_order: SortOrder::Descending,
            selected_pid: None,
        }
    }

    /// Current free-text process filter query.
    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    /// Mutable access to the process filter query (used by the app-view seam).
    pub(crate) fn filter_query_mut(&mut self) -> &mut String {
        &mut self.filter_query
    }

    /// Subscribe to the internal event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<TaskManagerEvent> {
        self.event_tx.subscribe()
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: TaskManagerEvent) -> usize {
        self.event_tx.send(event).unwrap_or(0)
    }

    /// Get a read handle to the current configuration.
    pub fn config(&self) -> &Arc<RwLock<TaskManagerConfig>> {
        &self.config
    }

    /// Get a read handle to the aggregator.
    pub fn aggregator(&self) -> &Arc<RwLock<Aggregator>> {
        &self.aggregator
    }

    /// Get the currently active tab.
    pub async fn active_tab(&self) -> TabId {
        self.active_tab.read().await.clone()
    }

    /// Switch to a different tab.
    pub async fn set_active_tab(&self, tab: TabId) {
        *self.active_tab.write().await = tab;
    }

    /// Check whether the elevated helper is connected.
    pub async fn is_elevated(&self) -> bool {
        *self.elevated.read().await
    }

    /// Mark the elevated helper as connected.
    pub async fn set_elevated(&self, value: bool) {
        *self.elevated.write().await = value;
    }

    /// Override the sampling interval (for high-rate or paused modes).
    pub async fn set_sampling_override(&self, ms: Option<u64>) {
        *self.sampling_override_ms.write().await = ms;
    }

    /// Get the current effective sampling interval in milliseconds.
    pub async fn effective_sampling_ms(&self) -> u64 {
        if let Some(ms) = *self.sampling_override_ms.read().await {
            return ms;
        }
        let cfg = self.config.read().await;
        cfg.performance.update_interval_ms as u64
    }

    /// Pause or resume updates for a specific tab.
    pub async fn set_tab_paused(&self, tab: TabId, paused: bool) {
        self.paused_tabs.write().await.insert(tab, paused);
    }

    /// Check whether a tab is paused.
    pub async fn is_tab_paused(&self, tab: &TabId) -> bool {
        self.paused_tabs
            .read()
            .await
            .get(tab)
            .copied()
            .unwrap_or(false)
    }

    // -- Process & metrics refresh -----------------------------------------

    /// Refresh the process list and system metrics using native OS APIs.
    ///
    /// Call this periodically (every 1-2 seconds) from the main update loop.
    /// `now_ms` is a monotonic timestamp in milliseconds used for delta-based
    /// CPU percentage computation and time-series recording.
    pub fn refresh(&mut self, now_ms: u64) {
        let elapsed = now_ms.saturating_sub(self.last_refresh_ms);

        // Collect processes via native FFI
        let mut procs = self.native_collector.collect_processes();

        // Collect system metrics
        self.system_metrics = self.native_collector.collect_system_metrics();

        // Update CPU tracker with processor count
        self.cpu_tracker
            .set_cpu_count(self.system_metrics.cpu_count);

        // Compute per-process CPU% from delta
        self.cpu_tracker.update(&mut procs, elapsed);

        // Fill in process/thread counts from the live snapshot
        self.system_metrics.process_count = procs.len() as u32;
        self.system_metrics.thread_count = procs.iter().map(|p| p.threads).sum();

        // Compute per-process memory% using system total
        if self.system_metrics.memory_total > 0 {
            let total = self.system_metrics.memory_total as f64;
            for p in &mut procs {
                let pct = p.mem_working_bytes as f64 / total * 100.0;
                // Store in the closest available field — we don't have a
                // dedicated memory_percent on ProcessInfo, but
                // mem_working_bytes is already there for absolute display.
                let _ = pct; // percentage can be computed at display time
            }
        }

        // Garbage-collect stale CPU tracker entries
        let live_pids: HashSet<u32> = procs.iter().map(|p| p.pid).collect();
        self.cpu_tracker.gc(&live_pids);

        // Sort by CPU% descending (like Win11 default)
        procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.processes = procs;
        self.last_refresh_ms = now_ms;
    }

    /// Refresh and record time-series data into the aggregator.
    ///
    /// This is the async-friendly version that acquires the aggregator lock
    /// and records CPU/memory samples for Performance tab graphs.
    pub async fn refresh_and_record(&mut self, now_ms: u64) {
        self.refresh(now_ms);

        let mut agg = self.aggregator.write().await;
        agg.record(
            "cpu.percent",
            now_ms,
            self.system_metrics.cpu_percent as f64,
        );
        let mem_pct = self.system_metrics.memory_percent as f64;
        agg.record("memory.percent", now_ms, mem_pct);
        agg.record(
            "memory.used_bytes",
            now_ms,
            self.system_metrics.memory_used as f64,
        );
    }

    /// Get the most recent process list (sorted by CPU% descending).
    pub fn visible_processes(&self) -> &[ProcessInfo] {
        &self.processes
    }

    // -- Synchronous widget UI state ---------------------------------------

    /// The tab currently shown by the widget UI.
    #[must_use]
    pub fn widget_tab(&self) -> TabId {
        self.widget_tab.clone()
    }

    /// Switch the widget UI tab. Returns `true` if it changed.
    pub fn set_widget_tab(&mut self, tab: TabId) -> bool {
        if self.widget_tab == tab {
            return false;
        }
        self.widget_tab = tab;
        true
    }

    /// The column the widget process table is sorted by.
    #[must_use]
    pub fn sort_column(&self) -> ProcessSortColumn {
        self.sort_column
    }

    /// The direction of the widget process-table sort.
    #[must_use]
    pub fn sort_order(&self) -> SortOrder {
        self.sort_order.clone()
    }

    /// Apply a sort to the given column.
    ///
    /// Clicking the already-active column toggles its direction; clicking a new
    /// column sorts it ascending. Returns `true` if the column or direction
    /// changed.
    pub fn sort_by(&mut self, column: ProcessSortColumn) -> bool {
        if self.sort_column == column {
            let toggled = match self.sort_order {
                SortOrder::Ascending => SortOrder::Descending,
                SortOrder::Descending => SortOrder::Ascending,
            };
            self.sort_order = toggled;
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        true
    }

    /// The processes ordered by the current widget sort column/direction.
    ///
    /// This does not mutate the stored snapshot (which `refresh` keeps in
    /// CPU-descending order); it returns a freshly-ordered view for the widget
    /// table so the displayed rows reflect the active sort deterministically.
    #[must_use]
    pub fn sorted_processes(&self) -> Vec<ProcessInfo> {
        let mut procs = self.processes.clone();
        let column = self.sort_column;
        procs.sort_by(|a, b| {
            let ord = match column {
                ProcessSortColumn::Name => a.name.cmp(&b.name),
                ProcessSortColumn::Pid => a.pid.cmp(&b.pid),
                ProcessSortColumn::Cpu => a
                    .cpu_percent
                    .partial_cmp(&b.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortColumn::Memory => a.mem_working_bytes.cmp(&b.mem_working_bytes),
            };
            // Tie-break on PID for a stable, deterministic order.
            ord.then(a.pid.cmp(&b.pid))
        });
        if self.sort_order == SortOrder::Descending {
            procs.reverse();
        }
        procs
    }

    /// The PID of the currently-selected process row, if any.
    #[must_use]
    pub fn selected_pid(&self) -> Option<u32> {
        self.selected_pid
    }

    /// Select a process by PID. Returns `true` if the selection changed.
    pub fn select_pid(&mut self, pid: u32) -> bool {
        if self.selected_pid == Some(pid) {
            return false;
        }
        self.selected_pid = Some(pid);
        true
    }

    /// End (remove) the selected process from the model.
    ///
    /// This mirrors the "End task" action on the synchronous model: the process
    /// is dropped from the snapshot and the selection cleared. Returns `true` if
    /// a process was removed.
    pub fn end_selected_task(&mut self) -> bool {
        let Some(pid) = self.selected_pid else {
            return false;
        };
        let before = self.processes.len();
        self.processes.retain(|p| p.pid != pid);
        let removed = self.processes.len() != before;
        if removed {
            self.selected_pid = None;
        }
        removed
    }

    /// Replace the live process snapshot with a fixed set.
    ///
    /// This freezes a known process set for deterministic widget/UI behaviour
    /// (tests, and any caller that wants to drive the model without the live
    /// tick()/`refresh` sampler). It does **not** re-sort: the snapshot is taken
    /// as given and the widget table applies the active sort on top.
    pub fn set_processes(&mut self, processes: Vec<ProcessInfo>) {
        self.processes = processes;
    }

    /// Get the most recent system-wide metrics.
    pub fn system_metrics(&self) -> &SystemMetrics {
        &self.system_metrics
    }

    /// Get CPU usage history from the aggregator (returns values 0-100).
    pub async fn cpu_history(&self) -> Vec<f64> {
        let agg = self.aggregator.read().await;
        agg.get_series("cpu.percent")
            .map(|ts| ts.samples().map(|s| s.value).collect())
            .unwrap_or_default()
    }

    /// Get memory usage history from the aggregator (returns values 0-100).
    pub async fn memory_history(&self) -> Vec<f64> {
        let agg = self.aggregator.read().await;
        agg.get_series("memory.percent")
            .map(|ts| ts.samples().map(|s| s.value).collect())
            .unwrap_or_default()
    }

    /// Get the total number of running processes.
    pub fn process_count(&self) -> u32 {
        self.processes.len() as u32
    }

    /// Get the total thread count across all processes.
    pub fn thread_count(&self) -> u32 {
        self.system_metrics.thread_count
    }
}

/// Runtime status snapshot for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    /// Currently active tab.
    pub active_tab: TabId,
    /// Whether elevated helper is connected.
    pub elevated: bool,
    /// Effective sampling interval (ms).
    pub sampling_ms: u64,
    /// Number of event subscribers.
    pub subscriber_count: usize,
}
