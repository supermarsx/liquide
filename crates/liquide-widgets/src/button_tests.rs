//! `<lq-button>` real-pipeline gallery tests (no fake-green).
//!
//! Every test drives the REAL style -> layout -> paint pipeline + the REAL event
//! dispatcher through [`Gallery`](crate::gallery::Gallery): render produces a
//! paint box; a scripted click on the LAID-OUT box fires the Action; keyboard
//! activates; disabled swallows; :hover restyles the actual pixels.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::button::Button;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 320;
const H: u32 = 200;

fn gallery_with(btn: Button) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    g.mount("btn", Box::new(btn));
    g.relayout();
    g
}

fn center(g: &Gallery) -> (f32, f32) {
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).expect("button laid out");
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

fn as_button(g: &Gallery) -> &Button {
    g.host
        .behavior("btn")
        .unwrap()
        .as_any()
        .downcast_ref::<Button>()
        .unwrap()
}

/// Renders a real paint box at the CSS-driven size.
#[test]
fn button_renders_paint_box_from_css() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).expect("button must lay out");
    // widgets.css sets the 120px width — Rust does not. (Height grows to fit the
    // label content in the current block/flex model; the load-bearing CSS-driven
    // dimension here is the width.)
    assert!(
        (r.width - 120.0).abs() < 2.0,
        "button width must come from CSS (got {})",
        r.width
    );
    assert!(r.height > 0.0 && r.height < 120.0, "button height sane (got {})", r.height);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(px.a > 0, "button must paint (alpha {})", px.a);
}

/// A scripted click on the laid-out box fires the button's Action.
#[test]
fn click_fires_action() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let (cx, cy) = center(&g);
    g.left_click(cx, cy);
    let actions = g.process();
    assert_eq!(actions.len(), 1, "one action from the click");
    assert_eq!(actions[0].name, "confirm");
    assert_eq!(as_button(&g).activations(), 1);
}

/// A click OUTSIDE the laid-out box does NOT fire (geometry from layout).
#[test]
fn click_outside_box_does_not_fire() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    let ox = r.x + r.width + 20.0;
    assert!((ox as u32) < W);
    g.left_click(ox, r.y + r.height / 2.0);
    let actions = g.process();
    assert!(actions.is_empty(), "click past the box must not fire");
    assert_eq!(as_button(&g).activations(), 0);
}

/// The NO-FAKE-GREEN tooth: a CSS-widened button accepts a click a 120px constant
/// would reject — proving the hit-test reads the laid-out box.
#[test]
fn hit_geometry_comes_from_layout_not_constant() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; } lq-button { width: 197px; }");
    g.mount("btn", Box::new(Button::new("Wide", "go")));
    g.relayout();
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    assert!((r.width - 197.0).abs() < 2.0, "precondition: 197px (got {})", r.width);

    // x inside the real 197px box but outside a 120px assumption (20..140).
    let x = r.x + 170.0;
    g.left_click(x, r.y + r.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1, "click in the REAL wide box must fire (x={x})");
}

/// Disabled buttons swallow the click and emit nothing.
#[test]
fn disabled_button_swallows_click() {
    let mut g = gallery_with(Button::new("No", "confirm").disabled(true));
    let (cx, cy) = center(&g);
    g.left_click(cx, cy);
    let actions = g.process();
    assert!(actions.is_empty(), "disabled button must not fire");
    assert_eq!(as_button(&g).activations(), 0);
    // And it drops out of the focus ring.
    assert!(!as_button(&g).focusable());
}

/// Enter / Space activate the focused button (keyboard a11y).
#[test]
fn keyboard_enter_and_space_activate() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    g.host.set_focus(Some("btn"), &mut g.doc, &mut g.dispatcher);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1, "Enter activates");
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a.len(), 1, "Space activates");
    assert_eq!(as_button(&g).activations(), 2);

    // A non-activating key does nothing.
    let a = g.key(KeyInput::new('x' as u32, 0));
    assert!(a.is_empty());
}

