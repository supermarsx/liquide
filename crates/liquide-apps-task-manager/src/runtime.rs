//! Runtime coordinator for the task manager.
//!
//! Owns the data pipeline: collectors → aggregator → renderers, plus the
//! action dispatcher for privileged operations.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::aggregator::Aggregator;
use crate::config::TaskManagerConfig;
use crate::event::TaskManagerEvent;
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
