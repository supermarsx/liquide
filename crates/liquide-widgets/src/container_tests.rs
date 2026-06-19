//! `<lq-panel>` / `<lq-card>` / `<lq-group-box>` real-pipeline gallery tests.
//!
//! Containers are static, so the teeth here are: they RENDER a real paint box at
//! the CSS-driven geometry (not a Rust constant), they slot children into the
//! right region (located by `data-part`), and a CSS change moves the box — proving
//! the geometry is layout-derived.
#![cfg(test)]

use liquide_components::template::TemplateNode;

use crate::container::{Card, GroupBox, Panel};
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;

const W: u32 = 360;
const H: u32 = 280;

/// A panel renders a real paint box and slots its child text.
#[test]
fn panel_renders_paint_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-panel { width: 200px; }");
    g.mount("p", Box::new(Panel::new().text("Hello")));
    g.relayout();
    let node = g.host.root_of("p").unwrap();
    let r = g.box_of(node).expect("panel laid out");
    assert!((r.width - 200.0).abs() < 2.0, "panel width from CSS (got {})", r.width);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + 8.0) as u32, (r.y + 8.0) as u32);
    assert!(px.a > 0, "panel must paint a surface");
}

/// The NO-FAKE-GREEN tooth: a CSS-widened panel lays out at the CSS width — the
/// geometry is layout-derived, not a constant.
#[test]
fn panel_box_comes_from_css_not_constant() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-panel { width: 311px; }");
    g.mount("p", Box::new(Panel::new().text("x")));
    g.relayout();
    let r = g.box_of(g.host.root_of("p").unwrap()).unwrap();
    assert!((r.width - 311.0).abs() < 2.0, "panel must track the CSS width (got {})", r.width);
}

/// A card emits header / body / footer regions, locatable by data-part, each
/// laid out as a real box.
#[test]
fn card_has_header_body_footer_regions() {
    let card = Card::new()
        .header_text("Title")
        .child(TemplateNode::text("Body content"))
        .footer_text("Footer");
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-card { width: 240px; }");
    g.mount("c", Box::new(card));
    g.relayout();

    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let header = q.box_of_part(root, "header").expect("header region");
    let body = q.box_of_part(root, "body").expect("body region");
    let footer = q.box_of_part(root, "footer").expect("footer region");

    // Regions stack top-to-bottom: header above body above footer.
    assert!(header.y < body.y, "header above body");
    assert!(body.y < footer.y, "body above footer");
    // The footer's border-top draws a divider -> the body has a real extent.
    assert!(body.height > 0.0);
}

/// A card without header/footer omits those regions structurally.
#[test]
fn card_omits_absent_regions() {
    let card = Card::new().child(TemplateNode::text("Just a body"));
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("c", Box::new(card));
    g.relayout();
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.find_part(root, "body").is_some(), "body always present");
    assert!(q.find_part(root, "header").is_none(), "no header region");
    assert!(q.find_part(root, "footer").is_none(), "no footer region");
}

/// A group box renders a caption + a bordered content region, each a real box,
/// with the caption above the content.
#[test]
fn group_box_caption_and_content() {
    let gb = GroupBox::new("Options").text("inside the group");
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-group-box { width: 220px; }");
    g.mount("gb", Box::new(gb));
    g.relayout();

    let root = g.host.root_of("gb").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let caption = q.box_of_part(root, "caption").expect("caption box");
    let content = q.box_of_part(root, "content").expect("content box");
    assert!(caption.y <= content.y, "caption sits at/above the content region");

    let outer = g.box_of(root).unwrap();
    assert!((outer.width - 220.0).abs() < 2.0, "group box width from CSS (got {})", outer.width);
    let fb = g.rasterize();
    // The caption has a solid background — sample its centre to prove the group
    // box renders real ink.
    let px = Gallery::pixel(
        &fb,
        (caption.x + caption.width / 2.0) as u32,
        (caption.y + caption.height / 2.0) as u32,
    );
    assert!(px.a > 0, "group box caption surface must paint");
}

// ─────────────────────────────────────────────────────────────────────────
// STATE × STYLING coverage (pixel-delta, no-fake-green).
//
// NOTE: Panel / Card / GroupBox are STATIC containers — they want no events and
// are not focusable (see `containers_are_inert`), so they genuinely have NO
// interactive states (:hover / :active / :focus / :checked / :disabled). The
// teeth below therefore prove the BASE + VARIANT + structural border styling
// paints real, distinct ink. Each fails if its CSS rule were removed.
// ─────────────────────────────────────────────────────────────────────────

