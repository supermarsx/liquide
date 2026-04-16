//! Platform-agnostic data collection traits and native implementations.
//!
//! Each collector trait abstracts over platform-specific data sources
//! (procfs/sysfs on Linux, WMI/PDH/ETW on Windows, sysctl/IOKit on macOS)
//! and returns normalised types consumed by the aggregation layer.
//!
//! The [`NativeProcessCollector`] struct provides cross-platform process
//! enumeration using direct OS APIs (no shelling out). The [`CpuTracker`]
//! computes delta-based CPU percentages between sampling intervals.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::audio::stream::AudioStream;
use crate::devices::DeviceInfo;
use crate::energy::process_energy::ProcessEnergyInfo;
use crate::network::connection::ConnectionInfo;
use crate::performance::*;
use crate::process::{ProcessInfo, ProcessStatus, ProcessType};
use crate::services::ServiceInfo;
use crate::system_events::SystemEvent;

/// Collects per-process data from the operating system.
///
/// Implementations read raw counters from platform-specific APIs and return
/// normalised [`ProcessInfo`] structs ready for aggregation.
pub trait ProcessCollector {
    /// Return a snapshot of every running process.
    fn list_processes(&self) -> Result<Vec<ProcessInfo>, String>;

    /// Return detailed information for a single process identified by its PID.
    fn get_process(&self, pid: u32) -> Result<ProcessInfo, String>;
}

/// Collects system-wide performance statistics.
///
/// Provides CPU, memory, disk, GPU, network, power and audio metrics
/// that feed the Performance tab graphs and statistics panels.
pub trait PerformanceCollector {
    /// Return current CPU statistics.
    fn cpu_stats(&self) -> Result<CpuStats, String>;

    /// Return current memory statistics.
    fn memory_stats(&self) -> Result<MemoryStats, String>;

    /// Return current statistics for the disk identified by `index`.
    fn disk_stats(&self, index: u8) -> Result<DiskStats, String>;

    /// Return current statistics for the GPU identified by `index`.
    fn gpu_stats(&self, index: u8) -> Result<GpuStats, String>;

    /// Return current network adapter performance statistics.
    fn network_stats(&self) -> Result<NetworkPerfStats, String>;

    /// Return current power and battery statistics.
    fn power_stats(&self) -> Result<PowerStats, String>;

    /// Return current audio subsystem statistics.
    fn audio_stats(&self) -> Result<AudioPerfStats, String>;
}

/// Collects information about system services.
///
/// Implementations query systemd, the Windows SCM, or launchd to enumerate
/// all registered services and their current state.
pub trait ServiceCollector {
    /// Return a snapshot of every registered system service.
    fn list_services(&self) -> Result<Vec<ServiceInfo>, String>;
}

/// Collects hardware device inventory data.
///
/// Implementations enumerate PCI, USB, Bluetooth, and other bus devices
/// together with their driver and resource details.
pub trait DeviceCollector {
    /// Return a snapshot of every detected hardware device.
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, String>;
}

/// Collects active network connection data.
///
/// Implementations read from `/proc/net/*`, Netlink, or the IP Helper API
/// to enumerate all TCP, UDP, and QUIC connections.
pub trait NetworkCollector {
    /// Return a snapshot of every active network connection.
    fn list_connections(&self) -> Result<Vec<ConnectionInfo>, String>;
}

/// Collects per-process energy and power consumption data.
///
/// Implementations use RAPL, ACPI, or software estimation to attribute
/// system power draw to individual processes.
pub trait EnergyCollector {
    /// Return per-process energy consumption estimates.
    fn list_process_energy(&self) -> Result<Vec<ProcessEnergyInfo>, String>;
}

/// Collects active audio stream data.
///
/// Implementations query PipeWire, PulseAudio, WASAPI, or CoreAudio to
/// enumerate all render and capture streams.
pub trait AudioCollector {
    /// Return a snapshot of every active audio stream.
    fn list_audio_streams(&self) -> Result<Vec<AudioStream>, String>;
}

