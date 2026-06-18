//! `<lq-donut-chart>` (+ pie) real-pipeline gallery tests.
#![cfg(test)]

use crate::donut_chart::{DonutChart, Segment, HOVER_ACTION};
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;

const W: u32 = 280;
const H: u32 = 280;

fn as_chart(g: &Gallery) -> &DonutChart {
    g.host.behavior("c").unwrap().as_any().downcast_ref::<DonutChart>().unwrap()
}

fn disc(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "disc").expect("disc box")
}

fn segs() -> Vec<Segment> {
    // Equal quarters: 4 segments of value 1 each -> 90deg apiece.
    vec![
        Segment::new("N", 1.0),
        Segment::new("E", 1.0),
        Segment::new("S", 1.0),
        Segment::new("W", 1.0),
    ]
}

/// The donut renders a real disc that paints (the conic gradient fills it).
#[test]
fn donut_renders() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("c", Box::new(DonutChart::donut(segs())));
    g.relayout();
    let d = disc(&g);
    assert!(d.width > 120.0 && d.height > 120.0, "disc from CSS (got {}x{})", d.width, d.height);
    let fb = g.rasterize();
    // Sample partway out from center at ~45deg (well inside segment 0, past the
    // donut hole, away from any segment boundary).
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    let r = d.width.min(d.height) / 2.0;
    let rad = 45.0f32.to_radians();
    let px = Gallery::pixel(&fb, (cx + rad.sin() * r * 0.8) as u32, (cy - rad.cos() * r * 0.8) as u32);
    assert!(px.a > 0, "ring paints");
}

/// NO-FAKE-GREEN: segment angular spans are proportional to the data. Unequal
/// values produce unequal spans, which is observable via the hover hit-test: a
/// pointer in a wide segment's angular range hits it, while the same range would
/// belong to a different segment under equal spans.
#[test]
fn spans_proportional_drive_hit_test() {
    // Segment 0 takes 3/4 of the circle (270deg), segments 1..3 share the rest.
    let weighted = vec![
        Segment::new("big", 6.0),
        Segment::new("a", 1.0),
        Segment::new("b", 1.0),
    ];
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::pie(weighted)));
    g.relayout();
    let d = disc(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    let r = d.width.min(d.height) / 2.0;

    // 180deg (6 o'clock / straight down) lies inside the big 0..270deg segment.
    g.pointer_move(cx, cy + r * 0.6);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(0), "the big segment owns the bottom (got {:?})", as_chart(&g).hovered());
}

/// NO-FAKE-GREEN: hover resolves the segment from the pointer's angle about the
/// LAID-OUT disc center. For equal quarters: right(3 o'clock)->seg1(E),
/// bottom(6)->seg2(S), left(9)->seg3(W), top(12)->seg0(N).
#[test]
fn hover_resolves_segment_from_layout_angle() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::pie(segs())));
    g.relayout();
    let d = disc(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    let r = d.width.min(d.height) / 2.0;
    let probe = r * 0.6;

    // 3 o'clock (right) -> 90deg -> segment 1 (E).
    g.pointer_move(cx + probe, cy);
    let a = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(1), "right -> seg 1");
    assert!(a.iter().any(|x| x.name == HOVER_ACTION && x.payload.as_deref() == Some("1,E,1")), "payload index,label,value (got {a:?})");

    // 6 o'clock (down) -> 180deg -> segment 2 (S).
    g.pointer_move(cx, cy + probe);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(2), "down -> seg 2");

    // 9 o'clock (left) -> 270deg -> segment 3 (W).
    g.pointer_move(cx - probe, cy);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(3), "left -> seg 3");
}

/// NO-FAKE-GREEN (anti-constant): the donut HOLE rejects center hits — a press at
/// the exact center resolves NO segment, because the hit is measured against the
/// laid-out radius/hole, not a constant. (A pie has no hole, so its center DOES
/// hit a segment — proving the geometry is consulted.)
#[test]
fn donut_hole_rejects_center_pie_does_not() {
    let mut donut = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    donut.mount("c", Box::new(DonutChart::donut(segs()).hole(0.6)));
    donut.relayout();
    let d = disc(&donut);
    let (cx, cy) = (d.x + d.width / 2.0, d.y + d.height / 2.0);
    donut.pointer_move(cx, cy);
    let _ = donut.process();
    assert_eq!(as_chart(&donut).hovered(), None, "donut center is in the hole -> no hit");

    let mut pie = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    pie.mount("c", Box::new(DonutChart::pie(segs())));
    pie.relayout();
    let pd = disc(&pie);
    let (pcx, pcy) = (pd.x + pd.width / 2.0, pd.y + pd.height / 2.0);
    // A hair off-center so the angle is well-defined; still well inside the disc.
    pie.pointer_move(pcx + 2.0, pcy + 2.0);
    let _ = pie.process();
    assert!(as_chart(&pie).hovered().is_some(), "pie center hits a segment (no hole)");
}

/// NO-FAKE-GREEN: hits are measured against the laid-out radius — a point OUTSIDE
/// the disc radius is rejected, even within the chart's bounding box corner.
#[test]
fn outside_radius_rejected() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::pie(segs())));
    g.relayout();
    let d = disc(&g);
    // Top-left corner of the bounding box is outside the inscribed circle.
    g.pointer_move(d.x + 4.0, d.y + 4.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), None, "corner is outside the radius -> no hit");
}