/// :hover restyles the actual rasterized pixels (CSS round-trips the state).
#[test]
fn hover_restyles_pixels() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    let (cx, cy) = ((r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);

    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    let (fx, fy) = center(&g);
    g.pointer_move(fx, fy);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(before != after, "hover must restyle (before {before:?} after {after:?})");
    assert!(as_button(&g).is_hovered());
}

// ── Added: deeper visual-STATE pixel-delta coverage (no fake-green) ──────────

/// :active (mouse held DOWN, no up) restyles the fill to the accent — the button
/// root carries the ACTIVE pseudo (via `pressed`) so CSS `lq-button:active` lands
/// in the rasterized center pixel.
#[test]
fn active_restyles_pixels() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    let (cx, cy) = ((r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);

    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    // Press WITHOUT releasing: drives MouseDown -> pressed=true -> :active.
    g.mouse_down(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(as_button(&g).is_pressed(), "mouse-down must set pressed/:active");
    assert!(
        before != after,
        ":active must restyle the fill (before {before:?} after {after:?})"
    );
}

/// :focus restyles the BORDER (focus ring) — sample just inside the border edge
/// (not the center, which the focus rule does not touch). Focus is applied via the
/// dispatcher (set_focus sets the FOCUS pseudo on the root); no re-render follows
/// so it survives into the rasterize.
#[test]
fn focus_restyles_border_pixels() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    // Sample on the top border line, a couple px in from the corner.
    let (bx, by) = ((r.x + 6.0) as u32, (r.y + 0.0) as u32);

    let before = Gallery::pixel(&g.rasterize(), bx, by);
    g.host.set_focus(Some("btn"), &mut g.doc, &mut g.dispatcher);
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        before != after,
        ":focus must restyle the border ring (before {before:?} after {after:?})"
    );
}