/// Collects system event log entries.
///
/// Implementations read from the Windows Event Log, Linux journald,
/// or macOS Unified Logging to enumerate system events.
pub trait SystemEventCollector {
    /// Return recent system events, optionally filtered by source.
    fn list_events(&self, source: Option<&str>, max: u32) -> Result<Vec<SystemEvent>, String>;
}

// ===========================================================================
// SystemMetrics — system-wide performance snapshot
// ===========================================================================

/// System-wide performance metrics snapshot used by the Performance tab and
/// status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Overall CPU utilization (0.0 - 100.0).
    pub cpu_percent: f32,
    /// Number of logical processors.
    pub cpu_count: u32,
    /// Total installed physical RAM in bytes.
    pub memory_total: u64,
    /// Physical RAM currently in use in bytes.
    pub memory_used: u64,
    /// Memory utilization (0.0 - 100.0).
    pub memory_percent: f32,
    /// Aggregate disk read rate in bytes per second.
    pub disk_read_bps: u64,
    /// Aggregate disk write rate in bytes per second.
    pub disk_write_bps: u64,
    /// Aggregate network send rate in bytes per second.
    pub network_send_bps: u64,
    /// Aggregate network receive rate in bytes per second.
    pub network_recv_bps: u64,
    /// System uptime in seconds.
    pub uptime_seconds: u64,
    /// Total number of running processes.
    pub process_count: u32,
    /// Total number of threads across all processes.
    pub thread_count: u32,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            cpu_count: 0,
            memory_total: 0,
            memory_used: 0,
            memory_percent: 0.0,
            disk_read_bps: 0,
            disk_write_bps: 0,
            network_send_bps: 0,
            network_recv_bps: 0,
            uptime_seconds: 0,
            process_count: 0,
            thread_count: 0,
        }
    }
}

// ===========================================================================
// CpuTracker — delta-based per-process CPU percentage
// ===========================================================================

/// Tracks per-process CPU times between samples to compute CPU usage
/// percentages from the delta.
#[derive(Debug, Clone)]
pub struct CpuTracker {
    /// Previous per-process CPU times: pid -> (user_ms, kernel_ms).
    prev_times: HashMap<u32, (u64, u64)>,
    /// Total CPU ticks at last sample (Linux: from /proc/stat, Windows: from
    /// GetSystemTimes, macOS: from host_statistics).
    prev_total_ms: u64,
    /// Number of logical processors, used to scale percentages.
    cpu_count: u32,
}

impl CpuTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self {
            prev_times: HashMap::new(),
            prev_total_ms: 0,
            cpu_count: 1,
        }
    }

    /// Set the logical processor count (used for scaling).
    pub fn set_cpu_count(&mut self, count: u32) {
        self.cpu_count = count.max(1);
    }

    /// Update CPU percentages on a list of processes using delta computation.
    ///
    /// `wall_elapsed_ms` is the wall-clock milliseconds since the last call.
    /// When zero, all percentages are set to 0.
    pub fn update(&mut self, processes: &mut [ProcessInfo], wall_elapsed_ms: u64) {
        if wall_elapsed_ms == 0 {
            return;
        }

        let total_budget_ms = wall_elapsed_ms as f64 * self.cpu_count as f64;

        for proc in processes.iter_mut() {
            let current_user = proc.cpu_user_ms;
            let current_kernel = proc.cpu_kernel_ms;
            let current_total = current_user + current_kernel;

            let prev_total = self
                .prev_times
                .get(&proc.pid)
                .map(|(u, k)| u + k)
                .unwrap_or(0);

            let delta = current_total.saturating_sub(prev_total);
            proc.cpu_percent = (delta as f64 / total_budget_ms * 100.0).min(100.0);

            self.prev_times
                .insert(proc.pid, (current_user, current_kernel));
        }

        self.prev_total_ms = self.prev_total_ms.wrapping_add(wall_elapsed_ms);
    }

    /// Remove stale entries for PIDs that no longer exist.
    pub fn gc(&mut self, live_pids: &std::collections::HashSet<u32>) {
        self.prev_times.retain(|pid, _| live_pids.contains(pid));
    }
}

