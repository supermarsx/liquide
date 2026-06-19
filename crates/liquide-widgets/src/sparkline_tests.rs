//! `<lq-sparkline>` real-pipeline gallery tests.
//!
//! Vertical extent is a `scaleY` transform (paint-only in this engine — the
//! layout box stays full-height), so proportionality is asserted via PIXELS
//! (scanning a column for painted accent pixels). Horizontal distribution uses
//! flex columns, whose laid-out boxes ARE data/box driven.
#![cfg(test)]

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::Color;

use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;
use crate::sparkline::Sparkline;

const W: u32 = 240;
const H: u32 = 120;

fn plot(g: &Gallery, id: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "plot").expect("plot box")
}

/// The laid-out box of the i-th flex column (x distribution is real layout).
fn col_box(g: &Gallery, id: &str, idx: usize) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
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

/// Count the painted accent (non-background) pixels in the column above the plot
/// bottom — a proxy for the bar/point's painted vertical extent.
fn painted_height(fb: &FrameBuffer, plot: liquide_layout::geometry::Rect, col: liquide_layout::geometry::Rect, bg: Color) -> u32 {
    let x = (col.x + col.width / 2.0) as u32;
    let mut top = None;
    for y in (plot.y as u32)..((plot.y + plot.height) as u32) {
        let p = fb.get_pixel(x, y);
        if p != bg && p.a > 0 {
            top = Some(y);
            break;
        }
    }
    match top {
        Some(t) => (plot.y + plot.height) as u32 - t,
        None => 0,
    }
}

/// The sparkline renders a real plot box that paints.
#[test]
fn sparkline_renders() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(Sparkline::bars(vec![1.0, 3.0, 2.0, 5.0])));
    g.relayout();
    let p = plot(&g, "s");
    assert!(p.width > 50.0 && p.height > 10.0, "plot sized from CSS (got {}x{})", p.width, p.height);
}

/// NO-FAKE-GREEN: columns evenly divide the laid-out plot width (x is data/box
/// driven). 3 data -> 3 equal columns spanning the plot.
#[test]
fn columns_divide_plot_width() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; } lq-sparkline { width: 210px; height: 80px; }");
    g.mount("s", Box::new(Sparkline::bars(vec![2.0, 4.0, 8.0])));
    g.relayout();
    let p = plot(&g, "s");
    let c0 = col_box(&g, "s", 0);
    let c2 = col_box(&g, "s", 2);
    assert!((c0.width - p.width / 3.0).abs() < p.width * 0.06, "col ~1/3 of plot ({} vs {})", c0.width, p.width / 3.0);
    assert!(c2.x > c0.x + p.width * 0.5, "col 2 is to the right of col 0");
}

/// NO-FAKE-GREEN: bar painted heights are proportional to the data within the
/// laid-out plot. The max-value bar paints ~the full plot height; a half-value bar
/// ~half. A fixed-pixel bar fails this.
#[test]
fn bar_painted_heights_are_data_proportional() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; } lq-sparkline { width: 200px; height: 80px; }");
    g.mount("s", Box::new(Sparkline::bars(vec![0.0, 5.0, 10.0])));
    g.relayout();
    let p = plot(&g, "s");
    let bg = Color::new(0, 0, 0, 0); // gallery has no background -> transparent
    let fb = g.rasterize();
    let h_mid = painted_height(&fb, p, col_box(&g, "s", 1), bg);
    let h_max = painted_height(&fb, p, col_box(&g, "s", 2), bg);
    assert!(h_max > h_mid, "max bar paints taller than mid ({h_max} vs {h_mid})");
    assert!((h_max as f32 - p.height).abs() < p.height * 0.2, "max bar ~= plot height ({h_max} vs {})", p.height);
    assert!((h_mid as f32 - p.height * 0.5).abs() < p.height * 0.25, "mid bar ~= half plot ({h_mid} vs {})", p.height * 0.5);
}

/// NO-FAKE-GREEN: bars rescale with the box. The max bar paints ~full plot height
/// in BOTH a short and a tall plot — extent tracks the laid-out box, not a const.
#[test]
fn bars_rescale_with_box() {
    let mk = |css: &str| {
        let mut g = Gallery::new(300, 300, css);
        g.mount("s", Box::new(Sparkline::bars(vec![2.0, 4.0, 8.0])));
        g.relayout();
        let p = plot(&g, "s");
        let fb = g.rasterize();
        let h = painted_height(&fb, p, col_box(&g, "s", 2), Color::new(0, 0, 0, 0));
        (p.height, h)
    };
    let (ph_small, bh_small) = mk("lq-gallery{padding:8px;} lq-sparkline{width:200px;height:40px;}");
    let (ph_big, bh_big) = mk("lq-gallery{padding:8px;} lq-sparkline{width:200px;height:160px;}");
    assert!(ph_big > ph_small * 3.0, "precondition: taller plot");
    assert!(bh_big as f32 > bh_small as f32 * 2.5, "max bar painted extent scales with the plot ({bh_small} -> {bh_big})");
}

/// NO-FAKE-GREEN: different data produces different painted geometry.
#[test]
fn different_data_differs() {
    let css = "lq-gallery{padding:8px;} lq-sparkline{width:200px;height:80px;}";
    let mut a = Gallery::new(W, H, css);
    a.mount("s", Box::new(Sparkline::bars(vec![1.0, 2.0, 9.0])));
    a.relayout();
    let pa = plot(&a, "s");
    let fa = a.rasterize();
    let a2 = painted_height(&fa, pa, col_box(&a, "s", 2), Color::new(0, 0, 0, 0));

    let mut b = Gallery::new(W, H, css);
    b.mount("s", Box::new(Sparkline::bars(vec![9.0, 2.0, 1.0])));
    b.relayout();
    let pb = plot(&b, "s");
    let fb = b.rasterize();
    let b2 = painted_height(&fb, pb, col_box(&b, "s", 2), Color::new(0, 0, 0, 0));
    assert!((a2 as i64 - b2 as i64).abs() > 10, "reversed data -> different bar-2 painted height ({a2} vs {b2})");
}

