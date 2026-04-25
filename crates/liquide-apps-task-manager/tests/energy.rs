//! Tests for `energy` submodule types.

use liquide_apps_task_manager::energy::battery::*;
use liquide_apps_task_manager::energy::carbon::*;
use liquide_apps_task_manager::energy::component::*;
use liquide_apps_task_manager::energy::history::*;
use liquide_apps_task_manager::energy::power_plan::*;
use liquide_apps_task_manager::energy::process_energy::*;
use liquide_apps_task_manager::energy::thermal::*;
use liquide_apps_task_manager::energy::wake_lock::*;
use liquide_apps_task_manager::energy::*;

// ---------------------------------------------------------------------------
// EnergyView
// ---------------------------------------------------------------------------

#[test]
fn energy_view_all_variants() {
    let variants = [
        EnergyView::Overview,
        EnergyView::PerProcess,
        EnergyView::Components,
        EnergyView::Battery,
        EnergyView::Thermal,
        EnergyView::PowerPlan,
        EnergyView::History,
        EnergyView::Carbon,
        EnergyView::WakeLocks,
    ];
    assert_eq!(variants.len(), 9);
}

#[test]
fn energy_view_display() {
    assert_eq!(EnergyView::Overview.to_string(), "Overview");
    assert_eq!(EnergyView::PerProcess.to_string(), "Per Process");
    assert_eq!(EnergyView::WakeLocks.to_string(), "Wake Locks");
}

// ---------------------------------------------------------------------------
// PowerSource
// ---------------------------------------------------------------------------

#[test]
fn power_source_all_variants() {
    let variants = [PowerSource::Ac, PowerSource::Battery, PowerSource::Usb];
    assert_eq!(variants.len(), 3);
}

#[test]
fn power_source_display() {
    assert_eq!(PowerSource::Ac.to_string(), "AC");
    assert_eq!(PowerSource::Battery.to_string(), "Battery");
    assert_eq!(PowerSource::Usb.to_string(), "USB");
}

