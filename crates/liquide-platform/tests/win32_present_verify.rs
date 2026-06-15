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
    live, make_test_pattern, PresentPath, PresentVerifyMetrics,
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

/// `is_remote_session()` must not panic; it simply reports whether this process
/// is in an RDP session (false on a local console). Documents the runtime probe
/// the live report uses.
#[test]
fn is_remote_session_is_queryable() {
    let _remote = live::is_remote_session();
}
