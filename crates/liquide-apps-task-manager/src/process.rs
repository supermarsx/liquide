use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// ProcessStatus
// ---------------------------------------------------------------------------

/// Current state of a process (see spec section 4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Running,
    Sleeping,
    Stopped,
    Zombie,
    Idle,
    NotResponding,
    Suspended,
    Waiting,
    DiskSleep,
}

impl ProcessStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Sleeping => "Sleeping",
            Self::Stopped => "Stopped",
            Self::Zombie => "Zombie",
            Self::Idle => "Idle",
            Self::NotResponding => "Not Responding",
            Self::Suspended => "Suspended",
            Self::Waiting => "Waiting",
            Self::DiskSleep => "Disk Sleep",
        }
    }
}

impl fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ProcessType
// ---------------------------------------------------------------------------

/// Classification of a process (see spec section 4.2 – Identity Columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessType {
    App,
    Background,
    Service,
    System,
    Shell,
}

impl ProcessType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::App => "App",
            Self::Background => "Background",
            Self::Service => "Service",
            Self::System => "System",
            Self::Shell => "Shell",
        }
    }
}

impl fmt::Display for ProcessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// GroupingMode
// ---------------------------------------------------------------------------

/// How the process list can be grouped (see spec section 4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupingMode {
    Type,
    Status,
    User,
    Session,
    Priority,
    GpuAdapter,
    Package,
    None,
}

impl GroupingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Status => "Status",
            Self::User => "User",
            Self::Session => "Session",
            Self::Priority => "Priority",
            Self::GpuAdapter => "GPU Adapter",
            Self::Package => "Package",
            Self::None => "None",
        }
    }
}

impl fmt::Display for GroupingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SchedulingPriority
// ---------------------------------------------------------------------------

/// Scheduling priority classes (see spec section 4.5 – Set Priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SchedulingPriority {
    Realtime,
    High,
    AboveNormal,
    #[default]
    Normal,
    BelowNormal,
    Idle,
}

impl SchedulingPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Realtime => "Realtime",
            Self::High => "High",
            Self::AboveNormal => "Above Normal",
            Self::Normal => "Normal",
            Self::BelowNormal => "Below Normal",
            Self::Idle => "Idle",
        }
    }
}

impl fmt::Display for SchedulingPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// IoPriority
// ---------------------------------------------------------------------------

/// I/O scheduling priority levels (see spec section 4.5 – Set I/O Priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum IoPriority {
    Critical,
    High,
    #[default]
    Normal,
    Low,
    VeryLow,
}

impl IoPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Normal => "Normal",
            Self::Low => "Low",
            Self::VeryLow => "Very Low",
        }
    }
}

impl fmt::Display for IoPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ProcessAction
// ---------------------------------------------------------------------------

/// Actions available from the process context menu (see spec section 4.5).
///
/// Because some variants carry data (`SetPriority`, `SetAffinity`,
/// `SetIoPriority`), this enum intentionally does **not** derive `Copy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessAction {
    EndTask,
    EndProcessTree,
    Restart,
    Suspend,
    Resume,
    SetPriority(SchedulingPriority),
    SetAffinity(u64),
    SetIoPriority(IoPriority),
    CreateMiniDump,
    CreateFullDump,
    AnalyzeWaitChain,
    OpenFileLocation,
    OpenProperties,
    CopyName,
    CopyPid,
    CopyCmdLine,
    CopyPath,
    CopyAllColumns,
    SearchOnline,
    ViewInProcessTree,
    ShowThreads,
    ShowHandles,
    ShowModules,
    ShowConnections,
    ShowGpuDetails,
    AttachDebugger,
    GenerateStackTrace,
    RunAsAdmin,
}

