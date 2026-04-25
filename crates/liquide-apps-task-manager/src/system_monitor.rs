//! Comprehensive system monitoring module.
//!
//! Provides unified system resource monitoring types, resource history
//! tracking, process tree construction, platform-specific data collection
//! bridges, and per-interface network statistics with rate computation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::process::{ProcessInfo, ProcessStatus};

// ---------------------------------------------------------------------------
// ProcessStatus helpers for simplified ProcessInfo
// ---------------------------------------------------------------------------

/// Simplified process information for system monitoring.
///
/// This is a lighter-weight view of process data compared to the full
/// [`ProcessInfo`] in the process module, focused on the fields most
/// commonly needed for system monitoring dashboards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub cmdline: String,
    pub user: String,
    pub status: ProcessStatus,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub memory_percent: f32,
    pub threads: u32,
    pub start_time: u64,
    pub priority: i32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

impl Default for MonitorProcessInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            ppid: 0,
            name: String::new(),
            exe: None,
            cmdline: String::new(),
            user: String::new(),
            status: ProcessStatus::Running,
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_percent: 0.0,
            threads: 0,
            start_time: 0,
            priority: 0,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }
}

impl MonitorProcessInfo {
    /// Convert from the full ProcessInfo type.
    pub fn from_full(p: &ProcessInfo) -> Self {
        Self {
            pid: p.pid,
            ppid: p.ppid.unwrap_or(0),
            name: p.name.clone(),
            exe: p.exe_path.clone(),
            cmdline: p.cmdline.clone(),
            user: p.user.clone(),
            status: p.status,
            cpu_percent: p.cpu_percent as f32,
            memory_bytes: p.mem_working_bytes,
            memory_percent: 0.0, // Caller fills this with system total
            threads: p.threads,
            start_time: p.uptime_secs.unwrap_or(0),
            priority: match p.priority {
                crate::process::SchedulingPriority::Realtime => 24,
                crate::process::SchedulingPriority::High => 13,
                crate::process::SchedulingPriority::AboveNormal => 10,
                crate::process::SchedulingPriority::Normal => 8,
                crate::process::SchedulingPriority::BelowNormal => 6,
                crate::process::SchedulingPriority::Idle => 4,
            },
            io_read_bytes: p.disk_read_total_bytes,
            io_write_bytes: p.disk_write_total_bytes,
        }
    }
}

// ---------------------------------------------------------------------------
// CpuInfo
// ---------------------------------------------------------------------------

/// CPU information for system monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: String,
    pub cores: u32,
    pub threads: u32,
    pub frequency_mhz: u32,
    pub usage_percent: f32,
    pub per_core_usage: Vec<f32>,
    pub temperature: Option<f32>,
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            model: String::new(),
            cores: 0,
            threads: 0,
            frequency_mhz: 0,
            usage_percent: 0.0,
            per_core_usage: Vec::new(),
            temperature: None,
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryInfo
// ---------------------------------------------------------------------------

/// Memory information for system monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub cached: u64,
    pub buffers: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl Default for MemoryInfo {
    fn default() -> Self {
        Self {
            total: 0,
            used: 0,
            available: 0,
            cached: 0,
            buffers: 0,
            swap_total: 0,
            swap_used: 0,
        }
    }
}

impl MemoryInfo {
    /// Return used memory as a percentage of total.
    pub fn usage_percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f64 / self.total as f64 * 100.0) as f32
    }

    /// Return swap usage as a percentage of total swap.
    pub fn swap_usage_percent(&self) -> f32 {
        if self.swap_total == 0 {
            return 0.0;
        }
        (self.swap_used as f64 / self.swap_total as f64 * 100.0) as f32
    }
}

// ---------------------------------------------------------------------------
// GpuInfo
// ---------------------------------------------------------------------------

/// GPU information for system monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub memory_total: u64,
    pub memory_used: u64,
    pub utilization_percent: f32,
    pub temperature: Option<f32>,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            memory_total: 0,
            memory_used: 0,
            utilization_percent: 0.0,
            temperature: None,
        }
    }
}

impl GpuInfo {
    /// Return VRAM usage as a percentage of total.
    pub fn memory_usage_percent(&self) -> f32 {
        if self.memory_total == 0 {
            return 0.0;
        }
        (self.memory_used as f64 / self.memory_total as f64 * 100.0) as f32
    }
}

// ---------------------------------------------------------------------------
// SystemResources
// ---------------------------------------------------------------------------

/// Overall system resource usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpu: Option<GpuInfo>,
    pub uptime_seconds: u64,
    pub load_average: (f64, f64, f64),
}

impl Default for SystemResources {
    fn default() -> Self {
        Self {
            cpu: CpuInfo::default(),
            memory: MemoryInfo::default(),
            gpu: None,
            uptime_seconds: 0,
            load_average: (0.0, 0.0, 0.0),
        }
    }
}

// ---------------------------------------------------------------------------
// NetworkStats
// ---------------------------------------------------------------------------

/// Per-interface network statistics with transfer rate computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_rate_bps: u64,
    pub tx_rate_bps: u64,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            interface: String::new(),
            rx_bytes: 0,
            tx_bytes: 0,
            rx_packets: 0,
            tx_packets: 0,
            rx_rate_bps: 0,
            tx_rate_bps: 0,
        }
    }
}

/// Tracks previous network counters to compute rates between samples.
#[derive(Debug, Clone)]
pub struct NetworkRateTracker {
    previous: HashMap<String, (u64, u64, u64)>, // (rx_bytes, tx_bytes, timestamp_ms)
}

impl NetworkRateTracker {
    /// Create a new rate tracker.
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
        }
    }

    /// Update counters and compute rates for an interface.
    ///
    /// Returns a `NetworkStats` with `rx_rate_bps` and `tx_rate_bps` computed
    /// from the difference since the last call for this interface.
    pub fn update(
        &mut self,
        interface: &str,
        rx_bytes: u64,
        tx_bytes: u64,
        rx_packets: u64,
        tx_packets: u64,
        timestamp_ms: u64,
    ) -> NetworkStats {
        let (rx_rate, tx_rate) =
            if let Some(&(prev_rx, prev_tx, prev_ts)) = self.previous.get(interface) {
                let elapsed_ms = timestamp_ms.saturating_sub(prev_ts);
                if elapsed_ms == 0 {
                    (0, 0)
                } else {
                    let rx_delta = rx_bytes.saturating_sub(prev_rx);
                    let tx_delta = tx_bytes.saturating_sub(prev_tx);
                    // Convert bytes per elapsed_ms to bytes per second
                    let rx_bps = rx_delta * 1000 / elapsed_ms;
                    let tx_bps = tx_delta * 1000 / elapsed_ms;
                    (rx_bps, tx_bps)
                }
            } else {
                (0, 0)
            };

        self.previous
            .insert(interface.to_string(), (rx_bytes, tx_bytes, timestamp_ms));

        NetworkStats {
            interface: interface.to_string(),
            rx_bytes,
            tx_bytes,
            rx_packets,
            tx_packets,
            rx_rate_bps: rx_rate,
            tx_rate_bps: tx_rate,
        }
    }

    /// Clear all tracked state.
    pub fn clear(&mut self) {
        self.previous.clear();
    }

    /// Return the number of interfaces being tracked.
    pub fn tracked_count(&self) -> usize {
        self.previous.len()
    }
}

impl Default for NetworkRateTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ResourceHistory
// ---------------------------------------------------------------------------

