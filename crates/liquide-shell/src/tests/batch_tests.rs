use liquide_compositor::geometry::Rect;

use crate::shell::Shell;
use crate::shell::batch::{WindowBatch, WindowOp, ZOrderOp, compute_move_valid_rect};
use crate::window::WindowId;

// ---------------------------------------------------------------------------
// compute_move_valid_rect — single-window blit-move geometry (t164-blit-move)
// ---------------------------------------------------------------------------

/// Total area of a set of disjoint axis-aligned rects (used to assert the
/// strip/footprint decomposition exactly tiles a region with no overlap/gap).
fn area_sum(rects: &[Rect]) -> f32 {
    rects.iter().map(|r| r.width * r.height).sum()
}

#[test]
fn move_valid_rect_overlap_blit_and_strips_partition_exactly() {
    // A 100x80 window slides right+down by (30, 20). The overlap is the
    // blittable region; the new strips + old uncovered must EXACTLY account for
    // the rest of new / old with no double-count.
    let old = Rect::new(0.0, 0.0, 100.0, 80.0);
    let new = Rect::new(30.0, 20.0, 100.0, 80.0);
    let vr = compute_move_valid_rect(old, new);

    assert_eq!(vr.dx, 30.0);
    assert_eq!(vr.dy, 20.0);
    // Overlap, expressed in new coords: x in [30,100], y in [20,80] → 70x60.
    assert_eq!(vr.blit_rect, Rect::new(30.0, 20.0, 70.0, 60.0));

    // new = blit ∪ new_strips, disjoint → areas add up to new's area.
    let new_area = new.width * new.height;
    assert!(
        (vr.blit_rect.width * vr.blit_rect.height + area_sum(&vr.new_strips) - new_area).abs()
            < 0.01,
        "blit_rect + new_strips must exactly cover new (no gap, no overlap)"
    );
    // old_uncovered = old minus new; with new fully inside the lower-right, the
    // uncovered region is old's area minus the overlap area.
    let old_area = old.width * old.height;
    let overlap_area = vr.blit_rect.width * vr.blit_rect.height;
    assert!(
        (area_sum(&vr.old_uncovered) - (old_area - overlap_area)).abs() < 0.01,
        "old_uncovered must equal old minus the overlap"
    );
    // No strip / footprint rect may intersect the blit_rect interior (else the
    // re-raster would clobber blitted pixels). Check via intersection emptiness.
    for r in vr.new_strips.iter().chain(vr.old_uncovered.iter()) {
        if let Some(i) = r.intersection(&vr.blit_rect) {
            assert!(
                i.width < 0.01 || i.height < 0.01,
                "strip/footprint {r:?} must not overlap blit_rect {:?}",
                vr.blit_rect
            );
        }
    }
}

#[test]
fn move_valid_rect_disjoint_move_has_no_blit() {
    // Move so far the old and new bounds do not overlap → no blittable region;
    // all of new is a strip and all of old is uncovered (full fallback case).
    let old = Rect::new(0.0, 0.0, 50.0, 50.0);
    let new = Rect::new(200.0, 200.0, 50.0, 50.0);
    let vr = compute_move_valid_rect(old, new);
    assert_eq!(vr.blit_rect, Rect::ZERO);
    assert_eq!(vr.new_strips, vec![new]);
    assert_eq!(vr.old_uncovered, vec![old]);
}

#[test]
fn move_valid_rect_pure_horizontal_move() {
    // Pure rightward slide: the uncovered old footprint is exactly the left band
    // the window vacated; the new strip is exactly the right band it revealed.
    let old = Rect::new(0.0, 0.0, 100.0, 100.0);
    let new = Rect::new(40.0, 0.0, 100.0, 100.0);
    let vr = compute_move_valid_rect(old, new);
    assert_eq!(vr.blit_rect, Rect::new(40.0, 0.0, 60.0, 100.0));
    // Revealed new strip on the right: x in [100,140].
    assert_eq!(vr.new_strips, vec![Rect::new(100.0, 0.0, 40.0, 100.0)]);
    // Uncovered old band on the left: x in [0,40].
    assert_eq!(vr.old_uncovered, vec![Rect::new(0.0, 0.0, 40.0, 100.0)]);
}

// ---------------------------------------------------------------------------
// WindowBatch unit tests (no Shell required)
// ---------------------------------------------------------------------------

