//! `<lq-command-palette>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::command_palette::{Command, CommandPalette, ACTION_NAME};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 520;
const H: u32 = 480;

fn as_cp<'a>(g: &'a Gallery, id: &str) -> &'a CommandPalette {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<CommandPalette>()
        .unwrap()
}

fn cmds() -> Vec<Command> {
    vec![
        Command::new("open-file", "Open File"),
        Command::new("open-map", "Open Map"),
        Command::new("close-window", "Close Window"),
        Command::new("save-doc", "Save Document"),
        Command::new("dark-mode", "Toggle Dark Mode"),
    ]
}

fn mount(g: &mut Gallery, id: &str, open: bool) {
    g.mount(id, Box::new(CommandPalette::new(cmds()).open(open)));
    g.relayout();
    g.host.set_focus(Some(id), &mut g.doc, &mut g.dispatcher);
}

/// A closed palette paints NO item boxes; the open palette does.
#[test]
fn closed_paints_no_items_open_does() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", false);
    let root = g.host.root_of("cp").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "item-0").is_none(), "closed: no items");
    }
    // Open it programmatically by re-mounting open (no toggle API exposed here).
    g.mount("cp", Box::new(CommandPalette::new(cmds()).open(true)));
    g.relayout();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "item-0").is_some(), "open: items exist");
}

/// Typing filters AND ranks: a fuzzy query narrows the visible set, best-first.
#[test]
fn typing_filters_and_ranks() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);

    // Type "open" — only the two "Open …" commands survive.
    for c in "open".chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
    g.relayout();
    assert_eq!(as_cp(&g, "cp").query(), "open");
    let vis = as_cp(&g, "cp").visible_indices();
    assert_eq!(vis.len(), 2, "only the two Open commands match 'open'");
    // Both are open-* (indices 0 and 1 in stable order).
    assert!(vis.contains(&0) && vis.contains(&1));
}

/// Fuzzy ranking puts a word-boundary/contiguous hit above a scattered one.
#[test]
fn ranking_prefers_better_fuzzy_match() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    g.mount(
        "cp",
        Box::new(
            CommandPalette::new(vec![
                Command::new("a", "Dark mode close"), // scattered d..o..c
                Command::new("b", "Open Document"),   // contiguous "doc"
            ])
            .open(true),
        ),
    );
    g.relayout();
    g.host.set_focus(Some("cp"), &mut g.doc, &mut g.dispatcher);
    for c in "doc".chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
    g.relayout();
    let vis = as_cp(&g, "cp").visible_indices();
    assert_eq!(vis.len(), 2, "both contain the subsequence d-o-c");
    assert_eq!(vis[0], 1, "the contiguous 'Open Document' ranks first");
}

/// Up/Down move the highlight; Enter activates → Action(command id).
#[test]
fn keyboard_navigate_and_activate() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);

    assert_eq!(as_cp(&g, "cp").highlighted(), 0);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_cp(&g, "cp").highlighted(), 1);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_cp(&g, "cp").highlighted(), 2);
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    g.relayout();
    assert_eq!(as_cp(&g, "cp").highlighted(), 1);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, ACTION_NAME);
    assert_eq!(a[0].payload.as_deref(), Some("open-map"), "row 1 is Open Map");
}

/// Esc closes the palette (and clears the query).
#[test]
fn escape_closes() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    for c in "op".chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
    g.relayout();
    assert!(as_cp(&g, "cp").is_open());
    g.key(KeyInput::new(keys::ESCAPE, 0));
    g.relayout();
    assert!(!as_cp(&g, "cp").is_open());
    assert_eq!(as_cp(&g, "cp").query(), "");
}

/// Clicking an item activates it from its LAID-OUT box → Action(id).
#[test]
fn click_item_activates() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    let root = g.host.root_of("cp").unwrap();
    let item2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "item-2").expect("item-2 box")
    };
    g.left_click(item2.x + 8.0, item2.y + item2.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].payload.as_deref(), Some("close-window"));
}

