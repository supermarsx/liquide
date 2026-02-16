# Local Coordinates + Transform Design Document

## Migration from Absolute Coordinates to Blink-Style Local Coordinates

**Date:** 2026-02-16
**Status:** Proposed
**Scope:** `liquide-layout`, `liquide-paint`, `liquide-hit-test`, `liquide-shell`

---

## Table of Contents

1. [Blink's Coordinate Model](#1-blinks-coordinate-model)
2. [Current Liquide Model](#2-current-liquide-model)
3. [Why the Current Approach is Broken](#3-why-the-current-approach-is-broken)
4. [Proposed Architecture](#4-proposed-architecture)
5. [File-by-File Change Specification](#5-file-by-file-change-specification)
6. [Data Structure Changes](#6-data-structure-changes)
7. [Migration Strategy](#7-migration-strategy)

---

## 1. Blink's Coordinate Model

### 1.1 Local Coordinates in LayoutObjects

In Chromium's Blink engine, **every `LayoutObject` stores its geometry in coordinates local to its containing block**. A `LayoutBox` stores:

- **`PhysicalOffset`**: the offset of this box's border-box from its parent's content-box origin. This is a *relative* value — it does NOT include the accumulated position of all ancestors.
- **`PhysicalSize`**: the border-box dimensions.
- **`PhysicalRect`** for content/padding/border/margin: all expressed relative to the box's own border-box origin (i.e. padding_rect.x = border_left, not an absolute screen coordinate).

The key insight: **no LayoutObject knows its absolute screen position**. It only knows its offset from its parent.

### 1.2 Paint Offset Accumulation

During painting, Blink's `PrePaintTreeWalk` computes a **`PaintOffset`** for each `FragmentData`. This is the accumulated offset from the paint root to the current object's border-box origin. It is computed incrementally during traversal:

```
parent_paint_offset + child.PhysicalOffset → child_paint_offset
```

The `PaintOffset` is stored in `FragmentData` (part of `ObjectPaintProperties`) and is computed once per pre-paint walk. When a painter draws a display item, it uses:

```
display_item_rect = local_rect.offset_by(paint_offset)
```

This means:
- Layout is fast — it only computes local offsets
- Parent repositioning does NOT require touching children
- Only the pre-paint walk recomputes accumulated offsets (and only for dirty subtrees)

### 1.3 Property Trees for Transforms/Clips/Effects

Blink builds **property trees** (Transform, Clip, Effect, Scroll) during `PrePaintTreeWalk`. Each node in these trees represents a coordinate-space-changing operation:

- **TransformPaintPropertyNode**: CSS transforms, scrolling offsets, perspective
- **ClipPaintPropertyNode**: overflow clips, CSS clip-path
- **EffectPaintPropertyNode**: opacity, filters, blend modes
- **ScrollPaintPropertyNode**: scrollable regions

Display items reference their property tree state (which transform/clip/effect nodes apply). The compositor uses these trees to efficiently composite layers without flattening everything.

### 1.4 Hit Testing with Coordinate Transforms

Blink's hit testing does NOT compare screen-space coordinates against absolute rects. Instead:

1. The screen-space point is transformed through the **inverse** property tree path
2. `MapAncestorToLocal()` converts a point from an ancestor's coordinate space to a descendant's local space
3. `MapLocalToAncestor()` does the reverse
4. Each LayoutObject tests against its **local** geometry

The hit test walks the tree in paint order (reverse for front-to-back), and at each level:
```
local_point = MapAncestorToLocal(screen_point)
if local_rect.contains(local_point) → hit
```

This correctly handles CSS transforms, scrolling, and nested coordinate spaces.

### 1.5 Key Blink Architecture Summary

| Aspect | Blink Approach |
|--------|---------------|
| Box coordinates | Local to containing block |
| Position storage | `PhysicalOffset` from parent |
| Absolute position | Computed on-demand via `MapLocalToAncestor` |
| Paint coordinates | Accumulated `PaintOffset` during tree walk |
| Hit testing | Screen→local transform via inverse property tree |
| Parent move | Zero child updates (children's local offsets unchanged) |
| Layout invalidation | Only dirty subtree re-laid-out |

---

## 2. Current Liquide Model

### 2.1 Absolute Coordinates Everywhere

Every `LayoutBox` in liquide stores **absolute screen-space coordinates** in all four rect fields:

**File: [`crates/liquide-layout/src/tree.rs`](crates/liquide-layout/src/tree.rs#L54-L78)**
```rust
pub struct LayoutBox {
    pub content_rect: Rect,   // absolute screen coords
    pub padding_rect: Rect,   // absolute screen coords
    pub border_rect: Rect,    // absolute screen coords
    pub margin_rect: Rect,    // absolute screen coords
    pub children: Vec<LayoutBoxId>,
    // ...
}
```

The `Rect` type stores `(x, y, width, height)` where x,y are **absolute pixel positions from (0,0) screen origin**.

### 2.2 How Absolute Coords Are Computed

#### Block Layout ([`crates/liquide-layout/src/block.rs`](crates/liquide-layout/src/block.rs))

`layout_block()` accepts `offset_x` and `offset_y` parameters that represent the **absolute position** of the parent's content box. Every child layout call receives these absolute offsets.

At [line 16-30](crates/liquide-layout/src/block.rs#L16-L30), the function signature shows the offset propagation:
```rust
pub fn layout_block(
    // ...
    offset_x: f32,    // absolute X of parent content origin
    offset_y: f32,    // absolute Y of parent content origin
    // ...
) -> LayoutBoxId
```

At [lines 475-488](crates/liquide-layout/src/block.rs#L475-L488), the absolute position is computed:
```rust
let content_x = offset_x + mar_left + border_left + pad_left;
let content_y = offset_y + mar_top + border_top + pad_top;
```

Then at [lines 508-519](crates/liquide-layout/src/block.rs#L508-L519) — the critical `offset_box_recursive` call:
```rust
// Children were laid out at offset_x=0.0, so their positions are relative
// to this block's content area origin. Shift them by (content_x, content_y)
if content_x != 0.0 || content_y != 0.0 {
    let child_ids = tree.get(box_id).map(|b| b.children.clone())...;
    for cid in child_ids {
        crate::positioned::offset_box_recursive(tree, cid, content_x, content_y);
    }
}
```

**This is the core problem**: children are laid out at (0,0), then `offset_box_recursive` walks the entire subtree to add the parent's absolute offset to every descendant.

#### Flex Layout ([`crates/liquide-layout/src/flex.rs`](crates/liquide-layout/src/flex.rs))

Same pattern. At [line 127](crates/liquide-layout/src/flex.rs#L127):
```rust
let content_x = offset_x + mar_left + border_left + pad_left;
let content_y = offset_y + mar_top + border_top + pad_top;
```

Children are laid out at `(0.0, 0.0)` ([lines 192-213](crates/liquide-layout/src/flex.rs#L192-L213)), then repositioned by computing `dx = target_x - current_x` and calling `offset_box_recursive` at [lines 545-552](crates/liquide-layout/src/flex.rs#L545-L552):
```rust
let dx = x - b.margin_rect.x;
let dy = y - b.margin_rect.y;
shift_box(b, dx, dy);
if dx != 0.0 || dy != 0.0 {
    for cid in child_ids {
        crate::positioned::offset_box_recursive(tree, cid, dx, dy);
    }
}
```

Cross-axis alignment ([lines 615-650](crates/liquide-layout/src/flex.rs#L615-L650)) and align-content ([lines 578-600](crates/liquide-layout/src/flex.rs#L578-L600)) each trigger **additional** `offset_box_recursive` calls, meaning a single flex item might get its entire subtree walked 3+ times.

#### Positioned Layout ([`crates/liquide-layout/src/positioned.rs`](crates/liquide-layout/src/positioned.rs))

`layout_positioned()` computes absolute coords directly from the containing block's absolute rect at [lines 169-180](crates/liquide-layout/src/positioned.rs#L169-L180):
```rust
let x = if let Some(l) = left { cb.x + l } else ...;
let y = if let Some(t) = top { cb.y + t } else ...;
let content_x = x + border_left + pad_left;
```

The `offset_box_recursive` function itself at [lines 383-410](crates/liquide-layout/src/positioned.rs#L383-L410):
```rust
pub fn offset_box_recursive(tree: &mut LayoutTree, box_id: LayoutBoxId, dx: f32, dy: f32) {
    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect.x += dx;
        b.content_rect.y += dy;
        b.padding_rect.x += dx;
        b.padding_rect.y += dy;
        b.border_rect.x += dx;
        b.border_rect.y += dy;
        b.margin_rect.x += dx;
        b.margin_rect.y += dy;
        if let BoxType::Text { ref mut line_boxes } = b.box_type {
            for lb in line_boxes.iter_mut() { lb.rect.x += dx; lb.rect.y += dy; }
        }
    }
    let children = tree.get(box_id).map(|b| b.children.clone())...;
    for child in children { offset_box_recursive(tree, child, dx, dy); }
}
```

This recursively walks **every descendant** and modifies 4 rects × 2 coordinates = 8 fields per box, plus line boxes.

### 2.3 How Painting Uses Coordinates

The painter at [`crates/liquide-paint/src/painter.rs`](crates/liquide-paint/src/painter.rs) directly reads absolute coordinates from the layout tree:

At [lines 175-178](crates/liquide-paint/src/painter.rs#L175-L178) (background):
```rust
list.push(DisplayItem::SolidColor {
    rect: layout_box.padding_rect,  // absolute coords
    // ...
});
```

At [lines 236-239](crates/liquide-paint/src/painter.rs#L236-L239) (text):
```rust
list.push(DisplayItem::Text {
    rect: layout_box.content_rect,  // absolute coords
    // ...
});
```

The painter does NOT accumulate any offset during traversal — it simply copies the absolute rect from each layout box into the display item. No coordinate transformation happens.

### 2.4 How Hit Testing Uses Coordinates

At [`crates/liquide-hit-test/src/engine.rs`](crates/liquide-hit-test/src/engine.rs), hit testing compares the screen-space point directly against absolute rects:

At [lines 67-70](crates/liquide-hit-test/src/engine.rs#L67-L70):
```rust
fn hit_test_box(&self, box_id: LayoutBoxId, point: Point) -> Option<HitTestResult> {
    let layout_box = self.layout.get(box_id)?;
    if !layout_box.border_rect.contains(point) {  // absolute rect vs screen point
        return None;
    }
```

No coordinate transformation is applied. This means:
- CSS `transform` is completely ignored during hit testing
- Scrolled content will hit-test at wrong positions if scroll offset isn't baked into absolute coords
- Nested transforms (scale inside rotate) cannot be correctly inverted

The ancestor chain construction at [lines 95-101](crates/liquide-hit-test/src/engine.rs#L95-L101) is also O(N²) — it scans ALL boxes to find parents.

### 2.5 How Pipeline Converts to Scene

At [`crates/liquide-shell/src/pipeline.rs`](crates/liquide-shell/src/pipeline.rs#L223), `display_list_to_scene()` maintains a primitive state stack for transforms/clips/opacity but these operate on already-absolute coordinates in the display items. The transform accumulation at [lines 291-309](crates/liquide-shell/src/pipeline.rs#L291-L309) applies to the **absolute-coord rects** from the display list.

---

## 3. Why the Current Approach is Broken

### 3.1 Double-Offset Bugs

The `offset_box_recursive` pattern creates opportunities for double-offsetting:

1. `block.rs` lays out children at `(0, 0)` then offsets them by `(content_x, content_y)`
2. If a flex container is nested inside a block, `flex.rs` also calls `offset_box_recursive` on its children
3. `positioned.rs` may offset children from an intrinsic layout pass, then offset them again to the final position

When the order of these offset passes is wrong, or when both the parent and child's layout function apply offsets, coordinates get doubled. The comment in `block.rs` at [line 508](crates/liquide-layout/src/block.rs#L508) explicitly acknowledges this:
```rust
// Children were laid out at offset_x=0.0, so their positions are relative
// to this block's content area origin. Shift them by (content_x, content_y)
// so all boxes in the tree use absolute screen-space coordinates.
```

But this assumes children were always laid out at `(0,0)`, which breaks when:
- A child's layout function also receives non-zero `offset_x/offset_y` from another call path
- Re-layout in flex step 4b re-creates boxes at `(0,0)` but the containing flex's `shift_box` expects them to still be at `(0,0)` — if they got partially offset before re-layout, the delta calculation `dx = x - b.margin_rect.x` will be wrong

### 3.2 Fragile Coordinate Propagation

**Problem: Multiple offset passes on the same subtree.**

In `flex.rs`, a single item may be offset **three separate times**:

1. **Step 5** ([line 539-552](crates/liquide-layout/src/flex.rs#L539-L552)): Position items on main axis → `shift_box + offset_box_recursive`
2. **Step 6 align-content** ([line 578-600](crates/liquide-layout/src/flex.rs#L578-L600)): Multi-line cross distribution → `shift_box + offset_box_recursive`
3. **Step 6 align-items** ([line 637-650](crates/liquide-layout/src/flex.rs#L637-L650)): Per-item cross alignment → `shift_box + offset_box_recursive`

Each pass walks the **entire subtree** of every flex item. For a 3-deep nested flex layout with 10 items each, that's `10 × 10 × 10 = 1000` leaf nodes, each walked 3 times per ancestor level = **9,000 recursive walks** where with local coordinates it would be **0**.

### 3.3 Layout Invalidation Forces Full Re-Offset

Because every box stores absolute coordinates, **any change to a parent's position requires re-offsetting the entire subtree**. In Blink, moving a parent just changes its `PhysicalOffset` — children are untouched because their local offsets haven't changed. In liquide:

- Scrolling requires re-offsetting all visible content
- Window resize triggers full relayout + re-offset of every box
- Animation of `top`/`left` on a positioned element forces `offset_box_recursive` on all descendants every frame

### 3.4 Hit Testing Cannot Handle Transforms

The current hit-test at [engine.rs](crates/liquide-hit-test/src/engine.rs) compares the raw screen point against `border_rect` which is the **pre-transform** absolute position. If a CSS `transform: rotate(45deg)` is applied, the visual position on screen differs from `border_rect`, but the hit test still uses the untransformed rect.

### 3.5 O(N) Full-Tree Walks During Layout

`offset_box_recursive` has **O(N)** cost where N = subtree size. It is called:
- Once per block box (in `block.rs` — [line 512](crates/liquide-layout/src/block.rs#L512))
- Up to 3× per flex item (in `flex.rs` — lines 548, 593, 645)
- Once per positioned element relocation (in `positioned.rs` — [line 246](crates/liquide-layout/src/positioned.rs#L246))

For a tree of depth D with branching factor B, the total work is O(B^D × D) — each level must offset everything below it. With local coordinates, layout is O(B^D) — each node computed once.

---

## 4. Proposed Architecture

### 4.1 Overview

Adopt Blink's approach with three key changes:

1. **`LayoutBox` stores local coordinates** relative to parent's content-box origin
2. **Painter accumulates a `PaintOffset`** during tree traversal to convert local → absolute for display items
3. **Hit-test engine transforms screen coordinates** to local coordinates using the inverse of the accumulated offset/transform stack

### 4.2 New Coordinate Semantics

```
LayoutBox.content_rect.x = padding_left           (relative to own border-box)
LayoutBox.content_rect.y = padding_top             (relative to own border-box)
LayoutBox.border_rect.x  = 0.0                     (always zero — it IS the origin)
LayoutBox.border_rect.y  = 0.0                     (always zero)
LayoutBox.offset.x       = margin_left + position  (relative to parent content-box)
LayoutBox.offset.y       = margin_top + position   (relative to parent content-box)
```

### 4.3 PaintOffset Accumulation (Painter)

```
fn paint_box(layout, box_id, paint_offset):
    box = layout.get(box_id)
    absolute_border_origin = paint_offset + box.offset
    absolute_content_origin = absolute_border_origin + (border_left + pad_left, border_top + pad_top)

    emit DisplayItem::SolidColor { rect: absolute_padding_rect }
    emit DisplayItem::Text { rect: absolute_content_rect }

    for child in box.children:
        child_paint_offset = absolute_content_origin  // children are relative to content-box
        paint_box(layout, child, child_paint_offset)
```

### 4.4 Hit-Test Coordinate Transform

```
fn hit_test_box(layout, box_id, local_point):
    box = layout.get(box_id)

    // Transform point from parent-content-space to this box's border-box space
    let point_in_border_box = local_point - box.offset

    // Apply inverse CSS transform if any
    if box has transform:
        point_in_border_box = inverse_transform(point_in_border_box)

    if !box.border_rect.contains(point_in_border_box):
        return None

    // Point in content-box space for children
    let point_in_content = point_in_border_box - (border_left + pad_left, border_top + pad_top)

    for child in children.rev():
        if let Some(hit) = hit_test_box(layout, child, point_in_content):
            return Some(hit)

    return Some(HitTestResult { node: box.node, point_in_node: point_in_content })
```

---

## 5. File-by-File Change Specification

### 5.1 `crates/liquide-layout/src/geometry.rs`

**Add `PhysicalOffset` type:**

```rust
/// Offset from a parent's content-box origin to a child's border-box origin.
/// Equivalent to Blink's PhysicalOffset.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PhysicalOffset {
    pub x: f32,
    pub y: f32,
}

impl PhysicalOffset {
    pub fn new(x: f32, y: f32) -> Self { Self { x, y } }
    pub fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
}

impl std::ops::Add for PhysicalOffset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self { x: self.x + rhs.x, y: self.y + rhs.y } }
}
```

**Modify `Rect`:** Add helper method for offset application:

```rust
impl Rect {
    /// Create a new rect at an absolute position by applying an offset.
    pub fn at_offset(&self, offset: PhysicalOffset) -> Self {
        Self { x: self.x + offset.x, y: self.y + offset.y, ..*self }
    }
}
```

### 5.2 `crates/liquide-layout/src/tree.rs`

**Current (lines 54-78):** All rects store absolute coordinates, no offset field.

**Proposed changes:**

```rust
pub struct LayoutBox {
    pub id: LayoutBoxId,
    pub node: NodeId,
    pub box_type: BoxType,

    // NEW: offset from parent's content-box origin to this box's border-box origin.
    // Encodes: margin + relative position offset.
    // Equivalent to Blink's PhysicalOffset on LayoutBox.
    pub offset: PhysicalOffset,

    // CHANGED: All rects are now LOCAL to this box's border-box origin.
    // content_rect.x = border_left + padding_left (offset from border-box origin)
    // border_rect.x = 0.0 (always, border-box IS the origin)
    pub content_rect: Rect,   // local to border-box
    pub padding_rect: Rect,   // local to border-box
    pub border_rect: Rect,    // local to border-box (origin always 0,0)
    pub margin_rect: Rect,    // local — extends negatively from border-box

    pub children: Vec<LayoutBoxId>,
    pub baseline: Option<f32>,
    pub scroll_size: Option<Size>,

    // NEW: parent box ID for efficient ancestor walks
    pub parent: Option<LayoutBoxId>,
}
```

**Key semantic change:** `border_rect.x` and `border_rect.y` will always be `0.0`. The position information that was in `border_rect.x/y` moves to `offset.x/y`. Content rect x/y become the inset from border-box origin (= `border_left + padding_left`).

**Add helper methods:**

```rust
impl LayoutBox {
    /// Compute the absolute border-box rect given an accumulated paint offset.
    pub fn absolute_border_rect(&self, paint_offset: PhysicalOffset) -> Rect {
        self.border_rect.at_offset(paint_offset + self.offset)
    }

    /// Compute the absolute content rect given an accumulated paint offset.
    pub fn absolute_content_rect(&self, paint_offset: PhysicalOffset) -> Rect {
        self.content_rect.at_offset(paint_offset + self.offset)
    }
}
```

**Add to `LayoutTree`:**

```rust
impl LayoutTree {
    /// Compute absolute position of a box by walking to root.
    /// Used for debugging and rare queries. NOT for hot-path painting.
    pub fn absolute_offset(&self, box_id: LayoutBoxId) -> PhysicalOffset {
        let mut offset = PhysicalOffset::zero();
        let mut current = Some(box_id);
        while let Some(id) = current {
            if let Some(b) = self.get(id) {
                offset = PhysicalOffset::new(
                    offset.x + b.offset.x,
                    offset.y + b.offset.y,
                );
                current = b.parent;
            } else {
                break;
            }
        }
        offset
    }
}
```

### 5.3 `crates/liquide-layout/src/block.rs`

**Remove `offset_x` and `offset_y` parameters** from `layout_block()` signature.

**Current signature (line 16):**
```rust
pub fn layout_block(doc, node_id, styles, tree, text_measurer, image_measurer,
    container_width, container_height, offset_x, offset_y, viewport_w, viewport_h, base_font_size)
```

**New signature:**
```rust
pub fn layout_block(doc, node_id, styles, tree, text_measurer, image_measurer,
    container_width, container_height, viewport_w, viewport_h, base_font_size)
```

**Current coordinate computation (line 475-488):**
```rust
let content_x = offset_x + mar_left + border_left + pad_left;
let content_y = offset_y + mar_top + border_top + pad_top;
// ... sets content_rect to absolute (content_x, content_y, ...)
```

**New coordinate computation:**
```rust
// Offset from parent's content-box to this box's border-box
let offset = PhysicalOffset::new(mar_left, child_y_in_parent); // y set by caller or tracked here

// Local rects (relative to own border-box origin)
let content_x_local = border_left + pad_left;
let content_y_local = border_top + pad_top;

b.offset = offset;
b.content_rect = Rect::new(content_x_local, content_y_local, content_width, content_height);
b.padding_rect = Rect::new(border_left, border_top,
    content_width + pad_left + pad_right,
    content_height + pad_top + pad_bottom);
b.border_rect = Rect::new(0.0, 0.0,
    b.padding_rect.width + border_left + border_right,
    b.padding_rect.height + border_top + border_bottom);
b.margin_rect = Rect::new(-mar_left, -mar_top,
    b.border_rect.width + mar_left + mar_right,
    b.border_rect.height + mar_top + mar_bottom);
```

**DELETE the `offset_box_recursive` call at lines 508-519.** This is the **entire point** of the migration — children's local coordinates don't need parent offsets applied.

**Child layout calls change from:**
```rust
layout_block(doc, child_id, styles, tree, ..., 0.0, child_y, ...)
```
**To:**
```rust
layout_block(doc, child_id, styles, tree, ..., content_width, container_height, ...)
// After: set child's offset.y = child_y (tracked by the block BFC)
```

The parent sets each child's `offset` after the child returns, based on the block formatting context's cursor position (`child_y`).

### 5.4 `crates/liquide-layout/src/flex.rs`

**Same parameter removal** — remove `offset_x`, `offset_y` from `layout_flex()`.

**Current signature (line 24):**
```rust
pub fn layout_flex(doc, node_id, styles, tree, text_measurer, image_measurer,
    container_width, container_height, offset_x, offset_y, viewport_w, viewport_h, base_font_size)
```

**New signature:**
```rust
pub fn layout_flex(doc, node_id, styles, tree, text_measurer, image_measurer,
    container_width, container_height, viewport_w, viewport_h, base_font_size)
```

**Step 5 positioning (lines 530-552) changes from:**
```rust
let (x, y) = if is_row {
    (content_x + main_pos, content_y + cross_offset)  // absolute
};
let dx = x - b.margin_rect.x;
let dy = y - b.margin_rect.y;
shift_box(b, dx, dy);
// WALK ENTIRE SUBTREE:
offset_box_recursive(tree, cid, dx, dy);
```

**To:**
```rust
let (offset_x, offset_y) = if is_row {
    (main_pos, cross_offset)  // local to flex content-box
} else {
    (cross_offset, main_pos)
};
// Just set the child's offset — NO subtree walk
tree.get_mut(item.box_id).unwrap().offset = PhysicalOffset::new(offset_x, offset_y);
```

**DELETE all `offset_box_recursive` calls in flex.rs** (currently at 3 locations: step 5, align-content, align-items). Replace with simple `offset` field updates on the flex item box directly.

**Align-items cross offset (lines 637-650) becomes:**
```rust
tree.get_mut(item.box_id).unwrap().offset.y += cross_offset_val; // row axis
// No subtree walk needed!
```

**`shift_box` helper (line 757-767) becomes unnecessary.** Replace with:
```rust
fn adjust_offset(b: &mut LayoutBox, dx: f32, dy: f32) {
    b.offset.x += dx;
    b.offset.y += dy;
}
```

### 5.5 `crates/liquide-layout/src/positioned.rs`

**Major changes:**

**`layout_positioned()` (line 14):**
- Remove `containing_rect` parameter (which contained absolute coords)
- Instead accept `containing_size: Size` (just width/height of the containing block)
- Compute `offset` as the position within the containing block

**Current (lines 169-180):**
```rust
let x = if let Some(l) = left { cb.x + l } ...;  // cb.x is absolute
let content_x = x + border_left + pad_left;        // absolute
```

**New:**
```rust
let offset_x = if let Some(l) = left { l } ...;    // local to containing block
let offset_y = if let Some(t) = top { t } ...;     // local to containing block
b.offset = PhysicalOffset::new(offset_x, offset_y);
// content_rect/border_rect are self-local as usual
```

**DELETE `offset_box_recursive` function entirely** (lines 383-410). This function should not exist in the local coordinate model. If callers are identified that still need it during migration, they can use the `offset` field instead.

**DELETE `layout_children_in_positioned`'s absolute offset passing** — children are laid out with local coordinates, their `offset` is set by the parent.

### 5.6 `crates/liquide-paint/src/painter.rs`

**Add `PaintOffset` accumulation to the paint walk.**

**Current `paint_box` signature (line 35):**
```rust
fn paint_box(&self, doc, layout, styles, box_id, list)
```

**New signature:**
```rust
fn paint_box(&self, doc, layout, styles, box_id, paint_offset: PhysicalOffset, list)
```

**Current coordinate usage (e.g. line 175):**
```rust
list.push(DisplayItem::SolidColor {
    rect: layout_box.padding_rect,  // absolute from layout
});
```

**New coordinate usage:**
```rust
let absolute_offset = paint_offset + layout_box.offset;
let abs_padding = layout_box.padding_rect.at_offset(absolute_offset);
let abs_border = layout_box.border_rect.at_offset(absolute_offset);
let abs_content = layout_box.content_rect.at_offset(absolute_offset);

list.push(DisplayItem::SolidColor {
    rect: abs_padding,  // computed from local + accumulated offset
});
```

**Child paint calls change from:**
```rust
self.paint_box(doc, layout, styles, child_id, list);
```

**To:**
```rust
// Children are relative to this box's content-box origin
let child_paint_offset = absolute_offset + PhysicalOffset::new(
    layout_box.content_rect.x,  // = border_left + pad_left
    layout_box.content_rect.y,  // = border_top + pad_top
);
self.paint_box(doc, layout, styles, child_id, child_paint_offset, list);
```

**Entry point change — `paint()` at line 24:**
```rust
pub fn paint(&self, doc, layout, styles) -> DisplayList {
    let mut list = DisplayList::new();
    self.paint_box(doc, layout, styles, layout.root, PhysicalOffset::zero(), &mut list);
    list
}
```

### 5.7 `crates/liquide-hit-test/src/engine.rs`

**Transform screen coordinates to local coordinates during traversal.**

**Current `hit_test_box` (line 67):**
```rust
fn hit_test_box(&self, box_id: LayoutBoxId, point: Point) -> Option<HitTestResult> {
    let layout_box = self.layout.get(box_id)?;
    if !layout_box.border_rect.contains(point) {  // screen point vs absolute rect
        return None;
    }
```

**New `hit_test_box`:**
```rust
/// `point_in_parent_content` is the test point in the parent's content-box coordinate space.
fn hit_test_box(&self, box_id: LayoutBoxId, point_in_parent_content: Point) -> Option<HitTestResult> {
    let layout_box = self.layout.get(box_id)?;

    // Transform from parent-content-space to this box's border-box space
    let point_in_border = Point::new(
        point_in_parent_content.x - layout_box.offset.x,
        point_in_parent_content.y - layout_box.offset.y,
    );

    // TODO: Apply inverse CSS transform here for transform support
    // let point_in_border = inverse_transform(point_in_border, &style.transform);

    // Check against local border rect (origin 0,0)
    if !layout_box.border_rect.contains(point_in_border) {
        return None;
    }

    // Check pointer-events
    if let Some(style) = self.styles.get(layout_box.node) {
        if style.pointer_events == PointerEvents::None {
            return None;
        }
    }

    // Transform to content-box space for child testing
    let point_in_content = Point::new(
        point_in_border.x - layout_box.content_rect.x,  // subtract border+padding inset
        point_in_border.y - layout_box.content_rect.y,
    );

    // Test children in reverse (front-to-back)
    let children = layout_box.children.clone();
    for &child_id in children.iter().rev() {
        if let Some(result) = self.hit_test_box(child_id, point_in_content) {
            return Some(result);
        }
    }

    Some(HitTestResult {
        node: layout_box.node,
        point_in_node: point_in_content,
        ancestors: self.build_ancestors(box_id),
    })
}
```

**Entry point `hit_test` (line 56):**
```rust
pub fn hit_test(&self, screen_point: Point) -> Option<HitTestResult> {
    // Root box's parent is the viewport — point is already in viewport space
    self.hit_test_box(self.layout.root, screen_point)
}
```

**Fix ancestor chain building** — use the new `parent` field instead of O(N²) scan:
```rust
fn build_ancestors(&self, box_id: LayoutBoxId) -> Vec<NodeId> {
    let mut ancestors = Vec::new();
    let mut current = self.layout.get(box_id).and_then(|b| b.parent);
    while let Some(pid) = current {
        if let Some(p) = self.layout.get(pid) {
            ancestors.push(p.node);
            current = p.parent;
        } else {
            break;
        }
    }
    ancestors
}
```

### 5.8 `crates/liquide-shell/src/pipeline.rs`

**`display_list_to_scene` (line 223):** No major changes needed here because the painter already converts local→absolute before emitting display items. The display items in the list will contain absolute screen-space rects (computed by the painter's offset accumulation). The transform/clip/opacity state stack remains the same.

**`extract_glass_nodes` (line 186):** Currently iterates `layout.boxes` and reads `border_rect` directly. With local coordinates, this needs to compute absolute rects:

```rust
fn extract_glass_nodes(&mut self, output: &PipelineOutput, base_z: u32) -> Vec<SceneNode> {
    // Need to compute absolute rects for glass nodes
    // Option A: Walk tree accumulating offsets (like painter does)
    // Option B: Cache absolute rects during paint and store alongside
    // Recommended: walk tree once, accumulate offsets
    let absolute_rects = self.compute_absolute_rects(&output.layout);
    for (box_id, layout_box) in output.layout.boxes.iter().enumerate() {
        if let Some(style) = output.styles.get(layout_box.node) {
            if style.x_blur_radius > 0.0 {
                let rect = absolute_rects[box_id]; // pre-computed absolute rect
                // ... rest unchanged
            }
        }
    }
}
```

### 5.9 `crates/liquide-layout/src/engine.rs`

**Layout engine main entry (line 29):** Remove `offset_x: 0.0, offset_y: 0.0` from all top-level layout calls since these parameters are removed.

**`layout_positioned_elements` (line 152):** Change `containing_rect` (absolute Rect) to `containing_size` (Size):

**Current (lines 164-167):**
```rust
let containing_rect = tree.find_by_node(node_id)
    .map(|b| b.padding_rect)  // absolute rect
```

**New:**
```rust
let containing_size = tree.find_by_node(node_id)
    .map(|b| Size::new(b.padding_rect.width, b.padding_rect.height))
```

**Add parent tracking:** After `tree.add_child(parent, child)`, also set `child.parent = Some(parent)`.

### 5.10 Other Layout Modules

The same changes apply to these files (remove `offset_x`/`offset_y`, use local coords, delete `offset_box_recursive` calls):

| File | Key changes |
|------|-------------|
| `crates/liquide-layout/src/grid.rs` | Remove offset params, local coords |
| `crates/liquide-layout/src/table.rs` | Remove offset params, local coords |
| `crates/liquide-layout/src/multicol.rs` | Remove offset params, local coords |
| `crates/liquide-layout/src/inline.rs` | Remove offset params, local coords |
| `crates/liquide-layout/src/float.rs` | Remove offset params, local coords |

---

## 6. Data Structure Changes Summary

### 6.1 Before vs After: LayoutBox

| Field | Before | After |
|-------|--------|-------|
| `offset` | (doesn't exist) | **NEW** — `PhysicalOffset` from parent content-box to this border-box |
| `content_rect.x` | absolute screen X | local: `border_left + pad_left` |
| `content_rect.y` | absolute screen Y | local: `border_top + pad_top` |
| `border_rect.x` | absolute screen X | `0.0` (always) |
| `border_rect.y` | absolute screen Y | `0.0` (always) |
| `padding_rect.x` | absolute screen X | local: `border_left` |
| `padding_rect.y` | absolute screen Y | local: `border_top` |
| `margin_rect.x` | absolute screen X | local: `-margin_left` |
| `margin_rect.y` | absolute screen Y | local: `-margin_top` |
| `parent` | (doesn't exist) | **NEW** — `Option<LayoutBoxId>` for ancestor walks |

### 6.2 Before vs After: Function Signatures

| Function | Before params | After params |
|----------|--------------|--------------|
| `layout_block()` | `offset_x, offset_y` | (removed) |
| `layout_flex()` | `offset_x, offset_y` | (removed) |
| `layout_grid()` | `offset_x, offset_y` | (removed) |
| `layout_table()` | `offset_x, offset_y` | (removed) |
| `layout_multicol()` | `offset_x, offset_y` | (removed) |
| `layout_inline()` | `offset_x, offset_y` | (removed) |
| `layout_positioned()` | `containing_rect: Rect` | `containing_size: Size` |
| `Painter::paint_box()` | (no offset) | `paint_offset: PhysicalOffset` |
| `HitTestEngine::hit_test_box()` | `point: Point` (screen) | `point_in_parent_content: Point` (local) |

### 6.3 Deleted Code

| Item | Location | Reason |
|------|----------|--------|
| `offset_box_recursive()` | `positioned.rs:383-410` | Entire function deleted — no longer needed |
| `shift_box()` | `flex.rs:757-767` | Replaced by `offset` field updates |
| `offset_x`/`offset_y` params | All layout functions | Absolute offsets no longer propagated |
| O(N²) ancestor scan | `engine.rs:95-101` | Replaced by `parent` field |

---

## 7. Migration Strategy

### Phase 1: Add `offset` and `parent` fields (non-breaking)

1. Add `offset: PhysicalOffset` field to `LayoutBox` (defaults to `PhysicalOffset::zero()`)
2. Add `parent: Option<LayoutBoxId>` field to `LayoutBox`
3. Populate `parent` in `LayoutTree::add_child()`
4. All existing code continues to work — offset field is unused

### Phase 2: Migrate layout functions (one at a time)

For each layout module (`block`, `flex`, `grid`, `table`, `inline`, `multicol`, `positioned`):

1. Remove `offset_x`/`offset_y` parameters
2. Compute local coordinates instead of absolute
3. Set `offset` field instead of baking position into rects
4. Remove `offset_box_recursive` calls
5. Update all callers to not pass offset params

**Order:** `block.rs` → `flex.rs` → `positioned.rs` → others (block is the foundation; flex depends on block; positioned depends on both)

### Phase 3: Update painter

1. Add `paint_offset: PhysicalOffset` parameter to `paint_box()`
2. Compute absolute rects from `local_rect + paint_offset + offset`
3. Pass `child_paint_offset` to child traversal
4. Update `paint()` entry point

### Phase 4: Update hit-test engine

1. Change `hit_test_box` to accept `point_in_parent_content`
2. Subtract `offset` to get point in border-box space
3. Subtract content inset to get point in content-box space for children
4. Fix ancestor chain via `parent` field
5. Add inverse transform support (future)

### Phase 5: Update pipeline and consumers

1. Fix `extract_glass_nodes` to compute absolute rects
2. Update any code that reads `border_rect.x/y` as absolute coordinates
3. Add `absolute_offset()` utility for callers that need screen-space positions

### Testing Strategy

After each phase, all existing tests should pass (the output — display list items and hit test results — should produce the same absolute positions). The internal representation changes but the externally-visible behavior stays identical.

Key test: for any `LayoutBox`, verify:
```
old_absolute_border_rect == new_tree.absolute_offset(box_id) + new_box.border_rect
```

---

## Appendix A: Performance Analysis

### Current Cost Model

For a tree of N nodes, depth D, branching factor B:

| Operation | Current cost | With local coords |
|-----------|-------------|-------------------|
| Full layout | O(N × D) due to offset_box_recursive per level | O(N) — each node processed once |
| Move parent | O(subtree_size) — re-offset all children | O(1) — change parent's offset only |
| Flex layout (per item) | O(subtree × 3) — three offset passes | O(1) — three field assignments |
| Paint | O(N) — copy absolute rects | O(N) — accumulate + copy (same) |
| Hit test | O(N) per test (flat scan) | O(D) per test (tree descent) |
| Ancestor chain | O(N²) — scan all boxes | O(D) — walk parent pointers |

### Estimated Savings

For a typical desktop shell with ~5000 layout boxes, depth ~15:

- **Layout**: ~15× fewer memory writes (no recursive offsetting)
- **Flex re-position**: ~3× fewer subtree walks per item
- **Hit test**: ~300× faster (O(15) vs O(5000))
- **Ancestor lookup**: ~300× faster (O(15) vs O(5000))
- **Total layout time**: estimated 40-60% reduction for deep nested layouts

---

## Appendix B: Future Work Enabled by Local Coordinates

1. **CSS Transform hit-testing**: With local coordinates, applying inverse transforms at each level is natural
2. **Scroll offset as transform node**: Scroll can be a coordinate-space change in the property tree, not a re-offset of content
3. **Incremental re-layout**: Only re-layout dirty subtree; siblings and ancestors untouched
4. **Layer caching**: Display item subsequences for unchanged subtrees (like Blink's subsequence caching)
5. **Property tree compositor**: Transform/Clip/Effect trees for GPU-composited animations
6. **Contain: paint isolation**: Subtrees with `contain: paint` become independent coordinate islands
