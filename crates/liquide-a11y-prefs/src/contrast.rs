//! WCAG 2.1 contrast ratio utilities.
//!
//! Implements relative luminance and contrast ratio calculations per
//! [WCAG 2.1 §1.4.3](https://www.w3.org/TR/WCAG21/#contrast-minimum).

/// Compute the relative luminance of an sRGB color per WCAG 2.1.
///
/// Returns a value in `[0.0, 1.0]` where 0 is darkest and 1 is lightest.
///
/// Formula: `L = 0.2126 * R + 0.7152 * G + 0.0722 * B`
/// where each channel is linearised from sRGB.
#[must_use]
pub fn luminance(r: u8, g: u8, b: u8) -> f64 {
    fn linearize(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// Compute the WCAG 2.1 contrast ratio between two sRGB colors.
///
/// The result is in `[1.0, 21.0]`. The order of `fg` and `bg` does
/// not matter — the brighter color is always the numerator.
#[must_use]
pub fn contrast_ratio(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f64 {
    let l1 = luminance(fg.0, fg.1, fg.2);
    let l2 = luminance(bg.0, bg.1, bg.2);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Check if a contrast ratio meets WCAG AA for normal text (>= 4.5:1).
#[must_use]
pub fn meets_aa(ratio: f64) -> bool {
    ratio >= 4.5
}

/// Check if a contrast ratio meets WCAG AAA for normal text (>= 7.0:1).
#[must_use]
pub fn meets_aaa(ratio: f64) -> bool {
    ratio >= 7.0
}

/// Check if a contrast ratio meets WCAG AA for large text (>= 3.0:1).
///
/// Large text is defined as 18pt (24px) or 14pt (18.66px) bold.
#[must_use]
pub fn meets_aa_large(ratio: f64) -> bool {
    ratio >= 3.0
}

/// Suggest an adjusted foreground color that meets the given `target_ratio`
/// against `bg`.
///
/// The function preserves the hue of `fg` as much as possible by
/// scaling luminance up or down. If the target cannot be met exactly
/// (e.g. the background is mid-grey and the target is 21:1), the
/// function returns the closest achievable color (pure black or white).
#[must_use]
pub fn suggest_color(fg: (u8, u8, u8), bg: (u8, u8, u8), target_ratio: f64) -> (u8, u8, u8) {
    let current = contrast_ratio(fg, bg);
    if current >= target_ratio {
        return fg;
    }

    let bg_lum = luminance(bg.0, bg.1, bg.2);

    // Try both directions (darken and lighten) and pick the one that
    // meets the target while staying closest to the original color.
    let dark_candidate = search_darken(fg, bg, target_ratio);
    let light_candidate = search_lighten(fg, bg, target_ratio);

    let dark_ratio = contrast_ratio(dark_candidate, bg);
    let light_ratio = contrast_ratio(light_candidate, bg);

    let dark_ok = dark_ratio >= target_ratio;
    let light_ok = light_ratio >= target_ratio;

    match (dark_ok, light_ok) {
        (true, true) => {
            // Both directions work — pick the one closer to the original.
            let dark_dist = color_distance(fg, dark_candidate);
            let light_dist = color_distance(fg, light_candidate);
            if dark_dist <= light_dist {
                dark_candidate
            } else {
                light_candidate
            }
        }
        (true, false) => dark_candidate,
        (false, true) => light_candidate,
        (false, false) => {
            // Neither direction achieves the target — pick the better one.
            // Typically one direction gives contrast_ratio close to 1:1
            // (approaching bg) while the other gives max contrast.
            if bg_lum >= 0.5 {
                (0, 0, 0)
            } else {
                (255, 255, 255)
            }
        }
    }
}

/// Binary-search darkening: scale fg toward black until target is met.
/// Returns the closest-to-original color that meets the target, or pure black.
fn search_darken(fg: (u8, u8, u8), bg: (u8, u8, u8), target_ratio: f64) -> (u8, u8, u8) {
    let mut best = (0u8, 0u8, 0u8);
    let mut best_ratio = contrast_ratio(best, bg);
    // factor: 0.0 = black, 1.0 = unchanged
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        let candidate = scale_color(fg, mid);
        let r = contrast_ratio(candidate, bg);
        if r >= target_ratio {
            // Dark enough — try closer to original (higher factor).
            lo = mid;
            best = candidate;
            best_ratio = r;
        } else {
            // Not dark enough — go darker (lower factor).
            hi = mid;
        }
    }
    if best_ratio < target_ratio {
        // Even pure black didn't meet it (shouldn't normally happen).
        best = (0, 0, 0);
    }
    best
}

/// Binary-search lightening: scale fg toward white until target is met.
/// Returns the closest-to-original color that meets the target, or pure white.
fn search_lighten(fg: (u8, u8, u8), bg: (u8, u8, u8), target_ratio: f64) -> (u8, u8, u8) {
    let mut best = (255u8, 255u8, 255u8);
    let mut best_ratio = contrast_ratio(best, bg);
    // t: 0.0 = unchanged, 1.0 = white
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        let candidate = scale_toward_white(fg, mid);
        let r = contrast_ratio(candidate, bg);
        if r >= target_ratio {
            // Bright enough — try closer to original (lower t).
            hi = mid;
            best = candidate;
            best_ratio = r;
        } else {
            // Not bright enough — go brighter (higher t).
            lo = mid;
        }
    }
    if best_ratio < target_ratio {
        best = (255, 255, 255);
    }
    best
}

/// Squared Euclidean distance between two colors in sRGB space.
fn color_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let dr = a.0 as f64 - b.0 as f64;
    let dg = a.1 as f64 - b.1 as f64;
    let db = a.2 as f64 - b.2 as f64;
    dr * dr + dg * dg + db * db
}

/// Scale each channel by `factor` (0.0 = black, 1.0 = unchanged).
fn scale_color(c: (u8, u8, u8), factor: f64) -> (u8, u8, u8) {
    (
        (c.0 as f64 * factor).round().clamp(0.0, 255.0) as u8,
        (c.1 as f64 * factor).round().clamp(0.0, 255.0) as u8,
        (c.2 as f64 * factor).round().clamp(0.0, 255.0) as u8,
    )
}

/// Lerp each channel from `c` toward 255 by `t` (0.0 = unchanged, 1.0 = white).
fn scale_toward_white(c: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    fn lerp(a: u8, t: f64) -> u8 {
        (a as f64 + (255.0 - a as f64) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    }
    (lerp(c.0, t), lerp(c.1, t), lerp(c.2, t))
}