/// Time-series ring buffer of `(timestamp, value)` pairs for graphing.
///
/// Default capacity is 300 samples (5 minutes at 1 sample/second).
#[derive(Debug, Clone)]
pub struct ResourceHistory {
    buf: Vec<(u64, f32)>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl ResourceHistory {
    /// Create a new resource history with the given capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ResourceHistory capacity must be > 0");
        Self {
            buf: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Create a new resource history with the default capacity of 300.
    pub fn with_default_capacity() -> Self {
        Self::new(300)
    }

    /// Push a new `(timestamp, value)` pair, evicting the oldest if full.
    pub fn push(&mut self, timestamp: u64, value: f32) {
        let item = (timestamp, value);
        if self.buf.len() < self.capacity {
            self.buf.push(item);
            self.len = self.buf.len();
            self.head = self.len;
        } else {
            let idx = self.head % self.capacity;
            self.buf[idx] = item;
            self.head = idx + 1;
            self.len = self.capacity;
        }
    }

    /// Return all stored values in chronological order (oldest first).
    pub fn values(&self) -> Vec<(u64, f32)> {
        let mut result = Vec::with_capacity(self.len);
        for i in 0..self.len {
            if let Some(item) = self.get(i) {
                result.push(*item);
            }
        }
        result
    }

    /// Get an item by logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<&(u64, f32)> {
        if index >= self.len {
            return None;
        }
        if self.len < self.capacity {
            Some(&self.buf[index])
        } else {
            let actual = (self.head + index) % self.capacity;
            Some(&self.buf[actual])
        }
    }

    /// Get the most recently pushed item.
    pub fn last(&self) -> Option<&(u64, f32)> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Return the arithmetic mean of all stored values.
    pub fn average(&self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let sum: f64 = self.values().iter().map(|&(_, v)| v as f64).sum();
        (sum / self.len as f64) as f32
    }

    /// Return the peak (maximum) value in the history.
    pub fn peak(&self) -> f32 {
        self.values()
            .iter()
            .map(|&(_, v)| v)
            .fold(f32::NEG_INFINITY, f32::max)
    }

    /// Return the minimum value in the history.
    pub fn min(&self) -> f32 {
        self.values()
            .iter()
            .map(|&(_, v)| v)
            .fold(f32::INFINITY, f32::min)
    }

    /// Number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether the buffer has reached capacity.
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Clear all stored data.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.head = 0;
        self.len = 0;
    }
}

impl Default for ResourceHistory {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

// ---------------------------------------------------------------------------
// ProcessNode / ProcessTree
// ---------------------------------------------------------------------------

/// A node in the process hierarchy tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessNode {
    pub process: MonitorProcessInfo,
    pub children: Vec<ProcessNode>,
}

/// Build a process hierarchy tree from a flat list of processes.
///
/// Processes whose parent is not in the list become root nodes.
/// The tree is built in a single pass using a HashMap for O(n) construction.
pub fn build_tree(processes: &[MonitorProcessInfo]) -> Vec<ProcessNode> {
    if processes.is_empty() {
        return Vec::new();
    }

    // Collect all PIDs for quick parent lookup.
    let pid_set: std::collections::HashSet<u32> = processes.iter().map(|p| p.pid).collect();

    // Build a mapping from ppid -> list of children indices.
    let mut children_map: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, proc) in processes.iter().enumerate() {
        if proc.ppid == 0 || proc.ppid == proc.pid || !pid_set.contains(&proc.ppid) {
            root_indices.push(i);
        } else {
            children_map
                .entry(proc.ppid)
                .or_insert_with(Vec::new)
                .push(i);
        }
    }

    fn build_subtree(
        processes: &[MonitorProcessInfo],
        children_map: &HashMap<u32, Vec<usize>>,
        index: usize,
    ) -> ProcessNode {
        let proc = &processes[index];
        let children = if let Some(child_indices) = children_map.get(&proc.pid) {
            child_indices
                .iter()
                .map(|&ci| build_subtree(processes, children_map, ci))
                .collect()
        } else {
            Vec::new()
        };
        ProcessNode {
            process: proc.clone(),
            children,
        }
    }

    root_indices
        .iter()
        .map(|&i| build_subtree(processes, &children_map, i))
        .collect()
}

/// Count the total number of nodes in a tree (including all descendants).
pub fn count_tree_nodes(roots: &[ProcessNode]) -> usize {
    fn count_recursive(node: &ProcessNode) -> usize {
        1 + node.children.iter().map(count_recursive).sum::<usize>()
    }
    roots.iter().map(count_recursive).sum()
}

/// Find a process by PID in the tree, returning a reference if found.
pub fn find_in_tree(roots: &[ProcessNode], pid: u32) -> Option<&ProcessNode> {
    fn find_recursive(node: &ProcessNode, pid: u32) -> Option<&ProcessNode> {
        if node.process.pid == pid {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_recursive(child, pid) {
                return Some(found);
            }
        }
        None
    }

    for root in roots {
        if let Some(found) = find_recursive(root, pid) {
            return Some(found);
        }
    }
    None
}

/// Flatten a process tree back into a linear list (depth-first pre-order).
pub fn flatten_tree(roots: &[ProcessNode]) -> Vec<&MonitorProcessInfo> {
    fn collect_recursive<'a>(node: &'a ProcessNode, out: &mut Vec<&'a MonitorProcessInfo>) {
        out.push(&node.process);
        for child in &node.children {
            collect_recursive(child, out);
        }
    }

    let mut result = Vec::new();
    for root in roots {
        collect_recursive(root, &mut result);
    }
    result
}

// ---------------------------------------------------------------------------
// Platform bridges
// ---------------------------------------------------------------------------

/// Platform-specific system resource collector.
///
/// Implementations read from OS-specific data sources and return
/// normalized types.
pub trait SystemMonitorCollector {
    /// Collect a snapshot of system resources (CPU, memory, GPU, uptime, load).
    fn collect_resources(&self) -> Result<SystemResources, String>;

    /// Collect a list of all running processes with monitoring-level detail.
    fn collect_processes(&self) -> Result<Vec<MonitorProcessInfo>, String>;

    /// Collect per-interface network statistics.
    fn collect_network(&self) -> Result<Vec<NetworkStats>, String>;
}

