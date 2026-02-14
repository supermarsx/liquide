# Liquide Task Manager — Comprehensive Specification

**Version:** 1.0  
**Date:** 2026-02-12  
**Status:** Draft  
**Target Platform:** Liquide Desktop (cross-platform core, native rendering)

---

## Table of Contents

1. [Overview](#1-overview)  
2. [Architecture](#2-architecture)  
3. [Global UI Shell](#3-global-ui-shell)  
4. [Tab: Processes](#4-tab-processes)  
5. [Tab: Performance](#5-tab-performance)  
6. [Tab: App History](#6-tab-app-history)  
7. [Tab: Startup (Boot)](#7-tab-startup-boot)  
8. [Tab: Users & Sessions](#8-tab-users--sessions)  
9. [Tab: Services](#9-tab-services)  
10. [Tab: Devices](#10-tab-devices)  
11. [Tab: Files & Folders In Use](#11-tab-files--folders-in-use)  
12. [Tab: Resource Unlocking](#12-tab-resource-unlocking)  
13. [Tab: Process Tree (Roots)](#13-tab-process-tree-roots)  
14. [Tab: Network Traffic](#14-tab-network-traffic)  
15. [Tab: Energy & Power](#15-tab-energy--power)  
16. [Tab: Audio](#16-tab-audio)  
17. [Settings & Configuration](#17-settings--configuration)  
18. [Keyboard Shortcuts](#18-keyboard-shortcuts)  
19. [Accessibility](#19-accessibility)  
20. [Security Model](#20-security-model)  
21. [Data Collection & Telemetry](#21-data-collection--telemetry)  
22. [Performance Budget](#22-performance-budget)  
23. [API Surface](#23-api-surface)  

---

## 1. Overview

### 1.1 Purpose

The Liquide Task Manager is a system-level diagnostic and management tool providing real-time visibility into all running processes, system resource utilization, hardware performance, active services, device states, file locks, user sessions, and boot-time applications. It aims to exceed the capabilities of the Windows 11 Task Manager while maintaining a clean, information-dense UI.

### 1.2 Goals

| Goal | Description |
|------|-------------|
| **Comprehensive Monitoring** | Expose every measurable system metric across CPU, GPU, memory, disk, network, and power |
| **Actionable Control** | Allow users to end, suspend, resume, prioritize, and affinity-pin any process or service |
| **Resource Unlocking** | Identify and release locked files/folders/handles without third-party tools |
| **Performance Analysis** | Provide rolling graphs, historical snapshots, and exportable data for all metrics |
| **Low Overhead** | Task Manager itself must consume < 0.5% CPU and < 50 MB RAM at idle |
| **Extensibility** | Plugin architecture for custom tabs, columns, and data sources |

### 1.3 Non-Goals

- Replacing a full profiler (e.g., perf, VTune)
- Acting as a package manager (use Software Center)
- Real-time kernel debugging

---

## 2. Architecture

### 2.1 Component Diagram

```
┌─────────────────────────────────────────────────────┐
│                  Task Manager UI                     │
│  ┌──────────┬──────────┬──────────┬──────────┐      │
│  │Processes │Performnce│ Services │ Devices  │ ...  │
│  └────┬─────┴────┬─────┴────┬─────┴────┬─────┘      │
│       │          │          │          │              │
│  ┌────▼──────────▼──────────▼──────────▼────┐        │
│  │         Data Aggregation Layer            │        │
│  │  (sampling, smoothing, ring buffers)      │        │
│  └────┬──────────┬──────────┬──────────┬────┘        │
│       │          │          │          │              │
│  ┌────▼────┐ ┌───▼───┐ ┌───▼───┐ ┌───▼────┐        │
│  │ procfs  │ │sysfs  │ │ WMI/  │ │ D-Bus/ │        │
│  │ reader  │ │reader │ │CIM    │ │systemd │        │
│  └─────────┘ └───────┘ └───────┘ └────────┘        │
└─────────────────────────────────────────────────────┘
```

### 2.2 Data Pipeline

1. **Collectors** — Platform-specific modules that read raw counters (procfs/sysfs on Linux, WMI/PDH/ETW on Windows, sysctl/IOKit on macOS).
2. **Aggregator** — Normalizes, samples (configurable 0.5–10 Hz), and stores data in fixed-size ring buffers (default 60 s at 1 Hz = 60 samples per metric).
3. **Renderer** — Pulls from aggregator, renders graphs and tables using the compositor's GPU-accelerated canvas.
4. **Action Dispatcher** — Sends privileged commands (kill, renice, unlock) via an elevated helper daemon.

### 2.3 Elevated Helper Daemon

- Runs as root/SYSTEM.
- Communicates over a local Unix socket / named pipe.
- Commands are signed with a session token obtained at launch via polkit/UAC prompt.
- Supports: `kill`, `suspend`, `resume`, `renice`, `setaffinity`, `unlock_handle`, `enable_service`, `disable_service`, `set_startup`.

---

## 3. Global UI Shell

### 3.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│ ☰  Liquide Task Manager          ─  □  ✕   [⚙ Settings]    │
├──────────────────────────────────────────────────────────────┤
│ [Processes] [Performance] [App History] [Startup] [Users]    │
│ [Services] [Devices] [Files In Use] [Unlock] [Process Tree]  │
│ [Network Traffic] [Energy & Power] [Audio]                    │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                    << Active Tab Content >>                   │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ Status: CPU 12% | RAM 8.2/16 GB | Disk 45 MB/s | ↑↓ Net     │
│   GPU 34% | 🔋 87% 4h12m | 🔊 Out: Speakers | ⚡ 28W        │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 View Modes

| Mode | Description |
|------|-------------|
| **Compact** | Processes tab only, minimal columns (Name, CPU%, RAM). Resizable mini-window. |
| **Standard** | All tabs, default column sets, graphs at medium resolution. |
| **Advanced** | All tabs, all columns visible, high-resolution graphs, debug info. |
| **Floating Widget** | Always-on-top mini overlay showing CPU/RAM/GPU gauges (detachable). |

### 3.3 Theme Integration

- Respects the active Liquide desktop theme (light/dark/custom).
- Graph colors configurable per-metric (see §14).
- High-contrast mode with WCAG AAA compliance.

### 3.4 Status Bar

Always visible at the bottom. Shows at-a-glance system totals:

| Field | Format |
|-------|--------|
| CPU | `CPU: 12% (4.2 GHz)` |
| Memory | `RAM: 8.2 / 16.0 GB (51%)` |
| Disk | `Disk: R 45 MB/s W 12 MB/s` |
| Network | `Net: ↑ 1.2 MB/s ↓ 8.4 MB/s` |
| GPU | `GPU: 34% (VRAM 2.1/8 GB)` |
| Processes | `Processes: 312 | Threads: 4,891` |
| Uptime | `Up: 3d 14h 22m` |

---

## 4. Tab: Processes

### 4.1 Overview

The primary tab. Displays every running process in a sortable, filterable, groupable table with real-time updates.

### 4.2 Column Definitions

All columns are optional and togglable. Default columns marked with ★.

#### Identity Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Name | `name` | Executable display name (friendly name from manifest or binary name) | ★ |
| PID | `pid` | Process ID (numeric) | ★ |
| PPID | `ppid` | Parent Process ID | |
| Status | `status` | Current process state | ★ |
| Command Line | `cmdline` | Full command line with all arguments | ★ |
| Executable Path | `exe_path` | Absolute path to the binary on disk | |
| Working Directory | `cwd` | Current working directory of the process | |
| User | `user` | User account running the process | ★ |
| Session ID | `session_id` | Login session identifier | |
| Process Type | `proc_type` | App / Background / Service / System / Shell | ★ |

#### CPU Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| CPU % | `cpu_percent` | Current CPU usage as percentage of all cores | ★ |
| CPU Time | `cpu_time` | Total accumulated CPU time (user + kernel) | |
| CPU User Time | `cpu_user` | User-mode CPU time | |
| CPU Kernel Time | `cpu_kernel` | Kernel-mode CPU time | |
| Threads | `threads` | Number of active threads | ★ |
| Handles / FDs | `handles` | Open file descriptors / handles count | |
| Base Priority | `priority` | Scheduling priority (Idle/BelowNormal/Normal/AboveNormal/High/Realtime) | |
| Affinity Mask | `affinity` | CPU core affinity bitmask (display as core list) | |
| Context Switches | `ctx_switches` | Voluntary + involuntary context switch count / sec | |
| CPU Cycles | `cpu_cycles` | Total CPU cycles consumed (where available) | |
| Wait Reason | `wait_reason` | Why the thread is currently waiting (if applicable) | |

#### Memory Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Memory (Working Set) | `mem_working` | Physical memory currently in use | ★ |
| Memory (Private) | `mem_private` | Private (non-shared) memory | |
| Memory (Shared) | `mem_shared` | Shared memory | |
| Memory (Virtual) | `mem_virtual` | Total virtual address space committed | |
| Peak Working Set | `mem_peak` | Peak physical memory used | |
| Page Faults / sec | `page_faults` | Rate of page faults | |
| Paged Pool | `paged_pool` | Paged pool kernel memory | |
| Non-Paged Pool | `nonpaged_pool` | Non-paged pool kernel memory | |
| Commit Size | `commit` | Total committed memory | |

#### Disk Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Disk Read (B/s) | `disk_read` | Current disk read rate | ★ |
| Disk Write (B/s) | `disk_write` | Current disk write rate | ★ |
| Disk Read (Total) | `disk_read_total` | Total bytes read since process start | |
| Disk Write (Total) | `disk_write_total` | Total bytes written since process start | |
| IOPS Read | `iops_read` | Read I/O operations per second | |
| IOPS Write | `iops_write` | Write I/O operations per second | |
| I/O Priority | `io_priority` | I/O scheduling priority | |
| Pending I/O | `pending_io` | Count of pending I/O requests | |

#### GPU / Graphics Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| GPU % | `gpu_percent` | GPU engine utilization by this process | ★ |
| GPU Engine | `gpu_engine` | Which GPU engine (3D, Copy, Video Decode, Video Encode, Compute) | |
| GPU Memory (Dedicated) | `gpu_mem_dedicated` | Dedicated VRAM usage | |
| GPU Memory (Shared) | `gpu_mem_shared` | Shared GPU memory usage | |
| GPU Memory (Total) | `gpu_mem_total` | Total GPU memory committed | |
| GPU Temperature Contribution | `gpu_temp_contrib` | Estimated thermal contribution (relative) | |
| DirectX Feature Level | `dx_level` | DirectX / Vulkan feature level in use | |
| Frame Rate | `fps` | Frames per second (for graphical apps, where detectable) | |
| GPU Adapter | `gpu_adapter` | Which GPU the process is bound to (multi-GPU systems) | |
| Render API | `render_api` | OpenGL / Vulkan / DirectX / Metal / Software | |

#### Network Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Network Send (B/s) | `net_send` | Current network send rate | |
| Network Recv (B/s) | `net_recv` | Current network receive rate | |
| Network Send (Total) | `net_send_total` | Total bytes sent | |
| Network Recv (Total) | `net_recv_total` | Total bytes received | |
| Connections | `connections` | Active TCP/UDP connection count | |

#### Energy Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Power Usage | `power` | Estimated power draw (Very Low / Low / Moderate / High / Very High) | |
| Power Trend | `power_trend` | Increasing / Decreasing / Stable over last 60s | |
| Battery Impact | `battery_impact` | Estimated mW impact on battery | |

#### Misc Columns

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Start Time | `start_time` | When the process was started (absolute timestamp) | |
| Uptime | `uptime` | How long the process has been running | |
| Description | `description` | From application manifest or PE version info | |
| Publisher | `publisher` | Signed publisher / developer name | |
| Package Name | `package` | Package ID if installed via package manager | |
| Integrity Level | `integrity` | Untrusted / Low / Medium / High / System | |
| Virtualization | `virtualization` | UAC virtualization status (Enabled/Disabled/Not Allowed) | |
| DEP | `dep` | Data Execution Prevention status | |
| ASLR | `aslr` | Address Space Layout Randomization status | |
| Control Flow Guard | `cfg` | CFG status | |
| Elevated | `elevated` | Whether running with elevated/admin privileges | ★ |
| Sandboxed | `sandboxed` | Whether running in a sandbox/container | |

### 4.3 Process Status Values

| Status | Icon | Color | Description |
|--------|------|-------|-------------|
| Running | ▶ | Green | Actively executing or ready |
| Sleeping | 💤 | Gray | Waiting for I/O or timer |
| Stopped | ⏸ | Yellow | Suspended / paused |
| Zombie | ☠ | Red | Terminated but not yet reaped |
| Idle | ○ | Light Gray | System idle process |
| Not Responding | ⚠ | Orange | Window message pump unresponsive for > 5s |
| Suspended | ⏸ | Blue | Explicitly suspended by user or system |
| Waiting | ⏳ | Cyan | Waiting on a specific resource |
| Disk Sleep | 💾 | Purple | Uninterruptible disk I/O wait |

### 4.4 Grouping Modes

| Group By | Description |
|----------|-------------|
| **Type** | App / Background Process / Windows Process |
| **Status** | Running / Suspended / Not Responding |
| **User** | Group by owning user account |
| **Session** | Group by login session |
| **Priority** | Group by scheduling priority class |
| **GPU Adapter** | Group by which GPU is being used |
| **Package** | Group by installation package |
| **None** | Flat list |

### 4.5 Context Menu Actions

Right-clicking a process shows:

```
├── End Task                          (Ctrl+Del)
├── End Process Tree                  (Ctrl+Shift+Del)
├── Restart                           (Ctrl+R)
├── ─────────────────────────
├── Suspend Process                   
├── Resume Process                    
├── ─────────────────────────
├── Set Priority ►
│   ├── Realtime
│   ├── High
│   ├── Above Normal
│   ├── Normal              ✓
│   ├── Below Normal
│   └── Idle
├── Set Affinity...                   (opens core selector dialog)
├── Set I/O Priority ►
│   ├── Critical
│   ├── High
│   ├── Normal               ✓
│   ├── Low
│   └── Very Low
├── ─────────────────────────
├── Create Dump File ►
│   ├── Mini Dump
│   └── Full Dump
├── Analyze Wait Chain...
├── ─────────────────────────
├── Open File Location
├── Open Properties
├── Copy ►
│   ├── Process Name
│   ├── PID
│   ├── Command Line
│   ├── Full Path
│   └── All Columns (TSV)
├── ─────────────────────────
├── Search Online
├── View in Process Tree
├── Show Threads                      (expands inline thread list)
├── Show Open Handles                 (expands inline handle list)
├── Show Loaded Modules               (expands inline DLL/SO list)
├── Show TCP/IP Connections
├── Show GPU Details
├── ─────────────────────────
├── Debug ►
│   ├── Attach Debugger
│   └── Generate Stack Trace
└── UAC: Run as Administrator
```

### 4.6 Inline Expansions

Each process row can be expanded to show sub-tables:

#### 4.6.1 Threads Sub-Table

| Column | Description |
|--------|-------------|
| TID | Thread ID |
| State | Running / Waiting / Suspended |
| CPU % | Per-thread CPU usage |
| CPU Time | Accumulated CPU time |
| Priority | Thread priority |
| Start Address | Entry point function name (if symbols available) |
| Wait Reason | Executive / FreePage / PageIn / PoolAllocation / ... |
| Ideal Processor | Preferred CPU core |
| Stack Size | Thread stack size |

#### 4.6.2 Handles Sub-Table

| Column | Description |
|--------|-------------|
| Handle | Handle value (hex) |
| Type | File / Key / Event / Mutex / Section / Semaphore / Thread / Process / Token / ... |
| Name | Object name / path |
| Access | Access flags |
| Action | [Close Handle] button (with confirmation) |

#### 4.6.3 Modules Sub-Table

| Column | Description |
|--------|-------------|
| Name | Module / DLL / SO filename |
| Path | Full path |
| Base Address | Load address |
| Size | Memory footprint |
| Version | File version |
| Publisher | Digital signature publisher |
| Description | Module description |

#### 4.6.4 Connections Sub-Table

| Column | Description |
|--------|-------------|
| Protocol | TCP / UDP / TCP6 / UDP6 |
| Local Address | IP:Port |
| Remote Address | IP:Port (or *:*) |
| State | ESTABLISHED / LISTEN / TIME_WAIT / CLOSE_WAIT / ... |
| Bytes Sent | Total bytes sent on this connection |
| Bytes Received | Total bytes received |

### 4.7 Search & Filter

- **Search bar** (Ctrl+F): Free-text search across name, PID, command line, path, user.
- **Quick filters**: Toggle buttons for Apps / Background / System / Elevated / Not Responding.
- **Advanced filter dialog**: Boolean expressions on any column (e.g., `cpu_percent > 10 AND user = "admin"`).
- **Save/Load filter presets**: Named filter sets stored in config.

### 4.8 Heat Map Overlays

Optional visual mode where CPU%, Memory, Disk, GPU, Network columns use background color gradients:

| Range | Color |
|-------|-------|
| 0–10% | Transparent |
| 10–30% | Light Blue |
| 30–50% | Light Yellow |
| 50–70% | Orange |
| 70–90% | Light Red |
| 90–100% | Bright Red |

Configurable via Settings → Appearance → Heat Map.

See also: [Tab: Network Traffic](#14-tab-network-traffic) for deep network analysis, [Tab: Energy & Power](#15-tab-energy--power) for detailed power/thermal data, [Tab: Audio](#16-tab-audio) for audio device management.

---

## 5. Tab: Performance

### 5.1 Overview

Full-screen performance dashboard with real-time graphs and detailed hardware statistics. Left sidebar shows resource categories; main area shows the selected resource's graph(s) and stats.

### 5.2 Layout

```
┌──────────────┬───────────────────────────────────────────────┐
│              │                                               │
│  [CPU]       │   <<< Selected Resource Graph >>>             │
│  [Memory]    │                                               │
│  [Disk 0]    │   ┌─────────────────────────────────────┐     │
│  [Disk 1]    │   │  ████████████                       │     │
│  [GPU 0]     │   │  █████████████████                  │     │
│  [GPU 1]     │   │  ██████████████████████████         │     │
│  [Network]   │   │  ████████████████████████████████   │     │
│  [Power]     │   └─────────────────────────────────────┘     │
│  [Bluetooth] │                                               │
│  [Audio]     │   <<< Detailed Statistics Panel >>>           │
│              │                                               │
└──────────────┴───────────────────────────────────────────────┘
```

### 5.3 CPU Performance

#### 5.3.1 Graph Options

| Graph Type | Description |
|------------|-------------|
| **Overall Utilization** | Single line, 0–100%, all cores aggregated |
| **Per-Core Utilization** | Grid of mini-graphs, one per logical core |
| **Per-Core + Frequency** | Dual-axis: utilization + clock speed per core |
| **NUMA Node View** | Grouped by NUMA node |
| **Kernel vs User** | Stacked area: user-mode (blue) + kernel-mode (red) |
| **Core Heatmap** | Grid with color-coded intensity per core |

Right-click graph → "Change graph to" → select type.

#### 5.3.2 CPU Statistics Panel

| Statistic | Description |
|-----------|-------------|
| Utilization | Current % (overall) |
| Speed | Current base clock (GHz) |
| Effective Speed | Current boost clock (GHz) |
| Base Speed | Rated base frequency |
| Max Boost | Maximum turbo/boost frequency |
| Sockets | Physical CPU socket count |
| Physical Cores | Core count |
| Logical Processors | Thread count (with HT/SMT) |
| Virtualization | Enabled / Disabled |
| L1 Cache | Size (per-core and total) |
| L2 Cache | Size (per-core and total) |
| L3 Cache | Size (shared) |
| Architecture | x86_64 / ARM64 / RISC-V |
| Up Time | System uptime |
| Processes | Total process count |
| Threads | Total thread count |
| Handles | Total handle count |
| Interrupts / sec | Current interrupt rate |
| DPCs / sec | Deferred procedure call rate |
| System Calls / sec | Syscall rate |
| Context Switches / sec | Total context switch rate |
| CPU Temperature | Per-core and package temperature (°C) |
| CPU Power Draw | Package power (W) |
| CPU Voltage | Core voltage (V) |
| Throttling | Yes/No + reason (thermal/power/current) |
| C-State Residency | % time in C0/C1/C3/C6/C7/C10 states |
| Instructions Retired / sec | IPC metric (where HW counters available) |
| Branch Prediction Miss Rate | % (where HW counters available) |
| Cache Miss Rate | L1/L2/L3 miss rates (where HW counters available) |

#### 5.3.3 CPU Graph Overlays

Optional overlay lines on the CPU graph:

- Clock Speed (right Y-axis, GHz)
- Temperature (right Y-axis, °C)
- Power Draw (right Y-axis, W)
- Thread Count
- Process Count

### 5.4 Memory Performance

#### 5.4.1 Graph

- **Memory Composition**: Stacked area showing In Use / Modified / Standby / Free
- **Commit Charge**: Line graph showing committed vs. limit
- **Page Faults**: Line graph of hard + soft faults/sec

#### 5.4.2 Memory Statistics Panel

| Statistic | Description |
|-----------|-------------|
| In Use | Physical RAM in use |
| Available | Available physical RAM |
| Committed | Virtual memory committed (used / limit) |
| Cached | Standby + modified pages |
| Paged Pool | Kernel paged pool size |
| Non-Paged Pool | Kernel non-paged pool size |
| Total | Installed RAM |
| Speed | Memory clock speed (MHz) |
| Effective Speed | Effective data rate (MT/s) |
| Slots Used | e.g., 2 of 4 |
| Form Factor | DIMM / SO-DIMM |
| Type | DDR4 / DDR5 / LPDDR5 |
| Channel Config | Single / Dual / Quad channel |
| Hardware Reserved | Memory reserved by firmware |
| NUMA Nodes | Memory split across NUMA nodes |
| ECC | Enabled / Disabled / Not Supported |
| Page File Usage | Current / Max size for each page/swap file |
| Compression Ratio | Memory compression ratio (if applicable) |
| Compressed Size | Amount of compressed memory |

### 5.5 Disk Performance

#### 5.5.1 Graphs (per physical disk)

| Graph | Description |
|-------|-------------|
| **Active Time %** | Disk busy percentage |
| **Transfer Rate** | Read (blue) + Write (red) in MB/s |
| **IOPS** | Read + Write operations per second |
| **Queue Depth** | Average disk queue length |
| **Latency** | Average read/write latency (ms) |

#### 5.5.2 Disk Statistics Panel

| Statistic | Description |
|-----------|-------------|
| Active Time | % disk is actively servicing requests |
| Average Response Time | Mean I/O latency (ms) |
| Read Speed | Current read throughput |
| Write Speed | Current write throughput |
| Read IOPS | Current read operations/sec |
| Write IOPS | Current write operations/sec |
| Queue Depth | Current I/O queue length |
| Capacity | Total disk size |
| Free Space | Available free space |
| Formatted | Total formatted capacity |
| Type | SSD / HDD / NVMe / USB / Network |
| Interface | NVMe / SATA / USB 3.2 / Thunderbolt |
| Model | Drive model string |
| Firmware | Firmware version |
| Serial Number | Drive serial |
| Partitions | Count and layout |
| File System | NTFS / ext4 / btrfs / APFS / ... |
| S.M.A.R.T. Status | Healthy / Warning / Critical |
| Temperature | Drive temperature (°C) |
| Power On Hours | Total lifetime hours |
| Total Bytes Read | Lifetime bytes read |
| Total Bytes Written | Lifetime bytes written (TBW for SSD wear) |
| Wear Leveling | SSD wear percentage remaining |
| TRIM Support | Enabled / Disabled / Not Supported |
| Write Cache | Enabled / Disabled |
| NCQ Depth | Maximum native command queue depth |

### 5.6 GPU / Graphics Performance

#### 5.6.1 Graphs (per GPU adapter)

| Graph | Description |
|-------|-------------|
| **GPU Overall** | Combined engine utilization |
| **3D Engine** | 3D rendering pipeline usage % |
| **Copy Engine** | DMA/copy engine usage % |
| **Video Decode** | Hardware video decode usage % |
| **Video Encode** | Hardware video encode usage % |
| **Compute** | GPU compute / GPGPU usage % |
| **VRAM Usage** | Dedicated + Shared stacked area |
| **GPU Temperature** | Temperature line graph |
| **GPU Clock** | Core + Memory clock line graph |
| **GPU Power** | Power draw (W) line graph |
| **Fan Speed** | RPM or % line graph |
| **Frame Time** | Per-frame timing histogram (for active 3D apps) |

#### 5.6.2 GPU Statistics Panel

| Statistic | Description |
|-----------|-------------|
| GPU Utilization | Overall % |
| 3D Utilization | 3D engine % |
| Copy Utilization | Copy engine % |
| Video Decode | Decode engine % |
| Video Encode | Encode engine % |
| Compute | Compute engine % |
| Dedicated GPU Memory | Used / Total (VRAM) |
| Shared GPU Memory | System RAM shared with GPU |
| GPU Temperature | Current (°C) |
| GPU Hotspot Temp | Hotspot junction temperature (°C) |
| GPU Memory Temp | VRAM temperature (°C) |
| GPU Core Clock | Current (MHz) |
| GPU Memory Clock | Current (MHz) |
| GPU Boost Clock | Maximum boost clock (MHz) |
| GPU Base Clock | Base clock (MHz) |
| GPU Power Draw | Current (W) |
| GPU TDP | Thermal design power |
| GPU Voltage | Core voltage (V) |
| Fan Speed | RPM and % |
| PCIe Link | Gen + Width (e.g., Gen4 x16) |
| PCIe Bandwidth | Current throughput |
| Driver Version | GPU driver version |
| DirectX Version | Supported DX feature level |
| Vulkan Version | Supported Vulkan version |
| OpenGL Version | Supported OpenGL version |
| OpenCL Version | Supported OpenCL version |
| CUDA Cores / Stream Processors | Core count |
| Ray Tracing Cores | RT core count (if present) |
| Tensor Cores | Tensor/AI core count (if present) |
| Encoder Sessions | Active HW encode sessions |
| Decoder Sessions | Active HW decode sessions |
| Adapter Name | GPU model name |
| Manufacturer | NVIDIA / AMD / Intel / ... |
| BIOS Version | GPU BIOS/firmware version |
| Resizable BAR | Enabled / Disabled |
| Multi-GPU Mode | SLI / CrossFire / None |

#### 5.6.3 Per-Process GPU Breakdown

Below the GPU stats, a mini-table shows top GPU consumers:

| Process | GPU % | VRAM | Engine | FPS |
|---------|-------|------|--------|-----|

### 5.7 Network Performance

#### 5.7.1 Graphs (per adapter)

| Graph | Description |
|-------|-------------|
| **Throughput** | Send (blue) + Receive (orange) |
| **Packets** | Packets/sec send + receive |
| **Connections** | Active TCP connection count |
| **Latency** | Round-trip time to gateway |

#### 5.7.2 Network Statistics Panel

| Statistic | Description |
|-----------|-------------|
| Send | Current send rate |
| Receive | Current receive rate |
| Link Speed | Negotiated link speed |
| Adapter Name | Network adapter name |
| Connection Type | Ethernet / Wi-Fi / Cellular / VPN / Loopback |
| IPv4 Address | Current IPv4 |
| IPv6 Address | Current IPv6 |
| DNS Servers | Configured DNS |
| Gateway | Default gateway |
| SSID | Wi-Fi network name (if applicable) |
| Signal Strength | Wi-Fi signal (dBm / bars) |
| Channel | Wi-Fi channel |
| Frequency | 2.4 GHz / 5 GHz / 6 GHz |
| Security | WPA3 / WPA2 / Open / ... |
| Packets Sent | Total packets sent |
| Packets Received | Total packets received |
| Packets Lost | Dropped packet count |
| Errors In | Inbound errors |
| Errors Out | Outbound errors |
| Unicast / Broadcast / Multicast | Packet type breakdown |
| Bytes Sent (Total) | Lifetime bytes sent |
| Bytes Received (Total) | Lifetime bytes received |
| TCP Connections | Open TCP connections |
| UDP Listeners | Open UDP listen ports |

### 5.8 Power / Battery Performance

#### 5.8.1 Graphs

| Graph | Description |
|-------|-------------|
| **Battery Level** | % over time |
| **Power Draw** | System wattage (W) over time |
| **Discharge Rate** | mW discharge rate |
| **Estimated Remaining** | Time remaining projection |

#### 5.8.2 Power Statistics

| Statistic | Description |
|-----------|-------------|
| Power Source | AC / Battery / UPS |
| Battery Level | Current % |
| Battery Health | Design capacity vs. current full charge capacity |
| Charge Rate | Watts being charged at |
| Discharge Rate | Current drain (mW) |
| Estimated Remaining | Time estimate |
| Cycle Count | Battery charge cycles |
| Design Capacity | Original Wh |
| Full Charge Capacity | Current max Wh |
| Voltage | Current battery voltage |
| Temperature | Battery temperature (°C) |
| Power Plan | Active power profile |

### 5.9 Audio Performance

| Statistic | Description |
|-----------|-------------|
| Active Output Device | Name + sample rate + bit depth |
| Active Input Device | Name + sample rate + bit depth |
| Output Level | VU meter (peak, RMS) |
| Input Level | VU meter (peak, RMS) |
| Latency | Audio pipeline latency (ms) |
| Buffer Size | Audio buffer (samples) |
| Sample Rate | Hz |
| Bit Depth | 16/24/32 bit |
| Channels | Mono/Stereo/5.1/7.1 |
| Exclusive Mode | Yes/No |
| Active Streams | Count of audio-producing processes |

### 5.10 Graph Controls

All graphs support:

| Control | Description |
|---------|-------------|
| Time Range | 60s / 5m / 15m / 1h / 6h / 24h / All |
| Pause / Resume | Freeze graph updates |
| Zoom | Mouse wheel zoom on time axis |
| Tooltip | Hover for exact value at timestamp |
| Legend | Toggle individual series on/off |
| Scale | Linear / Logarithmic Y-axis |
| Export | Copy graph image / Export CSV data |
| Overlay | Add comparison overlays from other metrics |
| Markers | Add manual annotation markers on the timeline |
| Grid | Toggle background grid lines |
| Smooth | Toggle line smoothing (Bézier vs. linear) |
| Fill | Toggle area fill under line |
| Stacked | Toggle stacked vs. overlaid for multi-series |

---

## 6. Tab: App History

### 6.1 Overview

Historical resource consumption per application over configurable time periods (default: since last reboot, up to 30 days with persistent storage).

### 6.2 Columns

| Column | Description |
|--------|-------------|
| Name | Application name |
| Publisher | Developer / publisher |
| CPU Time | Total accumulated CPU time |
| GPU Time | Total accumulated GPU time |
| Network (Metered) | Bytes transferred on metered connections |
| Network (Non-Metered) | Bytes transferred on unmetered connections |
| Network (Total) | Total network usage |
| Disk Read (Total) | Total bytes read |
| Disk Write (Total) | Total bytes written |
| Memory (Peak) | Peak memory usage observed |
| Memory (Average) | Mean memory usage |
| Launch Count | Number of times launched |
| Total Foreground Time | Time spent as foreground app |
| Total Background Time | Time spent in background |
| Last Used | Timestamp of last execution |
| Battery Used (mWh) | Estimated battery consumption |
| Crash Count | Number of crashes observed |
| Hang Count | Number of not-responding incidents |
| Tile Updates | Count of live tile / notification updates |

### 6.3 Features

- **Time Period Selector**: Today / Last 7 Days / Last 30 Days / Custom Range
- **Sort** by any column
- **Delete History** for individual apps or all
- **Export** history as CSV / JSON
- **Compare**: Select two apps and show side-by-side resource comparison chart
- **Trend Graphs**: Click an app to see sparkline trends for each metric over the selected period

---

## 7. Tab: Startup (Boot)

### 7.1 Overview

Manage applications and services that run at system boot or user login.

### 7.2 Columns

| Column | Description |
|--------|-------------|
| Name | Application / service friendly name |
| Publisher | Developer |
| Status | Enabled / Disabled |
| Startup Impact | None / Low / Medium / High / Not Measured |
| Startup Type | Registry / Startup Folder / Scheduled Task / Service / Login Script / Shell Extension |
| CPU at Startup | CPU time consumed during boot (ms) |
| Disk at Startup | Disk I/O during boot (MB) |
| Boot Delay | Estimated delay added to boot (ms) |
| Last Boot Time | When it last ran during boot |
| Command Line | Full command line executed at startup |
| File Path | Executable path |
| Registry Key | Registry location (if applicable) |
| Run As | User context (Current User / All Users / SYSTEM) |
| Digital Signature | Signed / Unsigned / Invalid |
| Hash | SHA-256 of the executable |

### 7.3 Context Menu

```
├── Enable
├── Disable
├── ─────────────────
├── Open File Location
├── Open Registry Key
├── View Properties
├── Delete Startup Entry
├── ─────────────────
├── Search Online
├── Check VirusTotal
├── Copy Command Line
├── ─────────────────
├── Create Delayed Start (30s / 60s / 120s)
├── Set Startup Order Priority
└── Measure Boot Impact (re-time on next boot)
```

### 7.4 Boot Timeline

A graphical timeline view showing:
- BIOS/UEFI initialization duration
- Bootloader duration
- Kernel initialization
- Service startup waterfall (parallel lanes showing each service starting)
- User login to desktop-ready time
- Application startup waterfall after login

Each step is color-coded by duration:
- Green: < expected
- Yellow: slow
- Red: bottleneck

### 7.5 Boot History

| Column | Description |
|--------|-------------|
| Date | Boot timestamp |
| BIOS Time | Firmware init duration |
| Boot Time | Kernel + service start duration |
| Login Time | User login to desktop ready |
| Total Boot | Total wall-clock time |
| Shutdown Type | Clean / BSOD / Power Loss / Hibernate / Sleep |
| Services Started | Count of services started |
| Startup Apps | Count of startup apps that ran |
| Errors | Any services or apps that failed to start |

---

## 8. Tab: Users & Sessions

### 8.1 Overview

Shows all logged-in users and their session details, with the ability to manage sessions.

### 8.2 Session List

| Column | Description |
|--------|-------------|
| User | Username |
| Session ID | Numeric session ID |
| Session Type | Console / RDP / SSH / VNC / TTY / Wayland / X11 |
| Status | Active / Disconnected / Locked / Idle |
| Client Name | Remote client hostname (if remote) |
| Client IP | Remote IP address |
| Login Time | When the session started |
| Idle Time | Duration idle |
| CPU % | Total CPU usage by all processes in this session |
| Memory | Total memory usage by all processes |
| Disk | Aggregate disk I/O |
| Network | Aggregate network I/O |
| Process Count | Number of processes in this session |

### 8.3 Per-User Process List

Expanding a user row shows all processes belonging to that session (same columns as Processes tab).

### 8.4 Context Menu

```
├── Connect
├── Disconnect
├── Log Off
├── Send Message...
├── ─────────────────
├── Switch to this User
├── Lock Session
├── ─────────────────
├── View Process Details
├── Show Resource Usage Graph
└── Remote Shadow (observe session)
```

### 8.5 Login History

A searchable log of all login/logout events:

| Column | Description |
|--------|-------------|
| Timestamp | Event time |
| User | Username |
| Event | Login / Logout / Lock / Unlock / Failed Login / Session Connect / Session Disconnect |
| Session Type | Console / RDP / SSH / ... |
| Source IP | Remote IP (if applicable) |
| Duration | Session duration (for logout events) |
| Failure Reason | Bad password / Account locked / Certificate error / ... |

---

## 9. Tab: Services

### 9.1 Overview

Complete service management interface showing all system services (systemd units / Windows services / launchd plists).

### 9.2 Columns

| Column | Description |
|--------|-------------|
| Name | Service name (short name) |
| Display Name | Friendly display name |
| Status | Running / Stopped / Starting / Stopping / Paused / Error / Degraded |
| Startup Type | Automatic / Automatic (Delayed) / Manual / Disabled / Triggered |
| PID | Process ID (if running) |
| CPU % | Current CPU usage |
| Memory | Current memory usage |
| User / Account | Service account (LocalSystem / NetworkService / root / custom) |
| Description | Service description |
| Dependencies | List of required services |
| Dependents | Services that depend on this one |
| Group | Load ordering group |
| Log On As | Account the service runs under |
| Recovery Actions | First/Second/Subsequent failure actions |
| Path | Executable path |
| Command Line | Full startup command with arguments |
| Type | Own Process / Share Process / Kernel Driver / File System / Interactive |
| Error Control | Ignore / Normal / Severe / Critical |
| Last Start Time | When the service last started |
| Restart Count | Number of auto-restarts |
| Exit Code | Last exit code (if stopped) |
| Binary Hash | SHA-256 of the service binary |
| Signed | Digital signature status |

### 9.3 Context Menu

```
├── Start
├── Stop
├── Restart
├── Pause / Continue
├── ─────────────────
├── Properties... (opens detailed dialog)
│   ├── General tab (name, path, startup type, status)
│   ├── Log On tab (account configuration)
│   ├── Recovery tab (failure actions: restart/reboot/run program/take no action)
│   ├── Dependencies tab (visual dependency tree)
│   └── Security tab (DACL editor)
├── ─────────────────
├── Set Startup Type ►
│   ├── Automatic
│   ├── Automatic (Delayed)
│   ├── Manual
│   └── Disabled
├── ─────────────────
├── Open File Location
├── View in Processes Tab
├── View Logs (last 100 log entries)
├── ─────────────────
├── Export Service Config
├── Copy Service Name
└── Search Online
```

### 9.4 Service Dependency Viewer

A visual directed-graph showing service dependency relationships. Nodes are services; edges show dependencies. Color-coded by status (green = running, red = stopped). Interactive—click to select, double-click to view properties.

---

## 10. Tab: Devices

### 10.1 Overview

Hardware device inventory and status monitoring, similar to Device Manager plus real-time performance data.

### 10.2 View Modes

| View | Description |
|------|-------------|
| **By Type** | USB / PCI / Display / Audio / Network / Storage / Input / Bluetooth / ... |
| **By Connection** | Tree view showing physical connection topology |
| **By Status** | Working / Error / Disabled / Unknown |
| **Flat List** | All devices in a single table |

### 10.3 Columns

| Column | Description |
|--------|-------------|
| Device Name | Friendly name |
| Category | Device class |
| Status | OK / Error / Disabled / Not Started / Driver Error |
| Manufacturer | Hardware manufacturer |
| Driver | Driver name and version |
| Driver Date | Driver release date |
| Driver Provider | Who signed the driver |
| Location | Bus / Slot / Port path |
| Hardware ID | PnP hardware identifier |
| Compatible IDs | Alternate identifiers |
| Instance Path | Unique device instance path |
| Power State | D0 (On) / D1–D3 (Sleep states) |
| Power Usage | Estimated power draw |
| IRQ | Interrupt request line (if applicable) |
| I/O Ports | I/O port range |
| Memory Range | MMIO range |
| DMA Channel | DMA channel (if applicable) |
| Bus Type | PCI / USB / Thunderbolt / Bluetooth / I2C / SPI |
| Bus Speed | Negotiated bus speed |
| Firmware Version | Device firmware |
| Serial Number | Device serial |

### 10.4 Context Menu

```
├── Enable Device
├── Disable Device
├── Uninstall Device
├── Update Driver
├── Roll Back Driver
├── ─────────────────
├── Properties...
├── View Driver Files
├── View Events (device-specific log entries)
├── ─────────────────
├── Safely Remove (for removable devices)
├── Eject Media
├── Power Cycle USB Port
├── ─────────────────
├── Scan for Hardware Changes
├── Copy Hardware ID
└── Export Device Report
```

### 10.5 USB Device Tree

Special expanded view for USB showing:
- Controller → Hub → Device hierarchy
- Speed negotiation (USB 2.0 / 3.0 / 3.2 / 4.0)
- Power draw per port
- Bandwidth utilization per controller

### 10.6 Bluetooth Devices

| Column | Description |
|--------|-------------|
| Name | Device name |
| Type | Audio / Input / Display / File Transfer / ... |
| Status | Connected / Paired / Disconnected |
| Signal Strength | RSSI (dBm) |
| Battery | Remote device battery % (if reported) |
| Protocol | BLE / BR/EDR / Dual |
| Profile | A2DP / HFP / HID / ... |
| Firmware | Remote firmware version |

---

## 11. Tab: Files & Folders In Use

### 11.1 Overview

Shows all currently open file handles system-wide, which processes hold them, and lock status.

### 11.2 Columns

| Column | Description |
|--------|-------------|
| File / Folder Path | Full path of the open resource |
| Type | File / Directory / Named Pipe / Mailslot / Device |
| Process Name | Process holding the handle |
| PID | Process ID |
| Handle Type | Read / Write / Read-Write / Delete / Execute |
| Lock Type | None / Shared / Exclusive |
| Lock Range | Byte range locked (if applicable) |
| Access Flags | Detailed access mask |
| Share Mode | ShareRead / ShareWrite / ShareDelete / Exclusive |
| Open Since | When the handle was opened |
| Bytes Read | Bytes read through this handle |
| Bytes Written | Bytes written through this handle |
| File Size | Size of the file |
| Network Path | UNC path (if network file) |

### 11.3 Features

- **Search**: Filter by path, PID, or process name
- **Highlight Conflicts**: Show files held by multiple processes with conflicting locks
- **Group by**: File, Process, Lock Type, or Folder
- **Folder Statistics**: Show aggregate open file counts per directory tree
- **Path Watch**: Add specific paths to a watch list for real-time notifications when accessed

### 11.4 Context Menu

```
├── Close Handle (with confirmation + warning)
├── ─────────────────
├── Go to Process (switch to Processes tab)
├── Open File Location
├── Open File Properties
├── ─────────────────
├── Copy Path
├── Copy All Handle Info
├── ─────────────────
├── Add to Watch List
├── Show All Handles for This File
├── Show All Handles for This Process
└── Export Open Files Report
```

---

## 12. Tab: Resource Unlocking

### 12.1 Overview

Dedicated tool for finding and releasing locked resources. Combines the functionality of tools like Handle.exe / lsof / Unlocker.

### 12.2 Workflow

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  🔍 Enter file/folder path to check:                           │
│  ┌───────────────────────────────────────────┐  [Browse] [Find] │
│  │ C:\Users\demo\Documents\report.docx       │                  │
│  └───────────────────────────────────────────┘                  │
│                                                                 │
│  Results:                                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Process       PID    Handle   Access      Lock     Action   ││
│  │─────────────────────────────────────────────────────────────││
│  │ Word.exe      4521   0x1A4    ReadWrite   Excl.   [Unlock] ││
│  │ SearchIdx     1102   0x38C    Read        Shared  [Unlock] ││
│  │ Backup.exe    8844   0x0F2    Read        Shared  [Unlock] ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                 │
│  [Unlock All]  [Unlock Selected]  [Copy Report]  [Kill Process] │
│                                                                 │
│  ⚠ Warning: Forcibly unlocking handles may cause data loss     │
│    in the affected applications.                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 12.3 Unlock Operations

| Operation | Description |
|-----------|-------------|
| **Close Handle** | Forcibly close the file handle (DuplicateHandle + CloseHandle technique) |
| **Suspend + Close** | Suspend the process, close the handle, optionally resume |
| **Terminate Process** | Kill the holding process entirely |
| **Kill Process Tree** | Kill the process and all children |
| **Rename Lock** | Rename the locked file by changing the file name in the directory entry (advanced) |
| **Volume Shadow Copy** | Access the file via VSS snapshot (read-only, no unlock needed) |
| **Schedule on Reboot** | Queue the operation (delete/move/rename) for next reboot |

### 12.4 Batch Operations

- **Unlock Folder**: Recursively find and unlock all handles under a directory tree.
- **Unlock by Process**: Close all handles held by a specific process.
- **Unlock by Pattern**: Match file paths using glob patterns (e.g., `*.log`).

### 12.5 Safety Features

| Feature | Description |
|---------|-------------|
| Confirmation Dialog | Always prompt before closing handles |
| System Process Warning | Extra warning for system-critical processes |
| Audit Log | Log all unlock operations with timestamp, user, process, path, result |
| Undo Hint | Show "The application may need to be restarted" after unlock |
| Dry Run | Preview what would be unlocked without actually doing it |
| Process Snapshot | Take a mini-dump before forcible unlock (optional) |

---

## 13. Tab: Process Tree (Roots)

### 13.1 Overview

Visualizes the full process hierarchy from init/PID 1/System down to every leaf process.

### 13.2 Tree View

```
├─ System (PID 4)
│  ├─ smss.exe (PID 452)
│  │  └─ csrss.exe (PID 630)
│  ├─ Memory Compression (PID 1840)
│  └─ Registry (PID 100)
├─ wininit.exe (PID 644)
│  ├─ services.exe (PID 732)
│  │  ├─ svchost.exe (PID 900) [DcomLaunch, PlugPlay, Power]
│  │  ├─ svchost.exe (PID 988) [RpcSs]
│  │  ├─ svchost.exe (PID 1200) [NetworkService]
│  │  └─ ...
│  └─ lsass.exe (PID 748)
├─ winlogon.exe (PID 660)
│  ├─ dwm.exe (PID 1032)
│  └─ explorer.exe (PID 3540)
│     ├─ firefox.exe (PID 4200)
│     │  ├─ firefox.exe (PID 4280) [GPU]
│     │  ├─ firefox.exe (PID 4312) [Content]
│     │  └─ firefox.exe (PID 4340) [Content]
│     ├─ code.exe (PID 5600)
│     │  ├─ code.exe (PID 5680) [GPU Helper]
│     │  └─ code.exe (PID 5712) [Extension Host]
│     └─ terminal.exe (PID 6100)
│        └─ bash.exe (PID 6200)
│           └─ cargo (PID 6280)
```

### 13.3 Tree Columns

| Column | Description |
|--------|-------------|
| Process | Name with tree indentation |
| PID | Process ID |
| CPU % | Current CPU usage |
| Memory | Working set |
| Disk | Combined I/O rate |
| GPU % | GPU usage |
| User | Owning user |
| Start Time | When started |
| Command Line | Full command line |
| Annotations | Service names, process role, package name |

### 13.4 Tree Features

- **Collapse / Expand** all or individual subtrees (click or keyboard)
- **Highlight Subtree**: Click a node to highlight its entire subtree
- **Subtree Totals**: Show aggregate CPU/RAM/Disk for collapsed subtree
- **Kill Subtree**: Right-click → Kill this process and all descendants
- **Reparent View**: Option to reroot the tree at any chosen process
- **Orphan Detection**: Highlight processes whose parent has exited (orphans reparented to init)
- **Search in Tree**: Highlight all matching nodes and expand their ancestors
- **Color Coding**: Color nodes by CPU intensity / memory / type
- **Mini-map**: Scrollable overview of the entire tree for large process counts
- **Graphical DAG View**: Optional force-directed graph layout (alternative to indented tree)
- **Process Ancestry**: Right-click → Show Ancestors (path from this process to root)

### 13.5 Timeline View

Optional horizontal timeline showing process lifetimes:

```
Time →  0s        30s       60s       90s      120s
PID 4200 ████████████████████████████████████████████
PID 4280  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
PID 4312   ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
PID 6280                          ░░░░░░░░░░░░░░░░░
```

---

## 14. Tab: Network Traffic

### 14.1 Overview

A comprehensive network traffic monitoring and analysis tab providing deep visibility into all network activity across every adapter, connection, process, and protocol. Goes far beyond the basic network stats in the Performance tab by offering per-connection inspection, DNS query logging, firewall rule visibility, traffic classification, bandwidth allocation, and historical analysis.

### 14.2 Layout

```
┌───────────────┬──────────────────────────────────────────────────┐
│               │                                                  │
│ [Overview]    │  <<< Selected View Content >>>                   │
│ [Connections] │                                                  │
│ [Per-Process] │  ┌────────────────────────────────────────────┐  │
│ [DNS]         │  │  Real-time traffic graph / table / map     │  │
│ [Protocols]   │  │                                            │  │
│ [Firewall]    │  └────────────────────────────────────────────┘  │
│ [Bandwidth]   │                                                  │
│ [Interfaces]  │  <<< Detail Panel >>>                            │
│ [Traffic Map] │                                                  │
│ [Captures]    │                                                  │
│               │                                                  │
└───────────────┴──────────────────────────────────────────────────┘
```

### 14.3 Overview Dashboard

A summary view showing aggregate network activity at a glance:

| Widget | Description |
|--------|-------------|
| **Total Throughput** | Combined upload + download sparkline graph |
| **Active Connections** | Count of open TCP/UDP connections |
| **Top Talkers** | Top 5 processes by bandwidth (bar chart) |
| **Protocol Breakdown** | Pie chart: HTTP/HTTPS/DNS/SSH/Other |
| **Packet Rate** | Packets/sec send + receive |
| **Error Rate** | Dropped, retransmitted, errored packets/sec |
| **Connection Rate** | New connections / closures per second |
| **Latency** | Round-trip time to default gateway and configured targets |
| **External IP** | Detected public IP address (with geolocation) |
| **VPN Status** | Active VPN tunnel info (if applicable) |
| **DNS Resolution Rate** | Queries/sec + avg resolution time |
| **Bandwidth Quota** | Usage against quota (if metered connection configured) |

### 14.4 Connections View

#### 14.4.1 Connection Table

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Process | `conn_process` | Owning process name + PID | ★ |
| Protocol | `conn_proto` | TCP / TCP6 / UDP / UDP6 / SCTP / QUIC | ★ |
| State | `conn_state` | LISTEN / ESTABLISHED / TIME_WAIT / CLOSE_WAIT / SYN_SENT / SYN_RECV / FIN_WAIT / CLOSING / LAST_ACK | ★ |
| Local Address | `conn_local` | Local IP:Port | ★ |
| Local Port Name | `conn_local_svc` | Resolved service name (e.g., "https", "ssh") | |
| Remote Address | `conn_remote` | Remote IP:Port | ★ |
| Remote Hostname | `conn_remote_host` | Reverse DNS of remote IP | |
| Remote Geo | `conn_remote_geo` | Country / City of remote IP (offline GeoIP DB) | |
| Remote ASN | `conn_remote_asn` | Autonomous System name and number | |
| Send Rate | `conn_send_rate` | Current bytes/sec being sent | ★ |
| Recv Rate | `conn_recv_rate` | Current bytes/sec being received | ★ |
| Bytes Sent | `conn_bytes_sent` | Total bytes sent on this connection | |
| Bytes Received | `conn_bytes_recv` | Total bytes received on this connection | |
| Packets Sent | `conn_pkts_sent` | Total packets sent | |
| Packets Recv | `conn_pkts_recv` | Total packets received | |
| Retransmits | `conn_retrans` | TCP retransmission count | |
| RTT | `conn_rtt` | Smoothed round-trip time (ms) | |
| RTT Variance | `conn_rtt_var` | RTT variance (ms) | |
| Window Size | `conn_window` | TCP receive window (bytes) | |
| Congestion Window | `conn_cwnd` | TCP congestion window (segments) | |
| Congestion Algorithm | `conn_cc` | cubic / bbr / reno / vegas | |
| MSS | `conn_mss` | Maximum segment size (bytes) | |
| TTL | `conn_ttl` | IP time-to-live / hop limit | |
| TLS Version | `conn_tls` | TLS 1.2 / TLS 1.3 / None | |
| TLS Cipher | `conn_cipher` | Negotiated cipher suite | |
| TLS Certificate | `conn_cert` | Remote certificate subject/issuer (abbreviated) | |
| Interface | `conn_iface` | Network adapter used for this connection | |
| Connection Age | `conn_age` | Duration since connection opened | |
| Last Activity | `conn_last_act` | Time since last data transfer | |
| Socket Buffer Recv | `conn_rcvbuf` | Receive buffer size (bytes) | |
| Socket Buffer Send | `conn_sndbuf` | Send buffer size (bytes) | |

#### 14.4.2 Connection States Visual

```
                 ┌─────────┐
          ┌──────┤  CLOSED  ├──────┐
          │      └─────────┘      │
          ▼                       ▼
     ┌─────────┐           ┌──────────┐
     │  LISTEN │           │ SYN_SENT │
     └────┬────┘           └────┬─────┘
          │                     │
     ┌────▼─────┐         ┌────▼──────────┐
     │SYN_RECV  │         │ ESTABLISHED   │◄──┐
     └────┬─────┘         └────┬──────────┘   │
          │                    │              │
          └────────►───────────┘              │
                                             │
     ┌──────────┐  ┌──────────┐  ┌─────────┐│
     │ FIN_WAIT │→ │TIME_WAIT │→ │  CLOSED ││
     └──────────┘  └──────────┘  └─────────┘│
     ┌───────────┐ ┌──────────┐             │
     │CLOSE_WAIT │→│ LAST_ACK │─────────────┘
     └───────────┘ └──────────┘
```

Color-coded connection rows by state:
- 🟢 ESTABLISHED — active healthy connection
- 🔵 LISTEN — server waiting for connections
- 🟡 TIME_WAIT — connection closing (normal)
- 🟠 CLOSE_WAIT — remote side closed, waiting for local close
- 🔴 SYN_SENT — connection attempt in progress

#### 14.4.3 Connection Context Menu

```
├── Close Connection (TCP RST)
├── Block Remote Address (add firewall rule)
├── ─────────────────────────
├── Trace Route to Remote
├── Ping Remote
├── Reverse DNS Lookup
├── WHOIS Lookup
├── GeoIP Lookup
├── ─────────────────────────
├── Go to Process
├── Show All Connections for This Process
├── Show All Connections to This Remote
├── ─────────────────────────
├── Copy Local Address
├── Copy Remote Address
├── Copy All Connection Info
├── ─────────────────────────
├── Start Packet Capture for Connection
├── View TLS Certificate Details
├── Export Connection History
└── Add to Watch List
```

### 14.5 Per-Process Network View

Shows network usage aggregated per process:

| Column | Description |
|--------|-------------|
| Process | Name + PID + Icon |
| Total Send Rate | Aggregate outbound bandwidth |
| Total Recv Rate | Aggregate inbound bandwidth |
| Total Sent | Cumulative bytes sent (session) |
| Total Received | Cumulative bytes received (session) |
| Connection Count | Number of active connections |
| Listen Ports | Ports this process is listening on |
| DNS Queries | Count of DNS queries made |
| Protocol Mix | Breakdown bar: TCP/UDP/QUIC percentages |
| Remote Hosts | Count of unique remote endpoints |
| Retransmit Rate | Overall retransmission percentage |
| Average RTT | Mean round-trip time across connections |
| Bandwidth Limit | Configured limit (if set) |
| Classification | Streaming / Browsing / Download / Upload / Gaming / P2P / System / Background |

Context menu includes: Set Bandwidth Limit, Block Network Access, View Connections Detail, View DNS Queries, Export Usage Report.

### 14.6 DNS Query Monitor

Real-time log of all DNS resolutions:

| Column | Description |
|--------|-------------|
| Timestamp | When the query was made |
| Process | Requesting process name + PID |
| Query Name | Domain being resolved (e.g., `api.example.com`) |
| Query Type | A / AAAA / CNAME / MX / SRV / TXT / PTR / SOA |
| Response | Resolved IP address(es) or NXDOMAIN / SERVFAIL / REFUSED |
| TTL | DNS response TTL (seconds) |
| Response Time | Resolution latency (ms) |
| DNS Server | Which DNS server answered |
| Protocol | UDP / TCP / DoH / DoT |
| Source | Stub resolver / System cache / Application cache |
| DNSSEC | Validated / Not Validated / Bogus |
| Cached | Whether the result was from cache |
| Category | Domain category (if classification DB available): Ad / Tracker / CDN / Social / Search / Malware / ... |

Features:
- **Filter by domain**: Regex/glob filter on query names
- **Filter by process**: Show queries from specific process
- **Block domain**: Right-click → Add to blocklist (integrates with system DNS blocking)
- **Flush cache**: Button to flush DNS resolver cache
- **Top Domains**: Bar chart of most-queried domains
- **Query Rate Graph**: Queries/sec over time
- **Failure Rate**: Percentage of failed lookups
- **Export**: Full query log export (CSV/JSON)

### 14.7 Protocol Analysis

Breakdown of traffic by network protocol:

#### 14.7.1 Protocol Hierarchy

```
├─ IPv4 ─── 78% ████████████████████████████████████ 
│  ├─ TCP ── 72% ██████████████████████████████████
│  │  ├─ HTTPS (443) ── 58% ███████████████████████████
│  │  ├─ HTTP (80) ──── 6%  ███
│  │  ├─ SSH (22) ───── 3%  ██
│  │  ├─ SMTP (25) ──── 1%  █
│  │  └─ Other ──────── 4%  ██
│  ├─ UDP ── 5%  ███
│  │  ├─ DNS (53) ───── 2%  █
│  │  ├─ NTP (123) ──── 0.5%
│  │  ├─ QUIC (443) ─── 1.5% █
│  │  └─ DHCP ───────── 0.1%
│  └─ ICMP ─ 1%  █
├─ IPv6 ─── 22% ████████████
│  ├─ TCP6 ─ 18% █████████
│  ├─ UDP6 ─ 3%  ██
│  └─ ICMPv6  1%  █
└─ ARP ──── <1%
```

#### 14.7.2 Protocol Statistics Table

| Column | Description |
|--------|-------------|
| Protocol | Protocol name and layer |
| Port | Standard port number (if applicable) |
| Bytes In | Total inbound bytes |
| Bytes Out | Total outbound bytes |
| Packets In | Total inbound packets |
| Packets Out | Total outbound packets |
| Connections | Active connection count |
| Bandwidth % | Percentage of total bandwidth |
| Top Process | Largest consumer of this protocol |
| Avg Packet Size | Mean packet size (bytes) |
| Error Rate | Protocol error percentage |

#### 14.7.3 Application-Layer Protocol Detection

Automatic identification of application protocols via deep packet inspection (optional, privacy-conscious):

| Protocol | Detection Method |
|----------|-----------------|
| HTTP/1.1 | Method/URI pattern |
| HTTP/2 | ALPN negotiation |
| HTTP/3 / QUIC | UDP 443 + QUIC header |
| TLS | ClientHello SNI |
| SSH | Protocol banner |
| FTP | Command patterns |
| SMTP/IMAP/POP3 | Port + banner |
| DNS | Port 53 + message format |
| NTP | Port 123 + packet size |
| DHCP | Port 67/68 |
| mDNS | Port 5353 multicast |
| SSDP/UPnP | Port 1900 multicast |
| BitTorrent | Protocol header / DHT |
| WireGuard | Port + handshake |
| RDP | Port 3389 + header |
| VNC | Port 5900 + RFB header |

### 14.8 Firewall Rules Viewer

Display and manage system firewall rules:

| Column | Description |
|--------|-------------|
| Rule Name | Descriptive name |
| Direction | Inbound / Outbound |
| Action | Allow / Block / Log |
| Protocol | TCP / UDP / ICMP / Any |
| Local Port | Port or range |
| Remote Port | Port or range |
| Local Address | IP or subnet |
| Remote Address | IP or subnet |
| Program | Associated executable path |
| Profile | Domain / Private / Public |
| Status | Enabled / Disabled |
| Hit Count | Number of times triggered (since boot) |
| Last Hit | Timestamp of last match |
| Created | When the rule was created |
| Description | Rule description |

Features:
- **Create Rule**: Wizard for creating new rules
- **Quick Block**: Right-click any connection → create blocking rule
- **Quick Allow**: Right-click a blocked process → create allow rule
- **Import/Export**: Backup and restore firewall rules
- **Rule Validation**: Detect conflicting or redundant rules
- **Active Blocks Log**: Real-time log of blocked connections with process and target info

### 14.9 Bandwidth Monitor & Traffic Shaping

#### 14.9.1 Per-Interface Bandwidth

| Column | Description |
|--------|-------------|
| Interface | Adapter name |
| Link Speed | Negotiated speed |
| Upload | Current upload rate |
| Download | Current download rate |
| Upload (Peak) | Peak upload rate (session) |
| Download (Peak) | Peak download rate (session) |
| Total Uploaded | Cumulative upload |
| Total Downloaded | Cumulative download |
| Utilization % | Current usage vs link capacity |
| MTU | Maximum transmission unit |
| Errors | Error count |
| Drops | Dropped packet count |
| Collisions | Collision count (if applicable) |

#### 14.9.2 Traffic Shaping (QoS)

Per-process or per-application bandwidth limiting:

| Setting | Description |
|---------|-------------|
| Process Bandwidth Limit | Max upload / download rate per process |
| Priority Class | Real-time / High / Normal / Low / Background |
| Schedule | Time-of-day rules (e.g., limit backups to nighttime) |
| Connection Limit | Max concurrent connections per process |
| Per-Interface Rules | Apply limits to specific adapters |
| Application Groups | Group processes for shared limits |
| Total Bandwidth Cap | Overall system bandwidth ceiling |
| Reserved Bandwidth | Guaranteed minimum for priority apps |
| Burst Allowance | Short-term burst permission above limit |

#### 14.9.3 Usage History & Quotas

| Feature | Description |
|---------|-------------|
| Daily Usage | Bar chart of daily upload/download |
| Weekly Usage | Week-over-week comparison |
| Monthly Usage | Monthly totals with trend line |
| Data Quota | Configurable quota with warning thresholds |
| Quota Alerts | Notifications at 50% / 75% / 90% / 100% of quota |
| Per-App Metering | Track data usage per application over time |
| Metered vs Unmetered | Separate tracking for metered connections |
| Cost Estimation | Configurable $/GB for billing estimation |
| Export Reports | Monthly usage reports (PDF/CSV) |

### 14.10 Network Interfaces Detail

Per-adapter deep information:

| Statistic | Description |
|-----------|-------------|
| Adapter Name | Friendly name |
| Adapter Type | Ethernet / Wi-Fi / Loopback / VPN / Cellular / Bluetooth PAN / Bridge / TAP |
| MAC Address | Hardware address |
| IPv4 Addresses | All assigned IPv4 + subnet masks |
| IPv6 Addresses | All assigned IPv6 + prefix lengths |
| DHCP Enabled | Yes / No |
| DHCP Server | DHCP server IP |
| DHCP Lease Expires | Lease expiration timestamp |
| DNS Servers | Configured DNS (per-interface) |
| Gateway | Default gateway |
| Metric | Interface metric / priority |
| Link Speed | Negotiated speed |
| Duplex | Full / Half |
| Wake-on-LAN | Enabled / Disabled |
| Offloading | Checksum / LSO / RSS / RSC offload status |
| VLAN ID | VLAN tag (if applicable) |
| Bond/Team | Bond/team membership (if applicable) |
| Promiscuous Mode | Yes / No |
| Carrier Detect | Connected / Disconnected |
| Driver | Driver name + version |
| Firmware | Adapter firmware version |
| PCI/USB Location | Bus location path |
| RDMA Capable | Yes / No |
| SR-IOV | Enabled / Disabled |

Wi-Fi specific:
| Statistic | Description |
|-----------|-------------|
| SSID | Connected network name |
| BSSID | Access point MAC |
| Signal Strength | dBm + quality % + bars |
| Noise Floor | dBm |
| SNR | Signal-to-noise ratio (dB) |
| Channel | Channel number + width |
| Frequency | 2.4 / 5 / 6 GHz |
| PHY Mode | 802.11a/b/g/n/ac/ax/be |
| Security | Open / WEP / WPA2-Personal / WPA2-Enterprise / WPA3-Personal / WPA3-Enterprise |
| Authentication | PSK / 802.1X / SAE / OWE |
| Encryption | AES / TKIP / GCMP-256 |
| TX Rate | Current transmit rate (Mbps) |
| RX Rate | Current receive rate (Mbps) |
| Spatial Streams | MIMO stream count |
| Guard Interval | Short / Long |
| AP Vendor | Access point manufacturer (OUI lookup) |
| Roaming Events | Count of AP transitions |
| Associated Since | Duration of current association |

### 14.11 Network Topology Map

Visual interactive map showing:

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│    ┌──────────┐                    ┌──────────┐             │
│    │ Internet │◄───────────────────┤ Gateway  │             │
│    │ ☁        │   WAN: 100 Mbps    │ 192.168. │             │
│    └──────────┘                    │ 1.1      │             │
│                                    └────┬─────┘             │
│                                         │                   │
│                           ┌─────────────┼──────────┐        │
│                           │             │          │        │
│                      ┌────▼────┐  ┌─────▼───┐ ┌───▼────┐   │
│                      │This PC  │  │Phone    │ │NAS     │   │
│                      │.1.100   │  │.1.101   │ │.1.50   │   │
│                      │12 conns │  │         │ │        │   │
│                      └─────────┘  └─────────┘ └────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

Features:
- Auto-discovered local network topology
- ARP/NDP neighbor detection
- Traffic flow lines with bandwidth thickness
- Click any node to see its connections
- Highlight traffic flowing to/from specific hosts
- Export topology diagram

### 14.12 Packet Capture (Lite)

Built-in lightweight packet capture for troubleshooting:

| Feature | Description |
|---------|-------------|
| Capture Filter | BPF-style filter expressions (e.g., `tcp port 443 and host 10.0.0.1`) |
| Display Filter | Post-capture filter on captured data |
| Capture Interface | Select adapter(s) to capture from |
| Capture Duration | Time-limited or manual stop |
| Capture Size Limit | Max file size or packet count |
| Ring Buffer | Rolling capture with configurable file count and size |
| Save Format | PCAP / PCAPNG |
| Open in External Tool | Export and open in Wireshark (if installed) |
| Summary Stats | Packet counts by protocol, conversation list, endpoint list |
| Conversation Tracking | Group packets into TCP streams and UDP conversations |
| Follow Stream | Reconstruct and display TCP stream content (ASCII/hex) |
| DNS Extraction | Parse and display all DNS queries/responses from capture |
| TLS Handshake Info | Extract and display TLS handshake details (SNI, certs, cipher) |
| HTTP Request/Response | Reconstruct HTTP transactions (headers + body, non-encrypted only) |
| Capture Statistics | Capture rate, buffer utilization, drops |

### 14.13 Network Health & Diagnostics

| Test | Description |
|------|-------------|
| Connectivity Check | Verify internet access (configurable target) |
| DNS Resolution Test | Test DNS resolution for configurable domains |
| Gateway Ping | Continuous ping to default gateway with jitter/loss stats |
| Traceroute | Visual traceroute with hop latency graph |
| Speed Test | Built-in bandwidth speed test (configurable server) |
| Port Scanner | Test connectivity to specific remote ports |
| MTU Discovery | Path MTU discovery to a target |
| Interface Diagnostics | Adapter self-test, driver health, cable test (where supported) |
| ARP Table | View and manage ARP cache |
| Routing Table | Display system routing table |
| Listening Ports Audit | List all listening ports with owning processes |
| Open Ports Security Check | Flag unexpected listening ports |

---

## 15. Tab: Energy & Power

### 15.1 Overview

Comprehensive energy management and power consumption analysis. Tracks per-component and per-process power draw, battery health analytics, thermal management, and energy efficiency scoring. Designed for both desktop (total system power) and portable devices (battery optimization).

### 15.2 Layout

```
┌───────────────┬──────────────────────────────────────────────────┐
│               │                                                  │
│ [Overview]    │  <<< Selected View Content >>>                   │
│ [By Process]  │                                                  │
│ [By Component]│  ┌─────────────────────────────────────────────┐ │
│ [Battery]     │  │  Power draw graph / efficiency gauges       │ │
│ [Thermal]     │  │                                             │ │
│ [Power Plans] │  └─────────────────────────────────────────────┘ │
│ [History]     │                                                  │
│ [Efficiency]  │  <<< Detail Panel >>>                            │
│ [Carbon]      │                                                  │
│               │                                                  │
└───────────────┴──────────────────────────────────────────────────┘
```

### 15.3 Energy Overview Dashboard

| Widget | Description |
|--------|-------------|
| **System Power Draw** | Real-time total system wattage (gauge + sparkline) |
| **Power Breakdown** | Donut chart: CPU / GPU / Display / Storage / Network / Other |
| **Energy Efficiency Score** | 0–100 score based on workload vs power consumption |
| **Top Power Consumers** | Top 5 processes by estimated power draw (bar chart) |
| **Battery Status** | Level + estimated time remaining + charge/discharge rate |
| **Thermal Summary** | Key temperatures: CPU / GPU / SSD / Ambient |
| **Power Plan** | Active power profile with quick-switch button |
| **Carbon Footprint** | Estimated CO₂ based on energy source and usage |
| **24h Energy Consumed** | Total energy consumed in last 24 hours (Wh) |
| **Power Source** | AC Adapter / Battery / UPS with wattage info |

### 15.4 Per-Process Energy View

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Process | `energy_proc` | Process name + PID | ★ |
| Power Usage | `energy_power` | Estimated instantaneous power (mW) | ★ |
| Power Category | `energy_cat` | Very Low / Low / Moderate / High / Very High | ★ |
| Power Trend | `energy_trend` | ↑ Increasing / → Stable / ↓ Decreasing (60s window) | ★ |
| CPU Energy | `energy_cpu` | CPU-attributable power (mW) | |
| GPU Energy | `energy_gpu` | GPU-attributable power (mW) | |
| Disk Energy | `energy_disk` | Storage-attributable power (mW) | |
| Network Energy | `energy_net` | Network-attributable power (mW) | |
| Display Wake Locks | `energy_wake` | Count of display/sleep preventing wake locks | |
| Background Activity | `energy_bg` | Background wake-ups per minute | |
| Energy (Session) | `energy_total` | Total energy consumed since process start (mWh) | |
| Energy (24h) | `energy_24h` | Energy consumed in last 24 hours (mWh) | |
| Battery Impact | `energy_battery` | Estimated % battery impact per hour | ★ |
| Efficiency Rating | `energy_rating` | A+ / A / B / C / D / F efficiency grade | |
| Recommendations | `energy_reco` | "Limit background activity" / "Reduce GPU usage" / etc. | |

Features:
- Sort by any column to find energy hogs
- Comparison mode: compare energy profile before/after an action
- Energy timeline: per-process energy use over time
- Background energy audit: processes consuming energy while user is inactive
- Sleep prevention tracking: processes preventing system sleep/screen off

### 15.5 Per-Component Energy Breakdown

#### 15.5.1 CPU Power

| Statistic | Description |
|-----------|-------------|
| Package Power | Total CPU package power draw (W) |
| Core Power | CPU core power (W) |
| Uncore Power | Memory controller, cache, etc. (W) |
| DRAM Power | Memory subsystem power (W) |
| Per-Core Power | Individual core power draw |
| C-State Distribution | Time % in each C-state (C0/C1/C3/C6/C7/C10) |
| P-State / Frequency | Current operating frequency per core |
| Power Limit (PL1) | Sustained power limit (W) |
| Power Limit (PL2) | Burst power limit (W) |
| Power Throttling | Active / Inactive + reason |
| TDP | Thermal Design Power rating |
| Turbo Budget | Remaining turbo power budget |
| Efficiency Cores | Power draw of E-cores vs P-cores (hybrid CPUs) |

Graphs: Package power over time, C-state residency bar chart, frequency vs power scatter plot.

#### 15.5.2 GPU Power

| Statistic | Description |
|-----------|-------------|
| GPU Board Power | Total GPU board power (W) |
| GPU Core Power | GPU core power (W) |
| GPU Memory Power | VRAM power (W) |
| Power Limit | Current power limit (W) |
| Power Limit % | Usage vs limit |
| Power State | D0 (Active) / D1–D3 (Sleep states) |
| Performance State | P0 (Max) through P8 (Min) |
| Power Cap | User-configurable power cap |
| Throttle Reason | Thermal / Power / Current / Voltage / None |
| Fan Power | Estimated fan motor power (W) |

#### 15.5.3 Display Power

| Statistic | Description |
|-----------|-------------|
| Backlight Level | Current brightness % |
| Estimated Power | Backlight power draw (W) |
| Auto-Brightness | Enabled / Disabled |
| Ambient Light | Current ambient light sensor reading (lux) |
| Display Timeout | Screen off timer setting |
| HDR Active | HDR content being displayed |
| Refresh Rate | Current refresh rate (Hz) |
| Adaptive Sync | VRR active / inactive |
| Panel Type | OLED / LCD / Mini-LED |
| Always-On Display | Enabled / Disabled |

#### 15.5.4 Storage Power

| Statistic | Description |
|-----------|-------------|
| Per-Drive Power | Power draw per storage device (W) |
| Active vs Idle | Drive active time % |
| Spindown Status | HDD spindown state |
| Low Power Mode | NVMe APST / SATA ALPM state |
| Device Sleep | DevSleep support + state |

#### 15.5.5 Network Power

| Statistic | Description |
|-----------|-------------|
| Wi-Fi Power | Wi-Fi radio power draw (mW) |
| Wi-Fi Power Save | Power save mode (active / CAM / U-APSD) |
| Ethernet Power | Ethernet PHY power (wake vs connected vs EEE idle) |
| EEE Status | Energy-Efficient Ethernet state |
| Bluetooth Power | BT radio power draw (mW) |
| Cellular Power | Cellular modem power (mW) |
| Wake-on-LAN | Power reserved for WoL |
| Radio States | On / Off / Airplane mode per radio |

#### 15.5.6 Peripheral Power

| Statistic | Description |
|-----------|-------------|
| USB Total Power | Total USB bus power draw (mW) |
| Per-Port Power | Power draw per USB port |
| USB Suspend | Devices in USB selective suspend |
| Thunderbolt Power | Thunderbolt bus power |
| PCIe Devices | Power state per PCIe device |
| Audio Subsystem | DAC/amp power draw (mW) |
| Keyboard Backlight | Power draw (mW) |
| Biometric Sensors | Fingerprint / IR camera power |

### 15.6 Battery Analytics

#### 15.6.1 Battery Status Panel

| Statistic | Description |
|-----------|-------------|
| Status | Charging / Discharging / Full / Not Charging / Plugged In (not charging) |
| Level | Current % (to 0.1% resolution) |
| Estimated Remaining | Hours:Minutes on current load |
| Estimated Full Charge | Hours:Minutes until full |
| Charge Rate | Current charging power (W) |
| Discharge Rate | Current drain power (W) |
| Voltage | Current voltage (V) |
| Current | Charge/discharge current (mA) |
| Temperature | Battery temperature (°C) |
| Cycle Count | Total charge cycles |
| Design Capacity | Original capacity (mWh) |
| Full Charge Capacity | Current max capacity (mWh) |
| Health | Full charge / Design capacity × 100% |
| Chemistry | Li-ion / Li-polymer / LFP |
| Manufacturer | Battery manufacturer |
| Serial Number | Battery serial |
| Manufacture Date | When the battery was made |
| Cells | Cell count and configuration |
| Calibration Status | Last calibrated date + recommendation |

#### 15.6.2 Battery Graphs

| Graph | Description |
|-------|-------------|
| **Charge Level** | % over time (24h / 7d / 30d) |
| **Discharge Rate** | mW over time |
| **Estimated Remaining** | Projection line |
| **Charge Cycles** | Cycle count history over months |
| **Health Trend** | Capacity degradation over battery lifetime |
| **Charge/Discharge Curves** | Voltage vs capacity curves |
| **Temperature History** | Battery temp during charge/discharge |
| **Average Daily Usage** | Hours on battery per day (30-day trend) |
| **Power Distribution** | Stacked area: what's consuming battery power |

#### 15.6.3 Battery Health Report

Generates a comprehensive battery health report:
- Current health percentage with color indicator (🟢 > 80%, 🟡 60–80%, 🔴 < 60%)
- Capacity fade graph over entire battery lifetime  
- Cycle count vs capacity correlation
- Prediction of time until battery reaches 80% health
- Charge habit analysis (fast charging frequency, deep discharge count, trickle charge time)
- Temperature stress events (charges above 40°C)
- Comparison to expected degradation curve for this battery model
- Recommendations: optimal charge range, charge frequency, thermal management tips
- Exportable as PDF/HTML battery health report

### 15.7 Thermal Management

#### 15.7.1 Thermal Overview

```
┌─────────────────────────────────────────────────┐
│           Thermal Map (Schematic View)           │
│                                                  │
│    ┌────────────┐     ┌────────────┐            │
│    │   CPU      │     │   GPU      │            │
│    │  ██ 72°C   │     │  ██ 65°C   │            │
│    │  (Pkg)     │     │  (Core)    │            │
│    └────────────┘     └────────────┘            │
│                                                  │
│    ┌────────────┐     ┌────────────┐            │
│    │   SSD      │     │  Battery   │            │
│    │  ░░ 42°C   │     │  ░░ 34°C   │            │
│    └────────────┘     └────────────┘            │
│                                                  │
│    ┌────────────┐     ┌────────────┐            │
│    │  Ambient   │     │   VRM      │            │
│    │  ░░ 28°C   │     │  ▒▒ 55°C   │            │
│    └────────────┘     └────────────┘            │
│                                                  │
│  Legend: ░░ Cool   ▒▒ Warm   ██ Hot   ██ Critical│
└─────────────────────────────────────────────────┘
```

#### 15.7.2 Thermal Sensor Table

| Column | Description |
|--------|-------------|
| Sensor Name | Descriptive name (CPU Package, GPU Core, SSD, etc.) |
| Location | Component / zone identifier |
| Temperature | Current reading (°C / °F toggle) |
| Min | Minimum observed (session) |
| Max | Maximum observed (session) |
| Average | Mean temperature (session) |
| Threshold Warning | Warning threshold (°C) |
| Threshold Critical | Critical/shutdown threshold (°C) |
| Throttle Point | Temperature at which throttling begins |
| Status | 🟢 Normal / 🟡 Warm / 🟠 Hot / 🔴 Critical / ⚫ Throttling |
| Trend | ↑ Rising / → Stable / ↓ Cooling |

#### 15.7.3 Fan Control

| Statistic | Description |
|-----------|-------------|
| Fan Name | Fan identifier (CPU Fan, GPU Fan, System Fan 1, etc.) |
| Speed (RPM) | Current RPM |
| Speed (%) | Current duty cycle percentage |
| Mode | Auto / Manual / Silent / Performance |
| Target Temperature | Temperature the fan is targeting |
| Min RPM | Minimum configurable speed |
| Max RPM | Maximum rated speed |
| Fan Curve | Configurable temperature-to-RPM curve (visual editor) |
| Noise Level | Estimated dBA (if sensor available) |
| Status | Running / Stopped / Error |

Features:
- Custom fan curves with visual drag-and-drop editor
- Fan profiles: Silent / Balanced / Performance / Custom
- Temperature-triggered alerts
- Fan failure detection and alert

#### 15.7.4 Thermal Graphs

| Graph | Description |
|-------|-------------|
| **All Sensors** | Overlaid temperature lines for all sensors |
| **Per-Component** | Individual sensor with threshold lines |
| **Heat vs Load** | Dual-axis: temperature + CPU/GPU load correlation |
| **Thermal Throttle Events** | Timeline markers showing when throttling occurred |
| **Fan Speed History** | RPM/% over time, all fans overlaid |
| **Ambient vs Component** | Compare ambient to component deltas |

### 15.8 Power Plans & Profiles

| Feature | Description |
|---------|-------------|
| Active Profile | Currently active power plan |
| Available Profiles | List all system power plans |
| Quick Switch | One-click profile change |
| Profile Details | Detailed settings for each profile |
| Create Custom | Wizard for creating custom power profiles |
| Scheduled Profiles | Automatic profile switching by time-of-day or battery level |
| App-Specific Profiles | Override profiles when specific apps are running |
| Profile Comparison | Side-by-side comparison of profile settings |
| Import/Export | Share power profiles between machines |

Profile Settings Include:
| Setting | Description |
|---------|-------------|
| CPU Min/Max Frequency | Frequency range limits |
| CPU Boost | Turbo boost enabled / disabled |
| Core Parking | Minimum active cores |
| GPU Power Limit | GPU power cap |
| Display Brightness | Default brightness level |
| Display Timeout | Screen off delay (AC / battery) |
| Sleep Timeout | Sleep delay (AC / battery) |
| Hibernate Timeout | Hibernate delay (AC / battery) |
| HDD Spindown | Hard drive sleep timer |
| USB Selective Suspend | USB power management |
| Wi-Fi Power Save | Wi-Fi power saving aggressiveness |
| PCIe ASPM | PCIe active state power management |
| Processor Power Policy | Favor performance / efficiency balance |
| Cooling Policy | Active (fan first) / Passive (throttle first) |

### 15.9 Energy History & Reporting

| Feature | Description |
|---------|-------------|
| Hourly Energy | Wh consumed per hour (bar chart) |
| Daily Energy | Wh consumed per day (bar chart, 30-day view) |
| Weekly Summary | Total energy + average wattage + cost estimate |
| Monthly Report | Full monthly energy report with breakdown |
| Per-App Energy | Historical energy consumption per application |
| AC vs Battery Distribution | Time spent on each power source |
| Energy Cost Calculation | Configurable electricity rate ($/kWh) for cost projection |
| Year-over-Year | Compare energy consumption patterns annually |
| Export | PDF/CSV/JSON energy reports |
| Efficiency Trends | Track efficiency scores over time |

### 15.10 Carbon Footprint Tracking

| Feature | Description |
|---------|-------------|
| Grid Carbon Intensity | Real-time carbon intensity (gCO₂/kWh) from electricity grid API |
| Session Carbon | Estimated CO₂ emitted during current session |
| Daily Carbon | Daily CO₂ estimate |
| Monthly Carbon | Monthly CO₂ with comparison to average |
| Carbon Budget | Set personal carbon goals with progress tracking |
| Green Energy % | Detected percentage of renewable energy in grid mix |
| Work Schedule Optimization | Suggest shifting heavy workloads to low-carbon hours |
| Carbon Offset Equivalent | Display CO₂ in relatable terms (e.g., "equivalent to X km driving") |
| Regional Data Source | Configurable data source for carbon intensity (Electricity Maps, WattTime, etc.) |

### 15.11 Wake Lock & Sleep Prevention Audit

| Column | Description |
|--------|-------------|
| Process | Process preventing sleep/display off |
| PID | Process ID |
| Lock Type | Display Required / System Required / Away Mode / Execution Required |
| Reason | Stated reason for wake lock (if provided by process) |
| Duration | How long the lock has been held |
| Power Impact | Estimated extra power from this lock (mW) |
| Action | [Release Lock] button (may cause app issues) |

Features:
- Alert when unknown processes hold wake locks
- History of wake lock events
- Aggregate wake lock time per process
- Identify processes preventing standby, hibernate, or screen-off

---

## 16. Tab: Audio

### 16.1 Overview

Comprehensive audio device management, stream monitoring, audio routing, and real-time analysis. Provides visibility into every audio stream, device, and effect in the system with professional-level metering and diagnostic tools.

### 16.2 Layout

```
┌───────────────┬──────────────────────────────────────────────────┐
│               │                                                  │
│ [Output]      │  <<< Selected View Content >>>                   │
│ [Input]       │                                                  │
│ [Streams]     │  ┌──────────────────────────────────────┐        │
│ [Routing]     │  │  VU Meters / Device Info / Routing   │        │
│ [Devices]     │  │  Diagram / Stream Table              │        │
│ [Effects]     │  └──────────────────────────────────────┘        │
│ [Spatial]     │                                                  │
│ [MIDI]        │  <<< Detail Panel >>>                            │
│ [Stats]       │                                                  │
│ [Diagnostics] │                                                  │
│               │                                                  │
└───────────────┴──────────────────────────────────────────────────┘
```

### 16.3 Output Devices

#### 16.3.1 Output Device List

| Column | Description |
|--------|-------------|
| Device Name | Friendly name (e.g., "Speakers (Realtek Audio)") |
| Status | Active / Inactive / Default / Disabled / Unplugged / Error |
| Type | Speakers / Headphones / HDMI / DisplayPort / USB / Bluetooth / S/PDIF / Line Out / Monitor |
| Volume | Current volume level (%) |
| Muted | Yes / No |
| Sample Rate | Current sample rate (Hz) |
| Bit Depth | 16 / 24 / 32 / 32-float bit |
| Channels | Mono / Stereo / 5.1 / 7.1 / Atmos configuration |
| Channel Layout | Speaker configuration (FL, FR, C, LFE, RL, RR, etc.) |
| Latency | Output pipeline latency (ms) |
| Buffer Size | Audio buffer (samples) |
| Format | PCM / DSD / AC3 / EAC3 / TrueHD / DTS / DTS-HD |
| Exclusive Mode | Available / In Use by [process] / Disabled |
| Spatial Audio | Off / Windows Sonic / Dolby Atmos / DTS:X / Custom |
| Volume Limiter | Enabled / Disabled + level |
| Enhancement | Effects applied (EQ, loudness, virtual surround, etc.) |
| Peak Level (L) | Peak meter left channel (dBFS) |
| Peak Level (R) | Peak meter right channel (dBFS) |
| RMS Level | RMS level (dBFS) |
| LUFS | Integrated loudness (LUFS) |
| Driver | Audio driver name + version |
| Endpoint ID | System device endpoint ID |

#### 16.3.2 Output VU Meters

Professional real-time level meters displayed alongside each output device:

```
┌─────────────────────────────────────────────────────┐
│  Speakers (Realtek Audio) — 48kHz / 24-bit / Stereo │
│                                                      │
│  L ████████████████████████████░░░░░░░  -6.2 dBFS   │
│  R ██████████████████████████░░░░░░░░░  -8.1 dBFS   │
│                                                      │
│  Peak: -3.4 dBFS    RMS: -12.8 dBFS    LUFS: -14.2  │
│  Clipping: None     Dynamic Range: 18.4 dB           │
│                                                      │
│  Headroom: 3.4 dB   Crest Factor: 9.4 dB            │
│                                                      │
│  [Volume: ████████████████░░░░ 78%]  [🔊]  [Default] │
└─────────────────────────────────────────────────────┘
```

Features:
- Peak hold with configurable decay time
- Peak-to-RMS crest factor display
- Clip indicators with red flash
- Configurable meter scales: dBFS / dBu / VU / PPM
- Multi-channel metering for surround configurations
- Meter ballistics: Peak / PPM (Type I/II) / VU / True Peak

### 16.4 Input Devices

#### 16.4.1 Input Device List

| Column | Description |
|--------|-------------|
| Device Name | Friendly name (e.g., "Microphone (Blue Yeti)") |
| Status | Active / Inactive / Default / Disabled / Unplugged / Error |
| Type | Microphone / Line In / Headset Mic / USB / Bluetooth / Loopback / Digital In / Array Mic |
| Volume | Input gain level (%) |
| Boost | Additional boost (dB, if applicable) |
| Muted | Yes / No |
| Sample Rate | Current sample rate (Hz) |
| Bit Depth | 16 / 24 / 32-float bit |
| Channels | Mono / Stereo / Multi-channel count |
| Pickup Pattern | Cardioid / Omnidirectional / Bidirectional / Stereo (if USB mic) |
| Latency | Input pipeline latency (ms) |
| Buffer Size | Audio buffer (samples) |
| Noise Gate | Enabled / Disabled + threshold |
| Noise Suppression | Enabled / Disabled + level |
| Echo Cancellation | Enabled / Disabled |
| AGC | Automatic Gain Control on / off |
| Peak Level | Current input peak (dBFS) |
| RMS Level | Current input RMS (dBFS) |
| Noise Floor | Measured noise floor (dBFS or dBA) |
| SNR | Signal-to-noise ratio (dB) |
| THD+N | Total harmonic distortion + noise (%) |
| DC Offset | Detected DC offset (mV) |
| Active Consumers | Processes currently reading from this input |
| Exclusive Mode | Available / In Use by [process] |
| Driver | Audio driver name + version |

#### 16.4.2 Input VU Meters

```
┌─────────────────────────────────────────────────────┐
│  Microphone (Blue Yeti) — 48kHz / 24-bit / Mono     │
│                                                      │
│  ████████████████████████░░░░░░░░░░░░░  -12.4 dBFS  │
│                                                      │
│  Peak: -8.2 dBFS    RMS: -18.6 dBFS                 │
│  Noise Floor: -62 dBFS   SNR: 43.6 dB               │
│  Clipping: None                                      │
│                                                      │
│  Waveform: ⌒∿⌒∿∿⌒∿⌒⌒∿⌒∿∿⌒                         │
│                                                      │
│  [Gain: ████████░░░░░░░░░░░░ 42%]  [🔇]  [Default]  │
└─────────────────────────────────────────────────────┘
```

Features:
- Real-time waveform display
- Spectrum analyzer (optional overlay)
- Voice activity detection indicator
- Background noise level indicator
- Configurable clipping threshold alert
- Input level history graph

### 16.5 Audio Streams

Per-process audio stream monitoring:

#### 16.5.1 Stream Table

| Column | Key | Description | Default |
|--------|-----|-------------|---------|
| Process | `stream_proc` | Process name + PID | ★ |
| Direction | `stream_dir` | Output (Render) / Input (Capture) | ★ |
| Device | `stream_dev` | Target audio device | ★ |
| Volume | `stream_vol` | Per-stream volume (%) | ★ |
| Muted | `stream_mute` | Yes / No | ★ |
| Peak Level | `stream_peak` | Current peak (dBFS) | ★ |
| RMS Level | `stream_rms` | Current RMS (dBFS) | |
| Sample Rate | `stream_rate` | Stream sample rate (Hz) | |
| Bit Depth | `stream_bits` | Stream bit depth | |
| Channels | `stream_ch` | Channel count | |
| Format | `stream_fmt` | Float32 / Int16 / Int24 / etc. | |
| Latency | `stream_lat` | Stream presentation latency (ms) | |
| Buffer Frames | `stream_buf` | Buffer size in audio frames | |
| State | `stream_state` | Playing / Paused / Stopped / Error | ★ |
| Duration Active | `stream_dur` | How long the stream has been active | |
| Bytes Processed | `stream_bytes` | Total audio data processed | |
| Underruns | `stream_under` | Buffer underrun count (glitches) | |
| Overruns | `stream_over` | Buffer overrun count | |
| Session ID | `stream_sess` | Audio session identifier | |
| Exclusive | `stream_excl` | Shared / Exclusive mode | |
| Ducking | `stream_duck` | Stream is being ducked (auto-lowered) | |

#### 16.5.2 Stream Context Menu

```
├── Adjust Volume (slider)
├── Mute / Unmute
├── ─────────────────────────
├── Move to Device ► (reassign output/input device)
│   ├── Speakers (Realtek)
│   ├── Headphones (USB)
│   └── HDMI Output
├── ─────────────────────────
├── Go to Process
├── Set as Default Audio Source
├── ─────────────────────────
├── Per-Stream EQ...
├── Apply Effect ►
│   ├── Noise Suppression
│   ├── Volume Normalization
│   ├── Dynamic Compression
│   └── Spatial Processing
├── ─────────────────────────
├── Show Waveform
├── Show Spectrum
├── Record Stream... (capture to file)
├── ─────────────────────────
├── Copy Stream Info
├── Export Stream Stats
└── Reset Stream
```

### 16.6 Audio Routing Matrix

Visual audio routing showing how all streams connect to devices:

```
┌─────────────────────────────────────────────────────────────────┐
│               Audio Routing Matrix                               │
│                                                                   │
│  Sources                    Outputs                              │
│  ┌─────────────┐            ┌──────────────────┐                │
│  │ Firefox     │────────────►│ Speakers         │                │
│  │  (Music)    │            │ (Realtek Audio)  │                │
│  └─────────────┘     ┌─────►│                  │                │
│  ┌─────────────┐     │      └──────────────────┘                │
│  │ Discord     │─────┘                                          │
│  │  (Voice)    │────────────►┌──────────────────┐                │
│  └─────────────┘            │ Headphones       │                │
│  ┌─────────────┐     ┌─────►│ (USB Audio)      │                │
│  │ Game.exe    │─────┘      └──────────────────┘                │
│  │  (Effects)  │                                                │
│  └─────────────┘            ┌──────────────────┐                │
│  ┌─────────────┐            │ HDMI Output      │                │
│  │ Video Player│────────────►│ (LG TV)          │                │
│  │  (Movie)    │            └──────────────────┘                │
│  └─────────────┘                                                │
│                                                                   │
│  Inputs                     Consumers                            │
│  ┌─────────────┐            ┌──────────────────┐                │
│  │ Microphone  │────────────►│ Discord          │                │
│  │ (Blue Yeti) │     ┌─────►│  (Voice Input)   │                │
│  └─────────────┘     │      └──────────────────┘                │
│  ┌─────────────┐     │      ┌──────────────────┐                │
│  │ Line In     │─────┘      │ OBS Studio       │                │
│  │ (Realtek)   │────────────►│  (Recording)     │                │
│  └─────────────┘            └──────────────────┘                │
│                                                                   │
│  [Drag to re-route] [Add Virtual Cable] [Reset Routing]          │
└─────────────────────────────────────────────────────────────────┘
```

Features:
- Drag-and-drop audio routing between sources and sinks
- Virtual audio cables (create virtual loopback devices)
- Multi-output routing (send one stream to multiple devices)
- Volume mixing at routing edges
- Saved routing profiles (e.g., "Production", "Gaming", "Meeting")
- Auto-switch routing rules (when headphones connected → route all to headphones)
- Visual indication of active audio flow (animated lines)

### 16.7 Device Hardware Details

Expanded hardware information per audio device:

| Statistic | Description |
|-----------|-------------|
| Device Name | Friendly name |
| Manufacturer | Hardware manufacturer |
| Hardware ID | PnP hardware identifier |
| Driver | Driver name + version + date |
| INF File | Driver INF path |
| Interface | USB / PCI / Bluetooth A2DP / Bluetooth HFP / HDMI / DisplayPort / S/PDIF / Analog 3.5mm / Thunderbolt |
| Codec | Audio codec chip (e.g., Realtek ALC1220, ESS Sabre) |
| DAC Resolution | DAC bit depth capability |
| Max Sample Rate | Maximum supported rate (Hz) |
| Supported Formats | List of all supported format/rate/channel combos |
| Min Buffer Size | Minimum supported buffer (samples) |
| Max Buffer Size | Maximum supported buffer (samples) |
| ASIO Support | Yes / No + ASIO driver name |
| WASAPI Exclusive | Supported / Not Supported |
| Jack Detection | Supported / Not Supported + Current state |
| Impedance Matching | Output impedance (Ω) if known |
| Max Output Power | mW at rated impedance |
| Frequency Response | Listed range (Hz) if known |
| SNR Rating | Rated signal-to-noise ratio (dB) |
| THD Rating | Rated total harmonic distortion (%) |
| Power Delivery | USB bus power / Self-powered / Phantom power |
| Firmware Version | Device firmware |
| Serial Number | Device serial |

### 16.8 Audio Effects & Processing Chain

View and manage the audio processing pipeline:

#### 16.8.1 System Effects

| Effect | Description |
|--------|-------------|
| Equalizer | System-wide or per-device parametric EQ (visual editor with frequency response curve) |
| Bass Boost | Low-frequency enhancement |
| Loudness Equalization | Dynamic loudness normalization |
| Virtual Surround | Stereo-to-surround upmix |
| Room Correction | Acoustic room calibration correction |
| Dynamic Range Compression | Compressor/limiter for output |
| Noise Suppression (Input) | AI-based noise removal for mic input |
| Echo Cancellation | Acoustic echo removal |
| Voice Enhancement | Clarity/presence boost for voice |
| Spatial Audio Engine | Head-tracked spatial rendering (Windows Sonic / Dolby Atmos / DTS:X) |

#### 16.8.2 Effect Chain Visualization

```
Input → [Noise Gate] → [Noise Suppression] → [EQ] → [Compressor] → [Limiter] → Output
         -40 dBFS      AI Enhanced           +3dB     3:1 ratio      -0.3 dBFS
         Enabled ✓      Enabled ✓            Enabled ✓ Enabled ✓      Enabled ✓
```

Each effect node is clickable to edit parameters. Effects can be:
- Enabled / Disabled per effect
- Reordered via drag-and-drop
- Bypassed (all effects off for comparison)
- Per-device or per-stream
- Saved as presets

#### 16.8.3 DSP Load

| Statistic | Description |
|-----------|-------------|
| DSP CPU Usage | CPU used by audio processing (%) |
| Effect Latency | Total latency added by effects (ms) |
| Per-Effect Latency | Breakdown by individual effect |
| Processing Load | DSP processing load indicator (safe / moderate / high) |

### 16.9 Spatial Audio

| Feature | Description |
|---------|-------------|
| Spatial Engine | Active spatial audio engine (None / Sonic / Atmos / DTS:X / Custom HRTF) |
| Head Tracking | Connected head tracker device + status |
| HRTF Profile | Active Head-Related Transfer Function (Generic / Personalized) |
| Speaker Virtualization | Virtual speaker positions (3D diagram) |
| Channel Mapping | Input channel → virtual speaker position mapping |
| Object Count | Active audio objects (object-based audio) |
| Bed Channels | Bed audio channel configuration |
| Height Channels | Overhead/height channel count |
| Render Quality | Low / Medium / High / Ultra |
| Personalization | Head dimensions / ear shape profile (if measured) |

### 16.10 MIDI Devices

| Column | Description |
|--------|-------------|
| Device Name | MIDI device name |
| Type | Input / Output / Input+Output |
| Status | Connected / Error / In Use |
| Interface | USB / Bluetooth / DIN / Virtual |
| Active Process | Process currently using this MIDI device |
| Messages/sec | Current MIDI message rate |
| Last Note | Last MIDI note received/sent (visual keyboard) |
| Channel Filter | Channels being listened to (1–16 or All) |
| Manufacturer | Device manufacturer |
| SysEx ID | System Exclusive device ID |
| Firmware | Device firmware version |

MIDI Monitor:
- Real-time MIDI message log (Note On/Off, CC, Program Change, SysEx, Clock, etc.)
- Virtual MIDI keyboard display showing active notes
- MIDI channel activity indicators
- Timing accuracy analysis (jitter measurement for MIDI clock)
- MIDI message rate graph

### 16.11 Audio Statistics & Analysis

#### 16.11.1 Real-Time Spectrum Analyzer

```
┌────────────────────────────────────────────────────────┐
│  Spectrum Analyzer — Speakers (Realtek Audio)           │
│                                                         │
│  dBFS                                                   │
│   0  ┤                                                  │
│ -10  ┤          ██                                      │
│ -20  ┤    ██    ██  ██                                  │
│ -30  ┤  ████  ████  ████                                │
│ -40  ┤  ████  ████  ██████  ██                          │
│ -50  ┤████████████████████████████                      │
│ -60  ┤████████████████████████████████████              │
│ -70  ┤████████████████████████████████████████████████  │
│      └──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬─  │
│        20  50 100 200 500 1k 2k  5k 10k 20k            │
│                    Frequency (Hz)                       │
│                                                         │
│  Mode: [1/3 Oct] [FFT] [Linear] [Waterfall]            │
│  Window: [Hann] [Hamming] [Blackman] [Flat-Top]         │
│  FFT Size: [1024] [2048] [4096] [8192] [16384]         │
└────────────────────────────────────────────────────────┘
```

Display modes:
- **1/3 Octave Band**: Standard 31-band analyzer
- **FFT**: Raw frequency-domain display (configurable FFT size)
- **Linear**: Linear frequency scale
- **Logarithmic**: Logarithmic frequency scale (default)
- **Waterfall**: Scrolling spectrogram (frequency × time × intensity)
- **3D Waterfall**: Three-dimensional spectral view
- **Sonogram**: Color-coded spectrogram image

#### 16.11.2 Audio Quality Metrics

| Metric | Description |
|--------|-------------|
| Sample Rate Mismatch | Detecting resampling in the pipeline |
| Bit Depth Truncation | Detecting bit depth reduction |
| Clipping Events | Count of digital clipping occurrences |
| Buffer Underruns (Glitches) | Audio dropout events per minute |
| Buffer Overruns | Input buffer overflow events |
| Latency (Round-Trip) | Total input-to-output latency (ms) |
| Latency (Pipeline) | Audio engine processing latency (ms) |
| Latency (Device) | Hardware device latency (ms) |
| Latency (Driver) | Driver buffering latency (ms) |
| Jitter | Sample clock jitter (μs) |
| Drift | Clock drift between devices (ppm) |
| CPU Audio Load | CPU time spent on audio processing (%) |
| DPC Latency | Deferred procedure call latency affecting audio (μs) |
| ISR Latency | Interrupt service routine latency (μs) |
| Audio Thread Priority | Audio renderer thread scheduling priority |
| Peak Inter-Sample Level | True peak level (may exceed 0 dBFS) |
| Dynamic Range | Measured dynamic range (dB) |
| Loudness (LUFS) | Integrated loudness measurement |
| Loudness Range (LRA) | Loudness range statistic (LU) |
| True Peak | Maximum true-peak level (dBTP) |

#### 16.11.3 Session Statistics

| Statistic | Description |
|-----------|-------------|
| Total Audio Time | Total time audio has been active (session) |
| Total Streams Created | Number of audio streams opened |
| Peak Concurrent Streams | Maximum simultaneous active streams |
| Total Glitches | Total buffer underrun count (session) |
| Glitch-Free Duration | Longest period without audio glitches |
| Average Latency | Mean audio pipeline latency |
| Volume Changes | Number of volume adjustments |
| Device Switches | Number of audio device changes |
| Exclusive Mode Sessions | Count of exclusive mode acquisitions |
| Spatial Audio Active Time | Duration of spatial audio processing |

### 16.12 Audio Diagnostics

| Test | Description |
|------|-------------|
| Playback Test | Play test tones through selected output (sine, pink noise, frequency sweep, channel identification) |
| Microphone Test | Record and playback from selected input with analysis |
| Loopback Latency Test | Measure round-trip latency (requires loopback cable or software loopback) |
| Speaker Identification | Play sequential tones from each speaker to verify channel mapping |
| Frequency Response Test | Sweep test to measure device frequency response (requires calibrated mic for output) |
| THD Test | Measure total harmonic distortion (requires loopback) |
| Noise Floor Measurement | Measure input device noise floor with silence |
| Driver Latency Test | Measure driver and DPC latency impact on audio |
| Bluetooth Codec Test | Identify active Bluetooth audio codec (SBC / AAC / aptX / aptX HD / LDAC / LC3) |
| USB Audio Class | Identify USB Audio Class version (UAC1 / UAC2 / UAC3) |
| ASIO Test | Test ASIO driver availability and configuration |
| Jack Detection Test | Test audio jack plug/unplug detection |
| Volume Normalization Check | Verify system volume normalization behavior |
| Spatial Audio Test | Verify spatial audio object positioning with test sounds |

### 16.13 Audio Event Log

Chronological log of audio system events:

| Column | Description |
|--------|-------------|
| Timestamp | Event time |
| Event Type | Device Connected / Disconnected / Default Changed / Volume Changed / Stream Started / Stream Ended / Format Changed / Glitch / Error / Exclusive Mode / Driver Event |
| Device | Affected device |
| Process | Related process (if applicable) |
| Details | Event-specific details |
| Severity | Info / Warning / Error / Critical |

---

## 17. Settings & Configuration

### 17.1 General Settings

| Setting | Options | Default |
|---------|---------|---------|
| Default Tab | Any tab name | Processes |
| Default View Mode | Compact / Standard / Advanced | Standard |
| Always on Top | On / Off | Off |
| Minimize to System Tray | On / Off | Off |
| Show System Tray Icon | On / Off / When Minimized | When Minimized |
| Confirm Before Ending Tasks | On / Off | On |
| Show Full Account Name | On / Off (DOMAIN\user vs user) | Off |
| Show Full Path in Title Bar | On / Off | Off |
| Remember Window Position | On / Off | On |
| Remember Column Layout | On / Off | On |
| Start Minimized | On / Off | Off |
| Launch at Login | On / Off | Off |
| Language | System / Manual selection | System |
| Date/Time Format | System / ISO 8601 / Custom | System |
| Number Format | 1,000.00 / 1.000,00 / 1000.00 | System |
| Reset All Settings | Button | — |

### 17.2 Processes Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Visible Columns | Multi-select from all available | See §4.2 defaults |
| Column Order | Drag-and-drop reorder | — |
| Column Widths | Per-column pixel width | Auto |
| Default Sort Column | Any column | CPU % |
| Default Sort Direction | Ascending / Descending | Descending |
| Default Grouping | See §4.4 | Type |
| Show Heat Map | On / Off | Off |
| Heat Map Intensity | Low / Medium / High | Medium |
| Row Height | Compact (20px) / Normal (28px) / Comfortable (36px) | Normal |
| Show Process Icons | On / Off | On |
| Show Status Icons | On / Off | On |
| Show Inline Graphs | Off / CPU / Memory / Disk / GPU / All | Off |
| Inline Graph Width | 50–200px | 100px |
| Highlight New Processes | On / Off + color + duration | On / Green / 3s |
| Highlight Ending Processes | On / Off + color + duration | On / Red / 2s |
| Show Suspended Processes | On / Off | On |
| Show System Processes | On / Off | On |
| Show Service Host Details | On / Off (expand svchost services) | On |
| Process Name Display | Executable Name / Friendly Name / Both | Friendly Name |
| PID Display | Decimal / Hexadecimal / Both | Decimal |
| Memory Display Unit | Auto / KB / MB / GB | Auto |
| Rate Display Unit | Auto / KB/s / MB/s / GB/s | Auto |
| Cumulative I/O | Since Process Start / Since Tab Opened / Since Last Reset | Since Process Start |

### 17.3 Performance Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Update Speed | Paused / Low (4s) / Normal (1s) / High (0.5s) | Normal |
| Graph Time Range | 60s / 5m / 15m / 1h / 6h / 24h | 60s |
| Graph Line Width | 1px / 2px / 3px | 2px |
| Graph Anti-Aliasing | On / Off | On |
| Graph Fill Opacity | 0–100% | 30% |
| Graph Background | Solid / Grid / None | Grid |
| Grid Line Style | Solid / Dashed / Dotted | Dashed |
| Grid Line Color | Color picker | #333333 |
| Graph Interpolation | Linear / Bézier / Step | Bézier |
| Show Axis Labels | On / Off | On |
| Show Current Value | On / Off (show live value on graph) | On |
| Show Min/Max/Avg | On / Off | Off |
| CPU Graph Default | Overall / Per-Core / Kernel+User | Overall |
| CPU Show Temperature | On / Off (overlay) | Off |
| CPU Show Clock Speed | On / Off (overlay) | Off |
| Memory Show Composition | On / Off | On |
| Disk Show Latency | On / Off (overlay) | Off |
| GPU Show All Engines | On / Off | Off |
| Network Scale | Auto / Fixed (specify max) | Auto |
| Hardware Counters | On / Off (requires admin) | Off |
| Show Sidebar Sparklines | On / Off (mini-graphs in selector) | On |

#### 17.3.1 Graph Color Configuration

| Metric | Default Color | Configurable |
|--------|--------------|--------------|
| CPU Overall | `#4fc3f7` (Light Blue) | Yes |
| CPU Kernel | `#ef5350` (Red) | Yes |
| CPU User | `#42a5f5` (Blue) | Yes |
| Memory In Use | `#ab47bc` (Purple) | Yes |
| Memory Standby | `#66bb6a` (Green) | Yes |
| Memory Modified | `#ffa726` (Orange) | Yes |
| Memory Free | `#bdbdbd` (Gray) | Yes |
| Disk Read | `#29b6f6` (Sky Blue) | Yes |
| Disk Write | `#ef5350` (Red) | Yes |
| GPU 3D | `#66bb6a` (Green) | Yes |
| GPU Copy | `#ffa726` (Orange) | Yes |
| GPU Decode | `#ab47bc` (Purple) | Yes |
| GPU Encode | `#26c6da` (Cyan) | Yes |
| GPU Compute | `#ec407a` (Pink) | Yes |
| GPU VRAM | `#7e57c2` (Deep Purple) | Yes |
| Network Send | `#42a5f5` (Blue) | Yes |
| Network Receive | `#ffa726` (Orange) | Yes |
| Temperature | `#ef5350` (Red) | Yes |
| Power | `#ffee58` (Yellow) | Yes |

### 17.4 App History Settings

| Setting | Options | Default |
|---------|---------|---------|
| History Retention | 7 / 14 / 30 / 60 / 90 days | 30 days |
| Track Background Apps | On / Off | On |
| Track System Apps | On / Off | Off |
| Track Network by Connection Type | On / Off (separate metered) | On |
| Default Time Range | Today / 7 Days / 30 Days | 7 Days |
| Auto-Delete Uninstalled App History | On / Off | On |
| Storage Location | Default / Custom path | Default |
| Max Database Size | 50 / 100 / 250 / 500 MB / Unlimited | 100 MB |

### 17.5 Startup Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Show System Services | On / Off | Off |
| Measure Boot Impact | On / Off (adds measurement overhead) | Off |
| Boot Timeline Detail | Minimal / Standard / Detailed | Standard |
| Boot History Retention | 7 / 30 / 90 days | 30 days |

### 17.6 Services Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Show Running Only | On / Off | Off |
| Show Driver Services | On / Off | Off |
| Show Dependency Viewer | Embedded / Dialog / Off | Dialog |
| Group by Status | On / Off | Off |
| Confirm Service State Changes | On / Off | On |

### 17.7 Files In Use Settings

| Setting | Options | Default |
|---------|---------|---------|
| Auto-Refresh Interval | 1s / 5s / 15s / 30s / Manual | 5s |
| Show System Files | On / Off | Off |
| Show Kernel Handles | On / Off | Off |
| Max Results | 1000 / 5000 / 10000 / Unlimited | 5000 |
| Watch List Notification | Toast / Sound / Both / None | Toast |

### 17.8 Resource Unlocking Settings

| Setting | Options | Default |
|---------|---------|---------|
| Require Confirmation | Always / When System Process / Never | Always |
| Auto-Create Backup | On / Off (copy file before unlock) | Off |
| Create Process Dump Before Kill | On / Off | Off |
| Audit Log Location | Default / Custom path | Default |
| Max Audit Log Size | 10 / 50 / 100 MB | 50 MB |

### 17.9 Network Traffic Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Default View | Overview / Connections / Per-Process / DNS / Protocols | Overview |
| Connection Table Columns | Multi-select from all available | See §14.4 defaults |
| Show Reverse DNS | On / Off | On |
| Show GeoIP | On / Off | On |
| GeoIP Database | Built-in / MaxMind / Custom | Built-in |
| GeoIP Auto-Update | On / Off | On |
| DNS Query Logging | On / Off | On |
| DNS Log Retention | 1h / 6h / 24h / 7d / 30d | 24h |
| Protocol Detection | Off / Port-Based / Deep Inspection | Port-Based |
| Traffic Shaping | On / Off | Off |
| Packet Capture | Enabled / Disabled (requires admin) | Disabled |
| Capture Buffer Size | 1 / 10 / 50 / 100 / 500 MB | 10 MB |
| Capture Auto-Delete | On / Off | On |
| Bandwidth Quota | Off / Custom (specify GB/month) | Off |
| Quota Reset Day | 1–28 | 1 |
| Network Map Discovery | ARP / mDNS / UPnP / All / Off | All |
| Firewall Rules View | Show / Hide | Show |
| Connection Rate Limit Alert | Off / Threshold connections/sec | Off |
| Show Loopback Traffic | On / Off | Off |
| Resolve Port Names | On / Off | On |
| TLS Certificate Display | Subject Only / Full Chain / Off | Subject Only |
| Data Usage Persistence | Session / 7d / 30d / 90d | 30d |
| Speed Test Server | Auto / Custom URL | Auto |
| Metered Connection Detection | Auto / Manual | Auto |
| Traffic Classification | On / Off | On |

### 17.10 Energy & Power Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Default View | Overview / By Process / By Component / Battery / Thermal | Overview |
| Power Estimation Method | Auto / RAPL / ACPI / Software Estimation | Auto |
| Temperature Unit | Celsius / Fahrenheit | Celsius |
| Fan Control | Read-Only / Full Control (requires admin) | Read-Only |
| Fan Profiles | Silent / Balanced / Performance / Custom | Balanced |
| Custom Fan Curves | Visual editor | — |
| Thermal Alert Threshold | Per-sensor configurable (°C) | Auto (use hw thresholds) |
| Carbon Tracking | On / Off | Off |
| Carbon Intensity Source | Electricity Maps / WattTime / Custom API / Manual | Electricity Maps |
| Electricity Rate | Custom ($/kWh, €/kWh, etc.) | 0.12 $/kWh |
| Energy History Retention | 7d / 30d / 90d / 365d | 90d |
| Battery Health Report | On / Off | On |
| Battery Calibration Reminder | On / Off | On |
| Wake Lock Alerts | On / Off | On |
| Power Plan Quick-Switch | Show / Hide (in status bar) | Show |
| Scheduled Power Profiles | On / Off | Off |
| Per-Process Energy Tracking | On / Off | On |
| Show Carbon Dashboard | On / Off | Off |
| Efficiency Scoring | On / Off | On |
| Thermal Map Style | Schematic / Table / Both | Schematic |
| Show Peripheral Power | On / Off | Off |
| Battery Cycle Warning Threshold | 300 / 500 / 800 / 1000 / Custom | 500 |

### 17.11 Audio Tab Settings

| Setting | Options | Default |
|---------|---------|---------|
| Default View | Output / Input / Streams / Routing / Devices | Output |
| Meter Type | Peak / PPM Type I / PPM Type II / VU / True Peak | Peak |
| Meter Scale | dBFS / dBu / VU | dBFS |
| Meter Ballistics | Fast / Medium / Slow | Fast |
| Peak Hold Time | Off / 1s / 3s / 5s / Infinite | 3s |
| Peak Hold Decay | Instant / Gradual | Gradual |
| Spectrum Analyzer | On / Off | Off |
| Spectrum FFT Size | 1024 / 2048 / 4096 / 8192 / 16384 | 4096 |
| Spectrum Window | Hann / Hamming / Blackman / Flat-Top | Hann |
| Spectrum Mode | 1/3 Octave / FFT / Waterfall | 1/3 Octave |
| Show LUFS Metering | On / Off | Off |
| Show Waveform | On / Off | Off |
| Audio Routing | Simple / Matrix / Diagram | Diagram |
| Virtual Audio Cables | On / Off (requires admin) | Off |
| Per-Stream Volume Control | On / Off | On |
| Per-Stream Effects | On / Off | Off |
| MIDI Monitoring | On / Off | Off |
| Audio Event Logging | On / Off | On |
| Audio Event Log Retention | 1h / 6h / 24h / 7d | 24h |
| Glitch Detection Sensitivity | Low / Medium / High | Medium |
| Latency Display | On / Off | On |
| Show DSP Load | On / Off | On |
| Playback Test Tone Type | Sine / Pink Noise / White Noise / Sweep | Sine |
| Playback Test Frequency | 100–10000 Hz | 1000 |
| Playback Test Volume | -40 to 0 dBFS | -20 |
| Show Spatial Audio Controls | On / Off | On |
| Show MIDI Section | On / Off (hidden if no MIDI devices) | Auto |
| Bluetooth Codec Display | On / Off | On |
| Recording Format | WAV / FLAC / OGG | WAV |
| Recording Sample Rate | Device / 44100 / 48000 / 96000 | Device |
| Recording Bit Depth | 16 / 24 / 32-float | 24 |
| Auto-Switch on Headphones | On / Off | On |

### 17.12 Notification Settings

| Setting | Options | Default |
|---------|---------|---------|
| High CPU Alert | Off / Threshold % + Duration | Off |
| High Memory Alert | Off / Threshold % | Off |
| High Disk Alert | Off / Threshold % | Off |
| High GPU Alert | Off / Threshold % | Off |
| High Temperature Alert | Off / Threshold °C | Off |
| Process Crash Alert | On / Off | Off |
| Service Failure Alert | On / Off | Off |
| New Process Alert | On / Off (useful for security) | Off |
| Alert Sound | None / System / Custom file | None |
| Alert Method | Toast / Status Bar / Dialog / All | Status Bar |
| Alert Cooldown | 5s / 15s / 30s / 60s | 30s |

### 17.13 Data Export Settings

| Setting | Options | Default |
|---------|---------|---------|
| Default Export Format | CSV / JSON / XML / TSV / HTML | CSV |
| Include Headers | On / Off | On |
| Timestamp Format | ISO 8601 / Unix / Local | ISO 8601 |
| Decimal Separator | . / , | . |
| Export Encoding | UTF-8 / UTF-16 / ASCII | UTF-8 |
| Auto-Export Interval | Off / 1m / 5m / 15m / 1h | Off |
| Auto-Export Path | Directory picker | — |
| Include Performance Snapshots | On / Off | Off |

### 17.14 Plugin Settings

| Setting | Options | Default |
|---------|---------|---------|
| Enable Plugins | On / Off | On |
| Plugin Directory | Default / Custom path | Default |
| Auto-Update Plugins | On / Off | On |
| Plugin Sandboxing | Strict / Permissive / Off | Strict |
| Show Plugin Tabs | On / Off | On |

### 17.15 Keyboard & Mouse Settings

| Setting | Options | Default |
|---------|---------|---------|
| Keyboard Shortcut Scheme | Default / Custom / Vim / Emacs | Default |
| Single-Click Action | Select / Expand | Select |
| Double-Click Action | Properties / Open File Location / None | Properties |
| Middle-Click Action | End Task / None | None |
| Scroll Wheel on Graph | Zoom / Scroll | Zoom |
| Custom Shortcuts | Full shortcut editor | — |

### 17.16 Accessibility Settings

| Setting | Options | Default |
|---------|---------|---------|
| High Contrast Mode | System / Force On / Force Off | System |
| Font Size | 10 / 12 / 14 / 16 / 18 / Custom | 12 |
| Font Family | System / Monospace / Custom | System |
| Screen Reader Announcements | Minimal / Standard / Verbose | Standard |
| Reduce Motion | On / Off | System preference |
| Color Blind Mode | Off / Protanopia / Deuteranopia / Tritanopia | Off |
| Focus Indicator Style | Default / High Visibility | Default |
| Tab Key Navigation | Standard / Graph Navigation | Standard |

### 17.17 Advanced Settings

| Setting | Options | Default |
|---------|---------|---------|
| Sampling Rate | 0.5 Hz / 1 Hz / 2 Hz / 5 Hz / 10 Hz | 1 Hz |
| Ring Buffer Size | 60 / 300 / 900 / 3600 / 21600 | 300 |
| Enable ETW/eBPF Tracing | On / Off (requires admin, higher overhead) | Off |
| Debug Logging | Off / Info / Debug / Trace | Off |
| Log File Path | Default / Custom | Default |
| Max Log File Size | 10 / 50 / 100 / 500 MB | 50 MB |
| Process Scan Method | Snapshot / Incremental | Incremental |
| GPU Monitoring Backend | Auto / NVML / ADL / IGCL / D3DKMT | Auto |
| Temperature Source | Auto / ACPI / WMI / Direct MSR | Auto |
| Show Debug Tab | On / Off (shows internal metrics) | Off |
| Enable Performance Counters | On / Off | On |
| Hardware Counter Access | Auto / Require Admin | Auto |
| Config File Location | Shows path, [Open] [Reset] | — |
| Import Config | File picker | — |
| Export Config | File picker | — |

---

## 18. Keyboard Shortcuts

### 18.1 Global

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+Esc` | Open/Focus Task Manager |
| `Ctrl+Alt+Del` → Task Manager | Alternative launch |
| `Escape` | Close / Minimize (configurable) |
| `F5` | Refresh now |
| `Ctrl+F` | Find / Search |
| `F1` | Help |
| `Ctrl+,` | Open Settings |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Alt+1..0` | Switch to tab 1–10 |
| `Ctrl+C` | Copy selected item info |
| `Ctrl+A` | Select all |
| `Ctrl+E` | Export current view |

### 18.2 Processes Tab

| Shortcut | Action |
|----------|--------|
| `Delete` | End selected task (with confirmation) |
| `Shift+Delete` | End process tree (with confirmation) |
| `Ctrl+R` | Restart selected process |
| `Space` | Suspend / Resume toggle |
| `Enter` | Open properties |
| `Ctrl+G` | Cycle grouping mode |
| `Ctrl+H` | Toggle heat map |
| `Ctrl+T` | Toggle process tree view |

### 18.3 Performance Tab

| Shortcut | Action |
|----------|--------|
| `Ctrl+P` | Pause / Resume graphs |
| `+` / `-` | Zoom in / out on time axis |
| `Home` | Reset zoom to default |
| `Ctrl+S` | Screenshot current graph |
| Arrow Keys | Navigate between resources in sidebar |

### 18.4 Network Traffic Tab

| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | Toggle network topology map |
| `Ctrl+D` | Toggle DNS query log |
| `Ctrl+K` | Start / stop packet capture |
| `Ctrl+B` | Toggle bandwidth monitor |
| `Ctrl+L` | Clear connection log |
| `F6` | Run connectivity check |
| `Ctrl+Shift+T` | Trace route to selected connection |
| `Ctrl+Shift+P` | Ping selected remote |

### 18.5 Energy & Power Tab

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+E` | Cycle power profile |
| `Ctrl+B` | Generate battery health report |
| `Ctrl+T` | Toggle thermal map view |
| `F7` | Toggle fan control panel |
| `Ctrl+Shift+C` | Toggle carbon footprint dashboard |
| Arrow Keys | Navigate between components in sidebar |

### 18.6 Audio Tab

| Shortcut | Action |
|----------|--------|
| `Ctrl+M` | Mute / unmute default output |
| `Ctrl+Shift+M` | Mute / unmute default input |
| `Ctrl+Shift+A` | Open spectrum analyzer |
| `Ctrl+Shift+R` | Open audio routing matrix |
| `F8` | Play test tone |
| `Ctrl+Shift+D` | Run audio diagnostics |
| `Ctrl+Shift+L` | Toggle audio event log |
| Arrow Keys | Navigate between devices/streams |

---

## 19. Accessibility

### 19.1 Screen Reader Support

- All tables are proper ARIA grids with row/column headers.
- Graph data is exposed as a navigable data table for screen readers.
- Live regions announce critical changes (high CPU, process end, service failure).
- Every action and control has an accessible label.
- Status bar values are announced periodically (configurable interval).

### 19.2 Keyboard Navigation

- Full tab-order navigation through all UI elements.
- Arrow keys navigate tables and trees.
- Keyboard-accessible graph navigation (left/right to move through time, up/down to switch series).
- Skip-navigation landmarks for each section.

### 19.3 Visual Accessibility

- Minimum 4.5:1 contrast ratio (WCAG AA) in all themes; 7:1 in high-contrast mode (AAA).
- No information conveyed by color alone—always paired with icon, pattern, or text.
- Color-blind safe palette option with distinct patterns for graph series.
- Configurable font size from 10pt to 24pt.
- Focus indicators visible at minimum 2px width.
- Reduced motion mode disables graph animations and transitions.

---

## 20. Security Model

### 20.1 Privilege Levels

| Level | Capabilities |
|-------|-------------|
| **User** | View own processes, view system metrics (read-only), manage own processes |
| **Elevated** | View all processes, end/suspend any process, manage services, unlock handles, view all sessions |
| **Kernel** | Access hardware counters, MSR reads, ETW sessions, eBPF probes |

### 20.2 Elevation Flow

1. Task Manager launches as user-level.
2. Privileged operations trigger a polkit/UAC prompt.
3. On approval, the elevated helper daemon is started (or an existing one is connected).
4. A session token is established over a secure local channel.
5. Token expires after configurable timeout (default: until Task Manager closes).

### 20.3 Data Protection

- No process data is transmitted off-device.
- Exported files respect the user's file permissions.
- Audit log is append-only and world-readable but only admin-writable.
- Process memory dumps are written with restricted ACLs (owner-only).
- Plugin sandboxing prevents plugins from accessing raw process data without permission.

---

## 21. Data Collection & Telemetry

### 21.1 Local Only by Default

- All data is collected and stored locally.
- No network communication unless the user explicitly enables crash reporting or usage analytics.
- Analytics are opt-in with full transparency on what is collected.

### 21.2 Optional Analytics (Opt-In)

| Data Point | Description |
|-----------|-------------|
| Feature Usage | Which tabs are used most, which columns are enabled |
| Crash Reports | Stack traces on Task Manager crashes |
| Error Reports | Service/device errors encountered |
| Performance | Task Manager's own CPU/RAM usage samples |

### 21.3 Data Retention

- Ring buffer data (graphs) is ephemeral—lost on close unless exported.
- App history database persists on disk (configurable retention, see §14.4).
- Boot history persists on disk (configurable retention, see §14.5).
- Audit log persists on disk (configurable max size, see §14.8).
- Login history is read from system event logs (no duplicate storage).

---

## 22. Performance Budget

### 22.1 Task Manager Resource Targets

| Metric | Target |
|--------|--------|
| CPU Usage (idle, 1 Hz) | < 0.5% |
| CPU Usage (active, 1 Hz) | < 1.5% |
| CPU Usage (high rate, 10 Hz) | < 5% |
| RAM (Compact mode) | < 25 MB |
| RAM (Standard mode) | < 50 MB |
| RAM (Advanced mode, all tabs) | < 100 MB |
| RAM (with 24h history loaded) | < 200 MB |
| Startup Time (cold) | < 500 ms |
| Startup Time (warm) | < 200 ms |
| Tab Switch | < 50 ms |
| Process List Render (1000 rows) | < 16 ms (60 FPS) |
| Graph Render (60s at 1 Hz) | < 4 ms per frame |
| Graph Render (24h at 1 Hz) | < 16 ms per frame |
| Disk I/O (sampling) | < 1 MB/s |
| GPU Usage (graphs) | < 2% |

### 22.2 Scalability Targets

| Scenario | Target |
|----------|--------|
| 10,000 processes | Smooth scrolling, < 100 ms sort |
| 100,000 open handles | Searchable in < 500 ms |
| 200 services | Instant rendering |
| 8 GPU adapters | All visible in sidebar |
| 16 CPU cores | Full per-core view |
| 256 CPU cores | Core heatmap view (per-core graph optional) |
| 64 disks | All listed with sparklines |
| 24 hours of history | Zoomable graph, < 16 ms render |

---

## 23. API Surface

### 23.1 Plugin API

Plugins can extend the Task Manager with:

```rust
pub trait TaskManagerPlugin {
    /// Plugin metadata
    fn info(&self) -> PluginInfo;
    
    /// Register custom tabs
    fn tabs(&self) -> Vec<TabDefinition>;
    
    /// Register custom columns for existing tabs
    fn columns(&self) -> Vec<ColumnDefinition>;
    
    /// Register custom context menu items
    fn context_menu_items(&self) -> Vec<MenuItemDefinition>;
    
    /// Called each sampling tick with current system state
    fn on_tick(&mut self, state: &SystemState);
    
    /// Handle custom actions
    fn on_action(&mut self, action: &Action) -> Result<(), PluginError>;
}
```

### 23.2 CLI Interface

```bash
# Launch with specific tab
liquide-taskmanager --tab=performance

# Export current process list
liquide-taskmanager --export processes --format csv --output /tmp/procs.csv

# Query specific process
liquide-taskmanager --query "pid=4200" --columns "name,cpu,mem,cmdline"

# Unlock a file
liquide-taskmanager --unlock "/path/to/locked/file"

# List open handles for a path
liquide-taskmanager --handles "/path/to/file"

# Measure boot time
liquide-taskmanager --boot-timeline --format json

# Start in floating widget mode
liquide-taskmanager --widget

# Headless monitoring (daemon mode)
liquide-taskmanager --daemon --alert-cpu 90 --alert-mem 95 --alert-email admin@local

# Network: list active connections
liquide-taskmanager --connections --format json --output /tmp/conns.json

# Network: DNS query log
liquide-taskmanager --dns-log --duration 60 --format csv

# Network: per-process bandwidth
liquide-taskmanager --bandwidth --sort recv --top 20

# Network: run speed test
liquide-taskmanager --speed-test --format json

# Network: traceroute
liquide-taskmanager --traceroute 8.8.8.8

# Network: start packet capture
liquide-taskmanager --capture --interface eth0 --filter "tcp port 443" --output /tmp/capture.pcapng --duration 60

# Network: block a remote address
liquide-taskmanager --firewall-block 203.0.113.50

# Network: usage report
liquide-taskmanager --net-usage --period monthly --format pdf --output /tmp/usage.pdf

# Energy: current power draw summary
liquide-taskmanager --power-summary

# Energy: battery health report
liquide-taskmanager --battery-report --format html --output /tmp/battery.html

# Energy: thermal sensor readings
liquide-taskmanager --thermals --format json

# Energy: per-process energy ranking
liquide-taskmanager --energy-top --top 20

# Energy: switch power profile
liquide-taskmanager --power-profile silent

# Energy: carbon footprint report
liquide-taskmanager --carbon-report --period weekly

# Audio: list devices
liquide-taskmanager --audio-devices --format json

# Audio: list active streams
liquide-taskmanager --audio-streams

# Audio: set default output
liquide-taskmanager --audio-default-output "Speakers (Realtek Audio)"

# Audio: set volume
liquide-taskmanager --audio-volume 75 --device "Speakers (Realtek Audio)"

# Audio: run diagnostics
liquide-taskmanager --audio-diag --test playback,latency

# Audio: record input for testing
liquide-taskmanager --audio-record --device "Microphone" --duration 10 --output /tmp/mic-test.wav
```

### 23.3 D-Bus / IPC Interface

For integration with other Liquide apps:

| Method | Description |
|--------|-------------|
| `GetProcessList()` | Returns array of process structs |
| `GetProcessInfo(pid)` | Returns detailed info for one process |
| `EndProcess(pid, force)` | Terminate a process |
| `SuspendProcess(pid)` | Suspend a process |
| `ResumeProcess(pid)` | Resume a suspended process |
| `GetSystemMetrics()` | Returns current CPU/RAM/Disk/GPU/Net |
| `GetPerformanceHistory(resource, timerange)` | Returns graph data |
| `GetServices()` | Returns service list |
| `SetServiceState(name, state)` | Start/Stop/Restart a service |
| `GetOpenHandles(path?)` | Returns open file handles |
| `CloseHandle(pid, handle)` | Force-close a handle |
| `GetBootTimeline()` | Returns boot timing data |
| `Subscribe(event_filter)` | Stream of real-time events |
| `GetNetworkConnections(filter?)` | Returns active TCP/UDP connections with stats |
| `GetNetworkBandwidth(interface?)` | Returns per-interface bandwidth usage |
| `GetDnsQueryLog(count?, filter?)` | Returns recent DNS query log entries |
| `GetNetworkInterfaces()` | Returns all network interfaces with status and config |
| `SetProcessBandwidthLimit(pid, upload_bps, download_bps)` | Set bandwidth limit for a process |
| `BlockRemoteAddress(address, protocol?)` | Create firewall rule blocking an address |
| `RunSpeedTest(server?)` | Trigger bandwidth speed test and return results |
| `TracerouteTo(host)` | Run traceroute and return hop data |
| `StartPacketCapture(interface, filter?, duration?)` | Start packet capture session |
| `StopPacketCapture(session_id)` | Stop a running capture |
| `GetNetworkUsageHistory(interface?, period)` | Returns historical bandwidth data |
| `GetNetTopology()` | Returns discovered network topology map |
| `GetEnergyOverview()` | Returns system-wide power draw breakdown |
| `GetProcessEnergy(pid?)` | Returns per-process energy consumption |
| `GetBatteryStatus()` | Returns battery level, health, charge rate, etc. |
| `GetBatteryHealthReport()` | Generates comprehensive battery health report |
| `GetThermalSensors()` | Returns all temperature sensor readings |
| `GetFanStatus()` | Returns fan speeds, modes, and curves |
| `SetFanProfile(profile)` | Switch fan profile (silent/balanced/performance/custom) |
| `SetFanSpeed(fan_id, speed_percent)` | Manually set fan speed (requires admin) |
| `GetPowerProfile()` | Returns active power profile and settings |
| `SetPowerProfile(profile_name)` | Switch power profile |
| `GetCarbonFootprint(period)` | Returns carbon emission estimates |
| `GetWakeLocks()` | Returns list of active wake locks |
| `ReleaseWakeLock(pid, lock_id)` | Force-release a wake lock |
| `GetEnergyHistory(period)` | Returns historical energy consumption data |
| `GetAudioDevices()` | Returns all audio input/output devices |
| `GetAudioStreams()` | Returns active audio streams with levels |
| `SetAudioVolume(device_id, volume)` | Set device volume |
| `SetAudioMute(device_id, muted)` | Mute/unmute a device |
| `SetDefaultAudioDevice(device_id, role)` | Set default audio device |
| `MoveAudioStream(stream_id, device_id)` | Redirect audio stream to different device |
| `GetAudioRouting()` | Returns current audio routing matrix |
| `SetStreamVolume(stream_id, volume)` | Set per-stream volume |
| `GetAudioMetrics()` | Returns latency, glitch count, buffer stats |
| `GetSpectrumData(device_id)` | Returns current FFT spectrum data |
| `GetAudioEventLog(count?)` | Returns recent audio system events |
| `RunAudioDiagnostic(test_name)` | Run specific audio diagnostic test |
| `GetMidiDevices()` | Returns connected MIDI devices |

### 23.4 Signals / Events

Subscribable event stream for other applications:

| Event | Payload |
|-------|---------|
| `ProcessStarted` | pid, name, cmdline, user, ppid |
| `ProcessEnded` | pid, name, exit_code, signal |
| `ProcessNotResponding` | pid, name, duration |
| `HighCpuUsage` | pid, name, cpu_percent, duration |
| `HighMemoryUsage` | total_percent, available_mb |
| `ServiceStateChanged` | service_name, old_state, new_state |
| `DiskSpaceLow` | mount_point, free_bytes, free_percent |
| `TemperatureWarning` | sensor, temperature_c, threshold_c |
| `BatteryLow` | percent, estimated_minutes |
| `NewUserSession` | user, session_id, session_type |
| `UserSessionEnded` | user, session_id, reason |
| `HandleLockConflict` | path, processes[] |
| `NetworkConnectionOpened` | pid, process, protocol, local_addr, remote_addr |
| `NetworkConnectionClosed` | pid, process, protocol, local_addr, remote_addr, bytes_sent, bytes_recv, duration |
| `HighBandwidthUsage` | pid, process, rate_bps, direction, duration |
| `NetworkInterfaceChanged` | interface, old_state, new_state, speed |
| `DnsResolutionFailed` | process, domain, error, dns_server |
| `SuspiciousConnection` | pid, process, remote_addr, remote_geo, reason |
| `BandwidthQuotaWarning` | interface, used_bytes, quota_bytes, percent |
| `FirewallRuleTriggered` | rule_name, direction, action, remote_addr, port, process |
| `PacketCaptureComplete` | session_id, file_path, packet_count, duration |
| `WifiSignalWeak` | interface, ssid, signal_dbm, quality_percent |
| `PowerDrawHigh` | component, watts, threshold_w |
| `ThermalThrottling` | component, temperature_c, throttle_type |
| `FanFailure` | fan_id, fan_name, expected_rpm, actual_rpm |
| `BatteryCritical` | percent, estimated_minutes, discharge_rate_mw |
| `BatteryHealthDegraded` | health_percent, cycle_count, recommendation |
| `PowerSourceChanged` | old_source, new_source, battery_percent |
| `WakeLockAcquired` | pid, process, lock_type, reason |
| `WakeLockReleased` | pid, process, lock_type, duration_held |
| `PowerProfileChanged` | old_profile, new_profile, trigger |
| `CarbonIntensityChanged` | old_gco2_kwh, new_gco2_kwh, region |
| `AudioDeviceConnected` | device_id, device_name, device_type, interface |
| `AudioDeviceDisconnected` | device_id, device_name, device_type |
| `AudioDefaultChanged` | device_id, device_name, role, previous_device |
| `AudioStreamStarted` | stream_id, pid, process, direction, device, format |
| `AudioStreamEnded` | stream_id, pid, process, duration, glitch_count |
| `AudioGlitch` | device_id, device_name, glitch_type, stream_id, pid |
| `AudioVolumeChanged` | device_id, old_volume, new_volume, source |
| `AudioExclusiveMode` | device_id, pid, process, acquired_or_released |
| `AudioClipping` | device_id, peak_dbfs, duration_ms |
| `MidiDeviceConnected` | device_id, device_name, interface |

---

## Appendix A: File Formats

### A.1 Config File

Location: `~/.config/liquide/task-manager/config.toml`

```toml
[general]
default_tab = "processes"
view_mode = "standard"
always_on_top = false
confirm_end_task = true
language = "system"

[processes]
visible_columns = ["name", "pid", "status", "cpu_percent", "mem_working", "disk_read", "disk_write", "gpu_percent", "cmdline", "user", "elevated"]
sort_column = "cpu_percent"
sort_direction = "descending"
grouping = "type"
heat_map = false
row_height = "normal"
show_process_icons = true
highlight_new = true
highlight_new_color = "#4caf50"
highlight_new_duration_ms = 3000

[performance]
update_interval_ms = 1000
graph_time_range_s = 60
graph_line_width = 2
graph_interpolation = "bezier"
graph_fill_opacity = 30
show_sidebar_sparklines = true

[performance.colors]
cpu = "#4fc3f7"
cpu_kernel = "#ef5350"
memory_in_use = "#ab47bc"
memory_standby = "#66bb6a"
disk_read = "#29b6f6"
disk_write = "#ef5350"
gpu_3d = "#66bb6a"
network_send = "#42a5f5"
network_recv = "#ffa726"

[app_history]
retention_days = 30
track_background = true
max_db_size_mb = 100

[startup]
show_system_services = false
boot_history_days = 30

[services]
show_running_only = false
show_drivers = false
confirm_state_changes = true

[files_in_use]
refresh_interval_ms = 5000
show_system = false
max_results = 5000

[unlock]
require_confirmation = "always"
audit_log_max_mb = 50

[network_traffic]
default_view = "overview"
show_reverse_dns = true
show_geoip = true
geoip_database = "built-in"
dns_query_logging = true
dns_log_retention_hours = 24
protocol_detection = "port-based"
traffic_shaping = false
packet_capture = false
capture_buffer_mb = 10
bandwidth_quota_gb = 0
quota_reset_day = 1
network_map_discovery = "all"
show_firewall_rules = true
show_loopback = false
resolve_port_names = true
tls_certificate_display = "subject"
data_usage_retention_days = 30
metered_detection = "auto"
traffic_classification = true

[energy]
default_view = "overview"
power_estimation = "auto"
temperature_unit = "celsius"
fan_control = "read-only"
fan_profile = "balanced"
carbon_tracking = false
carbon_source = "electricity-maps"
electricity_rate_per_kwh = 0.12
energy_history_days = 90
battery_health_report = true
battery_calibration_reminder = true
wake_lock_alerts = true
power_plan_quick_switch = true
per_process_energy = true
efficiency_scoring = true
thermal_map_style = "schematic"
battery_cycle_warning = 500

[audio]
default_view = "output"
meter_type = "peak"
meter_scale = "dbfs"
peak_hold_time_s = 3
peak_hold_decay = "gradual"
spectrum_analyzer = false
spectrum_fft_size = 4096
spectrum_window = "hann"
spectrum_mode = "1/3-octave"
show_lufs = false
show_waveform = false
routing_view = "diagram"
virtual_audio_cables = false
per_stream_volume = true
per_stream_effects = false
midi_monitoring = false
event_logging = true
event_log_retention_hours = 24
glitch_sensitivity = "medium"
show_latency = true
show_dsp_load = true
test_tone_type = "sine"
test_tone_frequency_hz = 1000
test_tone_volume_dbfs = -20
show_spatial_controls = true
show_midi = "auto"
bluetooth_codec_display = true
recording_format = "wav"
recording_sample_rate = "device"
recording_bit_depth = 24
auto_switch_headphones = true

[notifications]
high_cpu_threshold = 0
high_memory_threshold = 0
high_temp_threshold = 0
alert_method = "status_bar"
alert_cooldown_s = 30

[export]
format = "csv"
encoding = "utf-8"
timestamp_format = "iso8601"

[advanced]
sampling_rate_hz = 1.0
ring_buffer_size = 300
gpu_backend = "auto"
temp_source = "auto"
debug_logging = "off"
enable_etw = false
```

### A.2 Export File Formats

#### CSV

```csv
"Name","PID","Status","CPU %","Memory (MB)","Disk Read (KB/s)","Disk Write (KB/s)","GPU %","Command Line","User"
"firefox.exe",4200,"Running",12.4,842.3,1024,256,8.2,"C:\Program Files\Firefox\firefox.exe --profile default","john"
```

#### JSON

```json
{
  "timestamp": "2026-02-12T14:30:00Z",
  "processes": [
    {
      "name": "firefox.exe",
      "pid": 4200,
      "status": "Running",
      "cpu_percent": 12.4,
      "memory_working_mb": 842.3,
      "disk_read_kbs": 1024,
      "disk_write_kbs": 256,
      "gpu_percent": 8.2,
      "command_line": "C:\\Program Files\\Firefox\\firefox.exe --profile default",
      "user": "john"
    }
  ]
}
```

---

## Appendix B: Implementation Priority

### Phase 1 — MVP

- Processes tab (core columns, end task, sort, search)
- Performance tab (CPU, Memory graphs and stats)
- Basic settings (update speed, column visibility)

### Phase 2 — Core Features

- Performance tab (Disk, GPU, Network)
- Services tab
- Startup tab
- Process Tree tab
- App History tab
- Energy tab (battery status, per-component power)
- Audio tab (devices, streams, VU meters)
- Full settings UI

### Phase 3 — Advanced

- Files In Use tab
- Resource Unlocking tab
- Users & Sessions tab
- Devices tab
- Network Traffic tab (connections, per-process, DNS, protocols)
- Energy tab (thermal management, fan control, carbon tracking)
- Audio tab (routing matrix, effects chain, spectrum analyzer, MIDI)
- Plugin API
- CLI interface

### Phase 4 — Polish

- Floating widget
- Boot timeline visualization
- Service dependency graph
- IPC/D-Bus API
- Advanced GPU stats (frame time, per-engine)
- Hardware performance counters
- Network Traffic tab (packet capture, traffic shaping, topology map, firewall, bandwidth quotas)
- Energy tab (battery health reports, power profiles scheduling, energy history, efficiency scoring)
- Audio tab (spatial audio, diagnostics suite, device hardware detail, DSP load analysis)
- Accessibility audit and fixes

---

## Appendix C: Platform-Specific Notes

### C.1 Linux

- Process data from `/proc/<pid>/stat`, `/proc/<pid>/status`, `/proc/<pid>/io`, `/proc/<pid>/fd/`
- GPU data from NVML (NVIDIA), `/sys/class/drm/` (AMD/Intel), or `amdgpu_pm_info`
- Services from systemd D-Bus interface (`org.freedesktop.systemd1`)
- Temperatures from `/sys/class/hwmon/` or `lm-sensors`
- File handles from `/proc/<pid>/fd/` and `/proc/locks`
- Boot timeline from `systemd-analyze`
- Network connections from `/proc/net/tcp`, `/proc/net/udp`, Netlink sockets (SOCK_DIAG)
- Firewall rules from nftables / iptables via netfilter API
- Packet capture via AF_PACKET sockets or libpcap
- Network interfaces from `/sys/class/net/`, Netlink (RTM_GETLINK)
- DNS queries via NSS hooks or eBPF tracing of `getaddrinfo`
- Power/energy data from RAPL via `/sys/class/powercap/intel-rapl/`, `perf_event` subsystem
- Battery data from `/sys/class/power_supply/`
- Fan speed from `/sys/class/hwmon/` fan* attributes
- Audio devices from PipeWire / PulseAudio / ALSA (via D-Bus or client libraries)
- Audio streams from PipeWire session manager or `pactl`
- MIDI from ALSA sequencer (`/dev/snd/seq`) or PipeWire MIDI

### C.2 Windows

- Process data from `NtQuerySystemInformation`, `GetProcessTimes`, PDH counters
- GPU data from `D3DKMTQueryStatistics`, NVML, ADL
- Services from SCM API (`EnumServicesStatusEx`)
- ETW for detailed tracing
- File handles from `NtQuerySystemInformation(SystemHandleInformation)`
- Boot timeline from ETW boot trace + Windows Performance Recorder
- Network connections from `GetExtendedTcpTable` / `GetExtendedUdpTable` (IP Helper API)
- Per-connection stats from ETW (Microsoft-Windows-TCPIP provider)
- Firewall rules from Windows Filtering Platform (WFP) / HNetCfg / `netsh advfirewall`
- Packet capture via ETW (Microsoft-Windows-NDIS-PacketCapture, pktmon)
- Network interfaces from IP Helper API (`GetAdaptersAddresses`)
- Wi-Fi from Native Wifi API (`WlanGetAvailableNetworkList`, `WlanQueryInterface`)
- DNS query tracing from ETW (Microsoft-Windows-DNS-Client provider)
- Power/energy from IOCTL_BATTERY_QUERY_STATUS, PDH counters, Intel Power Gadget / RAPL MSRs
- Battery data from `IOCTL_BATTERY_*`, WMI (`Win32_Battery`)
- Temperature from WMI ACPI thermal zone, OHM/LibreHardwareMonitor interop
- Fan data from ACPI / WMI vendor namespaces (Dell/HP/Lenovo/ASUS EC interface)
- Audio devices from Windows Audio Session API (WASAPI) / MMDevice API
- Audio streams from WASAPI `IAudioSessionManager2`, per-session metering
- Audio effects from Audio Processing Objects (APO) registry
- MIDI from Windows MIDI Services / `midiIn*` / `midiOut*` API

### C.3 macOS

- Process data from `sysctl`, `proc_pidinfo`, `libproc`
- GPU data from IOKit / Metal Performance Shaders
- Services from `launchctl` / XPC
- Temperatures from IOKit SMC
- File handles from `proc_pidinfo(PROC_PIDLISTFDS)`
- Boot timeline from `log show --predicate 'process == "kernel"'`
- Network connections from `proc_pidinfo(PROC_PIDLISTFDS)` + `getpeername`, Network Extension framework
- Firewall rules from `pfctl` (pf firewall) / Application Firewall (`/usr/libexec/ApplicationFirewall/socketfilterfw`)
- Packet capture via BPF (`/dev/bpfN`) or libpcap
- Network interfaces from `getifaddrs()`, System Configuration framework
- Wi-Fi from CoreWLAN framework (`CWInterface`)
- DNS query tracing from `dns_sd` API / mDNSResponder logs
- Power/energy from IOKit `IOPMPowerSource`, SMC keys, Intel Power Gadget (on Intel Macs)
- Apple Silicon power from IOReport framework (CPU/GPU/ANE power counters)
- Battery data from `IOPMPowerSource` IOKit service
- Temperature from IOKit SMC keys (`TC0P`, `TG0P`, etc.)
- Fan control from SMC fan keys (`F0Ac`, `F0Mn`, `F0Mx`)
- Audio devices from CoreAudio (`AudioObjectGetPropertyData`)
- Audio streams from CoreAudio HAL, AudioQueue / AVAudioEngine session tracking
- Audio effects from AudioUnit / AUGraph introspection
- MIDI from CoreMIDI framework (`MIDIGetNumberOfSources`, `MIDIGetNumberOfDestinations`)

---

*End of Specification*
