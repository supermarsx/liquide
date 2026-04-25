//! macOS power management implementation.
//!
//! Uses IOKit FFI for battery info, `pmset` for sleep/display,
//! `CGEventSourceSecondsSinceLastEventType` for idle detection,
//! and `caffeinate` for inhibit.

use crate::{
    BatteryInfo, DisplayPower, IdleAction, InhibitGuard, PowerBackend, PowerError, PowerEvent,
    PowerState,
};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// ── CoreGraphics FFI for idle detection ────────────────────────────────

type CGEventSourceStateID = i32;
type CGEventType = u32;

const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: CGEventSourceStateID = 0;
const K_CG_ANY_INPUT_EVENT_TYPE: CGEventType = !0u32;

unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(
        source_state: CGEventSourceStateID,
        event_type: CGEventType,
    ) -> f64;
}

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
    /// `caffeinate` child process.
    child: Option<Child>,
}

// ── PowerManager ───────────────────────────────────────────────────────

pub struct PowerManager {
    next_id: u64,
    inhibits: Vec<InhibitEntry>,
    state: PowerState,
    display: DisplayPower,
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
    #[allow(unused)]
    last_tick: Instant,
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            inhibits: Vec::new(),
            state: PowerState::Active,
            display: DisplayPower::On,
            dim_timeout: Duration::MAX,
            off_timeout: Duration::MAX,
            suspend_timeout: Duration::MAX,
            fired_dim: false,
            fired_off: false,
            fired_suspend: false,
            last_known_idle: Duration::ZERO,
            last_battery: None,
            last_tick: Instant::now(),
        }
    }

    /// Parse `pmset -g batt` output for battery info.
    fn parse_pmset_battery() -> Option<BatteryInfo> {
        let output = Command::new("pmset").args(["-g", "batt"]).output().ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);

        // Example output:
        //   Now drawing from 'Battery Power'
        //    -InternalBattery-0 (id=...)  72%; discharging; 3:45 remaining
        //
        // Or on a desktop Mac, there will be no battery line.

        let mut percent: Option<u8> = None;
        let mut charging = false;
        let mut time_remaining_secs: Option<u32> = None;

        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("-Internal") && !trimmed.starts_with("-Apple") {
                continue;
            }

            // Extract percentage: look for "NN%"
            for part in trimmed.split(';') {
                let part = part.trim();
                if part.ends_with('%') {
                    // The part before %)  might be like "72%"
                    // But it could also be "... 72%"
                    if let Some(pct_str) = part.strip_suffix('%') {
                        // Take last word which is the number
                        if let Some(num) = pct_str.split_whitespace().last() {
                            percent = num.parse().ok();
                        }
                    }
                } else if part.contains("charging") {
                    charging = part == "charging" || part == "AC attached; charging";
                    if part == "discharging" {
                        charging = false;
                    }
                } else if part.contains("remaining") {
                    // "3:45 remaining"
                    let time_part = part.replace("remaining", "").trim().to_owned();
                    if let Some((h, m)) = time_part.split_once(':') {
                        let hours: u32 = h.trim().parse().unwrap_or(0);
                        let mins: u32 = m.trim().parse().unwrap_or(0);
                        time_remaining_secs = Some(hours * 3600 + mins * 60);
                    }
                }
            }
        }

        percent.map(|p| BatteryInfo {
            present: true,
            charging,
            percent: p.min(100),
            time_remaining_secs,
        })
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerBackend for PowerManager {
    fn battery_info(&self) -> Option<BatteryInfo> {
        Self::parse_pmset_battery()
    }

    fn power_state(&self) -> PowerState {
        self.state
    }

    fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError> {
        let result = match state {
            DisplayPower::On => Command::new("caffeinate")
                .args(["-u", "-t", "1"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status(),
            DisplayPower::Dimmed => {
                // macOS has no direct "dim" — wake the display instead.
                Command::new("caffeinate")
                    .args(["-u", "-t", "1"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
            }
            DisplayPower::Off => Command::new("pmset")
                .args(["displaysleepnow"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status(),
        };

        match result {
            Ok(s) if s.success() => {
                self.display = state;
                Ok(())
            }
            Ok(_) => Err(PowerError::PlatformError(
                "display power command failed".into(),
            )),
            Err(e) => Err(PowerError::PlatformError(format!("command error: {e}"))),
        }
    }

    fn inhibit_sleep(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;

        // caffeinate -i = prevent idle sleep
        let child = Command::new("caffeinate")
            .arg("-i")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok();

        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::Sleep,
            reason: reason.to_owned(),
            child,
        });
        Ok(InhibitGuard { id })
    }

    fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;

        // caffeinate -d = prevent display sleep
        let child = Command::new("caffeinate")
            .arg("-d")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok();

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
            let mut entry = self.inhibits.remove(pos);
            if let Some(ref mut child) = entry.child {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn suspend(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::Suspended;
        let result = Command::new("pmset")
            .arg("sleepnow")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => {
                self.state = PowerState::Active;
                Ok(())
            }
            _ => {
                self.state = PowerState::Active;
                Err(PowerError::PlatformError("pmset sleepnow failed".into()))
            }
        }
    }

    fn hibernate(&mut self) -> Result<(), PowerError> {
        // macOS doesn't have a direct hibernate command for userspace;
        // `pmset sleepnow` is the closest equivalent.
        self.state = PowerState::Hibernated;
        let result = Command::new("pmset")
            .arg("sleepnow")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => {
                self.state = PowerState::Active;
                Ok(())
            }
            _ => {
                self.state = PowerState::Active;
                Err(PowerError::PlatformError(
                    "hibernate not directly supported".into(),
                ))
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::ShuttingDown;
        // osascript -e 'tell app "System Events" to shut down'
        let result = Command::new("osascript")
            .args(["-e", "tell app \"System Events\" to shut down"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => Ok(()),
            _ => {
                self.state = PowerState::Active;
                Err(PowerError::PlatformError("shutdown failed".into()))
            }
        }
    }

    fn reboot(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::ShuttingDown;
        let result = Command::new("osascript")
            .args(["-e", "tell app \"System Events\" to restart"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match result {
            Ok(s) if s.success() => Ok(()),
            _ => {
                self.state = PowerState::Active;
                Err(PowerError::PlatformError("reboot failed".into()))
            }
        }
    }

    fn idle_duration(&self) -> Duration {
        let secs = unsafe {
            CGEventSourceSecondsSinceLastEventType(
                K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                K_CG_ANY_INPUT_EVENT_TYPE,
            )
        };
        if secs < 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(secs)
        }
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
