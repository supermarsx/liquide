# LiquiDE Layout Engine — Blink Parity Analysis

This document provides a detailed comparison between `liquide-layout` and Chromium's Blink layout engine, analyzing each major layout module against the relevant CSS specifications.

---

## Executive Summary

| Layout Module | Implementation Status | Blink Parity Estimate |
|---------------|----------------------|----------------------|
| **Flexbox** (`flex.rs`) | Functional | ~75% |
| **Grid** (`grid.rs`) | Functional | ~55% |
| **Block** (`block.rs`) | Functional | ~70% |
| **Inline** (`inline.rs`) | Functional | ~50% |
| **Table** (`table.rs`) | Functional | ~45% |
| **Positioned** (`positioned.rs`) | Functional | ~65% |
| **Float** (`float.rs`) | Functional | ~55% |
| **Multi-column** (`multicol.rs`) | Functional | ~40% |

**Overall Layout Module Parity: ~57%**

---

## 1. Flexbox (`flex.rs`) — CSS Flexible Box Layout Level 1

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| `flex-direction` (row/row-reverse/column/column-reverse) | ✅ Full | Proper axis handling |
| `flex-wrap` (nowrap/wrap/wrap-reverse) | ✅ Full | Multi-line support |
| `flex-grow` distribution | ✅ Full | Proportional growth |
| `flex-shrink` distribution | ✅ Full | Weighted shrink with `flex-basis × flex-shrink` factor |
| `flex-basis` | ✅ Full | Resolved before grow/shrink |
| `order` property | ✅ Full | Items sorted by order, then reversed if needed |
| `justify-content` | ✅ Full | flex-start/end/center/space-between/around/evenly |
| `align-items` | ✅ Full | flex-start/end/center/stretch/baseline |
| `align-self` | ✅ Full | Per-item override of align-items |
| `align-content` | ✅ Full | Multi-line cross-axis distribution |
| `gap` (row-gap/column-gap) | ✅ Full | Proper main-axis and cross-axis gaps |
| Min/max size clamping | ✅ Full | `min_main` / `max_main` respected during grow/shrink |
| Nested flex containers | ✅ Full | Recursive layout dispatch |
| Aspect ratio fallback | ✅ Partial | Applied when cross-size is 0 |
| Re-layout after grow/shrink | ✅ Full | Children re-laid out at resolved size |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Auto margins on flex items** | §8.1 | High — auto margins should absorb free space before justify-content |
| **Intrinsic main-size contribution** | §9.2 | Medium — content-based sizing not fully computed |
| **Definite cross-size in nested flex** | §9.3 | Medium — percentage resolution in nested contexts |
| **Baseline alignment (full algorithm)** | §8.4 | Medium — currently simplified to 0 offset |
| **`visibility: collapse` handling** | §4.4 | Low — collapsed items should leave gaps |
| **Safe/unsafe alignment** | CSS Box Alignment | Low — no overflow-safe keyword support |
| **`flex: none` optimization** | §7.2.2 | Low — works but not optimized |

### Gap Analysis vs Blink

```
Blink supports:
- Full intrinsic sizing algorithm (min-content/max-content/fit-content)
- Proper baseline alignment with first/last baseline calculation
- Auto margin absorption for alignment
- Safe alignment fallback for overflow cases
- Proper definite size resolution in nested contexts
- Writing mode awareness (vertical-rl, etc.)

liquide-layout lacks:
- Auto margin handling in flex containers
- Full intrinsic sizing calculation
- Writing mode support
- Safe alignment keywords
```

**Flexbox Parity: ~75%**

---

