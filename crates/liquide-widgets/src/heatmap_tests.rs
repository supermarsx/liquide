//! `<lq-heatmap>` real-pipeline gallery tests.
//!
//! Cells are placed by flex columns (x, real layout) + `scaleY` with a per-row
//! `transform-origin` (vertical, paint-only). Cell positions are therefore
//! computed from the LAID-OUT plot box (rows/cols), and colour is asserted via
//! PIXELS at those computed centers. Hover resolves the cell from the same plot
//! box (grid math), never a constant.
#![cfg(test)]

use crate::gallery::Gallery;
use crate::heatmap::{Heatmap, HOVER_ACTION};
use crate::layout_query::LayoutQuery;

const W: u32 = 300;
const H: u32 = 300;

fn as_map(g: &Gallery) -> &Heatmap {
    g.host.behavior("c").unwrap().as_any().downcast_ref::<Heatmap>().unwrap()
}

fn plot(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "plot").expect("plot box")
}

/// The laid-out center of cell (r,c) computed from the plot box and grid dims.
fn cell_center(p: liquide_layout::geometry::Rect, rows: usize, cols: usize, r: usize, c: usize) -> (f32, f32) {
    let x = p.x + (c as f32 + 0.5) * p.width / cols as f32;
    let y = p.y + (r as f32 + 0.5) * p.height / rows as f32;
    (x, y)
}

fn map3x3() -> Heatmap {
    Heatmap::new(3, 3, (0..9).map(|v| v as f32).collect())
}

/// NO-FAKE-GREEN: the plot fills the chart box (rescales) and a cell paints.
#[test]
fn heatmap_renders() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-heatmap { width: 210px; height: 210px; }");
    g.mount("c", Box::new(map3x3()));
    g.relayout();
    let p = plot(&g);
    assert!((p.width - 210.0).abs() < 4.0 && (p.height - 210.0).abs() < 4.0, "plot fills box (got {}x{})", p.width, p.height);
    let (x, y) = cell_center(p, 3, 3, 2, 2);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, x as u32, y as u32);
    assert!(px.a > 0, "cell paints");
}

/// NO-FAKE-GREEN: cell colour encodes the value on the scale — the min-value cell
/// differs in colour from the max-value cell, sampled at their laid-out centers.
#[test]
fn cell_color_encodes_value() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-heatmap { width: 210px; height: 210px; }");
    g.mount("c", Box::new(map3x3())); // (0,0)=0 (min), (2,2)=8 (max)
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    let (lx, ly) = cell_center(p, 3, 3, 0, 0);
    let (hx, hy) = cell_center(p, 3, 3, 2, 2);
    let lo_px = Gallery::pixel(&fb, lx as u32, ly as u32);
    let hi_px = Gallery::pixel(&fb, hx as u32, hy as u32);
    assert!(lo_px != hi_px, "min and max cells differ in colour ({lo_px:?} vs {hi_px:?})");
}

/// NO-FAKE-GREEN: cells tile the laid-out plot — distinct cells paint distinct
/// colours at their computed centers, and the grid spans the whole plot. (A
/// fixed-pixel grid that ignored the box would mis-sample.)
#[test]
fn cells_tile_the_laid_out_plot() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-heatmap { width: 240px; height: 240px; }");
    // A gradient so each row/col is a different shade.
    let vals: Vec<f32> = (0..9).map(|v| v as f32).collect();
    g.mount("c", Box::new(Heatmap::new(3, 3, vals)));
    g.relayout();
    let p = plot(&g);
    let fb = g.rasterize();
    // Corner cells (0,0) and (2,2) sit at opposite ends of the plot and differ.
    let (x00, y00) = cell_center(p, 3, 3, 0, 0);
    let (x22, y22) = cell_center(p, 3, 3, 2, 2);
    assert!(x22 > x00 + p.width * 0.5, "(2,2) is far right of (0,0)");
    assert!(y22 > y00 + p.height * 0.5, "(2,2) is far below (0,0)");
    assert!(Gallery::pixel(&fb, x00 as u32, y00 as u32) != Gallery::pixel(&fb, x22 as u32, y22 as u32), "corner cells differ");
}

