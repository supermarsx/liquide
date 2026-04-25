//! Paragraph layout: line breaking, alignment, and glyph positioning.
//!
//! Takes shaped runs and lays them out into positioned lines, handling:
//! - Word wrapping with greedy or optimal (Knuth-Plass) breaking
//! - Alignment: left, right, center, justify
//! - Indentation and margins
//! - Inter-line spacing (leading)
//! - Inline direction (LTR/RTL) with bidi

use serde::{Deserialize, Serialize};

use crate::bidi::Direction;
use crate::font_fallback::FontId;
use crate::rasterizer::FontMetrics;
use crate::shaping::ShapedGlyph;

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    /// Align to the start of the inline direction (left for LTR).
    Start,
    /// Align to the end of the inline direction (right for LTR).
    End,
    /// Center each line.
    Center,
    /// Justify text (expand spaces to fill width).
    Justify,
}

impl Default for TextAlignment {
    fn default() -> Self {
        Self::Start
    }
}

/// Vertical alignment within a text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
    Baseline,
}

impl Default for VerticalAlignment {
    fn default() -> Self {
        Self::Top
    }
}

/// Text overflow behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    /// Clip at the boundary.
    Clip,
    /// Show ellipsis (...) for overflowing text.
    Ellipsis,
    /// Let text overflow visibly.
    Visible,
}

impl Default for TextOverflow {
    fn default() -> Self {
        Self::Clip
    }
}

/// Line wrapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    /// No wrapping (single line).
    NoWrap,
    /// Wrap at word boundaries.
    Word,
    /// Wrap at character boundaries.
    Character,
    /// Try word boundaries first, then break words.
    WordCharacter,
}

impl Default for WrapMode {
    fn default() -> Self {
        Self::Word
    }
}

/// Configuration for paragraph layout.
#[derive(Debug, Clone)]
pub struct ParagraphStyle {
    pub alignment: TextAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub wrap_mode: WrapMode,
    pub overflow: TextOverflow,
    /// Maximum width for line breaking (None = no limit).
    pub max_width: Option<f32>,
    /// Maximum number of lines (None = no limit).
    pub max_lines: Option<usize>,
    /// First-line indent in pixels.
    pub indent: f32,
    /// Line height multiplier (1.0 = normal).
    pub line_height_factor: f32,
    /// Additional space between paragraphs.
    pub paragraph_spacing: f32,
    /// Base direction for the paragraph.
    pub direction: Direction,
    /// Use Knuth-Plass optimal line breaking instead of greedy.
    pub use_optimal_breaking: bool,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            alignment: TextAlignment::Start,
            vertical_alignment: VerticalAlignment::Top,
            wrap_mode: WrapMode::Word,
            overflow: TextOverflow::Clip,
            max_width: None,
            max_lines: None,
            indent: 0.0,
            line_height_factor: 1.2,
            paragraph_spacing: 0.0,
            direction: Direction::Ltr,
            use_optimal_breaking: false,
        }
    }
}

/// A single glyph in the final layout, fully positioned.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// Glyph index.
    pub glyph_id: u32,
    /// Font that owns this glyph.
    pub font_id: FontId,
    /// Font size in px.
    pub size: f32,
    /// X position relative to the paragraph origin.
    pub x: f32,
    /// Y position (baseline) relative to the paragraph origin.
    pub y: f32,
    /// Character cluster index in the source text.
    pub cluster: u32,
}

/// A single line in the laid-out paragraph.
#[derive(Debug, Clone)]
pub struct LayoutLine {
    /// Glyphs on this line.
    pub glyphs: Vec<PositionedGlyph>,
    /// Byte range in the source text.
    pub start: usize,
    pub end: usize,
    /// Baseline Y position relative to the paragraph top.
    pub baseline_y: f32,
    /// Ascent of this line (max ascent of all fonts on this line).
    pub ascent: f32,
    /// Descent of this line.
    pub descent: f32,
    /// Total width of this line's content.
    pub width: f32,
    /// Whether this line was hard-broken (newline) or soft-broken (wrapped).
    pub hard_break: bool,
}

