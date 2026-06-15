//! Live present-path verification diagnostic (t64-rdp facility #3).
//!
//! Run this **inside an RDP session** to verify the Win32 GDI present-path
//! flicker fix (off-screen DIB back-buffer + atomic BitBlt) WITHOUT subjective
//! eyeballing. It:
//!
//! 1. Opens a real visible window.
//! 2. Presents a sequence of distinct test patterns + a gradient "real frame"
//!    through the production-equivalent off-screen-DIB + atomic-BitBlt path.
//! 3. Reads back the ACTUAL presented window pixels after each present.
//! 4. Writes each presented frame to `target/present-verify/frame_NNN.png`.
//! 5. Writes a report `target/present-verify/report.txt` with: present path used
//!    (DXGI vs GDI/off-screen-DIB), remote_session bool, and a per-frame
//!    completeness check (presented == source, no torn / partial / missing rows).
//!
//! A PASS = every frame's `complete=true` and the report says
//! `ALL FRAMES COMPLETE`. If any frame is incomplete the report lists the torn
//! rows and the process exits non-zero.
//!
//! Usage (over RDP):
//!     cargo run -p liquide-platform --bin present-verify --offline
//! Optional args: `<width> <height> <frame_count>` (defaults 640 480 24).

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("present-verify is a Windows-only diagnostic (Win32 GDI present path).");
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
fn main() {
    use liquide_platform::win32::present_verify::{
        encode_png_bgra, evaluate_partial_present, live, make_frame_with_cursor, make_test_pattern,
        PixelRect, PresentPath, PresentVerifyMetrics,
    };
    use std::fmt::Write as _;
    use std::fs;
    use std::path::PathBuf;

    let mut args = std::env::args().skip(1);
    let width: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(640);
    let height: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(480);
    let frames: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(24);

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("present-verify");
    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("failed to create output dir {}: {e}", out_dir.display());
        std::process::exit(1);
    }

    let remote = live::is_remote_session();
    // Over RDP there is no hardware DXGI swap-chain, so the present path is the
    // GDI off-screen-DIB + BitBlt path this diagnostic exercises directly.
    let present_path = PresentPath::GdiOffscreenDib;

    println!("liquide present-verify");
    println!("  output dir     : {}", out_dir.display());
    println!("  remote_session : {remote}");
    println!("  present path    : {}", present_path.label());
    println!("  size x frames   : {width}x{height} x {frames}");

    let captured = match live::run_windowed_capture(width, height, frames) {
        Some(c) => c,
        None => {
            eprintln!("FAILED: could not open verification window / run capture.");
            std::process::exit(1);
        }
    };

    let mut metrics = PresentVerifyMetrics::default();
    let mut report = String::new();
    let _ = writeln!(report, "liquide present-verify report");
    let _ = writeln!(report, "remote_session = {remote}");
    let _ = writeln!(report, "present_path   = {}", present_path.label());
    let _ = writeln!(report, "frame_size     = {width}x{height}");
    let _ = writeln!(report, "frames         = {}", captured.len());
    let _ = writeln!(report, "----");

    // Determine the real client size from the first frame's comparison (rows).
    let mut all_complete = true;
    for frame in &captured {
        // Re-derive the source from the same generator the capture used, so the
        // PNG written is exactly what was meant to be presented. The capture
        // used the window's client size; comparison.rows is that height.
        let h = frame.comparison.rows;
        let row_len = if h > 0 {
            frame.readback.len() / h as usize
        } else {
            0
        };
        let w = (row_len / 4) as u32;
        let source = make_test_pattern(w, h, frame.index);

        let complete = frame.comparison.is_complete();
        all_complete &= complete;
        metrics.record(present_path, complete);

        // Write the PRESENTED (read-back) pixels to PNG.
        let png = encode_png_bgra(&frame.readback, w, h);
        let png_path = out_dir.join(format!("frame_{:03}.png", frame.index));
        if let Err(e) = fs::write(&png_path, &png) {
            eprintln!("failed to write {}: {e}", png_path.display());
        }

        let _ = writeln!(
            report,
            "frame {:03}: complete={} matching_rows={}/{} mismatched_bytes={} torn_rows={:?} (source_vs_readback {})",
            frame.index,
            complete,
            frame.comparison.matching_rows,
            frame.comparison.rows,
            frame.comparison.mismatched_bytes,
            frame.comparison.first_mismatched_rows,
            if frame.readback == source { "exact" } else { "DIFF" },
        );
    }

    let _ = writeln!(report, "----");
    let _ = writeln!(report, "metrics: {}", metrics.summary());

    // ── PARTIAL-DAMAGE (cursor-move) live check (t66-present) ─────────────────
    // The full-frame loop above proves whole-frame presents are atomic. This
    // section proves the cursor-only / hover partial-damage present is ALSO
    // smear-free over RDP: present cursor@P1, then cursor@P2 (P1 cleared) through
    // one reused back-buffer (as production does), read back the visible surface,
    // and assert the old cursor is GONE and the new cursor is PRESENT.
    let _ = writeln!(report, "---- partial-damage (cursor-move) check ----");
    let mut partial_ok = false;
    {
        let bg = [10u8, 20, 30, 0xFF];
        let cursor_color = [200u8, 210, 220, 0xFF];
        let none = PixelRect { x: 0, y: 0, w: 0, h: 0 };
        match live::VerifyWindow::create(width, height, "liquide present-verify (cursor)") {
            Some(window) => {
                let (cw, ch) = window.client_size();
                let (w, h) = if cw == 0 || ch == 0 { (width, height) } else { (cw, ch) };
                let cs = (w.min(h) / 12).max(6);
                let p1 = PixelRect { x: w / 8, y: h / 8, w: cs, h: cs };
                let p2 = PixelRect {
                    x: (w * 6 / 8).min(w.saturating_sub(cs)),
                    y: (h * 6 / 8).min(h.saturating_sub(cs)),
                    w: cs,
                    h: cs,
                };
                let frame_a = make_frame_with_cursor(w, h, bg, p1, cursor_color);
                let frame_b = make_frame_with_cursor(w, h, bg, p2, cursor_color);
                let background = make_frame_with_cursor(w, h, bg, none, cursor_color);
                window.pump_messages();
                match window.present_sequence_readback_last(&[&frame_a, &frame_b], w, h) {
                    Some(rt) => {
                        window.pump_messages();
                        let check =
                            evaluate_partial_present(&rt.readback, &frame_b, &background, w, h, p1, p2);
                        partial_ok = check.is_clean();
                        let _ = writeln!(
                            report,
                            "cursor move P1({},{})->P2({},{}) cursor={}px : complete={} old_region_residue={} new_region_occupancy={} clean={}",
                            p1.x, p1.y, p2.x, p2.y, cs,
                            check.comparison.is_complete(),
                            check.old_region_residue,
                            check.new_region_occupancy,
                            partial_ok,
                        );
                        // Dump the read-back so smear (if any) is visible.
                        let png = encode_png_bgra(&rt.readback, w, h);
                        let png_path = out_dir.join("cursor_move_readback.png");
                        let _ = fs::write(&png_path, &png);
                    }
                    None => {
                        let _ = writeln!(report, "cursor move: FAILED to present/read back");
                    }
                }
            }
            None => {
                let _ = writeln!(report, "cursor move: FAILED to open verification window");
            }
        }
    }

    let verdict = if all_complete && !captured.is_empty() && partial_ok {
        "ALL FRAMES COMPLETE + cursor-move partial-damage clean — present path is atomic (no tearing/partial/missing/smear)"
    } else if !partial_ok {
        "CURSOR-MOVE SMEAR/RESIDUE DETECTED — partial-damage present left the old cursor (see cursor-move line + cursor_move_readback.png)"
    } else {
        "INCOMPLETE FRAMES DETECTED — tearing / stale / missing rows (see torn_rows above)"
    };
    let _ = writeln!(report, "VERDICT: {verdict}");

    let report_path = out_dir.join("report.txt");
    if let Err(e) = fs::write(&report_path, &report) {
        eprintln!("failed to write report {}: {e}", report_path.display());
    }

    println!("{report}");
    println!("report  : {}", report_path.display());
    println!("frames  : {}", out_dir.display());

    if all_complete && !captured.is_empty() && partial_ok {
        println!("PASS");
        std::process::exit(0);
    } else {
        eprintln!("FAIL");
        std::process::exit(1);
    }
}
