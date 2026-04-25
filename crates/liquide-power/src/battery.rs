//! Battery monitoring and event generation.
//!
//! Provides a richer battery model than the top-level [`crate::BatteryInfo`]:
//! health, cycle count, voltage, temperature, and time-to-full. The
//! [`BatteryMonitor`] polls battery state and emits events when levels or
//! charging state change, including low-battery warnings at configurable
//! thresholds.
//!
//! Modelled after UPower's battery properties.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Battery state
// ---------------------------------------------------------------------------

/// Charging/discharging state of the battery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotPresent,
    Unknown,
}

impl std::fmt::Display for BatteryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Full => "full",
            Self::NotPresent => "not-present",
            Self::Unknown => "unknown",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Extended battery info
// ---------------------------------------------------------------------------

/// Extended battery information (superset of the top-level `BatteryInfo`).
#[derive(Debug, Clone, PartialEq)]
pub struct BatteryDetail {
    /// Charge level as a percentage (0-100).
    pub level_percent: u8,
    /// Whether the battery is currently charging.
    pub is_charging: bool,
    /// Whether an external power source is connected (may be true even if
    /// the battery is full and not actively charging).
    pub is_plugged_in: bool,
    /// Estimated time until the battery is empty.
    pub time_to_empty: Option<Duration>,
    /// Estimated time until the battery is fully charged.
    pub time_to_full: Option<Duration>,
    /// Battery health as a percentage of design capacity (0-100).
    pub health_percent: Option<u8>,
    /// Number of charge/discharge cycles.
    pub cycle_count: Option<u32>,
    /// Current voltage in millivolts.
    pub voltage_mv: Option<u32>,
    /// Battery temperature in tenths of a degree Celsius (e.g., 350 = 35.0C).
    pub temperature_deci_celsius: Option<i32>,
    /// Overall battery state.
    pub state: BatteryState,
}

impl BatteryDetail {
    /// Returns the temperature in degrees Celsius, if available.
    pub fn temperature_celsius(&self) -> Option<f32> {
        self.temperature_deci_celsius.map(|t| t as f32 / 10.0)
    }

    /// Returns the voltage in volts, if available.
    pub fn voltage(&self) -> Option<f32> {
        self.voltage_mv.map(|v| v as f32 / 1000.0)
    }

    /// Constructs a detail record for a system with no battery.
    pub fn not_present() -> Self {
        Self {
            level_percent: 0,
            is_charging: false,
            is_plugged_in: false,
            time_to_empty: None,
            time_to_full: None,
            health_percent: None,
            cycle_count: None,
            voltage_mv: None,
            temperature_deci_celsius: None,
            state: BatteryState::NotPresent,
        }
    }
}

// ---------------------------------------------------------------------------
// Battery events
// ---------------------------------------------------------------------------

/// Events emitted by the [`BatteryMonitor`].
#[derive(Debug, Clone, PartialEq)]
pub enum BatteryEvent {
    /// The charge level changed (contains the new percentage).
    LevelChanged(u8),
    /// The charging/discharging state changed.
    StateChanged(BatteryState),
    /// Battery level dropped below the warning threshold.
    LowBattery {
        level: u8,
        severity: BatterySeverity,
    },
    /// Battery level is critically low -- system should take action.
    CriticalBattery { level: u8 },
}

/// Severity classification for low-battery events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BatterySeverity {
    /// Informational warning (default 20%).
    Warning,
    /// Low -- user should plug in soon (default 10%).
    Low,
    /// Critical -- system may hibernate/shutdown (default 5%).
    Critical,
    /// Action threshold -- immediate action required (default 3%).
    Action,
}

// ---------------------------------------------------------------------------
// Threshold constants
// ---------------------------------------------------------------------------

/// Battery percentage at which a warning is emitted.
pub const THRESHOLD_WARNING: u8 = 20;
/// Battery percentage considered "low".
pub const THRESHOLD_LOW: u8 = 10;
/// Battery percentage considered "critical".
pub const THRESHOLD_CRITICAL: u8 = 5;
/// Battery percentage at which the system should take emergency action.
pub const THRESHOLD_ACTION: u8 = 3;

// ---------------------------------------------------------------------------
// BatteryMonitor
// ---------------------------------------------------------------------------

/// Monitors battery state and emits [`BatteryEvent`]s when changes are
/// detected. Consumers should call [`BatteryMonitor::update`] periodically
/// with fresh battery data (obtained from the platform backend).
pub struct BatteryMonitor {
    last: Option<BatteryDetail>,
    /// Which low-battery thresholds have already fired (to avoid repeats).
    fired_warning: bool,
    fired_low: bool,
    fired_critical: bool,
    fired_action: bool,
}

