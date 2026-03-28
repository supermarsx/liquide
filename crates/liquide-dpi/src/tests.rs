//! Tests for the liquide-dpi crate.

use crate::geometry::*;
use crate::monitor::*;
use crate::platform::PlatformDpi;
use crate::scale::*;
use crate::fractional::*;
use crate::per_monitor::*;
use crate::text_scaling::*;
use crate::cursor_scale::*;
use crate::xsettings::*;

// ── DpiScale tests ────────────────────────────────────────────────────

#[test]
fn scale_identity() {
    let s = DpiScale::identity();
    assert_eq!(s.factor(), 1.0);
    assert_eq!(s.dpi(), 96.0);
    assert!(!s.is_hidpi());
}

#[test]
fn scale_from_dpi() {
    let s = DpiScale::from_dpi(144.0);
    assert_eq!(s.factor(), 1.5);
    assert_eq!(s.dpi(), 144.0);
    assert!(s.is_hidpi());
}

#[test]
fn scale_from_dpi_retina() {
    let s = DpiScale::from_dpi(192.0);
    assert_eq!(s.factor(), 2.0);
    assert!(s.is_hidpi());
}

#[test]
fn scale_clamping() {
    let too_low = DpiScale::new(0.01);
    assert_eq!(too_low.factor(), DpiScale::MIN);

    let too_high = DpiScale::new(100.0);
    assert_eq!(too_high.factor(), DpiScale::MAX);

    // Normal range is preserved.
    let normal = DpiScale::new(1.5);
    assert_eq!(normal.factor(), 1.5);
}

#[test]
fn scale_logical_physical_roundtrip() {
    let s = DpiScale::new(2.0);
    let logical = 100.0f32;
    let physical = s.to_physical(logical);
    assert_eq!(physical, 200.0);
    let back = s.to_logical(physical);
    assert_eq!(back, logical);
}

#[test]
fn scale_default_is_identity() {
    let s = DpiScale::default();
    assert_eq!(s.factor(), 1.0);
}

#[test]
fn scale_from_f32() {
    let s: DpiScale = 1.25f32.into();
    assert_eq!(s.factor(), 1.25);
}

#[test]
fn scale_display() {
    let s = DpiScale::new(2.0);
    let text = format!("{s}");
    assert!(text.contains("2x"));
    assert!(text.contains("192"));
}

// ── ScaleRounding tests ───────────────────────────────────────────────

#[test]
fn rounding_floor() {
    assert_eq!(ScaleRounding::Floor.apply(1.7), 1.0);
    assert_eq!(ScaleRounding::Floor.apply(1.2), 1.0);
    assert_eq!(ScaleRounding::Floor.apply(-0.3), -1.0);
}

#[test]
fn rounding_ceil() {
    assert_eq!(ScaleRounding::Ceil.apply(1.1), 2.0);
    assert_eq!(ScaleRounding::Ceil.apply(1.0), 1.0);
    assert_eq!(ScaleRounding::Ceil.apply(-0.3), 0.0);
}

#[test]
fn rounding_round() {
    assert_eq!(ScaleRounding::Round.apply(1.5), 2.0);
    assert_eq!(ScaleRounding::Round.apply(1.4), 1.0);
    assert_eq!(ScaleRounding::Round.apply(2.5), 3.0);
}

#[test]
fn rounding_nearest_bankers() {
    // Exactly halfway: round to even.
    assert_eq!(ScaleRounding::Nearest.apply(0.5), 0.0); // 0 is even
    assert_eq!(ScaleRounding::Nearest.apply(1.5), 2.0); // 2 is even
    assert_eq!(ScaleRounding::Nearest.apply(2.5), 2.0); // 2 is even
    assert_eq!(ScaleRounding::Nearest.apply(3.5), 4.0); // 4 is even
    // Not halfway: behaves like round.
    assert_eq!(ScaleRounding::Nearest.apply(1.3), 1.0);
    assert_eq!(ScaleRounding::Nearest.apply(1.7), 2.0);
}

#[test]
fn rounding_default_is_round() {
    assert_eq!(ScaleRounding::default(), ScaleRounding::Round);
}

// ── Snap-to-pixel tests ──────────────────────────────────────────────

#[test]
fn snap_identity_scale() {
    let s = DpiScale::identity();
    // At 1x, snapping rounds to nearest integer.
    assert_eq!(snap_to_pixel(10.3, s), 10.0);
    assert_eq!(snap_to_pixel(10.7, s), 11.0);
}

#[test]
fn snap_2x_scale() {
    let s = DpiScale::new(2.0);
    // At 2x, half-pixel steps are valid physical positions.
    assert_eq!(snap_to_pixel(10.3, s), 10.5); // 10.3*2=20.6 -> 21 -> 21/2=10.5
    assert_eq!(snap_to_pixel(10.0, s), 10.0);
}

#[test]
fn snap_1_5x_scale() {
    let s = DpiScale::new(1.5);
    // 10.0 * 1.5 = 15.0 -> 15 -> 15/1.5 = 10.0
    assert_eq!(snap_to_pixel(10.0, s), 10.0);
    // 10.5 * 1.5 = 15.75 -> 16 -> 16/1.5 = 10.666...
    let snapped = snap_to_pixel(10.5, s);
    assert!((snapped - 10.666_667).abs() < 0.001);
}