impl ProcessAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EndTask => "End Task",
            Self::EndProcessTree => "End Process Tree",
            Self::Restart => "Restart",
            Self::Suspend => "Suspend",
            Self::Resume => "Resume",
            Self::SetPriority(_) => "Set Priority",
            Self::SetAffinity(_) => "Set Affinity",
            Self::SetIoPriority(_) => "Set I/O Priority",
            Self::CreateMiniDump => "Create Mini Dump",
            Self::CreateFullDump => "Create Full Dump",
            Self::AnalyzeWaitChain => "Analyze Wait Chain",
            Self::OpenFileLocation => "Open File Location",
            Self::OpenProperties => "Open Properties",
            Self::CopyName => "Copy Name",
            Self::CopyPid => "Copy PID",
            Self::CopyCmdLine => "Copy Command Line",
            Self::CopyPath => "Copy Path",
            Self::CopyAllColumns => "Copy All Columns",
            Self::SearchOnline => "Search Online",
            Self::ViewInProcessTree => "View in Process Tree",
            Self::ShowThreads => "Show Threads",
            Self::ShowHandles => "Show Handles",
            Self::ShowModules => "Show Modules",
            Self::ShowConnections => "Show Connections",
            Self::ShowGpuDetails => "Show GPU Details",
            Self::AttachDebugger => "Attach Debugger",
            Self::GenerateStackTrace => "Generate Stack Trace",
            Self::RunAsAdmin => "Run as Administrator",
        }
    }
}

impl fmt::Display for ProcessAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ThreadInfo
// ---------------------------------------------------------------------------

/// Per-thread detail shown in the inline Threads sub-table (spec section 4.6.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    /// Thread ID.
    pub tid: u32,
    /// Running / Waiting / Suspended.
    pub state: String,
    /// Per-thread CPU usage as a percentage.
    pub cpu_percent: f64,
    /// Accumulated CPU time in milliseconds.
    pub cpu_time_ms: u64,
    /// Thread priority value.
    pub priority: i32,
    /// Entry point function name (if symbols are available).
    pub start_address: Option<String>,
    /// Executive / FreePage / PageIn / PoolAllocation / etc.
    pub wait_reason: Option<String>,
    /// Preferred CPU core.
    pub ideal_processor: Option<u32>,
    /// Thread stack size in bytes.
    pub stack_size_bytes: Option<u64>,
}

// ---------------------------------------------------------------------------
// HandleInfo
// ---------------------------------------------------------------------------

/// Per-handle detail shown in the inline Handles sub-table (spec section 4.6.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleInfo {
    /// Handle value.
    pub handle: u64,
    /// File / Key / Event / Mutex / Section / Semaphore / Thread / Process / Token / etc.
    pub handle_type: String,
    /// Object name or path.
    pub name: Option<String>,
    /// Access flags.
    pub access: u32,
}

// ---------------------------------------------------------------------------
// ModuleInfo
// ---------------------------------------------------------------------------

/// Per-module detail shown in the inline Modules sub-table (spec section 4.6.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// Module / DLL / SO filename.
    pub name: String,
    /// Full path on disk.
    pub path: String,
    /// Load address.
    pub base_address: u64,
    /// Memory footprint in bytes.
    pub size_bytes: u64,
    /// File version.
    pub version: Option<String>,
    /// Digital signature publisher.
    pub publisher: Option<String>,
    /// Module description.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// ConnectionSummary
// ---------------------------------------------------------------------------

/// Per-connection detail shown in the inline Connections sub-table (spec section 4.6.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSummary {
    /// TCP / UDP / TCP6 / UDP6.
    pub protocol: String,
    /// Local IP:Port.
    pub local_address: String,
    /// Remote IP:Port (or *:*).
    pub remote_address: String,
    /// ESTABLISHED / LISTEN / TIME_WAIT / CLOSE_WAIT / etc.
    pub state: String,
    /// Total bytes sent on this connection.
    pub bytes_sent: u64,
    /// Total bytes received on this connection.
    pub bytes_received: u64,
}

// ---------------------------------------------------------------------------
// ProcessInfo
// ---------------------------------------------------------------------------

