//! Stub power backend for unsupported platforms.
//!
//! Returns `NotSupported` for all actions; battery_info is always `None`.

use crate::{
    BatteryInfo, DisplayPower, InhibitGuard, PowerBackend, PowerError, PowerEvent, PowerState,
};
use std::time::Duration;

pub struct PowerManager {
    state: PowerState,
    next_id: u64,
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            state: PowerState::Active,
            next_id: 1,
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
        None
    }

    fn power_state(&self) -> PowerState {
        self.state
    }

    fn set_display_power(&mut self, _state: DisplayPower) -> Result<(), PowerError> {
        Err(PowerError::NotSupported)
    }

    fn inhibit_sleep(&mut self, _reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        Ok(InhibitGuard { id })
    }

    fn inhibit_display_off(&mut self, _reason: &str) -> Result<InhibitGuard, PowerError> {
        let id = self.next_id;
        self.next_id += 1;
        Ok(InhibitGuard { id })
    }

    fn release_inhibit(&mut self, _guard: InhibitGuard) {}

    fn suspend(&mut self) -> Result<(), PowerError> {
        Err(PowerError::NotSupported)
    }

    fn hibernate(&mut self) -> Result<(), PowerError> {
        Err(PowerError::NotSupported)
    }

    fn shutdown(&mut self) -> Result<(), PowerError> {
        Err(PowerError::NotSupported)
    }

    fn reboot(&mut self) -> Result<(), PowerError> {
        Err(PowerError::NotSupported)
    }

    fn idle_duration(&self) -> Duration {
        Duration::ZERO
    }

    fn set_idle_timeout(
        &mut self,
        _display_dim: Duration,
        _display_off: Duration,
        _suspend: Duration,
    ) {
    }

    fn tick(&mut self) -> Vec<PowerEvent> {
        Vec::new()
    }
}
