//! SVG path painting — parse SVG `d` path data and emit display items.
//!
//! Supports a subset of SVG path commands:
//! - M/m (moveto), L/l (lineto), H/h (horizontal), V/v (vertical)
//! - C/c (cubic bezier), S/s (smooth cubic)
//! - Q/q (quadratic bezier), T/t (smooth quadratic)
//! - A/a (arc), Z/z (close)
//!
//! Paths are flattened into line segments for painting via `DisplayItem::Line`.

use liquide_compositor::pixel::Color;
use liquide_layout::Rect;

use crate::display_list::{DisplayItem, DisplayList};

/// A parsed SVG path command.
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    HLineTo { x: f32 },
    VLineTo { y: f32 },
    CubicTo { x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32 },
    SmoothCubicTo { x2: f32, y2: f32, x: f32, y: f32 },
    QuadTo { x1: f32, y1: f32, x: f32, y: f32 },
    SmoothQuadTo { x: f32, y: f32 },
    ArcTo { rx: f32, ry: f32, rotation: f32, large_arc: bool, sweep: bool, x: f32, y: f32 },
    Close,
}

/// A flattened path segment (line).
#[derive(Debug, Clone, Copy)]
pub struct PathSegment {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// Parse an SVG `d` attribute into path commands.
pub fn parse_svg_path(d: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut chars = d.chars().peekable();
    let mut current_cmd = ' ';
    let mut relative = false;

    fn skip_ws_comma(chars: &mut std::iter::Peekable<std::str::Chars>) {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
    }

    fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<f32> {
        skip_ws_comma(chars);
        let mut s = String::new();
        // Handle sign
        if let Some(&c) = chars.peek() {
            if c == '-' || c == '+' {
                s.push(c);
                chars.next();
            }
        }
        let mut has_dot = false;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                chars.next();
            } else if c == '.' && !has_dot {
                has_dot = true;
                s.push(c);
                chars.next();
            } else if c == 'e' || c == 'E' {
                s.push(c);
                chars.next();
                if let Some(&sign) = chars.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        chars.next();
                    }
                }
            } else {
                break;
            }
        }
        if s.is_empty() { None } else { s.parse().ok() }
    }

    fn parse_flag(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<bool> {
        skip_ws_comma(chars);
        match chars.peek() {
            Some('0') => { chars.next(); Some(false) }
            Some('1') => { chars.next(); Some(true) }
            _ => None,
        }
    }

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
            continue;
        }

        if c.is_ascii_alphabetic() {
            current_cmd = c;
            relative = c.is_ascii_lowercase();
            chars.next();
        }

        match current_cmd.to_ascii_uppercase() {
            'M' => {
                if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                    commands.push(PathCommand::MoveTo { x, y });
                    // Subsequent coords are implicit LineTo
                    current_cmd = if relative { 'l' } else { 'L' };
                }
            }
            'L' => {
                if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                    commands.push(PathCommand::LineTo { x, y });
                }
            }
            'H' => {
                if let Some(x) = parse_number(&mut chars) {
                    commands.push(PathCommand::HLineTo { x });
                }
            }
            'V' => {
                if let Some(y) = parse_number(&mut chars) {
                    commands.push(PathCommand::VLineTo { y });
                }
            }
            'C' => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    parse_number(&mut chars), parse_number(&mut chars),
                    parse_number(&mut chars), parse_number(&mut chars),
                    parse_number(&mut chars), parse_number(&mut chars),
                ) {
                    commands.push(PathCommand::CubicTo { x1, y1, x2, y2, x, y });
                }
            }
            'S' => {
                if let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    parse_number(&mut chars), parse_number(&mut chars),
                    parse_number(&mut chars), parse_number(&mut chars),
                ) {
                    commands.push(PathCommand::SmoothCubicTo { x2, y2, x, y });
                }
            }
            'Q' => {
                if let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    parse_number(&mut chars), parse_number(&mut chars),
                    parse_number(&mut chars), parse_number(&mut chars),
                ) {
                    commands.push(PathCommand::QuadTo { x1, y1, x, y });
                }
            }
            'T' => {
                if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                    commands.push(PathCommand::SmoothQuadTo { x, y });
                }
            }
            'A' => {
                if let (Some(rx), Some(ry), Some(rot)) = (
                    parse_number(&mut chars), parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    if let (Some(la), Some(sf)) = (parse_flag(&mut chars), parse_flag(&mut chars)) {
                        if let (Some(x), Some(y)) = (parse_number(&mut chars), parse_number(&mut chars)) {
                            commands.push(PathCommand::ArcTo {
                                rx, ry, rotation: rot, large_arc: la, sweep: sf, x, y,
                            });
                        }
                    }
                }
            }
            'Z' => {
                commands.push(PathCommand::Close);
            }
            _ => {
                chars.next(); // Skip unknown
            }
        }
    }

    commands
}

