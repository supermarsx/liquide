//! `<lq-bar-chart>` real-pipeline gallery tests.
//!
//! Vertical bar extent is now a real inline `height:%` anchored at the bottom, so
//! the bar's LAID-OUT BOX reflects the value directly (asserted via layout boxes)
//! as well as via pixels. Horizontal distribution uses flex columns (real layout).
//! Hover is resolved from the laid-out plot box (bar-slot math), never a constant.
#![cfg(test)]

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::Color;

use crate::bar_chart::{BarChart, HOVER_ACTION};
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;

const W: u32 = 480;
const H: u32 = 280;
const BG: Color = Color { r: 39, g: 39, b: 42, a: 255 }; // --widget-bg plot fill

fn as_chart(g: &Gallery) -> &BarChart {
    g.host.behavior("c").unwrap().as_any().downcast_ref::<BarChart>().unwrap()
}

fn plot(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "plot").expect("plot box")
}

fn col_box(g: &Gallery, idx: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let mut cols = Vec::new();
    fn walk(doc: &liquide_dom::Document, n: liquide_dom::NodeId, out: &mut Vec<liquide_dom::NodeId>) {
        if doc.get_attribute(n, "data-part").as_deref() == Some("col") {
            out.push(n);
        }
        for &c in doc.children(n) {
            walk(doc, c, out);
        }
    }
    walk(g.doc(), root, &mut cols);
    q.box_of(cols[idx]).expect("col box")
}

/// Painted bar height in a column: distance from the first non-bg painted pixel
/// (the bar's top) down to the plot bottom.
fn painted_bar_height(fb: &FrameBuffer, plot: liquide_layout::geometry::Rect, col: liquide_layout::geometry::Rect) -> u32 {
    let x = (col.x + col.width / 2.0) as u32;
    let mut top = None;
    for y in (plot.y as u32 + 2)..((plot.y + plot.height) as u32 - 1) {
        let p = fb.get_pixel(x, y);
        // The bar is the (macOS-dark) graphite accent — a bright neutral gray
        // (~#8e8e93, all channels ~140+). The plot bg is the dark widget bg
        // (~#2c2c2e, ~44) and gridlines are a faint white wash (~61 over bg),
        // both well under 100, so a >100-on-all-channels test isolates the bar.
        if p.r > 100 && p.g > 100 && p.b > 100 && p.a > 0 {
            top = Some(y);
            break;
        }
    }
    match top {
        Some(t) => (plot.y + plot.height) as u32 - t,
        None => 0,
    }
}

fn bar_box(g: &Gallery, idx: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let mut bars = Vec::new();
    fn walk(doc: &liquide_dom::Document, n: liquide_dom::NodeId, out: &mut Vec<liquide_dom::NodeId>) {
        if doc.get_attribute(n, "data-part").as_deref() == Some("bar") {
            out.push(n);
        }
        for &c in doc.children(n) {
            walk(doc, c, out);
        }
    }
    walk(g.doc(), root, &mut bars);
    q.box_of(bars[idx]).expect("bar box")
}

fn chart(values: Vec<f32>) -> BarChart {
    BarChart::new(values)
}

/// NO-FAKE-GREEN: with real height-based sizing the bar's LAID-OUT BOX height is
/// the value fraction of the plot, and the box is bottom-anchored. A half-value
/// bar's box is ~half the plot tall; the max-value bar ~full; and the box bottom
/// sits at the plot bottom. (The old scaleY rendering left every box full-height —
/// this asserts the layout box itself now reflects the value.)
#[test]
fn bar_box_height_reflects_value() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![0.0, 5.0, 10.0])));
    g.relayout();
    let p = plot(&g);
    let b1 = bar_box(&g, 1); // value 5 of 10 -> ~half
    let b2 = bar_box(&g, 2); // value 10 -> ~full
    assert!(b2.height > b1.height, "taller value -> taller laid-out box ({} vs {})", b2.height, b1.height);
    assert!((b2.height - p.height).abs() < p.height * 0.06, "max bar box ~= full plot ({} vs {})", b2.height, p.height);
    assert!((b1.height - p.height * 0.5).abs() < p.height * 0.08, "mid bar box ~= half plot ({} vs {})", b1.height, p.height * 0.5);
    // Bottom-anchored: each bar's bottom is at the plot bottom.
    let plot_bottom = p.y + p.height;
    assert!((b1.y + b1.height - plot_bottom).abs() < 3.0, "bar box bottom-anchored to plot bottom");
    let _ = BG;
}

/// The chart renders a real plot box, and a bar paints accent pixels.
#[test]
fn bar_chart_renders() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![3.0, 7.0, 2.0, 5.0])));
    g.relayout();
    let p = plot(&g);
    assert!(p.width > 200.0 && p.height > 100.0, "plot from CSS (got {}x{})", p.width, p.height);
    let fb = g.rasterize();
    let h = painted_bar_height(&fb, p, col_box(&g, 1));
    assert!(h > 10, "a bar paints accent pixels (got height {h})");
}

/// NO-FAKE-GREEN: painted bar heights are data-proportional within the laid-out
/// plot. The max-value bar paints ~full plot; a half-value bar ~half.
#[test]
fn bar_painted_heights_data_proportional() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![0.0, 5.0, 10.0])));
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    let h1 = painted_bar_height(&fb, p, col_box(&g, 1));
    let h2 = painted_bar_height(&fb, p, col_box(&g, 2));
    assert!(h2 > h1, "taller value -> taller painted bar ({h2} vs {h1})");
    assert!((h2 as f32 - p.height).abs() < p.height * 0.18, "max bar ~= full plot ({h2} vs {})", p.height);
    assert!((h1 as f32 - p.height * 0.5).abs() < p.height * 0.22, "mid bar ~= half plot ({h1} vs {})", p.height * 0.5);
}

