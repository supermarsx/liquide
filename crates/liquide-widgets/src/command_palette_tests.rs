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