#[test]
fn snap_with_floor() {
    let s = DpiScale::new(1.5);
    // 10.5 * 1.5 = 15.75 -> floor 15 -> 15/1.5 = 10.0
    assert_eq!(snap_to_pixel_with(10.5, s, ScaleRounding::Floor), 10.0);
}

#[test]
fn snap_with_ceil() {
    let s = DpiScale::new(1.5);
    // 10.0 * 1.5 = 15.0 -> ceil 15 -> 15/1.5 = 10.0
    assert_eq!(snap_to_pixel_with(10.0, s, ScaleRounding::Ceil), 10.0);
    // 10.1 * 1.5 = 15.15 -> ceil 16 -> 16/1.5 = 10.666...
    let snapped = snap_to_pixel_with(10.1, s, ScaleRounding::Ceil);
    assert!((snapped - 10.666_667).abs() < 0.001);
}

// ── LogicalSize / PhysicalSize tests ─────────────────────────────────

#[test]
fn logical_size_to_physical() {
    let logical = LogicalSize::new(100.0, 50.0);
    let s = DpiScale::new(2.0);
    let physical = logical.to_physical(s);
    assert_eq!(physical.width, 200);
    assert_eq!(physical.height, 100);
}

#[test]
fn physical_size_to_logical() {
    let physical = PhysicalSize::new(300, 150);
    let s = DpiScale::new(1.5);
    let logical = physical.to_logical(s);
    assert_eq!(logical.width, 200.0);
    assert_eq!(logical.height, 100.0);
}

#[test]
fn size_area() {
    assert_eq!(LogicalSize::new(10.0, 20.0).area(), 200.0);
    assert_eq!(PhysicalSize::new(10, 20).area(), 200);
}

#[test]
fn size_zero() {
    assert!(!LogicalSize::zero().is_positive());
    assert!(!PhysicalSize::zero().is_positive());
    assert!(LogicalSize::new(1.0, 1.0).is_positive());
}

#[test]
fn framebuffer_size_bytes() {
    let size = PhysicalSize::new(1920, 1080);
    assert_eq!(size.framebuffer_size_bytes(4), 1920 * 1080 * 4);
}

// ── LogicalPoint / PhysicalPoint tests ───────────────────────────────

#[test]
fn point_conversions() {
    let lp = LogicalPoint::new(50.0, 75.0);
    let s = DpiScale::new(2.0);
    let pp = lp.to_physical(s);
    assert_eq!(pp.x, 100);
    assert_eq!(pp.y, 150);

    let back = pp.to_logical(s);
    assert_eq!(back.x, 50.0);
    assert_eq!(back.y, 75.0);
}

#[test]
fn point_offset() {
    let lp = LogicalPoint::new(10.0, 20.0);
    let moved = lp.offset(5.0, -3.0);
    assert_eq!(moved.x, 15.0);
    assert_eq!(moved.y, 17.0);
}

#[test]
fn point_distance() {
    let a = LogicalPoint::new(0.0, 0.0);
    let b = LogicalPoint::new(3.0, 4.0);
    assert!((a.distance_to(b) - 5.0).abs() < f32::EPSILON);
}

// ── LogicalRect / PhysicalRect tests ─────────────────────────────────

#[test]
fn rect_to_physical() {
    let lr = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
    let s = DpiScale::new(2.0);
    let pr = lr.to_physical(s);
    assert_eq!(pr.x, 20);
    assert_eq!(pr.y, 40);
    assert_eq!(pr.width, 200);
    assert_eq!(pr.height, 100);
}

#[test]
fn rect_to_logical() {
    let pr = PhysicalRect::new(20, 40, 200, 100);
    let s = DpiScale::new(2.0);
    let lr = pr.to_logical(s);
    assert_eq!(lr.x, 10.0);
    assert_eq!(lr.y, 20.0);
    assert_eq!(lr.width, 100.0);
    assert_eq!(lr.height, 50.0);
}

#[test]
fn rect_contains_point() {
    let r = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
    assert!(r.contains_point(LogicalPoint::new(50.0, 40.0)));
    assert!(!r.contains_point(LogicalPoint::new(5.0, 40.0)));
    assert!(!r.contains_point(LogicalPoint::new(50.0, 80.0)));
}

#[test]
fn rect_intersects() {
    let a = LogicalRect::new(0.0, 0.0, 100.0, 100.0);
    let b = LogicalRect::new(50.0, 50.0, 100.0, 100.0);
    assert!(a.intersects(b));

    let c = LogicalRect::new(200.0, 200.0, 10.0, 10.0);
    assert!(!a.intersects(c));
}

#[test]
fn rect_intersection() {
    let a = LogicalRect::new(0.0, 0.0, 100.0, 100.0);
    let b = LogicalRect::new(50.0, 50.0, 100.0, 100.0);
    let inter = a.intersection(b).unwrap();
    assert_eq!(inter.x, 50.0);
    assert_eq!(inter.y, 50.0);
    assert_eq!(inter.width, 50.0);
    assert_eq!(inter.height, 50.0);

    let c = LogicalRect::new(200.0, 200.0, 10.0, 10.0);
    assert!(a.intersection(c).is_none());
}

#[test]
fn rect_from_point_size() {
    let r = LogicalRect::from_point_size(
        LogicalPoint::new(10.0, 20.0),
        LogicalSize::new(100.0, 50.0),
    );
    assert_eq!(r.x, 10.0);
    assert_eq!(r.y, 20.0);
    assert_eq!(r.width, 100.0);
    assert_eq!(r.height, 50.0);
}

