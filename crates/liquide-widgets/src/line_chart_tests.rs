//! `<lq-line-chart>` real-pipeline gallery tests.
//!
//! x-position of each point comes from the laid-out flex cell (real layout box).
//! The vertical position is a `scaleY` spacer (paint-only), so vertical
//! proportionality is asserted via PIXELS. Hover is resolved from the laid-out
//! plot box (nearest-index + value math), never a constant.
#![cfg(test)]

use liquide_compositor::pixel::Color;

use crate::chart::Series;
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;
use crate::line_chart::{LineChart, HOVER_ACTION};

const W: u32 = 480;
const H: u32 = 280;
const PLOT_BG: Color = Color { r: 39, g: 39, b: 42, a: 255 };

fn as_chart(g: &Gallery) -> &LineChart {
    g.host.behavior("c").unwrap().as_any().downcast_ref::<LineChart>().unwrap()
}

fn plot(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "plot").expect("plot box")
}

/// The laid-out box of the (series, index) flex cell (x is data/box driven).
fn cell_box(g: &Gallery, series: usize, index: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    // Walk: find the series row with data-series==series, then its index-th cell.
    let mut cell = None;
    fn find_series(doc: &liquide_dom::Document, n: liquide_dom::NodeId, s: &str) -> Option<liquide_dom::NodeId> {
        if doc.get_attribute(n, "data-part").as_deref() == Some("series")
            && doc.get_attribute(n, "data-series").as_deref() == Some(s)
        {
            return Some(n);
        }
        for &c in doc.children(n) {
            if let Some(f) = find_series(doc, c, s) {
                return Some(f);
            }
        }
        None
    }
    if let Some(srow) = find_series(g.doc(), root, &series.to_string()) {
        let cells: Vec<_> = g
            .doc()
            .children(srow)
            .iter()
            .copied()
            .filter(|&c| g.doc().get_attribute(c, "data-part").as_deref() == Some("cell"))
            .collect();
        cell = cells.get(index).copied();
    }
    q.box_of(cell.expect("cell node")).expect("cell box")
}

fn chart(values: Vec<f32>) -> LineChart {
    LineChart::from_values(values)
}

/// The chart renders a real plot box that paints.
#[test]
fn line_chart_renders() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("c", Box::new(chart(vec![1.0, 4.0, 2.0, 6.0, 3.0])));
    g.relayout();
    let p = plot(&g);
    assert!(p.width > 200.0 && p.height > 100.0, "plot sized from CSS (got {}x{})", p.width, p.height);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (p.x + p.width / 2.0) as u32, (p.y + p.height / 2.0) as u32);
    assert!(px.a > 0, "plot paints");
}

/// NO-FAKE-GREEN: point x positions are evenly spread across the laid-out plot
/// (flex cells). Cell 0 is at the left, the last cell at the right.
#[test]
fn point_x_spread_across_plot() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![0.0, 10.0, 0.0, 10.0])));
    g.relayout();
    let p = plot(&g);
    let c0 = cell_box(&g, 0, 0);
    let c3 = cell_box(&g, 0, 3);
    assert!((c0.x - p.x).abs() < p.width * 0.05, "cell 0 at left edge");
    assert!(c3.x + c3.width > p.x + p.width * 0.7, "cell 3 toward the right");
    assert!((c0.width - p.width / 4.0).abs() < p.width * 0.05, "each cell ~1/4 plot");
}

/// NO-FAKE-GREEN: a point's painted y is data-proportional — a low value paints
/// lower on screen than a high value (within the same plot).
#[test]
fn point_y_is_data_proportional() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![0.0, 10.0])));
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    // For each cell, find the painted marker y (the accent dot).
    let marker_y = |cell: liquide_layout::geometry::Rect| -> Option<u32> {
        let x = (cell.x + cell.width / 2.0) as u32;
        for y in (p.y as u32)..((p.y + p.height) as u32) {
            let px = fb.get_pixel(x, y);
            if px.b > 150 && px.r < 120 && px.a > 0 {
                return Some(y);
            }
        }
        None
    };
    let y0 = marker_y(cell_box(&g, 0, 0)).expect("point 0 paints");
    let y1 = marker_y(cell_box(&g, 0, 1)).expect("point 1 paints");
    assert!(y0 > y1, "low value point paints lower on screen ({y0} vs {y1})");
}