// ---------------------------------------------------------------------------
// Linux platform bridge
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use std::fs;

    /// Linux system monitor using /proc and /sys filesystems.
    pub struct LinuxCollector;

    impl LinuxCollector {
        pub fn new() -> Self {
            Self
        }

        /// Parse /proc/stat for CPU usage.
        fn parse_proc_stat() -> Result<(f32, Vec<f32>), String> {
            let content =
                fs::read_to_string("/proc/stat").map_err(|e| format!("read /proc/stat: {e}"))?;

            let mut total_usage = 0.0f32;
            let mut per_core = Vec::new();

            for line in content.lines() {
                if line.starts_with("cpu") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 5 {
                        continue;
                    }

                    let user: u64 = parts[1].parse().unwrap_or(0);
                    let nice: u64 = parts[2].parse().unwrap_or(0);
                    let system: u64 = parts[3].parse().unwrap_or(0);
                    let idle: u64 = parts[4].parse().unwrap_or(0);
                    let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let irq: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let softirq: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);

                    let total = user + nice + system + idle + iowait + irq + softirq;
                    let busy = total - idle - iowait;

                    let usage = if total > 0 {
                        (busy as f64 / total as f64 * 100.0) as f32
                    } else {
                        0.0
                    };

                    if parts[0] == "cpu" {
                        total_usage = usage;
                    } else {
                        per_core.push(usage);
                    }
                }
            }

            Ok((total_usage, per_core))
        }

        /// Parse /proc/meminfo for memory statistics.
        fn parse_proc_meminfo() -> Result<MemoryInfo, String> {
            let content = fs::read_to_string("/proc/meminfo")
                .map_err(|e| format!("read /proc/meminfo: {e}"))?;

            let mut info = MemoryInfo::default();

            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                let key = parts[0].trim_end_matches(':');
                let value_kb: u64 = parts[1].parse().unwrap_or(0);
                let value_bytes = value_kb * 1024;

                match key {
                    "MemTotal" => info.total = value_bytes,
                    "MemFree" | "MemAvailable" => {
                        if key == "MemAvailable" {
                            info.available = value_bytes;
                        }
                    }
                    "Buffers" => info.buffers = value_bytes,
                    "Cached" => info.cached = value_bytes,
                    "SwapTotal" => info.swap_total = value_bytes,
                    "SwapFree" => {
                        // swap_used = swap_total - swap_free, computed after loop
                    }
                    _ => {}
                }
            }

            // Compute used = total - available
            info.used = info.total.saturating_sub(info.available);

            // Re-read for swap_free to compute swap_used
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0].trim_end_matches(':') == "SwapFree" {
                    let swap_free_bytes: u64 = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    info.swap_used = info.swap_total.saturating_sub(swap_free_bytes);
                    break;
                }
            }

            Ok(info)
        }

        /// Parse /proc/net/dev for network interface statistics.
        fn parse_proc_net_dev() -> Result<Vec<NetworkStats>, String> {
            let content = fs::read_to_string("/proc/net/dev")
                .map_err(|e| format!("read /proc/net/dev: {e}"))?;

            let mut stats = Vec::new();

            for line in content.lines().skip(2) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 11 {
                    continue;
                }

                let iface = parts[0].trim_end_matches(':');
                let rx_bytes: u64 = parts[1].parse().unwrap_or(0);
                let rx_packets: u64 = parts[2].parse().unwrap_or(0);
                let tx_bytes: u64 = parts[9].parse().unwrap_or(0);
                let tx_packets: u64 = parts[10].parse().unwrap_or(0);

                stats.push(NetworkStats {
                    interface: iface.to_string(),
                    rx_bytes,
                    tx_bytes,
                    rx_packets,
                    tx_packets,
                    rx_rate_bps: 0, // Caller uses NetworkRateTracker
                    tx_rate_bps: 0,
                });
            }

            Ok(stats)
        }

        /// Parse /proc/[pid]/stat for a single process.
        fn parse_proc_pid(pid: u32) -> Result<MonitorProcessInfo, String> {
            let stat_path = format!("/proc/{pid}/stat");
            let stat_content =
                fs::read_to_string(&stat_path).map_err(|e| format!("read {stat_path}: {e}"))?;

            // The comm field is in parens and may contain spaces.
            let open = stat_content
                .find('(')
                .ok_or_else(|| format!("malformed /proc/{pid}/stat"))?;
            let close = stat_content
                .rfind(')')
                .ok_or_else(|| format!("malformed /proc/{pid}/stat"))?;

            let name = stat_content[open + 1..close].to_string();
            let rest = &stat_content[close + 2..]; // skip ") "
            let fields: Vec<&str> = rest.split_whitespace().collect();

            if fields.len() < 20 {
                return Err(format!("/proc/{pid}/stat has too few fields"));
            }

            let state_char = fields[0];
            let ppid: u32 = fields[1].parse().unwrap_or(0);
            let priority: i32 = fields[15].parse().unwrap_or(0);
            let threads: u32 = fields[17].parse().unwrap_or(1);
            let start_time: u64 = fields[19].parse().unwrap_or(0);

            let status = match state_char {
                "R" => ProcessStatus::Running,
                "S" => ProcessStatus::Sleeping,
                "D" => ProcessStatus::DiskSleep,
                "Z" => ProcessStatus::Zombie,
                "T" => ProcessStatus::Stopped,
                "t" => ProcessStatus::Stopped,
                "X" | "x" => ProcessStatus::Stopped,
                "W" => ProcessStatus::Waiting,
                "I" => ProcessStatus::Idle,
                _ => ProcessStatus::Running,
            };

            // Read memory from /proc/[pid]/statm
            let statm_path = format!("/proc/{pid}/statm");
            let memory_bytes = if let Ok(statm) = fs::read_to_string(&statm_path) {
                let parts: Vec<&str> = statm.split_whitespace().collect();
                // Field 1 is resident pages
                let resident_pages: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                resident_pages * 4096 // Assume 4KB pages
            } else {
                0
            };

            // Read exe path
            let exe = fs::read_link(format!("/proc/{pid}/exe"))
                .ok()
                .map(|p| p.to_string_lossy().to_string());

            // Read cmdline
            let cmdline = fs::read_to_string(format!("/proc/{pid}/cmdline"))
                .unwrap_or_default()
                .replace('\0', " ")
                .trim()
                .to_string();

            // Read I/O stats from /proc/[pid]/io (may fail without permissions)
            let (io_read, io_write) =
                if let Ok(io_content) = fs::read_to_string(format!("/proc/{pid}/io")) {
                    let mut read = 0u64;
                    let mut write = 0u64;
                    for line in io_content.lines() {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() == 2 {
                            let key = parts[0].trim();
                            let val: u64 = parts[1].trim().parse().unwrap_or(0);
                            match key {
                                "read_bytes" => read = val,
                                "write_bytes" => write = val,
                                _ => {}
                            }
                        }
                    }
                    (read, write)
                } else {
                    (0, 0)
                };

            Ok(MonitorProcessInfo {
                pid,
                ppid,
                name,
                exe,
                cmdline,
                user: String::new(), // Would need /proc/[pid]/status or uid lookup
                status,
                cpu_percent: 0.0, // Requires delta computation
                memory_bytes,
                memory_percent: 0.0,
                threads,
                start_time,
                priority,
                io_read_bytes: io_read,
                io_write_bytes: io_write,
            })
        }

        /// Read CPU model from /proc/cpuinfo.
        fn cpu_model() -> String {
            if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
                for line in content.lines() {
                    if line.starts_with("model name") {
                        if let Some(val) = line.split(':').nth(1) {
                            return val.trim().to_string();
                        }
                    }
                }
            }
            "Unknown CPU".to_string()
        }

        /// Read system uptime from /proc/uptime.
        fn uptime_seconds() -> u64 {
            if let Ok(content) = fs::read_to_string("/proc/uptime") {
                if let Some(secs_str) = content.split_whitespace().next() {
                    return secs_str.parse::<f64>().unwrap_or(0.0) as u64;
                }
            }
            0
        }

        /// Read load average from /proc/loadavg.
        fn load_average() -> (f64, f64, f64) {
            if let Ok(content) = fs::read_to_string("/proc/loadavg") {
                let parts: Vec<&str> = content.split_whitespace().collect();
                if parts.len() >= 3 {
                    let a: f64 = parts[0].parse().unwrap_or(0.0);
                    let b: f64 = parts[1].parse().unwrap_or(0.0);
                    let c: f64 = parts[2].parse().unwrap_or(0.0);
                    return (a, b, c);
                }
            }
            (0.0, 0.0, 0.0)
        }
    }

    impl Default for LinuxCollector {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SystemMonitorCollector for LinuxCollector {
        fn collect_resources(&self) -> Result<SystemResources, String> {
            let (total_cpu, per_core) = Self::parse_proc_stat()?;
            let memory = Self::parse_proc_meminfo()?;
            let model = Self::cpu_model();
            let uptime = Self::uptime_seconds();
            let load = Self::load_average();

            Ok(SystemResources {
                cpu: CpuInfo {
                    model,
                    cores: per_core.len() as u32,
                    threads: per_core.len() as u32,
                    frequency_mhz: 0, // Would need /proc/cpuinfo or cpufreq
                    usage_percent: total_cpu,
                    per_core_usage: per_core,
                    temperature: None, // Would need /sys/class/thermal
                },
                memory,
                gpu: None, // Would need nvidia-smi / sysfs GPU parsing
                uptime_seconds: uptime,
                load_average: load,
            })
        }

        fn collect_processes(&self) -> Result<Vec<MonitorProcessInfo>, String> {
            let entries = fs::read_dir("/proc").map_err(|e| format!("read /proc: {e}"))?;

            let mut processes = Vec::new();

            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if let Ok(pid) = name_str.parse::<u32>() {
                    if let Ok(info) = Self::parse_proc_pid(pid) {
                        processes.push(info);
                    }
                }
            }

            Ok(processes)
        }

        fn collect_network(&self) -> Result<Vec<NetworkStats>, String> {
            Self::parse_proc_net_dev()
        }
    }
}