/// Comprehensive per-process information encompassing every column defined in
/// spec section 4.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    // -- Identity ----------------------------------------------------------
    /// Executable display name (friendly name from manifest or binary name).
    pub name: String,
    /// Process ID.
    pub pid: u32,
    /// Parent Process ID.
    pub ppid: Option<u32>,
    /// Current process state.
    pub status: ProcessStatus,
    /// Full command line with all arguments.
    pub cmdline: String,
    /// Absolute path to the binary on disk.
    pub exe_path: Option<String>,
    /// Current working directory of the process.
    pub cwd: Option<String>,
    /// User account running the process.
    pub user: String,
    /// Login session identifier.
    pub session_id: Option<u32>,
    /// App / Background / Service / System / Shell.
    pub proc_type: ProcessType,

    // -- CPU ---------------------------------------------------------------
    /// Current CPU usage as a percentage of all cores.
    pub cpu_percent: f64,
    /// Total accumulated CPU time in milliseconds (user + kernel).
    pub cpu_time_ms: u64,
    /// User-mode CPU time in milliseconds.
    pub cpu_user_ms: u64,
    /// Kernel-mode CPU time in milliseconds.
    pub cpu_kernel_ms: u64,
    /// Number of active threads.
    pub threads: u32,
    /// Open file descriptors / handles count.
    pub handles: u32,
    /// Scheduling priority class.
    pub priority: SchedulingPriority,
    /// CPU core affinity bitmask.
    pub affinity: u64,
    /// Voluntary + involuntary context switches per second.
    pub ctx_switches_per_sec: u64,
    /// Total CPU cycles consumed (where available).
    pub cpu_cycles: Option<u64>,
    /// Why the thread is currently waiting (if applicable).
    pub wait_reason: Option<String>,

    // -- Memory ------------------------------------------------------------
    /// Physical memory currently in use (working set) in bytes.
    pub mem_working_bytes: u64,
    /// Private (non-shared) memory in bytes.
    pub mem_private_bytes: u64,
    /// Shared memory in bytes.
    pub mem_shared_bytes: u64,
    /// Total virtual address space committed in bytes.
    pub mem_virtual_bytes: u64,
    /// Peak physical memory used in bytes.
    pub mem_peak_bytes: u64,
    /// Rate of page faults per second.
    pub page_faults_per_sec: u64,
    /// Paged pool kernel memory in bytes.
    pub paged_pool_bytes: u64,
    /// Non-paged pool kernel memory in bytes.
    pub nonpaged_pool_bytes: u64,
    /// Total committed memory in bytes.
    pub commit_bytes: u64,

    // -- Disk --------------------------------------------------------------
    /// Current disk read rate in bytes per second.
    pub disk_read_bytes_sec: u64,
    /// Current disk write rate in bytes per second.
    pub disk_write_bytes_sec: u64,
    /// Total bytes read since process start.
    pub disk_read_total_bytes: u64,
    /// Total bytes written since process start.
    pub disk_write_total_bytes: u64,
    /// Read I/O operations per second.
    pub iops_read: u64,
    /// Write I/O operations per second.
    pub iops_write: u64,
    /// I/O scheduling priority.
    pub io_priority: IoPriority,
    /// Count of pending I/O requests.
    pub pending_io: u32,

    // -- GPU / Graphics ----------------------------------------------------
    /// GPU engine utilization by this process as a percentage.
    pub gpu_percent: f64,
    /// Which GPU engine (3D, Copy, Video Decode, Video Encode, Compute).
    pub gpu_engine: Option<String>,
    /// Dedicated VRAM usage in bytes.
    pub gpu_mem_dedicated_bytes: u64,
    /// Shared GPU memory usage in bytes.
    pub gpu_mem_shared_bytes: u64,
    /// Total GPU memory committed in bytes.
    pub gpu_mem_total_bytes: u64,
    /// Estimated thermal contribution (relative).
    pub gpu_temp_contrib: Option<f64>,
    /// DirectX / Vulkan feature level in use.
    pub dx_level: Option<String>,
    /// Frames per second (for graphical apps, where detectable).
    pub fps: Option<f64>,
    /// Which GPU the process is bound to (multi-GPU systems).
    pub gpu_adapter: Option<String>,
    /// OpenGL / Vulkan / DirectX / Metal / Software.
    pub render_api: Option<String>,

    // -- Network -----------------------------------------------------------
    /// Current network send rate in bytes per second.
    pub net_send_bytes_sec: u64,
    /// Current network receive rate in bytes per second.
    pub net_recv_bytes_sec: u64,
    /// Total bytes sent.
    pub net_send_total_bytes: u64,
    /// Total bytes received.
    pub net_recv_total_bytes: u64,
    /// Active TCP/UDP connection count.
    pub connections: u32,

    // -- Energy ------------------------------------------------------------
    /// Estimated power draw (Very Low / Low / Moderate / High / Very High).
    pub power_usage: Option<String>,
    /// Increasing / Decreasing / Stable over last 60s.
    pub power_trend: Option<String>,
    /// Estimated mW impact on battery.
    pub battery_impact_mw: Option<f64>,

    // -- Misc --------------------------------------------------------------
    /// When the process was started (absolute timestamp).
    pub start_time: Option<String>,
    /// How long the process has been running in seconds.
    pub uptime_secs: Option<u64>,
    /// From application manifest or PE version info.
    pub description: Option<String>,
    /// Signed publisher / developer name.
    pub publisher: Option<String>,
    /// Package ID if installed via package manager.
    pub package_name: Option<String>,
    /// Untrusted / Low / Medium / High / System.
    pub integrity_level: Option<String>,
    /// UAC virtualization status (Enabled / Disabled / Not Allowed).
    pub virtualization: Option<String>,
    /// Data Execution Prevention status.
    pub dep_enabled: bool,
    /// Address Space Layout Randomization status.
    pub aslr_enabled: bool,
    /// Control Flow Guard status.
    pub cfg_enabled: bool,
    /// Whether running with elevated/admin privileges.
    pub elevated: bool,
    /// Whether running in a sandbox/container.
    pub sandboxed: bool,
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self {
            // Identity
            name: String::new(),
            pid: 0,
            ppid: None,
            status: ProcessStatus::Running,
            cmdline: String::new(),
            exe_path: None,
            cwd: None,
            user: String::new(),
            session_id: None,
            proc_type: ProcessType::App,

            // CPU
            cpu_percent: 0.0,
            cpu_time_ms: 0,
            cpu_user_ms: 0,
            cpu_kernel_ms: 0,
            threads: 0,
            handles: 0,
            priority: SchedulingPriority::Normal,
            affinity: 0,
            ctx_switches_per_sec: 0,
            cpu_cycles: None,
            wait_reason: None,

            // Memory
            mem_working_bytes: 0,
            mem_private_bytes: 0,
            mem_shared_bytes: 0,
            mem_virtual_bytes: 0,
            mem_peak_bytes: 0,
            page_faults_per_sec: 0,
            paged_pool_bytes: 0,
            nonpaged_pool_bytes: 0,
            commit_bytes: 0,

            // Disk
            disk_read_bytes_sec: 0,
            disk_write_bytes_sec: 0,
            disk_read_total_bytes: 0,
            disk_write_total_bytes: 0,
            iops_read: 0,
            iops_write: 0,
            io_priority: IoPriority::Normal,
            pending_io: 0,

            // GPU
            gpu_percent: 0.0,
            gpu_engine: None,
            gpu_mem_dedicated_bytes: 0,
            gpu_mem_shared_bytes: 0,
            gpu_mem_total_bytes: 0,
            gpu_temp_contrib: None,
            dx_level: None,
            fps: None,
            gpu_adapter: None,
            render_api: None,

            // Network
            net_send_bytes_sec: 0,
            net_recv_bytes_sec: 0,
            net_send_total_bytes: 0,
            net_recv_total_bytes: 0,
            connections: 0,

            // Energy
            power_usage: None,
            power_trend: None,
            battery_impact_mw: None,

            // Misc
            start_time: None,
            uptime_secs: None,
            description: None,
            publisher: None,
            package_name: None,
            integrity_level: None,
            virtualization: None,
            dep_enabled: false,
            aslr_enabled: false,
            cfg_enabled: false,
            elevated: false,
            sandboxed: false,
        }
    }
}