impl Default for CpuTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// NativeProcessCollector — cross-platform FFI-based process enumeration
// ===========================================================================

/// Cross-platform process collector that uses native OS APIs directly
/// (no shelling out to PowerShell or ps).
#[derive(Debug)]
pub struct NativeProcessCollector;

impl NativeProcessCollector {
    /// Create a new native collector.
    pub fn new() -> Self {
        Self
    }

    /// Collect all running processes from the OS.
    pub fn collect_processes(&self) -> Vec<ProcessInfo> {
        collect_processes_native()
    }

    /// Collect system-wide performance metrics.
    pub fn collect_system_metrics(&self) -> SystemMetrics {
        collect_system_metrics_native()
    }
}

impl Default for NativeProcessCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessCollector for NativeProcessCollector {
    fn list_processes(&self) -> Result<Vec<ProcessInfo>, String> {
        Ok(self.collect_processes())
    }

    fn get_process(&self, pid: u32) -> Result<ProcessInfo, String> {
        let procs = self.collect_processes();
        procs
            .into_iter()
            .find(|p| p.pid == pid)
            .ok_or_else(|| format!("process {pid} not found"))
    }
}

// ===========================================================================
// Linux: /proc-based process enumeration
// ===========================================================================

#[cfg(target_os = "linux")]
fn collect_processes_native() -> Vec<ProcessInfo> {
    use std::fs;
    use std::path::Path;

    let proc_dir = Path::new("/proc");
    let mut procs = Vec::new();

    let entries = match fs::read_dir(proc_dir) {
        Ok(e) => e,
        Err(_) => return procs,
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let pid: u32 = match name_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Read /proc/{pid}/stat for process state, CPU times, ppid
        let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Parse stat: pid (comm) state ppid ...
        // comm is in parentheses and may contain spaces/parens
        let comm_start = match stat.find('(') {
            Some(i) => i + 1,
            None => continue,
        };
        let comm_end = match stat.rfind(')') {
            Some(i) => i,
            None => continue,
        };
        let comm = stat[comm_start..comm_end].to_string();
        let rest = match stat.get(comm_end + 2..) {
            Some(r) => r,
            None => continue,
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();

        if fields.len() < 20 {
            continue;
        }

        let state_char = fields[0].chars().next().unwrap_or('?');
        let ppid: u32 = fields[1].parse().unwrap_or(0);
        let utime: u64 = fields[11].parse().unwrap_or(0);
        let stime: u64 = fields[12].parse().unwrap_or(0);
        let threads: u32 = fields[17].parse().unwrap_or(1);
        let start_time_ticks: u64 = fields[19].parse().unwrap_or(0);

        let status = match state_char {
            'R' => ProcessStatus::Running,
            'S' => ProcessStatus::Sleeping,
            'D' => ProcessStatus::DiskSleep,
            'Z' => ProcessStatus::Zombie,
            'T' | 't' => ProcessStatus::Stopped,
            'I' => ProcessStatus::Idle,
            'W' => ProcessStatus::Waiting,
            _ => ProcessStatus::Running,
        };

        // Convert jiffies to milliseconds (assuming 100 Hz tick rate)
        let hz: u64 = 100;
        let user_ms = utime * 1000 / hz;
        let kernel_ms = stime * 1000 / hz;

        // Read /proc/{pid}/status for VmRSS and UID
        let status_file =
            fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
        let mut memory_kb: u64 = 0;
        let mut uid: u32 = 0;
        let mut vm_size_kb: u64 = 0;
        let mut vm_peak_kb: u64 = 0;
        for line in status_file.lines() {
            if let Some(val) = line.strip_prefix("VmRSS:") {
                memory_kb = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("Uid:") {
                uid = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("VmSize:") {
                vm_size_kb = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("VmPeak:") {
                vm_peak_kb = val
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }

        // Read /proc/{pid}/cmdline for full command
        let cmdline = fs::read_to_string(format!("/proc/{pid}/cmdline"))
            .unwrap_or_default()
            .replace('\0', " ")
            .trim()
            .to_string();

        // Read /proc/{pid}/exe for binary path
        let exe_path = fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        // Read I/O stats from /proc/{pid}/io (may fail without permissions)
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

        let proc_type = if pid <= 2 {
            ProcessType::System
        } else if uid == 0 {
            ProcessType::Service
        } else if exe_path.is_none() {
            ProcessType::Background
        } else {
            ProcessType::App
        };

        // Uptime: convert start_time_ticks to approximate seconds since boot
        let uptime_secs = if hz > 0 {
            Some(start_time_ticks / hz)
        } else {
            None
        };

        procs.push(ProcessInfo {
            pid,
            name: comm,
            ppid: Some(ppid),
            status,
            cmdline,
            exe_path,
            proc_type,
            cpu_user_ms: user_ms,
            cpu_kernel_ms: kernel_ms,
            cpu_time_ms: user_ms + kernel_ms,
            threads,
            mem_working_bytes: memory_kb * 1024,
            mem_virtual_bytes: vm_size_kb * 1024,
            mem_peak_bytes: vm_peak_kb * 1024,
            disk_read_total_bytes: io_read,
            disk_write_total_bytes: io_write,
            uptime_secs,
            user: uid.to_string(),
            ..ProcessInfo::default()
        });
    }

    procs
}

#[cfg(target_os = "linux")]
fn collect_system_metrics_native() -> SystemMetrics {
    use std::fs;

    let mut metrics = SystemMetrics::default();

    // CPU: parse /proc/stat (first line: cpu user nice system idle ...)
    if let Ok(content) = fs::read_to_string("/proc/stat") {
        if let Some(line) = content.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 && parts[0] == "cpu" {
                let user: u64 = parts[1].parse().unwrap_or(0);
                let nice: u64 = parts[2].parse().unwrap_or(0);
                let system: u64 = parts[3].parse().unwrap_or(0);
                let idle: u64 = parts[4].parse().unwrap_or(0);
                let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                let irq: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                let softirq: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);

                let total = user + nice + system + idle + iowait + irq + softirq;
                let busy = total - idle - iowait;
                if total > 0 {
                    metrics.cpu_percent = (busy as f64 / total as f64 * 100.0) as f32;
                }
            }
            // Count logical processors from per-core lines
            let cpu_count = content
                .lines()
                .filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(|b| b.is_ascii_digit()))
                .count();
            metrics.cpu_count = cpu_count as u32;
        }
    }

    // Memory: parse /proc/meminfo
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        let mut total: u64 = 0;
        let mut available: u64 = 0;
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let key = parts[0].trim_end_matches(':');
            let value_kb: u64 = parts[1].parse().unwrap_or(0);
            match key {
                "MemTotal" => total = value_kb * 1024,
                "MemAvailable" => available = value_kb * 1024,
                _ => {}
            }
        }
        metrics.memory_total = total;
        metrics.memory_used = total.saturating_sub(available);
        if total > 0 {
            metrics.memory_percent =
                (metrics.memory_used as f64 / total as f64 * 100.0) as f32;
        }
    }

    // Uptime
    if let Ok(content) = fs::read_to_string("/proc/uptime") {
        if let Some(secs_str) = content.split_whitespace().next() {
            metrics.uptime_seconds = secs_str.parse::<f64>().unwrap_or(0.0) as u64;
        }
    }

    metrics
}