## 2. Grid (`grid.rs`) — CSS Grid Layout Level 1 & 2

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| `grid-template-columns` / `grid-template-rows` | ✅ Full | px, %, fr, auto, minmax, min-content, max-content |
| `grid-template-areas` | ✅ Partial | Parsed to named line mappings |
| `grid-column-start/end` | ✅ Full | Line numbers, including negative |
| `grid-row-start/end` | ✅ Full | Line numbers, including negative |
| `grid-auto-flow` (row/column, dense) | ✅ Full | Dense packing implemented |
| `grid-auto-columns` / `grid-auto-rows` | ✅ Full | Implicit track sizing |
| `gap` (row-gap/column-gap) | ✅ Full | Inter-track gaps |
| Item spanning (`span N`) | ✅ Full | Both explicit and implicit |
| Auto-placement algorithm | ✅ Full | Row-major and column-major |
| `justify-items` / `justify-self` | ✅ Full | start/end/center/stretch |
| Subgrid detection | ✅ Partial | `TrackSize::Subgrid` recognized, inherits parent tracks |
| Negative line numbers | ✅ Full | Resolved via `resolve_grid_line()` |
| fr unit distribution | ✅ Full | Space distribution after fixed tracks |
| minmax() support | ✅ Full | Min size used initially, expansion toward max |
| fit-content() | ✅ Partial | Acts as minmax(auto, percentage) |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Full track sizing algorithm** | CSS Grid §11 | High — Blink uses multi-pass algorithm for intrinsic sizing |
| **Grid template areas placement** | §8.3 | Medium — names parsed but not used for placement lookup |
| **Named lines** | §8.1 | Medium — line names in definitions not supported |
| **`repeat()` function** | §7.2.3 | Medium — not parsed (may be in style engine) |
| **`auto-fill` / `auto-fit`** | §7.2.3.2 | High — no implicit track repetition |
| **Subgrid (full CSS Grid Level 2)** | Grid Level 2 | Medium — detection only, no proper subgrid layout |
| **Masonry layout** | CSS Grid Level 3 | Low — experimental spec |
| **Align-items/content** | CSS Box Alignment | Medium — justify implemented, align missing |
| **Baseline alignment** | §10.6 | Low — items not baseline-aligned |
| **Minimum contribution sizing** | §11.5 | Medium — intrinsic sizing simplified |

### Gap Analysis vs Blink

```
Blink supports:
- Full track sizing algorithm (growth limits, base sizes, intrinsic contributions)
- repeat(auto-fill | auto-fit, track-list)
- Named lines and named areas for placement
- Full subgrid with inherited line names
- All alignment properties (justify-*/align-*)
- Baseline alignment for grid items
- Masonry (behind flag)

liquide-layout lacks:
- repeat() with auto-fill/auto-fit
- Named line resolution
- Full subgrid implementation
- Cross-axis alignment (align-items/content)
- Multi-pass track sizing algorithm
```

**Grid Parity: ~55%**

---

## 3. Block (`block.rs`) — CSS 2.1 Visual Formatting Model

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| Block formatting context (BFC) | ✅ Full | Vertical stacking of children |
| Margin collapsing (sibling) | ✅ Full | Proper positive/negative handling |
| `box-sizing` (content-box/border-box) | ✅ Full | Width/height interpretation |
| `min-width` / `max-width` | ✅ Full | Constraint application |
| `min-height` / `max-height` | ✅ Full | Constraint application |
| Auto height calculation | ✅ Full | Sum of children |
| Auto margin centering | ✅ Full | Horizontal auto margins |
| `aspect-ratio` | ✅ Full | Height from width when no explicit height |
| `contain: size` | ✅ Full | Uses contain-intrinsic-width/height |
| `zoom` factor | ✅ Full | Scales content dimensions |
| `line-clamp` | ✅ Full | Limits height to N lines |
| `scrollbar-gutter` | ✅ Full | Reserves space for scrollbar |
| `overflow: scroll/auto` | ✅ Full | Creates scroll container with scroll_size |
| `::before` / `::after` | ✅ Full | Generated content boxes |
| `display: contents` | ✅ Full | Children promoted to parent |
| `display: list-item` | ✅ Full | Marker generation (disc/circle/square/decimal/roman/alpha) |
| `display: inline-block` | ✅ Partial | Laid out as block |
| Counter properties | ✅ Consumed | counter-increment/reset/set read |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Parent-child margin collapsing** | CSS 2.1 §8.3.1 | High — only sibling collapsing implemented |
| **Margin collapsing through empty blocks** | CSS 2.1 §8.3.1 | Medium — not handled |
| **BFC prevents margin collapse** | CSS 2.1 §9.4.1 | Medium — flag exists but not fully applied |
| **Clearance calculation** | CSS 2.1 §9.5.2 | Medium — floats handled separately |
| **Min-content / max-content sizing** | CSS Sizing Level 3 | Medium — intrinsic sizing basic |
| **fit-content sizing** | CSS Sizing Level 3 | Low — not implemented |
| **Counter state machine** | CSS Lists Level 3 | Low — properties read but no registry |
| **Anonymous block boxes** | CSS 2.1 §9.2.1.1 | Low — implicit in layout |

