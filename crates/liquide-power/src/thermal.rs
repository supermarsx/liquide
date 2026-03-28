//! Thermal monitoring and management.
//!
//! Tracks thermal zones (CPU, GPU, battery, etc.), classifies temperature
//! into severity levels, and emits events when temperatures cross trip
//! points. Also provides fan speed control hints.
//!
//! Modelled after Linux sysfs thermal_zone and thermal policy concepts.

// ---------------------------------------------------------------------------
// Thermal policy
// ---------------------------------------------------------------------------

/// Thermal management strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThermalPolicy {
    /// Active cooling: fans spin up to reduce temperature.
    Active,
    /// Passive cooling: CPU/GPU are throttled to reduce temperature.
    Passive,
}

impl std::fmt::Display for ThermalPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Passive => write!(f, "passive"),
        }
    }
}

// ---------------------------------------------------------------------------
// Trip points
// ---------------------------------------------------------------------------

/// A temperature trip point that triggers an action.
#[derive(Debug, Clone, PartialEq)]
pub struct TripPoint {
    /// Temperature in millidegrees Celsius (e.g., 80_000 = 80.0 C).
    pub temp_milli_celsius: i32,
    /// What kind of trip point this is.
    pub kind: TripPointKind,
}

impl TripPoint {
    /// Temperature in degrees Celsius.
    pub fn temp_celsius(&self) -> f32 {
        self.temp_milli_celsius as f32 / 1000.0
    }
}

/// Classification of a trip point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TripPointKind {
    /// Start active cooling (fans).
    Active,
    /// Start passive cooling (throttling).
    Passive,
    /// System is hot -- warning threshold.
    Hot,
    /// Critical -- system should shut down.
    Critical,
}

// ---------------------------------------------------------------------------
// Thermal zone
// ---------------------------------------------------------------------------

/// A single thermal sensor / zone (e.g., "x86_pkg_temp", "gpu-thermal").
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalZone {
    /// Name or type of the thermal zone (e.g., "cpu", "gpu", "battery").
    pub name: String,
    /// Current temperature in millidegrees Celsius.
    pub temperature_milli_celsius: i32,
    /// Configured trip points for this zone.
    pub trip_points: Vec<TripPoint>,
    /// Active thermal policy for this zone.
    pub policy: ThermalPolicy,
}

impl ThermalZone {
    /// Current temperature in degrees Celsius.
    pub fn temperature_celsius(&self) -> f32 {
        self.temperature_milli_celsius as f32 / 1000.0
    }

    /// Classify the current temperature against the trip points.
    pub fn classify(&self) -> ThermalSeverity {
        let temp = self.temperature_milli_celsius;

        // Check from most severe to least.
        for tp in &self.trip_points {
            if tp.kind == TripPointKind::Critical && temp >= tp.temp_milli_celsius {
                return ThermalSeverity::Critical;
            }
        }
        for tp in &self.trip_points {
            if tp.kind == TripPointKind::Hot && temp >= tp.temp_milli_celsius {
                return ThermalSeverity::Hot;
            }
        }
        for tp in &self.trip_points {
            if matches!(tp.kind, TripPointKind::Passive | TripPointKind::Active)
                && temp >= tp.temp_milli_celsius
            {
                return ThermalSeverity::Warm;
            }
        }
        ThermalSeverity::Normal
    }
}

// ---------------------------------------------------------------------------
// Thermal severity / events
// ---------------------------------------------------------------------------

/// Temperature severity classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThermalSeverity {
    /// Temperature is within normal operating range.
    Normal,
    /// Temperature is elevated -- active/passive cooling engaged.
    Warm,
    /// Temperature is high -- throttling recommended.
    Hot,
    /// Temperature is critical -- immediate action required.
    Critical,
}

impl std::fmt::Display for ThermalSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Normal => "normal",
            Self::Warm => "warm",
            Self::Hot => "hot",
            Self::Critical => "critical",
        };
        write!(f, "{s}")
    }
}

/// Events emitted by the [`ThermalMonitor`].
#[derive(Debug, Clone, PartialEq)]
pub enum ThermalEvent {
    /// A zone's severity changed.
    SeverityChanged {
        zone_name: String,
        from: ThermalSeverity,
        to: ThermalSeverity,
    },
    /// A zone's temperature was updated (informational).
    TemperatureUpdated {
        zone_name: String,
        temp_milli_celsius: i32,
    },
}

// ---------------------------------------------------------------------------
// Fan speed hint
// ---------------------------------------------------------------------------

/// Desired fan speed (as a hint to the platform layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanSpeedHint {
    /// Let the firmware/EC control the fan.
    Auto,
    /// Fan off (only if temperature is safe).
    Off,
    /// Fan at a specific percentage (0-100).
    Percentage(u8),
}