/// `normal` render: a panel's BORDER + box-shadow paint, and the `flush` variant
/// (which removes border + shadow) paints differently at the same edge. A panel
/// that ignored its border CSS would look identical to the flush one — so the
/// pixel delta proves the base border style is on the box.
#[test]
fn panel_border_differs_from_flush_variant() {
    // Two identically-sized panels: one default (border + shadow), one flush.
    let css = "lq-gallery { padding: 16px; } lq-panel { width: 120px; height: 60px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("p", Box::new(Panel::new().text("x")));
    g.relayout();
    let r = g.box_of(g.host.root_of("p").unwrap()).unwrap();
    // Sample the left border column at vertical mid-height.
    let (ex, ey) = (r.x as u32, (r.y + r.height / 2.0) as u32);
    let bordered = Gallery::pixel(&g.rasterize(), ex, ey);
    assert!(bordered.a > 0, "default panel must paint a border edge (alpha {})", bordered.a);

    let mut gf = Gallery::new(W, H, css);
    gf.mount("p", Box::new(Panel::new().variant("flush").text("x")));
    gf.relayout();
    let rf = gf.box_of(gf.host.root_of("p").unwrap()).unwrap();
    let flush = Gallery::pixel(&gf.rasterize(), rf.x as u32, (rf.y + rf.height / 2.0) as u32);
    assert!(
        bordered != flush,
        "default panel's border edge must differ from the flush variant's (bordered {bordered:?} flush {flush:?})"
    );
}

/// `normal` render: a card's HEADER region carries a bottom-border divider
/// (`lq-card > lq-card-header { border-bottom }`). The divider row (just above the
/// header/body seam) must paint, and differ from the header body interior — proving
/// the structural divider style paints, not just the surface fill.
#[test]
fn card_header_divider_paints() {
    let card = Card::new()
        .header_text("Title")
        .child(TemplateNode::text("Body content"));
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; } lq-card { width: 240px; }");
    g.mount("c", Box::new(card));
    g.relayout();
    let root = g.host.root_of("c").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let header = q.box_of_part(root, "header").expect("header region");

    let fb = g.rasterize();
    let cx = (header.x + header.width / 2.0) as u32;
    // The header surface interior (top of the header, away from the divider).
    let interior = Gallery::pixel(&fb, cx, (header.y + 3.0) as u32);
    // The divider row sits at the header's bottom border.
    let divider = Gallery::pixel(&fb, cx, (header.y + header.height - 1.0) as u32);
    assert!(divider.a > 0, "header divider must paint (alpha {})", divider.a);
    assert!(
        divider != interior,
        "header's bottom-border divider must differ from the header interior (divider {divider:?} interior {interior:?})"
    );
}

/// `::before`-style legend idiom: the group box CAPTION sits on the top border
/// with its own SOLID background (`lq-caption { background-color }`), overlapping
/// the box border via a negative top-margin. The caption fill must paint and
/// differ from the bordered content region just below it — proving the legend
/// surface (not just the box border) is styled.
#[test]
fn group_box_caption_fill_differs_from_content() {
    let gb = GroupBox::new("Options").text("inside the group");
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; } lq-group-box { width: 220px; height: 120px; }");
    g.mount("gb", Box::new(gb));
    g.relayout();
    let root = g.host.root_of("gb").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let caption = q.box_of_part(root, "caption").expect("caption box");
    let content = q.box_of_part(root, "content").expect("content box");

    let fb = g.rasterize();
    let cap_px = Gallery::pixel(
        &fb,
        (caption.x + caption.width / 2.0) as u32,
        (caption.y + caption.height / 2.0) as u32,
    );
    // A point inside the content region but clear of the caption strip.
    let body_px = Gallery::pixel(
        &fb,
        (content.x + content.width / 2.0) as u32,
        (content.y + content.height - 4.0) as u32,
    );
    assert!(cap_px.a > 0, "caption fill must paint (alpha {})", cap_px.a);
    assert!(
        cap_px != body_px,
        "the legend caption's solid fill must differ from the content region (caption {cap_px:?} content {body_px:?})"
    );
}

/// Containers are inert: no events wanted, not focusable.
#[test]
fn containers_are_inert() {
    use crate::behavior::WidgetBehavior;
    assert!(Panel::new().wanted_events().is_empty());
    assert!(!Panel::new().focusable());
    assert!(Card::new().wanted_events().is_empty());
    assert!(!Card::new().focusable());
    assert!(GroupBox::new("x").wanted_events().is_empty());
    assert!(!GroupBox::new("x").focusable());
}