impl LayoutLine {
    /// Total height of this line (ascent + descent).
    #[must_use]
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// The fully laid-out paragraph.
#[derive(Debug, Clone)]
pub struct ParagraphLayout {
    /// All positioned lines.
    pub lines: Vec<LayoutLine>,
    /// Total width of the widest line.
    pub width: f32,
    /// Total height of all lines.
    pub height: f32,
    /// Whether the text was truncated (exceeded max_lines or overflowed).
    pub truncated: bool,
}

/// A glyph run to be laid out (input to layout).
#[derive(Debug, Clone)]
pub struct GlyphRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub font_id: FontId,
    pub size: f32,
    pub direction: Direction,
    pub metrics: FontMetrics,
    /// Byte range in source text.
    pub start: usize,
    pub end: usize,
}

/// The paragraph layouter.
pub struct ParagraphLayouter {
    style: ParagraphStyle,
}

impl ParagraphLayouter {
    #[must_use]
    pub fn new(style: ParagraphStyle) -> Self {
        Self { style }
    }

    /// Layout glyph runs into positioned lines.
    #[must_use]
    pub fn layout(&self, text: &str, runs: &[GlyphRun]) -> ParagraphLayout {
        if runs.is_empty() || text.is_empty() {
            return ParagraphLayout {
                lines: Vec::new(),
                width: 0.0,
                height: 0.0,
                truncated: false,
            };
        }

        // Flatten all glyphs with their font info into a sequence.
        // Each glyph's byte_offset is computed from the run's start plus the
        // cluster index (which corresponds to the byte offset within the run's
        // text for well-formed ASCII/Unicode clusters).
        let items: Vec<LayoutItem> = runs
            .iter()
            .flat_map(|run| {
                let run_text = &text[run.start..run.end.min(text.len())];
                // Build a mapping from cluster index to byte offset within the run.
                let char_offsets: Vec<usize> = run_text
                    .char_indices()
                    .map(|(byte_off, _)| byte_off)
                    .collect();
                run.glyphs.iter().map(move |g| {
                    let cluster_idx = g.cluster as usize;
                    let local_offset = char_offsets.get(cluster_idx).copied().unwrap_or(0);
                    LayoutItem {
                        glyph: *g,
                        font_id: run.font_id,
                        size: run.size,
                        metrics: run.metrics,
                        byte_offset: run.start + local_offset,
                    }
                })
            })
            .collect();

        // Break into lines.
        let max_width = self.style.max_width.unwrap_or(f32::INFINITY);
        let raw_lines = if self.style.use_optimal_breaking && max_width.is_finite() {
            self.break_lines_knuth_plass(text, &items, max_width)
        } else {
            self.break_lines(text, &items, max_width)
        };

        // Apply alignment and produce final layout.
        self.position_lines(raw_lines, max_width)
    }