#[test]
fn rect_center() {
    let r = LogicalRect::new(0.0, 0.0, 100.0, 200.0);
    let c = r.center();
    assert_eq!(c.x, 50.0);
    assert_eq!(c.y, 100.0);
}

#[test]
fn rect_edges() {
    let r = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
    assert_eq!(r.right(), 110.0);
    assert_eq!(r.bottom(), 70.0);

    let pr = PhysicalRect::new(10, 20, 100, 50);
    assert_eq!(pr.right(), 110);
    assert_eq!(pr.bottom(), 70);
}

#[test]
fn physical_rect_contains_point() {
    let r = PhysicalRect::new(0, 0, 100, 100);
    assert!(r.contains_point(PhysicalPoint::new(50, 50)));
    assert!(!r.contains_point(PhysicalPoint::new(100, 50))); // right edge exclusive
    assert!(!r.contains_point(PhysicalPoint::new(-1, 50)));
}

// ── MonitorDpi tests ─────────────────────────────────────────────────

#[test]
fn monitor_dpi_empty() {
    let m = MonitorDpi::new();
    assert!(m.is_empty());
    assert_eq!(m.count(), 0);
    assert_eq!(m.primary().factor(), 1.0);
    assert!(m.primary_id().is_none());
}

#[test]
fn monitor_dpi_with_primary() {
    let m = MonitorDpi::with_primary(0, DpiScale::new(2.0));
    assert_eq!(m.count(), 1);
    assert_eq!(m.primary().factor(), 2.0);
    assert_eq!(m.primary_id(), Some(0));
}

#[test]
fn monitor_dpi_set_and_get() {
    let mut m = MonitorDpi::new();
    m.set(1, DpiScale::new(1.5));
    m.set(2, DpiScale::new(2.0));
    assert_eq!(m.count(), 2);
    assert_eq!(m.for_monitor(1).unwrap().factor(), 1.5);
    assert_eq!(m.for_monitor(2).unwrap().factor(), 2.0);
    assert!(m.for_monitor(99).is_none());
}

#[test]
fn monitor_dpi_first_set_becomes_primary() {
    let mut m = MonitorDpi::new();
    m.set(5, DpiScale::new(1.25));
    assert_eq!(m.primary_id(), Some(5));
    assert_eq!(m.primary().factor(), 1.25);
}

#[test]
fn monitor_dpi_set_primary() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    m.set(1, DpiScale::new(2.0));
    assert!(m.set_primary(1));
    assert_eq!(m.primary().factor(), 2.0);
    // Setting primary to unknown monitor fails.
    assert!(!m.set_primary(99));
}

#[test]
fn monitor_dpi_remove() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    m.set(1, DpiScale::new(2.0));
    assert!(m.set_primary(0));
    let removed = m.remove(0);
    assert_eq!(removed.unwrap().factor(), 1.0);
    // Primary should have moved to the remaining monitor.
    assert_eq!(m.primary().factor(), 2.0);
    assert_eq!(m.count(), 1);
}

#[test]
fn monitor_dpi_for_monitor_or_primary() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    m.set(1, DpiScale::new(2.0));
    assert_eq!(m.for_monitor_or_primary(1).factor(), 2.0);
    assert_eq!(m.for_monitor_or_primary(99).factor(), 1.0); // falls back to primary
}

#[test]
fn monitor_dpi_max_min_scale() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    m.set(1, DpiScale::new(2.0));
    m.set(2, DpiScale::new(1.5));
    assert_eq!(m.max_scale().factor(), 2.0);
    assert_eq!(m.min_scale().factor(), 1.0);
}

#[test]
fn monitor_dpi_is_uniform() {
    let mut m = MonitorDpi::new();
    assert!(m.is_uniform()); // empty = uniform

    m.set(0, DpiScale::new(2.0));
    assert!(m.is_uniform()); // single = uniform

    m.set(1, DpiScale::new(2.0));
    assert!(m.is_uniform()); // same scale = uniform

    m.set(2, DpiScale::new(1.0));
    assert!(!m.is_uniform()); // different = not uniform
}

#[test]
fn monitor_dpi_update_existing() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    let prev = m.set(0, DpiScale::new(2.0));
    assert_eq!(prev.unwrap().factor(), 1.0);
    assert_eq!(m.for_monitor(0).unwrap().factor(), 2.0);
    assert_eq!(m.count(), 1);
}

#[test]
fn monitor_dpi_iter() {
    let mut m = MonitorDpi::new();
    m.set(0, DpiScale::new(1.0));
    m.set(1, DpiScale::new(2.0));
    let items: Vec<_> = m.iter().collect();
    assert_eq!(items.len(), 2);
    // Both monitors should appear (order not guaranteed with HashMap).
    assert!(items.iter().any(|(id, s)| *id == 0 && s.factor() == 1.0));
    assert!(items.iter().any(|(id, s)| *id == 1 && s.factor() == 2.0));
}

// ── Platform DPI tests ───────────────────────────────────────────────

#[test]
fn platform_dpi_creation() {
    let _p = PlatformDpi::new();
    // Should not panic.
}

#[test]
fn platform_system_dpi_returns_valid_scale() {
    let p = PlatformDpi::new();
    let scale = p.system_dpi();
    // Must be within the valid range.
    assert!(scale.factor() >= DpiScale::MIN);
    assert!(scale.factor() <= DpiScale::MAX);
}

