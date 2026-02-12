use liquide_apps_task_manager::performance::{
    AudioPerfStats, CpuGraphType, CpuStats, DiskGraphType, DiskStats, GpuGraphType, GpuStats,
    GraphControls, MemoryGraphType, MemoryStats, NetworkGraphType, NetworkPerfStats,
    PerformanceResource, PowerStats, TimeRange,
};

// ===========================================================================
// CpuGraphType (6 variants)
// ===========================================================================

#[test]
fn cpu_graph_type_as_str_all() {
    assert_eq!(CpuGraphType::OverallUtilization.as_str(), "Overall Utilization");
    assert_eq!(CpuGraphType::PerCoreUtilization.as_str(), "Per-Core Utilization");
    assert_eq!(CpuGraphType::PerCoreFrequency.as_str(), "Per-Core Frequency");
    assert_eq!(CpuGraphType::NumaNodeView.as_str(), "NUMA Node View");
    assert_eq!(CpuGraphType::KernelVsUser.as_str(), "Kernel vs User");
    assert_eq!(CpuGraphType::CoreHeatmap.as_str(), "Core Heatmap");
}

#[test]
fn cpu_graph_type_display() {
    assert_eq!(format!("{}", CpuGraphType::NumaNodeView), "NUMA Node View");
}

#[test]
fn cpu_graph_type_serde_roundtrip() {
    let gt = CpuGraphType::CoreHeatmap;
    let json = serde_json::to_string(&gt).unwrap();
    assert_eq!(json, "\"core_heatmap\"");
    let back: CpuGraphType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gt);
}

// ===========================================================================
// MemoryGraphType (3 variants)
// ===========================================================================

#[test]
fn memory_graph_type_as_str_all() {
    assert_eq!(MemoryGraphType::Composition.as_str(), "Composition");
    assert_eq!(MemoryGraphType::CommitCharge.as_str(), "Commit Charge");
    assert_eq!(MemoryGraphType::PageFaults.as_str(), "Page Faults");
}

#[test]
fn memory_graph_type_serde_roundtrip() {
    let gt = MemoryGraphType::CommitCharge;
    let json = serde_json::to_string(&gt).unwrap();
    assert_eq!(json, "\"commit_charge\"");
    let back: MemoryGraphType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gt);
}

// ===========================================================================
// DiskGraphType (5 variants)
// ===========================================================================

#[test]
fn disk_graph_type_as_str_all() {
    assert_eq!(DiskGraphType::ActiveTime.as_str(), "Active Time");
    assert_eq!(DiskGraphType::TransferRate.as_str(), "Transfer Rate");
    assert_eq!(DiskGraphType::Iops.as_str(), "IOPS");
    assert_eq!(DiskGraphType::QueueDepth.as_str(), "Queue Depth");
    assert_eq!(DiskGraphType::Latency.as_str(), "Latency");
}

#[test]
fn disk_graph_type_serde_roundtrip() {
    let gt = DiskGraphType::Iops;
    let json = serde_json::to_string(&gt).unwrap();
    assert_eq!(json, "\"iops\"");
    let back: DiskGraphType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gt);
}

// ===========================================================================
// GpuGraphType (9 variants)
// ===========================================================================

#[test]
fn gpu_graph_type_as_str_all() {
    assert_eq!(GpuGraphType::Overall.as_str(), "Overall");
    assert_eq!(GpuGraphType::Engine3d.as_str(), "3D Engine");
    assert_eq!(GpuGraphType::CopyEngine.as_str(), "Copy Engine");
    assert_eq!(GpuGraphType::VideoDecode.as_str(), "Video Decode");
    assert_eq!(GpuGraphType::VideoEncode.as_str(), "Video Encode");
    assert_eq!(GpuGraphType::Compute.as_str(), "Compute");
    assert_eq!(GpuGraphType::VramUsage.as_str(), "VRAM Usage");
    assert_eq!(GpuGraphType::Temperature.as_str(), "Temperature");
    assert_eq!(GpuGraphType::FanSpeed.as_str(), "Fan Speed");
}

