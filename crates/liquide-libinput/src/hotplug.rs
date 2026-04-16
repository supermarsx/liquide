//! Device hotplug monitoring via inotify on `/dev/input/`.

use crate::classify::DeviceInfo;
use crate::error::Result;

/// Events produced by the hotplug monitor.
#[derive(Debug, Clone)]
pub enum HotplugEvent {
    /// A new input device appeared.
    DeviceAdded { info: DeviceInfo },
    /// An input device was removed.
    DeviceRemoved { path: String },
}

/// Watches `/dev/input/` for device additions and removals.
///
/// On Linux this would use inotify; the current implementation is a stub
/// that can be fleshed out once running on a real TTY.
pub struct HotplugMonitor {
    watching: bool,
}

impl HotplugMonitor {
    pub fn new() -> Self {
        Self { watching: false }
    }

    /// Begin monitoring `/dev/input/` for hotplug events.
    ///
    /// On non-Linux platforms this returns [`LibinputError::NotSupported`].
    pub fn start(&mut self) -> Result<()> {
        self.start_inner()
    }

    /// Non-blocking poll for the next hotplug event.
    ///
    /// Returns `None` when no events are pending or monitoring is not active.
    pub fn poll(&mut self) -> Option<HotplugEvent> {
        if !self.watching {
            return None;
        }
        // Stub: real implementation would read from inotify fd.
        None
    }

    /// Stop monitoring.
    pub fn stop(&mut self) {
        self.watching = false;
    }

    /// Returns `true` if the monitor is actively watching.
    pub fn is_watching(&self) -> bool {
        self.watching
    }

    // ── Platform-specific start ─────────────────────────────────────

    #[cfg(target_os = "linux")]
    fn start_inner(&mut self) -> Result<()> {
        // Stub: would call inotify_init1 + inotify_add_watch on /dev/input.
        tracing::info!("hotplug monitor: started watching /dev/input/");
        self.watching = true;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn start_inner(&mut self) -> Result<()> {
        Err(crate::error::LibinputError::NotSupported)
    }
}

impl Default for HotplugMonitor {
    fn default() -> Self {
        Self::new()
    }
}