/// Flatten SVG path commands into line segments.
///
/// Curves are approximated by subdivision into short line segments.
pub fn flatten_path(commands: &[PathCommand]) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_cp_x = 0.0f32; // For smooth curves
    let mut last_cp_y = 0.0f32;

    for cmd in commands {
        match *cmd {
            PathCommand::MoveTo { x, y } => {
                cx = x; cy = y;
                start_x = x; start_y = y;
                last_cp_x = x; last_cp_y = y;
            }
            PathCommand::LineTo { x, y } => {
                segments.push(PathSegment { x1: cx, y1: cy, x2: x, y2: y });
                cx = x; cy = y;
                last_cp_x = x; last_cp_y = y;
            }
            PathCommand::HLineTo { x } => {
                segments.push(PathSegment { x1: cx, y1: cy, x2: x, y2: cy });
                cx = x;
                last_cp_x = cx; last_cp_y = cy;
            }
            PathCommand::VLineTo { y } => {
                segments.push(PathSegment { x1: cx, y1: cy, x2: cx, y2: y });
                cy = y;
                last_cp_x = cx; last_cp_y = cy;
            }
            PathCommand::CubicTo { x1, y1, x2, y2, x, y } => {
                flatten_cubic(&mut segments, cx, cy, x1, y1, x2, y2, x, y);
                last_cp_x = x2; last_cp_y = y2;
                cx = x; cy = y;
            }
            PathCommand::SmoothCubicTo { x2, y2, x, y } => {
                // Reflect previous control point
                let rx1 = 2.0 * cx - last_cp_x;
                let ry1 = 2.0 * cy - last_cp_y;
                flatten_cubic(&mut segments, cx, cy, rx1, ry1, x2, y2, x, y);
                last_cp_x = x2; last_cp_y = y2;
                cx = x; cy = y;
            }
            PathCommand::QuadTo { x1, y1, x, y } => {
                flatten_quadratic(&mut segments, cx, cy, x1, y1, x, y);
                last_cp_x = x1; last_cp_y = y1;
                cx = x; cy = y;
            }
            PathCommand::SmoothQuadTo { x, y } => {
                let rx1 = 2.0 * cx - last_cp_x;
                let ry1 = 2.0 * cy - last_cp_y;
                flatten_quadratic(&mut segments, cx, cy, rx1, ry1, x, y);
                last_cp_x = rx1; last_cp_y = ry1;
                cx = x; cy = y;
            }
            PathCommand::ArcTo { rx, ry, x, y, .. } => {
                // Simplified arc: approximate with line for now.
                // A full implementation would convert to cubic beziers.
                if rx.abs() < 0.01 || ry.abs() < 0.01 {
                    segments.push(PathSegment { x1: cx, y1: cy, x2: x, y2: y });
                } else {
                    // Approximate arc with 8 line segments
                    let steps = 8;
                    for i in 0..steps {
                        let t0 = i as f32 / steps as f32;
                        let t1 = (i + 1) as f32 / steps as f32;
                        let ax = cx + (x - cx) * t0;
                        let ay = cy + (y - cy) * t0;
                        let bx = cx + (x - cx) * t1;
                        let by = cy + (y - cy) * t1;
                        segments.push(PathSegment { x1: ax, y1: ay, x2: bx, y2: by });
                    }
                }
                cx = x; cy = y;
                last_cp_x = x; last_cp_y = y;
            }
            PathCommand::Close => {
                if (cx - start_x).abs() > 0.01 || (cy - start_y).abs() > 0.01 {
                    segments.push(PathSegment { x1: cx, y1: cy, x2: start_x, y2: start_y });
                }
                cx = start_x; cy = start_y;
                last_cp_x = cx; last_cp_y = cy;
            }
        }
    }

    segments
}