#[test]
fn gpu_graph_type_display() {
    assert_eq!(format!("{}", GpuGraphType::Engine3d), "3D Engine");
}

#[test]
fn gpu_graph_type_serde_roundtrip() {
    let gt = GpuGraphType::VramUsage;
    let json = serde_json::to_string(&gt).unwrap();
    assert_eq!(json, "\"vram_usage\"");
    let back: GpuGraphType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gt);
}

// ===========================================================================
// NetworkGraphType (3 variants)
// ===========================================================================

#[test]
fn network_graph_type_as_str_all() {
    assert_eq!(NetworkGraphType::Throughput.as_str(), "Throughput");
    assert_eq!(NetworkGraphType::ConnectionCount.as_str(), "Connection Count");
    assert_eq!(NetworkGraphType::PacketRate.as_str(), "Packet Rate");
}

#[test]
fn network_graph_type_serde_roundtrip() {
    let gt = NetworkGraphType::PacketRate;
    let json = serde_json::to_string(&gt).unwrap();
    assert_eq!(json, "\"packet_rate\"");
    let back: NetworkGraphType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, gt);
}

// ===========================================================================
// PerformanceResource (8 variants, some with data)
// ===========================================================================

#[test]
fn performance_resource_as_str_all() {
    assert_eq!(PerformanceResource::Cpu.as_str(), "CPU");
    assert_eq!(PerformanceResource::Memory.as_str(), "Memory");
    assert_eq!(PerformanceResource::Disk(0).as_str(), "Disk");
    assert_eq!(PerformanceResource::Gpu(1).as_str(), "GPU");
    assert_eq!(PerformanceResource::Network.as_str(), "Network");
    assert_eq!(PerformanceResource::Power.as_str(), "Power");
    assert_eq!(PerformanceResource::Bluetooth.as_str(), "Bluetooth");
    assert_eq!(PerformanceResource::Audio.as_str(), "Audio");
}

#[test]
fn performance_resource_display_with_index() {
    assert_eq!(format!("{}", PerformanceResource::Disk(2)), "Disk 2");
    assert_eq!(format!("{}", PerformanceResource::Gpu(0)), "GPU 0");
    assert_eq!(format!("{}", PerformanceResource::Cpu), "CPU");
}

#[test]
fn performance_resource_serde_roundtrip_simple() {
    let r = PerformanceResource::Network;
    let json = serde_json::to_string(&r).unwrap();
    let back: PerformanceResource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r);
}

#[test]
fn performance_resource_serde_roundtrip_with_index() {
    let r = PerformanceResource::Gpu(3);
    let json = serde_json::to_string(&r).unwrap();
    let back: PerformanceResource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r);
}

// ===========================================================================
// TimeRange (7 variants)
// ===========================================================================

#[test]
fn time_range_as_str_all() {
    assert_eq!(TimeRange::Last60Seconds.as_str(), "Last 60 Seconds");
    assert_eq!(TimeRange::Last5Minutes.as_str(), "Last 5 Minutes");
    assert_eq!(TimeRange::Last15Minutes.as_str(), "Last 15 Minutes");
    assert_eq!(TimeRange::Last30Minutes.as_str(), "Last 30 Minutes");
    assert_eq!(TimeRange::Last1Hour.as_str(), "Last 1 Hour");
    assert_eq!(TimeRange::Last6Hours.as_str(), "Last 6 Hours");
    assert_eq!(TimeRange::Last24Hours.as_str(), "Last 24 Hours");
}

#[test]
fn time_range_as_secs_all() {
    assert_eq!(TimeRange::Last60Seconds.as_secs(), 60);
    assert_eq!(TimeRange::Last5Minutes.as_secs(), 300);
    assert_eq!(TimeRange::Last15Minutes.as_secs(), 900);
    assert_eq!(TimeRange::Last30Minutes.as_secs(), 1_800);
    assert_eq!(TimeRange::Last1Hour.as_secs(), 3_600);
    assert_eq!(TimeRange::Last6Hours.as_secs(), 21_600);
    assert_eq!(TimeRange::Last24Hours.as_secs(), 86_400);
}

