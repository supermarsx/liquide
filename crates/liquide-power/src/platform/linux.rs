//! Linux power management implementation.
//!
//! Uses sysfs for battery info, dbus-send/systemctl commands for logind
//! integration, and xset for DPMS control.

use crate::{
    BatteryInfo, DisplayPower, IdleAction, InhibitGuard, PowerBackend, PowerError, PowerEvent,
    PowerState,
};
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

// ── Inhibit tracking ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InhibitKind {
    Sleep,
    DisplayOff,
}

struct InhibitEntry {
    id: u64,
    kind: InhibitKind,
    #[allow(unused)]
    reason: String,
    /// PID of the `systemd-inhibit` or `caffeinate` child process (if any).
    child_pid: Option<u32>,
}

// ── PowerManager ───────────────────────────────────────────────────────

pub struct PowerManager {
    next_id: u64,
    inhibits: Vec<InhibitEntry>,
    state: PowerState,
    display: DisplayPower,
    // Idle tracking — on Linux we track idle manually because X11's
    // XScreenSaverQueryInfo requires linking libXss. Instead we record
    // the last time the caller (the desktop session) told us about input.
    last_input: Instant,
    // Idle thresholds
    dim_timeout: Duration,
    off_timeout: Duration,
    suspend_timeout: Duration,
    fired_dim: bool,
    fired_off: bool,
    fired_suspend: bool,
    last_known_idle: Duration,
    // Battery change detection
    last_battery: Option<BatteryInfo>,
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            inhibits: Vec::new(),
            state: PowerState::Active,
            display: DisplayPower::On,
            last_input: Instant::now(),
            dim_timeout: Duration::MAX,
            off_timeout: Duration::MAX,
            suspend_timeout: Duration::MAX,
            fired_dim: false,
            fired_off: false,
            fired_suspend: false,
            last_known_idle: Duration::ZERO,
            last_battery: None,
        }
    }

    /// Call this whenever user input is detected to reset the idle timer.
    pub fn notify_user_input(&mut self) {
        self.last_input = Instant::now();
    }

    /// Try to read battery info from sysfs.
    fn read_sysfs_battery() -> Option<BatteryInfo> {
        // Look for /sys/class/power_supply/BAT0 (or BAT1, etc.)
        let ps_dir = "/sys/class/power_supply";
        let entries = fs::read_dir(ps_dir).ok()?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("BAT") {
                continue;
            }

            let base = entry.path();

            let read_file = |name: &str| -> Option<String> {
                fs::read_to_string(base.join(name))
                    .ok()
                    .map(|s| s.trim().to_owned())
            };

            let status = read_file("status").unwrap_or_default();
            let capacity_str = read_file("capacity").unwrap_or_default();
            let energy_now = read_file("energy_now")
                .and_then(|s| s.parse::<u64>().ok());
            let power_now = read_file("power_now")
                .and_then(|s| s.parse::<u64>().ok());

            let percent = capacity_str.parse::<u8>().unwrap_or(0).min(100);
            let charging = status == "Charging";

            let time_remaining_secs = match (energy_now, power_now) {
                (Some(e), Some(p)) if p > 0 && !charging => {
                    // energy is in microwatt-hours, power in microwatts
                    Some(((e as f64 / p as f64) * 3600.0) as u32)
                }
                _ => None,
            };

            return Some(BatteryInfo {
                present: true,
                charging,
                percent,
                time_remaining_secs,
            });
        }

        None
    }

    /// Spawn `systemd-inhibit` with the given lock type and return the child PID.
    fn spawn_inhibit(what: &str, reason: &str) -> Option<u32> {
        // systemd-inhibit --what=<what> --who=liquide --why=<reason> sleep infinity
        let child = Command::new("systemd-inhibit")
            .arg(format!("--what={what}"))
            .arg("--who=liquide")
            .arg(format!("--why={reason}"))
            .arg("sleep")
            .arg("infinity")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        Some(child.id())
    }

    /// Kill a previously spawned inhibit child.
    fn kill_inhibit(pid: u32) {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerBackend for PowerManager {
    fn battery_info(&self) -> Option<BatteryInfo> {
        Self::read_sysfs_battery()
    }

    fn power_state(&self) -> PowerState {
        self.state
    }

    fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError> {
        let arg = match state {
            DisplayPower::On => "on",
            DisplayPower::Dimmed => "on", // X11 DPMS has no "dim" — treat as on
            DisplayPower::Off => "off",
        };

        let status = Command::new("xset")
            .args(["dpms", "force", arg])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() => {
                self.display = state;
                Ok(())
            }
            Ok(_) => Err(PowerError::PlatformError("xset dpms failed".into())),
            Err(e) => Err(PowerError::PlatformError(format!("xset: {e}"))),
        }
    }

    fn inhibit_sleep(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        let child_pid = Self::spawn_inhibit("sleep", reason);
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::Sleep,
            reason: reason.to_owned(),
            child_pid,
        });
        Ok(InhibitGuard { id })
    }

    fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        let child_pid = Self::spawn_inhibit("idle", reason);
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::DisplayOff,
            reason: reason.to_owned(),
            child_pid,
        });
        Ok(InhibitGuard { id })
    }

    fn release_inhibit(&mut self, guard: InhibitGuard) {
        if let Some(pos) = self.inhibits.iter().position(|e| e.id == guard.id) {
            let entry = self.inhibits.remove(pos);
            if let Some(pid) = entry.child_pid {
                Self::kill_inhibit(pid);
            }
        }
    }

    fn suspend(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::Suspended;
        // Try logind first, fall back to systemctl.
        let result = Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager.Suspend",
                "boolean:true",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => {
                self.state = PowerState::Active;
                Ok(())
            }
            _ => {
                // Fallback
                let fallback = Command::new("systemctl")
                    .arg("suspend")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match fallback {
                    Ok(s) if s.success() => {
                        self.state = PowerState::Active;
                        Ok(())
                    }
                    _ => {
                        self.state = PowerState::Active;
                        Err(PowerError::PlatformError("suspend failed".into()))
                    }
                }
            }
        }
    }

    fn hibernate(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::Hibernated;
        let result = Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager.Hibernate",
                "boolean:true",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => {
                self.state = PowerState::Active;
                Ok(())
            }
            _ => {
                let fallback = Command::new("systemctl")
                    .arg("hibernate")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match fallback {
                    Ok(s) if s.success() => {
                        self.state = PowerState::Active;
                        Ok(())
                    }
                    _ => {
                        self.state = PowerState::Active;
                        Err(PowerError::PlatformError("hibernate failed".into()))
                    }
                }
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::ShuttingDown;
        let result = Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager.PowerOff",
                "boolean:true",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => Ok(()),
            _ => {
                let fallback = Command::new("systemctl")
                    .arg("poweroff")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match fallback {
                    Ok(s) if s.success() => Ok(()),
                    _ => {
                        self.state = PowerState::Active;
                        Err(PowerError::PlatformError("shutdown failed".into()))
                    }
                }
            }
        }
    }

    fn reboot(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::ShuttingDown;
        let result = Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager.Reboot",
                "boolean:true",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => Ok(()),
            _ => {
                let fallback = Command::new("systemctl")
                    .arg("reboot")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match fallback {
                    Ok(s) if s.success() => Ok(()),
                    _ => {
                        self.state = PowerState::Active;
                        Err(PowerError::PlatformError("reboot failed".into()))
                    }
                }
            }
        }
    }

    fn idle_duration(&self) -> Duration {
        self.last_input.elapsed()
    }

    fn set_idle_timeout(
        &mut self,
        display_dim: Duration,
        display_off: Duration,
        suspend: Duration,
    ) {
        self.dim_timeout = display_dim;
        self.off_timeout = display_off;
        self.suspend_timeout = suspend;
        self.fired_dim = false;
        self.fired_off = false;
        self.fired_suspend = false;
    }

    fn tick(&mut self) -> Vec<PowerEvent> {
        let mut events = Vec::new();
        let idle = self.idle_duration();

        if idle < self.last_known_idle {
            self.fired_dim = false;
            self.fired_off = false;
            self.fired_suspend = false;
        }
        self.last_known_idle = idle;

        if !self.fired_dim && idle >= self.dim_timeout {
            self.fired_dim = true;
            events.push(PowerEvent::IdleThresholdReached {
                kind: IdleAction::DimDisplay,
                after: idle,
            });
        }
        if !self.fired_off && idle >= self.off_timeout {
            self.fired_off = true;
            events.push(PowerEvent::IdleThresholdReached {
                kind: IdleAction::TurnOffDisplay,
                after: idle,
            });
        }
        if !self.fired_suspend && idle >= self.suspend_timeout {
            self.fired_suspend = true;
            events.push(PowerEvent::IdleThresholdReached {
                kind: IdleAction::Suspend,
                after: idle,
            });
        }

        // Battery change detection
        if let Some(bat) = self.battery_info() {
            let changed = match &self.last_battery {
                None => true,
                Some(prev) => {
                    prev.charging != bat.charging
                        || prev.percent != bat.percent
                        || prev.present != bat.present
                }
            };
            if changed {
                self.last_battery = Some(bat);
                events.push(PowerEvent::BatteryChanged(bat));
            }
        }

        events
    }
}
