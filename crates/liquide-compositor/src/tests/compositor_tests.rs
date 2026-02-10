use crate::compositor::*;
use crate::geometry::Rect;
use crate::scene::{NodeProperties, SceneNodeKind};

use crate::effects::QualityProfile;
use crate::scene::{GlassParams, SceneNode};
use crate::cursor::CursorUpdate;

#[test]
fn compositor_create() {
    let comp = Compositor::new(1920, 1080, 64, QualityProfile::Balanced);
    assert_eq!(comp.width(), 1920);
    assert_eq!(comp.height(), 1080);
    assert_eq!(comp.tile_size(), 64);
}

#[test]
fn compositor_submit_scene() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    let root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 800.0, 600.0)),
    );
    comp.submit_scene(root).unwrap();
    assert!(comp.scene().is_some());
}

#[test]
fn compositor_first_frame_damages_all() {
    let mut comp = Compositor::new(128, 128, 64, QualityProfile::Balanced);
    let damage = comp.compute_damage().unwrap();
    // 128/64 = 2x2 = 4 tiles, all damaged on first frame
    assert_eq!(damage.len(), 4);
}

#[test]
fn compositor_register_glass() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    comp.register_glass(42, GlassParams::default()).unwrap();
    comp.register_glass(42, GlassParams { blur_radius: 30, ..GlassParams::default() })
        .unwrap();
    // Should have replaced, not duplicated
    assert_eq!(comp.glass_surfaces.len(), 1);
}

#[test]
fn compositor_cursor() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    assert!(comp.cursor_update().is_none());
    comp.set_cursor(CursorUpdate::position_only(100, 200));
    let c = comp.cursor_update().unwrap();
    assert_eq!(c.x, 100);
    assert_eq!(c.y, 200);
}

#[test]
fn compositor_resize() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    comp.resize(1920, 1080).unwrap();
    assert_eq!(comp.width(), 1920);
    assert_eq!(comp.height(), 1080);
}

#[test]
fn compositor_begin_frame() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    comp.begin_frame();
    let budget = comp.effect_budget();
    assert_eq!(budget.profile, QualityProfile::Balanced);
    assert!(budget.total_frame_budget_ms > 0.0);
}

#[test]
fn compositor_report_frame_time() {
    let mut comp = Compositor::new(800, 600, 64, QualityProfile::Balanced);
    // Report under-budget frames — should not change level
    for _ in 0..5 {
        comp.report_frame_time(5.0);
    }
    // Should still be at L0 or close
    let params = comp.effect_params();
    assert!(params.blur_radius > 0); // not degraded
}

#[test]
fn compositor_width_height_tile_size() {
    let comp = Compositor::new(1920, 1080, 64, QualityProfile::Quality);
    assert_eq!(comp.width(), 1920);
    assert_eq!(comp.height(), 1080);
    assert_eq!(comp.tile_size(), 64);
}