#[test]
fn time_range_serde_roundtrip() {
    let tr = TimeRange::Last24Hours;
    let json = serde_json::to_string(&tr).unwrap();
    assert_eq!(json, "\"last24_hours\"");
    let back: TimeRange = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tr);
}

// ===========================================================================
// GraphControls (default + serde)
// ===========================================================================

#[test]
fn graph_controls_default_values() {
    let gc = GraphControls::default();
    assert!(gc.show_legend);
    assert!(gc.show_grid);
    assert!(gc.auto_scale);
    assert!(!gc.stacked);
    assert!(gc.smooth);
    assert!(gc.overlay_lines.is_empty());
}

#[test]
fn graph_controls_serde_roundtrip() {
    let gc = GraphControls::default();
    let json = serde_json::to_string(&gc).unwrap();
    let back: GraphControls = serde_json::from_str(&json).unwrap();
    assert!(back.show_legend);
    assert!(!back.stacked);
    assert!(back.overlay_lines.is_empty());
}

// ===========================================================================
// CpuStats (default + serde)
// ===========================================================================

#[test]
fn cpu_stats_default_key_fields() {
    let stats = CpuStats::default();
    assert!((stats.utilization_percent - 0.0).abs() < f64::EPSILON);
    assert_eq!(stats.physical_cores, 0);
    assert_eq!(stats.logical_processors, 0);
    assert!(!stats.virtualization_enabled);
    assert!(!stats.throttling);
    assert!(stats.temperature_celsius.is_none());
    assert!(stats.c_state_residency.is_empty());
}

#[test]
fn cpu_stats_serde_roundtrip() {
    let mut stats = CpuStats::default();
    stats.utilization_percent = 55.5;
    stats.physical_cores = 8;
    stats.architecture = "x86_64".to_string();
    let json = serde_json::to_string(&stats).unwrap();
    let back: CpuStats = serde_json::from_str(&json).unwrap();
    assert!((back.utilization_percent - 55.5).abs() < f64::EPSILON);
    assert_eq!(back.physical_cores, 8);
    assert_eq!(back.architecture, "x86_64");
}

// ===========================================================================
// MemoryStats (default + serde)
// ===========================================================================

#[test]
fn memory_stats_default_key_fields() {
    let stats = MemoryStats::default();
    assert_eq!(stats.total_bytes, 0);
    assert_eq!(stats.in_use_bytes, 0);
    assert_eq!(stats.speed_mhz, 0);
    assert_eq!(stats.slots_used, 0);
    assert!(stats.ecc.is_none());
    assert!(stats.compression_ratio.is_none());
}

#[test]
fn memory_stats_serde_roundtrip() {
    let mut stats = MemoryStats::default();
    stats.total_bytes = 16_000_000_000;
    stats.memory_type = "DDR5".to_string();
    let json = serde_json::to_string(&stats).unwrap();
    let back: MemoryStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.total_bytes, 16_000_000_000);
    assert_eq!(back.memory_type, "DDR5");
}

// ===========================================================================
// DiskStats (default + serde)
// ===========================================================================

#[test]
fn disk_stats_default_key_fields() {
    let stats = DiskStats::default();
    assert!((stats.active_time_percent - 0.0).abs() < f64::EPSILON);
    assert_eq!(stats.capacity_bytes, 0);
    assert_eq!(stats.disk_type, "");
    assert!(stats.firmware.is_none());
    assert!(stats.trim_supported.is_none());
}

#[test]
fn disk_stats_serde_roundtrip() {
    let mut stats = DiskStats::default();
    stats.model = "Samsung 990 Pro".to_string();
    stats.disk_type = "NVMe".to_string();
    stats.capacity_bytes = 2_000_000_000_000;
    let json = serde_json::to_string(&stats).unwrap();
    let back: DiskStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.model, "Samsung 990 Pro");
    assert_eq!(back.capacity_bytes, 2_000_000_000_000);
}

