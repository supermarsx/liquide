//! Win32 present-path verification tests.
//!
//! These tests verify the GDI off-screen-DIB + atomic-BitBlt present mechanism
//! (the path taken over RDP, where there is no hardware DXGI). See t64-rdp.
//!
//! ## What runs WHERE
//!
//! - The pure read-back / comparison / metrics / PNG logic is unit-tested
//!   inline in `win32::present_verify` (`cargo test -p liquide-platform`).
//! - The tests below run the REAL GDI BitBlt round-trip against an off-screen
//!   MEMORY DC: present a frame through an off-screen DIB-section, BitBlt it onto
//!   a destination DC, then BitBlt the destination back into a read-back DIB and
//!   assert the read-back equals the source. This needs only GDI (no visible
//!   window, no message pump), so it runs on any Windows host INCLUDING headless
//!   CI runners. It proves the BitBlt copy is atomic and complete.
//! - The truly-windowed / RDP-compositor behaviour is exercised by the
//!   `present-verify` bin (see that bin and `.orchestration/logs/t64-rdp.md`),
//!   which the user runs live over RDP.

#![cfg(target_os = "windows")]

use liquide_platform::win32::present_verify::{
    evaluate_partial_present, live, make_frame_with_cursor, make_test_pattern, PixelRect,
    PresentPath, PresentVerifyMetrics,
};

/// Read-back assertion (t64-rdp facility #1): present a known frame through the
/// real off-screen-DIB + BitBlt mechanism and assert the presented pixels EQUAL
/// the source — no partial / torn / missing rows.
#[test]
fn live_gdi_roundtrip_readback_equals_source() {
    let (w, h) = (64u32, 48u32);
    let source = make_test_pattern(w, h, 1);

    let rt = live::present_roundtrip_offscreen(&source, w, h)
        .expect("GDI off-screen present round-trip should succeed on a Windows host");

    assert_eq!(rt.readback.len(), source.len());
    assert!(
        rt.comparison.is_complete(),
        "presented frame must equal source exactly: {:?}",
        rt.comparison
    );
    assert_eq!(rt.comparison.matching_rows, h);
    assert_eq!(rt.comparison.mismatched_bytes, 0);
    // Byte-for-byte: the read-back IS the source.
    assert_eq!(rt.readback, source, "read-back pixels differ from source");
}

/// Present-path self-test (t64-rdp facility #2): drive N DISTINCT frames through
/// the real present round-trip and assert each read-back is the complete
/// corresponding source. Catches tearing (rows from a prior frame) and
/// stale-buffer (whole prior frame) regressions, and confirms each present is
/// independent (no bleed between frames).
#[test]
fn live_gdi_self_test_n_distinct_frames_all_complete() {
    let (w, h) = (80u32, 60u32);
    const N: u32 = 16;
    let mut metrics = PresentVerifyMetrics::default();
    let mut previous: Option<Vec<u8>> = None;

    for n in 0..N {
        let source = make_test_pattern(w, h, n);

        // Distinctness guard: each frame differs from the last, so a stale
        // read-back would be caught by the completeness check below.
        if let Some(prev) = &previous {
            assert_ne!(prev, &source, "frame {n} not distinct from previous");
        }

        let rt = live::present_roundtrip_offscreen(&source, w, h)
            .unwrap_or_else(|| panic!("present round-trip failed for frame {n}"));

        let complete = rt.comparison.is_complete();
        metrics.record(PresentPath::GdiOffscreenDib, complete);

        assert!(
            complete,
            "frame {n} read back incomplete (torn/stale/missing): {:?}",
            rt.comparison
        );
        assert_eq!(rt.readback, source, "frame {n} read-back != source");

        previous = Some(source);
    }

    assert_eq!(metrics.frames_presented, u64::from(N));
    assert_eq!(metrics.frames_complete, u64::from(N));
    assert_eq!(metrics.frames_incomplete, 0);
    assert!(
        metrics.all_complete(),
        "metrics report an incomplete frame: {}",
        metrics.summary()
    );
}

