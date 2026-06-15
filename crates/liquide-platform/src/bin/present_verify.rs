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
        encode_png_bgra, live, make_test_pattern, PresentPath, PresentVerifyMetrics,
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
    let verdict = if all_complete && !captured.is_empty() {
        "ALL FRAMES COMPLETE — present path is atomic (no tearing/partial/missing rows)"
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

    if all_complete && !captured.is_empty() {
        println!("PASS");
        std::process::exit(0);
    } else {
        eprintln!("FAIL");
        std::process::exit(1);
    }
}