#[test]
fn batch_new_is_empty() {
    let batch = WindowBatch::new();
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
}

#[test]
fn batch_with_capacity_is_empty() {
    let batch = WindowBatch::with_capacity(16);
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
}

#[test]
fn batch_push_increments_len() {
    let mut batch = WindowBatch::new();
    let id = WindowId(1);
    batch.move_window(id, 10.0, 20.0);
    assert_eq!(batch.len(), 1);
    batch.resize_window(id, 100.0, 200.0);
    assert_eq!(batch.len(), 2);
    assert!(!batch.is_empty());
}

#[test]
fn batch_default_is_empty() {
    let batch = WindowBatch::default();
    assert!(batch.is_empty());
}

// ---------------------------------------------------------------------------
// Optimize: coalesce Move + Resize into MoveResize
// ---------------------------------------------------------------------------

#[test]
fn optimize_coalesces_move_and_resize_into_move_resize() {
    let mut batch = WindowBatch::new();
    let id = WindowId(1);
    batch.move_window(id, 10.0, 20.0);
    batch.resize_window(id, 300.0, 400.0);

    batch.optimize();
    assert_eq!(batch.len(), 1);
    match &batch.ops()[0] {
        WindowOp::MoveResize {
            id: oid,
            x,
            y,
            width,
            height,
        } => {
            assert_eq!(*oid, id);
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 20.0);
            assert_eq!(*width, 300.0);
            assert_eq!(*height, 400.0);
        }
        other => panic!("expected MoveResize, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Optimize: multiple Moves keep only the last
// ---------------------------------------------------------------------------

#[test]
fn optimize_keeps_last_move() {
    let mut batch = WindowBatch::new();
    let id = WindowId(42);
    batch.move_window(id, 0.0, 0.0);
    batch.move_window(id, 50.0, 50.0);
    batch.move_window(id, 100.0, 200.0);

    batch.optimize();
    assert_eq!(batch.len(), 1);
    match &batch.ops()[0] {
        WindowOp::Move { x, y, .. } => {
            assert_eq!(*x, 100.0);
            assert_eq!(*y, 200.0);
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Optimize: multiple Resizes keep only the last
// ---------------------------------------------------------------------------

#[test]
fn optimize_keeps_last_resize() {
    let mut batch = WindowBatch::new();
    let id = WindowId(7);
    batch.resize_window(id, 100.0, 100.0);
    batch.resize_window(id, 200.0, 200.0);
    batch.resize_window(id, 300.0, 400.0);

    batch.optimize();
    assert_eq!(batch.len(), 1);
    match &batch.ops()[0] {
        WindowOp::Resize { width, height, .. } => {
            assert_eq!(*width, 300.0);
            assert_eq!(*height, 400.0);
        }
        other => panic!("expected Resize, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Optimize: MoveResize supersedes earlier Move and Resize
// ---------------------------------------------------------------------------

#[test]
fn optimize_move_resize_supersedes_individual_ops() {
    let mut batch = WindowBatch::new();
    let id = WindowId(5);
    batch.move_window(id, 10.0, 20.0);
    batch.resize_window(id, 50.0, 60.0);
    batch.move_resize(id, 100.0, 200.0, 300.0, 400.0);

    batch.optimize();
    assert_eq!(batch.len(), 1);
    match &batch.ops()[0] {
        WindowOp::MoveResize {
            x,
            y,
            width,
            height,
            ..
        } => {
            assert_eq!(*x, 100.0);
            assert_eq!(*y, 200.0);
            assert_eq!(*width, 300.0);
            assert_eq!(*height, 400.0);
        }
        other => panic!("expected MoveResize, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Optimize: different window IDs are kept separate
// ---------------------------------------------------------------------------

#[test]
fn optimize_preserves_separate_windows() {
    let mut batch = WindowBatch::new();
    let id_a = WindowId(1);
    let id_b = WindowId(2);
    batch.move_window(id_a, 10.0, 10.0);
    batch.move_window(id_b, 20.0, 20.0);
    batch.resize_window(id_a, 100.0, 100.0);
    batch.resize_window(id_b, 200.0, 200.0);

    batch.optimize();
    // Both should be coalesced into MoveResize, one per window.
    assert_eq!(batch.len(), 2);

    // Ops are sorted by window id for determinism.
    match &batch.ops()[0] {
        WindowOp::MoveResize {
            id,
            x,
            y,
            width,
            height,
        } => {
            assert_eq!(*id, id_a);
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 10.0);
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 100.0);
        }
        other => panic!("expected MoveResize for id_a, got {:?}", other),
    }
    match &batch.ops()[1] {
        WindowOp::MoveResize {
            id,
            x,
            y,
            width,
            height,
        } => {
            assert_eq!(*id, id_b);
            assert_eq!(*x, 20.0);
            assert_eq!(*y, 20.0);
            assert_eq!(*width, 200.0);
            assert_eq!(*height, 200.0);
        }
        other => panic!("expected MoveResize for id_b, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Optimize: non-geometric ops preserved
// ---------------------------------------------------------------------------

#[test]
fn optimize_preserves_non_geometric_ops() {
    let mut batch = WindowBatch::new();
    let id = WindowId(3);
    batch.move_window(id, 10.0, 20.0);
    batch.minimize(id);
    batch.resize_window(id, 100.0, 200.0);
    batch.close(WindowId(99));

    batch.optimize();
    // 1 coalesced MoveResize + 1 Minimize + 1 Close = 3
    assert_eq!(batch.len(), 3);
}

// ---------------------------------------------------------------------------
// Empty batch is no-op
// ---------------------------------------------------------------------------

#[test]
fn empty_batch_is_noop() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let batch = WindowBatch::new();
    shell.apply_batch(batch);
    assert_eq!(shell.window_count(), 0);
}

// ---------------------------------------------------------------------------
// apply_batch: Move
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_moves_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));

    let mut batch = WindowBatch::new();
    batch.move_window(id, 50.0, 75.0);
    shell.apply_batch(batch);

    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.x, 50.0);
    assert_eq!(w.bounds.y, 75.0);
    assert_eq!(w.bounds.width, 100.0);
    assert_eq!(w.bounds.height, 100.0);
}

// ---------------------------------------------------------------------------
// apply_batch: Resize
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_resizes_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(10.0, 20.0, 100.0, 100.0));

    let mut batch = WindowBatch::new();
    batch.resize_window(id, 300.0, 400.0);
    shell.apply_batch(batch);

    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.x, 10.0);
    assert_eq!(w.bounds.y, 20.0);
    assert_eq!(w.bounds.width, 300.0);
    assert_eq!(w.bounds.height, 400.0);
}

// ---------------------------------------------------------------------------
// apply_batch: MoveResize
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_move_resize() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(0.0, 0.0, 100.0, 100.0));

    let mut batch = WindowBatch::new();
    batch.move_resize(id, 50.0, 60.0, 300.0, 400.0);
    shell.apply_batch(batch);

    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.x, 50.0);
    assert_eq!(w.bounds.y, 60.0);
    assert_eq!(w.bounds.width, 300.0);
    assert_eq!(w.bounds.height, 400.0);
}

// ---------------------------------------------------------------------------
// apply_batch: multiple windows
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_multiple_windows() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id3 = shell.open_window("C", Rect::new(0.0, 0.0, 100.0, 100.0));

    let mut batch = WindowBatch::with_capacity(3);
    batch.move_resize(id1, 0.0, 0.0, 640.0, 540.0);
    batch.move_resize(id2, 640.0, 0.0, 640.0, 540.0);
    batch.move_resize(id3, 0.0, 540.0, 1280.0, 540.0);
    shell.apply_batch(batch);

    let w1 = shell.window(id1).unwrap();
    assert_eq!(w1.bounds.x, 0.0);
    assert_eq!(w1.bounds.width, 640.0);
    let w2 = shell.window(id2).unwrap();
    assert_eq!(w2.bounds.x, 640.0);
    let w3 = shell.window(id3).unwrap();
    assert_eq!(w3.bounds.y, 540.0);
    assert_eq!(w3.bounds.width, 1280.0);
}

