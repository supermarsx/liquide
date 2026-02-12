//! Performance tab types for the task manager.
//!
//! Defines resource selectors, graph types, graph controls, and detailed
//! statistics structs for every hardware resource category corresponding
//! to spec section 5 (Tab: Performance).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// PerformanceResource
// ---------------------------------------------------------------------------

/// Identifies a resource category in the performance side-bar.
///
/// `Disk` and `Gpu` carry a device index so this enum cannot derive `Copy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceResource {
    /// Central processing unit.
    Cpu,
    /// System memory (RAM).
    Memory,
    /// Physical disk identified by device index.
    Disk(u8),
    /// GPU adapter identified by device index.
    Gpu(u8),
    /// Network adapter aggregate.
    Network,
    /// Power / battery subsystem.
    Power,
    /// Bluetooth radio and devices.
    Bluetooth,
    /// Audio subsystem.
    Audio,
}

impl PerformanceResource {
    /// Return a human-readable base name for this resource.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Disk(_) => "Disk",
            Self::Gpu(_) => "GPU",
            Self::Network => "Network",
            Self::Power => "Power",
            Self::Bluetooth => "Bluetooth",
            Self::Audio => "Audio",
        }
    }
}

impl fmt::Display for PerformanceResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disk(idx) => write!(f, "Disk {idx}"),
            Self::Gpu(idx) => write!(f, "GPU {idx}"),
            _ => f.write_str(self.as_str()),
        }
    }
}

// ---------------------------------------------------------------------------
// CpuGraphType
// ---------------------------------------------------------------------------

/// Graph type selectable for the CPU performance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuGraphType {
    /// Single aggregated utilization line (0-100%).
    OverallUtilization,
    /// Grid of mini-graphs, one per logical core.
    PerCoreUtilization,
    /// Dual-axis utilization + clock speed per core.
    PerCoreFrequency,
    /// Cores grouped by NUMA node.
    NumaNodeView,
    /// Stacked area showing user-mode vs kernel-mode time.
    KernelVsUser,
    /// Grid with colour-coded intensity per core.
    CoreHeatmap,
}

impl CpuGraphType {
    /// Return a human-readable label for this graph type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OverallUtilization => "Overall Utilization",
            Self::PerCoreUtilization => "Per-Core Utilization",
            Self::PerCoreFrequency => "Per-Core Frequency",
            Self::NumaNodeView => "NUMA Node View",
            Self::KernelVsUser => "Kernel vs User",
            Self::CoreHeatmap => "Core Heatmap",
        }
    }
}

impl fmt::Display for CpuGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MemoryGraphType
// ---------------------------------------------------------------------------

/// Graph type selectable for the memory performance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryGraphType {
    /// Stacked area: In Use / Modified / Standby / Free.
    Composition,
    /// Line graph of committed vs limit.
    CommitCharge,
    /// Line graph of hard + soft page faults per second.
    PageFaults,
}

impl MemoryGraphType {
    /// Return a human-readable label for this graph type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Composition => "Composition",
            Self::CommitCharge => "Commit Charge",
            Self::PageFaults => "Page Faults",
        }
    }
}

impl fmt::Display for MemoryGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DiskGraphType
// ---------------------------------------------------------------------------

/// Graph type selectable for the per-disk performance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskGraphType {
    /// Disk busy percentage.
    ActiveTime,
    /// Read + Write throughput in bytes/sec.
    TransferRate,
    /// Read + Write I/O operations per second.
    Iops,
    /// Average disk queue length.
    QueueDepth,
    /// Average read/write latency in milliseconds.
    Latency,
}

impl DiskGraphType {
    /// Return a human-readable label for this graph type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ActiveTime => "Active Time",
            Self::TransferRate => "Transfer Rate",
            Self::Iops => "IOPS",
            Self::QueueDepth => "Queue Depth",
            Self::Latency => "Latency",
        }
    }
}

impl fmt::Display for DiskGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// GpuGraphType
// ---------------------------------------------------------------------------