impl BatteryMonitor {
    /// Create a new monitor with no prior state.
    pub fn new() -> Self {
        Self {
            last: None,
            fired_warning: false,
            fired_low: false,
            fired_critical: false,
            fired_action: false,
        }
    }

    /// Feed new battery data and receive any events that should be delivered.
    pub fn update(&mut self, current: &BatteryDetail) -> Vec<BatteryEvent> {
        let mut events = Vec::new();

        if let Some(prev) = &self.last {
            // Level change.
            if current.level_percent != prev.level_percent {
                events.push(BatteryEvent::LevelChanged(current.level_percent));
            }
            // State change.
            if current.state != prev.state {
                events.push(BatteryEvent::StateChanged(current.state));
                // Reset threshold flags when we start charging.
                if current.state == BatteryState::Charging || current.state == BatteryState::Full {
                    self.reset_thresholds();
                }
            }
        } else {
            // First reading -- always emit level.
            events.push(BatteryEvent::LevelChanged(current.level_percent));
            events.push(BatteryEvent::StateChanged(current.state));
        }

        // Low-battery thresholds (only when discharging).
        if current.state == BatteryState::Discharging {
            if current.level_percent <= THRESHOLD_ACTION && !self.fired_action {
                self.fired_action = true;
                events.push(BatteryEvent::CriticalBattery {
                    level: current.level_percent,
                });
            } else if current.level_percent <= THRESHOLD_CRITICAL && !self.fired_critical {
                self.fired_critical = true;
                events.push(BatteryEvent::LowBattery {
                    level: current.level_percent,
                    severity: BatterySeverity::Critical,
                });
            } else if current.level_percent <= THRESHOLD_LOW && !self.fired_low {
                self.fired_low = true;
                events.push(BatteryEvent::LowBattery {
                    level: current.level_percent,
                    severity: BatterySeverity::Low,
                });
            } else if current.level_percent <= THRESHOLD_WARNING && !self.fired_warning {
                self.fired_warning = true;
                events.push(BatteryEvent::LowBattery {
                    level: current.level_percent,
                    severity: BatterySeverity::Warning,
                });
            }
        }

        self.last = Some(current.clone());
        events
    }

    /// Returns the last known battery detail, if any.
    pub fn last_detail(&self) -> Option<&BatteryDetail> {
        self.last.as_ref()
    }

    /// Reset low-battery threshold flags (e.g., when AC is connected).
    pub fn reset_thresholds(&mut self) {
        self.fired_warning = false;
        self.fired_low = false;
        self.fired_critical = false;
        self.fired_action = false;
    }
}

