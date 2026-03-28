//! Integration tests for `system_monitor` module.

use liquide_apps_task_manager::system_monitor::*;
use liquide_apps_task_manager::process::ProcessStatus;

// ===========================================================================
// MonitorProcessInfo
// ===========================================================================

#[test]
fn monitor_process_info_defaults_zero() {
    let p = MonitorProcessInfo::default();
    assert_eq!(p.pid, 0);
    assert_eq!(p.ppid, 0);
    assert_eq!(p.name, "");
    assert!(p.exe.is_none());
    assert_eq!(p.cmdline, "");
    assert_eq!(p.user, "");
    assert_eq!(p.status, ProcessStatus::Running);
    assert_eq!(p.cpu_percent, 0.0);
    assert_eq!(p.memory_bytes, 0);
    assert_eq!(p.memory_percent, 0.0);
    assert_eq!(p.threads, 0);
    assert_eq!(p.start_time, 0);
    assert_eq!(p.priority, 0);
    assert_eq!(p.io_read_bytes, 0);
    assert_eq!(p.io_write_bytes, 0);
}

#[test]
fn monitor_process_info_from_full_process_info() {
    use liquide_apps_task_manager::process::{ProcessInfo, SchedulingPriority};

    let mut full = ProcessInfo::default();
    full.pid = 100;
    full.ppid = Some(1);
    full.name = "chrome".to_string();
    full.exe_path = Some("/usr/bin/chrome".to_string());
    full.cmdline = "chrome --headless".to_string();
    full.user = "bob".to_string();
    full.status = ProcessStatus::Sleeping;
    full.cpu_percent = 33.3;
    full.mem_working_bytes = 2_097_152;
    full.threads = 8;
    full.priority = SchedulingPriority::High;
    full.disk_read_total_bytes = 10_000;
    full.disk_write_total_bytes = 5_000;

    let m = MonitorProcessInfo::from_full(&full);
    assert_eq!(m.pid, 100);
    assert_eq!(m.ppid, 1);
    assert_eq!(m.name, "chrome");
    assert_eq!(m.exe.as_deref(), Some("/usr/bin/chrome"));
    assert_eq!(m.user, "bob");
    assert_eq!(m.status, ProcessStatus::Sleeping);
    assert!((m.cpu_percent - 33.3).abs() < 0.1);
    assert_eq!(m.memory_bytes, 2_097_152);
    assert_eq!(m.threads, 8);
    assert_eq!(m.priority, 13); // High = 13
    assert_eq!(m.io_read_bytes, 10_000);
    assert_eq!(m.io_write_bytes, 5_000);
}

// ===========================================================================
// CpuInfo
// ===========================================================================

#[test]
fn cpu_info_default_empty() {
    let c = CpuInfo::default();
    assert_eq!(c.model, "");
    assert_eq!(c.cores, 0);
    assert!(c.per_core_usage.is_empty());
    assert!(c.temperature.is_none());
}

#[test]
fn cpu_info_serde() {
    let c = CpuInfo {
        model: "AMD Ryzen 9 7950X".to_string(),
        cores: 16,
        threads: 32,
        frequency_mhz: 4500,
        usage_percent: 23.4,
        per_core_usage: vec![10.0, 20.0, 30.0],
        temperature: Some(55.0),
    };
    let json = serde_json::to_string(&c).unwrap();
    let back: CpuInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.model, "AMD Ryzen 9 7950X");
    assert_eq!(back.cores, 16);
    assert_eq!(back.per_core_usage.len(), 3);
    assert_eq!(back.temperature, Some(55.0));
}

// ===========================================================================
// MemoryInfo
// ===========================================================================

#[test]
fn memory_info_percentages() {
    let m = MemoryInfo {
        total: 32_000_000_000,
        used: 24_000_000_000,
        available: 8_000_000_000,
        cached: 4_000_000_000,
        buffers: 1_000_000_000,
        swap_total: 16_000_000_000,
        swap_used: 2_000_000_000,
    };
    assert!((m.usage_percent() - 75.0).abs() < 0.01);
    assert!((m.swap_usage_percent() - 12.5).abs() < 0.01);
}

#[test]
fn memory_info_zero_division_safe() {
    let m = MemoryInfo::default();
    assert_eq!(m.usage_percent(), 0.0);
    assert_eq!(m.swap_usage_percent(), 0.0);
}

// ===========================================================================
// GpuInfo
// ===========================================================================

#[test]
fn gpu_info_memory_percent() {
    let g = GpuInfo {
        name: "RTX 3090".to_string(),
        memory_total: 24_000_000_000,
        memory_used: 12_000_000_000,
        utilization_percent: 90.0,
        temperature: Some(82.0),
    };
    assert!((g.memory_usage_percent() - 50.0).abs() < 0.01);
}

// ===========================================================================
// SystemResources
// ===========================================================================