/// Graph type selectable for the per-GPU performance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuGraphType {
    /// Combined engine utilization.
    Overall,
    /// 3D rendering pipeline usage.
    Engine3d,
    /// DMA / copy engine usage.
    CopyEngine,
    /// Hardware video decode usage.
    VideoDecode,
    /// Hardware video encode usage.
    VideoEncode,
    /// GPU compute / GPGPU usage.
    Compute,
    /// Dedicated + shared VRAM stacked area.
    VramUsage,
    /// GPU temperature line graph.
    Temperature,
    /// Fan speed (RPM or %) line graph.
    FanSpeed,
}

impl GpuGraphType {
    /// Return a human-readable label for this graph type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Overall => "Overall",
            Self::Engine3d => "3D Engine",
            Self::CopyEngine => "Copy Engine",
            Self::VideoDecode => "Video Decode",
            Self::VideoEncode => "Video Encode",
            Self::Compute => "Compute",
            Self::VramUsage => "VRAM Usage",
            Self::Temperature => "Temperature",
            Self::FanSpeed => "Fan Speed",
        }
    }
}

impl fmt::Display for GpuGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// NetworkGraphType
// ---------------------------------------------------------------------------

/// Graph type selectable for the network performance view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkGraphType {
    /// Send + receive throughput.
    Throughput,
    /// Active TCP connection count.
    ConnectionCount,
    /// Packets per second (send + receive).
    PacketRate,
}

impl NetworkGraphType {
    /// Return a human-readable label for this graph type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Throughput => "Throughput",
            Self::ConnectionCount => "Connection Count",
            Self::PacketRate => "Packet Rate",
        }
    }
}

impl fmt::Display for NetworkGraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// TimeRange
// ---------------------------------------------------------------------------

/// Selectable time range for performance graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    /// Last 60 seconds of data.
    Last60Seconds,
    /// Last 5 minutes of data.
    Last5Minutes,
    /// Last 15 minutes of data.
    Last15Minutes,
    /// Last 30 minutes of data.
    Last30Minutes,
    /// Last 1 hour of data.
    Last1Hour,
    /// Last 6 hours of data.
    Last6Hours,
    /// Last 24 hours of data.
    Last24Hours,
}

impl TimeRange {
    /// Return a human-readable label for this time range.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Last60Seconds => "Last 60 Seconds",
            Self::Last5Minutes => "Last 5 Minutes",
            Self::Last15Minutes => "Last 15 Minutes",
            Self::Last30Minutes => "Last 30 Minutes",
            Self::Last1Hour => "Last 1 Hour",
            Self::Last6Hours => "Last 6 Hours",
            Self::Last24Hours => "Last 24 Hours",
        }
    }

    /// Return the duration of this range in seconds.
    #[must_use]
    pub fn as_secs(&self) -> u64 {
        match self {
            Self::Last60Seconds => 60,
            Self::Last5Minutes => 300,
            Self::Last15Minutes => 900,
            Self::Last30Minutes => 1_800,
            Self::Last1Hour => 3_600,
            Self::Last6Hours => 21_600,
            Self::Last24Hours => 86_400,
        }
    }
}

impl fmt::Display for TimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// GraphControls
// ---------------------------------------------------------------------------

/// User-facing controls that apply to every performance graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphControls {
    /// Show the legend identifying each data series.
    pub show_legend: bool,
    /// Show background grid lines.
    pub show_grid: bool,
    /// Automatically scale the Y-axis to fit visible data.
    pub auto_scale: bool,
    /// Stack multiple series rather than overlaying them.
    pub stacked: bool,
    /// Use Bezier smoothing on graph lines.
    pub smooth: bool,
    /// Additional metric series to overlay on the graph.
    pub overlay_lines: Vec<String>,
}

