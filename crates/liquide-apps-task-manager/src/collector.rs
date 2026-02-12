//! Platform-agnostic data collection traits (spec sections 2.1-2.2).
//!
//! Each collector trait abstracts over platform-specific data sources
//! (procfs/sysfs on Linux, WMI/PDH/ETW on Windows, sysctl/IOKit on macOS)
//! and returns normalised types consumed by the aggregation layer.

use crate::audio::stream::AudioStream;
use crate::devices::DeviceInfo;
use crate::energy::process_energy::ProcessEnergyInfo;
use crate::network::connection::ConnectionInfo;
use crate::performance::*;
use crate::process::ProcessInfo;
use crate::services::ServiceInfo;

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
