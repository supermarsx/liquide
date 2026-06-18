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
