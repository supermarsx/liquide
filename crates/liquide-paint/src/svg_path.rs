//! SVG path painting — parse SVG `d` path data and emit display items.
//!
//! Supports a subset of SVG path commands:
//! - M/m (moveto), L/l (lineto), H/h (horizontal), V/v (vertical)
//! - C/c (cubic bezier), S/s (smooth cubic)
//! - Q/q (quadratic bezier), T/t (smooth quadratic)
//! - A/a (arc), Z/z (close)
//!
//! Paths are flattened into line segments for painting via `DisplayItem::Line`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use liquide_compositor::pixel::Color;
use liquide_layout::Rect;

use crate::display_list::{DisplayItem, DisplayList};

// Thread-local cache for flattened SVG paths, keyed by hash of the `d` attribute string.
// Stores (original_string, segments) to detect hash collisions (CR-11).
thread_local! {
    static PATH_CACHE: RefCell<HashMap<u64, (String, Vec<PathSegment>)>> = RefCell::new(HashMap::new());
}

fn hash_path_string(d: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    d.hash(&mut hasher);
    hasher.finish()
}

/// A parsed SVG path command.
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    HLineTo {
        x: f32,
    },
    VLineTo {
        y: f32,
    },
    CubicTo {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x: f32,
        y: f32,
    },
    SmoothCubicTo {
        x2: f32,
        y2: f32,
        x: f32,
        y: f32,
    },
    QuadTo {
        x1: f32,
        y1: f32,
        x: f32,
        y: f32,
    },
    SmoothQuadTo {
        x: f32,
        y: f32,
    },
    ArcTo {
        rx: f32,
        ry: f32,
        rotation: f32,
        large_arc: bool,
        sweep: bool,
        x: f32,
        y: f32,
    },
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
///
/// Relative commands (lowercase letters) are converted to absolute
/// coordinates during parsing, so all emitted `PathCommand` values
/// use absolute coordinates.
pub fn parse_svg_path(d: &str) -> Vec<PathCommand> {
    let mut commands = Vec::new();
    let mut chars = d.chars().peekable();
    let mut current_cmd = ' ';
    let mut relative = false;
    // Current pen position (for converting relative → absolute).
    let mut cx = 0.0_f32;
    let mut cy = 0.0_f32;
    // Start of current sub-path (set by MoveTo, used by Close).
    let mut sx = 0.0_f32;
    let mut sy = 0.0_f32;

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
            Some('0') => {
                chars.next();
                Some(false)
            }
            Some('1') => {
                chars.next();
                Some(true)
            }
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
                if let (Some(mut x), Some(mut y)) =
                    (parse_number(&mut chars), parse_number(&mut chars))
                {
                    if relative {
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    sx = x;
                    sy = y;
                    commands.push(PathCommand::MoveTo { x, y });
                    // Subsequent coords are implicit LineTo
                    current_cmd = if relative { 'l' } else { 'L' };
                }
            }
            'L' => {
                if let (Some(mut x), Some(mut y)) =
                    (parse_number(&mut chars), parse_number(&mut chars))
                {
                    if relative {
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    commands.push(PathCommand::LineTo { x, y });
                }
            }
            'H' => {
                if let Some(mut x) = parse_number(&mut chars) {
                    if relative {
                        x += cx;
                    }
                    cx = x;
                    commands.push(PathCommand::HLineTo { x });
                }
            }
            'V' => {
                if let Some(mut y) = parse_number(&mut chars) {
                    if relative {
                        y += cy;
                    }
                    cy = y;
                    commands.push(PathCommand::VLineTo { y });
                }
            }
            'C' => {
                if let (
                    Some(mut x1),
                    Some(mut y1),
                    Some(mut x2),
                    Some(mut y2),
                    Some(mut x),
                    Some(mut y),
                ) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    if relative {
                        x1 += cx;
                        y1 += cy;
                        x2 += cx;
                        y2 += cy;
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    commands.push(PathCommand::CubicTo {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    });
                }
            }
            'S' => {
                if let (Some(mut x2), Some(mut y2), Some(mut x), Some(mut y)) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    if relative {
                        x2 += cx;
                        y2 += cy;
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    commands.push(PathCommand::SmoothCubicTo { x2, y2, x, y });
                }
            }
            'Q' => {
                if let (Some(mut x1), Some(mut y1), Some(mut x), Some(mut y)) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    if relative {
                        x1 += cx;
                        y1 += cy;
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    commands.push(PathCommand::QuadTo { x1, y1, x, y });
                }
            }
            'T' => {
                if let (Some(mut x), Some(mut y)) =
                    (parse_number(&mut chars), parse_number(&mut chars))
                {
                    if relative {
                        x += cx;
                        y += cy;
                    }
                    cx = x;
                    cy = y;
                    commands.push(PathCommand::SmoothQuadTo { x, y });
                }
            }
            'A' => {
                if let (Some(rx), Some(ry), Some(rot)) = (
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                    parse_number(&mut chars),
                ) {
                    if let (Some(la), Some(sf)) = (parse_flag(&mut chars), parse_flag(&mut chars)) {
                        if let (Some(mut x), Some(mut y)) =
                            (parse_number(&mut chars), parse_number(&mut chars))
                        {
                            if relative {
                                x += cx;
                                y += cy;
                            }
                            cx = x;
                            cy = y;
                            commands.push(PathCommand::ArcTo {
                                rx,
                                ry,
                                rotation: rot,
                                large_arc: la,
                                sweep: sf,
                                x,
                                y,
                            });
                        }
                    }
                }
            }
            'Z' => {
                cx = sx;
                cy = sy;
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
                cx = x;
                cy = y;
                start_x = x;
                start_y = y;
                last_cp_x = x;
                last_cp_y = y;
            }
            PathCommand::LineTo { x, y } => {
                segments.push(PathSegment {
                    x1: cx,
                    y1: cy,
                    x2: x,
                    y2: y,
                });
                cx = x;
                cy = y;
                last_cp_x = x;
                last_cp_y = y;
            }
            PathCommand::HLineTo { x } => {
                segments.push(PathSegment {
                    x1: cx,
                    y1: cy,
                    x2: x,
                    y2: cy,
                });
                cx = x;
                last_cp_x = cx;
                last_cp_y = cy;
            }
            PathCommand::VLineTo { y } => {
                segments.push(PathSegment {
                    x1: cx,
                    y1: cy,
                    x2: cx,
                    y2: y,
                });
                cy = y;
                last_cp_x = cx;
                last_cp_y = cy;
            }
            PathCommand::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                flatten_cubic(&mut segments, cx, cy, x1, y1, x2, y2, x, y, 0);
                last_cp_x = x2;
                last_cp_y = y2;
                cx = x;
                cy = y;
            }
            PathCommand::SmoothCubicTo { x2, y2, x, y } => {
                // Reflect previous control point
                let rx1 = 2.0 * cx - last_cp_x;
                let ry1 = 2.0 * cy - last_cp_y;
                flatten_cubic(&mut segments, cx, cy, rx1, ry1, x2, y2, x, y, 0);
                last_cp_x = x2;
                last_cp_y = y2;
                cx = x;
                cy = y;
            }
            PathCommand::QuadTo { x1, y1, x, y } => {
                flatten_quadratic(&mut segments, cx, cy, x1, y1, x, y);
                last_cp_x = x1;
                last_cp_y = y1;
                cx = x;
                cy = y;
            }
            PathCommand::SmoothQuadTo { x, y } => {
                let rx1 = 2.0 * cx - last_cp_x;
                let ry1 = 2.0 * cy - last_cp_y;
                flatten_quadratic(&mut segments, cx, cy, rx1, ry1, x, y);
                last_cp_x = rx1;
                last_cp_y = ry1;
                cx = x;
                cy = y;
            }
            PathCommand::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                // Degenerate arc: zero radii → straight line.
                if rx.abs() < 1e-6 || ry.abs() < 1e-6 {
                    segments.push(PathSegment {
                        x1: cx,
                        y1: cy,
                        x2: x,
                        y2: y,
                    });
                } else if (cx - x).abs() < 1e-6 && (cy - y).abs() < 1e-6 {
                    // Start == end → nothing to draw.
                } else {
                    arc_to_cubic_beziers(
                        &mut segments,
                        cx,
                        cy,
                        rx,
                        ry,
                        rotation,
                        large_arc,
                        sweep,
                        x,
                        y,
                    );
                }
                cx = x;
                cy = y;
                last_cp_x = x;
                last_cp_y = y;
            }
            PathCommand::Close => {
                if (cx - start_x).abs() > 0.01 || (cy - start_y).abs() > 0.01 {
                    segments.push(PathSegment {
                        x1: cx,
                        y1: cy,
                        x2: start_x,
                        y2: start_y,
                    });
                }
                cx = start_x;
                cy = start_y;
                last_cp_x = cx;
                last_cp_y = cy;
            }
        }
    }

    segments
}

