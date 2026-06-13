//! Cross-platform power management for the LiquiDE desktop environment.
//!
//! Provides battery monitoring, idle detection, display power control,
//! sleep/hibernate/shutdown actions, and inhibit guards to prevent
//! the system from sleeping while work is in progress.

pub mod battery;
pub mod gated;
pub mod idle;
pub mod inhibitor;
mod platform;
pub mod policy;
pub mod thermal;

pub use gated::GatedPowerManager;
pub use platform::PowerManager;

/// Power states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    Active,
    Idle,
    DisplayOff,
    Suspended,
    Hibernated,
    ShuttingDown,
}

/// Battery status.
#[derive(Debug, Clone, Copy)]
pub struct BatteryInfo {
    pub present: bool,
    pub charging: bool,
    /// Charge percentage, 0-100.
    pub percent: u8,
    /// Estimated seconds of battery life remaining, if known.
    pub time_remaining_secs: Option<u32>,
}

/// Display power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayPower {
    On,
    Dimmed,
    Off,
}

/// Inhibit token -- display/sleep stays on while this is held.
///
/// Dropping the guard does **not** automatically release the inhibit;
/// callers must pass it to [`PowerBackend::release_inhibit`].
#[derive(Debug)]
pub struct InhibitGuard {
    pub(crate) id: u64,
}

impl InhibitGuard {
    /// Returns the unique identifier for this inhibit token.
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// Callback invoked when the system has been idle for a given duration.
pub type IdleCallback = Box<dyn Fn(std::time::Duration) + Send + Sync>;

/// Platform-agnostic power management trait.
pub trait PowerBackend: Send {
    /// Get current battery info (`None` if no battery / desktop machine).
    fn battery_info(&self) -> Option<BatteryInfo>;

    /// Get current power state.
    fn power_state(&self) -> PowerState;

    /// Request display power change.
    fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError>;

    /// Inhibit sleep (returns guard that must be passed to `release_inhibit`).
    fn inhibit_sleep(&mut self, reason: &str) -> Result<InhibitGuard, PowerError>;

    /// Inhibit display-off (returns guard that must be passed to `release_inhibit`).
    fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError>;

    /// Release a previously acquired inhibit guard.
    fn release_inhibit(&mut self, guard: InhibitGuard);

    /// Request system suspend (sleep).
    fn suspend(&mut self) -> Result<(), PowerError>;

    /// Request system hibernate.
    fn hibernate(&mut self) -> Result<(), PowerError>;

    /// Request system shutdown.
    fn shutdown(&mut self) -> Result<(), PowerError>;

    /// Request system reboot.
    fn reboot(&mut self) -> Result<(), PowerError>;

    /// How long the system has been idle (no user input).
    fn idle_duration(&self) -> std::time::Duration;

    /// Configure idle timeout thresholds.
    fn set_idle_timeout(
        &mut self,
        display_dim: std::time::Duration,
        display_off: std::time::Duration,
        suspend: std::time::Duration,
    );

    /// Poll -- call periodically to check idle timeouts and platform events.
    fn tick(&mut self) -> Vec<PowerEvent>;
}

/// Events emitted by the power subsystem.
#[derive(Debug, Clone)]
pub enum PowerEvent {
    BatteryChanged(BatteryInfo),
    PowerStateChanged(PowerState),
    IdleThresholdReached {
        kind: IdleAction,
        after: std::time::Duration,
    },
    DisplayPowerChanged(DisplayPower),
    LidClosed,
    LidOpened,
}

/// Which idle-timeout threshold was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleAction {
    DimDisplay,
    TurnOffDisplay,
    Suspend,
}

/// Errors from the power subsystem.
#[derive(Debug, Clone)]
pub enum PowerError {
    NotSupported,
    PermissionDenied,
    PlatformError(String),
}

impl std::fmt::Display for PowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupported => write!(f, "operation not supported"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::PlatformError(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PowerError {}

// ── Stub backend (available on all platforms for testing) ──────────────

/// A null power backend that returns `NotSupported` for every operation.
/// Useful for testing and as a fallback when no platform backend is available.
pub struct StubPowerManager {
    next_id: u64,
    state: PowerState,
    display: DisplayPower,
    idle_start: std::time::Instant,
    dim_timeout: std::time::Duration,
    off_timeout: std::time::Duration,
    suspend_timeout: std::time::Duration,
    fired_dim: bool,
    fired_off: bool,
    fired_suspend: bool,
}

impl StubPowerManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            state: PowerState::Active,
            display: DisplayPower::On,
            idle_start: std::time::Instant::now(),
            dim_timeout: std::time::Duration::MAX,
            off_timeout: std::time::Duration::MAX,
            suspend_timeout: std::time::Duration::MAX,
            fired_dim: false,
            fired_off: false,
            fired_suspend: false,
        }
    }

    /// Reset the idle timer (simulates user input).
    pub fn reset_idle(&mut self) {
        self.idle_start = std::time::Instant::now();
        self.fired_dim = false;
        self.fired_off = false;
        self.fired_suspend = false;
    }
}

impl Default for StubPowerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerBackend for StubPowerManager {
    fn battery_info(&self) -> Option<BatteryInfo> {
        None
    }

    fn power_state(&self) -> PowerState {
        self.state
    }

    fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError> {
        self.display = state;
        Ok(())
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

    fn release_inhibit(&mut self, _guard: InhibitGuard) {
        // no-op
    }

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

    fn idle_duration(&self) -> std::time::Duration {
        self.idle_start.elapsed()
    }

    fn set_idle_timeout(
        &mut self,
        display_dim: std::time::Duration,
        display_off: std::time::Duration,
        suspend: std::time::Duration,
    ) {
        self.dim_timeout = display_dim;
        self.off_timeout = display_off;
        self.suspend_timeout = suspend;
        self.fired_dim = false;
        self.fired_off = false;
        self.fired_suspend = false;
    }

    fn tick(&mut self) -> Vec<PowerEvent> {
        let idle = self.idle_start.elapsed();
        let mut events = Vec::new();

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

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn power_manager_creation() {
        let pm = PowerManager::new();
        // Should not panic; state should be Active.
        assert_eq!(pm.power_state(), PowerState::Active);
    }

    #[test]
    fn stub_battery_returns_none() {
        let stub = StubPowerManager::new();
        assert!(stub.battery_info().is_none());
    }

    #[test]
    fn stub_power_actions_not_supported() {
        let mut stub = StubPowerManager::new();
        assert!(matches!(stub.suspend(), Err(PowerError::NotSupported)));
        assert!(matches!(stub.hibernate(), Err(PowerError::NotSupported)));
        assert!(matches!(stub.shutdown(), Err(PowerError::NotSupported)));
        assert!(matches!(stub.reboot(), Err(PowerError::NotSupported)));
    }

    #[test]
    fn stub_display_power() {
        let mut stub = StubPowerManager::new();
        assert!(stub.set_display_power(DisplayPower::Off).is_ok());
        assert!(stub.set_display_power(DisplayPower::Dimmed).is_ok());
        assert!(stub.set_display_power(DisplayPower::On).is_ok());
    }

    #[test]
    fn inhibit_guard_ids_are_unique() {
        let mut stub = StubPowerManager::new();
        let g1 = stub.inhibit_sleep("test1").unwrap();
        let g2 = stub.inhibit_sleep("test2").unwrap();
        let g3 = stub.inhibit_display_off("test3").unwrap();
        assert_ne!(g1.id(), g2.id());
        assert_ne!(g2.id(), g3.id());
        // Release them (no-op for stub, but should not panic).
        stub.release_inhibit(g1);
        stub.release_inhibit(g2);
        stub.release_inhibit(g3);
    }

    #[test]
    fn stub_idle_duration_increases() {
        let stub = StubPowerManager::new();
        let d1 = stub.idle_duration();
        // Spin briefly so the clock moves forward.
        std::thread::sleep(Duration::from_millis(10));
        let d2 = stub.idle_duration();
        assert!(d2 > d1);
    }

    #[test]
    fn stub_idle_reset() {
        let mut stub = StubPowerManager::new();
        std::thread::sleep(Duration::from_millis(10));
        stub.reset_idle();
        let d = stub.idle_duration();
        // After reset the idle duration should be very small.
        assert!(d < Duration::from_millis(50));
    }

    #[test]
    fn stub_tick_fires_idle_events() {
        let mut stub = StubPowerManager::new();
        stub.set_idle_timeout(
            Duration::from_millis(0),
            Duration::from_millis(0),
            Duration::from_millis(0),
        );
        // With zero timeouts, the very first tick should fire all three.
        let events = stub.tick();
        assert_eq!(events.len(), 3);

        // Verify the kinds.
        let kinds: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                PowerEvent::IdleThresholdReached { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert!(kinds.contains(&IdleAction::DimDisplay));
        assert!(kinds.contains(&IdleAction::TurnOffDisplay));
        assert!(kinds.contains(&IdleAction::Suspend));

        // Second tick should not fire again (already fired).
        let events2 = stub.tick();
        assert!(events2.is_empty());
    }

    #[test]
    fn stub_tick_no_events_before_timeout() {
        let mut stub = StubPowerManager::new();
        stub.set_idle_timeout(
            Duration::from_secs(3600),
            Duration::from_secs(7200),
            Duration::from_secs(14400),
        );
        let events = stub.tick();
        assert!(events.is_empty());
    }

    #[test]
    fn power_error_display() {
        assert_eq!(
            PowerError::NotSupported.to_string(),
            "operation not supported"
        );
        assert_eq!(
            PowerError::PermissionDenied.to_string(),
            "permission denied"
        );
        assert_eq!(PowerError::PlatformError("oops".into()).to_string(), "oops");
    }

    #[test]
    fn power_state_equality() {
        assert_eq!(PowerState::Active, PowerState::Active);
        assert_ne!(PowerState::Active, PowerState::Idle);
        assert_ne!(PowerState::Suspended, PowerState::Hibernated);
    }

    #[test]
    fn display_power_equality() {
        assert_eq!(DisplayPower::On, DisplayPower::On);
        assert_ne!(DisplayPower::On, DisplayPower::Off);
        assert_ne!(DisplayPower::Dimmed, DisplayPower::Off);
    }

    #[test]
    fn platform_power_manager_implements_backend() {
        // Verify that the platform PowerManager implements PowerBackend.
        fn assert_backend<T: PowerBackend>() {}
        assert_backend::<PowerManager>();
    }
}
