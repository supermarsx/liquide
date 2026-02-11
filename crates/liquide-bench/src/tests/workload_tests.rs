//! Tests for workload profiles, network presets, SLO definitions, and suite
//! selection.

use crate::config::SuiteSelection;
use crate::measurement::BenchMetrics;
use crate::network::{NetworkEmulator, NetworkPreset};
use crate::slo::{Slo, SloComparator, SloSet};
use crate::workload::{WorkloadParams, WorkloadProfile};

// ===========================================================================
// WorkloadProfile
// ===========================================================================

#[test]
fn workload_profile_labels() {
    assert_eq!(WorkloadProfile::Idle.label(), "idle");
    assert_eq!(WorkloadProfile::TextEditing.label(), "text-editing");
    assert_eq!(WorkloadProfile::VideoPlayback.label(), "video-playback");
    assert_eq!(WorkloadProfile::DesktopWorkflow.label(), "desktop-workflow");
}

#[test]
fn workload_profile_display() {
    assert_eq!(WorkloadProfile::Dashboard.to_string(), "dashboard");
    assert_eq!(WorkloadProfile::Presentation.to_string(), "presentation");
}

#[test]
fn workload_profile_fps_ranges_valid() {
    for profile in crate::workload::ALL {
        let (min, max) = profile.expected_fps_range();
        assert!(min <= max, "{}: min ({min}) > max ({max})", profile.label());
        assert!(max <= 240, "{}: max fps too high ({max})", profile.label());
    }
}

#[test]
fn workload_profile_bandwidth_ranges_valid() {
    for profile in crate::workload::ALL {
        let (min, max) = profile.expected_bandwidth_range();
        assert!(
            min <= max,
            "{}: min ({min}) > max ({max})",
            profile.label()
        );
    }
}

#[test]
fn workload_params_tiles() {
    let params = WorkloadParams {
        profile: WorkloadProfile::DesktopWorkflow,
        resolution_width: 1920,
        resolution_height: 1080,
        tile_size: 64,
        ..WorkloadParams::default()
    };
    assert_eq!(params.tiles_x(), 30);
    assert_eq!(params.tiles_y(), 17); // ceil(1080/64) = 17
    assert_eq!(params.total_tiles(), 510);
}

#[test]
fn workload_params_damaged_tiles() {
    let params = WorkloadParams {
        profile: WorkloadProfile::VideoPlayback,
        resolution_width: 1920,
        resolution_height: 1080,
        tile_size: 64,
        ..WorkloadParams::default()
    };
    // VideoPlayback has damage_fraction = 1.0, so all tiles are damaged.
    assert_eq!(params.damaged_tiles_per_frame(), params.total_tiles());

    let idle_params = WorkloadParams {
        profile: WorkloadProfile::Idle,
        ..params.clone()
    };
    // Idle has damage_fraction = 0.001, so very few tiles.
    let damaged = idle_params.damaged_tiles_per_frame();
    assert!(damaged >= 1);
    assert!(damaged < idle_params.total_tiles());
}

// ===========================================================================
// NetworkPreset
// ===========================================================================

#[test]
fn network_preset_labels() {
    assert_eq!(NetworkPreset::Lan.label(), "lan");
    assert_eq!(NetworkPreset::WanGood.label(), "wan-good");
    assert_eq!(NetworkPreset::Satellite.label(), "satellite");
}

#[test]
fn network_preset_display() {
    assert_eq!(NetworkPreset::Cellular4g.to_string(), "4g");
    assert_eq!(NetworkPreset::HotelWifi.to_string(), "hotel-wifi");
}

#[test]
fn network_preset_to_profile() {
    let lan = NetworkPreset::Lan.to_profile();
    assert_eq!(lan.name, "lan");
    assert!(lan.rtt_ms < 1.0);
    assert!(lan.packet_loss_percent == 0.0);

    let sat = NetworkPreset::Satellite.to_profile();
    assert!(sat.rtt_ms > 500.0);
}