/// Flatten SVG path commands with caching.
///
/// Looks up the path string in a thread-local cache to avoid re-parsing
/// and re-flattening identical paths every frame. The cache is bounded
/// at 1024 entries and cleared when full.
pub fn flatten_path_cached(d: &str) -> Vec<PathSegment> {
    let key = hash_path_string(d);

    // Check cache first, verifying the original string matches to detect hash collisions.
    let cached = PATH_CACHE.with(|cache| {
        cache.borrow().get(&key).and_then(|(original, segments)| {
            if original == d {
                Some(segments.clone())
            } else {
                None
            }
        })
    });

    if let Some(segments) = cached {
        return segments;
    }

    // Cache miss — parse and flatten.
    let commands = parse_svg_path(d);
    let segments = flatten_path(&commands);

    PATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 2048 {
            cache.clear();
        }
        cache.insert(key, (d.to_string(), segments.clone()));
    });

    segments
}

/// Flatten a cubic bezier into line segments by recursive subdivision.
fn flatten_cubic(
    out: &mut Vec<PathSegment>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    depth: u32,
) {
    // Guard against infinite recursion and NaN propagation. We must check
    // *all* coordinates: NaN in any control point taints the flatness
    // estimator (`d2`), and NaN comparisons return `false`, so the curve
    // would never be considered flat and we'd recurse forever.
    if depth >= 16
        || x0.is_nan()
        || y0.is_nan()
        || x1.is_nan()
        || y1.is_nan()
        || x2.is_nan()
        || y2.is_nan()
        || x3.is_nan()
        || y3.is_nan()
    {
        out.push(PathSegment {
            x1: x0,
            y1: y0,
            x2: x3,
            y2: y3,
        });
        return;
    }

    // Check if the curve is flat enough.
    let dx = x3 - x0;
    let dy = y3 - y0;
    let d2 = ((x1 - x3) * dy - (y1 - y3) * dx).abs() + ((x2 - x3) * dy - (y2 - y3) * dx).abs();
    let chord_sq = dx * dx + dy * dy;

    if d2 * d2 <= 0.25 * chord_sq {
        out.push(PathSegment {
            x1: x0,
            y1: y0,
            x2: x3,
            y2: y3,
        });
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

    flatten_cubic(out, x0, y0, m01x, m01y, m012x, m012y, mx, my, depth + 1);
    flatten_cubic(out, mx, my, m123x, m123y, m23x, m23y, x3, y3, depth + 1);
}

/// Convert an SVG arc (endpoint parameterization) to cubic Bézier curves.
///
/// Implements the SVG spec endpoint-to-center parameterization and then
/// approximates each arc segment (≤ π/2) with a cubic Bézier.
fn arc_to_cubic_beziers(
    out: &mut Vec<PathSegment>,
    x1: f32,
    y1: f32,
    rx: f32,
    ry: f32,
    x_rotation_deg: f32,
    large_arc: bool,
    sweep: bool,
    x2: f32,
    y2: f32,
) {
    use std::f32::consts::PI;

    let phi = x_rotation_deg.to_radians();
    let cos_phi = phi.cos();
    let sin_phi = phi.sin();

    // Step 1: Compute (x1', y1') — rotated midpoint
    let dx2 = (x1 - x2) / 2.0;
    let dy2 = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;

    // Step 2: Correct out-of-range radii
    let mut rx = rx.abs();
    let mut ry = ry.abs();
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let sqrt_lambda = lambda.sqrt();
        rx *= sqrt_lambda;
        ry *= sqrt_lambda;
    }

    // Step 3: Compute center point (cx', cy')
    let rx_sq = rx * rx;
    let ry_sq = ry * ry;
    let x1p_sq = x1p * x1p;
    let y1p_sq = y1p * y1p;

    let num = (rx_sq * ry_sq - rx_sq * y1p_sq - ry_sq * x1p_sq).max(0.0);
    let den = rx_sq * y1p_sq + ry_sq * x1p_sq;
    let sq = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };

    let cxp = sign * sq * (rx * y1p / ry);
    let cyp = sign * sq * -(ry * x1p / rx);

    // Step 4: Compute center in original coordinates
    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;

    // Step 5: Compute θ1 and Δθ
    fn angle(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
        let n = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        if n < 1e-10 {
            return 0.0;
        }
        let cos_a = ((ux * vx + uy * vy) / n).clamp(-1.0, 1.0);
        let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
        sign * cos_a.acos()
    }

    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut d_theta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    if !sweep && d_theta > 0.0 {
        d_theta -= 2.0 * PI;
    } else if sweep && d_theta < 0.0 {
        d_theta += 2.0 * PI;
    }

    // Step 6: Split into segments of ≤ π/2 and approximate each with a cubic Bézier
    let n_segs = (d_theta.abs() / (PI / 2.0)).ceil().max(1.0) as u32;
    let seg_angle = d_theta / n_segs as f32;

    let mut prev_x = x1;
    let mut prev_y = y1;

    for i in 0..n_segs {
        let t1 = theta1 + seg_angle * i as f32;
        let t2 = theta1 + seg_angle * (i + 1) as f32;

        // Control point factor: α = sin(Δ) * (√(4 + 3 * tan²(Δ/2)) - 1) / 3
        let half = seg_angle / 2.0;
        let alpha = half.sin() * ((4.0 + 3.0 * (half.tan() * half.tan())).sqrt() - 1.0) / 3.0;

        // Endpoint on the unit-radius ellipse
        let cos_t1 = t1.cos();
        let sin_t1 = t1.sin();
        let cos_t2 = t2.cos();
        let sin_t2 = t2.sin();

        // Control point 1 (on unit ellipse, then transform)
        let ep1x = rx * cos_t1;
        let ep1y = ry * sin_t1;
        let ep2x = rx * cos_t2;
        let ep2y = ry * sin_t2;

        // Tangent directions
        let d1x = -rx * sin_t1;
        let d1y = ry * cos_t1;
        let d2x = -rx * sin_t2;
        let d2y = ry * cos_t2;

        // Control points in ellipse-local space
        let cp1x = ep1x + alpha * d1x;
        let cp1y = ep1y + alpha * d1y;
        let cp2x = ep2x - alpha * d2x;
        let cp2y = ep2y - alpha * d2y;

        // Transform to world coordinates (rotate + translate)
        let q1x = cos_phi * cp1x - sin_phi * cp1y + cx;
        let q1y = sin_phi * cp1x + cos_phi * cp1y + cy;
        let q2x = cos_phi * cp2x - sin_phi * cp2y + cx;
        let q2y = sin_phi * cp2x + cos_phi * cp2y + cy;
        let end_x = cos_phi * ep2x - sin_phi * ep2y + cx;
        let end_y = sin_phi * ep2x + cos_phi * ep2y + cy;

        flatten_cubic(out, prev_x, prev_y, q1x, q1y, q2x, q2y, end_x, end_y, 0);
        prev_x = end_x;
        prev_y = end_y;
    }
}