impl Default for BatteryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_detail(level: u8, state: BatteryState) -> BatteryDetail {
        BatteryDetail {
            level_percent: level,
            is_charging: state == BatteryState::Charging,
            is_plugged_in: state == BatteryState::Charging || state == BatteryState::Full,
            time_to_empty: Some(Duration::from_secs(level as u64 * 60)),
            time_to_full: None,
            health_percent: Some(95),
            cycle_count: Some(120),
            voltage_mv: Some(3700),
            temperature_deci_celsius: Some(350),
            state,
        }
    }

    #[test]
    fn first_update_emits_level_and_state() {
        let mut mon = BatteryMonitor::new();
        let detail = make_detail(80, BatteryState::Discharging);
        let events = mon.update(&detail);
        assert!(events.contains(&BatteryEvent::LevelChanged(80)));
        assert!(events.contains(&BatteryEvent::StateChanged(BatteryState::Discharging)));
    }

    #[test]
    fn level_change_emits_event() {
        let mut mon = BatteryMonitor::new();
        let d1 = make_detail(80, BatteryState::Discharging);
        mon.update(&d1);

        let d2 = make_detail(79, BatteryState::Discharging);
        let events = mon.update(&d2);
        assert!(events.contains(&BatteryEvent::LevelChanged(79)));
    }

    #[test]
    fn no_event_when_unchanged() {
        let mut mon = BatteryMonitor::new();
        let d = make_detail(80, BatteryState::Discharging);
        mon.update(&d);
        let events = mon.update(&d);
        assert!(events.is_empty());
    }

    #[test]
    fn state_change_emits_event() {
        let mut mon = BatteryMonitor::new();
        let d1 = make_detail(50, BatteryState::Discharging);
        mon.update(&d1);

        let d2 = make_detail(50, BatteryState::Charging);
        let events = mon.update(&d2);
        assert!(events.contains(&BatteryEvent::StateChanged(BatteryState::Charging)));
    }

    #[test]
    fn warning_threshold() {
        let mut mon = BatteryMonitor::new();
        let d1 = make_detail(25, BatteryState::Discharging);
        mon.update(&d1);

        let d2 = make_detail(20, BatteryState::Discharging);
        let events = mon.update(&d2);
        assert!(events.iter().any(|e| matches!(
            e,
            BatteryEvent::LowBattery {
                severity: BatterySeverity::Warning,
                ..
            }
        )));
    }

    #[test]
    fn low_threshold() {
        let mut mon = BatteryMonitor::new();
        // Skip warning by going straight to 10.
        let d = make_detail(10, BatteryState::Discharging);
        let events = mon.update(&d);
        assert!(events.iter().any(|e| matches!(
            e,
            BatteryEvent::LowBattery {
                severity: BatterySeverity::Low,
                ..
            }
        )));
    }

    #[test]
    fn critical_threshold() {
        let mut mon = BatteryMonitor::new();
        let d = make_detail(5, BatteryState::Discharging);
        let events = mon.update(&d);
        assert!(events.iter().any(|e| matches!(
            e,
            BatteryEvent::LowBattery {
                severity: BatterySeverity::Critical,
                ..
            }
        )));
    }

    #[test]
    fn action_threshold_emits_critical_battery() {
        let mut mon = BatteryMonitor::new();
        let d = make_detail(3, BatteryState::Discharging);
        let events = mon.update(&d);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BatteryEvent::CriticalBattery { .. }))
        );
    }

    #[test]
    fn thresholds_fire_only_once() {
        let mut mon = BatteryMonitor::new();
        let d = make_detail(18, BatteryState::Discharging);
        let events1 = mon.update(&d);
        let warning_count = events1
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    BatteryEvent::LowBattery {
                        severity: BatterySeverity::Warning,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(warning_count, 1);

        // Same level again -- no repeat.
        let d2 = make_detail(17, BatteryState::Discharging);
        let events2 = mon.update(&d2);
        let warning_count2 = events2
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    BatteryEvent::LowBattery {
                        severity: BatterySeverity::Warning,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(warning_count2, 0);
    }

    #[test]
    fn charging_resets_thresholds() {
        let mut mon = BatteryMonitor::new();
        let d1 = make_detail(15, BatteryState::Discharging);
        mon.update(&d1);

        // Start charging -- thresholds should reset.
        let d2 = make_detail(15, BatteryState::Charging);
        mon.update(&d2);

        // Back to discharging at 18 -- warning should fire again.
        let d3 = make_detail(18, BatteryState::Discharging);
        let events = mon.update(&d3);
        assert!(events.iter().any(|e| matches!(
            e,
            BatteryEvent::LowBattery {
                severity: BatterySeverity::Warning,
                ..
            }
        )));
    }

    #[test]
    fn not_present_battery() {
        let detail = BatteryDetail::not_present();
        assert_eq!(detail.state, BatteryState::NotPresent);
        assert_eq!(detail.level_percent, 0);
        assert!(!detail.is_charging);
    }

    #[test]
    fn temperature_conversion() {
        let d = make_detail(80, BatteryState::Discharging);
        assert_eq!(d.temperature_celsius(), Some(35.0));
    }

    #[test]
    fn voltage_conversion() {
        let d = make_detail(80, BatteryState::Discharging);
        assert_eq!(d.voltage(), Some(3.7));
    }

    #[test]
    fn battery_state_display() {
        assert_eq!(BatteryState::Charging.to_string(), "charging");
        assert_eq!(BatteryState::Discharging.to_string(), "discharging");
        assert_eq!(BatteryState::Full.to_string(), "full");
        assert_eq!(BatteryState::NotPresent.to_string(), "not-present");
        assert_eq!(BatteryState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn last_detail_stored() {
        let mut mon = BatteryMonitor::new();
        assert!(mon.last_detail().is_none());
        let d = make_detail(50, BatteryState::Charging);
        mon.update(&d);
        assert_eq!(mon.last_detail().unwrap().level_percent, 50);
    }

    #[test]
    fn no_low_battery_events_when_charging() {
        let mut mon = BatteryMonitor::new();
        let d = make_detail(5, BatteryState::Charging);
        let events = mon.update(&d);
        let low_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    BatteryEvent::LowBattery { .. } | BatteryEvent::CriticalBattery { .. }
                )
            })
            .collect();
        assert!(low_events.is_empty());
    }
}