#[test]
fn network_preset_from_name() {
    assert_eq!(
        NetworkPreset::from_name("lan").unwrap() as u8,
        NetworkPreset::Lan as u8
    );
    assert_eq!(
        NetworkPreset::from_name("wan-good").unwrap() as u8,
        NetworkPreset::WanGood as u8
    );
    assert_eq!(
        NetworkPreset::from_name("4g").unwrap() as u8,
        NetworkPreset::Cellular4g as u8
    );
    assert!(NetworkPreset::from_name("unknown").is_err());
}

#[test]
fn network_emulator_rtt_deterministic() {
    let emulator = NetworkEmulator::from_preset(NetworkPreset::WanGood);
    let rtt1 = emulator.simulated_rtt(0);
    let rtt2 = emulator.simulated_rtt(0);
    assert_eq!(rtt1, rtt2);

    // Different iterations produce different RTTs.
    let rtt3 = emulator.simulated_rtt(1);
    // They should be in the range of rtt +/- jitter.
    let profile = emulator.profile();
    assert!(rtt1 >= 0.1);
    assert!(rtt1 <= profile.rtt_ms + profile.jitter_ms * 2.0);
    assert!(rtt3 >= 0.1);
}

#[test]
fn network_emulator_effective_bandwidth() {
    let emulator = NetworkEmulator::from_preset(NetworkPreset::Lan);
    let bw = emulator.effective_bandwidth();
    // 1000 Mbps * 1_000_000 / 8 * 0.95 = ~118_750_000 bytes/sec
    assert!(bw > 100_000_000.0);
    assert!(bw < 200_000_000.0);
}

#[test]
fn network_emulator_packet_loss_zero() {
    let mut emulator = NetworkEmulator::from_preset(NetworkPreset::Lan);
    // LAN has 0% loss.
    for _ in 0..100 {
        assert!(!emulator.should_drop_packet());
    }
}

#[test]
fn network_emulator_reset() {
    let mut emulator = NetworkEmulator::from_preset(NetworkPreset::HotelWifi);
    for _ in 0..10 {
        emulator.should_drop_packet();
    }
    emulator.reset();
    // After reset, packet counter starts from 0 again, so results should
    // be the same as a fresh emulator.
    let mut fresh = NetworkEmulator::from_preset(NetworkPreset::HotelWifi);
    for _ in 0..10 {
        assert_eq!(
            emulator.should_drop_packet(),
            fresh.should_drop_packet()
        );
    }
}

// ===========================================================================
// SLO definitions and checking
// ===========================================================================

#[test]
fn slo_comparator_less_than() {
    assert!(SloComparator::LessThan.check(5.0, 10.0));
    assert!(!SloComparator::LessThan.check(10.0, 10.0));
    assert!(!SloComparator::LessThan.check(15.0, 10.0));
}

#[test]
fn slo_comparator_less_than_or_equal() {
    assert!(SloComparator::LessThanOrEqual.check(10.0, 10.0));
    assert!(!SloComparator::LessThanOrEqual.check(10.1, 10.0));
}

#[test]
fn slo_comparator_greater_than_or_equal() {
    assert!(SloComparator::GreaterThanOrEqual.check(60.0, 60.0));
    assert!(SloComparator::GreaterThanOrEqual.check(61.0, 60.0));
    assert!(!SloComparator::GreaterThanOrEqual.check(59.0, 60.0));
}

#[test]
fn slo_check_pass() {
    let slo = Slo::new("fps", 60.0, SloComparator::GreaterThanOrEqual, "fps");
    let result = slo.check(65.0);
    assert!(result.passed);
    assert_eq!(result.actual_value, 65.0);
}

#[test]
fn slo_check_fail() {
    let slo = Slo::new("latency_p50", 16.0, SloComparator::LessThan, "ms");
    let result = slo.check(20.0);
    assert!(!result.passed);
}

