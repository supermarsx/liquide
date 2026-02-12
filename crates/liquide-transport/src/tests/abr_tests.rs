use std::time::Duration;

use crate::abr::{AbrConfig, AbrController, AbrDecision, AbrMetrics};

// ---------------------------------------------------------------------------
// Initial State
// ---------------------------------------------------------------------------

#[test]
fn initial_decision() {
    let ctrl = AbrController::with_defaults();
    let d = ctrl.decision();
    assert_eq!(d.video_fps_cap, 60);
    assert_eq!(d.quality_index, 20);
    assert_eq!(d.keyframe_interval_secs, 5);
    assert_eq!(d.tile_compression_level, 3);
    assert!(d.bandwidth_budget > 0);
}

#[test]
fn initial_stable_ticks() {
    let ctrl = AbrController::with_defaults();
    assert_eq!(ctrl.stable_ticks(), 0);
}

// ---------------------------------------------------------------------------
// Quality Downgrade
// ---------------------------------------------------------------------------

#[test]
fn downgrade_on_high_loss() {
    let mut ctrl = AbrController::with_defaults();
    let initial_qi = ctrl.decision().quality_index;

    let metrics = AbrMetrics {
        loss_rate: 0.05, // Above 0.02 threshold
        ..AbrMetrics::default()
    };
    ctrl.tick(metrics);

    assert!(ctrl.decision().quality_index > initial_qi);
    assert_eq!(ctrl.stable_ticks(), 0);
}

#[test]
fn downgrade_on_high_rtt() {
    let mut ctrl = AbrController::with_defaults();
    let initial_fps = ctrl.decision().video_fps_cap;

    let metrics = AbrMetrics {
        srtt: Duration::from_millis(300), // Above 200ms threshold
        ..AbrMetrics::default()
    };
    ctrl.tick(metrics);

    assert!(ctrl.decision().video_fps_cap < initial_fps);
}

#[test]
fn downgrade_on_cwnd_saturation() {
    let mut ctrl = AbrController::with_defaults();
    let initial_qi = ctrl.decision().quality_index;

    let metrics = AbrMetrics {
        cwnd_occupancy: 0.95, // Above 0.90 threshold
        ..AbrMetrics::default()
    };
    ctrl.tick(metrics);

    assert!(ctrl.decision().quality_index > initial_qi);
}

#[test]
fn downgrade_clamps_quality_index() {
    let mut ctrl = AbrController::with_defaults();
    // Force many downgrades
    for _ in 0..50 {
        ctrl.tick(AbrMetrics {
            loss_rate: 0.10,
            ..AbrMetrics::default()
        });
    }
    assert!(ctrl.decision().quality_index <= 51);
}

#[test]
fn downgrade_clamps_fps() {
    let mut ctrl = AbrController::with_defaults();
    for _ in 0..50 {
        ctrl.tick(AbrMetrics {
            loss_rate: 0.10,
            ..AbrMetrics::default()
        });
    }
    assert!(ctrl.decision().video_fps_cap >= 1);
}

// ---------------------------------------------------------------------------
// Quality Upgrade
// ---------------------------------------------------------------------------

#[test]
fn upgrade_after_stability() {
    let config = AbrConfig {
        upgrade_stability_ticks: 3,
        ..AbrConfig::default()
    };
    let mut ctrl = AbrController::new(config);

    // First downgrade to give room to upgrade
    ctrl.tick(AbrMetrics {
        loss_rate: 0.05,
        ..AbrMetrics::default()
    });
    let degraded_qi = ctrl.decision().quality_index;

    // 3 stable ticks → should upgrade
    let good = AbrMetrics::default();
    ctrl.tick(good);
    ctrl.tick(good);
    ctrl.tick(good);

    assert!(ctrl.decision().quality_index < degraded_qi);
}

#[test]
fn upgrade_clamps_quality_index() {
    let config = AbrConfig {
        upgrade_stability_ticks: 1,
        ..AbrConfig::default()
    };
    let mut ctrl = AbrController::new(config);

    // Many stable ticks
    let good = AbrMetrics::default();
    for _ in 0..100 {
        ctrl.tick(good);
    }
    assert_eq!(ctrl.decision().quality_index, 0);
}

#[test]
fn upgrade_clamps_fps() {
    let config = AbrConfig {
        upgrade_stability_ticks: 1,
        ..AbrConfig::default()
    };
    let mut ctrl = AbrController::new(config);

    let good = AbrMetrics::default();
    for _ in 0..100 {
        ctrl.tick(good);
    }
    assert_eq!(ctrl.decision().video_fps_cap, 60);
}

// ---------------------------------------------------------------------------
// Bandwidth Budget
// ---------------------------------------------------------------------------

#[test]
fn bandwidth_reduces_on_downgrade() {
    let mut ctrl = AbrController::with_defaults();
    let initial_bw = ctrl.decision().bandwidth_budget;

    ctrl.tick(AbrMetrics {
        loss_rate: 0.05,
        ..AbrMetrics::default()
    });

    assert!(ctrl.decision().bandwidth_budget < initial_bw);
}

#[test]
fn bandwidth_increases_on_upgrade() {
    let config = AbrConfig {
        upgrade_stability_ticks: 1,
        ..AbrConfig::default()
    };
    let mut ctrl = AbrController::new(config);

    let bw_before = ctrl.decision().bandwidth_budget;
    ctrl.tick(AbrMetrics::default());
    assert!(ctrl.decision().bandwidth_budget > bw_before);
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_restores_defaults() {
    let mut ctrl = AbrController::with_defaults();

    // Degrade
    for _ in 0..10 {
        ctrl.tick(AbrMetrics {
            loss_rate: 0.10,
            ..AbrMetrics::default()
        });
    }

    ctrl.reset();
    let d = ctrl.decision();
    let def = AbrDecision::default();
    assert_eq!(d.video_fps_cap, def.video_fps_cap);
    assert_eq!(d.quality_index, def.quality_index);
    assert_eq!(ctrl.stable_ticks(), 0);
}
