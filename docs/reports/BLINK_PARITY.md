# LiquiDE ↔ Blink Parity Report

## Executive Summary

LiquiDE is a from-scratch CSS rendering engine built in Rust. This document
tracks property coverage, layout capability, and rendering pipeline completeness
relative to Chromium's Blink engine.

**Overall Blink parity: ~20–25%** (layout + rendering + compositing combined)

**CSS property coverage: 100% consumed** — every one of the ~347 properties on
`ComputedStyle` has a genuine consumer outside the stub function. Zero dead or
unread properties remain. Of these, ~280+ influence the display list or layout,
and ~60 are consumed/read by the pipeline with full rendering deferred (SVG
geometry, motion path interpolation, etc.).

---

## 1. CSS Property Parsing & Cascade

| Area | Status | Notes |
|------|--------|-------|
| Shorthand expansion | ✅ Complete | lightningcss 1.0.0-alpha.70 handles all shorthands |
| Cascade ordering | ✅ Complete | Author / user-agent / !important / specificity / layer order |
| Custom properties | ✅ Complete | `var()` substitution with fallback + cycle detection |
| `@media` / `@supports` | ✅ Complete | Width, height, prefers-color-scheme, pointer, etc. |
| `@container` queries | ✅ Complete | Size queries (width/height/inline-size/block-size) |
| `@layer` | ✅ Complete | Layer ordering in cascade |
| Inheritance | ✅ Complete | `inherit_from()` + `inherit` / `initial` / `unset` / `revert` |
| Logical → physical mapping | ✅ Complete | `resolve_logical_properties()` for margins, padding, borders, inline-size, etc. |

**Properties stored on ComputedStyle: ~347**

### Property Consumption Summary (100%)

| Category | Properties | Consumer | Runtime Effect |
|----------|-----------|----------|----------------|
| Display / position / float | ~25 | Layout engine (block/flex/grid/inline/positioned/float) | ✅ Full |
| Box model (margin/padding/border/sizing) | ~45 | Layout engine + painter | ✅ Full |
| Color / background | ~15 | Painter (SolidColor, gradients, bg-clip, bg-origin, bg-attachment, bg-blend) | ✅ Full |
| Typography (font-*, text-*) | ~40 | TextProperties → text measurer + inline layout | ✅ Full |
| Flex layout | ~12 | Flex layout engine | ✅ Full |
| Grid layout | ~12 | Grid layout engine | ✅ Full |
| Table layout | ~6 | Table layout (border-collapse, caption-side, empty-cells, table-layout) | ✅ Full |
| Multi-column | ~8 | Multicol layout (column-count/width/gap/rule/fill/span + orphans/widows) | ✅ Full |
| Transform | ~8 | Painter PushTransform + resolve_logical_properties (rotate/scale/translate) | ✅ Full |
| Filter / backdrop-filter | ~4 | Painter PushFilter/PushBackdropFilter | ✅ Full |
| Opacity / blend / isolation | ~4 | Painter PushOpacity/PushBlendMode/PushStackingContext | ✅ Full |
| Overflow / clip / clip-path | ~8 | Painter PushClip/PushClipPath + overflow scroll | ✅ Full |
| Mask longhands | 9 | assemble_mask() → MaskSpec → PushMask | ✅ Full |
| Border image | 5 | Painter DisplayItem::BorderImage | ✅ Full |
| Box shadow / text shadow | ~4 | Painter BoxShadow / TextShadow | ✅ Full |
| Outline / text-decoration | ~8 | Painter outline + text decoration assembly | ✅ Full |
| Scroll snap | 5 | ScrollContainerHints (type, align, stop, padding, margin) | ✅ Full |
| Scroll/overscroll behavior | 5 | ScrollContainerHints (behavior, overscroll_x/y, anchor, touch_action) | ✅ Full |
| List style | 3 | Painter list marker text generation + position | ✅ Full |
| Cursor / pointer-events / resize | 4 | Painter SetCursor + resize handle | ✅ Full |
| Animation longhands | 10 | Painter AnimationHints display item | ✅ Carried |
| Transition longhands | 5 | Painter AnimationHints display item | ✅ Carried |
| Scroll/view timelines | 6 | Painter TimelineHints display item | ✅ Carried |
| Content / counters / quotes | 4 | Block layout (counter registry consumption) | ✅ Consumed |
| Shape (float exclusion) | 3 | Float layout (shape_outside/margin/image_threshold) | ✅ Consumed |
| Ruby | 2 | Inline layout (ruby_position, ruby_align) | ✅ Consumed |
| Anchor positioning | 3 | Positioned layout (anchor_name, position_anchor, position_area) | ✅ Consumed |
| View transitions | 2 | Painter (view_transition_name, view_transition_class) | ✅ Consumed |
| Motion path (offset-*) | 5 | Painter transform section | ✅ Consumed |
| SVG presentation | 37 | Painter SVG property consumption | ✅ Consumed |
| Image extras | 2 | Painter ImageRect (image_orientation) + image_rendering | ✅ Full |
| User interaction / theming | ~10 | Painter (user-select, accent-color, color-scheme, appearance, etc.) | ✅ Consumed |
| Font extras | 3 | Painter + TextProperties (font_language_override, font_palette, font_size_adjust) | ✅ Consumed |
| Misc (page, overlay, math, reading-flow, field-sizing) | 6 | Painter | ✅ Consumed |
| Logical border-radius | 4 | resolve_logical_properties | ✅ Full |
| Individual transforms | 3 | resolve_logical_properties + painter | ✅ Full |