/// NO-FAKE-GREEN: hover resolves the point from the LAID-OUT plot box. The pointer
/// at point 2's laid-out x maps to index 2, and at point 4's x maps to 4.
#[test]
fn hover_resolves_point_from_layout() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0, 4.0, 5.0])));
    g.relayout();

    let c2 = cell_box(&g, 0, 2);
    g.pointer_move(c2.x + c2.width / 2.0, c2.y + c2.height / 2.0);
    let actions = g.process();
    assert_eq!(as_chart(&g).hovered(), Some((0, 2)), "hover at point-2 x maps to index 2");
    assert!(
        actions.iter().any(|a| a.name == HOVER_ACTION && a.payload.as_deref() == Some("0,2,3")),
        "hover action carries series,index,value (got {actions:?})"
    );

    let c4 = cell_box(&g, 0, 4);
    g.pointer_move(c4.x + c4.width / 2.0, c4.y + c4.height / 2.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some((0, 4)), "hover at point-4 x maps to index 4");
}

/// NO-FAKE-GREEN (anti-constant): the same fractional pointer x resolves to a
/// different index when the plot is resized — index 2 at 0.5 of 5 points, index 4
/// at 0.95. A constant pitch fails one.
#[test]
fn hover_index_follows_plot_width() {
    let mk = || {
        let mut g = Gallery::new(640, 240, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 180px; }");
        g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0, 4.0, 5.0])));
        g.relayout();
        g
    };
    let mut a = mk();
    let pa = plot(&a);
    a.pointer_move(pa.x + pa.width * 0.5, pa.y + pa.height * 0.5);
    let _ = a.process();
    assert_eq!(as_chart(&a).hovered().map(|(_, i)| i), Some(2), "0.5 of 5 points -> index 2");

    let mut b = mk();
    let pb = plot(&b);
    b.pointer_move(pb.x + pb.width * 0.95, pb.y + pb.height * 0.5);
    let _ = b.process();
    assert_eq!(as_chart(&b).hovered().map(|(_, i)| i), Some(4), "0.95 fraction -> index 4");
}

/// Leaving the chart clears the hover.
#[test]
fn mouse_leave_clears_hover() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0])));
    g.relayout();
    let c1 = cell_box(&g, 0, 1);
    g.pointer_move(c1.x + c1.width / 2.0, c1.y + c1.height / 2.0);
    let _ = g.process();
    assert!(as_chart(&g).hovered().is_some());
    g.pointer_move(2.0, 2.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), None, "leaving clears hover");
}

/// The laid-out box of the i-th stroke segment (data-part="seg").
fn seg_box(g: &Gallery, series: usize, index: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    fn find_stroke(doc: &liquide_dom::Document, n: liquide_dom::NodeId, s: &str) -> Option<liquide_dom::NodeId> {
        if doc.get_attribute(n, "data-part").as_deref() == Some("stroke")
            && doc.get_attribute(n, "data-series").as_deref() == Some(s)
        {
            return Some(n);
        }
        for &c in doc.children(n) {
            if let Some(f) = find_stroke(doc, c, s) {
                return Some(f);
            }
        }
        None
    }
    let stroke = find_stroke(g.doc(), root, &series.to_string()).expect("stroke layer");
    let segs: Vec<_> = g
        .doc()
        .children(stroke)
        .iter()
        .copied()
        .filter(|&c| g.doc().get_attribute(c, "data-part").as_deref() == Some("seg"))
        .collect();
    q.box_of(segs.get(index).copied().expect("seg node")).expect("seg box")
}