/// The `.primary` variant paints a distinct (accent) fill vs the default button —
/// proving the variant class round-trips through the real style pipeline.
#[test]
fn primary_variant_paints_accent_fill() {
    let mut g_def = gallery_with(Button::new("OK", "go"));
    let def_px = {
        let node = g_def.host.root_of("btn").unwrap();
        let r = g_def.box_of(node).unwrap();
        Gallery::pixel(&g_def.rasterize(), (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
    };

    let mut g_pri = gallery_with(Button::new("OK", "go").variant("primary"));
    let pri_px = {
        let node = g_pri.host.root_of("btn").unwrap();
        let r = g_pri.box_of(node).unwrap();
        Gallery::pixel(&g_pri.rasterize(), (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
    };

    assert!(
        def_px != pri_px,
        ".primary must paint a distinct accent fill (default {def_px:?} primary {pri_px:?})"
    );
    // The accent fill is the macOS-dark GRAPHITE accent (~#8e8e93): a bright,
    // near-neutral gray, distinctly lighter than the default button's dark
    // surface (post-retheme successor to the old blue-dominant check).
    assert!(
        Gallery::is_graphite_accent(pri_px),
        "primary fill must be the bright graphite accent (got {pri_px:?})"
    );
}

// ---- optional icon (data-icon leaf forwarding) ---------------------------

/// A button WITH an icon emits a dedicated `lq-button-icon` leaf carrying
/// `data-icon="<name>"` BEFORE the label — and that name resolves to a NON-ZERO
/// IconId through the real paint name-map (the `icon_id > 0` gate the painter
/// uses to decide whether a glyph is drawn at all).
#[test]
fn button_with_icon_emits_data_icon_leaf_before_label() {
    let tree = Button::new("Back", "go").icon("go-previous").render();

    // The FIRST child is the icon leaf carrying the icon name.
    let icon = &tree.children[0];
    assert_eq!(icon.tag, "lq-button-icon", "first child is the icon slot");
    let name = icon
        .attrs
        .iter()
        .find(|(k, _)| k == "data-icon")
        .map(|(_, v)| v.as_str());
    assert_eq!(name, Some("go-previous"), "leaf carries the icon name");

    // The emitted name resolves to a REAL, non-zero glyph id (not just any
    // string): exactly what the painter gates on before drawing.
    assert!(
        liquide_paint::icons::icon_id_for_name("go-previous") > 0,
        "data-icon name must resolve to a non-zero IconId in the paint name-map"
    );

    // The label element follows the icon (icon strictly BEFORE the label).
    let icon_pos = tree
        .children
        .iter()
        .position(|c| c.tag == "lq-button-icon")
        .expect("icon child");
    let label_pos = tree
        .children
        .iter()
        .position(|c| c.tag == "lq-label")
        .expect("label child");
    assert!(icon_pos < label_pos, "icon leaf must precede the label");
}

/// A button WITHOUT an icon (`None`) emits NO `data-icon` leaf — it renders
/// label-only, exactly as before this feature (no phantom slot).
#[test]
fn button_without_icon_emits_no_data_icon_leaf() {
    let tree = Button::new("OK", "go").render();
    assert!(
        tree.children.iter().all(|c| c.tag != "lq-button-icon"),
        "an icon-less button must not emit a data-icon leaf"
    );
    // The label is still present.
    assert!(
        tree.children.iter().any(|c| c.tag == "lq-label"),
        "icon-less button still shows its label"
    );
}

/// REAL-LAYOUT guard: an iconed button lays the icon to the LEFT of the label,
/// both on the SAME row (vertically centered) — NOT stacked. Reads the laid-out
/// boxes through the real pipeline + LayoutQuery.
#[test]
fn iconed_button_lays_icon_left_of_label_same_row() {
    let mut g = gallery_with(Button::new("Back", "go").icon("go-previous"));
    let root = g.host.root_of("btn").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let doc = g.doc();

    let mut icon = None;
    let mut label = None;
    for &child in doc.children(root) {
        match doc.tag_name(child).as_deref() {
            Some("lq-button-icon") => icon = q.box_of(child),
            Some("lq-label") => label = q.box_of(child),
            _ => {}
        }
    }
    let icon = icon.expect("iconed button carries an icon slot with a laid-out box");
    let label = label.expect("button label box present");

    // Icon strictly to the LEFT of the label (icon-beside-label, not stacked).
    assert!(
        icon.x + icon.width <= label.x + 0.5,
        "icon must sit LEFT of the label (icon right edge {} <= label left {})",
        icon.x + icon.width,
        label.x
    );

    // Same ROW: the vertical extents OVERLAP (a stacked column would not).
    let overlap = icon.y < label.y + label.height && label.y < icon.y + icon.height;
    assert!(
        overlap,
        "icon and label must share a row (y-overlap): icon y[{}..{}] label y[{}..{}]",
        icon.y,
        icon.y + icon.height,
        label.y,
        label.y + label.height
    );

    // The icon slot keeps a consistent, undistorted 16x16 size in the flex row.
    assert!(
        (icon.width - 16.0).abs() < 1.0 && (icon.height - 16.0).abs() < 1.0,
        "icon slot stays 16x16 (got {}x{})",
        icon.width,
        icon.height
    );
}

/// The `.danger` variant paints a distinct (red) fill vs the default button.
#[test]
fn danger_variant_paints_red_fill() {
    let mut g_def = gallery_with(Button::new("OK", "go"));
    let def_px = {
        let node = g_def.host.root_of("btn").unwrap();
        let r = g_def.box_of(node).unwrap();
        Gallery::pixel(&g_def.rasterize(), (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
    };

    let mut g_dgr = gallery_with(Button::new("OK", "go").variant("danger"));
    let dgr_px = {
        let node = g_dgr.host.root_of("btn").unwrap();
        let r = g_dgr.box_of(node).unwrap();
        Gallery::pixel(&g_dgr.rasterize(), (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
    };

    assert!(def_px != dgr_px, ".danger must restyle the fill");
    assert!(
        dgr_px.r > dgr_px.b,
        "danger fill must be red-dominant (got {dgr_px:?})"
    );
}
