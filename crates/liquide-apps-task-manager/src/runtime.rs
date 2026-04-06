//! Runtime coordinator for the task manager.
//!
//! Owns the data pipeline: collectors → aggregator → renderers, plus the
//! action dispatcher for privileged operations.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::aggregator::Aggregator;
use crate::collector::{CpuTracker, NativeProcessCollector, SystemMetrics};
use crate::config::TaskManagerConfig;
use crate::event::TaskManagerEvent;
use crate::process::ProcessInfo;
use crate::ui::TabId;

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
        }
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
        self.cpu_tracker.set_cpu_count(self.system_metrics.cpu_count);

        // Compute per-process CPU% from delta
        self.cpu_tracker.update(&mut procs, elapsed);

        // Fill in process/thread counts from the live snapshot
        self.system_metrics.process_count = procs.len() as u32;
        self.system_metrics.thread_count =
            procs.iter().map(|p| p.threads).sum();

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
        agg.record("cpu.percent", now_ms, self.system_metrics.cpu_percent as f64);
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