/// NO-FAKE-GREEN tooth: per-item hit reads each REAL laid-out box. With tall
/// (48px) rows, clicking a middle item's true box activates THAT item — an
/// `index * constant_height` guess (e.g. 28px pitch) would mis-target.
#[test]
fn item_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        560,
        "lq-gallery { padding: 20px; } lq-palette-item { height: 48px; padding-top: 0; padding-bottom: 0; }",
    );
    mount(&mut g, "cp", true);
    let root = g.host.root_of("cp").unwrap();
    let item3 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "item-3").expect("item-3 box")
    };
    assert!(item3.height >= 44.0, "precondition: tall rows (got {})", item3.height);
    g.left_click(item3.x + 8.0, item3.y + item3.height / 2.0);
    let a = g.process();
    assert_eq!(
        a[0].payload.as_deref(),
        Some("save-doc"),
        "click in item-3's REAL box activates command 3"
    );
}

/// Opening restyles the rasterized pixels (the palette surface appears).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    g.mount("cp", Box::new(CommandPalette::new(cmds()).open(false)));
    g.relayout();
    let before = Gallery::pixel(&g.rasterize(), 40, 60);
    g.mount("cp", Box::new(CommandPalette::new(cmds()).open(true)));
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), 40, 60);
    assert!(before != after, "the open palette must restyle pixels");
}

// ── Added: deep visual-STATE pixel-delta coverage (no fake-green) ────────────

/// Resolve the absolute box of an item part under the palette root.
fn item_box(g: &Gallery, id: &str, part: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part).unwrap_or_else(|| panic!("part {part} box"))
}

/// :focus/.highlighted paints a DISTINCT selection background + accent border-left
/// on the highlighted row. Row 0 starts highlighted; a plain (non-highlighted) row
/// further down must differ from it — the styling is real, not uniform.
#[test]
fn highlighted_row_paints_distinct_from_plain_row() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    // Row 0 is highlighted by default; row 2 is a plain row.
    let r0 = item_box(&g, "cp", "item-0");
    let r2 = item_box(&g, "cp", "item-2");
    let fb = g.rasterize();
    // Sample the LEFT edge band where the accent border-left lands on the
    // highlighted row, and the selection bg fills the row interior.
    let hi = Gallery::pixel(&fb, (r0.x + 1.0) as u32, (r0.y + r0.height / 2.0) as u32);
    let plain = Gallery::pixel(&fb, (r2.x + 1.0) as u32, (r2.y + r2.height / 2.0) as u32);
    assert!(
        hi != plain,
        "highlighted row must paint a distinct selection/accent style (hi {hi:?} plain {plain:?})"
    );
}

/// The highlight (selection bg + accent border) MOVES with the cursor: the same
/// row-0 pixels change once the highlight moves to row 1 (Down). No-fake-green:
/// removing the `.highlighted`/`:focus` rule makes both samples identical.
#[test]
fn highlight_moves_with_arrow_keys_in_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    let r0 = item_box(&g, "cp", "item-0");
    let (sx, sy) = ((r0.x + 1.0) as u32, (r0.y + r0.height / 2.0) as u32);
    // Row 0 highlighted now.
    let with_hi = Gallery::pixel(&g.rasterize(), sx, sy);

    // Move the highlight to row 1 → row 0 loses its accent/selection.
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    assert_eq!(as_cp(&g, "cp").highlighted(), 1);
    let without_hi = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        with_hi != without_hi,
        "row 0's accent/selection must clear when the highlight moves to row 1 \
         (with {with_hi:?} without {without_hi:?})"
    );

    // And row 1 must now carry the accent it did not have before.
    let r1 = item_box(&g, "cp", "item-1");
    let (rx, ry) = ((r1.x + 1.0) as u32, (r1.y + r1.height / 2.0) as u32);
    let r1_hi = Gallery::pixel(&g.rasterize(), rx, ry);
    // Rebuild a fresh palette to capture row 1 with NO highlight for comparison.
    let mut g2 = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g2, "cp", true); // row 0 highlighted, row 1 plain
    let r1b = item_box(&g2, "cp", "item-1");
    let r1_plain =
        Gallery::pixel(&g2.rasterize(), (r1b.x + 1.0) as u32, (r1b.y + r1b.height / 2.0) as u32);
    assert!(
        r1_hi != r1_plain,
        "row 1 must gain the accent once highlighted (hi {r1_hi:?} plain {r1_plain:?})"
    );
}