#[test]
fn power_source_serde_roundtrip() {
    let val = PowerSource::Battery;
    let json = serde_json::to_string(&val).unwrap();
    let back: PowerSource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// BatteryState
// ---------------------------------------------------------------------------

#[test]
fn battery_state_all_variants() {
    let variants = [
        BatteryState::Charging,
        BatteryState::Discharging,
        BatteryState::Full,
        BatteryState::NotCharging,
        BatteryState::Unknown,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn battery_state_display() {
    assert_eq!(BatteryState::Charging.to_string(), "Charging");
    assert_eq!(BatteryState::Full.to_string(), "Full");
    assert_eq!(BatteryState::NotCharging.to_string(), "Not Charging");
}

// ---------------------------------------------------------------------------
// BatteryChemistry
// ---------------------------------------------------------------------------

#[test]
fn battery_chemistry_all_variants() {
    let variants = [
        BatteryChemistry::LithiumIon,
        BatteryChemistry::LithiumPolymer,
        BatteryChemistry::NickelMetalHydride,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// BatteryStatus
// ---------------------------------------------------------------------------

#[test]
fn battery_status_default() {
    let status = BatteryStatus::default();
    assert!(!status.present);
    assert_eq!(status.state, BatteryState::Unknown);
    assert_eq!(status.charge_percent, 0.0);
}

#[test]
fn battery_status_serde_roundtrip() {
    let status = BatteryStatus {
        present: true,
        state: BatteryState::Charging,
        chemistry: BatteryChemistry::LithiumPolymer,
        charge_percent: 75.0,
        voltage_mv: 11400,
        current_ma: 2000,
        temperature_celsius: Some(35.0),
        design_capacity_mwh: 50000,
        full_charge_capacity_mwh: 45000,
        remaining_capacity_mwh: 33750,
        charge_rate_watts: Some(45.0),
        discharge_rate_watts: None,
        time_to_full_secs: Some(1800),
        time_to_empty_secs: None,
        cycle_count: Some(200),
        health_percent: Some(90.0),
        manufacturer: Some("Samsung".into()),
        serial_number: Some("BAT-12345".into()),
    };
    let json = serde_json::to_string(&status).unwrap();
    let back: BatteryStatus = serde_json::from_str(&json).unwrap();
    assert!(back.present);
    assert_eq!(back.charge_percent, 75.0);
}

// ---------------------------------------------------------------------------
// ThermalStatus
// ---------------------------------------------------------------------------

#[test]
fn thermal_status_all_variants() {
    let variants = [
        ThermalStatus::Normal,
        ThermalStatus::Warm,
        ThermalStatus::Hot,
        ThermalStatus::Critical,
        ThermalStatus::Emergency,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn thermal_status_display() {
    assert_eq!(ThermalStatus::Normal.to_string(), "Normal");
    assert_eq!(ThermalStatus::Critical.to_string(), "Critical");
    assert_eq!(ThermalStatus::Emergency.to_string(), "Emergency");
}

// ---------------------------------------------------------------------------
// ThermalTrend
// ---------------------------------------------------------------------------

#[test]
fn thermal_trend_all_variants() {
    let variants = [
        ThermalTrend::Rising,
        ThermalTrend::Stable,
        ThermalTrend::Falling,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// FanMode
// ---------------------------------------------------------------------------

#[test]
fn fan_mode_all_variants() {
    let variants = [
        FanMode::Auto,
        FanMode::Manual,
        FanMode::Silent,
        FanMode::Performance,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn fan_mode_display() {
    assert_eq!(FanMode::Auto.to_string(), "Auto");
    assert_eq!(FanMode::Silent.to_string(), "Silent");
    assert_eq!(FanMode::Performance.to_string(), "Performance");
}

// ---------------------------------------------------------------------------
// EfficiencyRating
// ---------------------------------------------------------------------------

#[test]
fn efficiency_rating_all_variants() {
    let variants = [
        EfficiencyRating::VeryLow,
        EfficiencyRating::Low,
        EfficiencyRating::Moderate,
        EfficiencyRating::High,
        EfficiencyRating::VeryHigh,
        EfficiencyRating::Critical,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn efficiency_rating_display() {
    assert_eq!(EfficiencyRating::VeryLow.to_string(), "Very Low");
    assert_eq!(EfficiencyRating::Critical.to_string(), "Critical");
}

// ---------------------------------------------------------------------------
// ProcessEnergyInfo
// ---------------------------------------------------------------------------

#[test]
fn process_energy_info_default() {
    let info = ProcessEnergyInfo::default();
    assert_eq!(info.pid, 0);
    assert_eq!(info.total_power_mw, 0.0);
    assert_eq!(info.efficiency_rating, EfficiencyRating::VeryLow);
}

// ---------------------------------------------------------------------------
// ThermalSensor construction
// ---------------------------------------------------------------------------

#[test]
fn thermal_sensor_construction() {
    let sensor = ThermalSensor {
        name: "CPU Package".into(),
        location: "CPU".into(),
        temperature_celsius: 65.0,
        max_temperature_celsius: 80.0,
        critical_temperature_celsius: 100.0,
        status: ThermalStatus::Normal,
        trend: ThermalTrend::Stable,
        min_recorded: 30.0,
        max_recorded: 75.0,
        avg_temperature: 55.0,
        reading_count: 1000,
    };
    assert_eq!(sensor.name, "CPU Package");
    assert_eq!(sensor.status, ThermalStatus::Normal);
}

// ---------------------------------------------------------------------------
// FanInfo construction
// ---------------------------------------------------------------------------

#[test]
fn fan_info_construction() {
    let fan = FanInfo {
        name: "CPU Fan".into(),
        speed_rpm: 1500,
        max_speed_rpm: 3000,
        speed_percent: 50.0,
        mode: FanMode::Auto,
        controllable: true,
    };
    assert_eq!(fan.speed_rpm, 1500);
    assert!(fan.controllable);
}

// ---------------------------------------------------------------------------
// CarbonIntensitySource
// ---------------------------------------------------------------------------

#[test]
fn carbon_intensity_source_all_variants() {
    let variants = [
        CarbonIntensitySource::Grid,
        CarbonIntensitySource::Average,
        CarbonIntensitySource::Manual,
        CarbonIntensitySource::Estimated,
    ];
    assert_eq!(variants.len(), 4);
}

// ---------------------------------------------------------------------------
// CoolingPolicy
// ---------------------------------------------------------------------------

#[test]
fn cooling_policy_all_variants() {
    let variants = [CoolingPolicy::Active, CoolingPolicy::Passive];
    assert_eq!(variants.len(), 2);
}

// ---------------------------------------------------------------------------
// PanelType
// ---------------------------------------------------------------------------

#[test]
fn panel_type_all_variants() {
    let variants = [PanelType::Lcd, PanelType::Oled, PanelType::MiniLed];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// WakeLockType
// ---------------------------------------------------------------------------

#[test]
fn wake_lock_type_all_variants() {
    let variants = [
        WakeLockType::System,
        WakeLockType::Display,
        WakeLockType::PartialWake,
        WakeLockType::ProximityWake,
    ];
    assert_eq!(variants.len(), 4);
}

// ---------------------------------------------------------------------------
// EnergyOverview construction
// ---------------------------------------------------------------------------

#[test]
fn energy_overview_construction() {
    let overview = EnergyOverview {
        power_source: PowerSource::Battery,
        total_power_watts: 25.0,
        cpu_power_watts: 10.0,
        gpu_power_watts: 5.0,
        display_power_watts: 4.0,
        storage_power_watts: 2.0,
        network_power_watts: 1.0,
        peripheral_power_watts: 3.0,
        battery_percent: Some(80.0),
        battery_remaining_secs: Some(7200),
        energy_rating: "Good".into(),
    };
    assert_eq!(overview.power_source, PowerSource::Battery);
    assert_eq!(overview.total_power_watts, 25.0);
}

// ---------------------------------------------------------------------------
// EnergyHistoryEntry & EnergyReport
// ---------------------------------------------------------------------------

#[test]
fn energy_history_entry_construction() {
    let entry = EnergyHistoryEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        power_watts: 30.0,
        source: "AC".into(),
        battery_percent: None,
        cpu_percent: 25.0,
        gpu_percent: 10.0,
        screen_brightness: 75,
    };
    assert_eq!(entry.power_watts, 30.0);
}

#[test]
fn energy_report_construction() {
    let report = EnergyReport {
        period: "Last 24 hours".into(),
        total_energy_wh: 500.0,
        avg_power_watts: 20.8,
        peak_power_watts: 65.0,
        battery_drain_events: 2,
        screen_on_hours: 12.0,
        sleep_hours: 8.0,
        top_consumers: vec!["firefox".into(), "code".into()],
    };
    assert_eq!(report.total_energy_wh, 500.0);
    assert_eq!(report.top_consumers.len(), 2);
}