#[test]
fn platform_primary_monitor_dpi_returns_valid_scale() {
    let p = PlatformDpi::new();
    let scale = p.primary_monitor_dpi();
    assert!(scale.factor() >= DpiScale::MIN);
    assert!(scale.factor() <= DpiScale::MAX);
}

#[test]
fn platform_enumerate_returns_at_least_one() {
    let p = PlatformDpi::new();
    let monitors = p.enumerate_monitor_dpis();
    assert!(!monitors.is_empty());
    for (_, scale) in &monitors {
        assert!(scale.factor() >= DpiScale::MIN);
        assert!(scale.factor() <= DpiScale::MAX);
    }
}

// ── DpiAware trait tests ─────────────────────────────────────────────

#[test]
fn dpi_aware_trait_is_implementable() {
    struct TestWidget {
        scale: DpiScale,
        invalidated: bool,
    }

    impl DpiAware for TestWidget {
        fn on_dpi_changed(&mut self, _old_scale: DpiScale, new_scale: DpiScale) {
            self.scale = new_scale;
            self.invalidated = true;
        }
    }

    let mut w = TestWidget {
        scale: DpiScale::identity(),
        invalidated: false,
    };
    w.on_dpi_changed(DpiScale::identity(), DpiScale::new(2.0));
    assert_eq!(w.scale.factor(), 2.0);
    assert!(w.invalidated);
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn fractional_scale_size_rounding() {
    // At 1.25x, a 100-logical-pixel width should be 125 physical pixels.
    let s = DpiScale::new(1.25);
    let logical = LogicalSize::new(100.0, 100.0);
    let physical = logical.to_physical(s);
    assert_eq!(physical.width, 125);
    assert_eq!(physical.height, 125);
}

#[test]
fn rect_physical_conversion_no_gaps() {
    // Two adjacent logical rects at 1.5x should tile without gaps or overlaps.
    let s = DpiScale::new(1.5);
    let left = LogicalRect::new(0.0, 0.0, 10.0, 10.0);
    let right = LogicalRect::new(10.0, 0.0, 10.0, 10.0);
    let pl = left.to_physical(s);
    let pr = right.to_physical(s);
    // Left rect's right edge should equal right rect's left edge.
    assert_eq!(pl.right(), pr.x);
}

#[test]
fn zero_size_rect() {
    let r = LogicalRect::new(10.0, 20.0, 0.0, 0.0);
    let s = DpiScale::new(2.0);
    let pr = r.to_physical(s);
    assert_eq!(pr.width, 0);
    assert_eq!(pr.height, 0);
}

// ══════════════════════════════════════════════════════════════════════
// FractionalScale tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn fractional_scale_new_clamping() {
    let low = FractionalScale::new(0.5);
    assert_eq!(low.factor(), FractionalScale::MIN);

    let high = FractionalScale::new(10.0);
    assert_eq!(high.factor(), FractionalScale::MAX);

    let normal = FractionalScale::new(1.5);
    assert_eq!(normal.factor(), 1.5);
}

#[test]
fn fractional_scale_default_is_1x() {
    assert_eq!(FractionalScale::default().factor(), 1.0);
}

#[test]
fn fractional_scale_is_integer() {
    assert!(FractionalScale::new(1.0).is_integer());
    assert!(FractionalScale::new(2.0).is_integer());
    assert!(FractionalScale::new(3.0).is_integer());
    assert!(!FractionalScale::new(1.25).is_integer());
    assert!(!FractionalScale::new(1.5).is_integer());
}

#[test]
fn fractional_scale_is_fractional() {
    assert!(FractionalScale::new(1.25).is_fractional());
    assert!(FractionalScale::new(1.75).is_fractional());
    assert!(!FractionalScale::new(2.0).is_fractional());
}

#[test]
fn fractional_scale_to_dpi_scale() {
    let fs = FractionalScale::new(1.5);
    let ds = fs.to_dpi_scale();
    assert_eq!(ds.factor(), 1.5);
}

#[test]
fn fractional_scale_as_f32() {
    let fs = FractionalScale::new(2.5);
    assert_eq!(fs.as_f32(), 2.5f32);
}

#[test]
fn fractional_scale_display() {
    let fs = FractionalScale::new(1.75);
    let text = format!("{fs}");
    assert!(text.contains("1.75"));
}

#[test]
fn fractional_presets_values() {
    assert_eq!(SCALE_1X.factor(), 1.0);
    assert_eq!(SCALE_1_25X.factor(), 1.25);
    assert_eq!(SCALE_1_5X.factor(), 1.5);
    assert_eq!(SCALE_1_75X.factor(), 1.75);
    assert_eq!(SCALE_2X.factor(), 2.0);
    assert_eq!(SCALE_2_5X.factor(), 2.5);
    assert_eq!(SCALE_3X.factor(), 3.0);
}

#[test]
fn fractional_presets_array_ordered() {
    for i in 1..PRESETS.len() {
        assert!(PRESETS[i].factor() > PRESETS[i - 1].factor());
    }
}

#[test]
fn snap_to_nearest_exact_steps() {
    assert_eq!(snap_to_nearest(1.0).factor(), 1.0);
    assert_eq!(snap_to_nearest(1.25).factor(), 1.25);
    assert_eq!(snap_to_nearest(1.5).factor(), 1.5);
    assert_eq!(snap_to_nearest(2.0).factor(), 2.0);
    assert_eq!(snap_to_nearest(4.0).factor(), 4.0);
}

