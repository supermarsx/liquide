//! Integration tests for the CSS rendering pipeline.
//!
//! Tests the full path:  DOM construction → CSS styling → layout → paint
//!
//! Focuses on three reported regressions:
//!   1. DevTools text titles never rendered  (text nodes in flex containers)
//!   2. DevTools entries have excessive spacing
//!   3. StatusBar looks wonky in columns    (flex slot sizing)

use liquide_dom::Document;
use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
use liquide_paint::{DisplayItem, Painter};
use liquide_style_engine::engine::{StyleEngine, ViewportSize};

// ── helpers ──────────────────────────────────────────────────────

fn style_engine() -> StyleEngine {
    StyleEngine::new(
        ViewportSize {
            width: 1920.0,
            height: 1080.0,
        },
        16.0,
    )
}

fn layout_engine() -> LayoutEngine {
    LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0)
}

// ═════════════════════════════════════════════════════════════════
//  PART 1 – Text nodes inside flex containers
// ═════════════════════════════════════════════════════════════════

/// The PRIMARY bug: a text-node child of a flex container must have
/// measured width > 0, not a 0×0 box.
#[test]
fn flex_child_text_node_has_nonzero_size() {
    let mut doc = Document::new();
    let root = doc.root();
    let container = doc.create_element("div");
    let text = doc.create_text("Hello World");
    doc.append_child(root, container);
    doc.append_child(container, text);

    let mut se = style_engine();
    se.add_stylesheet("div { display: flex; width: 400; height: 40; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let text_box = tree
        .find_by_node(text)
        .expect("text node must have a layout box");
    assert!(
        text_box.content_rect.width > 0.0,
        "text node in flex container must have measured width, got {}",
        text_box.content_rect.width,
    );
    assert!(
        text_box.content_rect.height > 0.0,
        "text node in flex container must have measured height, got {}",
        text_box.content_rect.height,
    );
}

/// The parent flex item should be sized to contain its text child.
#[test]
fn flex_child_text_node_contributes_to_parent_size() {
    let mut doc = Document::new();
    let root = doc.root();
    let container = doc.create_element("row");
    let item = doc.create_element("item");
    let text = doc.create_text("Elements");
    doc.append_child(root, container);
    doc.append_child(container, item);
    doc.append_child(item, text);

    let mut se = style_engine();
    se.add_stylesheet(
        "row { display: flex; width: 600; }
         item { display: flex; align-items: center; padding-left: 10; padding-right: 10; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let item_box = tree.find_by_node(item).unwrap();
    assert!(
        item_box.content_rect.width > 0.0,
        "flex item with text child should have content_width > 0, got {}",
        item_box.content_rect.width,
    );

    let text_box = tree.find_by_node(text).unwrap();
    assert!(
        text_box.content_rect.width > 10.0,
        "'Elements' text should be wider than 10px, got {}",
        text_box.content_rect.width,
    );
}

/// Two text-containing items in a row should not overlap.
#[test]
fn multiple_flex_text_children_laid_out_sequentially() {
    let mut doc = Document::new();
    let root = doc.root();
    let row = doc.create_element("row");
    let tab_a = doc.create_element("tab");
    let text_a = doc.create_text("Console");
    let tab_b = doc.create_element("tab");
    let text_b = doc.create_text("Performance");
    doc.append_child(root, row);
    doc.append_child(row, tab_a);
    doc.append_child(tab_a, text_a);
    doc.append_child(row, tab_b);
    doc.append_child(tab_b, text_b);

    let mut se = style_engine();
    se.add_stylesheet(
        "row { display: flex; width: 600; gap: 4; }
         tab { display: flex; align-items: center; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let box_a = tree.find_by_node(tab_a).unwrap();
    let box_b = tree.find_by_node(tab_b).unwrap();
    let a_abs = tree.absolute_content_rect(box_a.id);
    let b_abs = tree.absolute_content_rect(box_b.id);

    assert!(
        b_abs.x >= a_abs.x + a_abs.width,
        "tab B (x={:.1}) must not overlap tab A (x={:.1}, w={:.1})",
        b_abs.x,
        a_abs.x,
        a_abs.width,
    );
}

/// Direct (bare) text nodes inside a flex row get measured.
#[test]
fn bare_text_in_flex_row_gets_measured() {
    let mut doc = Document::new();
    let root = doc.root();
    let flex = doc.create_element("bar");
    let t1 = doc.create_text("LiquiDE");
    let t2 = doc.create_text("12:34");
    doc.append_child(root, flex);
    doc.append_child(flex, t1);
    doc.append_child(flex, t2);

    let mut se = style_engine();
    se.add_stylesheet("bar { display: flex; width: 800; gap: 16; font-size: 14; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (id, label) in [(t1, "LiquiDE"), (t2, "12:34")] {
        let b = tree.find_by_node(id).unwrap_or_else(|| panic!("text '{}' missing box", label));
        assert!(b.content_rect.width > 0.0, "'{}' width must be > 0", label);
        assert!(
            b.content_rect.height > 0.0,
            "'{}' height must be > 0",
            label
        );
    }
}

/// flex-direction: column with text children stacks vertically.
#[test]
fn flex_column_text_children_stack_vertically() {
    let mut doc = Document::new();
    let root = doc.root();
    let col = doc.create_element("col");
    let t1 = doc.create_text("Line 1");
    let t2 = doc.create_text("Line 2");
    doc.append_child(root, col);
    doc.append_child(col, t1);
    doc.append_child(col, t2);

    let mut se = style_engine();
    se.add_stylesheet("col { display: flex; flex-direction: column; width: 300; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let b1 = tree.find_by_node(t1).unwrap();
    let b2 = tree.find_by_node(t2).unwrap();
    let abs1 = tree.absolute_content_rect(b1.id);
    let abs2 = tree.absolute_content_rect(b2.id);

    assert!(
        abs2.y >= abs1.y + abs1.height,
        "Line 2 (y={:.1}) must be below Line 1 (y={:.1}+h={:.1})",
        abs2.y,
        abs1.y,
        abs1.height,
    );
}

// ═════════════════════════════════════════════════════════════════
//  PART 2 – DevTools panel layout
// ═════════════════════════════════════════════════════════════════

/// Devtools tabs bar: every tab label must produce a non-zero layout box
/// and each tab must be wider than its horizontal padding alone.
#[test]
fn devtools_tab_text_is_visible() {
    let mut doc = Document::new();
    let root = doc.root();
    let panel = doc.create_element("devtools");
    let tabs = doc.create_element("devtools-tabs");
    doc.append_child(root, panel);
    doc.append_child(panel, tabs);

    let labels = ["Elements", "Console", "Sources", "Performance", "Mutations", "Scene"];
    let mut tab_ids = Vec::new();
    let mut text_ids = Vec::new();
    for label in &labels {
        let tab = doc.create_element("devtools-tab");
        let text = doc.create_text(label);
        doc.append_child(tabs, tab);
        doc.append_child(tab, text);
        tab_ids.push(tab);
        text_ids.push(text);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "devtools { display: flex; flex-direction: column; width: 1920; height: 400; font-size: 11; }
         devtools-tabs { display: flex; height: 32; overflow: hidden; }
         devtools-tab { display: flex; align-items: center; padding-left: 10; padding-right: 10; font-size: 11; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (i, &text_id) in text_ids.iter().enumerate() {
        let text_box = tree
            .find_by_node(text_id)
            .unwrap_or_else(|| panic!("text for '{}' must have a layout box", labels[i]));
        assert!(
            text_box.content_rect.width > 0.0,
            "tab label '{}' width must be > 0, got {}",
            labels[i],
            text_box.content_rect.width,
        );
    }

    // Each tab wider than just its padding (20px)
    for (i, &tab_id) in tab_ids.iter().enumerate() {
        let tab_box = tree.find_by_node(tab_id).unwrap();
        let tab_abs = tree.absolute_margin_rect(tab_box.id);
        assert!(
            tab_abs.width > 20.0,
            "tab '{}' should be wider than 20px padding, got {:.1}",
            labels[i],
            tab_abs.width,
        );
    }
}

/// Tree rows are 20 px fixed height; text inside must not inflate them,
/// and each tag's text must have non-zero width.
#[test]
fn devtools_tree_rows_have_correct_height() {
    let mut doc = Document::new();
    let root = doc.root();
    let tree_el = doc.create_element("devtools-tree");
    doc.append_child(root, tree_el);

    let tags = ["html", "head", "body", "div.container", "span.label"];
    let mut row_ids = Vec::new();
    let mut text_ids = Vec::new();
    for tag in &tags {
        let row = doc.create_element("devtools-tree-row");
        let arrow = doc.create_element("devtools-tree-arrow");
        let tag_el = doc.create_element("devtools-tree-tag");
        let text = doc.create_text(tag);
        doc.append_child(tree_el, row);
        doc.append_child(row, arrow);
        doc.append_child(row, tag_el);
        doc.append_child(tag_el, text);
        row_ids.push(row);
        text_ids.push(text);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "devtools-tree { display: flex; flex-direction: column; width: 400; }
         devtools-tree-row { display: flex; height: 20; min-height: 20; max-height: 20; align-items: center; }
         devtools-tree-arrow { width: 14; height: 14; flex-shrink: 0; }
         devtools-tree-tag { display: flex; align-items: center; font-size: 11; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (i, &row_id) in row_ids.iter().enumerate() {
        let row_box = tree.find_by_node(row_id).unwrap();
        assert!(
            (row_box.content_rect.height - 20.0).abs() < 1.0,
            "row {} ('{}') height should be ~20, got {:.1}",
            i,
            tags[i],
            row_box.content_rect.height,
        );
    }

    for (i, &text_id) in text_ids.iter().enumerate() {
        let text_box = tree.find_by_node(text_id).unwrap();
        assert!(
            text_box.content_rect.width > 0.0,
            "tag text '{}' width must be > 0, got {}",
            tags[i],
            text_box.content_rect.width,
        );
    }
}

/// Consecutive tree rows (20 px each, no gap) should abut with zero spacing.
#[test]
fn devtools_rows_do_not_have_excessive_spacing() {
    let mut doc = Document::new();
    let root = doc.root();
    let tree_el = doc.create_element("devtools-tree");
    doc.append_child(root, tree_el);

    let mut row_ids = Vec::new();
    for i in 0..5 {
        let row = doc.create_element("devtools-tree-row");
        let text = doc.create_text(&format!("Row {}", i));
        doc.append_child(tree_el, row);
        doc.append_child(row, text);
        row_ids.push(row);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "devtools-tree { display: flex; flex-direction: column; width: 400; }
         devtools-tree-row { display: flex; height: 20; min-height: 20; max-height: 20; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let mut prev_bottom: Option<f32> = None;
    for (i, &row_id) in row_ids.iter().enumerate() {
        let row_box = tree.find_by_node(row_id).unwrap();
        let abs = tree.absolute_content_rect(row_box.id);
        if let Some(pb) = prev_bottom {
            let gap = abs.y - pb;
            assert!(
                gap.abs() < 1.0,
                "gap between row {} and {} is {:.1}px, expected 0",
                i - 1,
                i,
                gap,
            );
        }
        prev_bottom = Some(abs.y + abs.height);
    }
}

/// Property panel: every name/value text should be visible.
#[test]
fn devtools_property_panel_text_visible() {
    let mut doc = Document::new();
    let root = doc.root();
    let panel = doc.create_element("devtools-styles");
    doc.append_child(root, panel);

    let props = [("display", "flex"), ("width", "200px"), ("color", "red")];
    let mut all_texts: Vec<(u64, &str)> = Vec::new();

    for (name, value) in &props {
        let row = doc.create_element("devtools-prop");
        let name_el = doc.create_element("devtools-prop-name");
        let nt = doc.create_text(name);
        let val_el = doc.create_element("devtools-prop-value");
        let vt = doc.create_text(value);
        doc.append_child(panel, row);
        doc.append_child(row, name_el);
        doc.append_child(name_el, nt);
        doc.append_child(row, val_el);
        doc.append_child(val_el, vt);
        all_texts.push((nt, name));
        all_texts.push((vt, value));
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "devtools-styles { display: flex; flex-direction: column; width: 300; }
         devtools-prop { display: flex; height: 16; align-items: center; }
         devtools-prop-name { display: flex; width: 100; font-size: 11; }
         devtools-prop-value { display: flex; flex-grow: 1; font-size: 11; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (id, label) in &all_texts {
        let b = tree.find_by_node(*id).unwrap();
        assert!(
            b.content_rect.width > 0.0,
            "prop text '{}' width must be > 0, got {}",
            label,
            b.content_rect.width,
        );
    }
}

// ═════════════════════════════════════════════════════════════════
//  PART 3 – StatusBar column layout
// ═════════════════════════════════════════════════════════════════

/// With flex-basis: 0 + flex-grow: 1, three slots should be ~equal width.
#[test]
fn statusbar_three_slots_equal_width() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let left = doc.create_element("statusbar-slot");
    let center = doc.create_element("statusbar-slot");
    let right = doc.create_element("statusbar-slot");
    doc.append_child(root, bar);
    doc.append_child(bar, left);
    doc.append_child(bar, center);
    doc.append_child(bar, right);

    // Different content lengths
    let tl = doc.create_text("LiquiDE");
    let tc = doc.create_text("12:34");
    let tr = doc.create_text("Connected");
    doc.append_child(left, tl);
    doc.append_child(center, tc);
    doc.append_child(right, tr);

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1920; height: 28; align-items: center; }
         statusbar-slot { display: flex; align-items: center; flex-grow: 1; flex-shrink: 1; flex-basis: 0; gap: 8; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let lw = tree.find_by_node(left).unwrap().margin_rect.width;
    let cw = tree.find_by_node(center).unwrap().margin_rect.width;
    let rw = tree.find_by_node(right).unwrap().margin_rect.width;

    let expected = 1920.0 / 3.0;
    let tolerance = expected * 0.05;
    assert!(
        (lw - expected).abs() < tolerance,
        "left {:.1} should be ~{:.1}",
        lw,
        expected,
    );
    assert!(
        (cw - expected).abs() < tolerance,
        "center {:.1} should be ~{:.1}",
        cw,
        expected,
    );
    assert!(
        (rw - expected).abs() < tolerance,
        "right {:.1} should be ~{:.1}",
        rw,
        expected,
    );
}

/// flex-basis: 0 makes slots equal even with different content;
/// flex-basis: auto would use content width as starting point.
/// Verify the equal-distribution behavior with flex-basis: 0.
#[test]
fn statusbar_flex_basis_zero_equalizes_slots() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let left = doc.create_element("slot");
    let right = doc.create_element("slot");
    doc.append_child(root, bar);
    doc.append_child(bar, left);
    doc.append_child(bar, right);

    let tl = doc.create_text("A");
    doc.append_child(left, tl);

    // Right slot gets significantly more content
    for label in &["Notifications", "Battery", "Network", "Session"] {
        let item = doc.create_element("item");
        let t = doc.create_text(label);
        doc.append_child(right, item);
        doc.append_child(item, t);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1200; height: 28; }
         slot { display: flex; align-items: center; flex-grow: 1; flex-shrink: 1; flex-basis: 0; gap: 8; }
         item { display: flex; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let lw = tree.find_by_node(left).unwrap().margin_rect.width;
    let rw = tree.find_by_node(right).unwrap().margin_rect.width;
    let diff = (rw - lw).abs();

    // With flex-basis: 0 + equal flex-grow, both slots should be ~equal
    assert!(
        diff < 1.0,
        "with flex-basis:0, left ({:.1}) and right ({:.1}) should be equal, diff={:.1}",
        lw,
        rw,
        diff,
    );
}

/// Each status bar item's text must be visible.
#[test]
fn statusbar_text_items_are_visible() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let slot = doc.create_element("statusbar-slot");
    doc.append_child(root, bar);
    doc.append_child(bar, slot);

    let labels = ["LiquiDE", "12:34 PM", "Connected", "3"];
    let mut text_ids: Vec<(u64, &str)> = Vec::new();
    for label in &labels {
        let item = doc.create_element("statusbar-item");
        let text = doc.create_text(label);
        doc.append_child(slot, item);
        doc.append_child(item, text);
        text_ids.push((text, label));
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1920; height: 28; }
         statusbar-slot { display: flex; align-items: center; gap: 10; flex-grow: 1; }
         statusbar-item { display: flex; align-items: center; padding-left: 4; padding-right: 4; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (id, label) in &text_ids {
        let b = tree
            .find_by_node(*id)
            .unwrap_or_else(|| panic!("text '{}' must have a layout box", label));
        assert!(
            b.content_rect.width > 0.0,
            "'{}' width must be > 0",
            label,
        );
        assert!(
            b.content_rect.height > 0.0,
            "'{}' height must be > 0",
            label,
        );
    }
}

/// Items within a slot must not overlap.
#[test]
fn statusbar_items_do_not_overlap() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let slot = doc.create_element("slot");
    doc.append_child(root, bar);
    doc.append_child(bar, slot);

    let mut item_ids = Vec::new();
    for label in &["Clock", "Battery", "Network"] {
        let item = doc.create_element("item");
        let text = doc.create_text(label);
        doc.append_child(slot, item);
        doc.append_child(item, text);
        item_ids.push(item);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1920; height: 28; }
         slot { display: flex; align-items: center; gap: 8; }
         item { display: flex; align-items: center; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let mut prev_right: Option<f32> = None;
    for (i, &item_id) in item_ids.iter().enumerate() {
        let b = tree.find_by_node(item_id).unwrap();
        let abs = tree.absolute_margin_rect(b.id);
        if let Some(pr) = prev_right {
            assert!(
                abs.x >= pr - 0.5,
                "item {} (x={:.1}) overlaps item {} (right={:.1})",
                i,
                abs.x,
                i - 1,
                pr,
            );
        }
        prev_right = Some(abs.x + abs.width);
    }
}

/// The center slot should be approximately centered in the bar.
#[test]
fn statusbar_center_slot_is_centered() {
    let mut doc = Document::new();
    let root = doc.root();
    let bar = doc.create_element("statusbar");
    let left = doc.create_element("slot");
    let center = doc.create_element("slot");
    let right = doc.create_element("slot");
    doc.append_child(root, bar);
    doc.append_child(bar, left);
    doc.append_child(bar, center);
    doc.append_child(bar, right);

    let t = doc.create_text("12:34 PM");
    doc.append_child(center, t);

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1200; height: 28; align-items: center; }
         slot { display: flex; align-items: center; flex-grow: 1; flex-shrink: 1; flex-basis: 0; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let center_box = tree.find_by_node(center).unwrap();
    let abs = tree.absolute_content_rect(center_box.id);
    let slot_mid = abs.x + abs.width / 2.0;
    let bar_mid = 1200.0 / 2.0;
    assert!(
        (slot_mid - bar_mid).abs() < 50.0,
        "center slot midpoint ({:.1}) should be near bar midpoint ({:.1})",
        slot_mid,
        bar_mid,
    );
}

/// Full desktop-like statusbar DOM: logo, clock, indicators.
#[test]
fn statusbar_full_dom_like_desktop() {
    let mut doc = Document::new();
    let root = doc.root();

    let bar = doc.create_element("statusbar");
    doc.append_child(root, bar);

    let slot_left = doc.create_element("statusbar-slot");
    doc.append_child(bar, slot_left);
    let logo = doc.create_element("statusbar-logo");
    let logo_text = doc.create_text("LiquiDE");
    doc.append_child(slot_left, logo);
    doc.append_child(logo, logo_text);

    let slot_center = doc.create_element("statusbar-slot");
    doc.append_child(bar, slot_center);
    let clock = doc.create_element("statusbar-item");
    let clock_text = doc.create_text("14:30");
    doc.append_child(slot_center, clock);
    doc.append_child(clock, clock_text);

    let slot_right = doc.create_element("statusbar-slot");
    doc.append_child(bar, slot_right);
    let notif = doc.create_element("indicator");
    let notif_text = doc.create_text("5");
    doc.append_child(slot_right, notif);
    doc.append_child(notif, notif_text);
    let conn = doc.create_element("indicator");
    let conn_text = doc.create_text("Connected");
    doc.append_child(slot_right, conn);
    doc.append_child(conn, conn_text);

    let mut se = style_engine();
    se.add_stylesheet(
        "statusbar { display: flex; width: 1920; height: 28; padding-left: 8; padding-right: 8; align-items: center; font-size: 13; }
         statusbar-slot { display: flex; align-items: center; flex-grow: 1; flex-shrink: 1; flex-basis: 0; gap: 8; }
         statusbar-logo { display: flex; align-items: center; padding-left: 6; padding-right: 10; }
         statusbar-item { display: flex; align-items: center; padding-left: 4; padding-right: 4; }
         indicator { display: flex; align-items: center; padding-left: 6; padding-right: 6; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    for (text_id, label) in [
        (logo_text, "LiquiDE"),
        (clock_text, "14:30"),
        (notif_text, "5"),
        (conn_text, "Connected"),
    ] {
        let b = tree.find_by_node(text_id).unwrap();
        assert!(
            b.content_rect.width > 0.0,
            "statusbar '{}' width must be > 0",
            label,
        );
    }

    // Slots should collectively fill the bar
    let bar_box = tree.find_by_node(bar).unwrap();
    assert!(bar_box.content_rect.width > 1900.0);

    // Center slot should be roughly centered
    let center_box = tree.find_by_node(slot_center).unwrap();
    let c_abs = tree.absolute_content_rect(center_box.id);
    let b_abs = tree.absolute_content_rect(bar_box.id);
    let cmid = c_abs.x + c_abs.width / 2.0;
    let bmid = b_abs.x + b_abs.width / 2.0;
    assert!(
        (cmid - bmid).abs() < 100.0,
        "center midpoint ({:.1}) ≈ bar midpoint ({:.1})",
        cmid,
        bmid,
    );
}

// ═════════════════════════════════════════════════════════════════
//  PART 4 – Grid text nodes
// ═════════════════════════════════════════════════════════════════

/// Same text-in-flex bug but via a grid parent.
#[test]
fn grid_child_text_node_has_nonzero_width() {
    let mut doc = Document::new();
    let root = doc.root();
    let grid = doc.create_element("grid");
    let cell = doc.create_element("cell");
    let text = doc.create_text("Grid content");
    doc.append_child(root, grid);
    doc.append_child(grid, cell);
    doc.append_child(cell, text);

    let mut se = style_engine();
    se.add_stylesheet("grid { display: grid; width: 600; } cell { display: flex; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let text_box = tree.find_by_node(text).expect("text in grid must have box");
    assert!(
        text_box.content_rect.width > 0.0,
        "grid→flex text width must be > 0, got {}",
        text_box.content_rect.width,
    );
}

// ═════════════════════════════════════════════════════════════════
//  PART 5 – Full paint pipeline
// ═════════════════════════════════════════════════════════════════

/// End-to-end: DOM → style → layout → paint.
/// Text DisplayItems should have non-zero rectangles.
#[test]
fn painted_text_items_have_nonzero_bounds() {
    let mut doc = Document::new();
    let root = doc.root();
    let flex = doc.create_element("bar");
    let item = doc.create_element("item");
    let text = doc.create_text("Hello Paint");
    doc.append_child(root, flex);
    doc.append_child(flex, item);
    doc.append_child(item, text);

    let mut se = style_engine();
    se.add_stylesheet(
        "bar  { display: flex; width: 800; height: 40; }
         item { display: flex; align-items: center; color: rgba(255,255,255,1.0); }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let painter = Painter::new();
    let dl = painter.paint(&doc, &tree, &styles);

    let text_items: Vec<_> = dl
        .items
        .iter()
        .filter(|it| matches!(it, DisplayItem::Text { .. }))
        .collect();

    assert!(
        !text_items.is_empty(),
        "paint must emit at least one Text display item",
    );

    for it in &text_items {
        if let DisplayItem::Text { rect, text, .. } = it {
            assert!(
                rect.width > 0.0,
                "painted '{}' rect width must be > 0",
                text,
            );
            assert!(
                rect.height > 0.0,
                "painted '{}' rect height must be > 0",
                text,
            );
        }
    }
}

/// Full devtools-like structure through the paint pipeline.
#[test]
fn full_devtools_pipeline_produces_visible_text() {
    let mut doc = Document::new();
    let root = doc.root();

    let panel = doc.create_element("devtools");
    doc.append_child(root, panel);

    // Toolbar with 3 tabs
    let toolbar = doc.create_element("devtools-toolbar");
    doc.append_child(panel, toolbar);
    let tabs = doc.create_element("devtools-tabs");
    doc.append_child(toolbar, tabs);

    for label in &["Elements", "Console", "Sources"] {
        let tab = doc.create_element("devtools-tab");
        let text = doc.create_text(label);
        doc.append_child(tabs, tab);
        doc.append_child(tab, text);
    }

    // Content with 3 tree rows
    let content = doc.create_element("devtools-content");
    doc.append_child(panel, content);
    let tree_el = doc.create_element("devtools-tree");
    doc.append_child(content, tree_el);

    for tag in &["<html>", "<head>", "<body>"] {
        let row = doc.create_element("devtools-tree-row");
        let arrow = doc.create_element("devtools-tree-arrow");
        let tag_el = doc.create_element("devtools-tree-tag");
        let tag_text = doc.create_text(tag);
        doc.append_child(tree_el, row);
        doc.append_child(row, arrow);
        doc.append_child(row, tag_el);
        doc.append_child(tag_el, tag_text);
    }

    let mut se = style_engine();
    se.add_stylesheet(
        "devtools { display: flex; flex-direction: column; width: 1920; height: 400; font-size: 11; color: rgba(200,200,200,1.0); }
         devtools-toolbar { display: flex; height: 32; }
         devtools-tabs { display: flex; overflow: hidden; }
         devtools-tab { display: flex; align-items: center; padding-left: 10; padding-right: 10; }
         devtools-content { display: flex; flex-grow: 1; }
         devtools-tree { display: flex; flex-direction: column; width: 400; }
         devtools-tree-row { display: flex; height: 20; min-height: 20; max-height: 20; align-items: center; }
         devtools-tree-arrow { width: 14; height: 14; flex-shrink: 0; }
         devtools-tree-tag { display: flex; font-size: 11; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let layout_tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let painter = Painter::new();
    let dl = painter.paint(&doc, &layout_tree, &styles);

    let text_items: Vec<_> = dl
        .items
        .iter()
        .filter_map(|it| {
            if let DisplayItem::Text { rect, text, .. } = it {
                Some((text.clone(), *rect))
            } else {
                None
            }
        })
        .collect();

    // 3 tabs + 3 tree tags = 6 text items
    assert!(
        text_items.len() >= 6,
        "expected ≥6 text display items, got {}; items: {:?}",
        text_items.len(),
        text_items
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>(),
    );

    for (text, rect) in &text_items {
        assert!(
            rect.width > 0.0 && rect.height > 0.0,
            "'{}' bounds must be non-zero ({:.1}×{:.1})",
            text,
            rect.width,
            rect.height,
        );
    }
}

// ═════════════════════════════════════════════════════════════════
//  PART 6 – Deep nesting & mixed children
// ═════════════════════════════════════════════════════════════════

/// flex > flex > flex > text – three nesting levels.
#[test]
fn deeply_nested_flex_text_still_visible() {
    let mut doc = Document::new();
    let root = doc.root();
    let a = doc.create_element("a");
    let b = doc.create_element("b");
    let c = doc.create_element("c");
    let text = doc.create_text("Deep Text");
    doc.append_child(root, a);
    doc.append_child(a, b);
    doc.append_child(b, c);
    doc.append_child(c, text);

    let mut se = style_engine();
    se.add_stylesheet(
        "a { display: flex; width: 800; }
         b { display: flex; flex-grow: 1; }
         c { display: flex; align-items: center; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let text_box = tree.find_by_node(text).unwrap();
    assert!(
        text_box.content_rect.width > 0.0,
        "deeply nested flex text width must be > 0",
    );
}

/// A flex row with both element and bare-text children.
#[test]
fn flex_with_mixed_element_and_text_children() {
    let mut doc = Document::new();
    let root = doc.root();
    let row = doc.create_element("row");
    let icon = doc.create_element("icon");
    let text = doc.create_text("Label Text");
    let badge = doc.create_element("badge");
    let badge_text = doc.create_text("3");
    doc.append_child(root, row);
    doc.append_child(row, icon);
    doc.append_child(row, text);
    doc.append_child(row, badge);
    doc.append_child(badge, badge_text);

    let mut se = style_engine();
    se.add_stylesheet(
        "row   { display: flex; width: 400; align-items: center; gap: 8; }
         icon  { width: 16; height: 16; flex-shrink: 0; }
         badge { display: flex; align-items: center; }",
    );
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let icon_box = tree.find_by_node(icon).unwrap();
    assert!(
        (icon_box.content_rect.width - 16.0).abs() < 0.5,
        "icon width should be 16, got {:.1}",
        icon_box.content_rect.width,
    );

    let text_box = tree.find_by_node(text).unwrap();
    assert!(
        text_box.content_rect.width > 0.0,
        "bare text 'Label Text' must have width > 0",
    );

    let badge_text_box = tree.find_by_node(badge_text).unwrap();
    assert!(
        badge_text_box.content_rect.width > 0.0,
        "badge '3' must have width > 0",
    );

    // All left-to-right without overlap
    let ai = tree.absolute_content_rect(icon_box.id);
    let at = tree.absolute_content_rect(text_box.id);
    let ab = tree.absolute_content_rect(tree.find_by_node(badge).unwrap().id);
    assert!(at.x > ai.x, "text should be right of icon");
    assert!(ab.x > at.x, "badge should be right of text");
}

// ═════════════════════════════════════════════════════════════════
//  PART 7 – Regression guards
// ═════════════════════════════════════════════════════════════════

/// Block layout text still works (already correct before the fix).
#[test]
fn block_layout_text_still_works() {
    let mut doc = Document::new();
    let root = doc.root();
    let div = doc.create_element("div");
    let text = doc.create_text("Block text");
    doc.append_child(root, div);
    doc.append_child(div, text);

    let mut se = style_engine();
    se.add_stylesheet("div { width: 400; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let b = tree.find_by_node(text).unwrap();
    assert!(b.content_rect.width > 0.0);
    assert!(b.content_rect.height > 0.0);
}

/// An empty text node must not crash the layout engine.
#[test]
fn empty_text_node_does_not_crash() {
    let mut doc = Document::new();
    let root = doc.root();
    let flex = doc.create_element("flex");
    let text = doc.create_text("");
    doc.append_child(root, flex);
    doc.append_child(flex, text);

    let mut se = style_engine();
    se.add_stylesheet("flex { display: flex; width: 400; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let _tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
    // no panic = pass
}

/// Whitespace-only text nodes should not crash.
#[test]
fn whitespace_text_node_does_not_crash() {
    let mut doc = Document::new();
    let root = doc.root();
    let flex = doc.create_element("flex");
    let text = doc.create_text("   ");
    doc.append_child(root, flex);
    doc.append_child(flex, text);

    let mut se = style_engine();
    se.add_stylesheet("flex { display: flex; width: 400; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let _tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);
}

/// Flex container with no children should produce a valid layout.
#[test]
fn empty_flex_container() {
    let mut doc = Document::new();
    let root = doc.root();
    let flex = doc.create_element("flex");
    doc.append_child(root, flex);

    let mut se = style_engine();
    se.add_stylesheet("flex { display: flex; width: 400; height: 40; }");
    let styles = se.restyle_all(&doc);

    let mut le = layout_engine();
    let tree = le.layout(&doc, &styles, &DefaultTextMeasurer, &DefaultImageMeasurer);

    let b = tree.find_by_node(flex).unwrap();
    assert!(
        (b.content_rect.width - 400.0).abs() < 0.5,
        "empty flex container width should be 400",
    );
}