// ---------------------------------------------------------------------------
// apply_batch: mixed ops (move + minimize + close)
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_mixed_ops() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id3 = shell.open_window("C", Rect::new(0.0, 0.0, 100.0, 100.0));

    let mut batch = WindowBatch::new();
    batch.move_window(id1, 500.0, 500.0);
    batch.minimize(id2);
    batch.close(id3);
    shell.apply_batch(batch);

    let w1 = shell.window(id1).unwrap();
    assert_eq!(w1.bounds.x, 500.0);
    let w2 = shell.window(id2).unwrap();
    assert!(!w2.visible);
    assert!(shell.window(id3).is_err()); // closed
    assert_eq!(shell.window_count(), 2);
}

// ---------------------------------------------------------------------------
// apply_batch: SetTitle
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_set_title() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Old", Rect::ZERO);

    let mut batch = WindowBatch::new();
    batch.set_title(id, "New Title");
    shell.apply_batch(batch);

    assert_eq!(shell.window(id).unwrap().title, "New Title");
}

// ---------------------------------------------------------------------------
// apply_batch: Show / Hide
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_show_hide() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    assert!(shell.window(id).unwrap().visible);

    let mut batch = WindowBatch::new();
    batch.hide(id);
    shell.apply_batch(batch);
    assert!(!shell.window(id).unwrap().visible);

    let mut batch2 = WindowBatch::new();
    batch2.show(id);
    shell.apply_batch(batch2);
    assert!(shell.window(id).unwrap().visible);
}