#[test]
fn snap_to_nearest_between_steps() {
    assert_eq!(snap_to_nearest(1.13).factor(), 1.25);
    assert_eq!(snap_to_nearest(1.37).factor(), 1.25);
    assert_eq!(snap_to_nearest(1.38).factor(), 1.5);
    assert_eq!(snap_to_nearest(1.87).factor(), 1.75);
}

#[test]
fn snap_to_nearest_clamps_low() {
    let s = snap_to_nearest(0.3);
    assert_eq!(s.factor(), 1.0);
}

#[test]
fn snap_to_nearest_clamps_high() {
    let s = snap_to_nearest(10.0);
    assert_eq!(s.factor(), 4.0);
}

#[test]
fn buffer_scale_for_integer() {
    assert_eq!(buffer_scale_for(SCALE_1X), 1);
    assert_eq!(buffer_scale_for(SCALE_2X), 2);
    assert_eq!(buffer_scale_for(SCALE_3X), 3);
}

#[test]
fn buffer_scale_for_fractional() {
    assert_eq!(buffer_scale_for(SCALE_1_25X), 2);
    assert_eq!(buffer_scale_for(SCALE_1_5X), 2);
    assert_eq!(buffer_scale_for(SCALE_1_75X), 2);
    assert_eq!(buffer_scale_for(SCALE_2_5X), 3);
}

#[test]
fn viewport_transform_roundtrip() {
    let vp = viewport_transform(SCALE_2X, PhysicalSize::new(3840, 2160));
    assert_eq!(vp.buffer_width, 3840);
    assert_eq!(vp.buffer_height, 2160);
    assert!((vp.logical_width - 1920.0).abs() < 0.01);
    assert!((vp.logical_height - 1080.0).abs() < 0.01);

    // Roundtrip: logical -> buffer -> logical
    let (bx, by) = vp.logical_to_buffer(100.0, 200.0);
    assert_eq!(bx, 200);
    assert_eq!(by, 400);
    let (lx, ly) = vp.buffer_to_logical(bx, by);
    assert!((lx - 100.0).abs() < 0.01);
    assert!((ly - 200.0).abs() < 0.01);
}

#[test]
fn viewport_transform_clamping() {
    let vp = viewport_transform(SCALE_1X, PhysicalSize::new(1920, 1080));
    // Negative logical coords clamp to 0.
    let (bx, by) = vp.logical_to_buffer(-10.0, -20.0);
    assert_eq!(bx, 0);
    assert_eq!(by, 0);
    // Overflow clamps to buffer edge.
    let (bx, by) = vp.logical_to_buffer(5000.0, 5000.0);
    assert_eq!(bx, 1919);
    assert_eq!(by, 1079);
}

#[test]
fn viewport_transform_size_accessors() {
    let vp = viewport_transform(SCALE_1_5X, PhysicalSize::new(2880, 1620));
    let bs = vp.buffer_size();
    assert_eq!(bs.width, 2880);
    assert_eq!(bs.height, 1620);
    let ls = vp.logical_size();
    assert!((ls.width - 1920.0).abs() < 0.1);
    assert!((ls.height - 1080.0).abs() < 0.1);
}

#[test]
fn effective_resolution_1x() {
    let res = effective_resolution(PhysicalSize::new(1920, 1080), SCALE_1X);
    assert_eq!(res.width, 1920.0);
    assert_eq!(res.height, 1080.0);
}

#[test]
fn effective_resolution_2x() {
    let res = effective_resolution(PhysicalSize::new(3840, 2160), SCALE_2X);
    assert_eq!(res.width, 1920.0);
    assert_eq!(res.height, 1080.0);
}

#[test]
fn effective_resolution_1_5x() {
    let res = effective_resolution(PhysicalSize::new(2880, 1620), SCALE_1_5X);
    assert!((res.width - 1920.0).abs() < 0.1);
    assert!((res.height - 1080.0).abs() < 0.1);
}

// ══════════════════════════════════════════════════════════════════════
// PerMonitor / ScaleManager tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn monitor_scale_creation() {
    let ms = MonitorScale::new(1, 1.5, 144.0);
    assert_eq!(ms.monitor_id, 1);
    assert_eq!(ms.scale_factor, 1.5);
    assert_eq!(ms.physical_dpi, 144.0);
    assert!(ms.is_hidpi());
}

#[test]
fn monitor_scale_clamping() {
    let ms = MonitorScale::new(0, 0.1, 0.0);
    assert_eq!(ms.scale_factor, 0.5);
    assert_eq!(ms.physical_dpi, 1.0);
}

#[test]
fn monitor_scale_display() {
    let ms = MonitorScale::new(2, 2.0, 192.0);
    let text = format!("{ms}");
    assert!(text.contains("Monitor 2"));
    assert!(text.contains("2.00x"));
}

#[test]
fn scale_manager_empty() {
    let sm = ScaleManager::new();
    assert_eq!(sm.monitor_count(), 0);
    assert_eq!(sm.global_scale(), 1.0);
}

#[test]
fn scale_manager_add_and_query() {
    let mut sm = ScaleManager::new();
    let ms = MonitorScale::new(1, 2.0, 192.0);
    sm.add_monitor(ms, LogicalRect::new(0.0, 0.0, 1920.0, 1080.0));
    assert_eq!(sm.monitor_count(), 1);
    assert_eq!(sm.scale_for_monitor(1), 2.0);
}

#[test]
fn scale_manager_fallback_to_global() {
    let sm = ScaleManager::new();
    assert_eq!(sm.scale_for_monitor(99), 1.0);
}