/// Flatten a cubic bezier into line segments by recursive subdivision.
fn flatten_cubic(
    out: &mut Vec<PathSegment>,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
    x2: f32, y2: f32,
    x3: f32, y3: f32,
) {
    // Check if the curve is flat enough.
    let dx = x3 - x0;
    let dy = y3 - y0;
    let d2 = ((x1 - x3) * dy - (y1 - y3) * dx).abs()
           + ((x2 - x3) * dy - (y2 - y3) * dx).abs();
    let chord_sq = dx * dx + dy * dy;

    if d2 * d2 <= 0.25 * chord_sq {
        out.push(PathSegment { x1: x0, y1: y0, x2: x3, y2: y3 });
        return;
    }

    // Subdivide at t=0.5 (de Casteljau).
    let m01x = (x0 + x1) * 0.5;
    let m01y = (y0 + y1) * 0.5;
    let m12x = (x1 + x2) * 0.5;
    let m12y = (y1 + y2) * 0.5;
    let m23x = (x2 + x3) * 0.5;
    let m23y = (y2 + y3) * 0.5;
    let m012x = (m01x + m12x) * 0.5;
    let m012y = (m01y + m12y) * 0.5;
    let m123x = (m12x + m23x) * 0.5;
    let m123y = (m12y + m23y) * 0.5;
    let mx = (m012x + m123x) * 0.5;
    let my = (m012y + m123y) * 0.5;

    flatten_cubic(out, x0, y0, m01x, m01y, m012x, m012y, mx, my);
    flatten_cubic(out, mx, my, m123x, m123y, m23x, m23y, x3, y3);
}

/// Flatten a quadratic bezier into line segments.
fn flatten_quadratic(
    out: &mut Vec<PathSegment>,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
    x2: f32, y2: f32,
) {
    // Convert quadratic to cubic and flatten.
    let cx1 = x0 + (x1 - x0) * 2.0 / 3.0;
    let cy1 = y0 + (y1 - y0) * 2.0 / 3.0;
    let cx2 = x2 + (x1 - x2) * 2.0 / 3.0;
    let cy2 = y2 + (y1 - y2) * 2.0 / 3.0;
    flatten_cubic(out, x0, y0, cx1, cy1, cx2, cy2, x2, y2);
}

/// Paint an SVG path into a display list as a series of line segments.
pub fn paint_svg_path(
    dl: &mut DisplayList,
    d: &str,
    offset_x: f32,
    offset_y: f32,
    stroke_color: Color,
    stroke_width: f32,
    fill_color: Option<Color>,
) {
    let commands = parse_svg_path(d);
    let segments = flatten_path(&commands);

    // Emit fill as a solid color behind the path bounds (approximate).
    if let Some(fill) = fill_color {
        if fill.a > 0 && !segments.is_empty() {
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for seg in &segments {
                min_x = min_x.min(seg.x1).min(seg.x2);
                min_y = min_y.min(seg.y1).min(seg.y2);
                max_x = max_x.max(seg.x1).max(seg.x2);
                max_y = max_y.max(seg.y1).max(seg.y2);
            }
            dl.push(DisplayItem::FillRect {
                rect: Rect::new(
                    offset_x + min_x,
                    offset_y + min_y,
                    max_x - min_x,
                    max_y - min_y,
                ),
                color: fill,
            });
        }
    }

    // Emit stroke lines
    if stroke_color.a > 0 && stroke_width > 0.0 {
        for seg in &segments {
            dl.push(DisplayItem::Line {
                x1: offset_x + seg.x1,
                y1: offset_y + seg.y1,
                x2: offset_x + seg.x2,
                y2: offset_y + seg.y2,
                color: stroke_color,
                width: stroke_width,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_path() {
        let cmds = parse_svg_path("M 10 20 L 30 40 Z");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], PathCommand::MoveTo { x, y } if (x - 10.0).abs() < 0.01 && (y - 20.0).abs() < 0.01));
        assert!(matches!(cmds[1], PathCommand::LineTo { .. }));
        assert!(matches!(cmds[2], PathCommand::Close));
    }

    #[test]
    fn parse_cubic_path() {
        let cmds = parse_svg_path("M 0 0 C 10 20 30 40 50 60");
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[1], PathCommand::CubicTo { .. }));
    }

    #[test]
    fn flatten_triangle() {
        let cmds = parse_svg_path("M 0 0 L 100 0 L 50 100 Z");
        let segs = flatten_path(&cmds);
        assert_eq!(segs.len(), 3); // Three sides of the triangle
    }

    #[test]
    fn flatten_cubic_curve() {
        let cmds = parse_svg_path("M 0 0 C 0 100 100 100 100 0");
        let segs = flatten_path(&cmds);
        assert!(segs.len() > 1); // Should subdivide into multiple lines
    }

    #[test]
    fn paint_path_emits_lines() {
        let mut dl = DisplayList::new();
        paint_svg_path(
            &mut dl,
            "M 0 0 L 100 0 L 100 100 Z",
            0.0, 0.0,
            Color { r: 0, g: 0, b: 0, a: 255 },
            1.0,
            None,
        );
        assert!(dl.len() >= 3);
    }
}