// ===========================================================================
// Windows: CreateToolhelp32Snapshot + native memory/system APIs
// ===========================================================================

#[cfg(target_os = "windows")]
fn collect_processes_native() -> Vec<ProcessInfo> {
    use std::ffi::c_void;
    use std::mem;

    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    #[repr(C)]
    struct ProcessMemoryCountersEx {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    #[repr(C)]
    struct Filetime {
        dw_low_date_time: u32,
        dw_high_date_time: u32,
    }

    const TH32CS_SNAPPROCESS: u32 = 0x02;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Process32FirstW(snap: *mut c_void, pe: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snap: *mut c_void, pe: *mut ProcessEntry32W) -> i32;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetProcessTimes(
            proc: *mut c_void,
            creation: *mut Filetime,
            exit: *mut Filetime,
            kernel: *mut Filetime,
            user: *mut Filetime,
        ) -> i32;
    }

    // GetProcessMemoryInfo is in psapi / kernel32
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(
            proc: *mut c_void,
            info: *mut ProcessMemoryCountersEx,
            size: u32,
        ) -> i32;
    }

    // SAFETY: CreateToolhelp32Snapshot is a Win32 API that returns an owned
    // handle. TH32CS_SNAPPROCESS is a valid flag. Returns INVALID_HANDLE_VALUE
    // on failure, which we check below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Vec::new();
    }

    let mut procs = Vec::new();
    // SAFETY: ProcessEntry32W is a repr(C) POD struct. Zero-init is valid
    // for this struct per the Win32 API documentation. We set dw_size to
    // the struct size as required by Process32FirstW.
    let mut pe: ProcessEntry32W = unsafe { mem::zeroed() };
    pe.dw_size = mem::size_of::<ProcessEntry32W>() as u32;

    // SAFETY: `snap` is a valid snapshot handle. `pe` has dw_size set
    // correctly. Process32FirstW writes into `pe` on success (returns != 0).
    if unsafe { Process32FirstW(snap, &mut pe) } != 0 {
        loop {
            let name_end = pe
                .sz_exe_file
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(260);
            let name = String::from_utf16_lossy(&pe.sz_exe_file[..name_end]);

            let pid = pe.th32_process_id;
            let ppid = pe.th32_parent_process_id;
            let thread_count = pe.cnt_threads;
            let priority_base = pe.pc_pri_class_base;

            // Open process for memory and timing queries
            let mut mem_working: u64 = 0;
            let mut mem_private: u64 = 0;
            let mut mem_peak: u64 = 0;
            let mut cpu_user_ms: u64 = 0;
            let mut cpu_kernel_ms: u64 = 0;
            let mut page_faults: u32 = 0;

            // SAFETY: OpenProcess with PROCESS_QUERY_LIMITED_INFORMATION is a
            // safe Win32 call. Returns null on failure (e.g. access denied).
            let proc_handle =
                unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            if !proc_handle.is_null() {
                // Memory info
                // SAFETY: `pmc` is zero-initialized with `cb` set to the
                // correct struct size. `proc_handle` is a valid process handle.
                let mut pmc: ProcessMemoryCountersEx = unsafe { mem::zeroed() };
                pmc.cb = mem::size_of::<ProcessMemoryCountersEx>() as u32;
                if unsafe { K32GetProcessMemoryInfo(proc_handle, &mut pmc, pmc.cb) } != 0 {
                    mem_working = pmc.working_set_size as u64;
                    mem_private = pmc.private_usage as u64;
                    mem_peak = pmc.peak_working_set_size as u64;
                    page_faults = pmc.page_fault_count;
                }

                // CPU times
                // SAFETY: All Filetime structs are zero-initialized PODs.
                // `proc_handle` is a valid process handle. GetProcessTimes
                // writes creation/exit/kernel/user times as FILETIME structs.
                let mut creation: Filetime = unsafe { mem::zeroed() };
                let mut exit: Filetime = unsafe { mem::zeroed() };
                let mut kernel: Filetime = unsafe { mem::zeroed() };
                let mut user: Filetime = unsafe { mem::zeroed() };
                if unsafe {
                    GetProcessTimes(
                        proc_handle,
                        &mut creation,
                        &mut exit,
                        &mut kernel,
                        &mut user,
                    )
                } != 0
                {
                    // FILETIME is in 100-nanosecond intervals
                    let user_100ns =
                        (user.dw_high_date_time as u64) << 32 | user.dw_low_date_time as u64;
                    let kernel_100ns = (kernel.dw_high_date_time as u64) << 32
                        | kernel.dw_low_date_time as u64;
                    cpu_user_ms = user_100ns / 10_000;
                    cpu_kernel_ms = kernel_100ns / 10_000;
                }

                // SAFETY: `proc_handle` is non-null and valid.
                unsafe {
                    CloseHandle(proc_handle);
                }
            }

            let proc_type = if pid == 0 || pid == 4 {
                ProcessType::System
            } else if priority_base >= 13 {
                ProcessType::System
            } else {
                ProcessType::App
            };

            let priority = match priority_base {
                24 => crate::process::SchedulingPriority::Realtime,
                13 => crate::process::SchedulingPriority::High,
                10 => crate::process::SchedulingPriority::AboveNormal,
                6 => crate::process::SchedulingPriority::BelowNormal,
                4 => crate::process::SchedulingPriority::Idle,
                _ => crate::process::SchedulingPriority::Normal,
            };

            procs.push(ProcessInfo {
                pid,
                name,
                ppid: Some(ppid),
                status: ProcessStatus::Running,
                proc_type,
                threads: thread_count,
                mem_working_bytes: mem_working,
                mem_private_bytes: mem_private,
                mem_peak_bytes: mem_peak,
                cpu_user_ms,
                cpu_kernel_ms,
                cpu_time_ms: cpu_user_ms + cpu_kernel_ms,
                priority,
                page_faults_per_sec: page_faults as u64, // snapshot, not rate
                ..ProcessInfo::default()
            });

            // SAFETY: Zero-init is valid for ProcessEntry32W. We set dw_size
            // as required by Process32NextW.
            pe = unsafe { mem::zeroed() };
            pe.dw_size = mem::size_of::<ProcessEntry32W>() as u32;
            // SAFETY: `snap` is still a valid snapshot handle.
            if unsafe { Process32NextW(snap, &mut pe) } == 0 {
                break;
            }
        }
    }

    // SAFETY: `snap` is a valid handle from CreateToolhelp32Snapshot.
    unsafe {
        CloseHandle(snap);
    }

    procs
}