/// NO-FAKE-GREEN: the chart emits a REAL CONNECTED POLYLINE, not isolated stems.
/// For n points there are n-1 stroke segments, and segment i's laid-out box spans
/// exactly the horizontal gap from point i to point i+1 (its left == cell i center
/// region, its right reaches point i+1) — so consecutive segments tile the plot
/// width edge-to-edge with no gaps. Isolated stems (the old degraded rendering)
/// would NOT produce gap-spanning boxes that meet end to end.
#[test]
fn emits_a_connected_polyline_not_isolated_stems() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![1.0, 4.0, 2.0, 6.0, 3.0])));
    g.relayout();
    let p = plot(&g);
    // 5 points -> 4 segments. Each spans 1/4 of the plot width.
    let s0 = seg_box(&g, 0, 0);
    let s1 = seg_box(&g, 0, 1);
    let s3 = seg_box(&g, 0, 3);
    // Segment 0 starts at the left edge of the plot.
    assert!((s0.x - p.x).abs() < p.width * 0.03, "seg0 starts at plot left (got {} vs {})", s0.x, p.x);
    // Each segment spans ~1/4 of the plot.
    assert!((s0.width - p.width / 4.0).abs() < p.width * 0.05, "seg spans 1/4 plot (got {})", s0.width);
    // Consecutive segments are CONNECTED: seg1 starts where seg0 ends (within 1px).
    assert!((s1.x - (s0.x + s0.width)).abs() < 2.0, "seg1 starts at seg0 end (connected, got {} vs {})", s1.x, s0.x + s0.width);
    // The last segment reaches the right edge of the plot.
    assert!(s3.x + s3.width > p.x + p.width * 0.95, "seg3 reaches plot right (got {})", s3.x + s3.width);
}

/// NO-FAKE-GREEN: the painted stroke is CONTINUOUS between adjacent points — at the
/// horizontal midpoint between point 0 and point 1, the line paints at roughly the
/// average of the two points' screen y (a real connecting segment), not at one
/// point's y or absent (which isolated stems would give).
#[test]
fn stroke_is_continuous_between_points() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    // Two points: low (0) then high (10). The midpoint of the line should paint
    // near the vertical middle of the plot.
    g.mount("c", Box::new(chart(vec![0.0, 10.0])));
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    let xmid = (p.x + p.width * 0.5) as u32;
    // Find the painted blue stroke at the horizontal midpoint.
    let mut line_y = None;
    for y in (p.y as u32 + 1)..((p.y + p.height) as u32 - 1) {
        let px = fb.get_pixel(xmid, y);
        if px.b > 150 && px.r < 120 {
            line_y = Some(y as f32);
            break;
        }
    }
    let ly = line_y.expect("stroke paints at the horizontal midpoint (continuous line)");
    // Point 0 (value 0) is near the bottom, point 1 (value 10) near the top; the
    // line midpoint is near the vertical center.
    let center = p.y + p.height * 0.5;
    assert!((ly - center).abs() < p.height * 0.25, "line midpoint near plot center ({ly} vs {center})");
}

/// Multiple series each lay out their own cells; overlapping points at the same
/// index paint at different y (different values).
#[test]
fn multiple_series_render() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-line-chart { width: 400px; height: 200px; }");
    g.mount(
        "c",
        Box::new(LineChart::new(vec![
            Series::new("a", vec![1.0, 5.0, 2.0]),
            Series::new("b", vec![4.0, 1.0, 6.0]).color("#ef4444"),
        ])),
    );
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    // At index 1, series a value 5 (high) paints higher than series b value 1 (low).
    let a1 = cell_box(&g, 0, 1);
    let xa = (a1.x + a1.width / 2.0) as u32;
    let mut a_blue = None;
    let mut b_red = None;
    for y in (p.y as u32)..((p.y + p.height) as u32) {
        let px = fb.get_pixel(xa, y);
        if px.b > 150 && px.r < 120 && a_blue.is_none() {
            a_blue = Some(y);
        }
        if px.r > 150 && px.b < 120 && b_red.is_none() {
            b_red = Some(y);
        }
    }
    let ay = a_blue.expect("series a (blue) paints");
    let by = b_red.expect("series b (red) paints");
    assert!(ay < by, "series a value 5 paints higher than series b value 1 ({ay} vs {by})");
    let _ = PLOT_BG;
}