    /// Greedy line breaking.
    fn break_lines(&self, text: &str, items: &[LayoutItem], max_width: f32) -> Vec<RawLine> {
        let mut lines: Vec<RawLine> = Vec::new();
        let mut line_start = 0;
        let mut line_width: f32 = 0.0;
        let mut last_break = 0;
        let mut last_break_width: f32 = 0.0;
        let mut line_ascent: f32 = 0.0;
        let mut line_descent: f32 = 0.0;

        let is_first_line = |lines: &Vec<RawLine>| lines.is_empty();

        for (i, item) in items.iter().enumerate() {
            let indent = if is_first_line(&lines) {
                self.style.indent
            } else {
                0.0
            };
            let effective_width = max_width - indent;

            line_ascent = line_ascent.max(item.metrics.ascent);
            line_descent = line_descent.max(item.metrics.descent);

            let glyph_width = item.glyph.x_advance;

            // Check if this is a break opportunity.
            let ch = text[item.byte_offset..].chars().next().unwrap_or(' ');

            if ch == ' ' || ch == '\t' {
                last_break = i + 1;
                last_break_width = line_width + glyph_width;
            }

            // Check for hard break.
            if ch == '\n' {
                lines.push(RawLine {
                    items: items[line_start..i].to_vec(),
                    width: line_width,
                    ascent: line_ascent,
                    descent: line_descent,
                    hard_break: true,
                });
                line_start = i + 1;
                line_width = 0.0;
                last_break = i + 1;
                last_break_width = 0.0;
                line_ascent = 0.0;
                line_descent = 0.0;
                continue;
            }

            line_width += glyph_width;

            // Need to wrap?
            if line_width > effective_width && self.style.wrap_mode != WrapMode::NoWrap {
                match self.style.wrap_mode {
                    WrapMode::Word | WrapMode::WordCharacter if last_break > line_start => {
                        // Break at last word boundary.
                        let break_items = items[line_start..last_break].to_vec();
                        lines.push(RawLine {
                            width: last_break_width
                                - break_items
                                    .last()
                                    .map(|it| it.glyph.x_advance)
                                    .unwrap_or(0.0),
                            items: break_items,
                            ascent: line_ascent,
                            descent: line_descent,
                            hard_break: false,
                        });
                        line_start = last_break;
                        line_width = items[last_break..=i]
                            .iter()
                            .map(|it| it.glyph.x_advance)
                            .sum();
                        last_break = line_start;
                        last_break_width = 0.0;
                    }
                    WrapMode::Character | WrapMode::WordCharacter => {
                        // Break right before this glyph.
                        lines.push(RawLine {
                            items: items[line_start..i].to_vec(),
                            width: line_width - glyph_width,
                            ascent: line_ascent,
                            descent: line_descent,
                            hard_break: false,
                        });
                        line_start = i;
                        line_width = glyph_width;
                        last_break = i;
                        last_break_width = 0.0;
                    }
                    _ => {
                        // NoWrap or Word without break point: keep going.
                    }
                }

                // Check max lines.
                if let Some(max_lines) = self.style.max_lines {
                    if lines.len() >= max_lines {
                        return lines;
                    }
                }
            }
        }

        // Final line.
        if line_start < items.len() {
            lines.push(RawLine {
                items: items[line_start..].to_vec(),
                width: line_width,
                ascent: line_ascent,
                descent: line_descent,
                hard_break: false,
            });
        }

        // Enforce max_lines.
        if let Some(max_lines) = self.style.max_lines {
            lines.truncate(max_lines);
        }

        lines
    }

