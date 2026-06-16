//! Basic path rasterization: build, fill, and stroke arbitrary paths.
//!
//! Uses scanline rendering with an active edge list and even-odd fill rule.
//! Curves (quadratic and cubic Bézier) are flattened to line segments via
//! adaptive subdivision. Anti-aliasing uses 4x vertical supersampling.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::blend;
use crate::color::SrgbLut;
use crate::rasterizer::Fill;

/// A 2D point used in path construction (kept separate from compositor Point
/// so the path module is self-contained).
#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
}

impl PathPoint {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A segment of a path.
#[derive(Debug, Clone, Copy)]
enum PathSegment {
    MoveTo(PathPoint),
    LineTo(PathPoint),
    QuadTo(PathPoint, PathPoint),
    CubicTo(PathPoint, PathPoint, PathPoint),
    Close,
}

/// An immutable path built from segments.
#[derive(Debug, Clone)]
pub struct Path {
    segments: Vec<PathSegment>,
}

impl Path {
    /// Compute the bounding rectangle of the path (approximate — uses control points).
    #[must_use]
    pub fn bounds(&self) -> Rect {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        let mut visit = |p: &PathPoint| {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        };

        for seg in &self.segments {
            match seg {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => visit(p),
                PathSegment::QuadTo(c, p) => {
                    visit(c);
                    visit(p);
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    visit(c1);
                    visit(c2);
                    visit(p);
                }
                PathSegment::Close => {}
            }
        }

        if min_x > max_x {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        }
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// Number of segments in the path.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Whether the path ends with a `Close` segment.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        matches!(self.segments.last(), Some(PathSegment::Close))
    }

    /// Flatten the path into a list of line segment edges.
    ///
    /// Curves are subdivided using adaptive flattening with the given tolerance.
    fn flatten(&self, tolerance: f32) -> Vec<Edge> {
        let mut edges = Vec::new();
        let mut current = PathPoint::new(0.0, 0.0);
        let mut subpath_start = current;

        for seg in &self.segments {
            match *seg {
                PathSegment::MoveTo(p) => {
                    current = p;
                    subpath_start = p;
                }
                PathSegment::LineTo(p) => {
                    if (current.y - p.y).abs() > f32::EPSILON {
                        edges.push(Edge::new(current, p));
                    }
                    current = p;
                }
                PathSegment::QuadTo(c, p) => {
                    flatten_quad(current, c, p, tolerance, &mut |a, b| {
                        if (a.y - b.y).abs() > f32::EPSILON {
                            edges.push(Edge::new(a, b));
                        }
                    });
                    current = p;
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    flatten_cubic(current, c1, c2, p, tolerance, &mut |a, b| {
                        if (a.y - b.y).abs() > f32::EPSILON {
                            edges.push(Edge::new(a, b));
                        }
                    });
                    current = p;
                }
                PathSegment::Close => {
                    if (current.y - subpath_start.y).abs() > f32::EPSILON {
                        edges.push(Edge::new(current, subpath_start));
                    }
                    current = subpath_start;
                }
            }
        }
        edges
    }
}

/// Builder for constructing a `Path`.
pub struct PathBuilder {
    segments: Vec<PathSegment>,
}

impl PathBuilder {
    /// Create a new empty path builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Move to a new point without drawing.
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.segments
            .push(PathSegment::MoveTo(PathPoint::new(x, y)));
        self
    }