#[cfg(target_os = "windows")]
fn collect_system_metrics_native() -> SystemMetrics {
    use std::ffi::c_void;
    use std::mem;

    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    #[repr(C)]
    struct Filetime {
        dw_low_date_time: u32,
        dw_high_date_time: u32,
    }

    #[repr(C)]
    struct SystemInfo {
        processor_architecture: u16,
        reserved: u16,
        page_size: u32,
        minimum_application_address: *mut c_void,
        maximum_application_address: *mut c_void,
        active_processor_mask: usize,
        number_of_processors: u32,
        processor_type: u32,
        allocation_granularity: u32,
        processor_level: u16,
        processor_revision: u16,
    }

    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatusEx) -> i32;
        fn GetSystemTimes(
            idle: *mut Filetime,
            kernel: *mut Filetime,
            user: *mut Filetime,
        ) -> i32;
        fn GetSystemInfo(info: *mut SystemInfo);
        fn GetTickCount64() -> u64;
    }

    let mut metrics = SystemMetrics::default();

    // CPU count
    // SAFETY: SystemInfo is a repr(C) POD struct; zero-init is valid.
    // GetSystemInfo is a safe Win32 call that fills the output struct.
    let mut sys_info: SystemInfo = unsafe { mem::zeroed() };
    unsafe { GetSystemInfo(&mut sys_info) };
    metrics.cpu_count = sys_info.number_of_processors;

    // CPU usage from GetSystemTimes
    // SAFETY: Filetime is a repr(C) POD struct; zero-init is valid.
    // GetSystemTimes writes idle/kernel/user times as FILETIME structs.
    let mut idle_ft: Filetime = unsafe { mem::zeroed() };
    let mut kernel_ft: Filetime = unsafe { mem::zeroed() };
    let mut user_ft: Filetime = unsafe { mem::zeroed() };
    if unsafe { GetSystemTimes(&mut idle_ft, &mut kernel_ft, &mut user_ft) } != 0 {
        let idle =
            (idle_ft.dw_high_date_time as u64) << 32 | idle_ft.dw_low_date_time as u64;
        let kernel = (kernel_ft.dw_high_date_time as u64) << 32
            | kernel_ft.dw_low_date_time as u64;
        let user =
            (user_ft.dw_high_date_time as u64) << 32 | user_ft.dw_low_date_time as u64;
        // kernel includes idle time
        let total = kernel + user;
        let busy = total - idle;
        if total > 0 {
            metrics.cpu_percent = (busy as f64 / total as f64 * 100.0) as f32;
        }
    }

    // Memory
    // SAFETY: MemoryStatusEx is a repr(C) POD struct with dw_length set
    // to the correct struct size. GlobalMemoryStatusEx fills the struct.
    let mut mem_status: MemoryStatusEx = unsafe { mem::zeroed() };
    mem_status.dw_length = mem::size_of::<MemoryStatusEx>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut mem_status) } != 0 {
        metrics.memory_total = mem_status.ull_total_phys;
        metrics.memory_used =
            mem_status.ull_total_phys.saturating_sub(mem_status.ull_avail_phys);
        metrics.memory_percent = mem_status.dw_memory_load as f32;
    }

    // Uptime
    // SAFETY: GetTickCount64 is a stateless Win32 function with no
    // preconditions; it returns milliseconds since system boot.
    metrics.uptime_seconds = unsafe { GetTickCount64() } / 1000;

    metrics
}