    /// Knuth-Plass optimal line breaking.
    ///
    /// Uses dynamic programming to minimise the sum of squared badness
    /// (deviation from the target line width) across all lines. This gives
    /// a globally optimal paragraph layout instead of the locally optimal
    /// greedy approach.
    ///
    /// Reference: D.E. Knuth & M.F. Plass, "Breaking Paragraphs into Lines",
    /// Software — Practice and Experience, Vol. 11, 1981.
    fn break_lines_knuth_plass(
        &self,
        text: &str,
        items: &[LayoutItem],
        max_width: f32,
    ) -> Vec<RawLine> {
        if items.is_empty() {
            return Vec::new();
        }

        let n = items.len();

        // Precompute prefix sums of advance widths for O(1) range queries.
        let mut prefix_w = vec![0.0f32; n + 1];
        for (i, item) in items.iter().enumerate() {
            prefix_w[i + 1] = prefix_w[i] + item.glyph.x_advance;
        }

        // Identify break opportunities (after spaces or hard breaks).
        let mut break_points: Vec<usize> = vec![0]; // Start of paragraph is a break
        for (i, item) in items.iter().enumerate() {
            let ch = text[item.byte_offset..].chars().next().unwrap_or(' ');
            if ch == '\n' || ch == ' ' || ch == '\t' {
                break_points.push(i + 1);
            }
        }
        if *break_points.last().unwrap_or(&0) != n {
            break_points.push(n);
        }

        let bp_count = break_points.len();

        // cost[i] = minimum total cost to typeset from break_points[i] to end.
        // parent[i] = which break_points[j] follows break_points[i] in optimal solution.
        let mut cost = vec![f64::INFINITY; bp_count];
        let mut parent = vec![0usize; bp_count];
        cost[bp_count - 1] = 0.0; // End of paragraph has zero cost.

        // Penalty constants
        const INFINITY_PENALTY: f64 = 1e10;

        // Work backwards
        for i in (0..bp_count - 1).rev() {
            let start = break_points[i];
            let indent = if i == 0 { self.style.indent } else { 0.0 };
            let effective_width = max_width - indent;

            for j in (i + 1)..bp_count {
                let end = break_points[j];

                // Compute line width for items[start..end], trimming trailing space.
                let mut line_w = prefix_w[end] - prefix_w[start];

                // Trim trailing whitespace advance
                if end > start {
                    let last_ch = text[items[end - 1].byte_offset..]
                        .chars()
                        .next()
                        .unwrap_or('x');
                    if last_ch == ' ' || last_ch == '\t' || last_ch == '\n' {
                        line_w -= items[end - 1].glyph.x_advance;
                    }
                }

                if line_w > effective_width * 1.0001 && j > i + 1 {
                    // Line is too wide — stop trying wider spans (they'll only be worse).
                    break;
                }

                // Check for hard break
                let is_hard = if end > start {
                    let ch = text[items[end - 1].byte_offset..]
                        .chars()
                        .next()
                        .unwrap_or('x');
                    ch == '\n'
                } else {
                    false
                };

                // Compute badness: (excess whitespace / total stretch) ^3
                let shortfall = effective_width - line_w;
                let line_cost = if j == bp_count - 1 {
                    // Last line: only penalise if it's far too long.
                    if line_w > effective_width * 1.0001 {
                        INFINITY_PENALTY
                    } else {
                        0.0 // Last line can be any width ≤ max
                    }
                } else if is_hard {
                    // Forced break: no badness for the break itself.
                    0.0
                } else if shortfall < 0.0 {
                    // Overfull line
                    let ratio = (-shortfall / effective_width.max(1.0)) as f64;
                    INFINITY_PENALTY * ratio * ratio
                } else {
                    // Normal line: penalise large deviations
                    let ratio = (shortfall / effective_width.max(1.0)) as f64;
                    ratio * ratio * ratio * 100.0
                };

                let total = line_cost + cost[j];
                if total < cost[i] {
                    cost[i] = total;
                    parent[i] = j;
                }
            }
        }

        // Reconstruct optimal break sequence
        let mut breaks = Vec::new();
        let mut idx = 0;
        while idx < bp_count - 1 {
            let next = parent[idx];
            let start = break_points[idx];
            let end = break_points[next];

            // Determine if this was a hard break
            let is_hard = if end > start {
                let ch = text[items[end - 1].byte_offset..]
                    .chars()
                    .next()
                    .unwrap_or('x');
                ch == '\n'
            } else {
                false
            };

            // Trim the line: exclude trailing whitespace/newline items
            let mut actual_end = end;
            while actual_end > start {
                let ch = text[items[actual_end - 1].byte_offset..]
                    .chars()
                    .next()
                    .unwrap_or('x');
                if ch == ' ' || ch == '\t' || ch == '\n' {
                    actual_end -= 1;
                } else {
                    break;
                }
            }

            let line_items = items[start..actual_end].to_vec();
            let line_w: f32 = line_items.iter().map(|it| it.glyph.x_advance).sum();
            let line_ascent = line_items
                .iter()
                .map(|it| it.metrics.ascent)
                .fold(0.0f32, f32::max);
            let line_descent = line_items
                .iter()
                .map(|it| it.metrics.descent)
                .fold(0.0f32, f32::max);

            breaks.push(RawLine {
                items: line_items,
                width: line_w,
                ascent: if line_ascent > 0.0 { line_ascent } else { 12.0 },
                descent: if line_descent > 0.0 {
                    line_descent
                } else {
                    4.0
                },
                hard_break: is_hard,
            });

            idx = next;
        }

        // Enforce max_lines.
        if let Some(max_lines) = self.style.max_lines {
            breaks.truncate(max_lines);
        }

        breaks
    }

