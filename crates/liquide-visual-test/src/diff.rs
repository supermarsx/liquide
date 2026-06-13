//! Threshold / perceptual image comparison.
//!
//! CPU anti-aliasing, the sRGB LUT, and subpixel glyph positioning make exact
//! byte-for-byte matching brittle across toolchains, so the default metric is a
//! per-pixel max-channel delta with a tolerance plus a budget on the number of
//! pixels allowed to exceed it. An exact mode (`tolerance == 0`,
//! `max_differing_pixels == 0`) is available for flat-color scenarios.

use crate::capture::Frame;

/// Tolerances controlling [`diff_frames`].
#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    /// Max allowed per-channel absolute delta before a pixel counts as
    /// "different". `0` = exact match per pixel.
    pub per_channel_tolerance: u8,
    /// Max number of pixels permitted to exceed `per_channel_tolerance` before
    /// the comparison is considered a failure.
    pub max_differing_pixels: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        // Small AA-tolerant default: a few channels of drift on a modest number
        // of pixels is acceptable; large structural differences are not.
        Self {
            per_channel_tolerance: 4,
            max_differing_pixels: 64,
        }
    }
}

impl DiffOptions {
    /// Exact comparison: zero tolerance, zero differing pixels allowed.
    #[must_use]
    pub const fn exact() -> Self {
        Self {
            per_channel_tolerance: 0,
            max_differing_pixels: 0,
        }
    }

    /// Builder: set the per-channel tolerance.
    #[must_use]
    pub const fn tolerance(mut self, t: u8) -> Self {
        self.per_channel_tolerance = t;
        self
    }

    /// Builder: set the differing-pixel budget.
    #[must_use]
    pub const fn budget(mut self, n: usize) -> Self {
        self.max_differing_pixels = n;
        self
    }
}

/// Outcome of comparing two frames.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// Whether the frames are considered equal under the given [`DiffOptions`].
    pub matched: bool,
    /// Number of pixels that exceeded the per-channel tolerance.
    pub differing_pixels: usize,
    /// Largest per-channel delta observed anywhere.
    pub max_channel_delta: u8,
    /// Whether the two frames had mismatched dimensions (always a failure).
    pub dimension_mismatch: bool,
    /// A visual diff image (changed pixels in red over a dimmed base), sized to
    /// the smaller common region. `None` when dimensions differ.
    pub diff_image: Option<Frame>,
}

/// Compare two RGBA frames under `opts`.
///
/// On dimension mismatch, returns `matched: false`, `dimension_mismatch: true`
/// and no diff image. Otherwise computes per-pixel deltas, counts violations
/// against the budget, and produces a diff visualisation.
#[must_use]
pub fn diff_frames(expected: &Frame, actual: &Frame, opts: DiffOptions) -> DiffResult {
    if expected.width != actual.width || expected.height != actual.height {
        return DiffResult {
            matched: false,
            differing_pixels: usize::MAX,
            max_channel_delta: u8::MAX,
            dimension_mismatch: true,
            diff_image: None,
        };
    }

    let w = expected.width;
    let h = expected.height;
    let mut diff_rgba = vec![0u8; (w * h * 4) as usize];
    let mut differing = 0usize;
    let mut max_delta = 0u8;

    for (i, (e, a)) in expected
        .rgba
        .chunks_exact(4)
        .zip(actual.rgba.chunks_exact(4))
        .enumerate()
    {
        let mut pixel_delta = 0u8;
        for c in 0..4 {
            pixel_delta = pixel_delta.max(e[c].abs_diff(a[c]));
        }
        max_delta = max_delta.max(pixel_delta);

        let off = i * 4;
        if pixel_delta > opts.per_channel_tolerance {
            differing += 1;
            // Highlight changed pixels in red.
            diff_rgba[off] = 255;
            diff_rgba[off + 1] = 0;
            diff_rgba[off + 2] = 0;
            diff_rgba[off + 3] = 255;
        } else {
            // Dimmed base for context.
            diff_rgba[off] = e[0] / 3;
            diff_rgba[off + 1] = e[1] / 3;
            diff_rgba[off + 2] = e[2] / 3;
            diff_rgba[off + 3] = 255;
        }
    }

    DiffResult {
        matched: differing <= opts.max_differing_pixels,
        differing_pixels: differing,
        max_channel_delta: max_delta,
        dimension_mismatch: false,
        diff_image: Some(Frame {
            width: w,
            height: h,
            rgba: diff_rgba,
        }),
    }
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
    fn identical_frames_match_exactly() {
        let a = solid(8, 8, [12, 34, 56, 255]);
        let r = diff_frames(&a, &a, DiffOptions::exact());
        assert!(r.matched);
        assert_eq!(r.differing_pixels, 0);
        assert_eq!(r.max_channel_delta, 0);
    }

    #[test]
    fn small_drift_within_tolerance_matches() {
        let a = solid(8, 8, [100, 100, 100, 255]);
        let b = solid(8, 8, [103, 100, 100, 255]);
        let r = diff_frames(&a, &b, DiffOptions::default());
        assert!(
            r.matched,
            "3-channel drift should be within default tolerance"
        );
        assert_eq!(r.max_channel_delta, 3);
    }

    #[test]
    fn large_difference_fails_and_marks_pixels() {
        // 16x16 = 256 differing pixels, well over the default budget (64).
        let a = solid(16, 16, [0, 0, 0, 255]);
        let b = solid(16, 16, [255, 255, 255, 255]);
        let r = diff_frames(&a, &b, DiffOptions::default());
        assert!(!r.matched);
        assert_eq!(r.differing_pixels, 256);
        assert_eq!(r.max_channel_delta, 255);
    }

    #[test]
    fn dimension_mismatch_never_matches() {
        let a = solid(8, 8, [0, 0, 0, 255]);
        let b = solid(4, 4, [0, 0, 0, 255]);
        let r = diff_frames(&a, &b, DiffOptions::exact());
        assert!(!r.matched);
        assert!(r.dimension_mismatch);
        assert!(r.diff_image.is_none());
    }
}
