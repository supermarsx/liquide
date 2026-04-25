//! Win32 power management implementation.
//!
//! Uses raw FFI to kernel32/user32 — no external crate dependencies.

use crate::{
    BatteryInfo, DisplayPower, IdleAction, InhibitGuard, PowerBackend, PowerError, PowerEvent,
    PowerState,
};
use std::time::{Duration, Instant};

// ── Win32 FFI ──────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Default)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

#[repr(C)]
#[derive(Default)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

const ES_CONTINUOUS: u32 = 0x80000000;
const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
const ES_DISPLAY_REQUIRED: u32 = 0x00000002;

const EWX_SHUTDOWN: u32 = 0x00000001;
const EWX_REBOOT: u32 = 0x00000002;
const EWX_FORCEIFHUNG: u32 = 0x00000010;

const SE_SHUTDOWN_NAME: &[u8] = b"SeShutdownPrivilege\0";

const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;
const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;

#[repr(C)]
struct Luid {
    low_part: u32,
    high_part: i32,
}

#[repr(C)]
struct LuidAndAttributes {
    luid: Luid,
    attributes: u32,
}

#[repr(C)]
struct TokenPrivileges {
    privilege_count: u32,
    privileges: [LuidAndAttributes; 1],
}

const HWND_BROADCAST: isize = 0xFFFF;
const WM_SYSCOMMAND: u32 = 0x0112;
const SC_MONITORPOWER: usize = 0xF170;

const MONITOR_ON: isize = -1;
const MONITOR_OFF: isize = 2;