// ---------------------------------------------------------------------------
// apply_batch: Raise via ZOrderOp::Top
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_raise() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    let _ = shell.raise_window(id2);

    let mut batch = WindowBatch::new();
    batch.raise(id1);
    shell.apply_batch(batch);

    // After raising id1, its z_order should be higher than id2.
    let w1 = shell.window(id1).unwrap();
    let w2 = shell.window(id2).unwrap();
    assert!(w1.z_order > w2.z_order);
}

// ---------------------------------------------------------------------------
// apply_batch: ignores nonexistent window IDs gracefully
// ---------------------------------------------------------------------------

#[test]
fn apply_batch_nonexistent_window_noop() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(10.0, 20.0, 100.0, 100.0));
    let ghost = WindowId(9999);

    let mut batch = WindowBatch::new();
    batch.move_window(ghost, 500.0, 500.0);
    batch.move_window(id, 50.0, 60.0);
    shell.apply_batch(batch);

    // Real window moved, ghost was silently ignored.
    let w = shell.window(id).unwrap();
    assert_eq!(w.bounds.x, 50.0);
    assert_eq!(w.bounds.y, 60.0);
}

// ---------------------------------------------------------------------------
// tile_visible_windows smoke test
// ---------------------------------------------------------------------------

#[test]
fn tile_visible_windows_arranges_all() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));

    shell.tile_visible_windows();

    let w1 = shell.window(id1).unwrap();
    let w2 = shell.window(id2).unwrap();

    // After tiling, the two windows should not overlap (master-stack split).
    // The exact positions depend on tiling config, but they shouldn't be
    // at their original (0,0) positions both at 100x100.
    assert!(
        w1.bounds.width > 100.0 || w2.bounds.width > 100.0,
        "at least one window should be larger than 100px wide after tiling"
    );
    // They shouldn't be identical rects (unless stacking mode).
    let same_x = (w1.bounds.x - w2.bounds.x).abs() < 0.001;
    let same_w = (w1.bounds.width - w2.bounds.width).abs() < 0.001;
    assert!(
        !same_x || !same_w,
        "two tiled windows should not have identical x and width"
    );
}

// ---------------------------------------------------------------------------
// tile_visible_windows: minimized windows excluded
// ---------------------------------------------------------------------------

#[test]
fn tile_visible_windows_excludes_minimized() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id2 = shell.open_window("B", Rect::new(0.0, 0.0, 100.0, 100.0));
    let id3 = shell.open_window("C", Rect::new(0.0, 0.0, 100.0, 100.0));

    let _ = shell.minimize(id2);

    shell.tile_visible_windows();

    // Only id1 and id3 should be tiled.
    let w1 = shell.window(id1).unwrap();
    let w3 = shell.window(id3).unwrap();
    // With 2 windows, the tiling engine uses master-stack: master takes
    // ~55% width, stack takes the rest.
    assert!(w1.bounds.width > 100.0 || w3.bounds.width > 100.0);
}

// ---------------------------------------------------------------------------
// Convenience: push raw WindowOp
// ---------------------------------------------------------------------------

#[test]
fn batch_push_raw_op() {
    let mut batch = WindowBatch::new();
    batch.push(WindowOp::SetTitle {
        id: WindowId(1),
        title: "Hello".to_string(),
    });
    batch.push(WindowOp::SetZOrder {
        id: WindowId(2),
        position: ZOrderOp::Bottom,
    });
    assert_eq!(batch.len(), 2);
}
