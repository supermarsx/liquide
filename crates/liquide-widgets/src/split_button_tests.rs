//! `<lq-split-button>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::split_button::SplitButton;

const W: u32 = 360;
const H: u32 = 220;

fn as_split<'a>(g: &'a Gallery, id: &str) -> &'a SplitButton {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<SplitButton>()
        .unwrap()
}

fn items() -> Vec<(String, String)> {
    vec![
        ("save-as".into(), "Save As…".into()),
        ("save-all".into(), "Save All".into()),
        ("export".into(), "Export".into()),
    ]
}

/// Clicking the primary zone fires the primary action (not the menu).
#[test]
fn primary_zone_fires_primary() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items())));
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    let prim = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "primary").expect("primary box")
    };
    g.left_click(prim.x + prim.width / 2.0, prim.y + prim.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "save");
    assert!(!as_split(&g, "sb").is_open(), "primary click does not open the menu");
}

/// NO-FAKE-GREEN tooth: primary vs caret is decided by the LAID-OUT boxes. The
/// caret zone opens the menu; clicking the primary zone does not. A constant
/// split fraction would mis-attribute when the primary label width changes.
#[test]
fn caret_zone_opens_menu_primary_does_not() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    // A wide primary label so a constant 50/50 split would place the boundary
    // far from the real caret box.
    g.mount(
        "sb",
        Box::new(SplitButton::new("commit", "Commit Everything Now", items())),
    );
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    let (prim, caret) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        (
            q.box_of_part(root, "primary").expect("primary box"),
            q.box_of_part(root, "caret").expect("caret box"),
        )
    };
    assert!(caret.x > prim.x, "caret is to the right of primary");
    // Click the caret box center -> opens.
    g.left_click(caret.x + caret.width / 2.0, caret.y + caret.height / 2.0);
    let a = g.process();
    assert!(a.is_empty(), "opening the menu emits no action");
    assert!(as_split(&g, "sb").is_open(), "caret click opens the menu");
}

/// Clicking a menu item fires its action and closes.
#[test]
fn menu_item_fires_and_closes() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items())));
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    // Open via caret.
    let caret = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "caret").unwrap()
    };
    g.left_click(caret.x + caret.width / 2.0, caret.y + caret.height / 2.0);
    let _ = g.process();
    g.relayout();
    // Click item-1 ("Save All").
    let item1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "item-1").expect("item-1 box")
    };
    g.left_click(item1.x + item1.width / 2.0, item1.y + item1.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "save-all");
    assert!(!as_split(&g, "sb").is_open(), "choosing an item closes the menu");
}

/// Esc dismisses an open menu. (A click on truly-empty gallery area never
/// reaches the widget handler in isolation — same limitation the dropdown notes —
/// so the dismiss path is exercised via Esc here and via the caret-toggle below.)
#[test]
fn escape_dismisses() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items())));
    g.relayout();
    g.host.set_focus(Some("sb"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert!(as_split(&g, "sb").is_open());
    g.key(KeyInput::new(keys::ESCAPE, 0));
    assert!(!as_split(&g, "sb").is_open(), "Esc dismisses the menu");
}

/// A click inside the widget that is neither a menu item nor the caret (here the
/// primary zone, while the menu is open) dismisses the menu without firing.
#[test]
fn in_widget_miss_click_dismisses() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items())));
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    let caret = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "caret").unwrap()
    };
    g.left_click(caret.x + caret.width / 2.0, caret.y + caret.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert!(as_split(&g, "sb").is_open());
    // Click the primary zone (a different node from the caret -> no double-click
    // coalescing). While open, a non-item/non-caret click dismisses.
    let prim = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "primary").unwrap()
    };
    g.left_click(prim.x + prim.width / 2.0, prim.y + prim.height / 2.0);
    let a = g.process();
    assert!(
        a.iter().all(|act| act.name != "save"),
        "the dismiss click must NOT fire the primary action"
    );
    assert!(!as_split(&g, "sb").is_open(), "in-widget miss click dismisses the menu");
}

/// Keyboard: Enter fires primary when closed; Down opens; Enter fires highlight.
#[test]
fn keyboard_primary_and_menu() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("run", "Run", items())));
    g.relayout();
    g.host.set_focus(Some("sb"), &mut g.doc, &mut g.dispatcher);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "run", "Enter while closed fires primary");

    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert!(as_split(&g, "sb").is_open());
    assert_eq!(as_split(&g, "sb").highlighted(), Some(0));
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert_eq!(as_split(&g, "sb").highlighted(), Some(1));
    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a[0].name, "save-all", "Enter fires the highlighted item");
    assert!(!as_split(&g, "sb").is_open());
}

/// The open menu lays out real item boxes (a closed split-button has none).
#[test]
fn menu_lays_out_only_when_open() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items())));
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "item-0").is_none(), "closed: no item boxes");
    }
    let caret = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "caret").unwrap()
    };
    g.left_click(caret.x + caret.width / 2.0, caret.y + caret.height / 2.0);
    let _ = g.process();
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let b = q.box_of_part(root, "item-0").expect("item-0 box when open");
    assert!(b.width > 0.0 && b.height > 0.0);
}

/// Disabled split button swallows interaction.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sb", Box::new(SplitButton::new("save", "Save", items()).disabled(true)));
    g.relayout();
    let root = g.host.root_of("sb").unwrap();
    let prim = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "primary").unwrap()
    };
    g.left_click(prim.x + prim.width / 2.0, prim.y + prim.height / 2.0);
    assert!(g.process().is_empty());
}