impl Default for GraphControls {
    fn default() -> Self {
        Self {
            show_legend: true,
            show_grid: true,
            auto_scale: true,
            stacked: false,
            smooth: true,
            overlay_lines: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// CpuStats
// ---------------------------------------------------------------------------

/// Detailed CPU statistics displayed in the CPU performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats {
    /// Overall CPU utilization (0.0 - 100.0).
    pub utilization_percent: f64,
    /// Current base clock speed in GHz.
    pub speed_ghz: f64,
    /// Current effective (boost) clock speed in GHz.
    pub effective_speed_ghz: f64,
    /// Rated base frequency in GHz.
    pub base_speed_ghz: f64,
    /// Maximum turbo/boost frequency in GHz.
    pub max_boost_ghz: f64,
    /// Physical CPU socket count.
    pub sockets: u32,
    /// Physical core count.
    pub physical_cores: u32,
    /// Logical processor count (including HT/SMT).
    pub logical_processors: u32,
    /// Whether hardware virtualization is enabled.
    pub virtualization_enabled: bool,
    /// L1 cache size in bytes.
    pub l1_cache_bytes: u64,
    /// L2 cache size in bytes.
    pub l2_cache_bytes: u64,
    /// L3 cache size in bytes.
    pub l3_cache_bytes: u64,
    /// CPU architecture (e.g. "x86_64", "ARM64", "RISC-V").
    pub architecture: String,
    /// System uptime in seconds.
    pub uptime_secs: u64,
    /// Total running process count.
    pub process_count: u32,
    /// Total thread count across all processes.
    pub thread_count: u32,
    /// Total open handle / file descriptor count.
    pub handle_count: u32,
    /// Current hardware interrupt rate per second.
    pub interrupts_per_sec: u64,
    /// Deferred procedure call rate per second.
    pub dpcs_per_sec: u64,
    /// System call rate per second.
    pub syscalls_per_sec: u64,
    /// Context switch rate per second.
    pub ctx_switches_per_sec: u64,
    /// Package temperature in degrees Celsius, if available.
    pub temperature_celsius: Option<f64>,
    /// CPU package power draw in watts, if available.
    pub power_draw_watts: Option<f64>,
    /// Core voltage in volts, if available.
    pub voltage: Option<f64>,
    /// Whether the CPU is currently being throttled.
    pub throttling: bool,
    /// Reason for throttling (thermal, power, current), if applicable.
    pub throttle_reason: Option<String>,
    /// Percentage of time spent in each C-state (C0, C1, C3, ...).
    pub c_state_residency: Vec<f64>,
    /// Instructions per cycle, if hardware counters are available.
    pub ipc: Option<f64>,
    /// Branch prediction miss rate, if hardware counters are available.
    pub branch_miss_rate: Option<f64>,
    /// L1 cache miss rate, if hardware counters are available.
    pub cache_miss_rate_l1: Option<f64>,
    /// L2 cache miss rate, if hardware counters are available.
    pub cache_miss_rate_l2: Option<f64>,
    /// L3 cache miss rate, if hardware counters are available.
    pub cache_miss_rate_l3: Option<f64>,
}

impl Default for CpuStats {
    fn default() -> Self {
        Self {
            utilization_percent: 0.0,
            speed_ghz: 0.0,
            effective_speed_ghz: 0.0,
            base_speed_ghz: 0.0,
            max_boost_ghz: 0.0,
            sockets: 0,
            physical_cores: 0,
            logical_processors: 0,
            virtualization_enabled: false,
            l1_cache_bytes: 0,
            l2_cache_bytes: 0,
            l3_cache_bytes: 0,
            architecture: String::new(),
            uptime_secs: 0,
            process_count: 0,
            thread_count: 0,
            handle_count: 0,
            interrupts_per_sec: 0,
            dpcs_per_sec: 0,
            syscalls_per_sec: 0,
            ctx_switches_per_sec: 0,
            temperature_celsius: None,
            power_draw_watts: None,
            voltage: None,
            throttling: false,
            throttle_reason: None,
            c_state_residency: Vec::new(),
            ipc: None,
            branch_miss_rate: None,
            cache_miss_rate_l1: None,
            cache_miss_rate_l2: None,
            cache_miss_rate_l3: None,
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryStats
// ---------------------------------------------------------------------------

/// Detailed memory statistics displayed in the memory performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Physical RAM currently in use (bytes).
    pub in_use_bytes: u64,
    /// Available physical RAM (bytes).
    pub available_bytes: u64,
    /// Virtual memory committed (bytes).
    pub committed_bytes: u64,
    /// Maximum commit limit (bytes).
    pub commit_limit_bytes: u64,
    /// Standby + modified cached pages (bytes).
    pub cached_bytes: u64,
    /// Kernel paged pool size (bytes).
    pub paged_pool_bytes: u64,
    /// Kernel non-paged pool size (bytes).
    pub nonpaged_pool_bytes: u64,
    /// Total installed RAM (bytes).
    pub total_bytes: u64,
    /// Memory clock speed in MHz.
    pub speed_mhz: u32,
    /// Effective data rate in MT/s.
    pub effective_speed_mt: u32,
    /// Number of memory slots currently populated.
    pub slots_used: u32,
    /// Total number of memory slots.
    pub slots_total: u32,
    /// Module form factor (e.g. "DIMM", "SO-DIMM").
    pub form_factor: String,
    /// Memory technology (e.g. "DDR4", "DDR5", "LPDDR5").
    pub memory_type: String,
    /// Channel configuration (e.g. "Single", "Dual", "Quad").
    pub channel_config: String,
    /// Memory reserved by firmware (bytes).
    pub hardware_reserved_bytes: u64,
    /// Number of NUMA nodes.
    pub numa_nodes: u32,
    /// ECC status if known.
    pub ecc: Option<bool>,
    /// Current page/swap file usage (bytes).
    pub page_file_usage_bytes: u64,
    /// Maximum page/swap file size (bytes).
    pub page_file_max_bytes: u64,
    /// Memory compression ratio, if applicable.
    pub compression_ratio: Option<f64>,
    /// Amount of compressed memory in bytes, if applicable.
    pub compressed_bytes: Option<u64>,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            in_use_bytes: 0,
            available_bytes: 0,
            committed_bytes: 0,
            commit_limit_bytes: 0,
            cached_bytes: 0,
            paged_pool_bytes: 0,
            nonpaged_pool_bytes: 0,
            total_bytes: 0,
            speed_mhz: 0,
            effective_speed_mt: 0,
            slots_used: 0,
            slots_total: 0,
            form_factor: String::new(),
            memory_type: String::new(),
            channel_config: String::new(),
            hardware_reserved_bytes: 0,
            numa_nodes: 0,
            ecc: None,
            page_file_usage_bytes: 0,
            page_file_max_bytes: 0,
            compression_ratio: None,
            compressed_bytes: None,
        }
    }
}

// ---------------------------------------------------------------------------
// DiskStats
// ---------------------------------------------------------------------------

/// Detailed per-disk statistics displayed in the disk performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats {
    /// Percentage of time the disk is actively servicing requests.
    pub active_time_percent: f64,
    /// Mean I/O latency in milliseconds.
    pub avg_response_time_ms: f64,
    /// Current read throughput (bytes per second).
    pub read_speed_bytes_sec: u64,
    /// Current write throughput (bytes per second).
    pub write_speed_bytes_sec: u64,
    /// Current read I/O operations per second.
    pub read_iops: u64,
    /// Current write I/O operations per second.
    pub write_iops: u64,
    /// Current I/O queue depth.
    pub queue_depth: u32,
    /// Total disk capacity in bytes.
    pub capacity_bytes: u64,
    /// Available free space in bytes.
    pub free_space_bytes: u64,
    /// Total formatted capacity in bytes.
    pub formatted_bytes: u64,
    /// Drive type (e.g. "SSD", "HDD", "NVMe", "USB", "Network").
    pub disk_type: String,
    /// Interface type (e.g. "NVMe", "SATA", "USB 3.2", "Thunderbolt").
    pub interface: String,
    /// Drive model string.
    pub model: String,
    /// Firmware version, if available.
    pub firmware: Option<String>,
    /// Drive serial number, if available.
    pub serial: Option<String>,
    /// Number of partitions on this disk.
    pub partition_count: u32,
    /// Primary file system (e.g. "NTFS", "ext4", "btrfs", "APFS").
    pub file_system: String,
    /// S.M.A.R.T. health status (e.g. "Healthy", "Warning", "Critical").
    pub smart_status: String,
    /// Drive temperature in degrees Celsius, if available.
    pub temperature_celsius: Option<f64>,
    /// Total lifetime power-on hours, if available.
    pub power_on_hours: Option<u64>,
    /// Lifetime total bytes read.
    pub total_bytes_read: u64,
    /// Lifetime total bytes written.
    pub total_bytes_written: u64,
    /// SSD wear leveling percentage remaining, if applicable.
    pub wear_leveling_percent: Option<f64>,
    /// Whether TRIM is supported, if applicable.
    pub trim_supported: Option<bool>,
    /// Whether the write cache is enabled, if applicable.
    pub write_cache_enabled: Option<bool>,
    /// Maximum native command queue depth, if applicable.
    pub ncq_depth: Option<u32>,
}

impl Default for DiskStats {
    fn default() -> Self {
        Self {
            active_time_percent: 0.0,
            avg_response_time_ms: 0.0,
            read_speed_bytes_sec: 0,
            write_speed_bytes_sec: 0,
            read_iops: 0,
            write_iops: 0,
            queue_depth: 0,
            capacity_bytes: 0,
            free_space_bytes: 0,
            formatted_bytes: 0,
            disk_type: String::new(),
            interface: String::new(),
            model: String::new(),
            firmware: None,
            serial: None,
            partition_count: 0,
            file_system: String::new(),
            smart_status: String::new(),
            temperature_celsius: None,
            power_on_hours: None,
            total_bytes_read: 0,
            total_bytes_written: 0,
            wear_leveling_percent: None,
            trim_supported: None,
            write_cache_enabled: None,
            ncq_depth: None,
        }
    }
}

// ---------------------------------------------------------------------------
// GpuStats
// ---------------------------------------------------------------------------

/// Detailed per-GPU statistics displayed in the GPU performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuStats {
    /// Combined engine utilization (0.0 - 100.0).
    pub overall_utilization: f64,
    /// 3D rendering pipeline usage percentage.
    pub engine_3d_percent: f64,
    /// Copy / DMA engine usage percentage.
    pub copy_engine_percent: f64,
    /// Hardware video decode usage percentage.
    pub video_decode_percent: f64,
    /// Hardware video encode usage percentage.
    pub video_encode_percent: f64,
    /// GPU compute / GPGPU usage percentage.
    pub compute_percent: f64,
    /// Total dedicated VRAM in bytes.
    pub dedicated_vram_bytes: u64,
    /// Dedicated VRAM currently in use (bytes).
    pub dedicated_vram_used_bytes: u64,
    /// Total shared GPU memory (system RAM) in bytes.
    pub shared_vram_bytes: u64,
    /// Shared GPU memory currently in use (bytes).
    pub shared_vram_used_bytes: u64,
    /// GPU model name.
    pub gpu_name: String,
    /// GPU driver version string.
    pub driver_version: String,
    /// GPU driver release date, if available.
    pub driver_date: Option<String>,
    /// Supported DirectX feature level, if available.
    pub directx_version: Option<String>,
    /// Supported Vulkan version, if available.
    pub vulkan_version: Option<String>,
    /// Supported OpenGL version, if available.
    pub opengl_version: Option<String>,
    /// Compute API (e.g. "CUDA", "OpenCL", "ROCm"), if applicable.
    pub compute_api: Option<String>,
    /// PCIe generation (e.g. 3, 4, 5), if available.
    pub pcie_generation: Option<u8>,
    /// PCIe lane width (e.g. 16), if available.
    pub pcie_lanes: Option<u8>,
    /// PCIe bandwidth in Gbps, if available.
    pub pcie_bandwidth_gbps: Option<f64>,
    /// GPU core temperature in degrees Celsius, if available.
    pub temperature_celsius: Option<f64>,
    /// GPU hotspot junction temperature in degrees Celsius, if available.
    pub hot_spot_celsius: Option<f64>,
    /// VRAM temperature in degrees Celsius, if available.
    pub memory_temp_celsius: Option<f64>,
    /// Fan speed as a percentage of maximum, if available.
    pub fan_speed_percent: Option<f64>,
    /// Fan speed in RPM, if available.
    pub fan_speed_rpm: Option<u32>,
    /// Current GPU board power draw in watts, if available.
    pub power_draw_watts: Option<f64>,
    /// GPU power limit in watts, if available.
    pub power_limit_watts: Option<f64>,
    /// Current GPU core clock in MHz, if available.
    pub core_clock_mhz: Option<u32>,
    /// Current GPU memory clock in MHz, if available.
    pub memory_clock_mhz: Option<u32>,
    /// Maximum boost clock in MHz, if available.
    pub boost_clock_mhz: Option<u32>,
    /// Memory bus width in bits, if available.
    pub memory_bus_width: Option<u32>,
    /// VRAM technology (e.g. "GDDR6", "GDDR6X", "HBM2e"), if available.
    pub memory_type: Option<String>,
    /// Shader / stream processor count, if available.
    pub shader_units: Option<u32>,
    /// Texture mapping unit count, if available.
    pub tmu_count: Option<u32>,
    /// Render output unit count, if available.
    pub rop_count: Option<u32>,
    /// Number of processes currently using this GPU.
    pub process_count: u32,
}