// ---------------------------------------------------------------------------
// ThermalMonitor
// ---------------------------------------------------------------------------

/// Monitors thermal zones and emits events when severity changes.
pub struct ThermalMonitor {
    zones: Vec<ThermalZone>,
    /// Last known severity per zone (by index).
    last_severity: Vec<ThermalSeverity>,
    /// Suggested fan speed based on worst-case zone.
    fan_hint: FanSpeedHint,
}

impl ThermalMonitor {
    /// Create an empty monitor.
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            last_severity: Vec::new(),
            fan_hint: FanSpeedHint::Auto,
        }
    }

    /// Set or replace all thermal zones. Resets severity tracking.
    pub fn set_zones(&mut self, zones: Vec<ThermalZone>) {
        self.last_severity = zones.iter().map(|z| z.classify()).collect();
        self.zones = zones;
        self.update_fan_hint();
    }

    /// Update a single zone by name. Returns any events generated.
    pub fn update_zone(&mut self, name: &str, temp_milli_celsius: i32) -> Vec<ThermalEvent> {
        let mut events = Vec::new();
        if let Some(idx) = self.zones.iter().position(|z| z.name == name) {
            self.zones[idx].temperature_milli_celsius = temp_milli_celsius;
            let new_severity = self.zones[idx].classify();
            let old_severity = self.last_severity[idx];

            events.push(ThermalEvent::TemperatureUpdated {
                zone_name: name.to_string(),
                temp_milli_celsius,
            });

            if new_severity != old_severity {
                self.last_severity[idx] = new_severity;
                events.push(ThermalEvent::SeverityChanged {
                    zone_name: name.to_string(),
                    from: old_severity,
                    to: new_severity,
                });
            }
            self.update_fan_hint();
        }
        events
    }

    /// Poll all zones. Returns events for any zones that changed severity.
    pub fn poll(&mut self) -> Vec<ThermalEvent> {
        let mut events = Vec::new();
        for (idx, zone) in self.zones.iter().enumerate() {
            let new_severity = zone.classify();
            if new_severity != self.last_severity[idx] {
                events.push(ThermalEvent::SeverityChanged {
                    zone_name: zone.name.clone(),
                    from: self.last_severity[idx],
                    to: new_severity,
                });
                self.last_severity[idx] = new_severity;
            }
        }
        self.update_fan_hint();
        events
    }

    /// Current fan speed hint.
    pub fn fan_hint(&self) -> FanSpeedHint {
        self.fan_hint
    }

    /// All zones.
    pub fn zones(&self) -> &[ThermalZone] {
        &self.zones
    }

    /// Worst-case severity across all zones.
    pub fn worst_severity(&self) -> ThermalSeverity {
        self.last_severity
            .iter()
            .copied()
            .max()
            .unwrap_or(ThermalSeverity::Normal)
    }

    fn update_fan_hint(&mut self) {
        let worst = self.worst_severity();
        self.fan_hint = match worst {
            ThermalSeverity::Normal => FanSpeedHint::Auto,
            ThermalSeverity::Warm => FanSpeedHint::Percentage(50),
            ThermalSeverity::Hot => FanSpeedHint::Percentage(80),
            ThermalSeverity::Critical => FanSpeedHint::Percentage(100),
        };
    }
}

impl Default for ThermalMonitor {
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

    fn cpu_zone(temp: i32) -> ThermalZone {
        ThermalZone {
            name: "cpu".into(),
            temperature_milli_celsius: temp,
            trip_points: vec![
                TripPoint {
                    temp_milli_celsius: 60_000,
                    kind: TripPointKind::Active,
                },
                TripPoint {
                    temp_milli_celsius: 80_000,
                    kind: TripPointKind::Hot,
                },
                TripPoint {
                    temp_milli_celsius: 100_000,
                    kind: TripPointKind::Critical,
                },
            ],
            policy: ThermalPolicy::Active,
        }
    }

    #[test]
    fn classify_normal() {
        let zone = cpu_zone(45_000);
        assert_eq!(zone.classify(), ThermalSeverity::Normal);
    }

    #[test]
    fn classify_warm() {
        let zone = cpu_zone(65_000);
        assert_eq!(zone.classify(), ThermalSeverity::Warm);
    }

    #[test]
    fn classify_hot() {
        let zone = cpu_zone(85_000);
        assert_eq!(zone.classify(), ThermalSeverity::Hot);
    }

    #[test]
    fn classify_critical() {
        let zone = cpu_zone(105_000);
        assert_eq!(zone.classify(), ThermalSeverity::Critical);
    }

