//! Logical property resolution -- maps logical CSS properties to physical equivalents
//! based on writing-mode and direction.

use super::StyleEngine;
use crate::computed::*;
use crate::dimension::Dimension;

impl StyleEngine {
    /// Resolve logical CSS properties to their physical equivalents based on
    /// writing-mode and direction.
    pub(crate) fn resolve_logical_properties(style: &mut ComputedStyle) {
        let is_horizontal = matches!(style.writing_mode, WritingMode::HorizontalTb);
        let is_ltr = matches!(style.direction, Direction::Ltr);

        // ── Logical sizing -> physical ──
        if !matches!(style.inline_size, Dimension::Auto) {
            if is_horizontal {
                style.width = style.inline_size.clone();
            } else {
                style.height = style.inline_size.clone();
            }
        }
        if !matches!(style.block_size, Dimension::Auto) {
            if is_horizontal {
                style.height = style.block_size.clone();
            } else {
                style.width = style.block_size.clone();
            }
        }
        if !matches!(style.min_inline_size, Dimension::Auto) {
            if is_horizontal {
                style.min_width = style.min_inline_size.clone();
            } else {
                style.min_height = style.min_inline_size.clone();
            }
        }
        if !matches!(style.min_block_size, Dimension::Auto) {
            if is_horizontal {
                style.min_height = style.min_block_size.clone();
            } else {
                style.min_width = style.min_block_size.clone();
            }
        }
        // max-* default to `None` (no limit), not `Auto`; only override the
        // physical longhand when a logical max value was actually authored,
        // otherwise the unset `None` clobbers a directly-set max-width/height.
        if !matches!(style.max_inline_size, Dimension::Auto | Dimension::None) {
            if is_horizontal {
                style.max_width = style.max_inline_size.clone();
            } else {
                style.max_height = style.max_inline_size.clone();
            }
        }
        if !matches!(style.max_block_size, Dimension::Auto | Dimension::None) {
            if is_horizontal {
                style.max_height = style.max_block_size.clone();
            } else {
                style.max_width = style.max_block_size.clone();
            }
        }

        // ── Logical margin -> physical ──
        // inline-start/end -> left/right (horizontal) or top/bottom (vertical)
        if !matches!(style.margin_inline_start, Dimension::Auto)
            || !matches!(style.margin_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.margin_inline_start.clone(),
                    style.margin_inline_end.clone(),
                )
            } else {
                (
                    style.margin_inline_end.clone(),
                    style.margin_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.margin.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.margin.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.margin.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.margin.bottom = end;
                }
            }
        }
        if !matches!(style.margin_block_start, Dimension::Auto)
            || !matches!(style.margin_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.margin_block_start, Dimension::Auto) {
                    style.margin.top = style.margin_block_start.clone();
                }
                if !matches!(style.margin_block_end, Dimension::Auto) {
                    style.margin.bottom = style.margin_block_end.clone();
                }
            } else {
                if !matches!(style.margin_block_start, Dimension::Auto) {
                    style.margin.left = style.margin_block_start.clone();
                }
                if !matches!(style.margin_block_end, Dimension::Auto) {
                    style.margin.right = style.margin_block_end.clone();
                }
            }
        }

        // ── Logical padding -> physical ──
        if !matches!(style.padding_inline_start, Dimension::Auto)
            || !matches!(style.padding_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.padding_inline_start.clone(),
                    style.padding_inline_end.clone(),
                )
            } else {
                (
                    style.padding_inline_end.clone(),
                    style.padding_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.padding.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.padding.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.padding.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.padding.bottom = end;
                }
            }
        }
        if !matches!(style.padding_block_start, Dimension::Auto)
            || !matches!(style.padding_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.padding_block_start, Dimension::Auto) {
                    style.padding.top = style.padding_block_start.clone();
                }
                if !matches!(style.padding_block_end, Dimension::Auto) {
                    style.padding.bottom = style.padding_block_end.clone();
                }
            } else {
                if !matches!(style.padding_block_start, Dimension::Auto) {
                    style.padding.left = style.padding_block_start.clone();
                }
                if !matches!(style.padding_block_end, Dimension::Auto) {
                    style.padding.right = style.padding_block_end.clone();
                }
            }
        }

        // ── Logical inset -> physical ──
        if !matches!(style.inset_inline_start, Dimension::Auto)
            || !matches!(style.inset_inline_end, Dimension::Auto)
        {
            let (start, end) = if is_ltr {
                (
                    style.inset_inline_start.clone(),
                    style.inset_inline_end.clone(),
                )
            } else {
                (
                    style.inset_inline_end.clone(),
                    style.inset_inline_start.clone(),
                )
            };
            if is_horizontal {
                if !matches!(start, Dimension::Auto) {
                    style.left = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.right = end;
                }
            } else {
                if !matches!(start, Dimension::Auto) {
                    style.top = start;
                }
                if !matches!(end, Dimension::Auto) {
                    style.bottom = end;
                }
            }
        }
        if !matches!(style.inset_block_start, Dimension::Auto)
            || !matches!(style.inset_block_end, Dimension::Auto)
        {
            if is_horizontal {
                if !matches!(style.inset_block_start, Dimension::Auto) {
                    style.top = style.inset_block_start.clone();
                }
                if !matches!(style.inset_block_end, Dimension::Auto) {
                    style.bottom = style.inset_block_end.clone();
                }
            } else {
                if !matches!(style.inset_block_start, Dimension::Auto) {
                    style.left = style.inset_block_start.clone();
                }
                if !matches!(style.inset_block_end, Dimension::Auto) {
                    style.right = style.inset_block_end.clone();
                }
            }
        }

        // ── Logical border-width -> physical ──
        if style.border_inline_start_width > 0.0 || style.border_inline_end_width > 0.0 {
            let (sw, ew) = if is_ltr {
                (
                    style.border_inline_start_width,
                    style.border_inline_end_width,
                )
            } else {
                (
                    style.border_inline_end_width,
                    style.border_inline_start_width,
                )
            };
            if is_horizontal {
                if sw > 0.0 {
                    style.border_width.left = sw;
                }
                if ew > 0.0 {
                    style.border_width.right = ew;
                }
            } else {
                if sw > 0.0 {
                    style.border_width.top = sw;
                }
                if ew > 0.0 {
                    style.border_width.bottom = ew;
                }
            }
        }
        if style.border_block_start_width > 0.0 || style.border_block_end_width > 0.0 {
            if is_horizontal {
                if style.border_block_start_width > 0.0 {
                    style.border_width.top = style.border_block_start_width;
                }
                if style.border_block_end_width > 0.0 {
                    style.border_width.bottom = style.border_block_end_width;
                }
            } else {
                if style.border_block_start_width > 0.0 {
                    style.border_width.left = style.border_block_start_width;
                }
                if style.border_block_end_width > 0.0 {
                    style.border_width.right = style.border_block_end_width;
                }
            }
        }

        // ── Logical border-radius -> physical ──
        // start-start -> top-left  (in horizontal-tb LTR)
        if style.border_start_start_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.top_left = style.border_start_start_radius.into();
            } else if is_horizontal {
                style.border_radius.top_right = style.border_start_start_radius.into();
            } else if is_ltr {
                style.border_radius.top_left = style.border_start_start_radius.into();
            } else {
                style.border_radius.bottom_left = style.border_start_start_radius.into();
            }
        }
        if style.border_start_end_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.top_right = style.border_start_end_radius.into();
            } else if is_horizontal {
                style.border_radius.top_left = style.border_start_end_radius.into();
            } else if is_ltr {
                style.border_radius.bottom_left = style.border_start_end_radius.into();
            } else {
                style.border_radius.top_left = style.border_start_end_radius.into();
            }
        }
        if style.border_end_start_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.bottom_left = style.border_end_start_radius.into();
            } else if is_horizontal {
                style.border_radius.bottom_right = style.border_end_start_radius.into();
            } else if is_ltr {
                style.border_radius.top_right = style.border_end_start_radius.into();
            } else {
                style.border_radius.bottom_right = style.border_end_start_radius.into();
            }
        }
        if style.border_end_end_radius > 0.0 {
            if is_horizontal && is_ltr {
                style.border_radius.bottom_right = style.border_end_end_radius.into();
            } else if is_horizontal {
                style.border_radius.bottom_left = style.border_end_end_radius.into();
            } else if is_ltr {
                style.border_radius.bottom_right = style.border_end_end_radius.into();
            } else {
                style.border_radius.top_right = style.border_end_end_radius.into();
            }
        }

        // ── Individual transform properties -> transform list ──
        // CSS spec: individual transforms are applied as translate -> rotate -> scale
        // AFTER the transform property list.
        if let Some(ref t) = style.translate {
            let parts: Vec<&str> = t.split_whitespace().collect();
            let tx = Self::parse_px_value(parts.first().copied().unwrap_or("0")).unwrap_or(0.0);
            let ty = Self::parse_px_value(parts.get(1).copied().unwrap_or("0")).unwrap_or(0.0);
            if tx != 0.0 || ty != 0.0 {
                style.transform.push(Transform::Translate(tx, ty));
            }
        }
        if let Some(ref r) = style.rotate {
            let angle = r.trim_end_matches("deg").parse::<f32>().unwrap_or(0.0);
            if angle != 0.0 {
                style.transform.push(Transform::Rotate(angle));
            }
        }
        if let Some(ref s) = style.scale {
            let parts: Vec<&str> = s.split_whitespace().collect();
            let sx = parts
                .first()
                .and_then(|p| p.parse::<f32>().ok())
                .unwrap_or(1.0);
            let sy = parts
                .get(1)
                .and_then(|p| p.parse::<f32>().ok())
                .unwrap_or(sx);
            if sx != 1.0 || sy != 1.0 {
                style.transform.push(Transform::Scale(sx, sy));
            }
        }

        // ── Border-style heuristic (TODO 17) ──
        // Themes commonly set border width/color without an explicit
        // `border-style`; CSS default `none` would make them invisible. Promote
        // any side with a positive width but still-default `none` to `solid` so
        // authored borders paint.
        Self::default_border_style_for_width(style);
    }

    /// Promote `border-style: none` to `solid` on any side with a positive
    /// border width, so width-only theme borders remain visible. (TODO 17)
    fn default_border_style_for_width(style: &mut ComputedStyle) {
        let promote = |w: f32, s: &mut BorderLineStyle| {
            if w > 0.0 && *s == BorderLineStyle::None {
                *s = BorderLineStyle::Solid;
            }
        };
        promote(style.border_width.top, &mut style.border_style.top);
        promote(style.border_width.right, &mut style.border_style.right);
        promote(style.border_width.bottom, &mut style.border_style.bottom);
        promote(style.border_width.left, &mut style.border_style.left);
    }
}