// ===========================================================================
// GpuStats (default + serde)
// ===========================================================================

#[test]
fn gpu_stats_default_key_fields() {
    let stats = GpuStats::default();
    assert!((stats.overall_utilization - 0.0).abs() < f64::EPSILON);
    assert_eq!(stats.gpu_name, "");
    assert_eq!(stats.dedicated_vram_bytes, 0);
    assert!(stats.directx_version.is_none());
    assert!(stats.temperature_celsius.is_none());
    assert_eq!(stats.process_count, 0);
}

#[test]
fn gpu_stats_serde_roundtrip() {
    let mut stats = GpuStats::default();
    stats.gpu_name = "RTX 4090".to_string();
    stats.dedicated_vram_bytes = 24_000_000_000;
    let json = serde_json::to_string(&stats).unwrap();
    let back: GpuStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.gpu_name, "RTX 4090");
    assert_eq!(back.dedicated_vram_bytes, 24_000_000_000);
}

// ===========================================================================
// NetworkPerfStats (default + serde)
// ===========================================================================

#[test]
fn network_perf_stats_default_key_fields() {
    let stats = NetworkPerfStats::default();
    assert_eq!(stats.send_bytes_sec, 0);
    assert_eq!(stats.adapter_name, "");
    assert_eq!(stats.link_speed_mbps, 0);
    assert!(!stats.dhcp_enabled);
    assert!(stats.ipv4_address.is_none());
    assert!(stats.dns_servers.is_empty());
}

#[test]
fn network_perf_stats_serde_roundtrip() {
    let mut stats = NetworkPerfStats::default();
    stats.adapter_name = "eth0".to_string();
    stats.link_speed_mbps = 1000;
    stats.dhcp_enabled = true;
    let json = serde_json::to_string(&stats).unwrap();
    let back: NetworkPerfStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.adapter_name, "eth0");
    assert_eq!(back.link_speed_mbps, 1000);
    assert!(back.dhcp_enabled);
}

// ===========================================================================
// PowerStats (default + serde)
// ===========================================================================

#[test]
fn power_stats_default_key_fields() {
    let stats = PowerStats::default();
    assert!(!stats.ac_power);
    assert!(!stats.battery_present);
    assert!(stats.battery_percent.is_none());
    assert_eq!(stats.battery_state, "");
    assert_eq!(stats.current_power_plan, "");
}

#[test]
fn power_stats_serde_roundtrip() {
    let mut stats = PowerStats::default();
    stats.ac_power = true;
    stats.current_power_plan = "Balanced".to_string();
    let json = serde_json::to_string(&stats).unwrap();
    let back: PowerStats = serde_json::from_str(&json).unwrap();
    assert!(back.ac_power);
    assert_eq!(back.current_power_plan, "Balanced");
}

// ===========================================================================
// AudioPerfStats (default + serde)
// ===========================================================================

#[test]
fn audio_perf_stats_default_key_fields() {
    let stats = AudioPerfStats::default();
    assert_eq!(stats.output_device, "");
    assert_eq!(stats.output_sample_rate_hz, 0);
    assert_eq!(stats.output_bit_depth, 0);
    assert!(!stats.exclusive_mode_active);
    assert!(!stats.spatial_audio_enabled);
    assert!(stats.input_device.is_none());
}

#[test]
fn audio_perf_stats_serde_roundtrip() {
    let mut stats = AudioPerfStats::default();
    stats.output_device = "Speakers".to_string();
    stats.output_sample_rate_hz = 48000;
    stats.output_bit_depth = 24;
    let json = serde_json::to_string(&stats).unwrap();
    let back: AudioPerfStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.output_device, "Speakers");
    assert_eq!(back.output_sample_rate_hz, 48000);
    assert_eq!(back.output_bit_depth, 24);
}