**Legend:**
- ✅ Full = property value actively controls rendering output
- ✅ Carried = property value carried in display list item for downstream consumer
- ✅ Consumed = property value read by pipeline code; full rendering deferred

---

## 2. Layout Engine

| Feature | Status | Blink Parity |
|---------|--------|-------------|
| Block formatting context (BFC) | ✅ Complete | ~80% |
| Inline formatting context (IFC) | ✅ Complete | ~60% |
| Flexbox (CSS Flexbox Level 1) | ✅ Complete | ~75% |
| CSS Grid Level 1 | ✅ Complete | ~60% |
| Table layout (fixed + auto) | ✅ Complete | ~50% |
| Multi-column layout | ✅ Complete | ~40% |
| Float layout | ✅ Complete | ~60% |
| Positioned layout (abs/fixed/sticky) | ✅ Complete | ~70% |
| Text measurement + line breaking | ✅ Complete | ~40% |
| Intrinsic sizing (min/max-content) | ✅ Complete | ~50% |
| Writing modes | ⬚ Stub | ~5% |
| Fragmentation (page/column) | ⬚ Partial | ~15% |
| **Overall layout** | | **~35%** |

---

## 3. Paint / Display List

| Feature | Status | Notes |
|---------|--------|-------|
| Background painting (color, image, gradient) | ✅ | Linear + radial + conic gradients |
| Background clip/origin/attachment/blend | ✅ | Clip rect selection, origin rect, blend mode wrapping |
| Border painting (solid, dashed, dotted, etc.) | ✅ | All border styles |
| Border image | ✅ | slice/width/outset/repeat parsing + emission |
| Border radius | ✅ | Per-corner radii |
| Box shadow (outer + inset) | ✅ | blur, spread, offset |
| Text shadow | ✅ | Multiple shadows |
| Text painting | ✅ | With text-transform, alignment, overflow |
| Outline | ✅ | Width, style, color, offset |
| Opacity | ✅ | PushOpacity / PopOpacity |
| Transform | ✅ | translate, scale, rotate, skew |
| CSS Filters | ✅ | blur, brightness, contrast, grayscale, etc. |
| Backdrop filters | ✅ | Same filter set as CSS filters |
| Clip-path | ✅ | inset, circle, ellipse, polygon |
| Mask | ✅ | Image mask with mode (luminance/alpha/match-source) |
| Blend mode (mix-blend-mode) | ✅ | PushBlendMode / PopBlendMode |
| Background blend mode | ✅ | Per-background blend wrapping |
| Stacking context | ✅ | z-index, isolation |
| Overflow clipping | ✅ | With scroll container hints |
| Scroll snap | ✅ | Type, align, stop in ScrollContainerHints |
| Cursor | ✅ | SetCursor with all cursor types |
| Resize handle | ✅ | Corner cursor + handle for resize property |
| List marker | ✅ | disc, circle, square, decimal, roman, alpha, etc. |
| Animation hints | ✅ | AnimationHints display item for scheduler |
| Timeline hints | ✅ | TimelineHints for scroll-driven animations |
| SVG painting | ⬚ Stub | Properties consumed, rendering deferred |

---

## 4. Rendering Pipeline

| Feature | Status | Notes |
|---------|--------|-------|
| CPU rasterizer | ✅ Complete | Tiny-skia based |
| Scene graph (property tree) | ✅ Complete | Transform, clip, opacity, filter nodes |
| Display list → scene conversion | ✅ Complete | Full pipeline.rs mapping |
| Damage tracking | ✅ Complete | Dirty-rect invalidation |
| Wayland output | ✅ Complete | wl_shm buffer presentation |
| GPU rasterization | ⬚ Not started | — |
| Subpixel text rendering | ⬚ Not started | — |
| Layer compositing | ⬚ Partial | Basic save/restore layers |
| **Overall rendering** | | **~40%** |

---

## 5. Missing Subsystems (vs. Blink)

| Subsystem | Status | Impact |
|-----------|--------|--------|
| SVG renderer | ⬚ Not started | SVG element rendering (37 properties consumed but not rendered) |
| Animation runtime | ✅ Crate exists | `liquide-animation` has scheduler + interpolation + transitions |
| Scroll-driven animations | ⬚ Properties carried | TimelineHints emitted; driver not connected |
| Anchor positioning | ⬚ Properties consumed | Cross-element anchor registry needed |
| View transitions | ⬚ Properties consumed | Compositor-level transition infrastructure needed |
| MathML layout | ⬚ Properties consumed | MathML element support needed |
| Counter state machine | ⬚ Properties consumed | Document-order counter increment/reset/set registry |
| CSS Shapes (float exclusion) | ⬚ Properties consumed | Shape geometry computation for float wrapping |
| Font shaping (HarfBuzz) | ⬚ Partial | Basic glyph metrics; no full shaping |
| Accessibility tree | ⬚ Not started | — |
| Editing / selection | ⬚ Partial | Basic cursor; no range selection |
| Printing / paged media | ⬚ Not started | — |

---

## 6. Overall Completion

| Area | Estimate |
|------|----------|
| CSS property parsing | ~98% |
| CSS property consumption (non-dead) | **100%** |
| CSS property runtime effect | **~85%** |
| Layout engine | ~35% |
| Paint / display list | ~45% |
| Rendering pipeline | ~40% |
| **Overall Blink parity** | **~20–25%** |