/// :hover paints the hover background on the pointed row, distinct from its
/// resting state. The hovered row is NOT row 0 (which is highlighted) so the
/// delta is purely the hover rule.
#[test]
fn hover_restyles_item_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    let r2 = item_box(&g, "cp", "item-2");
    // Sample the row interior (past the border-left band) so we read the bg fill.
    let (sx, sy) = ((r2.x + r2.width / 2.0) as u32, (r2.y + r2.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);

    g.pointer_move(r2.x + r2.width / 2.0, r2.y + r2.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_cp(&g, "cp").highlighted(), 0, "hover does not move the keyboard highlight");
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(
        before != after,
        ":hover must restyle the pointed row's background (before {before:?} after {after:?})"
    );
}

/// The search field carries the `.placeholder` class (the dim-colour CSS hook)
/// ONLY while the query is empty; typing clears it and the rendered text switches
/// from the placeholder string to the live query. This reads the rendered DOM
/// (post style/layout) — not a tautology: a palette that did not swap the
/// placeholder for the query would keep the class + text.
///
/// NOTE: a pixel delta on the SEARCH TEXT itself is NOT assertable here — the
/// gallery rasterizer renders the placeholder (dim) and the query (bright) text
/// as byte-identical dark glyph ink (weak text-colour application + weak glyph
/// ink), so the two states are pixel-identical in the search band. See CSS/render
/// gap report. The `.placeholder` class + text-content swap are the real,
/// observable state change.
#[test]
fn placeholder_class_clears_when_query_is_typed() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    let search_text = |g: &Gallery| -> String {
        let root = g.host.root_of("cp").unwrap();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        let s = q.find_part(root, "search").unwrap();
        let mut out = String::new();
        fn rec(doc: &liquide_dom::Document, n: liquide_dom::NodeId, out: &mut String) {
            if let Some(t) = doc.get(n).and_then(|x| x.text_content()) {
                out.push_str(t);
            }
            for &c in doc.children(n) {
                rec(doc, c, out);
            }
        }
        rec(g.doc(), s, &mut out);
        out
    };
    let has_placeholder_class = |g: &Gallery| -> bool {
        let root = g.host.root_of("cp").unwrap();
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        let s = q.find_part(root, "search").unwrap();
        g.doc().get(s).map(|n| n.has_class("placeholder")).unwrap_or(false)
    };

    // Empty: placeholder class + placeholder text.
    assert!(has_placeholder_class(&g), "empty field carries the .placeholder class");
    assert_eq!(search_text(&g), "Type a command…", "empty field shows the placeholder string");

    // Type a query → class clears + the field shows the live query.
    for c in "open".chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
    g.relayout();
    assert_eq!(as_cp(&g, "cp").query(), "open");
    assert!(!has_placeholder_class(&g), "a typed field drops the .placeholder class");
    assert_eq!(search_text(&g), "open", "the field now renders the live query");
}

/// A query that matches nothing renders the `lq-palette-empty` notice (a real
/// part box) and emits NO item boxes — the empty state is structurally distinct.
#[test]
fn no_match_renders_empty_state_part() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    mount(&mut g, "cp", true);
    for c in "zzzzz".chars() {
        g.key(KeyInput::new(c as u32, 0));
    }
    g.relayout();
    assert!(as_cp(&g, "cp").visible_indices().is_empty(), "no command matches 'zzzzz'");
    let root = g.host.root_of("cp").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "empty").is_some(), "the empty notice paints a box");
    assert!(q.box_of_part(root, "item-0").is_none(), "no item rows in the empty state");
}