### Gap Analysis vs Blink

```
Blink supports:
- Full margin collapsing (parent-child, through empty blocks)
- Proper BFC establishment and margin isolation
- Full intrinsic sizing algorithm
- Anonymous block/inline box generation
- Counter state machine with scope
- Clearance calculation for floats

liquide-layout lacks:
- Parent-child margin collapsing
- Margin collapse through empty blocks
- Full intrinsic sizing
- Counter state machine
```

**Block Parity: ~70%**

---

## 4. Inline (`inline.rs`) — CSS Inline Layout Level 3

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| Line box construction | ✅ Full | Words and spaces tokenized into fragments |
| Line breaking (soft wrap) | ✅ Full | Respects max_width |
| `text-align` (left/right/center/justify) | ✅ Full | Fragment positioning |
| `text-align-last` | ✅ Full | Last line alignment |
| `text-indent` | ✅ Full | First line indentation |
| `white-space` modes | ✅ Full | normal/pre/pre-wrap/pre-line/nowrap |
| Whitespace collapsing | ✅ Full | Proper mode detection |
| Newline preservation | ✅ Full | Forced breaks in pre modes |
| `vertical-align` | ✅ Full | baseline/top/bottom/middle/sub/super/length |
| Inline box model (margin/padding/border) | ✅ Full | InlineEdges propagated |
| Nested inline elements | ✅ Full | Open/Close inline markers |
| `overflow-wrap: break-word` | ✅ Partial | Forces word onto line, no character split |
| `text-wrap-mode: nowrap` | ✅ Full | Overrides wrapping |
| Baseline tracking | ✅ Full | First baseline stored |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **BiDi algorithm (UAX #9)** | CSS Writing Modes | High — no RTL/LTR reordering |
| **Character-level line breaking** | CSS Text Level 3 | High — no grapheme splitting for overflow-wrap |
| **`word-break: break-all`** | CSS Text Level 3 §5.2 | Medium — not implemented |
| **`hyphens: auto`** | CSS Text Level 3 §6 | Medium — no hyphenation dictionary |
| **Full line-height calculation** | CSS Inline §4 | Medium — simplified |
| **Leading trim (`text-box-trim`)** | CSS Inline Level 3 | Low — property consumed but not applied |
| **`initial-letter`** | CSS Inline Level 3 §5 | Low — property consumed |
| **Ruby annotation** | CSS Ruby Level 1 | Low — properties consumed |
| **Justification algorithm** | CSS Text Level 3 §7 | Medium — spacing not distributed |
| **`text-justify` implementation** | CSS Text Level 3 §7.3 | Low — property consumed |
| **First/last baseline alignment** | CSS Inline §8 | Medium — only single baseline |

### Gap Analysis vs Blink

```
Blink supports:
- Full BiDi implementation (ICU)
- Character-level line breaking with language awareness
- Hyphenation dictionaries
- Full justification with inter-word/inter-character spacing
- Ruby annotation layout
- Initial letter drop-cap layout
- Leading trim
- Multiple baseline alignment modes

liquide-layout lacks:
- BiDi reordering
- Character-level word breaking
- Hyphenation
- Full justification spacing distribution
- Ruby layout
- Initial letter
```

**Inline Parity: ~50%**

---

## 5. Table (`table.rs`) — CSS Tables Level 3

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| Table structure (rows, cells) | ✅ Full | Collects rows and cells |
| `colspan` | ✅ Full | Column spanning with width distribution |
| `rowspan` | ✅ Full | Row spanning with height distribution |
| Occupancy grid | ✅ Full | Proper span tracking |
| Column width calculation | ✅ Full | Content-based with span distribution |
| Row height calculation | ✅ Full | Content-based with span distribution |
| `border-spacing` | ✅ Full | Inter-cell spacing |
| `border-collapse: collapse` | ✅ Partial | Spacing set to 0, borders not merged |
| `caption-side` (top/bottom) | ✅ Full | Caption positioning |
| `empty-cells` | ✅ Consumed | Property read |
| `table-layout: fixed/auto` | ✅ Consumed | Property read |
| Anonymous table rows | ✅ Full | Non-row children become single-cell rows |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Border collapse algorithm** | CSS Tables §5 | High — border conflict resolution not implemented |
| **`table-layout: fixed` optimization** | CSS Tables §17.5.2.1 | Medium — first-row width not used |
| **Column/colgroup handling** | CSS Tables §17.2 | Medium — `<col>` elements not processed |
| **Percentage width resolution** | CSS Tables §17.5 | Medium — percentage widths in cells |
| **Table layer painting** | CSS Tables §17.6 | Low — proper background layer order |
| **Fixed table height distribution** | CSS Tables §17.5.3 | Low — height distribution basic |

### Gap Analysis vs Blink

```
Blink supports:
- Full border collapse with conflict resolution (precedence rules)
- Fixed table layout optimization
- Column/colgroup width constraints
- Percentage width resolution relative to table
- Proper table layer painting order (cell > row > rowgroup > column > colgroup > table)

liquide-layout lacks:
- Border collapse conflict resolution
- Fixed table layout optimization
- Column element processing
- Percentage width in cells
```

**Table Parity: ~45%**

---

## 6. Positioned (`positioned.rs`) — CSS Positioned Layout

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| `position: absolute` | ✅ Full | Positioned relative to containing block |
| `position: fixed` | ✅ Full | Positioned relative to viewport |
| `position: sticky` | ✅ Full | Clamped within containing block |
| Top/right/bottom/left offsets | ✅ Full | Proper auto handling |
| Containing block resolution | ✅ Full | Uses passed `containing_rect` |
| Intrinsic sizing | ✅ Full | Content-based when dimensions unset |
| Both horizontal/vertical auto | ✅ Full | Stretch to fill |
| Children layout dispatch | ✅ Full | Flex/grid/block detection |
| Anchor positioning properties | ✅ Consumed | anchor_name, position_anchor, position_area read |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Anchor positioning (full)** | CSS Anchor Positioning | High — properties consumed, no registry |
| **Sticky scroll coordination** | CSS Position §2.5 | Medium — no scroll offset integration |
| **Z-index stacking context** | CSS Position §9 | Medium — handled in painter, not layout |
| **Transform containing block** | CSS Transforms §7 | Medium — transforms create containing block |
| **Filter containing block** | CSS Filter §9 | Low — filters create containing block |
| **will-change containing block** | CSS Will Change §3 | Low — will-change creates containing block |

### Gap Analysis vs Blink

```
Blink supports:
- Full anchor positioning with anchor registry
- Sticky scroll coordination with compositor
- Transform/filter/will-change containing block
- Proper inset resolution with logical properties

liquide-layout lacks:
- Anchor positioning implementation
- Scroll-coordinated sticky
- Transform-based containing block detection
```

**Positioned Parity: ~65%**

---

## 7. Float (`float.rs`) — CSS Float and Clear

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| `float: left/right` | ✅ Full | FloatContext placement |
| `float: inline-start/inline-end` | ✅ Full | Mapped to left/right |
| `clear: left/right/both` | ✅ Full | Clear Y calculation |
| Exclusion area tracking | ✅ Full | Available width queries |
| Stacking floats | ✅ Full | Left floats stack from left |
| Shape-outside properties | ✅ Consumed | shape_outside/margin/image_threshold read |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **CSS Shapes (shape-outside)** | CSS Shapes Level 1 | High — shape geometry not computed |
| **Float interleaving** | CSS 2.1 §9.5 | Medium — complex float stacking |
| **Clearance calculation** | CSS 2.1 §9.5.2 | Medium — simplified |
| **Line box wrapping around floats** | CSS 2.1 §9.5 | Medium — block-level only |
| **Float fragmentation** | CSS Fragmentation | Low — no column/page breaks |

### Gap Analysis vs Blink

```
Blink supports:
- Full CSS Shapes with circle, ellipse, polygon, path
- Inline content wrapping around floats
- Complex float interleaving
- Proper clearance margin calculation

liquide-layout lacks:
- Shape geometry calculation
- Inline-level float wrapping
- Complex interleaving scenarios
```

**Float Parity: ~55%**

---

## 8. Multi-column (`multicol.rs`) — CSS Multi-column Layout Level 1

### ✅ Implemented Features

| Feature | Status | Implementation Quality |
|---------|--------|----------------------|
| `column-count` | ✅ Full | Explicit column count |
| `column-width` | ✅ Full | Suggested width → column count derivation |
| `column-gap` | ✅ Full | Inter-column spacing |
| `column-rule` (width/style/color) | ✅ Full | Properties resolved |
| `column-span: all` | ✅ Full | Spanners break flow |
| `column-fill: balance/auto` | ✅ Full | Height distribution |
| `break-before/after: column` | ✅ Full | Forced column breaks |
| `break-inside: avoid` | ✅ Full | Avoids breaking elements |
| Orphans/widows | ✅ Consumed | Properties read |
| `box-decoration-break` | ✅ Consumed | Property read |

### ❌ Missing/Incomplete Features

| Feature | Spec Reference | Impact |
|---------|---------------|--------|
| **Orphans/widows enforcement** | CSS Fragmentation §4 | Medium — properties read but not enforced |
| **Box decoration break** | CSS Fragmentation §4 | Medium — slicing/cloning not implemented |
| **Spanning calculation** | CSS Multicol §7 | Medium — height balancing simplified |
| **Overflow columns** | CSS Multicol §6 | Low — content may overflow last column |
| **Nested multicol** | CSS Multicol §8 | Low — not tested |

### Gap Analysis vs Blink

```
Blink supports:
- Full orphans/widows enforcement
- Box decoration break (slice/clone)
- Proper overflow column handling
- Nested multi-column contexts

liquide-layout lacks:
- Orphans/widows enforcement
- Box decoration break implementation
- Complex overflow handling
```

**Multi-column Parity: ~40%**

---

## Summary of Critical Missing Features

### High Priority (Common Use Cases)

1. **Auto margins in flexbox** — Space consumption before justify-content
2. **BiDi algorithm** — RTL/LTR text reordering
3. **`repeat(auto-fill/auto-fit)`** — Responsive grid tracks
4. **Parent-child margin collapsing** — Common layout pattern
5. **CSS Shapes** — float wrapping with custom shapes
6. **Border collapse algorithm** — Table border conflict resolution
7. **Anchor positioning** — New CSS feature, properties already parsed
8. **Character-level line breaking** — `overflow-wrap: break-word` character split

### Medium Priority (Feature Completeness)

1. **Intrinsic sizing algorithm** — min-content/max-content/fit-content
2. **Named grid lines** — `[line-name]` syntax
3. **Writing modes** — vertical-rl, vertical-lr, sideways-*
4. **Hyphenation** — `hyphens: auto` with dictionary
5. **Full justification** — Inter-word/character spacing distribution
6. **Sticky scroll coordination** — Compositor integration
7. **Subgrid (full)** — CSS Grid Level 2

### Low Priority (Edge Cases)

1. **Safe alignment** — Overflow-safe keyword
2. **Ruby annotation** — CJK ruby layout
3. **Initial letter** — Drop cap layout
4. **Counter state machine** — Ordered list numbering scope
5. **Masonry layout** — Experimental CSS Grid Level 3

---

## Recommendations

1. **Immediate wins:**
   - Implement auto margins in flexbox (high impact, moderate effort)
   - Add parent-child margin collapsing (high impact, moderate effort)
   - Implement `repeat(auto-fill/auto-fit)` for grid (high demand)

2. **Strategic improvements:**
   - Integrate ICU for BiDi support
   - Implement intrinsic sizing algorithm for better content-based layouts
   - Add anchor positioning (modern CSS feature)

3. **Testing focus:**
   - Create WPT-style test suite targeting gap areas
   - Run against Blink's layout tests for regression detection

---

*Document generated: 2026-02-17*
*Analyzed crate: liquide-layout v0.1.0*