unsafe extern "system" {
    // kernel32
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
    fn SetThreadExecutionState(flags: u32) -> u32;
    fn SetSuspendState(hibernate: i32, force: i32, disable_wake: i32) -> i32;
    fn GetTickCount() -> u32;
    fn GetCurrentProcess() -> isize;
    fn GetLastError() -> u32;
    fn OpenProcessToken(process: isize, desired: u32, token: *mut isize) -> i32;
    fn CloseHandle(handle: isize) -> i32;

    // advapi32
    fn LookupPrivilegeValueA(system: *const u8, name: *const u8, luid: *mut Luid) -> i32;
    fn AdjustTokenPrivileges(
        token: isize,
        disable_all: i32,
        new_state: *const TokenPrivileges,
        buf_len: u32,
        prev_state: *mut TokenPrivileges,
        ret_len: *mut u32,
    ) -> i32;
    fn ExitWindowsEx(flags: u32, reason: u32) -> i32;

    // user32
    fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
    fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
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
    // Tracks which idle events have already fired (reset on user input).
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

    /// Recompute the execution-state flags based on current inhibits and
    /// call `SetThreadExecutionState` once.
    fn apply_execution_state(&self) {
        let mut flags = ES_CONTINUOUS;
        for entry in &self.inhibits {
            match entry.kind {
                InhibitKind::Sleep => flags |= ES_SYSTEM_REQUIRED,
                InhibitKind::DisplayOff => flags |= ES_DISPLAY_REQUIRED,
            }
        }
        unsafe {
            SetThreadExecutionState(flags);
        }
    }

    /// Attempt to enable the shutdown privilege for the current process
    /// so that `ExitWindowsEx` succeeds.
    fn enable_shutdown_privilege() -> Result<(), PowerError> {
        unsafe {
            let process = GetCurrentProcess();
            let mut token: isize = 0;
            if OpenProcessToken(process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
                return Err(PowerError::PermissionDenied);
            }

            let mut luid = Luid {
                low_part: 0,
                high_part: 0,
            };
            if LookupPrivilegeValueA(std::ptr::null(), SE_SHUTDOWN_NAME.as_ptr(), &mut luid) == 0 {
                CloseHandle(token);
                return Err(PowerError::PermissionDenied);
            }

            let tp = TokenPrivileges {
                privilege_count: 1,
                privileges: [LuidAndAttributes {
                    luid,
                    attributes: SE_PRIVILEGE_ENABLED,
                }],
            };

            let ok =
                AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut());

            let err = GetLastError();
            CloseHandle(token);

            if ok == 0 || err != 0 {
                return Err(PowerError::PermissionDenied);
            }
            Ok(())
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
        let mut status = SystemPowerStatus::default();
        let ok = unsafe { GetSystemPowerStatus(&mut status) };
        if ok == 0 {
            return None;
        }

        // battery_flag == 128 means "no system battery"
        if status.battery_flag == 128 || status.battery_flag == 255 {
            return None;
        }

        let present = true;
        let charging = status.ac_line_status == 1 && (status.battery_flag & 8) != 0;
        let percent = if status.battery_life_percent > 100 {
            100
        } else {
            status.battery_life_percent
        };
        let time_remaining_secs = if status.battery_life_time == 0xFFFFFFFF {
            None
        } else {
            Some(status.battery_life_time)
        };

        Some(BatteryInfo {
            present,
            charging,
            percent,
            time_remaining_secs,
        })
    }

    fn power_state(&self) -> PowerState {
        self.state
    }

    fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError> {
        let param: isize = match state {
            DisplayPower::On => MONITOR_ON,
            DisplayPower::Dimmed => MONITOR_ON, // Win32 has no "dim" — treat as on
            DisplayPower::Off => MONITOR_OFF,
        };
        unsafe {
            SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, param);
        }
        self.display = state;
        Ok(())
    }

    fn inhibit_sleep(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::Sleep,
            reason: reason.to_owned(),
        });
        self.apply_execution_state();
        Ok(InhibitGuard { id })
    }

    fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        self.inhibits.push(InhibitEntry {
            id,
            kind: InhibitKind::DisplayOff,
            reason: reason.to_owned(),
        });
        self.apply_execution_state();
        Ok(InhibitGuard { id })
    }

    fn release_inhibit(&mut self, guard: InhibitGuard) {
        self.inhibits.retain(|e| e.id != guard.id);
        self.apply_execution_state();
    }

    fn suspend(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::Suspended;
        let ok = unsafe { SetSuspendState(0, 0, 0) };
        if ok == 0 {
            self.state = PowerState::Active;
            return Err(PowerError::PlatformError("SetSuspendState failed".into()));
        }
        // When we resume, the call returns.
        self.state = PowerState::Active;
        Ok(())
    }

    fn hibernate(&mut self) -> Result<(), PowerError> {
        self.state = PowerState::Hibernated;
        let ok = unsafe { SetSuspendState(1, 0, 0) };
        if ok == 0 {
            self.state = PowerState::Active;
            return Err(PowerError::PlatformError(
                "SetSuspendState(hibernate) failed".into(),
            ));
        }
        self.state = PowerState::Active;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), PowerError> {
        Self::enable_shutdown_privilege()?;
        self.state = PowerState::ShuttingDown;
        let ok = unsafe { ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG, 0) };
        if ok == 0 {
            self.state = PowerState::Active;
            return Err(PowerError::PlatformError(
                "ExitWindowsEx(shutdown) failed".into(),
            ));
        }
        Ok(())
    }

    fn reboot(&mut self) -> Result<(), PowerError> {
        Self::enable_shutdown_privilege()?;
        self.state = PowerState::ShuttingDown;
        let ok = unsafe { ExitWindowsEx(EWX_REBOOT | EWX_FORCEIFHUNG, 0) };
        if ok == 0 {
            self.state = PowerState::Active;
            return Err(PowerError::PlatformError(
                "ExitWindowsEx(reboot) failed".into(),
            ));
        }
        Ok(())
    }

    fn idle_duration(&self) -> Duration {
        let mut info = LastInputInfo {
            cb_size: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        let ok = unsafe { GetLastInputInfo(&mut info) };
        if ok == 0 {
            return Duration::ZERO;
        }
        let now = unsafe { GetTickCount() };
        // Handle u32 wrap-around.
        let elapsed_ms = now.wrapping_sub(info.dw_time);
        Duration::from_millis(elapsed_ms as u64)
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

        // ── Idle threshold detection ───────────────────────────────
        let idle = self.idle_duration();

        // If idle went down, user gave input — reset fired flags.
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

        // ── Battery change detection (poll every tick) ─────────────
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