#[test]
fn scale_manager_scale_for_window_single_monitor() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.5, 144.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    let window = LogicalRect::new(100.0, 100.0, 800.0, 600.0);
    assert_eq!(sm.scale_for_window(window), 1.5);
}

#[test]
fn scale_manager_scale_for_window_multi_monitor() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.add_monitor(
        MonitorScale::new(2, 2.0, 192.0),
        LogicalRect::new(1920.0, 0.0, 2560.0, 1440.0),
    );

    // Window fully on monitor 1.
    let w1 = LogicalRect::new(100.0, 100.0, 800.0, 600.0);
    assert_eq!(sm.scale_for_window(w1), 1.0);

    // Window fully on monitor 2.
    let w2 = LogicalRect::new(2000.0, 100.0, 800.0, 600.0);
    assert_eq!(sm.scale_for_window(w2), 2.0);
}

#[test]
fn scale_manager_scale_for_window_spanning_monitors() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.add_monitor(
        MonitorScale::new(2, 2.0, 192.0),
        LogicalRect::new(1920.0, 0.0, 1920.0, 1080.0),
    );

    // Window mostly on monitor 1 (1600 px on m1, 200 px on m2).
    let w = LogicalRect::new(1720.0, 0.0, 400.0, 1080.0);
    assert_eq!(sm.scale_for_window(w), 1.0);

    // Window mostly on monitor 2 (100 px on m1, 700 px on m2).
    let w2 = LogicalRect::new(1820.0, 0.0, 800.0, 1080.0);
    assert_eq!(sm.scale_for_window(w2), 2.0);
}

#[test]
fn scale_manager_window_no_overlap() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.5, 144.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    // Window completely off-screen.
    let w = LogicalRect::new(5000.0, 5000.0, 100.0, 100.0);
    assert_eq!(sm.scale_for_window(w), 1.0); // global fallback
}

#[test]
fn scale_manager_owning_monitor() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.add_monitor(
        MonitorScale::new(2, 2.0, 192.0),
        LogicalRect::new(1920.0, 0.0, 1920.0, 1080.0),
    );
    let w = LogicalRect::new(100.0, 100.0, 800.0, 600.0);
    assert_eq!(sm.owning_monitor(w), Some(1));

    let w_off = LogicalRect::new(5000.0, 5000.0, 10.0, 10.0);
    assert_eq!(sm.owning_monitor(w_off), None);
}

#[test]
fn scale_manager_on_monitor_change() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.on_monitor_change(1, 2.0);
    assert_eq!(sm.scale_for_monitor(1), 2.0);

    let events = sm.drain_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ScaleEvent::MonitorScaleChanged { monitor_id, old_scale, new_scale } => {
            assert_eq!(*monitor_id, 1);
            assert_eq!(*old_scale, 1.0);
            assert_eq!(*new_scale, 2.0);
        }
        _ => panic!("Expected MonitorScaleChanged"),
    }
}

#[test]
fn scale_manager_on_monitor_change_no_op() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.5, 144.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.on_monitor_change(1, 1.5); // same value
    assert!(sm.drain_events().is_empty());
}

#[test]
fn scale_manager_set_global_scale() {
    let mut sm = ScaleManager::new();
    sm.set_global_scale(1.5);
    assert_eq!(sm.global_scale(), 1.5);
    let events = sm.drain_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ScaleEvent::GlobalScaleChanged { old_scale, new_scale } => {
            assert_eq!(*old_scale, 1.0);
            assert_eq!(*new_scale, 1.5);
        }
        _ => panic!("Expected GlobalScaleChanged"),
    }
}

#[test]
fn scale_manager_remove_monitor() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.5, 144.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    let removed = sm.remove_monitor(1);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().scale_factor, 1.5);
    assert_eq!(sm.monitor_count(), 0);
}

#[test]
fn scale_manager_monitor_info() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(3, 2.0, 192.0),
        LogicalRect::new(0.0, 0.0, 2560.0, 1440.0),
    );
    let info = sm.monitor_info(3).unwrap();
    assert_eq!(info.physical_dpi, 192.0);
    assert!(sm.monitor_info(99).is_none());
}

#[test]
fn scale_manager_monitors_iterator() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    sm.add_monitor(
        MonitorScale::new(2, 2.0, 192.0),
        LogicalRect::new(1920.0, 0.0, 2560.0, 1440.0),
    );
    let ids: Vec<u32> = sm.monitors().map(|m| m.monitor_id).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

#[test]
fn scale_manager_set_monitor_bounds() {
    let mut sm = ScaleManager::new();
    sm.add_monitor(
        MonitorScale::new(1, 1.0, 96.0),
        LogicalRect::new(0.0, 0.0, 1920.0, 1080.0),
    );
    // Move monitor to a different position.
    sm.set_monitor_bounds(1, LogicalRect::new(1000.0, 0.0, 1920.0, 1080.0));
    // Window at origin should no longer overlap monitor 1.
    let w = LogicalRect::new(0.0, 0.0, 100.0, 100.0);
    assert_eq!(sm.scale_for_window(w), 1.0); // global fallback
}

// ══════════════════════════════════════════════════════════════════════
// TextScaling tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn text_scale_factor_default() {
    let ts = TextScaleFactor::default();
    assert_eq!(ts.factor(), 1.0);
    assert!(!ts.is_active());
}