#[test]
fn system_resources_serde_without_gpu() {
    let r = SystemResources {
        cpu: CpuInfo::default(),
        memory: MemoryInfo::default(),
        gpu: None,
        uptime_seconds: 7200,
        load_average: (2.0, 1.5, 1.0),
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: SystemResources = serde_json::from_str(&json).unwrap();
    assert!(back.gpu.is_none());
    assert_eq!(back.uptime_seconds, 7200);
}

#[test]
fn system_resources_serde_with_gpu() {
    let r = SystemResources {
        cpu: CpuInfo::default(),
        memory: MemoryInfo::default(),
        gpu: Some(GpuInfo {
            name: "RTX 4090".to_string(),
            ..Default::default()
        }),
        uptime_seconds: 100,
        load_average: (0.1, 0.2, 0.3),
    };
    let json = serde_json::to_string(&r).unwrap();
    let back: SystemResources = serde_json::from_str(&json).unwrap();
    assert!(back.gpu.is_some());
    assert_eq!(back.gpu.unwrap().name, "RTX 4090");
}

// ===========================================================================
// NetworkStats
// ===========================================================================

#[test]
fn network_stats_serde() {
    let n = NetworkStats {
        interface: "wlan0".to_string(),
        rx_bytes: 1_000_000,
        tx_bytes: 500_000,
        rx_packets: 5000,
        tx_packets: 2500,
        rx_rate_bps: 10_000,
        tx_rate_bps: 5_000,
    };
    let json = serde_json::to_string(&n).unwrap();
    let back: NetworkStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.interface, "wlan0");
    assert_eq!(back.rx_rate_bps, 10_000);
}

// ===========================================================================
// NetworkRateTracker
// ===========================================================================

#[test]
fn rate_tracker_first_sample_yields_zero_rate() {
    let mut t = NetworkRateTracker::new();
    let s = t.update("eth0", 5000, 3000, 50, 30, 1000);
    assert_eq!(s.rx_rate_bps, 0);
    assert_eq!(s.tx_rate_bps, 0);
    assert_eq!(s.interface, "eth0");
}

#[test]
fn rate_tracker_computes_byte_rates() {
    let mut t = NetworkRateTracker::new();
    t.update("eth0", 0, 0, 0, 0, 0);
    // After 500ms, 10000 rx bytes, 5000 tx bytes
    let s = t.update("eth0", 10_000, 5_000, 100, 50, 500);
    // Rate = delta * 1000 / elapsed
    assert_eq!(s.rx_rate_bps, 20_000); // 10000 * 1000 / 500
    assert_eq!(s.tx_rate_bps, 10_000); // 5000 * 1000 / 500
}

#[test]
fn rate_tracker_handles_counter_wrap_gracefully() {
    let mut t = NetworkRateTracker::new();
    t.update("eth0", 100, 50, 1, 1, 1000);
    // If counter wraps (new < old), saturating_sub gives 0
    let s = t.update("eth0", 50, 25, 1, 1, 2000);
    assert_eq!(s.rx_rate_bps, 0);
    assert_eq!(s.tx_rate_bps, 0);
}

// ===========================================================================
// ResourceHistory
// ===========================================================================

#[test]
fn resource_history_push_and_retrieve() {
    let mut h = ResourceHistory::new(5);
    h.push(100, 10.0);
    h.push(200, 20.0);
    h.push(300, 30.0);

    assert_eq!(h.len(), 3);
    let vals = h.values();
    assert_eq!(vals.len(), 3);
    assert_eq!(vals[0], (100, 10.0));
    assert_eq!(vals[2], (300, 30.0));
}

#[test]
fn resource_history_evicts_oldest() {
    let mut h = ResourceHistory::new(3);
    h.push(1, 1.0);
    h.push(2, 2.0);
    h.push(3, 3.0);
    h.push(4, 4.0);
    h.push(5, 5.0);

    assert_eq!(h.len(), 3);
    let vals = h.values();
    assert_eq!(vals, vec![(3, 3.0), (4, 4.0), (5, 5.0)]);
}

#[test]
fn resource_history_stats() {
    let mut h = ResourceHistory::new(10);
    h.push(1, 10.0);
    h.push(2, 40.0);
    h.push(3, 20.0);

    assert!((h.average() - 23.333).abs() < 0.1);
    assert!((h.peak() - 40.0).abs() < 0.01);
    assert!((h.min() - 10.0).abs() < 0.01);
}

#[test]
fn resource_history_empty_stats() {
    let h = ResourceHistory::new(10);
    assert_eq!(h.average(), 0.0);
}

#[test]
fn resource_history_last() {
    let mut h = ResourceHistory::new(5);
    assert!(h.last().is_none());
    h.push(1000, 42.0);
    assert_eq!(h.last(), Some(&(1000, 42.0)));
}

// ===========================================================================
// ProcessTree
// ===========================================================================

