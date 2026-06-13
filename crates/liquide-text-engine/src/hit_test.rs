//! Hit testing: maps pixel coordinates to text positions.
//!
//! Given a laid-out paragraph, determines which character (or inter-character
//! boundary) is closest to a pixel coordinate. Used for click placement,
//! mouse selection, and touch targeting.

use crate::paragraph::PositionedGlyph;
use crate::paragraph::{LayoutLine, ParagraphLayout};
use crate::selection::{Affinity, TextOffset};

/// Result of a hit test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitTestResult {
    /// The byte offset in the source text.
    pub offset: TextOffset,
    /// Whether the hit is on a glyph (true) or in whitespace/margin (false).
    pub is_inside: bool,
    /// The line index that was hit.
    pub line: usize,
    /// Affinity: which side of a line break the position is on.
    pub affinity: Affinity,
    /// Distance from the exact glyph center (useful for cursor snapping).
    pub distance: f32,
    /// Whether the offset is at the trailing edge of the hit glyph.
    pub trailing: bool,
}

impl HitTestResult {
    /// The result when hitting outside all text.
    #[must_use]
    pub fn outside(offset: usize, line: usize) -> Self {
        Self {
            offset: TextOffset(offset),
            is_inside: false,
            line,
            affinity: Affinity::Downstream,
            distance: f32::INFINITY,
            trailing: false,
        }
    }
}

/// Hit testing engine for laid-out text.
pub struct HitTester;

impl HitTester {
    /// Find the text offset nearest to the given pixel coordinates.
    ///
    /// `x` and `y` are relative to the paragraph's top-left corner.
    #[must_use]
    pub fn hit_test(layout: &ParagraphLayout, x: f32, y: f32) -> HitTestResult {
        if layout.lines.is_empty() {
            return HitTestResult::outside(0, 0);
        }

        // 1. Find the target line.
        let (line_idx, line) = Self::find_line(layout, y);

        // 2. Find the glyph within the line.
        Self::find_glyph_in_line(line, line_idx, x)
    }

    /// Find the text offset at the given X position on a specific line.
    #[must_use]
    pub fn hit_test_line(line: &LayoutLine, line_idx: usize, x: f32) -> HitTestResult {
        Self::find_glyph_in_line(line, line_idx, x)
    }

    /// Find the Y range (top, bottom) for a given line index.
    #[must_use]
    pub fn line_bounds(layout: &ParagraphLayout, line_idx: usize) -> (f32, f32) {
        if line_idx >= layout.lines.len() {
            return (layout.height, layout.height);
        }
        let line = &layout.lines[line_idx];
        let top = line.baseline_y - line.ascent;
        let bottom = line.baseline_y + line.descent;
        (top, bottom)
    }

    /// Get the X position for a given text offset on its line.
    ///
    /// Returns (x, line_idx).
    #[must_use]
    pub fn offset_to_point(layout: &ParagraphLayout, offset: usize) -> (f32, usize) {
        for (line_idx, line) in layout.lines.iter().enumerate() {
            // Check if offset is on this line.
            let line_contains = offset >= line.start && offset <= line.end;
            let is_last_line = line_idx == layout.lines.len() - 1;

            if line_contains || (is_last_line && offset >= line.start) {
                let x = Self::x_for_offset(line, offset);
                return (x, line_idx);
            }
        }

        // Fallback: end of last line.
        if let Some(last) = layout.lines.last() {
            (last.width, layout.lines.len() - 1)
        } else {
            (0.0, 0)
        }
    }

    /// Find X coordinate for a given *global* byte offset on a line.
    ///
    /// Glyph `cluster` values are line-local, so the global offset of a glyph is
    /// `line.start + glyph.cluster`.
    fn x_for_offset(line: &LayoutLine, offset: usize) -> f32 {
        for glyph in &line.glyphs {
            if line.start + glyph.cluster as usize >= offset {
                return glyph.x;
            }
        }

        // Past the last glyph: the caret sits at the end of the line's content.
        // Glyph x positions already include the alignment offset, so the
        // line-end x is the first glyph's left edge plus the line's advance
        // width. (F32: the previous code returned the last glyph's *left* edge,
        // collapsing the end-of-line caret onto the final glyph.)
        line.glyphs.first().map_or(line.width, |g| g.x + line.width)
    }

    /// Find which line the Y coordinate falls on.
    fn find_line(layout: &ParagraphLayout, y: f32) -> (usize, &LayoutLine) {
        let mut accumulated_y: f32 = 0.0;

        for (i, line) in layout.lines.iter().enumerate() {
            let line_top = accumulated_y;
            let line_bottom = accumulated_y + line.ascent + line.descent;

            if y >= line_top && y < line_bottom {
                return (i, line);
            }

            accumulated_y = line_bottom;
        }

        // Below all lines: return the last line.
        let last_idx = layout.lines.len() - 1;
        (last_idx, &layout.lines[last_idx])
    }