    /// Position lines according to alignment and produce the final layout.
    fn position_lines(&self, raw_lines: Vec<RawLine>, max_width: f32) -> ParagraphLayout {
        let mut lines: Vec<LayoutLine> = Vec::with_capacity(raw_lines.len());
        let mut y: f32 = 0.0;
        let mut total_width: f32 = 0.0;
        let truncated = self
            .style
            .max_lines
            .map_or(false, |max| raw_lines.len() >= max);

        let finite_max = if max_width.is_finite() {
            max_width
        } else {
            0.0
        };

        for (line_idx, raw) in raw_lines.iter().enumerate() {
            let line_height = (raw.ascent + raw.descent) * self.style.line_height_factor;
            let baseline = y + raw.ascent;

            // Compute alignment offset.
            let x_offset = match self.style.alignment {
                TextAlignment::Start => {
                    if line_idx == 0 {
                        self.style.indent
                    } else {
                        0.0
                    }
                }
                TextAlignment::End => {
                    (if max_width.is_finite() {
                        max_width
                    } else {
                        0.0
                    }) - raw.width
                }
                TextAlignment::Center => {
                    ((if max_width.is_finite() {
                        max_width
                    } else {
                        0.0
                    }) - raw.width)
                        / 2.0
                }
                TextAlignment::Justify if !raw.hard_break && raw.items.len() > 1 => {
                    if line_idx == 0 {
                        self.style.indent
                    } else {
                        0.0
                    }
                }
                TextAlignment::Justify => {
                    if line_idx == 0 {
                        self.style.indent
                    } else {
                        0.0
                    }
                }
            };

            // Position glyphs.
            let mut positioned = Vec::with_capacity(raw.items.len());
            let mut x = x_offset;

            // For justified text, compute extra spacing.
            let justify_extra = if self.style.alignment == TextAlignment::Justify
                && !raw.hard_break
                && raw.items.len() > 1
                && max_width.is_finite()
            {
                let gap = max_width - raw.width - x_offset;
                let spaces = (raw.items.len() - 1).max(1) as f32;
                (gap / spaces).max(0.0)
            } else {
                0.0
            };

            for item in &raw.items {
                positioned.push(PositionedGlyph {
                    glyph_id: item.glyph.glyph_id,
                    font_id: item.font_id,
                    size: item.size,
                    x: x + item.glyph.x_offset,
                    y: baseline + item.glyph.y_offset,
                    cluster: item.glyph.cluster,
                });
                x += item.glyph.x_advance + justify_extra;
            }

            let line_width = if positioned.is_empty() {
                0.0
            } else {
                x - justify_extra - x_offset
            };

            total_width = total_width.max(line_width);

            let line_start = raw.items.first().map(|it| it.byte_offset).unwrap_or(0);
            let line_end = raw
                .items
                .last()
                .map(|it| it.byte_offset + it.glyph.cluster as usize)
                .unwrap_or(0);

            lines.push(LayoutLine {
                glyphs: positioned,
                start: line_start,
                end: line_end,
                baseline_y: baseline,
                ascent: raw.ascent,
                descent: raw.descent,
                width: line_width,
                hard_break: raw.hard_break,
            });

            y += line_height;
        }

        let _ = finite_max;

        ParagraphLayout {
            lines,
            width: total_width,
            height: y,
            truncated,
        }
    }
}