/// NO-FAKE-GREEN: painted bars rescale with the box.
#[test]
fn bars_rescale_with_box() {
    let mk = |h: &str| {
        let css = format!("lq-gallery{{padding:12px;}} lq-bar-chart{{width:300px;height:{h};}}");
        let mut g = Gallery::new(360, 360, &css);
        g.mount("c", Box::new(chart(vec![2.0, 6.0, 10.0])));
        g.relayout();
        let p = plot(&g);
        let fb = g.rasterize();
        (p.height, painted_bar_height(&fb, p, col_box(&g, 2)))
    };
    let (ps, bs) = mk("80px");
    let (pb, bb) = mk("240px");
    assert!(pb > ps * 2.5, "precondition: taller plot");
    assert!(bb as f32 > bs as f32 * 2.2, "max bar painted extent tracks plot height ({bs} -> {bb})");
}

/// NO-FAKE-GREEN: columns divide the laid-out plot width (x is data/box driven).
#[test]
fn columns_divide_plot_width() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0, 4.0])));
    g.relayout();
    let p = plot(&g);
    let c0 = col_box(&g, 0);
    let c3 = col_box(&g, 3);
    assert!((c0.width - p.width / 4.0).abs() < p.width * 0.05, "col ~1/4 plot ({} vs {})", c0.width, p.width / 4.0);
    assert!(c3.x > c0.x + p.width * 0.6, "col 3 far right of col 0");
}

/// NO-FAKE-GREEN: hover resolves the bar from the LAID-OUT plot box. Moving over
/// each column maps to that bar's index, with the value in the action payload.
#[test]
fn hover_resolves_bar_from_layout() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![3.0, 6.0, 9.0, 4.0])));
    g.relayout();

    let c2 = col_box(&g, 2);
    g.pointer_move(c2.x + c2.width / 2.0, c2.y + c2.height / 2.0);
    let actions = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(2), "hover over column 2 maps to index 2");
    assert!(
        actions.iter().any(|a| a.name == HOVER_ACTION && a.payload.as_deref() == Some("2,9")),
        "hover action carries index,value (got {actions:?})"
    );

    let c0 = col_box(&g, 0);
    g.pointer_move(c0.x + c0.width / 2.0, c0.y + c0.height / 2.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(0), "hover over column 0 maps to index 0");
}

/// NO-FAKE-GREEN (anti-constant): a fractional pointer x resolves to the slab
/// owning that fraction of the REAL plot width — index 2 at 0.55 and index 3 at
/// 0.80 of 4 bars, regardless of pixel size.
#[test]
fn hover_slab_follows_plot_width() {
    let mk = |w: &str| {
        let css = format!("lq-gallery{{padding:12px;}} lq-bar-chart{{width:{w};height:180px;}}");
        let mut g = Gallery::new(640, 240, &css);
        g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0, 4.0])));
        g.relayout();
        g
    };
    let mut a = mk("400px");
    let pa = plot(&a);
    a.pointer_move(pa.x + pa.width * 0.55, pa.y + pa.height * 0.5);
    let _ = a.process();
    assert_eq!(as_chart(&a).hovered(), Some(2), "0.55 of 4 slabs -> index 2");

    let mut b = mk("200px");
    let pb = plot(&b);
    b.pointer_move(pb.x + pb.width * 0.80, pb.y + pb.height * 0.5);
    let _ = b.process();
    assert_eq!(as_chart(&b).hovered(), Some(3), "0.80 of 4 slabs -> index 3 (from real width)");
}

/// Hovering a bar restyles it (the :hover rule changes the bar's pixels).
#[test]
fn hover_restyles_bar_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![5.0, 8.0, 3.0])));
    g.relayout();
    let p = plot(&g);
    let c1 = col_box(&g, 1);
    let sx = (c1.x + c1.width / 2.0) as u32;
    let sy = (p.y + p.height - 8.0) as u32;
    let before = g.rasterize().get_pixel(sx, sy);
    g.pointer_move(c1.x + c1.width / 2.0, c1.y + c1.height / 2.0);
    let _ = g.process();
    let after = g.rasterize().get_pixel(sx, sy);
    assert!(before != after, "hover restyles the bar (before {before:?} after {after:?})");
}

/// Leaving the chart clears the hover.
#[test]
fn mouse_leave_clears_hover() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-bar-chart { width: 400px; height: 200px; }");
    g.mount("c", Box::new(chart(vec![1.0, 2.0, 3.0])));
    g.relayout();
    let c1 = col_box(&g, 1);
    g.pointer_move(c1.x + c1.width / 2.0, c1.y + c1.height / 2.0);
    let _ = g.process();
    assert!(as_chart(&g).hovered().is_some());
    g.pointer_move(2.0, 2.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), None);
}

/// Different data produces different painted bar geometry.
#[test]
fn different_data_differs() {
    let css = "lq-gallery{padding:12px;} lq-bar-chart{width:300px;height:160px;}";
    let mut a = Gallery::new(W, H, css);
    a.mount("c", Box::new(chart(vec![1.0, 2.0, 9.0])));
    a.relayout();
    let pa = plot(&a);
    let fa = a.rasterize();
    let a2 = painted_bar_height(&fa, pa, col_box(&a, 2));

    let mut b = Gallery::new(W, H, css);
    b.mount("c", Box::new(chart(vec![9.0, 2.0, 1.0])));
    b.relayout();
    let pb = plot(&b);
    let fbb = b.rasterize();
    let b2 = painted_bar_height(&fbb, pb, col_box(&b, 2));
    assert!((a2 as i64 - b2 as i64).abs() > 20, "reversed data -> different bar-2 painted height ({a2} vs {b2})");
    let _ = BG;
}