/// A line sparkline paints its points at data-proportional heights: the low-value
/// point paints lower on screen than the high-value point.
#[test]
fn line_points_track_data() {
    let mut g = Gallery::new(W, H, "lq-gallery{padding:8px;} lq-sparkline{width:200px;height:80px;}");
    g.mount("s", Box::new(Sparkline::line(vec![0.0, 10.0])));
    g.relayout();
    let p = plot(&g, "s");
    let fb = g.rasterize();
    // Point 0 (value 0) paints near the bottom; point 1 (value 10) near the top.
    let find_y = |col: liquide_layout::geometry::Rect| -> Option<u32> {
        let x = (col.x + col.width / 2.0) as u32;
        for y in (p.y as u32)..((p.y + p.height) as u32) {
            let px = fb.get_pixel(x, y);
            if px != Color::new(0, 0, 0, 0) && px.a > 0 {
                return Some(y);
            }
        }
        None
    };
    let y0 = find_y(col_box(&g, "s", 0)).expect("point 0 paints");
    let y1 = find_y(col_box(&g, "s", 1)).expect("point 1 paints");
    assert!(y0 > y1, "low value point paints lower on screen ({y0} vs {y1})");
}

/// The sparkline is display-only.
#[test]
fn sparkline_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(Sparkline::bars(vec![1.0, 2.0, 3.0])));
    g.relayout();
    let p = plot(&g, "s");
    g.left_click(p.x + p.width / 2.0, p.y + p.height / 2.0);
    assert!(g.process().is_empty());
}

// ── Added: display-only styling coverage (no fake-green) ─────────────────────
//
// The sparkline has NO interactive states (no hover/active/focus/checked/
// disabled); `sparkline_is_inert` confirms it ignores input. Below we prove the
// painted MARK styles (accent bars vs thin line stems) rasterize distinctly and
// that the mode (bar vs line) changes the painted geometry.

/// A bar paints the accent (blue-dominant) fill (CSS `lq-spark-bar` background:
/// accent). Sample near the bottom of a tall bar where it is certainly painted.
#[test]
fn bar_paints_accent_color() {
    let mut g = Gallery::new(W, H, "lq-gallery{padding:8px;} lq-sparkline{width:200px;height:80px;}");
    g.mount("s", Box::new(Sparkline::bars(vec![10.0, 10.0, 10.0])));
    g.relayout();
    let p = plot(&g, "s");
    let c1 = col_box(&g, "s", 1);
    let fb = g.rasterize();
    // Just above the plot bottom, inside the bar's 15%..85% horizontal band.
    let x = (c1.x + c1.width / 2.0) as u32;
    let y = (p.y + p.height - 3.0) as u32;
    let px = fb.get_pixel(x, y);
    assert!(px.a > 0, "the bar must paint (alpha {})", px.a);
    assert!(px.b > px.r, "the bar fill is the blue accent (got {px:?})");
}

/// Bar vs line MODE paints a different mark for the SAME data: a bar fills a wide
/// (70%-width) column band, while a line stem is a thin (30%-width) stem. At the
/// column's far-left interior (~18% across), a bar paints but a line stem does not.
#[test]
fn bar_vs_line_mode_paint_differently() {
    let probe = |g: &mut Gallery| -> liquide_compositor::pixel::Color {
        let p = plot(g, "s");
        let c1 = col_box(g, "s", 1);
        let fb = g.rasterize();
        // x at ~18% across the column: inside a bar's 15%-85% band but OUTSIDE a
        // line stem's 35%-65% band.
        let x = (c1.x + c1.width * 0.18) as u32;
        let y = (p.y + p.height - 3.0) as u32;
        fb.get_pixel(x, y)
    };
    let css = "lq-gallery{padding:8px;} lq-sparkline{width:200px;height:80px;}";
    let mut bar = Gallery::new(W, H, css);
    bar.mount("s", Box::new(Sparkline::bars(vec![10.0, 10.0, 10.0])));
    bar.relayout();
    let bar_px = probe(&mut bar);

    let mut line = Gallery::new(W, H, css);
    line.mount("s", Box::new(Sparkline::line(vec![10.0, 10.0, 10.0])));
    line.relayout();
    let line_px = probe(&mut line);

    assert!(bar_px.a > 0, "the wide bar paints at 18% across (got {bar_px:?})");
    assert!(
        bar_px != line_px,
        "bar and line modes paint different marks at 18% across the column (bar {bar_px:?} line {line_px:?})"
    );
}

/// A single data point still renders one full-width column spanning the plot — the
/// flex layout does not divide by zero or collapse.
#[test]
fn single_datum_spans_plot() {
    let mut g = Gallery::new(W, H, "lq-gallery{padding:8px;} lq-sparkline{width:200px;height:80px;}");
    g.mount("s", Box::new(Sparkline::bars(vec![5.0])));
    g.relayout();
    let p = plot(&g, "s");
    let c0 = col_box(&g, "s", 0);
    assert!(
        (c0.width - p.width).abs() < p.width * 0.1,
        "a single column spans ~the whole plot ({} vs {})",
        c0.width,
        p.width
    );
}