#[test]
fn text_scale_factor_clamping() {
    let low = TextScaleFactor::new(0.1);
    assert_eq!(low.factor(), TextScaleFactor::MIN);

    let high = TextScaleFactor::new(10.0);
    assert_eq!(high.factor(), TextScaleFactor::MAX);
}

#[test]
fn text_scale_factor_is_active() {
    assert!(!TextScaleFactor::new(1.0).is_active());
    assert!(TextScaleFactor::new(1.5).is_active());
    assert!(TextScaleFactor::new(0.8).is_active());
}

#[test]
fn text_scale_factor_step_up() {
    let ts = TextScaleFactor::new(1.0);
    let stepped = ts.step_up(0.1);
    assert!((stepped.factor() - 1.1).abs() < 1e-9);
}

#[test]
fn text_scale_factor_step_down() {
    let ts = TextScaleFactor::new(1.0);
    let stepped = ts.step_down(0.1);
    assert!((stepped.factor() - 0.9).abs() < 1e-9);
}

#[test]
fn text_scale_factor_step_clamping() {
    let ts = TextScaleFactor::new(TextScaleFactor::MAX);
    let stepped = ts.step_up(0.5);
    assert_eq!(stepped.factor(), TextScaleFactor::MAX);

    let ts_low = TextScaleFactor::new(TextScaleFactor::MIN);
    let stepped_low = ts_low.step_down(0.5);
    assert_eq!(stepped_low.factor(), TextScaleFactor::MIN);
}

#[test]
fn text_scale_factor_display() {
    let ts = TextScaleFactor::new(1.5);
    let text = format!("{ts}");
    assert!(text.contains("150%"));
}

#[test]
fn text_scale_range_validity() {
    assert!(TextScaleRange::is_valid(1.0));
    assert!(TextScaleRange::is_valid(0.5));
    assert!(TextScaleRange::is_valid(3.0));
    assert!(!TextScaleRange::is_valid(0.3));
    assert!(!TextScaleRange::is_valid(3.5));
}

#[test]
fn text_scale_range_presets() {
    assert_eq!(TextScaleRange::PRESETS.len(), 9);
    assert_eq!(TextScaleRange::PRESETS[0], 0.5);
    assert_eq!(TextScaleRange::PRESETS[8], 3.0);
}

#[test]
fn effective_font_size_no_scaling() {
    let ts = TextScaleFactor::new(1.0);
    assert_eq!(effective_font_size(14.0, 1.0, &ts), 14.0);
}

#[test]
fn effective_font_size_ui_scale_only() {
    let ts = TextScaleFactor::new(1.0);
    assert_eq!(effective_font_size(14.0, 2.0, &ts), 28.0);
}

#[test]
fn effective_font_size_text_scale_only() {
    let ts = TextScaleFactor::new(1.5);
    assert_eq!(effective_font_size(14.0, 1.0, &ts), 21.0);
}

#[test]
fn effective_font_size_both_scales() {
    let ts = TextScaleFactor::new(1.5);
    assert_eq!(effective_font_size(14.0, 2.0, &ts), 42.0);
}

#[test]
fn hinting_mode_selection() {
    assert_eq!(hinting_mode(0.8), HintingMode::Full);
    assert_eq!(hinting_mode(1.0), HintingMode::Medium);
    assert_eq!(hinting_mode(1.25), HintingMode::Medium);
    assert_eq!(hinting_mode(1.5), HintingMode::Slight);
    assert_eq!(hinting_mode(1.75), HintingMode::Slight);
    assert_eq!(hinting_mode(2.0), HintingMode::None);
    assert_eq!(hinting_mode(3.0), HintingMode::None);
}

#[test]
fn hinting_mode_display() {
    assert_eq!(format!("{}", HintingMode::None), "none");
    assert_eq!(format!("{}", HintingMode::Slight), "slight");
    assert_eq!(format!("{}", HintingMode::Medium), "medium");
    assert_eq!(format!("{}", HintingMode::Full), "full");
}

#[test]
fn subpixel_rendering_threshold() {
    assert!(subpixel_rendering(1.0));
    assert!(subpixel_rendering(1.5));
    assert!(subpixel_rendering(1.99));
    assert!(!subpixel_rendering(2.0));
    assert!(!subpixel_rendering(3.0));
}

// ══════════════════════════════════════════════════════════════════════
// CursorScale tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn cursor_size_for_scale_1x() {
    assert_eq!(cursor_size_for_scale(24, 1.0), 24);
}

#[test]
fn cursor_size_for_scale_1_5x() {
    // 24 * 1.5 = 36, which is a standard size.
    assert_eq!(cursor_size_for_scale(24, 1.5), 36);
}

#[test]
fn cursor_size_for_scale_2x() {
    // 24 * 2 = 48, which is a standard size.
    assert_eq!(cursor_size_for_scale(24, 2.0), 48);
}

#[test]
fn cursor_size_for_scale_3x() {
    // 24 * 3 = 72 -> nearest standard is 64.
    assert_eq!(cursor_size_for_scale(24, 3.0), 64);
}

#[test]
fn nearest_cursor_size_exact_matches() {
    for &s in &STANDARD_SIZES {
        assert_eq!(nearest_cursor_size(s), s);
    }
}

#[test]
fn nearest_cursor_size_between() {
    assert_eq!(nearest_cursor_size(25), 24);
    assert_eq!(nearest_cursor_size(30), 32);
    assert_eq!(nearest_cursor_size(40), 36);
    assert_eq!(nearest_cursor_size(50), 48);
    assert_eq!(nearest_cursor_size(80), 64);
    assert_eq!(nearest_cursor_size(100), 96);
}