impl Default for GpuStats {
    fn default() -> Self {
        Self {
            overall_utilization: 0.0,
            engine_3d_percent: 0.0,
            copy_engine_percent: 0.0,
            video_decode_percent: 0.0,
            video_encode_percent: 0.0,
            compute_percent: 0.0,
            dedicated_vram_bytes: 0,
            dedicated_vram_used_bytes: 0,
            shared_vram_bytes: 0,
            shared_vram_used_bytes: 0,
            gpu_name: String::new(),
            driver_version: String::new(),
            driver_date: None,
            directx_version: None,
            vulkan_version: None,
            opengl_version: None,
            compute_api: None,
            pcie_generation: None,
            pcie_lanes: None,
            pcie_bandwidth_gbps: None,
            temperature_celsius: None,
            hot_spot_celsius: None,
            memory_temp_celsius: None,
            fan_speed_percent: None,
            fan_speed_rpm: None,
            power_draw_watts: None,
            power_limit_watts: None,
            core_clock_mhz: None,
            memory_clock_mhz: None,
            boost_clock_mhz: None,
            memory_bus_width: None,
            memory_type: None,
            shader_units: None,
            tmu_count: None,
            rop_count: None,
            process_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// NetworkPerfStats
// ---------------------------------------------------------------------------

/// Detailed per-adapter network statistics displayed in the network
/// performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerfStats {
    /// Current outbound throughput (bytes per second).
    pub send_bytes_sec: u64,
    /// Current inbound throughput (bytes per second).
    pub recv_bytes_sec: u64,
    /// Total bytes sent since system boot.
    pub send_total_bytes: u64,
    /// Total bytes received since system boot.
    pub recv_total_bytes: u64,
    /// Current outbound packet rate per second.
    pub send_packets_sec: u64,
    /// Current inbound packet rate per second.
    pub recv_packets_sec: u64,
    /// Total packets sent since system boot.
    pub send_total_packets: u64,
    /// Total packets received since system boot.
    pub recv_total_packets: u64,
    /// Number of active TCP/UDP connections.
    pub active_connections: u32,
    /// Network adapter friendly name.
    pub adapter_name: String,
    /// Adapter technology (e.g. "Ethernet", "Wi-Fi", "Cellular").
    pub adapter_type: String,
    /// Negotiated link speed in Mbps.
    pub link_speed_mbps: u64,
    /// Primary IPv4 address, if assigned.
    pub ipv4_address: Option<String>,
    /// Primary IPv6 address, if assigned.
    pub ipv6_address: Option<String>,
    /// Hardware MAC address, if available.
    pub mac_address: Option<String>,
    /// Configured DNS server addresses.
    pub dns_servers: Vec<String>,
    /// Default gateway address, if configured.
    pub gateway: Option<String>,
    /// Subnet mask, if assigned.
    pub subnet_mask: Option<String>,
    /// Whether DHCP is enabled on this adapter.
    pub dhcp_enabled: bool,
    /// Connection type (e.g. "Ethernet", "Wi-Fi", "VPN", "Loopback").
    pub connection_type: String,
    /// Wi-Fi signal strength in dBm, if applicable.
    pub signal_strength_dbm: Option<i32>,
    /// Wi-Fi network SSID, if applicable.
    pub ssid: Option<String>,
    /// Wi-Fi frequency in MHz, if applicable.
    pub frequency_mhz: Option<u32>,
    /// Wi-Fi channel number, if applicable.
    pub channel: Option<u32>,
    /// Inbound error count.
    pub errors_in: u64,
    /// Outbound error count.
    pub errors_out: u64,
    /// Inbound discard count.
    pub discards_in: u64,
    /// Outbound discard count.
    pub discards_out: u64,
}

impl Default for NetworkPerfStats {
    fn default() -> Self {
        Self {
            send_bytes_sec: 0,
            recv_bytes_sec: 0,
            send_total_bytes: 0,
            recv_total_bytes: 0,
            send_packets_sec: 0,
            recv_packets_sec: 0,
            send_total_packets: 0,
            recv_total_packets: 0,
            active_connections: 0,
            adapter_name: String::new(),
            adapter_type: String::new(),
            link_speed_mbps: 0,
            ipv4_address: None,
            ipv6_address: None,
            mac_address: None,
            dns_servers: Vec::new(),
            gateway: None,
            subnet_mask: None,
            dhcp_enabled: false,
            connection_type: String::new(),
            signal_strength_dbm: None,
            ssid: None,
            frequency_mhz: None,
            channel: None,
            errors_in: 0,
            errors_out: 0,
            discards_in: 0,
            discards_out: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// PowerStats
// ---------------------------------------------------------------------------

/// Power and battery statistics displayed in the power performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStats {
    /// Whether the system is running on AC power.
    pub ac_power: bool,
    /// Whether a battery is physically present.
    pub battery_present: bool,
    /// Current battery charge percentage, if a battery is present.
    pub battery_percent: Option<f64>,
    /// Estimated battery time remaining in seconds, if discharging.
    pub battery_remaining_secs: Option<u64>,
    /// Battery state description (e.g. "Charging", "Discharging", "Full").
    pub battery_state: String,
    /// Total system power draw in watts, if measurable.
    pub system_power_watts: Option<f64>,
    /// CPU package power draw in watts, if measurable.
    pub cpu_power_watts: Option<f64>,
    /// GPU board power draw in watts, if measurable.
    pub gpu_power_watts: Option<f64>,
    /// Display backlight power draw in watts, if measurable.
    pub display_power_watts: Option<f64>,
    /// Name of the currently active power plan / profile.
    pub current_power_plan: String,
    /// Energy drain rate in milliwatt-hours, if measurable.
    pub energy_rate_mwh: Option<f64>,
    /// Charging rate in watts, if currently charging.
    pub charge_rate_watts: Option<f64>,
}

impl Default for PowerStats {
    fn default() -> Self {
        Self {
            ac_power: false,
            battery_present: false,
            battery_percent: None,
            battery_remaining_secs: None,
            battery_state: String::new(),
            system_power_watts: None,
            cpu_power_watts: None,
            gpu_power_watts: None,
            display_power_watts: None,
            current_power_plan: String::new(),
            energy_rate_mwh: None,
            charge_rate_watts: None,
        }
    }
}

// ---------------------------------------------------------------------------
// AudioPerfStats
// ---------------------------------------------------------------------------

/// Audio subsystem statistics displayed in the audio performance panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPerfStats {
    /// Name of the active audio output device.
    pub output_device: String,
    /// Output format description (e.g. "PCM", "DSD").
    pub output_format: String,
    /// Output sample rate in Hz.
    pub output_sample_rate_hz: u32,
    /// Output bit depth (e.g. 16, 24, 32).
    pub output_bit_depth: u16,
    /// Number of output channels.
    pub output_channels: u16,
    /// Output audio pipeline latency in milliseconds.
    pub output_latency_ms: f64,
    /// Name of the active audio input device, if any.
    pub input_device: Option<String>,
    /// Input sample rate in Hz, if an input device is active.
    pub input_sample_rate_hz: Option<u32>,
    /// Number of active audio streams system-wide.
    pub stream_count: u32,
    /// Whether an application has acquired exclusive mode on an audio device.
    pub exclusive_mode_active: bool,
    /// Whether spatial audio processing is enabled.
    pub spatial_audio_enabled: bool,
}

impl Default for AudioPerfStats {
    fn default() -> Self {
        Self {
            output_device: String::new(),
            output_format: String::new(),
            output_sample_rate_hz: 0,
            output_bit_depth: 0,
            output_channels: 0,
            output_latency_ms: 0.0,
            input_device: None,
            input_sample_rate_hz: None,
            stream_count: 0,
            exclusive_mode_active: false,
            spatial_audio_enabled: false,
        }
    }
}