/// PARTIAL-DAMAGE (cursor-move) present verification (t66-present): present a
/// frame with the cursor at P1, then a frame with the cursor moved to P2 (P1
/// cleared) through ONE reused off-screen back-buffer + atomic BitBlt — exactly
/// as the production present path keeps a single `GdiBackBuffer` per window and
/// re-fills + re-blits it on every present (full OR cursor-only). Read back the
/// ACTUAL visible surface and assert it equals the cursor-moved frame: old cursor
/// GONE at P1, new cursor PRESENT at P2, rest intact. Catches present-layer smear
/// / residue (a stale region BitBlt would leave the old cursor at P1).
#[test]
fn live_gdi_partial_damage_cursor_move_no_smear() {
    let (w, h) = (96u32, 72u32);
    let bg = [10u8, 20, 30, 0xFF];
    let cursor = [200u8, 210, 220, 0xFF];
    let none = PixelRect { x: 0, y: 0, w: 0, h: 0 };
    let p1 = PixelRect { x: 8, y: 8, w: 10, h: 10 };
    let p2 = PixelRect { x: 60, y: 50, w: 10, h: 10 };

    let frame_a = make_frame_with_cursor(w, h, bg, p1, cursor);
    let frame_b = make_frame_with_cursor(w, h, bg, p2, cursor);
    let background = make_frame_with_cursor(w, h, bg, none, cursor);

    // Drive A then B through one reused back-buffer; read back the final surface.
    let rt = live::present_sequence_offscreen(&[&frame_a, &frame_b], w, h)
        .expect("partial-damage present sequence should succeed on a Windows host");

    let check = evaluate_partial_present(&rt.readback, &frame_b, &background, w, h, p1, p2);
    assert!(
        check.comparison.is_complete(),
        "visible surface must equal the cursor-moved frame: {:?}",
        check.comparison
    );
    assert_eq!(
        check.old_region_residue, 0,
        "old cursor region must be clean (no smear/residue at present layer)"
    );
    assert!(
        check.new_region_occupancy > 0,
        "new cursor must be present at P2"
    );
    assert!(check.is_clean(), "partial-damage present must be clean");
    // Byte-exact: the visible surface IS frame B.
    assert_eq!(rt.readback, frame_b, "read-back != expected cursor-moved frame");
}

/// Multi-step cursor path (P1 -> P2 -> P3) through the reused back-buffer: after
/// the last present only P3 is occupied; P1 and P2 are residue-free. Models the
/// accumulation hazard where successive partial presents leave a trail.
#[test]
fn live_gdi_partial_damage_multi_step_no_trail() {
    let (w, h) = (96u32, 72u32);
    let bg = [5u8, 5, 5, 0xFF];
    let cursor = [240u8, 240, 240, 0xFF];
    let none = PixelRect { x: 0, y: 0, w: 0, h: 0 };
    let p1 = PixelRect { x: 6, y: 6, w: 8, h: 8 };
    let p2 = PixelRect { x: 40, y: 30, w: 8, h: 8 };
    let p3 = PixelRect { x: 80, y: 60, w: 8, h: 8 };

    let fa = make_frame_with_cursor(w, h, bg, p1, cursor);
    let fb = make_frame_with_cursor(w, h, bg, p2, cursor);
    let fc = make_frame_with_cursor(w, h, bg, p3, cursor);
    let background = make_frame_with_cursor(w, h, bg, none, cursor);

    let rt = live::present_sequence_offscreen(&[&fa, &fb, &fc], w, h)
        .expect("multi-step present sequence should succeed");

    // Each prior position must be residue-free; only P3 occupied.
    let r1 = liquide_platform::win32::present_verify::changed_pixels_in_region(
        &rt.readback,
        &background,
        w,
        h,
        p1,
    );
    let r2 = liquide_platform::win32::present_verify::changed_pixels_in_region(
        &rt.readback,
        &background,
        w,
        h,
        p2,
    );
    let occ3 = liquide_platform::win32::present_verify::changed_pixels_in_region(
        &rt.readback,
        &background,
        w,
        h,
        p3,
    );
    assert_eq!(r1, 0, "P1 must be clean after cursor moved away");
    assert_eq!(r2, 0, "P2 must be clean after cursor moved away");
    assert!(occ3 > 0, "cursor must be present at final position P3");
    assert_eq!(rt.readback, fc, "final visible surface must equal last frame");
}

/// `is_remote_session()` must not panic; it simply reports whether this process
/// is in an RDP session (false on a local console). Documents the runtime probe
/// the live report uses.
#[test]
fn is_remote_session_is_queryable() {
    let _remote = live::is_remote_session();
}