#[test]
fn nearest_cursor_size_zero() {
    assert_eq!(nearest_cursor_size(0), 24);
}

#[test]
fn nearest_cursor_size_large() {
    // Very large value should snap to 96 (largest standard).
    assert_eq!(nearest_cursor_size(200), 96);
}

#[test]
fn cursor_scale_config_default() {
    let cfg = CursorScaleConfig::new();
    assert_eq!(cfg.base_size, 24);
    assert!(cfg.scale_with_ui);
    assert!(cfg.custom_size_override.is_none());
}

#[test]
fn cursor_scale_config_fixed() {
    let cfg = CursorScaleConfig::fixed(48);
    assert_eq!(cfg.custom_size_override, Some(48));
}

#[test]
fn cursor_scale_config_resolve_default() {
    let cfg = CursorScaleConfig::new();
    assert_eq!(cfg.resolve(1.0), 24);
    assert_eq!(cfg.resolve(2.0), 48);
}

#[test]
fn cursor_scale_config_resolve_fixed() {
    let cfg = CursorScaleConfig::fixed(64);
    assert_eq!(cfg.resolve(1.0), 64);
    assert_eq!(cfg.resolve(2.0), 64); // override ignores scale
}

#[test]
fn cursor_scale_config_resolve_no_scale() {
    let cfg = CursorScaleConfig {
        base_size: 32,
        scale_with_ui: false,
        custom_size_override: None,
    };
    assert_eq!(cfg.resolve(2.0), 32); // stays at base since scale_with_ui=false
}

#[test]
fn cursor_scale_config_display() {
    let auto = CursorScaleConfig::new();
    assert!(format!("{auto}").contains("auto-scale"));

    let fixed = CursorScaleConfig::fixed(48);
    assert!(format!("{fixed}").contains("fixed 48px"));

    let no_scale = CursorScaleConfig {
        base_size: 32,
        scale_with_ui: false,
        custom_size_override: None,
    };
    assert!(format!("{no_scale}").contains("fixed"));
}

// ══════════════════════════════════════════════════════════════════════
// XSettings tests
// ══════════════════════════════════════════════════════════════════════

#[test]
fn xsettings_from_1x() {
    let xs = XSettings::from_ui_scale(1.0);
    assert_eq!(xs.xft_dpi, 96);
    assert_eq!(xs.gdk_scale, 1);
    assert!((xs.gdk_dpi_scale - 1.0).abs() < 1e-6);
    assert_eq!(xs.qt_scale_factor, 1.0);
}

#[test]
fn xsettings_from_2x() {
    let xs = XSettings::from_ui_scale(2.0);
    assert_eq!(xs.xft_dpi, 192);
    assert_eq!(xs.gdk_scale, 2);
    assert!((xs.gdk_dpi_scale - 1.0).abs() < 1e-6);
    assert_eq!(xs.qt_scale_factor, 2.0);
}

#[test]
fn xsettings_from_1_5x() {
    let xs = XSettings::from_ui_scale(1.5);
    assert_eq!(xs.xft_dpi, 144);
    assert_eq!(xs.gdk_scale, 2);
    assert!((xs.gdk_dpi_scale - 0.75).abs() < 1e-6);
    assert_eq!(xs.qt_scale_factor, 1.5);
}

#[test]
fn xsettings_from_1_25x() {
    let xs = XSettings::from_ui_scale(1.25);
    assert_eq!(xs.xft_dpi, 120);
    assert_eq!(xs.gdk_scale, 2);
    assert!((xs.gdk_dpi_scale - 0.625).abs() < 1e-6);
}

#[test]
fn xsettings_to_env_vars() {
    let xs = XSettings::from_ui_scale(1.5);
    let vars = xs.to_env_vars();
    assert_eq!(vars.len(), 4);
    assert!(vars.iter().any(|(k, v)| k == "QT_SCALE_FACTOR" && v == "1.5"));
    assert!(vars.iter().any(|(k, v)| k == "GDK_SCALE" && v == "2"));
    assert!(vars.iter().any(|(k, _)| k == "GDK_DPI_SCALE"));
    assert!(vars.iter().any(|(k, _)| k == "QT_FONT_DPI"));
}

#[test]
fn xsettings_xft_resource_string() {
    let xs = XSettings::from_ui_scale(1.5);
    assert_eq!(xs.xft_resource_string(), "Xft.dpi: 144");
}

#[test]
fn xsettings_effective_scale() {
    let xs = XSettings::from_ui_scale(1.75);
    assert_eq!(xs.effective_scale(), 1.75);
}

#[test]
fn xsettings_default() {
    let xs = XSettings::default();
    assert_eq!(xs.xft_dpi, 96);
    assert_eq!(xs.gdk_scale, 1);
    assert_eq!(xs.qt_scale_factor, 1.0);
}

#[test]
fn xsettings_display() {
    let xs = XSettings::from_ui_scale(2.0);
    let text = format!("{xs}");
    assert!(text.contains("Xft.dpi=192"));
    assert!(text.contains("GDK_SCALE=2"));
}

#[test]
fn xsettings_low_scale_clamped() {
    let xs = XSettings::from_ui_scale(0.1);
    assert_eq!(xs.xft_dpi, 48); // 0.5 * 96 = 48
    assert_eq!(xs.gdk_scale, 1);
}