/// NO-FAKE-GREEN: hover resolves the cell from the LAID-OUT plot grid. Moving over
/// each cell's computed center maps to its (row,col), with the value in the action.
#[test]
fn hover_resolves_cell_from_layout() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-heatmap { width: 210px; height: 210px; }");
    g.mount("c", Box::new(map3x3()));
    g.relayout();
    let p = plot(&g);

    let (x, y) = cell_center(p, 3, 3, 1, 2);
    g.pointer_move(x, y);
    let a = g.process();
    assert_eq!(as_map(&g).hovered(), Some((1, 2)), "center of (1,2) maps to (1,2)");
    // value at (1,2) = 1*3+2 = 5.
    assert!(a.iter().any(|x| x.name == HOVER_ACTION && x.payload.as_deref() == Some("1,2,5")), "payload row,col,value (got {a:?})");

    let (x0, y0) = cell_center(p, 3, 3, 0, 0);
    g.pointer_move(x0, y0);
    let _ = g.process();
    assert_eq!(as_map(&g).hovered(), Some((0, 0)), "center of (0,0) maps to (0,0)");
}

/// NO-FAKE-GREEN (anti-constant): the same fractional pointer resolves to the same
/// (row,col) regardless of pixel size — the grid math uses the laid-out plot. 0.5,
/// 0.5 -> center cell (1,1) for a 120px AND a 270px plot.
#[test]
fn hover_cell_follows_plot_size() {
    let mk = |s: &str| {
        let css = format!("lq-gallery{{padding:12px;}} lq-heatmap{{width:{s};height:{s};}}");
        let mut g = Gallery::new(360, 360, &css);
        g.mount("c", Box::new(map3x3()));
        g.relayout();
        g
    };
    let mut small = mk("120px");
    let ps = plot(&small);
    small.pointer_move(ps.x + ps.width * 0.5, ps.y + ps.height * 0.5);
    let _ = small.process();
    assert_eq!(as_map(&small).hovered(), Some((1, 1)), "center of small plot -> (1,1)");

    let mut big = mk("270px");
    let pb = plot(&big);
    big.pointer_move(pb.x + pb.width * 0.5, pb.y + pb.height * 0.5);
    let _ = big.process();
    assert_eq!(as_map(&big).hovered(), Some((1, 1)), "center of big plot -> (1,1) too");
    big.pointer_move(pb.x + pb.width * 0.85, pb.y + pb.height * 0.15);
    let _ = big.process();
    assert_eq!(as_map(&big).hovered(), Some((0, 2)), "0.85x,0.15y -> col 2, row 0");
}

/// Leaving the grid clears the hover.
#[test]
fn mouse_leave_clears_hover() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-heatmap { width: 210px; height: 210px; }");
    g.mount("c", Box::new(map3x3()));
    g.relayout();
    let p = plot(&g);
    let (x, y) = cell_center(p, 3, 3, 1, 1);
    g.pointer_move(x, y);
    let _ = g.process();
    assert!(as_map(&g).hovered().is_some());
    g.pointer_move(2.0, 2.0);
    let _ = g.process();
    assert_eq!(as_map(&g).hovered(), None);
}

/// Different data produces different cell colours at the same center.
#[test]
fn different_data_differs() {
    let css = "lq-gallery{padding:12px;} lq-heatmap{width:210px;height:210px;}";
    let mut a = Gallery::new(W, H, css);
    a.mount("c", Box::new(Heatmap::new(2, 2, vec![0.0, 1.0, 2.0, 3.0])));
    a.relayout();
    let pa = plot(&a);
    let (ax, ay) = cell_center(pa, 2, 2, 0, 0);
    let pa_px = Gallery::pixel(&a.rasterize(), ax as u32, ay as u32);

    let mut b = Gallery::new(W, H, css);
    b.mount("c", Box::new(Heatmap::new(2, 2, vec![3.0, 2.0, 1.0, 0.0])));
    b.relayout();
    let pb_px = Gallery::pixel(&b.rasterize(), ax as u32, ay as u32);
    assert!(pa_px != pb_px, "swapped data -> different (0,0) colour ({pa_px:?} vs {pb_px:?})");
}
