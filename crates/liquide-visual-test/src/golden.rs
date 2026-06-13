//! Golden-image load / compare / bless.
//!
//! Goldens live under `crates/liquide-visual-test/golden/<name>.png` (committed
//! — they ARE the spec). On mismatch, `expected.png`, `actual.png`, and
//! `diff.png` are written under `target/visual-test/<name>/` and the panic
//! message prints their absolute paths so a developer can eyeball them
//! immediately ("debug + cycle faster").
//!
//! Blessing: set `LIQUIDE_UPDATE_GOLDEN=1` (or `BLESS=1`) to (re)write the
//! golden from the current render instead of asserting. A missing golden is
//! treated as a blessing request when blessing is enabled, otherwise a failure.

use crate::capture::{Frame, VisualTestError};
use crate::diff::{DiffOptions, diff_frames};

/// Directory holding committed golden PNGs.
#[must_use]
pub fn golden_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden")
}

/// Output directory for expected/actual/diff artifacts on mismatch.
#[must_use]
pub fn output_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("visual-test")
        .join(name)
}

/// Whether golden blessing is enabled via env.
#[must_use]
pub fn blessing_enabled() -> bool {
    matches!(
        std::env::var("LIQUIDE_UPDATE_GOLDEN").ok().as_deref(),
        Some("1") | Some("true")
    ) || matches!(
        std::env::var("BLESS").ok().as_deref(),
        Some("1") | Some("true")
    )
}

/// Compare `frame` against golden `name` using the default tolerance, blessing
/// when enabled. Panics on mismatch with artifact paths.
///
/// This is the primary entry point for scenario `#[test]`s.
pub fn assert_golden(name: &str, frame: &Frame) {
    assert_golden_with(name, frame, DiffOptions::default());
}

/// Like [`assert_golden`] but with explicit [`DiffOptions`] (e.g.
/// [`DiffOptions::exact`] for flat-color goldens).
pub fn assert_golden_with(name: &str, frame: &Frame, opts: DiffOptions) {
    match compare_golden(name, frame, opts) {
        Ok(()) => {}
        Err(VisualTestError::Mismatch(msg)) => panic!("{msg}"),
        Err(e) => panic!("golden '{name}' error: {e}"),
    }
}

/// Non-panicking core of [`assert_golden`]: returns `Ok(())` on match/bless,
/// `Err(VisualTestError::Mismatch)` on a real mismatch, or another error on I/O.
pub fn compare_golden(name: &str, frame: &Frame, opts: DiffOptions) -> Result<(), VisualTestError> {
    let golden_path = golden_dir().join(format!("{name}.png"));
    let bless = blessing_enabled();

    // Load existing golden, if any.
    let existing = if golden_path.exists() {
        Some(Frame::load_png(&golden_path)?)
    } else {
        None
    };

    match existing {
        None => {
            if bless {
                frame.save_png(&golden_path)?;
                eprintln!("blessed new golden '{name}' -> {}", abspath(&golden_path));
                Ok(())
            } else {
                Err(VisualTestError::Mismatch(format!(
                    "golden '{name}' does not exist at {}; run with LIQUIDE_UPDATE_GOLDEN=1 to create it",
                    abspath(&golden_path)
                )))
            }
        }
        Some(expected) => {
            let result = diff_frames(&expected, frame, opts);
            if result.matched {
                return Ok(());
            }
            if bless {
                frame.save_png(&golden_path)?;
                eprintln!("re-blessed golden '{name}' -> {}", abspath(&golden_path));
                return Ok(());
            }
            // Emit expected/actual/diff for eyeballing.
            let out = output_dir(name);
            let expected_out = out.join("expected.png");
            let actual_out = out.join("actual.png");
            let diff_out = out.join("diff.png");
            let _ = expected.save_png(&expected_out);
            let _ = frame.save_png(&actual_out);
            if let Some(diff_img) = &result.diff_image {
                let _ = diff_img.save_png(&diff_out);
            }
            let detail = if result.dimension_mismatch {
                format!(
                    "dimension mismatch: golden {}x{} vs actual {}x{}",
                    expected.width, expected.height, frame.width, frame.height
                )
            } else {
                format!(
                    "{} pixels differ (budget {}), max channel delta {}",
                    result.differing_pixels, opts.max_differing_pixels, result.max_channel_delta
                )
            };
            Err(VisualTestError::Mismatch(format!(
                "golden '{name}' mismatch: {detail}\n  expected: {}\n  actual:   {}\n  diff:     {}\n  (set LIQUIDE_UPDATE_GOLDEN=1 to bless)",
                abspath(&expected_out),
                abspath(&actual_out),
                abspath(&diff_out),
            )))
        }
    }
}

fn abspath(p: &std::path::Path) -> String {
    std::fs::canonicalize(p)
        .map(|c| c.display().to_string())
        .unwrap_or_else(|_| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Frame {
        Frame {
            width: w,
            height: h,
            rgba: rgba
                .iter()
                .copied()
                .cycle()
                .take((w * h * 4) as usize)
                .collect(),
        }
    }

    #[test]
    fn bless_then_match_roundtrip() {
        // Use a unique temp golden name so this is hermetic and does not commit.
        let name = format!("__selftest_bless_{}", std::process::id());
        let path = golden_dir().join(format!("{name}.png"));
        let _ = std::fs::remove_file(&path);

        let frame = solid(6, 6, [10, 120, 200, 255]);

        // Without a golden and without blessing -> mismatch.
        let r = compare_golden(&name, &frame, DiffOptions::default());
        assert!(matches!(r, Err(VisualTestError::Mismatch(_))));

        // Bless it.
        // SAFETY: test is single-threaded over this env var.
        unsafe { std::env::set_var("LIQUIDE_UPDATE_GOLDEN", "1") };
        compare_golden(&name, &frame, DiffOptions::default()).expect("bless should succeed");
        // SAFETY: paired removal of the var set just above.
        unsafe { std::env::remove_var("LIQUIDE_UPDATE_GOLDEN") };
        assert!(path.exists(), "golden file should be written");

        // Now it matches.
        compare_golden(&name, &frame, DiffOptions::default())
            .expect("identical frame should match blessed golden");

        let _ = std::fs::remove_file(&path);
    }
}