// ===========================================================================
// macOS: proc_listpids / proc_pidinfo / proc_name
// ===========================================================================

#[cfg(target_os = "macos")]
fn collect_processes_native() -> Vec<ProcessInfo> {
    unsafe extern "C" {
        fn proc_listpids(type_: u32, typeinfo: u32, buffer: *mut i32, bufsize: i32) -> i32;
        fn proc_name(pid: i32, buffer: *mut u8, bufsize: u32) -> i32;
    }

    const PROC_ALL_PIDS: u32 = 1;

    // Get PID list
    // SAFETY: proc_listpids with a null buffer and bufsize 0 returns the
    // required buffer size. PROC_ALL_PIDS is a valid type constant.
    let count = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if count <= 0 {
        return Vec::new();
    }

    let mut pids = vec![0i32; count as usize / 4 + 16];
    // SAFETY: `pids` is a valid mutable buffer with length derived from the
    // prior proc_listpids call. bufsize is computed from the actual allocation
    // size. proc_listpids writes at most bufsize bytes into the buffer.
    let actual = unsafe {
        proc_listpids(
            PROC_ALL_PIDS,
            0,
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<i32>()) as i32,
        )
    };
    if actual <= 0 {
        return Vec::new();
    }
    let pid_count = actual as usize / std::mem::size_of::<i32>();

    let mut procs = Vec::new();
    for &pid in &pids[..pid_count] {
        if pid <= 0 {
            continue;
        }

        let mut name_buf = [0u8; 256];
        // SAFETY: `name_buf` is a stack-allocated 256-byte array.
        // `pid` is a positive PID from the system PID list.
        // proc_name writes at most `bufsize` (256) bytes into the buffer.
        let name_len = unsafe { proc_name(pid, name_buf.as_mut_ptr(), 256) };
        let name = if name_len > 0 {
            String::from_utf8_lossy(&name_buf[..name_len as usize]).to_string()
        } else {
            format!("pid_{pid}")
        };

        procs.push(ProcessInfo {
            pid: pid as u32,
            name,
            status: ProcessStatus::Running,
            proc_type: ProcessType::App,
            ..ProcessInfo::default()
        });
    }

    procs
}