    /// Find the nearest glyph boundary within a line.
    fn find_glyph_in_line(line: &LayoutLine, line_idx: usize, x: f32) -> HitTestResult {
        if line.glyphs.is_empty() {
            return HitTestResult::outside(line.start, line_idx);
        }

        // Glyph `cluster` values are line-local; the global byte offset of a
        // glyph is `line.start + glyph.cluster`.
        let global = |g: &PositionedGlyph| line.start + g.cluster as usize;

        // Before the first glyph.
        if x <= line.glyphs[0].x {
            return HitTestResult {
                offset: TextOffset(global(&line.glyphs[0])),
                is_inside: false,
                line: line_idx,
                affinity: Affinity::Downstream,
                distance: (x - line.glyphs[0].x).abs(),
                trailing: false,
            };
        }

        // Check each glyph.
        let mut best_offset = global(&line.glyphs[0]);
        let mut best_distance = f32::INFINITY;
        let mut best_trailing = false;

        // End-of-line x: glyph x positions include the alignment offset, so the
        // line's right edge is the first glyph's left edge plus the line's
        // advance width (F32). Using bare `line.width` undercounts on
        // centered/right-aligned lines and collapses the last glyph's hit zone.
        let line_end_x = line.glyphs[0].x + line.width;

        for (i, glyph) in line.glyphs.iter().enumerate() {
            let glyph_start = glyph.x;
            let glyph_end = if i + 1 < line.glyphs.len() {
                line.glyphs[i + 1].x
            } else {
                line_end_x.max(glyph_start)
            };

            let glyph_center = (glyph_start + glyph_end) / 2.0;

            if x >= glyph_start && x < glyph_end {
                // Inside this glyph.
                let trailing = x >= glyph_center;
                let offset = if trailing {
                    // Trailing edge: offset is after this cluster.
                    next_cluster_offset(line, i)
                } else {
                    global(glyph)
                };

                return HitTestResult {
                    offset: TextOffset(offset),
                    is_inside: true,
                    line: line_idx,
                    affinity: if trailing {
                        Affinity::Upstream
                    } else {
                        Affinity::Downstream
                    },
                    distance: (x - glyph_center).abs(),
                    trailing,
                };
            }

            // Track closest for fallback.
            let dist = (x - glyph_center).abs();
            if dist < best_distance {
                best_distance = dist;
                best_offset = global(glyph);
                best_trailing = x >= glyph_center;
            }
        }

        // After the last glyph.
        HitTestResult {
            offset: TextOffset(if best_trailing {
                next_cluster_offset(line, line.glyphs.len() - 1)
            } else {
                best_offset
            }),
            is_inside: false,
            line: line_idx,
            affinity: Affinity::Upstream,
            distance: best_distance,
            trailing: best_trailing,
        }
    }
}