#[test]
fn build_tree_three_levels() {
    let procs = vec![
        MonitorProcessInfo { pid: 1, ppid: 0, name: "systemd".into(), ..Default::default() },
        MonitorProcessInfo { pid: 10, ppid: 1, name: "sshd".into(), ..Default::default() },
        MonitorProcessInfo { pid: 20, ppid: 10, name: "bash".into(), ..Default::default() },
        MonitorProcessInfo { pid: 30, ppid: 20, name: "vim".into(), ..Default::default() },
    ];

    let tree = build_tree(&procs);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].process.name, "systemd");
    assert_eq!(tree[0].children[0].process.name, "sshd");
    assert_eq!(tree[0].children[0].children[0].process.name, "bash");
    assert_eq!(tree[0].children[0].children[0].children[0].process.name, "vim");
}

#[test]
fn build_tree_multiple_roots() {
    let procs = vec![
        MonitorProcessInfo { pid: 1, ppid: 0, name: "init".into(), ..Default::default() },
        MonitorProcessInfo { pid: 2, ppid: 0, name: "kthreadd".into(), ..Default::default() },
    ];

    let tree = build_tree(&procs);
    assert_eq!(tree.len(), 2);
}

#[test]
fn find_in_tree_deep() {
    let procs = vec![
        MonitorProcessInfo { pid: 1, ppid: 0, name: "root".into(), ..Default::default() },
        MonitorProcessInfo { pid: 2, ppid: 1, name: "a".into(), ..Default::default() },
        MonitorProcessInfo { pid: 3, ppid: 2, name: "b".into(), ..Default::default() },
        MonitorProcessInfo { pid: 4, ppid: 3, name: "target".into(), ..Default::default() },
    ];
    let tree = build_tree(&procs);
    let found = find_in_tree(&tree, 4);
    assert!(found.is_some());
    assert_eq!(found.unwrap().process.name, "target");
}

#[test]
fn flatten_tree_preserves_all() {
    let procs = vec![
        MonitorProcessInfo { pid: 1, ppid: 0, name: "a".into(), ..Default::default() },
        MonitorProcessInfo { pid: 2, ppid: 1, name: "b".into(), ..Default::default() },
        MonitorProcessInfo { pid: 3, ppid: 1, name: "c".into(), ..Default::default() },
        MonitorProcessInfo { pid: 4, ppid: 2, name: "d".into(), ..Default::default() },
        MonitorProcessInfo { pid: 5, ppid: 0, name: "e".into(), ..Default::default() },
    ];
    let tree = build_tree(&procs);
    let flat = flatten_tree(&tree);
    assert_eq!(flat.len(), 5);
}

#[test]
fn count_tree_nodes_multi_level() {
    let procs = vec![
        MonitorProcessInfo { pid: 1, ppid: 0, name: "a".into(), ..Default::default() },
        MonitorProcessInfo { pid: 2, ppid: 1, name: "b".into(), ..Default::default() },
        MonitorProcessInfo { pid: 3, ppid: 2, name: "c".into(), ..Default::default() },
    ];
    let tree = build_tree(&procs);
    assert_eq!(count_tree_nodes(&tree), 3);
}

// ===========================================================================
// SystemMonitor facade
// ===========================================================================

#[test]
fn system_monitor_records_multiple_snapshots() {
    let mut m = SystemMonitor::with_capacity(5);

    for i in 0..7 {
        let r = SystemResources {
            cpu: CpuInfo {
                usage_percent: i as f32 * 10.0,
                ..Default::default()
            },
            ..Default::default()
        };
        m.record_resources(i * 1000, r);
    }

    // Capacity 5, pushed 7: should have last 5
    assert_eq!(m.cpu_history.len(), 5);
    assert!((m.cpu_history.peak() - 60.0).abs() < 0.01);
}

#[test]
fn system_monitor_network_aggregate_history() {
    let mut m = SystemMonitor::new();

    let raw1 = vec![
        NetworkStats { interface: "eth0".into(), rx_bytes: 0, tx_bytes: 0, ..Default::default() },
        NetworkStats { interface: "wlan0".into(), rx_bytes: 0, tx_bytes: 0, ..Default::default() },
    ];
    m.record_network(0, raw1);

    let raw2 = vec![
        NetworkStats { interface: "eth0".into(), rx_bytes: 5000, tx_bytes: 3000, rx_packets: 10, tx_packets: 5, ..Default::default() },
        NetworkStats { interface: "wlan0".into(), rx_bytes: 2000, tx_bytes: 1000, rx_packets: 5, tx_packets: 3, ..Default::default() },
    ];
    let results = m.record_network(1000, raw2);

    assert_eq!(results.len(), 2);
    // eth0: 5000 bytes in 1 sec
    assert_eq!(results[0].rx_rate_bps, 5000);
    // wlan0: 2000 bytes in 1 sec
    assert_eq!(results[1].rx_rate_bps, 2000);

    // Total rx_rate in history = 5000 + 2000 = 7000
    assert_eq!(m.network_rx_history.len(), 2);
    assert!((m.network_rx_history.last().unwrap().1 - 7000.0).abs() < 0.01);
}
