//! Linux power management implementation.
//!
//! Uses sysfs for battery info, dbus-send/systemctl commands for logind
//! integration, and xset for DPMS control.

use crate::{
    BatteryInfo, DisplayPower, IdleAction, InhibitGuard, PowerBackend, PowerError, PowerEvent,
    PowerState,
};
use std::fs;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// ── Inhibit tracking ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InhibitKind {
    Sleep,
    DisplayOff,
}

struct InhibitEntry {
    id: u64,
    #[allow(unused)]
    kind: InhibitKind,
    #[allow(unused)]
    reason: String,
    /// The live `systemd-inhibit` child process holding the lock.
    ///
    /// Owning the `Child` (rather than a bare pid) is the invariant that
    /// guarantees we can both *kill* and *reap* the process — killing by
    /// pid alone would leave a zombie, and a bare pid could be reused by an
    /// unrelated process after the original exits.
    child: Child,
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
    // The binary used to acquire inhibit locks. Normally `systemd-inhibit`;
    // overridable in tests to inject a spawn failure (a non-existent binary).
    inhibit_bin: String,
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
            inhibit_bin: "systemd-inhibit".to_owned(),
        }
    }

    /// Override the binary used to acquire inhibit locks.
    ///
    /// Intended for tests: pointing this at a non-existent path forces the
    /// spawn in [`spawn_inhibit`](Self::spawn_inhibit) to fail so the
    /// fail-closed error path can be exercised deterministically.
    #[cfg(test)]
    fn set_inhibit_bin(&mut self, bin: impl Into<String>) {
        self.inhibit_bin = bin.into();
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
            let energy_now = read_file("energy_now").and_then(|s| s.parse::<u64>().ok());
            let power_now = read_file("power_now").and_then(|s| s.parse::<u64>().ok());

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

    /// Spawn `systemd-inhibit` with the given lock type and return the live child.
    ///
    /// Returns `Err(PowerError::PlatformError)` if the process cannot be
    /// spawned (e.g. `systemd-inhibit` missing). Callers MUST propagate this:
    /// returning `Ok` on spawn failure would falsely promise the system is
    /// inhibited while it remains free to sleep (fail-open).
    fn spawn_inhibit(&self, what: &str, reason: &str) -> Result<Child, PowerError> {
        // <inhibit_bin> --what=<what> --who=liquide --why=<reason> sleep infinity
        Command::new(&self.inhibit_bin)
            .arg(format!("--what={what}"))
            .arg("--who=liquide")
            .arg(format!("--why={reason}"))
            .arg("sleep")
            .arg("infinity")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                PowerError::PlatformError(format!("failed to spawn {}: {e}", self.inhibit_bin))
            })
    }

    /// Kill *and reap* an inhibit child so the lock is released and no zombie
    /// remains. Killing alone would leave a zombie until the parent exits.
    fn kill_and_reap(mut child: Child) {
        // SIGTERM lets `systemd-inhibit` drop the logind lock cleanly.
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        // Reap the process to avoid leaving a zombie.
        let _ = child.wait();
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PowerManager {
    /// Release every still-held inhibit on teardown.
    ///
    /// Without this, a session restart or crash-recovery path that drops the
    /// `PowerManager` with live inhibits would leave the `systemd-inhibit
    /// ... sleep infinity` children running forever, holding system-wide
    /// sleep/idle locks and leaking processes.
    fn drop(&mut self) {
        for entry in self.inhibits.drain(..) {
            Self::kill_and_reap(entry.child);
        }
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
        // Spawn first: on failure return Err *without* recording a phantom
        // inhibit, so the caller is never told sleep is inhibited when it isn't.
        let child = self.spawn_inhibit("sleep", reason)?;
        let id = self.next_id;
        self.next_id += 1;
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::Sleep,
            reason: reason.to_owned(),
            child,
        });
        Ok(InhibitGuard { id })
    }

    fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let child = self.spawn_inhibit("idle", reason)?;
        let id = self.next_id;
        self.next_id += 1;
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::DisplayOff,
            reason: reason.to_owned(),
            child,
        });
        Ok(InhibitGuard { id })
    }

    fn release_inhibit(&mut self, guard: InhibitGuard) {
        if let Some(pos) = self.inhibits.iter().position(|e| e.id == guard.id) {
            let entry = self.inhibits.remove(pos);
            Self::kill_and_reap(entry.child);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawn a harmless, long-lived child (`sleep infinity`) to stand in for
    /// the real `systemd-inhibit` process when testing release/teardown so the
    /// tests do not depend on logind being present in CI.
    fn spawn_dummy_child() -> Child {
        Command::new("sleep")
            .arg("infinity")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("`sleep` must be available on Linux CI")
    }

    /// Returns true if the given pid still exists (kill(pid, 0) succeeds).
    fn pid_alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    // ── e9-10: fail-closed on spawn failure ─────────────────────────────

    #[test]
    fn inhibit_sleep_errors_when_spawn_fails() {
        let mut pm = PowerManager::new();
        // Point at a binary that cannot exist so spawn() fails.
        pm.set_inhibit_bin("/nonexistent/liquide-no-such-inhibit-binary");
        let res = pm.inhibit_sleep("unit-test");
        assert!(
            matches!(res, Err(PowerError::PlatformError(_))),
            "spawn failure must surface as Err, not Ok (fail-open)"
        );
        // No phantom inhibit entry must be recorded on failure.
        assert!(pm.inhibits.is_empty());
    }

    #[test]
    fn inhibit_display_off_errors_when_spawn_fails() {
        let mut pm = PowerManager::new();
        pm.set_inhibit_bin("/nonexistent/liquide-no-such-inhibit-binary");
        let res = pm.inhibit_display_off("unit-test");
        assert!(matches!(res, Err(PowerError::PlatformError(_))));
        assert!(pm.inhibits.is_empty());
    }

    // ── e9-11: Drop / release kills and reaps the child ─────────────────

    #[test]
    fn release_inhibit_kills_and_reaps_child() {
        let mut pm = PowerManager::new();
        let child = spawn_dummy_child();
        let pid = child.id();
        let id = pm.next_id;
        pm.next_id += 1;
        pm.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::Sleep,
            reason: "unit-test".to_owned(),
            child,
        });
        assert!(pid_alive(pid), "dummy child should be alive before release");

        pm.release_inhibit(InhibitGuard { id });

        assert!(pm.inhibits.is_empty(), "entry must be removed on release");
        // After kill + reap the pid is gone (reaped) — no zombie left behind.
        // Poll briefly because signal delivery + reap is asynchronous.
        let mut alive = pid_alive(pid);
        for _ in 0..200 {
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            alive = pid_alive(pid);
        }
        assert!(!alive, "child must be killed and reaped after release");
    }

    #[test]
    fn drop_kills_remaining_inhibit_children() {
        let pid;
        {
            let mut pm = PowerManager::new();
            let child = spawn_dummy_child();
            pid = child.id();
            pm.inhibits.push(InhibitEntry {
                id: pm.next_id,
                kind: InhibitKind::Sleep,
                reason: "unit-test".to_owned(),
                child,
            });
            assert!(pid_alive(pid), "child alive before manager drop");
            // pm dropped here → Drop impl must kill + reap the child.
        }

        let mut alive = pid_alive(pid);
        for _ in 0..200 {
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            alive = pid_alive(pid);
        }
        assert!(!alive, "dropping PowerManager must kill remaining inhibits");
    }

    /// Structural invariant: an inhibit entry owns a `Child`, which is what
    /// makes kill + reap (and therefore the Drop guarantee) possible. A bare
    /// pid could not be reaped. This compiles only while the ownership holds.
    #[test]
    fn inhibit_entry_owns_child() {
        let child = spawn_dummy_child();
        let entry = InhibitEntry {
            id: 1,
            kind: InhibitKind::Sleep,
            reason: "structural".to_owned(),
            child,
        };
        // Move the owned Child out and reap it directly — proves ownership.
        PowerManager::kill_and_reap(entry.child);
    }
}