/// Internal line representation before positioning.
#[derive(Debug, Clone)]
struct RawLine {
    items: Vec<LayoutItem>,
    width: f32,
    ascent: f32,
    descent: f32,
    hard_break: bool,
}

/// A glyph with its associated font information, used during layout.
#[derive(Debug, Clone, Copy)]
struct LayoutItem {
    glyph: ShapedGlyph,
    font_id: FontId,
    size: f32,
    metrics: FontMetrics,
    byte_offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rasterizer::FontMetrics;

    fn make_run(text: &str, advance: f32) -> GlyphRun {
        let glyphs: Vec<ShapedGlyph> = text
            .chars()
            .enumerate()
            .map(|(i, _)| ShapedGlyph {
                glyph_id: i as u32,
                cluster: i as u32,
                x_advance: advance,
                y_advance: 0.0,
                x_offset: 0.0,
                y_offset: 0.0,
            })
            .collect();
        GlyphRun {
            glyphs,
            font_id: FontId(1),
            size: 16.0,
            direction: Direction::Ltr,
            metrics: FontMetrics::default_for_size(16.0),
            start: 0,
            end: text.len(),
        }
    }

    #[test]
    fn test_empty_layout() {
        let layouter = ParagraphLayouter::new(ParagraphStyle::default());
        let layout = layouter.layout("", &[]);
        assert!(layout.lines.is_empty());
        assert_eq!(layout.height, 0.0);
    }

    #[test]
    fn test_single_line() {
        let style = ParagraphStyle {
            max_width: Some(500.0),
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        let run = make_run("Hello", 10.0);
        let layout = layouter.layout("Hello", &[run]);
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.lines[0].glyphs.len(), 5);
        assert!((layout.lines[0].width - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_word_wrap() {
        let style = ParagraphStyle {
            max_width: Some(55.0),
            wrap_mode: WrapMode::Word,
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        // "Hello World" = 11 chars at 10px each = 110px → should wrap
        let run = make_run("Hello World", 10.0);
        let layout = layouter.layout("Hello World", &[run]);
        assert!(
            layout.lines.len() >= 2,
            "expected wrap, got {} lines",
            layout.lines.len()
        );
    }

    #[test]
    fn test_center_alignment() {
        let style = ParagraphStyle {
            max_width: Some(200.0),
            alignment: TextAlignment::Center,
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        let run = make_run("Hi", 10.0); // 20px wide
        let layout = layouter.layout("Hi", &[run]);
        assert_eq!(layout.lines.len(), 1);
        // Center: offset should be (200 - 20) / 2 = 90
        let first_x = layout.lines[0].glyphs[0].x;
        assert!((first_x - 90.0).abs() < 0.5, "first_x={first_x}");
    }

    #[test]
    fn test_max_lines() {
        let style = ParagraphStyle {
            max_width: Some(50.0),
            max_lines: Some(1),
            wrap_mode: WrapMode::Word,
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        let run = make_run("Hello World Test", 10.0);
        let layout = layouter.layout("Hello World Test", &[run]);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.truncated);
    }

    #[test]
    fn test_no_wrap() {
        let style = ParagraphStyle {
            max_width: Some(30.0),
            wrap_mode: WrapMode::NoWrap,
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        let run = make_run("Hello World", 10.0);
        let layout = layouter.layout("Hello World", &[run]);
        assert_eq!(layout.lines.len(), 1); // single line even though it overflows
    }

    #[test]
    fn test_line_height() {
        let style = ParagraphStyle {
            max_width: Some(500.0),
            line_height_factor: 1.5,
            ..Default::default()
        };
        let layouter = ParagraphLayouter::new(style);
        let run = make_run("Hello", 10.0);
        let layout = layouter.layout("Hello", &[run]);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.height > 0.0);
    }
}