#[test]
fn slo_display() {
    let slo = Slo::new("fps", 60.0, SloComparator::GreaterThanOrEqual, "fps");
    let text = slo.to_string();
    assert!(text.contains("fps"));
    assert!(text.contains(">="));
    assert!(text.contains("60.00"));
}

#[test]
fn slo_set_default_lan() {
    let set = SloSet::default_lan();
    assert_eq!(set.name, "lan");
    assert_eq!(set.slos.len(), 5);
}

#[test]
fn slo_set_default_wan() {
    let set = SloSet::default_wan();
    assert_eq!(set.name, "wan");
    assert_eq!(set.slos.len(), 2);
}

#[test]
fn slo_set_check_single() {
    let set = SloSet::default_lan();
    let result = set.check("fps", 65.0);
    assert!(result.is_some());
    assert!(result.unwrap().passed);

    let result = set.check("fps", 50.0);
    assert!(result.is_some());
    assert!(!result.unwrap().passed);

    // Non-existent metric.
    assert!(set.check("nonexistent", 0.0).is_none());
}

#[test]
fn slo_set_check_all() {
    let set = SloSet::default_lan();
    let mut metrics = BenchMetrics::new();
    // Record values that pass all SLOs.
    for i in 0..100 {
        let t = i as u64 * 1000;
        metrics.record("input_to_photon", t, 10.0); // p50 < 16, p99 < 25
        metrics.record("cursor", t, 3.0); // p50 < 5
        metrics.record("fps", t, 62.0); // mean >= 60
    }
    metrics.record("first_frame", 0, 300.0); // < 500

    let results = set.check_all(&metrics);
    assert!(set.all_passed(&results));
}

#[test]
fn slo_set_check_all_with_failure() {
    let set = SloSet::default_lan();
    let mut metrics = BenchMetrics::new();
    for i in 0..100 {
        let t = i as u64 * 1000;
        metrics.record("input_to_photon", t, 20.0); // p50 = 20 > 16 -> FAIL
        metrics.record("cursor", t, 3.0);
        metrics.record("fps", t, 62.0);
    }
    metrics.record("first_frame", 0, 300.0);

    let results = set.check_all(&metrics);
    assert!(!set.all_passed(&results));

    // At least input_to_photon_p50 should fail.
    let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    assert!(!failed.is_empty());
}

// ===========================================================================
// SuiteSelection
// ===========================================================================

#[test]
fn suite_selection_from_name() {
    assert_eq!(
        SuiteSelection::from_name("all").unwrap(),
        SuiteSelection::All
    );
    assert_eq!(
        SuiteSelection::from_name("compositor").unwrap(),
        SuiteSelection::Compositor
    );
    assert_eq!(
        SuiteSelection::from_name("encoder").unwrap(),
        SuiteSelection::Encoder
    );
    assert_eq!(
        SuiteSelection::from_name("protocol").unwrap(),
        SuiteSelection::Protocol
    );
    assert_eq!(
        SuiteSelection::from_name("ci-quick").unwrap(),
        SuiteSelection::CiQuick
    );
    assert!(SuiteSelection::from_name("invalid").is_err());
}

#[test]
fn suite_selection_includes() {
    let all = SuiteSelection::All;
    assert!(all.includes_compositor());
    assert!(all.includes_encoder());
    assert!(all.includes_protocol());

    let comp = SuiteSelection::Compositor;
    assert!(comp.includes_compositor());
    assert!(!comp.includes_encoder());
    assert!(!comp.includes_protocol());

    let ci = SuiteSelection::CiQuick;
    assert!(ci.includes_compositor());
    assert!(ci.includes_encoder());
    assert!(ci.includes_protocol());
}

#[test]
fn suite_selection_label_display() {
    assert_eq!(SuiteSelection::All.label(), "all");
    assert_eq!(SuiteSelection::CiFull.to_string(), "ci-full");
}