/// NO-FAKE-GREEN: different data produces different rendered geometry. The
/// segment-boundary spokes sit at the data-driven span angles, so reversing the
/// value split moves them — and the painted ring differs. (The fill is shown via
/// spokes + the hovered wedge because the renderer can't stack clip-path fills.)
#[test]
fn different_data_renders_differently() {
    let mut a = Gallery::new(W, H, "lq-gallery{padding:12px;} lq-donut-chart{width:200px;height:200px;}");
    a.mount("c", Box::new(DonutChart::pie(vec![Segment::new("x", 1.0), Segment::new("y", 9.0)])));
    a.relayout();
    let mut b = Gallery::new(W, H, "lq-gallery{padding:12px;} lq-donut-chart{width:200px;height:200px;}");
    b.mount("c", Box::new(DonutChart::pie(vec![Segment::new("x", 9.0), Segment::new("y", 1.0)])));
    b.relayout();
    let da = disc(&a);
    let fb_a = a.rasterize();
    let fb_b = b.rasterize();
    let cx = da.x + da.width / 2.0;
    let cy = da.y + da.height / 2.0;
    let r = da.width.min(da.height) / 2.0;
    // Densely scan the ring; the boundary spoke moves with the data so the painted
    // rings must differ at a meaningful number of points.
    let mut diffs = 0;
    for k in 0..180 {
        let deg = k as f32 * 2.0;
        let rad = deg.to_radians();
        let x = (cx + rad.sin() * r * 0.6) as u32;
        let y = (cy - rad.cos() * r * 0.6) as u32;
        if Gallery::pixel(&fb_a, x, y) != Gallery::pixel(&fb_b, x, y) {
            diffs += 1;
        }
    }
    assert!(diffs > 0, "different value splits -> the boundary spokes move (diffs={diffs})");
}

/// NO-FAKE-GREEN: every segment is FULLY FILLED with its own colour (not a single
/// base ring + spokes). Four equal quarters in four distinct palette colours: each
/// quadrant samples its segment's colour, and the four are all different — proving
/// N stacked clip-path wedges paint as N filled slices with no leak.
#[test]
fn every_segment_is_filled_with_its_colour() {
    let colored = vec![
        Segment::new("a", 1.0).color("#ff0000"), // red,  top-right quadrant (45deg)
        Segment::new("b", 1.0).color("#00ff00"), // green, bottom-right (135deg)
        Segment::new("c", 1.0).color("#0000ff"), // blue, bottom-left (225deg)
        Segment::new("d", 1.0).color("#ffff00"), // yellow, top-left (315deg)
    ];
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::pie(colored)));
    g.relayout();
    let d = disc(&g);
    let fb = g.rasterize();
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    let r = d.width.min(d.height) / 2.0 * 0.65;
    let samp = |deg: f32| {
        let rad = deg.to_radians();
        Gallery::pixel(&fb, (cx + rad.sin() * r) as u32, (cy - rad.cos() * r) as u32)
    };
    let tr = samp(45.0);
    let br = samp(135.0);
    let bl = samp(225.0);
    let tl = samp(315.0);
    // Each wedge shows its own colour (filled), and all four differ (no leak/overpaint).
    assert!(tr.r > 150 && tr.g < 100 && tr.b < 100, "segment a (red) fills TR quadrant (got {tr:?})");
    assert!(br.g > 150 && br.r < 100 && br.b < 100, "segment b (green) fills BR quadrant (got {br:?})");
    assert!(bl.b > 150 && bl.r < 100 && bl.g < 100, "segment c (blue) fills BL quadrant (got {bl:?})");
    assert!(tl.r > 150 && tl.g > 150 && tl.b < 100, "segment d (yellow) fills TL quadrant (got {tl:?})");
}

/// NO-FAKE-GREEN: a donut shows a CENTER LABEL with the total (then the hovered
/// value). The label text is reconciled into the DOM under the hole.
#[test]
fn donut_center_label_shows_total_then_hovered() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::donut(vec![
        Segment::new("a", 3.0),
        Segment::new("b", 7.0),
    ])));
    g.relayout();
    let center_text = |g: &Gallery| -> Option<String> {
        let root = g.host.root_of("c").unwrap();
        fn find(doc: &liquide_dom::Document, n: liquide_dom::NodeId) -> Option<String> {
            if doc.get_attribute(n, "data-part").as_deref() == Some("center") {
                let mut s = String::new();
                for &c in doc.children(n) {
                    if let Some(t) = doc.get(c).and_then(|node| node.text_content()) {
                        s.push_str(t);
                    }
                }
                return Some(s);
            }
            for &c in doc.children(n) {
                if let Some(f) = find(doc, c) { return Some(f); }
            }
            None
        }
        find(g.doc(), root)
    };
    assert_eq!(center_text(&g).as_deref(), Some("10"), "center shows total (3+7)");
    // Hover segment 1 -> center shows its value.
    let d = disc(&g);
    let (cx, cy) = (d.x + d.width / 2.0, d.y + d.height / 2.0);
    let r = d.width.min(d.height) / 2.0;
    // Segment 0 is 3/10 (0..108deg); segment 1 is 7/10 (108..360). 270deg (left)
    // is inside segment 1.
    g.pointer_move(cx - r * 0.8, cy);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), Some(1), "left hits the big segment 1");
    assert_eq!(center_text(&g).as_deref(), Some("7"), "center shows the hovered value");
}

/// Leaving clears the hover.
#[test]
fn mouse_leave_clears_hover() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-donut-chart { width: 200px; height: 200px; }");
    g.mount("c", Box::new(DonutChart::pie(segs())));
    g.relayout();
    let d = disc(&g);
    g.pointer_move(d.x + d.width * 0.8, d.y + d.height / 2.0);
    let _ = g.process();
    assert!(as_chart(&g).hovered().is_some());
    g.pointer_move(2.0, 2.0);
    let _ = g.process();
    assert_eq!(as_chart(&g).hovered(), None);
}