/// Get the *global* byte offset of the next cluster after the glyph at index
/// `i`. Glyph `cluster` values are line-local, so the global offset is
/// `line.start + cluster`.
fn next_cluster_offset(line: &LayoutLine, i: usize) -> usize {
    for glyph in &line.glyphs[i + 1..] {
        if glyph.cluster != line.glyphs[i].cluster {
            return line.start + glyph.cluster as usize;
        }
    }
    // Past the last cluster: use line end (already a global offset).
    line.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_fallback::FontId;

    fn make_layout_line(glyph_count: usize, glyph_width: f32) -> LayoutLine {
        let glyphs: Vec<PositionedGlyph> = (0..glyph_count)
            .map(|i| PositionedGlyph {
                glyph_id: i as u32,
                font_id: FontId(1),
                size: 16.0,
                x: i as f32 * glyph_width,
                y: 12.0,
                cluster: i as u32,
            })
            .collect();
        LayoutLine {
            glyphs,
            start: 0,
            end: glyph_count,
            baseline_y: 12.0,
            ascent: 12.0,
            descent: 4.0,
            width: glyph_count as f32 * glyph_width,
            hard_break: false,
        }
    }

    #[test]
    fn test_hit_test_empty() {
        let layout = ParagraphLayout {
            lines: vec![],
            width: 0.0,
            height: 0.0,
            truncated: false,
        };
        let result = HitTester::hit_test(&layout, 10.0, 5.0);
        assert_eq!(result.offset.0, 0);
        assert!(!result.is_inside);
    }

    #[test]
    fn test_hit_glyph() {
        let line = make_layout_line(5, 10.0); // 5 glyphs, 10px each
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 50.0,
            height: 16.0,
            truncated: false,
        };

        // Hit in the middle of glyph 2 (x=20..30), leading half
        let result = HitTester::hit_test(&layout, 22.0, 8.0);
        assert!(result.is_inside);
        assert_eq!(result.offset.0, 2); // Leading edge → offset 2
        assert!(!result.trailing);
    }

    #[test]
    fn test_hit_trailing() {
        let line = make_layout_line(5, 10.0);
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 50.0,
            height: 16.0,
            truncated: false,
        };

        // Hit in trailing half of glyph 2 (x=25..30)
        let result = HitTester::hit_test(&layout, 27.0, 8.0);
        assert!(result.is_inside);
        assert_eq!(result.offset.0, 3); // Trailing edge → next offset
        assert!(result.trailing);
    }

    #[test]
    fn test_hit_before_line() {
        let line = make_layout_line(5, 10.0);
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 50.0,
            height: 16.0,
            truncated: false,
        };

        // Hit before the first glyph
        let result = HitTester::hit_test(&layout, -5.0, 8.0);
        assert_eq!(result.offset.0, 0);
        assert!(!result.is_inside);
    }

    #[test]
    fn test_offset_to_point() {
        let line = make_layout_line(5, 10.0);
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 50.0,
            height: 16.0,
            truncated: false,
        };

        let (x, line_idx) = HitTester::offset_to_point(&layout, 3);
        assert_eq!(line_idx, 0);
        assert!((x - 30.0).abs() < 0.01, "x={x}");
    }

    #[test]
    fn test_line_bounds() {
        let line = make_layout_line(5, 10.0);
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 50.0,
            height: 16.0,
            truncated: false,
        };

        let (top, bottom) = HitTester::line_bounds(&layout, 0);
        assert!(top >= 0.0);
        assert!(bottom > top);
    }

    #[test]
    fn test_multi_line_hit() {
        let line1 = make_layout_line(5, 10.0); // 0..5
        let mut line2 = make_layout_line(3, 10.0); // 5..8
        line2.start = 5;
        line2.end = 8;
        line2.baseline_y = 28.0;
        // Clusters are line-local, so line 2's glyphs keep clusters 0,1,2; the
        // global offsets are recovered as `line.start + cluster`.
        for g in line2.glyphs.iter_mut() {
            g.y = 28.0;
        }

        let layout = ParagraphLayout {
            lines: vec![line1, line2],
            width: 50.0,
            height: 32.0,
            truncated: false,
        };

        // Hit on second line
        let result = HitTester::hit_test(&layout, 15.0, 24.0);
        assert_eq!(result.line, 1);
    }

    /// Regression (t49-e3-F11): glyph clusters are line-local, so hit-testing a
    /// line that starts at a non-zero global offset must return the *global*
    /// byte offset (line.start + line-local cluster), not the line-local value.
    #[test]
    fn test_hit_returns_global_offset() {
        // A line whose content lives at global bytes 100..103, with line-local
        // clusters 0,1,2 at x = 0,10,20.
        let mut line = make_layout_line(3, 10.0);
        line.start = 100;
        line.end = 103;
        // make_layout_line already sets clusters 0,1,2 (line-local).

        let layout = ParagraphLayout {
            lines: vec![line],
            width: 30.0,
            height: 16.0,
            truncated: false,
        };

        // Leading half of glyph 1 (x=10..15) → global offset 101.
        let result = HitTester::hit_test(&layout, 11.0, 8.0);
        assert!(result.is_inside);
        assert_eq!(
            result.offset.0, 101,
            "expected global offset, got line-local"
        );

        // Before the first glyph → global line start (100), not 0.
        let before = HitTester::hit_test(&layout, -5.0, 8.0);
        assert_eq!(before.offset.0, 100);
    }

    /// Regression (t49-e3-F32): the end-of-line caret x must equal the line's
    /// advance width measured from the first glyph (which includes the alignment
    /// offset), not the last glyph's left edge.
    #[test]
    fn test_end_of_line_caret_x() {
        // Centered-style line: alignment offset baked into glyph x positions.
        // First glyph at x=90, three glyphs 10px apart, advance width 30.
        let glyphs: Vec<PositionedGlyph> = (0..3)
            .map(|i| PositionedGlyph {
                glyph_id: i as u32,
                font_id: FontId(1),
                size: 16.0,
                x: 90.0 + i as f32 * 10.0,
                y: 12.0,
                cluster: i as u32,
            })
            .collect();
        let line = LayoutLine {
            glyphs,
            start: 0,
            end: 3,
            baseline_y: 12.0,
            ascent: 12.0,
            descent: 4.0,
            width: 30.0, // advance width, excludes the alignment offset
            hard_break: false,
        };
        let layout = ParagraphLayout {
            lines: vec![line],
            width: 200.0,
            height: 16.0,
            truncated: false,
        };

        // Caret at the end-of-line offset (3) should land at 90 + 30 = 120,
        // i.e. the right edge of the content — NOT the last glyph's left edge
        // (110) as the old code produced.
        let (x, line_idx) = HitTester::offset_to_point(&layout, 3);
        assert_eq!(line_idx, 0);
        assert!((x - 120.0).abs() < 0.01, "end-of-line x={x}, expected 120");
    }
}