    /// Draw a straight line to the given point.
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.segments
            .push(PathSegment::LineTo(PathPoint::new(x, y)));
        self
    }

    /// Draw a quadratic Bézier curve through a control point to an endpoint.
    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) -> &mut Self {
        self.segments.push(PathSegment::QuadTo(
            PathPoint::new(cx, cy),
            PathPoint::new(x, y),
        ));
        self
    }

    /// Draw a cubic Bézier curve through two control points to an endpoint.
    pub fn cubic_to(
        &mut self,
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    ) -> &mut Self {
        self.segments.push(PathSegment::CubicTo(
            PathPoint::new(c1x, c1y),
            PathPoint::new(c2x, c2y),
            PathPoint::new(x, y),
        ));
        self
    }

    /// Draw a circular arc approximated by cubic Bezier curves.
    ///
    /// `cx` and `cy` define the center of the circle, `radius` the radius,
    /// `start_angle` the starting angle in radians, and `sweep_angle` the
    /// angular extent (positive = counter-clockwise). The arc is connected
    /// to the current point with a straight line if necessary.
    pub fn arc_to(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: f32,
        sweep_angle: f32,
    ) -> &mut Self {
        if radius <= 0.0 || sweep_angle.abs() < f32::EPSILON {
            return self;
        }

        // Start point of the arc
        let sx = cx + radius * start_angle.cos();
        let sy = cy + radius * start_angle.sin();

        // Connect to arc start: move if no current point, otherwise line
        if self.segments.is_empty() {
            self.segments
                .push(PathSegment::MoveTo(PathPoint::new(sx, sy)));
        } else {
            self.segments
                .push(PathSegment::LineTo(PathPoint::new(sx, sy)));
        }

        // Split the arc into segments of at most 90 degrees each
        let n = ((sweep_angle.abs() / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
        let step = sweep_angle / n as f32;

        for i in 0..n {
            let a1 = start_angle + step * i as f32;
            let a2 = a1 + step;
            let half = (a2 - a1) / 2.0;
            let sin_half = half.sin();
            if sin_half.abs() < f32::EPSILON {
                continue;
            }
            let k = (4.0 / 3.0) * (1.0 - half.cos()) / sin_half;

            let cos1 = a1.cos();
            let sin1 = a1.sin();
            let cos2 = a2.cos();
            let sin2 = a2.sin();

            self.cubic_to(
                cx + radius * (cos1 - k * sin1),
                cy + radius * (sin1 + k * cos1),
                cx + radius * (cos2 + k * sin2),
                cy + radius * (sin2 - k * cos2),
                cx + radius * cos2,
                cy + radius * sin2,
            );
        }

        self
    }

    /// Close the current sub-path by drawing a line back to the most recent `move_to`.
    pub fn close(&mut self) -> &mut Self {
        self.segments.push(PathSegment::Close);
        self
    }

    /// Build the immutable `Path`.
    #[must_use]
    pub fn build(&self) -> Path {
        Path {
            segments: self.segments.clone(),
        }
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Edge and scanline internals
// ---------------------------------------------------------------------------

/// An edge in the edge table, always oriented so y0 <= y1.
#[derive(Debug, Clone)]
struct Edge {
    /// Top Y
    y0: f32,
    /// Bottom Y
    y1: f32,
    /// X at y0
    x_at_y0: f32,
    /// Inverse slope: dx / dy
    inv_slope: f32,
}

impl Edge {
    fn new(a: PathPoint, b: PathPoint) -> Self {
        let (top, bot) = if a.y <= b.y { (a, b) } else { (b, a) };
        let dy = bot.y - top.y;
        let inv_slope = if dy.abs() > f32::EPSILON {
            (bot.x - top.x) / dy
        } else {
            0.0
        };
        Self {
            y0: top.y,
            y1: bot.y,
            x_at_y0: top.x,
            inv_slope,
        }
    }

    /// X intercept at a given Y.
    #[inline]
    fn x_at(&self, y: f32) -> f32 {
        self.x_at_y0 + (y - self.y0) * self.inv_slope
    }
}

// ---------------------------------------------------------------------------
// Curve flattening
// ---------------------------------------------------------------------------

/// Flatten a quadratic Bézier by adaptive subdivision.
fn flatten_quad<F: FnMut(PathPoint, PathPoint)>(
    p0: PathPoint,
    c: PathPoint,
    p1: PathPoint,
    tolerance: f32,
    emit: &mut F,
) {
    // Check if the control point is close enough to the line p0→p1
    let mx = (p0.x + p1.x) * 0.5;
    let my = (p0.y + p1.y) * 0.5;
    let dx = c.x - mx;
    let dy = c.y - my;
    if dx * dx + dy * dy <= tolerance * tolerance {
        emit(p0, p1);
        return;
    }

    // De Casteljau subdivision at t=0.5
    let m01 = PathPoint::new((p0.x + c.x) * 0.5, (p0.y + c.y) * 0.5);
    let m12 = PathPoint::new((c.x + p1.x) * 0.5, (c.y + p1.y) * 0.5);
    let mid = PathPoint::new((m01.x + m12.x) * 0.5, (m01.y + m12.y) * 0.5);

    flatten_quad(p0, m01, mid, tolerance, emit);
    flatten_quad(mid, m12, p1, tolerance, emit);
}

/// Flatten a cubic Bézier by adaptive subdivision.
fn flatten_cubic<F: FnMut(PathPoint, PathPoint)>(
    p0: PathPoint,
    c1: PathPoint,
    c2: PathPoint,
    p1: PathPoint,
    tolerance: f32,
    emit: &mut F,
) {
    // Check flatness: max distance of control points from the chord p0→p1
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let len_sq = dx * dx + dy * dy;

    let d1 = if len_sq > f32::EPSILON {
        let t1 = ((c1.x - p0.x) * dx + (c1.y - p0.y) * dy) / len_sq;
        let proj_x = p0.x + t1 * dx;
        let proj_y = p0.y + t1 * dy;
        let ex = c1.x - proj_x;
        let ey = c1.y - proj_y;
        ex * ex + ey * ey
    } else {
        let ex = c1.x - p0.x;
        let ey = c1.y - p0.y;
        ex * ex + ey * ey
    };

    let d2 = if len_sq > f32::EPSILON {
        let t2 = ((c2.x - p0.x) * dx + (c2.y - p0.y) * dy) / len_sq;
        let proj_x = p0.x + t2 * dx;
        let proj_y = p0.y + t2 * dy;
        let ex = c2.x - proj_x;
        let ey = c2.y - proj_y;
        ex * ex + ey * ey
    } else {
        let ex = c2.x - p0.x;
        let ey = c2.y - p0.y;
        ex * ex + ey * ey
    };

    let tol_sq = tolerance * tolerance;
    if d1 <= tol_sq && d2 <= tol_sq {
        emit(p0, p1);
        return;
    }

    // De Casteljau subdivision at t=0.5
    let m01 = PathPoint::new((p0.x + c1.x) * 0.5, (p0.y + c1.y) * 0.5);
    let m12 = PathPoint::new((c1.x + c2.x) * 0.5, (c1.y + c2.y) * 0.5);
    let m23 = PathPoint::new((c2.x + p1.x) * 0.5, (c2.y + p1.y) * 0.5);
    let m012 = PathPoint::new((m01.x + m12.x) * 0.5, (m01.y + m12.y) * 0.5);
    let m123 = PathPoint::new((m12.x + m23.x) * 0.5, (m12.y + m23.y) * 0.5);
    let mid = PathPoint::new((m012.x + m123.x) * 0.5, (m012.y + m123.y) * 0.5);

    flatten_cubic(p0, m01, m012, mid, tolerance, emit);
    flatten_cubic(mid, m123, m23, p1, tolerance, emit);
}

// ---------------------------------------------------------------------------
// Public fill and stroke functions
// ---------------------------------------------------------------------------

/// Number of vertical supersamples for anti-aliasing.
const AA_SAMPLES: u32 = 4;

/// Fill a path into the framebuffer using the even-odd rule.
///
/// Uses scanline rasterization with 4x vertical supersampling for AA.
pub fn fill_path(fb: &mut FrameBuffer, path: &Path, fill: &Fill, mode: BlendMode, lut: &SrgbLut) {
    let tolerance = 0.25; // flatten tolerance in pixels
    let edges = path.flatten(tolerance);
    if edges.is_empty() {
        return;
    }

    let bounds = path.bounds();
    let x0 = (bounds.x.floor().max(0.0) as u32).min(fb.width);
    let y0 = (bounds.y.floor().max(0.0) as u32).min(fb.height);
    let x1 = (bounds.right().ceil() as u32).min(fb.width);
    let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
    // Confine to the per-thread write-scissor (t80). Coverage is computed from
    // absolute pixel coords, so clamping the window only skips edge pixels.
    let (x0, y0, x1, y1) = crate::rasterizer::scissor_clamp_window(x0, y0, x1, y1);

    for y in y0..y1 {
        // Accumulate coverage from AA_SAMPLES sub-scanlines
        // We allocate a coverage buffer for the scanline
        let width = (x1 - x0) as usize;
        if width == 0 {
            continue;
        }
        let mut coverage = vec![0u32; width];

        for sub in 0..AA_SAMPLES {
            let scan_y = y as f32 + (sub as f32 + 0.5) / AA_SAMPLES as f32;

            // Collect X intercepts of active edges at this sub-scanline
            let mut intercepts: Vec<f32> = Vec::new();
            for edge in &edges {
                if scan_y >= edge.y0 && scan_y < edge.y1 {
                    intercepts.push(edge.x_at(scan_y));
                }
            }

            intercepts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Even-odd fill: toggle between pairs of intercepts
            for pair in intercepts.chunks_exact(2) {
                let left = pair[0];
                let right = pair[1];

                let ix0 = (left.floor().max(x0 as f32) as u32).max(x0);
                let ix1 = (right.ceil().min(x1 as f32) as u32).min(x1);

                for x in ix0..ix1 {
                    let fx = x as f32 + 0.5;
                    // Compute fractional coverage at edges
                    let c = if fx >= left && fx <= right { 1 } else { 0 };
                    coverage[(x - x0) as usize] += c;
                }
            }
        }

        // Convert coverage to alpha and composite
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let x = x0 + i as u32;
            let alpha = (cov as f32 / AA_SAMPLES as f32 * 255.0 + 0.5) as u8;

            let base_color = match fill {
                Fill::Solid(c) => *c,
                Fill::Gradient(g) => {
                    let fx = x as f32 + 0.5;
                    let fy = y as f32 + 0.5;
                    crate::rasterizer::sample_gradient_at(g, fx, fy, lut)
                }
            };

            let mut pm = base_color.premultiply();
            if alpha < 255 {
                pm.a = ((pm.a as u16 * alpha as u16 + 127) / 255) as u8;
                pm.r = ((pm.r as u16 * alpha as u16 + 127) / 255) as u8;
                pm.g = ((pm.g as u16 * alpha as u16 + 127) / 255) as u8;
                pm.b = ((pm.b as u16 * alpha as u16 + 127) / 255) as u8;
            }

            let dst = fb.get_pixel(x, y);
            let result = blend::blend(dst, pm, mode);
            fb.set_pixel(x, y, result);
        }
    }
}

/// Stroke a path with the given width and color.
///
/// Implemented by offsetting the path outward and inward by half the stroke
/// width, creating two offset paths, and filling the region between them.
/// For simplicity, we use a direct per-pixel distance approach.
pub fn stroke_path(fb: &mut FrameBuffer, path: &Path, width: f32, color: Color, mode: BlendMode) {
    if width <= 0.0 {
        return;
    }

    let tolerance = 0.25;
    let line_segments = collect_line_segments(path, tolerance);
    if line_segments.is_empty() {
        return;
    }

    let half = width * 0.5;
    let bounds = path.bounds();
    let x0 = ((bounds.x - half - 1.0).floor().max(0.0) as u32).min(fb.width);
    let y0 = ((bounds.y - half - 1.0).floor().max(0.0) as u32).min(fb.height);
    let x1 = ((bounds.right() + half + 1.0).ceil() as u32).min(fb.width);
    let y1 = ((bounds.bottom() + half + 1.0).ceil() as u32).min(fb.height);
    // Confine to the per-thread write-scissor (t80).
    let (x0, y0, x1, y1) = crate::rasterizer::scissor_clamp_window(x0, y0, x1, y1);

    let pm = color.premultiply();

    for y in y0..y1 {
        let fy = y as f32 + 0.5;
        for x in x0..x1 {
            let fx = x as f32 + 0.5;

            // Find minimum distance to any line segment
            let mut min_dist = f32::MAX;
            for seg in &line_segments {
                let d = point_to_segment_dist(fx, fy, seg.0, seg.1);
                if d < min_dist {
                    min_dist = d;
                }
            }

            if min_dist > half + 0.5 {
                continue;
            }

            let alpha = if min_dist <= half - 0.5 {
                1.0
            } else {
                (half + 0.5 - min_dist).clamp(0.0, 1.0)
            };

            if alpha <= 0.0 {
                continue;
            }

            let mut src = pm;
            if alpha < 1.0 {
                src.a = (src.a as f32 * alpha + 0.5) as u8;
                src.r = (src.r as f32 * alpha + 0.5) as u8;
                src.g = (src.g as f32 * alpha + 0.5) as u8;
                src.b = (src.b as f32 * alpha + 0.5) as u8;
            }

            let dst = fb.get_pixel(x, y);
            let result = blend::blend(dst, src, mode);
            fb.set_pixel(x, y, result);
        }
    }
}

/// Collect flattened line segments as (PathPoint, PathPoint) pairs.
fn collect_line_segments(path: &Path, tolerance: f32) -> Vec<(PathPoint, PathPoint)> {
    let mut segments = Vec::new();
    let mut current = PathPoint::new(0.0, 0.0);
    let mut subpath_start = current;

    for seg in &path.segments {
        match *seg {
            PathSegment::MoveTo(p) => {
                current = p;
                subpath_start = p;
            }
            PathSegment::LineTo(p) => {
                segments.push((current, p));
                current = p;
            }
            PathSegment::QuadTo(c, p) => {
                flatten_quad(current, c, p, tolerance, &mut |a, b| {
                    segments.push((a, b));
                });
                current = p;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                flatten_cubic(current, c1, c2, p, tolerance, &mut |a, b| {
                    segments.push((a, b));
                });
                current = p;
            }
            PathSegment::Close => {
                segments.push((current, subpath_start));
                current = subpath_start;
            }
        }
    }
    segments
}

/// Distance from point (px, py) to the line segment (a, b).
fn point_to_segment_dist(px: f32, py: f32, a: PathPoint, b: PathPoint) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < f32::EPSILON {
        let ex = px - a.x;
        let ey = py - a.y;
        return (ex * ex + ey * ey).sqrt();
    }

    let t = ((px - a.x) * dx + (py - a.y) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let proj_x = a.x + t * dx;
    let proj_y = a.y + t * dy;
    let ex = px - proj_x;
    let ey = py - proj_y;
    (ex * ex + ey * ey).sqrt()
}