    #[test]
    fn temperature_celsius_conversion() {
        let zone = cpu_zone(72_500);
        assert!((zone.temperature_celsius() - 72.5).abs() < 0.01);
    }

    #[test]
    fn trip_point_celsius() {
        let tp = TripPoint {
            temp_milli_celsius: 80_000,
            kind: TripPointKind::Hot,
        };
        assert!((tp.temp_celsius() - 80.0).abs() < 0.01);
    }

    #[test]
    fn monitor_set_zones() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);
        assert_eq!(mon.zones().len(), 1);
        assert_eq!(mon.worst_severity(), ThermalSeverity::Normal);
    }

    #[test]
    fn monitor_update_zone() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);

        let events = mon.update_zone("cpu", 85_000);
        // Should have a temperature update and a severity change.
        assert!(events.iter().any(|e| matches!(
            e,
            ThermalEvent::TemperatureUpdated { temp_milli_celsius: 85_000, .. }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            ThermalEvent::SeverityChanged {
                from: ThermalSeverity::Normal,
                to: ThermalSeverity::Hot,
                ..
            }
        )));
    }

    #[test]
    fn monitor_no_severity_change_event() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);
        let events = mon.update_zone("cpu", 55_000);
        // Temperature updated but still Normal.
        assert!(events.iter().any(|e| matches!(e, ThermalEvent::TemperatureUpdated { .. })));
        assert!(!events.iter().any(|e| matches!(e, ThermalEvent::SeverityChanged { .. })));
    }

    #[test]
    fn monitor_unknown_zone_no_events() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);
        let events = mon.update_zone("gpu", 90_000);
        assert!(events.is_empty());
    }

    #[test]
    fn fan_hint_normal() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);
        assert_eq!(mon.fan_hint(), FanSpeedHint::Auto);
    }

    #[test]
    fn fan_hint_hot() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(85_000)]);
        assert_eq!(mon.fan_hint(), FanSpeedHint::Percentage(80));
    }

    #[test]
    fn fan_hint_critical() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(105_000)]);
        assert_eq!(mon.fan_hint(), FanSpeedHint::Percentage(100));
    }

    #[test]
    fn worst_severity_across_zones() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![
            cpu_zone(50_000), // Normal
            ThermalZone {
                name: "gpu".into(),
                temperature_milli_celsius: 90_000,
                trip_points: vec![
                    TripPoint {
                        temp_milli_celsius: 70_000,
                        kind: TripPointKind::Active,
                    },
                    TripPoint {
                        temp_milli_celsius: 85_000,
                        kind: TripPointKind::Hot,
                    },
                ],
                policy: ThermalPolicy::Active,
            },
        ]);
        assert_eq!(mon.worst_severity(), ThermalSeverity::Hot);
    }

    #[test]
    fn empty_monitor_normal() {
        let mon = ThermalMonitor::new();
        assert_eq!(mon.worst_severity(), ThermalSeverity::Normal);
        assert_eq!(mon.fan_hint(), FanSpeedHint::Auto);
    }

    #[test]
    fn thermal_severity_ordering() {
        assert!(ThermalSeverity::Normal < ThermalSeverity::Warm);
        assert!(ThermalSeverity::Warm < ThermalSeverity::Hot);
        assert!(ThermalSeverity::Hot < ThermalSeverity::Critical);
    }

    #[test]
    fn thermal_policy_display() {
        assert_eq!(ThermalPolicy::Active.to_string(), "active");
        assert_eq!(ThermalPolicy::Passive.to_string(), "passive");
    }

    #[test]
    fn thermal_severity_display() {
        assert_eq!(ThermalSeverity::Normal.to_string(), "normal");
        assert_eq!(ThermalSeverity::Warm.to_string(), "warm");
        assert_eq!(ThermalSeverity::Hot.to_string(), "hot");
        assert_eq!(ThermalSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn zone_with_no_trip_points_is_normal() {
        let zone = ThermalZone {
            name: "ambient".into(),
            temperature_milli_celsius: 200_000,
            trip_points: vec![],
            policy: ThermalPolicy::Passive,
        };
        assert_eq!(zone.classify(), ThermalSeverity::Normal);
    }

    #[test]
    fn poll_detects_changes() {
        let mut mon = ThermalMonitor::new();
        mon.set_zones(vec![cpu_zone(50_000)]);

        // Manually change temperature and poll.
        mon.zones[0].temperature_milli_celsius = 85_000;
        let events = mon.poll();
        assert!(events.iter().any(|e| matches!(
            e,
            ThermalEvent::SeverityChanged {
                to: ThermalSeverity::Hot,
                ..
            }
        )));
    }
}