/// Flatten a quadratic bezier into line segments.
fn flatten_quadratic(
    out: &mut Vec<PathSegment>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
) {
    // Convert quadratic to cubic and flatten.
    let cx1 = x0 + (x1 - x0) * 2.0 / 3.0;
    let cy1 = y0 + (y1 - y0) * 2.0 / 3.0;
    let cx2 = x2 + (x1 - x2) * 2.0 / 3.0;
    let cy2 = y2 + (y1 - y2) * 2.0 / 3.0;
    flatten_cubic(out, x0, y0, cx1, cy1, cx2, cy2, x2, y2, 0);
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
    let segments = flatten_path_cached(d);

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
        assert!(
            matches!(cmds[0], PathCommand::MoveTo { x, y } if (x - 10.0).abs() < 0.01 && (y - 20.0).abs() < 0.01)
        );
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
            0.0,
            0.0,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            1.0,
            None,
        );
        assert!(dl.len() >= 3);
    }

    #[test]
    fn flatten_path_cached_returns_same_result() {
        let d = "M 0 0 L 100 0 L 50 100 Z";
        let cmds = parse_svg_path(d);
        let expected = flatten_path(&cmds);
        let cached = flatten_path_cached(d);
        assert_eq!(expected.len(), cached.len());
        for (a, b) in expected.iter().zip(cached.iter()) {
            assert!((a.x1 - b.x1).abs() < 0.001);
            assert!((a.y1 - b.y1).abs() < 0.001);
            assert!((a.x2 - b.x2).abs() < 0.001);
            assert!((a.y2 - b.y2).abs() < 0.001);
        }
    }

    #[test]
    fn flatten_path_cached_hit() {
        let d = "M 10 10 L 90 10 L 90 90 Z";
        let first = flatten_path_cached(d);
        let second = flatten_path_cached(d);
        assert_eq!(first.len(), second.len());
    }

    #[test]
    fn flatten_cubic_depth_limit_prevents_stack_overflow() {
        // Craft a curve with NaN that would recurse infinitely without the depth guard.
        let mut out = Vec::new();
        flatten_cubic(
            &mut out,
            0.0,
            0.0,
            f32::NAN,
            f32::NAN,
            100.0,
            100.0,
            200.0,
            200.0,
            0,
        );
        // Should produce a single line segment (bailout at depth 0 due to NaN).
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn flatten_cubic_deep_subdivision_terminates() {
        // A pathological curve that causes many subdivisions.
        let mut out = Vec::new();
        flatten_cubic(&mut out, 0.0, 0.0, 1e6, -1e6, -1e6, 1e6, 1.0, 1.0, 0);
        // Must terminate with at most 2^16 segments.
        assert!(out.len() <= 65536);
    }

    #[test]
    fn arc_to_cubic_semicircle() {
        // A semicircular arc: M 0 0 A 50 50 0 0 1 100 0
        let cmds = parse_svg_path("M 0 0 A 50 50 0 0 1 100 0");
        let segs = flatten_path(&cmds);
        // Should produce curved segments (more than the old 8-line-segment hack)
        assert!(segs.len() > 1);
        // First segment starts near origin, last ends near (100, 0)
        let first = &segs[0];
        let last = &segs[segs.len() - 1];
        assert!((first.x1).abs() < 0.1);
        assert!((first.y1).abs() < 0.1);
        assert!((last.x2 - 100.0).abs() < 1.0);
        assert!((last.y2).abs() < 1.0);
        // The arc should bulge upward or downward — check that some point is not on the line
        let has_curvature = segs.iter().any(|s| s.y1.abs() > 1.0 || s.y2.abs() > 1.0);
        assert!(
            has_curvature,
            "Arc segments should show curvature, not straight lines"
        );
    }

    #[test]
    fn arc_degenerate_zero_radii() {
        let cmds = parse_svg_path("M 0 0 A 0 0 0 0 1 100 100");
        let segs = flatten_path(&cmds);
        // Zero radii → single line segment to endpoint.
        assert_eq!(segs.len(), 1);
        assert!((segs[0].x2 - 100.0).abs() < 0.01);
        assert!((segs[0].y2 - 100.0).abs() < 0.01);
    }

    #[test]
    fn cache_collision_different_paths_returns_correct() {
        // Two different paths should return different segment counts.
        let d1 = "M 0 0 L 100 0 Z";
        let d2 = "M 0 0 L 100 0 L 100 100 L 0 100 Z";
        let s1 = flatten_path_cached(d1);
        let s2 = flatten_path_cached(d2);
        // d1 has 2 segments (line + close), d2 has 4 segments
        assert_ne!(s1.len(), s2.len());
    }

    // ── Parse M L Z basic ──

    #[test]
    fn parse_move_line_close() {
        let cmds = parse_svg_path("M 0 0 L 10 10");
        assert_eq!(cmds.len(), 2);
        match cmds[0] {
            PathCommand::MoveTo { x, y } => {
                assert_eq!(x, 0.0);
                assert_eq!(y, 0.0);
            }
            _ => panic!("expected MoveTo"),
        }
        match cmds[1] {
            PathCommand::LineTo { x, y } => {
                assert_eq!(x, 10.0);
                assert_eq!(y, 10.0);
            }
            _ => panic!("expected LineTo"),
        }
    }

    // ── Parse arc commands ──

    #[test]
    fn parse_arc_command() {
        let cmds = parse_svg_path("M 10 80 A 25 25 0 0 1 50 80");
        assert_eq!(cmds.len(), 2);
        match cmds[1] {
            PathCommand::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                assert_eq!(rx, 25.0);
                assert_eq!(ry, 25.0);
                assert_eq!(rotation, 0.0);
                assert!(!large_arc);
                assert!(sweep);
                assert_eq!(x, 50.0);
                assert_eq!(y, 80.0);
            }
            _ => panic!("expected ArcTo"),
        }
    }

    // ── Parse relative commands ──

    #[test]
    fn parse_relative_line() {
        let cmds = parse_svg_path("M 10 20 l 5 5");
        assert_eq!(cmds.len(), 2);
        match cmds[1] {
            PathCommand::LineTo { x, y } => {
                // Relative: 10+5=15, 20+5=25
                assert!((x - 15.0).abs() < 0.01);
                assert!((y - 25.0).abs() < 0.01);
            }
            _ => panic!("expected LineTo"),
        }
    }

    #[test]
    fn parse_relative_move() {
        let cmds = parse_svg_path("M 10 20 m 5 5 L 30 30");
        assert_eq!(cmds.len(), 3);
        match cmds[1] {
            PathCommand::MoveTo { x, y } => {
                assert!((x - 15.0).abs() < 0.01);
                assert!((y - 25.0).abs() < 0.01);
            }
            _ => panic!("expected MoveTo"),
        }
    }

    #[test]
    fn parse_relative_horizontal_vertical() {
        let cmds = parse_svg_path("M 10 20 h 30 v 40");
        assert_eq!(cmds.len(), 3);
        match cmds[1] {
            PathCommand::HLineTo { x } => assert!((x - 40.0).abs() < 0.01),
            _ => panic!("expected HLineTo"),
        }
        match cmds[2] {
            PathCommand::VLineTo { y } => assert!((y - 60.0).abs() < 0.01),
            _ => panic!("expected VLineTo"),
        }
    }

    // ── Parse malformed/empty paths ──

    #[test]
    fn parse_empty_path() {
        let cmds = parse_svg_path("");
        assert!(cmds.is_empty());
    }

    #[test]
    fn parse_malformed_path_partial() {
        // Missing second coordinate — parser should handle gracefully
        let cmds = parse_svg_path("M 10");
        // No valid command emitted since y is missing
        assert!(cmds.is_empty());
    }

    #[test]
    fn parse_whitespace_only_path() {
        let cmds = parse_svg_path("   ");
        assert!(cmds.is_empty());
    }

    #[test]
    fn parse_unknown_commands_skipped() {
        let cmds = parse_svg_path("M 0 0 X 5 5 L 10 10");
        // M and L should parse, X is unknown and skipped
        assert_eq!(cmds.len(), 2);
    }

    // ── Parse smooth curves ──

    #[test]
    fn parse_smooth_cubic() {
        let cmds = parse_svg_path("M 0 0 C 10 20 30 40 50 60 S 80 90 100 110");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[2], PathCommand::SmoothCubicTo { .. }));
    }

    #[test]
    fn parse_quadratic() {
        let cmds = parse_svg_path("M 0 0 Q 25 50 50 0");
        assert_eq!(cmds.len(), 2);
        match cmds[1] {
            PathCommand::QuadTo { x1, y1, x, y } => {
                assert_eq!(x1, 25.0);
                assert_eq!(y1, 50.0);
                assert_eq!(x, 50.0);
                assert_eq!(y, 0.0);
            }
            _ => panic!("expected QuadTo"),
        }
    }

    #[test]
    fn parse_smooth_quad() {
        let cmds = parse_svg_path("M 0 0 Q 25 50 50 0 T 100 0");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[2], PathCommand::SmoothQuadTo { .. }));
    }

    // ── Flatten edge cases ──

    #[test]
    fn flatten_empty_commands() {
        let segs = flatten_path(&[]);
        assert!(segs.is_empty());
    }

    #[test]
    fn flatten_move_only() {
        let cmds = parse_svg_path("M 10 20");
        let segs = flatten_path(&cmds);
        assert!(segs.is_empty()); // MoveTo alone produces no segments
    }

    #[test]
    fn flatten_horizontal_line() {
        let cmds = parse_svg_path("M 0 0 H 100");
        let segs = flatten_path(&cmds);
        assert_eq!(segs.len(), 1);
        assert!((segs[0].x1 - 0.0).abs() < 0.01);
        assert!((segs[0].y1 - 0.0).abs() < 0.01);
        assert!((segs[0].x2 - 100.0).abs() < 0.01);
        assert!((segs[0].y2 - 0.0).abs() < 0.01);
    }

    #[test]
    fn flatten_vertical_line() {
        let cmds = parse_svg_path("M 0 0 V 100");
        let segs = flatten_path(&cmds);
        assert_eq!(segs.len(), 1);
        assert!((segs[0].x2 - 0.0).abs() < 0.01);
        assert!((segs[0].y2 - 100.0).abs() < 0.01);
    }

    #[test]
    fn flatten_close_returns_to_start() {
        let cmds = parse_svg_path("M 10 10 L 50 10 L 50 50 Z");
        let segs = flatten_path(&cmds);
        assert_eq!(segs.len(), 3);
        let last = &segs[2];
        assert!((last.x2 - 10.0).abs() < 0.01);
        assert!((last.y2 - 10.0).abs() < 0.01);
    }

    #[test]
    fn flatten_quadratic_produces_segments() {
        let cmds = parse_svg_path("M 0 0 Q 50 100 100 0");
        let segs = flatten_path(&cmds);
        assert!(segs.len() > 1); // Quadratic should subdivide
        // First seg starts at origin
        assert!((segs[0].x1).abs() < 0.01);
        assert!((segs[0].y1).abs() < 0.01);
        // Last seg ends at (100, 0)
        let last = &segs[segs.len() - 1];
        assert!((last.x2 - 100.0).abs() < 0.5);
        assert!((last.y2).abs() < 0.5);
    }

    // ── paint_svg_path ──

    #[test]
    fn paint_svg_path_with_fill() {
        let mut dl = DisplayList::new();
        paint_svg_path(
            &mut dl,
            "M 0 0 L 100 0 L 100 100 Z",
            10.0,
            20.0,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            2.0,
            Some(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 128,
            }),
        );
        // Should have fill rect + stroke lines
        assert!(dl.len() >= 4); // 1 fill + 3 line segments
    }

    #[test]
    fn paint_svg_path_transparent_stroke_no_lines() {
        let mut dl = DisplayList::new();
        paint_svg_path(
            &mut dl,
            "M 0 0 L 100 0 L 100 100 Z",
            0.0,
            0.0,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }, // transparent
            1.0,
            None,
        );
        assert!(dl.is_empty());
    }

    #[test]
    fn paint_svg_path_zero_width_no_lines() {
        let mut dl = DisplayList::new();
        paint_svg_path(
            &mut dl,
            "M 0 0 L 100 0 L 100 100 Z",
            0.0,
            0.0,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            0.0, // zero width
            None,
        );
        assert!(dl.is_empty());
    }

    // ── Implicit LineTo after M ──

    #[test]
    fn implicit_lineto_after_move() {
        // After M, subsequent coordinate pairs are treated as L
        let cmds = parse_svg_path("M 0 0 10 10 20 20");
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], PathCommand::MoveTo { .. }));
        assert!(matches!(cmds[1], PathCommand::LineTo { .. }));
        assert!(matches!(cmds[2], PathCommand::LineTo { .. }));
    }

    // ── Scientific notation ──

    #[test]
    fn parse_scientific_notation() {
        let cmds = parse_svg_path("M 1e1 2E1 L 1.5e2 2.0E2");
        assert_eq!(cmds.len(), 2);
        match cmds[0] {
            PathCommand::MoveTo { x, y } => {
                assert!((x - 10.0).abs() < 0.01);
                assert!((y - 20.0).abs() < 0.01);
            }
            _ => panic!("expected MoveTo"),
        }
    }
}