#[cfg(target_os = "macos")]
fn collect_system_metrics_native() -> SystemMetrics {
    use std::process::Command;

    let mut metrics = SystemMetrics::default();

    // CPU count via sysctl
    if let Ok(output) = Command::new("sysctl").args(["-n", "hw.logicalcpu"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            metrics.cpu_count = s.trim().parse().unwrap_or(1);
        }
    }

    // Memory via sysctl
    if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            metrics.memory_total = s.trim().parse().unwrap_or(0);
        }
    }

    metrics
}

// ===========================================================================
// Fallback for other platforms
// ===========================================================================

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn collect_processes_native() -> Vec<ProcessInfo> {
    Vec::new()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn collect_system_metrics_native() -> SystemMetrics {
    SystemMetrics::default()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_tracker_zero_elapsed_noop() {
        let mut tracker = CpuTracker::new();
        let mut procs = vec![ProcessInfo {
            pid: 1,
            cpu_user_ms: 100,
            cpu_kernel_ms: 50,
            ..ProcessInfo::default()
        }];
        tracker.update(&mut procs, 0);
        assert_eq!(procs[0].cpu_percent, 0.0);
    }

    #[test]
    fn cpu_tracker_computes_delta() {
        let mut tracker = CpuTracker::new();
        tracker.set_cpu_count(4);

        // First sample: seed the tracker
        let mut procs = vec![ProcessInfo {
            pid: 42,
            cpu_user_ms: 100,
            cpu_kernel_ms: 50,
            ..ProcessInfo::default()
        }];
        tracker.update(&mut procs, 1000);
        // First sample always measures from 0
        let first_pct = procs[0].cpu_percent;
        assert!(first_pct > 0.0);

        // Second sample: 200ms more CPU in 1000ms wall time on 4 cores
        procs[0].cpu_user_ms = 250;
        procs[0].cpu_kernel_ms = 100;
        tracker.update(&mut procs, 1000);
        // delta = (250+100) - (100+50) = 200, budget = 1000*4 = 4000
        // percent = 200/4000 * 100 = 5.0
        assert!((procs[0].cpu_percent - 5.0).abs() < 0.01);
    }

    #[test]
    fn cpu_tracker_gc_removes_stale() {
        let mut tracker = CpuTracker::new();
        let mut procs = vec![
            ProcessInfo {
                pid: 1,
                cpu_user_ms: 10,
                cpu_kernel_ms: 5,
                ..ProcessInfo::default()
            },
            ProcessInfo {
                pid: 2,
                cpu_user_ms: 20,
                cpu_kernel_ms: 10,
                ..ProcessInfo::default()
            },
        ];
        tracker.update(&mut procs, 1000);
        assert_eq!(tracker.prev_times.len(), 2);

        let mut live = std::collections::HashSet::new();
        live.insert(1);
        tracker.gc(&live);
        assert_eq!(tracker.prev_times.len(), 1);
        assert!(tracker.prev_times.contains_key(&1));
    }

    #[test]
    fn system_metrics_default_is_zero() {
        let m = SystemMetrics::default();
        assert_eq!(m.cpu_percent, 0.0);
        assert_eq!(m.memory_total, 0);
        assert_eq!(m.process_count, 0);
    }

    #[test]
    fn native_collector_returns_processes() {
        let collector = NativeProcessCollector::new();
        let procs = collector.collect_processes();
        // On any real OS there should be at least 1 process (ourselves)
        assert!(!procs.is_empty(), "should find at least one process");
    }

    #[test]
    fn native_collector_system_metrics_nonzero() {
        let collector = NativeProcessCollector::new();
        let metrics = collector.collect_system_metrics();
        // Memory total should be > 0 on any real system
        assert!(metrics.memory_total > 0, "memory_total should be > 0");
    }

    #[test]
    fn native_collector_process_has_pid() {
        let collector = NativeProcessCollector::new();
        let procs = collector.collect_processes();
        // At least one process should have a non-empty name
        let has_named = procs.iter().any(|p| !p.name.is_empty());
        assert!(has_named, "at least one process should have a name");
    }

    #[test]
    fn process_collector_trait_impl() {
        let collector = NativeProcessCollector::new();
        let result = collector.list_processes();
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn process_collector_get_nonexistent() {
        let collector = NativeProcessCollector::new();
        let result = collector.get_process(u32::MAX);
        assert!(result.is_err());
    }
}
