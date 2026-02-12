//! Process tree types for the Process Tree tab (spec section 13).
//!
//! Visualizes the full process hierarchy from init/PID 1/System down to every
//! leaf process, with features for subtree operations, orphan detection,
//! color coding, and timeline views of process lifetimes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Features available in the process tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeFeature {
    ExpandAll,
    CollapseAll,
    HighlightCritical,
    ShowOrphans,
    ShowJobObjects,
    ShowContainers,
    FilterSubtree,
    SearchInTree,
    ExportTree,
}

impl TreeFeature {
    /// Returns the string representation of this tree feature.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExpandAll => "Expand All",
            Self::CollapseAll => "Collapse All",
            Self::HighlightCritical => "Highlight Critical",
            Self::ShowOrphans => "Show Orphans",
            Self::ShowJobObjects => "Show Job Objects",
            Self::ShowContainers => "Show Containers",
            Self::FilterSubtree => "Filter Subtree",
            Self::SearchInTree => "Search in Tree",
            Self::ExportTree => "Export Tree",
        }
    }
}

impl fmt::Display for TreeFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Color coding mode for tree nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeColorMode {
    None,
    ByUser,
    ByStatus,
}

impl TreeColorMode {
    /// Returns the string representation of this color mode.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::ByUser => "By User",
            Self::ByStatus => "By Status",
        }
    }
}

impl fmt::Display for TreeColorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifetime record of a process for the timeline view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessLifetime {
    /// Process ID.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// Timestamp when the process started.
    pub start_time: Option<String>,
    /// Timestamp when the process ended, if it has exited.
    pub end_time: Option<String>,
    /// Exit code of the process, if it has exited.
    pub exit_code: Option<i32>,
}

/// A node in the process hierarchy tree, with recursive children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTreeNode {
    /// Process ID.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// User account running the process.
    pub user: String,
    /// Current CPU usage as a percentage.
    pub cpu_percent: f64,
    /// Current memory usage in bytes (working set).
    pub mem_bytes: u64,
    /// Process status string (e.g. "Running", "Suspended").
    pub status_str: String,
    /// Depth of this node in the tree (0 for root processes).
    pub depth: u32,
    /// Child processes in the tree hierarchy.
    pub children: Vec<ProcessTreeNode>,
    /// Number of threads in this process.
    pub thread_count: u32,
    /// Number of open handles in this process.
    pub handle_count: u32,
}

impl Default for ProcessTreeNode {
    fn default() -> Self {
        Self {
            pid: 0,
            name: String::new(),
            user: String::new(),
            cpu_percent: 0.0,
            mem_bytes: 0,
            status_str: String::from("Unknown"),
            depth: 0,
            children: Vec::new(),
            thread_count: 0,
            handle_count: 0,
        }
    }
}