// ---------------------------------------------------------------------------
// Windows platform bridge
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use std::process::Command;

    /// Windows system monitor using system APIs.
    pub struct WindowsCollector;

    impl WindowsCollector {
        pub fn new() -> Self {
            Self
        }

        /// Query system information using PowerShell's Get-CimInstance.
        fn run_powershell(script: &str) -> Result<String, String> {
            let output = Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .output()
                .map_err(|e| format!("powershell exec failed: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("powershell error: {stderr}"));
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }

        /// Parse a "key = value" or "key : value" line.
        fn parse_kv_line<'a>(line: &'a str, sep: char) -> Option<(&'a str, &'a str)> {
            let mut parts = line.splitn(2, sep);
            let key = parts.next()?.trim();
            let val = parts.next()?.trim();
            if key.is_empty() {
                return None;
            }
            Some((key, val))
        }

        /// Collect CPU information.
        fn collect_cpu() -> Result<CpuInfo, String> {
            let script = r#"
                $p = Get-CimInstance Win32_Processor | Select-Object -First 1
                Write-Output "Name=$($p.Name)"
                Write-Output "NumberOfCores=$($p.NumberOfCores)"
                Write-Output "NumberOfLogicalProcessors=$($p.NumberOfLogicalProcessors)"
                Write-Output "MaxClockSpeed=$($p.MaxClockSpeed)"
                Write-Output "LoadPercentage=$($p.LoadPercentage)"
            "#;

            let output = Self::run_powershell(script)?;

            let mut info = CpuInfo::default();

            for line in output.lines() {
                if let Some((key, val)) = Self::parse_kv_line(line, '=') {
                    match key {
                        "Name" => info.model = val.to_string(),
                        "NumberOfCores" => info.cores = val.parse().unwrap_or(0),
                        "NumberOfLogicalProcessors" => info.threads = val.parse().unwrap_or(0),
                        "MaxClockSpeed" => info.frequency_mhz = val.parse().unwrap_or(0),
                        "LoadPercentage" => {
                            info.usage_percent = val.parse().unwrap_or(0.0);
                        }
                        _ => {}
                    }
                }
            }

            Ok(info)
        }

        /// Collect memory information.
        fn collect_memory() -> Result<MemoryInfo, String> {
            let script = r#"
                $os = Get-CimInstance Win32_OperatingSystem
                $cs = Get-CimInstance Win32_ComputerSystem
                Write-Output "TotalVisibleMemorySize=$($os.TotalVisibleMemorySize)"
                Write-Output "FreePhysicalMemory=$($os.FreePhysicalMemory)"
                Write-Output "TotalVirtualMemorySize=$($os.TotalVirtualMemorySize)"
                Write-Output "FreeVirtualMemory=$($os.FreeVirtualMemory)"
                Write-Output "SizeStoredInPagingFiles=$($os.SizeStoredInPagingFiles)"
                Write-Output "FreeSpaceInPagingFiles=$($os.FreeSpaceInPagingFiles)"
            "#;

            let output = Self::run_powershell(script)?;

            let mut total_kb: u64 = 0;
            let mut free_kb: u64 = 0;
            let mut swap_total_kb: u64 = 0;
            let mut swap_free_kb: u64 = 0;

            for line in output.lines() {
                if let Some((key, val)) = Self::parse_kv_line(line, '=') {
                    match key {
                        "TotalVisibleMemorySize" => total_kb = val.parse().unwrap_or(0),
                        "FreePhysicalMemory" => free_kb = val.parse().unwrap_or(0),
                        "SizeStoredInPagingFiles" => swap_total_kb = val.parse().unwrap_or(0),
                        "FreeSpaceInPagingFiles" => swap_free_kb = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }

            Ok(MemoryInfo {
                total: total_kb * 1024,
                available: free_kb * 1024,
                used: (total_kb.saturating_sub(free_kb)) * 1024,
                cached: 0, // Windows doesn't separate cached easily via CIM
                buffers: 0,
                swap_total: swap_total_kb * 1024,
                swap_used: (swap_total_kb.saturating_sub(swap_free_kb)) * 1024,
            })
        }

        /// Collect process list.
        fn collect_process_list() -> Result<Vec<MonitorProcessInfo>, String> {
            let script = r#"
                Get-Process | ForEach-Object {
                    $p = $_
                    Write-Output "---"
                    Write-Output "Id=$($p.Id)"
                    Write-Output "ProcessName=$($p.ProcessName)"
                    Write-Output "WorkingSet64=$($p.WorkingSet64)"
                    Write-Output "Threads=$($p.Threads.Count)"
                    Write-Output "CPU=$($p.CPU)"
                    Write-Output "Path=$($p.Path)"
                    Write-Output "StartTime=$($p.StartTime)"
                    Write-Output "PriorityClass=$($p.PriorityClass)"
                }
            "#;

            let output = Self::run_powershell(script)?;

            let mut processes = Vec::new();
            let mut current = MonitorProcessInfo::default();
            let mut in_record = false;

            for line in output.lines() {
                let line = line.trim();
                if line == "---" {
                    if in_record {
                        processes.push(current);
                    }
                    current = MonitorProcessInfo::default();
                    in_record = true;
                    continue;
                }

                if let Some((key, val)) = Self::parse_kv_line(line, '=') {
                    match key {
                        "Id" => current.pid = val.parse().unwrap_or(0),
                        "ProcessName" => current.name = val.to_string(),
                        "WorkingSet64" => current.memory_bytes = val.parse().unwrap_or(0),
                        "Threads" => current.threads = val.parse().unwrap_or(0),
                        "CPU" => {
                            current.cpu_percent = val.parse().unwrap_or(0.0);
                        }
                        "Path" => {
                            if !val.is_empty() {
                                current.exe = Some(val.to_string());
                            }
                        }
                        "PriorityClass" => {
                            current.priority = match val {
                                "RealTime" => 24,
                                "High" => 13,
                                "AboveNormal" => 10,
                                "Normal" => 8,
                                "BelowNormal" => 6,
                                "Idle" => 4,
                                _ => 8,
                            };
                        }
                        _ => {}
                    }
                }
            }

            if in_record {
                processes.push(current);
            }

            Ok(processes)
        }

        /// Collect uptime.
        fn uptime_seconds() -> u64 {
            let script = r#"[math]::Round((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime | Select-Object -ExpandProperty TotalSeconds)"#;
            Self::run_powershell(script)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
        }
    }

    impl Default for WindowsCollector {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SystemMonitorCollector for WindowsCollector {
        fn collect_resources(&self) -> Result<SystemResources, String> {
            let cpu = Self::collect_cpu()?;
            let memory = Self::collect_memory()?;
            let uptime = Self::uptime_seconds();

            Ok(SystemResources {
                cpu,
                memory,
                gpu: None,
                uptime_seconds: uptime,
                load_average: (0.0, 0.0, 0.0), // Windows doesn't have UNIX load average
            })
        }

        fn collect_processes(&self) -> Result<Vec<MonitorProcessInfo>, String> {
            Self::collect_process_list()
        }

        fn collect_network(&self) -> Result<Vec<NetworkStats>, String> {
            let script = r#"
                Get-NetAdapterStatistics | ForEach-Object {
                    Write-Output "---"
                    Write-Output "Name=$($_.Name)"
                    Write-Output "ReceivedBytes=$($_.ReceivedBytes)"
                    Write-Output "SentBytes=$($_.SentBytes)"
                    Write-Output "ReceivedUnicastPackets=$($_.ReceivedUnicastPackets)"
                    Write-Output "SentUnicastPackets=$($_.SentUnicastPackets)"
                }
            "#;

            let output = Self::run_powershell(script)?;

            let mut stats_list = Vec::new();
            let mut current = NetworkStats::default();
            let mut in_record = false;

            for line in output.lines() {
                let line = line.trim();
                if line == "---" {
                    if in_record {
                        stats_list.push(current);
                    }
                    current = NetworkStats::default();
                    in_record = true;
                    continue;
                }

                if let Some((key, val)) = Self::parse_kv_line(line, '=') {
                    match key {
                        "Name" => current.interface = val.to_string(),
                        "ReceivedBytes" => current.rx_bytes = val.parse().unwrap_or(0),
                        "SentBytes" => current.tx_bytes = val.parse().unwrap_or(0),
                        "ReceivedUnicastPackets" => {
                            current.rx_packets = val.parse().unwrap_or(0);
                        }
                        "SentUnicastPackets" => {
                            current.tx_packets = val.parse().unwrap_or(0);
                        }
                        _ => {}
                    }
                }
            }

            if in_record {
                stats_list.push(current);
            }

            Ok(stats_list)
        }
    }
}

// ---------------------------------------------------------------------------
// macOS platform bridge
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::process::Command;

    /// macOS system monitor using sysctl, vm_stat, and ps.
    pub struct MacOsCollector;

    impl MacOsCollector {
        pub fn new() -> Self {
            Self
        }

        /// Run a command and return stdout.
        fn run_command(cmd: &str, args: &[&str]) -> Result<String, String> {
            let output = Command::new(cmd)
                .args(args)
                .output()
                .map_err(|e| format!("{cmd} exec failed: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("{cmd} error: {stderr}"));
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }

        /// Get CPU info via sysctl.
        fn collect_cpu() -> Result<CpuInfo, String> {
            let model = Self::run_command("sysctl", &["-n", "machdep.cpu.brand_string"])
                .unwrap_or_else(|_| "Unknown CPU".to_string())
                .trim()
                .to_string();

            let cores: u32 = Self::run_command("sysctl", &["-n", "hw.physicalcpu"])
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            let threads: u32 = Self::run_command("sysctl", &["-n", "hw.logicalcpu"])
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            let freq_hz: u64 = Self::run_command("sysctl", &["-n", "hw.cpufrequency"])
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            Ok(CpuInfo {
                model,
                cores,
                threads,
                frequency_mhz: (freq_hz / 1_000_000) as u32,
                usage_percent: 0.0, // Would need host_statistics for CPU ticks
                per_core_usage: Vec::new(),
                temperature: None,
            })
        }

        /// Get memory info via vm_stat and sysctl.
        fn collect_memory() -> Result<MemoryInfo, String> {
            let total_bytes: u64 = Self::run_command("sysctl", &["-n", "hw.memsize"])
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            let vm_stat = Self::run_command("vm_stat", &[])?;

            let mut free_pages: u64 = 0;
            let mut active_pages: u64 = 0;
            let mut inactive_pages: u64 = 0;
            let mut wired_pages: u64 = 0;
            let mut compressor_pages: u64 = 0;
            let page_size: u64 = 16384; // Apple Silicon default; Intel uses 4096

            for line in vm_stat.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let key = parts[0].trim();
                let val: u64 = parts[1].trim().trim_end_matches('.').parse().unwrap_or(0);

                match key {
                    "Pages free" => free_pages = val,
                    "Pages active" => active_pages = val,
                    "Pages inactive" => inactive_pages = val,
                    "Pages wired down" => wired_pages = val,
                    "Pages occupied by compressor" => compressor_pages = val,
                    _ => {}
                }
            }

            let used = (active_pages + wired_pages + compressor_pages) * page_size;
            let available = (free_pages + inactive_pages) * page_size;
            let cached = inactive_pages * page_size;

            // Get swap via sysctl
            let swap_output =
                Self::run_command("sysctl", &["-n", "vm.swapusage"]).unwrap_or_default();
            let mut swap_total: u64 = 0;
            let mut swap_used: u64 = 0;
            for part in swap_output.split_whitespace() {
                if part.ends_with('M') {
                    let val: f64 = part.trim_end_matches('M').parse().unwrap_or(0.0);
                    if swap_total == 0 {
                        swap_total = (val * 1024.0 * 1024.0) as u64;
                    } else if swap_used == 0 {
                        swap_used = (val * 1024.0 * 1024.0) as u64;
                    }
                }
            }

            Ok(MemoryInfo {
                total: total_bytes,
                used,
                available,
                cached,
                buffers: 0,
                swap_total,
                swap_used,
            })
        }

        /// Collect processes via ps.
        fn collect_processes_via_ps() -> Result<Vec<MonitorProcessInfo>, String> {
            let output =
                Self::run_command("ps", &["aux", "-o", "pid,ppid,pcpu,rss,stat,user,command"])?;

            let mut processes = Vec::new();

            for line in output.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 11 {
                    continue;
                }

                let user = parts[0].to_string();
                let pid: u32 = parts[1].parse().unwrap_or(0);
                let cpu_percent: f32 = parts[2].parse().unwrap_or(0.0);
                let rss_kb: u64 = parts[3].parse().unwrap_or(0);
                let stat = parts[7];
                let command = parts[10..].join(" ");

                let status = if stat.starts_with('R') {
                    ProcessStatus::Running
                } else if stat.starts_with('S') {
                    ProcessStatus::Sleeping
                } else if stat.starts_with('T') {
                    ProcessStatus::Stopped
                } else if stat.starts_with('Z') {
                    ProcessStatus::Zombie
                } else if stat.starts_with('I') {
                    ProcessStatus::Idle
                } else {
                    ProcessStatus::Running
                };

                let name = command
                    .split('/')
                    .last()
                    .unwrap_or(&command)
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();

                processes.push(MonitorProcessInfo {
                    pid,
                    ppid: 0, // ps aux doesn't show ppid by default
                    name,
                    exe: None,
                    cmdline: command,
                    user,
                    status,
                    cpu_percent,
                    memory_bytes: rss_kb * 1024,
                    memory_percent: 0.0,
                    threads: 0,
                    start_time: 0,
                    priority: 0,
                    io_read_bytes: 0,
                    io_write_bytes: 0,
                });
            }

            Ok(processes)
        }

        /// Get uptime via sysctl.
        fn uptime_seconds() -> u64 {
            // sysctl kern.boottime returns struct timeval
            let output = Self::run_command("sysctl", &["-n", "kern.boottime"]).unwrap_or_default();
            // Format: "{ sec = 1234567890, usec = 0 }"
            if let Some(start) = output.find("sec = ") {
                let rest = &output[start + 6..];
                if let Some(end) = rest.find(',') {
                    let boot_time: u64 = rest[..end].trim().parse().unwrap_or(0);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    return now.saturating_sub(boot_time);
                }
            }
            0
        }

        /// Get load average via sysctl.
        fn load_average() -> (f64, f64, f64) {
            let output = Self::run_command("sysctl", &["-n", "vm.loadavg"]).unwrap_or_default();
            // Format: "{ 1.23 0.45 0.67 }"
            let cleaned = output.replace('{', "").replace('}', "");
            let parts: Vec<&str> = cleaned.split_whitespace().collect();
            if parts.len() >= 3 {
                let a: f64 = parts[0].parse().unwrap_or(0.0);
                let b: f64 = parts[1].parse().unwrap_or(0.0);
                let c: f64 = parts[2].parse().unwrap_or(0.0);
                (a, b, c)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
    }

    impl Default for MacOsCollector {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SystemMonitorCollector for MacOsCollector {
        fn collect_resources(&self) -> Result<SystemResources, String> {
            let cpu = Self::collect_cpu()?;
            let memory = Self::collect_memory()?;
            let uptime = Self::uptime_seconds();
            let load = Self::load_average();

            Ok(SystemResources {
                cpu,
                memory,
                gpu: None,
                uptime_seconds: uptime,
                load_average: load,
            })
        }

        fn collect_processes(&self) -> Result<Vec<MonitorProcessInfo>, String> {
            Self::collect_processes_via_ps()
        }

        fn collect_network(&self) -> Result<Vec<NetworkStats>, String> {
            // macOS: use netstat -ib for interface statistics
            let output = Self::run_command("netstat", &["-ib"])?;

            let mut stats = Vec::new();
            let mut seen = std::collections::HashSet::new();

            for line in output.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 7 {
                    continue;
                }

                let iface = parts[0];
                if seen.contains(iface) {
                    continue; // Skip duplicate entries (IPv4/IPv6)
                }
                seen.insert(iface.to_string());

                // netstat -ib columns: Name Mtu Network Address Ipkts Ibytes Opkts Obytes
                let rx_packets: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
                let rx_bytes: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                let tx_packets: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                let tx_bytes: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);

                stats.push(NetworkStats {
                    interface: iface.to_string(),
                    rx_bytes,
                    tx_bytes,
                    rx_packets,
                    tx_packets,
                    rx_rate_bps: 0,
                    tx_rate_bps: 0,
                });
            }

            Ok(stats)
        }
    }
}

// ---------------------------------------------------------------------------
// SystemMonitor — unified monitoring facade
// ---------------------------------------------------------------------------

/// Unified system monitor that tracks history and computes derived metrics.
///
/// Wraps a platform-specific collector and adds resource history tracking,
/// rate computation, and process tree construction.
#[derive(Debug)]
pub struct SystemMonitor {
    pub cpu_history: ResourceHistory,
    pub memory_history: ResourceHistory,
    pub gpu_history: ResourceHistory,
    pub network_rx_history: ResourceHistory,
    pub network_tx_history: ResourceHistory,
    pub network_tracker: NetworkRateTracker,
    last_resources: Option<SystemResources>,
    last_processes: Vec<MonitorProcessInfo>,
}

impl SystemMonitor {
    /// Create a new system monitor with default history capacity (300 samples).
    pub fn new() -> Self {
        Self::with_capacity(300)
    }

    /// Create a new system monitor with a custom history capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cpu_history: ResourceHistory::new(capacity),
            memory_history: ResourceHistory::new(capacity),
            gpu_history: ResourceHistory::new(capacity),
            network_rx_history: ResourceHistory::new(capacity),
            network_tx_history: ResourceHistory::new(capacity),
            network_tracker: NetworkRateTracker::new(),
            last_resources: None,
            last_processes: Vec::new(),
        }
    }

    /// Record a system resources snapshot, updating all histories.
    pub fn record_resources(&mut self, timestamp_ms: u64, resources: SystemResources) {
        self.cpu_history
            .push(timestamp_ms, resources.cpu.usage_percent);
        self.memory_history
            .push(timestamp_ms, resources.memory.usage_percent());

        if let Some(ref gpu) = resources.gpu {
            self.gpu_history.push(timestamp_ms, gpu.utilization_percent);
        }

        self.last_resources = Some(resources);
    }

    /// Record network statistics, computing rates from previous samples.
    pub fn record_network(
        &mut self,
        timestamp_ms: u64,
        raw_stats: Vec<NetworkStats>,
    ) -> Vec<NetworkStats> {
        let mut results = Vec::new();
        let mut total_rx_rate = 0u64;
        let mut total_tx_rate = 0u64;

        for raw in raw_stats {
            let updated = self.network_tracker.update(
                &raw.interface,
                raw.rx_bytes,
                raw.tx_bytes,
                raw.rx_packets,
                raw.tx_packets,
                timestamp_ms,
            );
            total_rx_rate += updated.rx_rate_bps;
            total_tx_rate += updated.tx_rate_bps;
            results.push(updated);
        }

        self.network_rx_history
            .push(timestamp_ms, total_rx_rate as f32);
        self.network_tx_history
            .push(timestamp_ms, total_tx_rate as f32);

        results
    }

    /// Store a process list snapshot.
    pub fn record_processes(&mut self, processes: Vec<MonitorProcessInfo>) {
        self.last_processes = processes;
    }

    /// Get the most recent system resources snapshot.
    pub fn last_resources(&self) -> Option<&SystemResources> {
        self.last_resources.as_ref()
    }

    /// Get the most recent process list.
    pub fn last_processes(&self) -> &[MonitorProcessInfo] {
        &self.last_processes
    }

    /// Build a process tree from the most recently recorded process list.
    pub fn process_tree(&self) -> Vec<ProcessNode> {
        build_tree(&self.last_processes)
    }

    /// Get the top N processes by CPU usage.
    pub fn top_by_cpu(&self, n: usize) -> Vec<&MonitorProcessInfo> {
        let mut sorted: Vec<&MonitorProcessInfo> = self.last_processes.iter().collect();
        sorted.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(n);
        sorted
    }

    /// Get the top N processes by memory usage.
    pub fn top_by_memory(&self, n: usize) -> Vec<&MonitorProcessInfo> {
        let mut sorted: Vec<&MonitorProcessInfo> = self.last_processes.iter().collect();
        sorted.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        sorted.truncate(n);
        sorted
    }

    /// Clear all recorded history and state.
    pub fn clear(&mut self) {
        self.cpu_history.clear();
        self.memory_history.clear();
        self.gpu_history.clear();
        self.network_rx_history.clear();
        self.network_tx_history.clear();
        self.network_tracker.clear();
        self.last_resources = None;
        self.last_processes.clear();
    }
}

impl Default for SystemMonitor {
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

    // -- MonitorProcessInfo --

    #[test]
    fn monitor_process_info_default() {
        let p = MonitorProcessInfo::default();
        assert_eq!(p.pid, 0);
        assert_eq!(p.ppid, 0);
        assert_eq!(p.name, "");
        assert_eq!(p.cpu_percent, 0.0);
        assert_eq!(p.memory_bytes, 0);
        assert_eq!(p.threads, 0);
    }

    #[test]
    fn monitor_process_info_from_full() {
        let mut full = ProcessInfo::default();
        full.pid = 42;
        full.ppid = Some(1);
        full.name = "firefox".to_string();
        full.exe_path = Some("/usr/bin/firefox".to_string());
        full.cmdline = "firefox --new-tab".to_string();
        full.user = "alice".to_string();
        full.cpu_percent = 25.5;
        full.mem_working_bytes = 1_048_576;
        full.threads = 12;
        full.disk_read_total_bytes = 5000;
        full.disk_write_total_bytes = 3000;

        let m = MonitorProcessInfo::from_full(&full);
        assert_eq!(m.pid, 42);
        assert_eq!(m.ppid, 1);
        assert_eq!(m.name, "firefox");
        assert_eq!(m.exe.as_deref(), Some("/usr/bin/firefox"));
        assert_eq!(m.cmdline, "firefox --new-tab");
        assert_eq!(m.user, "alice");
        assert!((m.cpu_percent - 25.5).abs() < 0.01);
        assert_eq!(m.memory_bytes, 1_048_576);
        assert_eq!(m.threads, 12);
        assert_eq!(m.io_read_bytes, 5000);
        assert_eq!(m.io_write_bytes, 3000);
    }

    // -- CpuInfo --

    #[test]
    fn cpu_info_default() {
        let c = CpuInfo::default();
        assert_eq!(c.model, "");
        assert_eq!(c.cores, 0);
        assert_eq!(c.threads, 0);
        assert_eq!(c.frequency_mhz, 0);
        assert_eq!(c.usage_percent, 0.0);
        assert!(c.per_core_usage.is_empty());
        assert!(c.temperature.is_none());
    }

    // -- MemoryInfo --

    #[test]
    fn memory_info_default() {
        let m = MemoryInfo::default();
        assert_eq!(m.total, 0);
        assert_eq!(m.used, 0);
        assert_eq!(m.available, 0);
    }

    #[test]
    fn memory_info_usage_percent() {
        let m = MemoryInfo {
            total: 16_000_000_000,
            used: 8_000_000_000,
            available: 8_000_000_000,
            cached: 0,
            buffers: 0,
            swap_total: 0,
            swap_used: 0,
        };
        assert!((m.usage_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn memory_info_usage_percent_zero_total() {
        let m = MemoryInfo::default();
        assert_eq!(m.usage_percent(), 0.0);
    }

    #[test]
    fn memory_info_swap_usage_percent() {
        let m = MemoryInfo {
            total: 0,
            used: 0,
            available: 0,
            cached: 0,
            buffers: 0,
            swap_total: 4_000_000_000,
            swap_used: 1_000_000_000,
        };
        assert!((m.swap_usage_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn memory_info_swap_zero_total() {
        let m = MemoryInfo::default();
        assert_eq!(m.swap_usage_percent(), 0.0);
    }

    // -- GpuInfo --

    #[test]
    fn gpu_info_default() {
        let g = GpuInfo::default();
        assert_eq!(g.name, "");
        assert_eq!(g.memory_total, 0);
    }

    #[test]
    fn gpu_info_memory_usage_percent() {
        let g = GpuInfo {
            name: "RTX 4090".to_string(),
            memory_total: 24_000_000_000,
            memory_used: 6_000_000_000,
            utilization_percent: 75.0,
            temperature: Some(65.0),
        };
        assert!((g.memory_usage_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn gpu_info_memory_usage_zero_total() {
        let g = GpuInfo::default();
        assert_eq!(g.memory_usage_percent(), 0.0);
    }

    // -- SystemResources --

    #[test]
    fn system_resources_default() {
        let r = SystemResources::default();
        assert_eq!(r.uptime_seconds, 0);
        assert!(r.gpu.is_none());
        assert_eq!(r.load_average, (0.0, 0.0, 0.0));
    }

    #[test]
    fn system_resources_with_gpu() {
        let r = SystemResources {
            cpu: CpuInfo {
                model: "Intel i9".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: 3600,
                usage_percent: 45.0,
                per_core_usage: vec![30.0, 50.0, 40.0, 60.0, 35.0, 55.0, 45.0, 65.0],
                temperature: Some(72.0),
            },
            memory: MemoryInfo {
                total: 32_000_000_000,
                used: 20_000_000_000,
                available: 12_000_000_000,
                cached: 5_000_000_000,
                buffers: 1_000_000_000,
                swap_total: 8_000_000_000,
                swap_used: 500_000_000,
            },
            gpu: Some(GpuInfo {
                name: "RTX 4090".to_string(),
                memory_total: 24_000_000_000,
                memory_used: 8_000_000_000,
                utilization_percent: 80.0,
                temperature: Some(70.0),
            }),
            uptime_seconds: 86400,
            load_average: (1.5, 1.2, 0.8),
        };

        assert_eq!(r.cpu.cores, 8);
        assert!(r.gpu.is_some());
        assert_eq!(r.uptime_seconds, 86400);
    }

    // -- NetworkStats --

    #[test]
    fn network_stats_default() {
        let n = NetworkStats::default();
        assert_eq!(n.interface, "");
        assert_eq!(n.rx_bytes, 0);
        assert_eq!(n.tx_bytes, 0);
    }

    // -- NetworkRateTracker --

    #[test]
    fn network_rate_tracker_first_sample_zero_rate() {
        let mut tracker = NetworkRateTracker::new();
        let stats = tracker.update("eth0", 1000, 500, 10, 5, 1000);
        assert_eq!(stats.rx_rate_bps, 0);
        assert_eq!(stats.tx_rate_bps, 0);
        assert_eq!(stats.rx_bytes, 1000);
        assert_eq!(stats.tx_bytes, 500);
    }

    #[test]
    fn network_rate_tracker_computes_rates() {
        let mut tracker = NetworkRateTracker::new();
        tracker.update("eth0", 1000, 500, 10, 5, 1000);

        // 1 second later, 2000 more rx bytes and 1000 more tx bytes
        let stats = tracker.update("eth0", 3000, 1500, 20, 10, 2000);
        assert_eq!(stats.rx_rate_bps, 2000); // 2000 bytes / 1 second
        assert_eq!(stats.tx_rate_bps, 1000); // 1000 bytes / 1 second
    }

    #[test]
    fn network_rate_tracker_multiple_interfaces() {
        let mut tracker = NetworkRateTracker::new();
        tracker.update("eth0", 1000, 500, 10, 5, 1000);
        tracker.update("wlan0", 2000, 1000, 20, 10, 1000);

        assert_eq!(tracker.tracked_count(), 2);

        let eth0 = tracker.update("eth0", 2000, 1500, 20, 10, 2000);
        assert_eq!(eth0.rx_rate_bps, 1000);

        let wlan0 = tracker.update("wlan0", 5000, 2000, 30, 15, 2000);
        assert_eq!(wlan0.rx_rate_bps, 3000);
    }

    #[test]
    fn network_rate_tracker_clear() {
        let mut tracker = NetworkRateTracker::new();
        tracker.update("eth0", 1000, 500, 10, 5, 1000);
        assert_eq!(tracker.tracked_count(), 1);
        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn network_rate_tracker_zero_elapsed() {
        let mut tracker = NetworkRateTracker::new();
        tracker.update("eth0", 1000, 500, 10, 5, 1000);
        // Same timestamp, should return zero rate (no division by zero)
        let stats = tracker.update("eth0", 2000, 1000, 20, 10, 1000);
        assert_eq!(stats.rx_rate_bps, 0);
        assert_eq!(stats.tx_rate_bps, 0);
    }

    // -- ResourceHistory --

    #[test]
    fn resource_history_new() {
        let h = ResourceHistory::new(10);
        assert_eq!(h.len(), 0);
        assert!(h.is_empty());
        assert_eq!(h.capacity(), 10);
        assert!(!h.is_full());
    }

    #[test]
    fn resource_history_default_capacity() {
        let h = ResourceHistory::with_default_capacity();
        assert_eq!(h.capacity(), 300);
    }

    #[test]
    fn resource_history_push_and_get() {
        let mut h = ResourceHistory::new(5);
        h.push(1000, 10.0);
        h.push(2000, 20.0);
        h.push(3000, 30.0);

        assert_eq!(h.len(), 3);
        assert_eq!(h.get(0), Some(&(1000, 10.0)));
        assert_eq!(h.get(1), Some(&(2000, 20.0)));
        assert_eq!(h.get(2), Some(&(3000, 30.0)));
        assert_eq!(h.get(3), None);
    }

    #[test]
    fn resource_history_wraps() {
        let mut h = ResourceHistory::new(3);
        h.push(1000, 10.0);
        h.push(2000, 20.0);
        h.push(3000, 30.0);
        h.push(4000, 40.0); // evicts (1000, 10.0)

        assert_eq!(h.len(), 3);
        assert!(h.is_full());

        let vals = h.values();
        assert_eq!(vals, vec![(2000, 20.0), (3000, 30.0), (4000, 40.0)]);
    }

    #[test]
    fn resource_history_last() {
        let mut h = ResourceHistory::new(5);
        assert!(h.last().is_none());
        h.push(1000, 42.0);
        assert_eq!(h.last(), Some(&(1000, 42.0)));
        h.push(2000, 99.0);
        assert_eq!(h.last(), Some(&(2000, 99.0)));
    }

    #[test]
    fn resource_history_average() {
        let mut h = ResourceHistory::new(10);
        h.push(1000, 10.0);
        h.push(2000, 20.0);
        h.push(3000, 30.0);
        assert!((h.average() - 20.0).abs() < 0.01);
    }

    #[test]
    fn resource_history_average_empty() {
        let h = ResourceHistory::new(10);
        assert_eq!(h.average(), 0.0);
    }

    #[test]
    fn resource_history_peak() {
        let mut h = ResourceHistory::new(10);
        h.push(1000, 10.0);
        h.push(2000, 50.0);
        h.push(3000, 30.0);
        assert!((h.peak() - 50.0).abs() < 0.01);
    }

    #[test]
    fn resource_history_min() {
        let mut h = ResourceHistory::new(10);
        h.push(1000, 10.0);
        h.push(2000, 5.0);
        h.push(3000, 30.0);
        assert!((h.min() - 5.0).abs() < 0.01);
    }

    #[test]
    fn resource_history_clear() {
        let mut h = ResourceHistory::new(10);
        h.push(1000, 10.0);
        h.push(2000, 20.0);
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn resource_history_values_chronological() {
        let mut h = ResourceHistory::new(4);
        for i in 0..6 {
            h.push(i * 1000, i as f32 * 10.0);
        }
        // Capacity 4, pushed 6 items: should have [2, 3, 4, 5]
        let vals = h.values();
        assert_eq!(vals.len(), 4);
        assert_eq!(vals[0], (2000, 20.0));
        assert_eq!(vals[3], (5000, 50.0));
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn resource_history_zero_capacity_panics() {
        ResourceHistory::new(0);
    }

    // -- ProcessNode / build_tree --

    #[test]
    fn build_tree_empty() {
        let tree = build_tree(&[]);
        assert!(tree.is_empty());
    }

    #[test]
    fn build_tree_single_root() {
        let procs = vec![MonitorProcessInfo {
            pid: 1,
            ppid: 0,
            name: "init".to_string(),
            ..Default::default()
        }];
        let tree = build_tree(&procs);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].process.pid, 1);
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn build_tree_parent_child() {
        let procs = vec![
            MonitorProcessInfo {
                pid: 1,
                ppid: 0,
                name: "init".to_string(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                ppid: 1,
                name: "bash".to_string(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                ppid: 1,
                name: "sshd".to_string(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 4,
                ppid: 2,
                name: "vim".to_string(),
                ..Default::default()
            },
        ];

        let tree = build_tree(&procs);
        assert_eq!(tree.len(), 1); // Only init is root
        assert_eq!(tree[0].children.len(), 2); // bash, sshd
        let bash = &tree[0].children[0];
        assert_eq!(bash.process.name, "bash");
        assert_eq!(bash.children.len(), 1); // vim
        assert_eq!(bash.children[0].process.name, "vim");
    }

    #[test]
    fn build_tree_orphan_becomes_root() {
        let procs = vec![
            MonitorProcessInfo {
                pid: 10,
                ppid: 999, // Parent not in list
                name: "orphan".to_string(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 20,
                ppid: 10,
                name: "child".to_string(),
                ..Default::default()
            },
        ];

        let tree = build_tree(&procs);
        assert_eq!(tree.len(), 1); // orphan becomes root
        assert_eq!(tree[0].process.pid, 10);
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn count_tree_nodes_works() {
        let procs = vec![
            MonitorProcessInfo {
                pid: 1,
                ppid: 0,
                name: "root".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                ppid: 1,
                name: "a".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                ppid: 1,
                name: "b".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 4,
                ppid: 2,
                name: "c".into(),
                ..Default::default()
            },
        ];
        let tree = build_tree(&procs);
        assert_eq!(count_tree_nodes(&tree), 4);
    }

    #[test]
    fn find_in_tree_found() {
        let procs = vec![
            MonitorProcessInfo {
                pid: 1,
                ppid: 0,
                name: "root".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                ppid: 1,
                name: "child".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                ppid: 2,
                name: "grandchild".into(),
                ..Default::default()
            },
        ];
        let tree = build_tree(&procs);

        let found = find_in_tree(&tree, 3);
        assert!(found.is_some());
        assert_eq!(found.unwrap().process.name, "grandchild");
    }

    #[test]
    fn find_in_tree_not_found() {
        let procs = vec![MonitorProcessInfo {
            pid: 1,
            ppid: 0,
            name: "root".into(),
            ..Default::default()
        }];
        let tree = build_tree(&procs);
        assert!(find_in_tree(&tree, 999).is_none());
    }

    #[test]
    fn flatten_tree_order() {
        let procs = vec![
            MonitorProcessInfo {
                pid: 1,
                ppid: 0,
                name: "root".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                ppid: 1,
                name: "a".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                ppid: 2,
                name: "b".into(),
                ..Default::default()
            },
        ];
        let tree = build_tree(&procs);
        let flat = flatten_tree(&tree);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].pid, 1);
        assert_eq!(flat[1].pid, 2);
        assert_eq!(flat[2].pid, 3);
    }

    // -- SystemMonitor --

    #[test]
    fn system_monitor_new() {
        let m = SystemMonitor::new();
        assert!(m.last_resources().is_none());
        assert!(m.last_processes().is_empty());
        assert!(m.cpu_history.is_empty());
    }

    #[test]
    fn system_monitor_record_resources() {
        let mut m = SystemMonitor::new();
        let resources = SystemResources {
            cpu: CpuInfo {
                usage_percent: 45.0,
                ..Default::default()
            },
            memory: MemoryInfo {
                total: 16_000_000_000,
                used: 8_000_000_000,
                available: 8_000_000_000,
                ..Default::default()
            },
            gpu: Some(GpuInfo {
                utilization_percent: 80.0,
                ..Default::default()
            }),
            uptime_seconds: 3600,
            load_average: (1.0, 0.5, 0.3),
        };

        m.record_resources(1000, resources);
        assert_eq!(m.cpu_history.len(), 1);
        assert_eq!(m.memory_history.len(), 1);
        assert_eq!(m.gpu_history.len(), 1);
        assert!(m.last_resources().is_some());
    }

    #[test]
    fn system_monitor_record_network() {
        let mut m = SystemMonitor::new();
        let raw = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 1000,
            tx_bytes: 500,
            rx_packets: 10,
            tx_packets: 5,
            rx_rate_bps: 0,
            tx_rate_bps: 0,
        }];

        let first = m.record_network(1000, raw.clone());
        assert_eq!(first[0].rx_rate_bps, 0);

        let raw2 = vec![NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 3000,
            tx_bytes: 1500,
            rx_packets: 20,
            tx_packets: 10,
            rx_rate_bps: 0,
            tx_rate_bps: 0,
        }];
        let second = m.record_network(2000, raw2);
        assert_eq!(second[0].rx_rate_bps, 2000);
        assert_eq!(second[0].tx_rate_bps, 1000);
    }

    #[test]
    fn system_monitor_process_tree() {
        let mut m = SystemMonitor::new();
        m.record_processes(vec![
            MonitorProcessInfo {
                pid: 1,
                ppid: 0,
                name: "init".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                ppid: 1,
                name: "bash".into(),
                ..Default::default()
            },
        ]);

        let tree = m.process_tree();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn system_monitor_top_by_cpu() {
        let mut m = SystemMonitor::new();
        m.record_processes(vec![
            MonitorProcessInfo {
                pid: 1,
                cpu_percent: 10.0,
                name: "a".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                cpu_percent: 50.0,
                name: "b".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                cpu_percent: 30.0,
                name: "c".into(),
                ..Default::default()
            },
        ]);

        let top = m.top_by_cpu(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].pid, 2);
        assert_eq!(top[1].pid, 3);
    }

    #[test]
    fn system_monitor_top_by_memory() {
        let mut m = SystemMonitor::new();
        m.record_processes(vec![
            MonitorProcessInfo {
                pid: 1,
                memory_bytes: 1000,
                name: "a".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 2,
                memory_bytes: 5000,
                name: "b".into(),
                ..Default::default()
            },
            MonitorProcessInfo {
                pid: 3,
                memory_bytes: 3000,
                name: "c".into(),
                ..Default::default()
            },
        ]);

        let top = m.top_by_memory(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].pid, 2);
        assert_eq!(top[1].pid, 3);
    }

    #[test]
    fn system_monitor_clear() {
        let mut m = SystemMonitor::new();
        m.record_resources(1000, SystemResources::default());
        m.record_processes(vec![MonitorProcessInfo {
            pid: 1,
            ..Default::default()
        }]);
        m.clear();

        assert!(m.cpu_history.is_empty());
        assert!(m.last_resources().is_none());
        assert!(m.last_processes().is_empty());
    }

    // -- Serde roundtrips --

    #[test]
    fn monitor_process_info_serde_roundtrip() {
        let p = MonitorProcessInfo {
            pid: 42,
            ppid: 1,
            name: "test".to_string(),
            exe: Some("/usr/bin/test".to_string()),
            cmdline: "test --flag".to_string(),
            user: "root".to_string(),
            status: ProcessStatus::Running,
            cpu_percent: 12.5,
            memory_bytes: 1024,
            memory_percent: 5.0,
            threads: 4,
            start_time: 1000,
            priority: 8,
            io_read_bytes: 500,
            io_write_bytes: 300,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: MonitorProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pid, 42);
        assert_eq!(back.name, "test");
    }

    #[test]
    fn system_resources_serde_roundtrip() {
        let r = SystemResources {
            cpu: CpuInfo {
                model: "Intel i9".to_string(),
                cores: 8,
                threads: 16,
                frequency_mhz: 3600,
                usage_percent: 45.0,
                per_core_usage: vec![30.0, 50.0],
                temperature: Some(72.0),
            },
            memory: MemoryInfo {
                total: 32_000_000_000,
                used: 20_000_000_000,
                available: 12_000_000_000,
                cached: 0,
                buffers: 0,
                swap_total: 0,
                swap_used: 0,
            },
            gpu: None,
            uptime_seconds: 3600,
            load_average: (1.0, 0.5, 0.3),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: SystemResources = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cpu.model, "Intel i9");
        assert_eq!(back.uptime_seconds, 3600);
    }

    #[test]
    fn network_stats_serde_roundtrip() {
        let n = NetworkStats {
            interface: "eth0".to_string(),
            rx_bytes: 100_000,
            tx_bytes: 50_000,
            rx_packets: 1000,
            tx_packets: 500,
            rx_rate_bps: 2000,
            tx_rate_bps: 1000,
        };
        let json = serde_json::to_string(&n).unwrap();
        let back: NetworkStats = serde_json::from_str(&json).unwrap();
        assert_eq!(back.interface, "eth0");
        assert_eq!(back.rx_rate_bps, 2000);
    }

    #[test]
    fn gpu_info_serde_roundtrip() {
        let g = GpuInfo {
            name: "RTX 4090".to_string(),
            memory_total: 24_000_000_000,
            memory_used: 8_000_000_000,
            utilization_percent: 80.0,
            temperature: Some(70.0),
        };
        let json = serde_json::to_string(&g).unwrap();
        let back: GpuInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "RTX 4090");
        assert!((back.utilization_percent - 80.0).abs() < 0.01);
    }

    // -- ProcessNode serde --

    #[test]
    fn process_node_serde_roundtrip() {
        let node = ProcessNode {
            process: MonitorProcessInfo {
                pid: 1,
                name: "init".to_string(),
                ..Default::default()
            },
            children: vec![ProcessNode {
                process: MonitorProcessInfo {
                    pid: 2,
                    ppid: 1,
                    name: "child".to_string(),
                    ..Default::default()
                },
                children: vec![],
            }],
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: ProcessNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.process.pid, 1);
        assert_eq!(back.children.len(), 1);
        assert_eq!(back.children[0].process.pid, 2);
    }
}
