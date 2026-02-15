# Chromium vs LiquiDE — Rendering Engine Comparison

> Generated comparison of Google Chrome's Blink/cc rendering pipeline against LiquiDE's
> custom rendering engine. Numbers reflect the current state of both codebases.

---

## Executive Summary

| Dimension | Chromium (Blink + cc) | LiquiDE |
|---|---|---|
| **Language** | C++ (~8M lines in renderer) | Rust (~70 crates) |
| **CSS properties** | ~580 longhands + ~230 shorthands | 118 property handlers + 28 shorthands |
| **ComputedStyle fields** | ~500+ longhands | 111 fields, 53 enum types |
| **Layout algorithms** | Block, Flex, Grid, Table, Inline, Multicolumn, MathML, Custom Layout API | Block, Flex, Grid (basic), Inline (basic), Positioned |
| **Property trees** | 4 (Transform, Clip, Effect, Scroll) | 4 (Transform, Clip, Effect, Scroll) |
| **Rendering backend** | Skia (GPU via Ganesh/Graphite, software fallback) | Custom CPU rasterizer (23 modules, 3100+ lines) |
| **Threading model** | Multi-process, multi-thread (Main + Compositor + Raster workers + GPU) | Single-thread, single-process |
| **Display items** | ~40 display item types + PaintChunks | 38 DisplayItem variants |
| **Compositing** | Tiled layers, GPU compositing, hardware overlays | Scene graph (31 SceneNodeKind), damage tracking |

**LiquiDE implements roughly 15-20% of Chromium's CSS surface area but mirrors its architectural patterns (property trees, display lists, cascade) at a structural level.** The main gaps are in CSS property coverage, layout algorithm completeness, GPU acceleration, and threading.

---

## 1. CSS Parsing

### Chromium (Blink)
- **Parser**: `CSSParserImpl` — hand-written recursive descent parser
- **Property definitions**: `css_properties.json5` code-generates ~580 longhand + ~230 shorthand property classes
- **Class hierarchy**: `CSSUnresolvedProperty` → `CSSProperty` → `Longhand`/`Shorthand`; each property is its own class with `ParseSingleValue()`, `CSSValueFromComputedStyle()`, etc.
- **At-rules**: Full support for `@media`, `@keyframes`, `@font-face`, `@supports`, `@layer`, `@container`, `@scope`, `@property`, `@counter-style`, `@font-feature-values`, `@page`, `@namespace`, `@import`
- **Custom properties**: Full `var()` resolution with cycle detection and fallback values
- **Selector parsing**: Full CSS Selectors Level 4 including `:is()`, `:where()`, `:has()`, `:not()`, `:nth-child(An+B of S)`, `::part()`, `::slotted()`

### LiquiDE
- **Parser**: `lightningcss` Rust crate (Mozilla-derived, spec-compliant)
- **Property handling**: 118 match arms in `apply_single_property`, converting parsed CSS values to computed values
- **Shorthands**: 28 expansion handlers in `shorthand.rs`
- **At-rules**: `@media`, `@keyframes`, `@font-face`, `@supports` — no `@layer`, `@container`, `@scope`, `@property`
- **Custom properties**: Partial (`var()` via lightningcss, no custom `@property` registration)
- **Selectors**: 15 pseudo-classes, 4 combinators (descendant, child, next-sibling, subsequent-sibling)

### Gap Analysis
| Feature | Chromium | LiquiDE | Gap |
|---|---|---|---|
| Longhand properties | ~580 | 118 | **~462 missing** |
| Shorthand properties | ~230 | 28 | **~202 missing** |
| `@layer` cascade layers | Yes | No | **Missing** |
| `@container` queries | Yes | No | **Missing** |
| `@scope` | Yes | No | **Missing** |
| `@property` (registered custom props) | Yes | No | **Missing** |
| `:has()` selector | Yes | No | **Missing** |
| `:is()` / `:where()` | Yes | No | **Missing** |
| `::part()` / `::slotted()` | Yes | No | **Missing** |
| Nesting (`&`) | Yes | No | **Missing** |

---

## 2. Style Resolution & Cascade

### Chromium (Blink)
- **StyleResolver**: Multi-pass cascade with origin sorting (UA → user → author), `!important` reversal, `@layer` ordering, scope proximity
- **Cascade layers** (`@layer`): Full implementation — layers create explicit ordering within each origin
- **Style sharing**: Bloom filter optimization to share `ComputedStyle` between siblings with identical applicable rules
- **Style invalidation**: `InvalidationSet` system — tracks which selectors are affected by DOM mutations, avoids full restyle
- **Computed values**: `ComputedStyle` with ~500+ longhand fields, `ComputedStyleBase` code-generated from `css_properties.json5`
- **Inheritance**: Fine-grained per-property inheritance flags; inherited properties stored in shared `StyleInheritedData` groups
- **Style recalc scoping**: Dirty bits (`NeedsStyleRecalc`, `ChildNeedsStyleRecalc`) scope recalculation to changed subtrees

### LiquiDE
- **CascadeMap**: 6 origin levels (UserAgentNormal, UserNormal, AuthorNormal, AuthorImportant, UserImportant, UserAgentImportant)
- **Specificity**: Standard (a, b, c) calculation with correct `!important` handling
- **Shorthand expansion**: 28 shorthands expanded to longhands before cascade insertion
- **Computed values**: `ComputedStyle` with 111 fields, 53 enum types; `with_inherited()` factory for inheritance
- **Dirty tracking**: `mark_dirty()` / `clear_dirty()` on DOM nodes; pipeline restyles all dirty nodes
- **No style sharing**: Each element gets its own full restyle pass
- **No invalidation sets**: Changes trigger full subtree restyle

### Gap Analysis
| Feature | Chromium | LiquiDE | Gap |
|---|---|---|---|
| Cascade layers (`@layer`) | Yes | No | **Missing** |
| Style sharing / Bloom filter | Yes | No | **Performance gap** |
| Invalidation sets | Yes | No | **Performance gap** — full restyle instead of targeted |
| Container queries (`@container`) | Yes | No | **Missing** |
| Scope proximity (`@scope`) | Yes | No | **Missing** |
| Anchor positioning | Yes | No | **Missing** |
| `revert` / `revert-layer` | Yes | No | **Missing** |
| `env()` / `calc()` in computed values | Full | Partial (via lightningcss) | **Partial** |
| Logical properties (full) | Yes | Partial | ~50% coverage |

---

## 3. Layout

### Chromium (Blink) — LayoutNG
- **Architecture**: LayoutNG — immutable constraint space in, immutable fragment tree out
- **Input**: `BlockNode` + `ConstraintSpace` → `PhysicalFragment`
- **Fragment tree**: Immutable output fragments replace mutable LayoutObject geometry; enables caching
- **Fragment caching**: `CachedLayoutResult` — reuses fragments when constraint space matches
- **Layout modes**:
  - **Block flow**: Full CSS 2.1 block formatting contexts, margin collapsing (§8.3.1 including parent-child, empty blocks), clearance, floats
  - **Flex**: Full CSS Flexbox Level 1 including multi-line wrap, `order`, `align-content`, `align-items`, `align-self`, `justify-content`, min/max intrinsic sizing, percentage resolution
  - **Grid**: Full CSS Grid Level 1 + Level 2 (subgrid), line naming, `auto-fill`/`auto-fit`, spanning, implicit grid, areas, alignment
  - **Table**: Full CSS 2.1 table layout algorithm, `table-layout: fixed`, auto table layout, caption, column groups
  - **Inline**: Full ICU-based line breaking, BiDi reordering (UBA), shaping (HarfBuzz), `text-align`, `vertical-align`, `word-spacing`, `letter-spacing`, first-line handling
  - **Multicolumn**: `column-count`, `column-width`, `column-gap`, column spanning, fragmentation
  - **MathML**: MathML Core layout algorithm
  - **Custom Layout API**: CSS Houdini `registerLayout()`
  - **Block fragmentation**: Pagination, printing, multicol fragmentation with break tokens

- **Float positioning**: Full float placement with clearance, float avoidance, exclusion areas
- **Out-of-flow**: `position: absolute`, `position: fixed`, `position: sticky` (with scroll container awareness), anchor positioning
- **Intrinsic sizing**: `min-content`, `max-content`, `fit-content`, `stretch`
- **Writing modes**: Full `writing-mode` + `direction` + `text-orientation` with logical coordinates

### LiquiDE
- **Architecture**: Mutable layout tree — `LayoutEngine::layout()` produces `LayoutTree` with `LayoutBox` nodes
- **Block flow**: Basic block layout with simplified adjacent-sibling margin collapsing; no floats, no clearance, no parent-child collapse
- **Flex**: Full multi-line wrap, `flex-grow`/`flex-shrink` with min/max clamping, `align-content` (6 modes), `justify-content`, `order`, cross-axis gaps
- **Grid**: Simplified — resolves explicit tracks, places children by grid-line, no `auto-fill`/`auto-fit`, no spanning, no subgrid
- **Inline**: Simplified — basic text measurement and wrapping, no BiDi, no complex shaping
- **Positioned**: `absolute` and `fixed` positioning with inset resolution; no `sticky`
- **No table layout**
- **No multicolumn layout**
- **No fragmentation / pagination**
- **No writing mode support** (horizontal-tb only)
- **No intrinsic sizing keywords**

### Gap Analysis
| Feature | Chromium | LiquiDE | Status |
|---|---|---|---|
| Block — floats / clearance | Full | None | **Missing** |
| Block — full margin collapsing | Full (parent-child, empty, through) | Adjacent-sibling only | **Partial** |
| Flex — intrinsic sizing | Full | None | **Missing** |
| Grid — `auto-fill` / `auto-fit` | Yes | No | **Missing** |
| Grid — spanning / subgrid | Yes | No | **Missing** |
| Grid — implicit grid | Yes | No | **Missing** |
| Table layout | Full | None | **Missing** |
| Inline — BiDi / HarfBuzz shaping | Full | None | **Missing** |
| Inline — line breaking (ICU) | Full | Basic word-wrap | **Partial** |
| Multicolumn | Full | None | **Missing** |
| Fragmentation / pagination | Full | None | **Missing** |
| `position: sticky` | Yes | No | **Missing** |
| Anchor positioning | Yes | No | **Missing** |
| Writing modes / `direction` | Full | horizontal-tb only | **Missing** |
| Fragment caching | Yes | No | **Performance gap** |
| Constraint space model | Immutable in/out | Mutable tree | **Architectural difference** |

---

## 4. Paint

### Chromium (Blink)
- **PrePaint phase**:
  - **Paint invalidation**: `PaintInvalidator` traverses layout tree, marks dirty display items using NeedsPaintPropertyUpdate / SubtreeNeedsPaintPropertyUpdate dirty bits
  - **Property tree building**: `PaintPropertyTreeBuilder` builds Transform, Clip, Effect trees with isolation boundary optimization (`contain: paint`)
  - Uses `FragmentData` + `ObjectPaintProperties` per layout object
- **Paint phase**:
  - Walks `PhysicalFragment` tree in paint-order using static painter classes (`BoxFragmentPainter`, `TextFragmentPainter`)
  - Single `PaintController` per `LocalFrameView`
  - Segments display items into `PaintChunk`s by shared property tree state
  - **Display item caching**: Reuses identical `DrawingDisplayItem`s from previous paint
  - **Subsequence caching**: Caches entire `PaintLayer` subsequences
  - **Empty paint phase optimization**: Skips phases (outlines, floats) when `NeedsPaintPhaseXXX` not set
  - **Property tree update optimization**: Fast-path for transform/opacity-only changes (no tree walk needed)
  - **Hit test recording**: Paint-order hit test info (touch action rects, wheel event rects, scroll hit test, hit-test opaqueness)
- **Compositing**:
  - `PaintArtifactCompositor::Update()` — layerizes PaintChunks into `cc::Layer` list
  - Converts Blink property tree nodes → cc property tree nodes
  - `PaintChunksToCcLayer::Convert()` — non-composited nodes become meta display items

### LiquiDE
- **Paint phase**:
  - `Painter::paint()` walks layout tree, emits `DisplayList` (sequential display items)
  - 38 DisplayItem variants: 16 draw ops + 19 state ops (Push/Pop) + 3 other
  - Emits Push/Pop pairs for Transform, Clip, ClipPath, Opacity, BlendMode, Filter, BackdropFilter, Mask, StackingContext
  - Dedicated `Outline` item, `BoxShadow` item, `BorderImage` item
  - Spatial indexing on display list
- **No paint invalidation** — full repaint each frame
- **No display item caching** — all items regenerated
- **No subsequence caching**
- **No PaintChunk concept** — linear display list without property-tree-state grouping
- **No hit-test recording in paint** — hit testing done separately

### Gap Analysis
| Feature | Chromium | LiquiDE | Gap |
|---|---|---|---|
| Paint invalidation | Incremental (dirty bits, subtree scoping) | Full repaint | **Performance gap** |
| Display item caching | Yes | No | **Performance gap** |
| Subsequence caching | Yes | No | **Performance gap** |
| PaintChunk grouping | Yes | No (linear list) | **Architectural difference** |
| Property tree building in PrePaint | Yes | Done in pipeline bridge | **Different location, same concept** |
| Hit-test-in-paint | Yes | No | **Missing** |
| Empty phase optimization | Yes | No | **Missing** |
| Fast-path transform/opacity update | Yes | No | **Missing** |
| Display item count | ~40 | 38 | **Comparable** |
| Scrollbar painting | Native + composited | None | **Missing** |

---

## 5. Compositing & Property Trees

### Chromium (cc/)
- **Architecture**: `cc/` is a standalone compositor ("content collator") — takes painted input from Blink, rasterizes, animates, and produces `CompositorFrame`s
- **Threading**: Main thread (Blink) → Compositor thread (cc) → Raster worker threads → GPU process
  - **Commit**: Atomic snapshot from main thread Layer tree to pending tree (blocks main thread)
  - **Activation**: Pending tree → Active tree (when raster complete)
  - **Draw**: Active tree produces compositor frames
- **Trees**: 4 concurrent trees — Main thread tree, Pending tree, Active tree, Recycle tree
- **Property trees**: Transform, Clip, Effect, Scroll — same 4 tree types
  - Driven by `PropertyTreeBuilder` or directly set by Blink (Slimming Paint)
  - `ElementId` for stable cross-thread animation targeting
- **Layers**: `cc::Layer` (main thread) / `cc::LayerImpl` (compositor thread)
  - Types: PictureLayer, TextureLayer, SolidColorLayer, SurfaceLayer, ScrollbarLayer
  - Each has `AppendQuads()` to produce draw quads
- **Tiling**: `PictureLayerTiling` — sparse 2D grid of `cc::Tile` at different scales
  - `TileManager` schedules rasterization across worker threads
  - Software tiles ~256×256px, GPU tiles ~viewport/4
  - Tile priority based on distance to viewport
- **Animation**: Compositor-driven keyframe animations of transform/opacity/filter directly on property tree nodes — runs without main thread
- **Scheduling**: `cc::Scheduler` + `SchedulerStateMachine` — manages BeginImplFrame → BeginMainFrame → Commit → Activate → Draw pipeline
  - High-latency vs low-latency mode
  - Can draw without waiting for slow main thread commits
- **Output**: `CompositorFrame` → `DrawQuad` list + `RenderPass` list → `SurfaceAggregator` → `DirectRenderer` (GL/Skia/Software)
- **Damage tracking**: `DamageTracker` — invalidation damage + expose damage; enables partial swap

### LiquiDE
- **Scene graph**: 31 `SceneNodeKind` variants — domain-specific nodes (Glass, BlurBackdrop, Tint, Workspace, LockScreen, etc.)
- **Property trees**: 4 trees matching Chromium — Transform, Clip, Effect, Scroll
  - `TransformNode` with parent chaining, to_root cache, will_change, sorting_context
  - `ClipNode` with accumulated_clip cache
  - `EffectNode` with opacity, blend_mode, filters, backdrop_filters, render_surface_reason
  - `ScrollNode` with overscroll_behavior
  - `PropertyTrees` struct holds all 4 + node pools + caches
- **Pipeline bridge**: `build_property_trees()` walks display list Push/Pop structure to populate trees
- **Damage tracking**: `DegradationController` + damage tracking in compositor
- **Single-threaded**: No compositor thread, no pending/active tree split, no tiling
- **No animation system**: No compositor-driven animations
- **No scheduling**: Render-on-demand (dirty flag driven)
- **No layer concept**: Direct scene graph, no tiled PictureLayers

### Gap Analysis
| Feature | Chromium | LiquiDE | Gap |
|---|---|---|---|
| Threading model | 4+ threads (Main, Compositor, Raster×N, GPU) | Single thread | **Major architectural difference** |
| Pending/Active tree | Yes (atomic frame staging) | No | **Missing** |
| Tiled rasterization | Yes (sparse 2D tiles, multi-scale) | No | **Missing** |
| Compositor-driven animation | Transform, opacity, filter on compositor thread | No | **Missing** |
| Scheduler | `cc::Scheduler` with latency modes | Render-on-demand | **Different model** |
| Layerization | PaintChunks → cc::Layers → Quads | DisplayList → SceneNodes | **Different model** |
| SurfaceAggregator | Cross-process frame aggregation | N/A (single process) | **Not needed** |
| Property trees | 4 types | 4 types (identical) | **Match** |
| Damage tracking | DamageTracker (invalidation + expose) | DegradationController | **Partial match** |
| GPU compositing | Hardware overlays, render passes | CPU-only | **Missing** |
| Domain-specific nodes | Generic layers | 31 custom scene kinds (Glass, Blur, Tint...) | **LiquiDE advantage** |

---

## 6. Rendering / Rasterization

### Chromium
- **Engine**: Skia (Google's 2D graphics library)
  - **GPU backends**: Ganesh (OpenGL/Vulkan/Metal/Direct3D), Graphite (next-gen)
  - **Software fallback**: `SkBitmap`-based software rasterizer
- **Raster modes**:
  - **Software raster**: PaintRecord → software bitmap via raster workers
  - **GPU raster**: PaintRecord → paint ops sent over command buffer → GPU process draws via Skia GPU backend
- **Image pipeline**: `SoftwareImageDecodeCache` / `GpuImageDecodeCache` — decode, scale, color-correct, upload
- **Buffer providers**: `ZeroCopyRasterBufferProvider`, `OneCopyRasterBufferProvider`, `GpuRasterBufferProvider`
- **Text**: HarfBuzz shaping + FreeType rendering + subpixel positioning
- **Paint ops**: `PaintRecord` (`PaintOpBuffer`) contains `PaintOp`s — mutable, introspectable, serializable equivalent of `SkPicture`
- **OOP rasterization**: Out-of-process raster (security boundary — GPU process rasters, renderer just records)
- **Display compositing**: `viz::DirectRenderer` with GL, Skia, or Software backends
- **Color management**: Full ICC profile support, wide gamut, HDR

### LiquiDE
- **Engine**: Custom CPU software rasterizer — 23 modules, 3100+ lines
  - Modules: `rasterizer`, `blend`, `blit`, `blur`, `blur_worker`, `color`, `dirty_rects`, `effects`, `filter`, `font_worker`, `glyph`, `icons`, `image_decode`, `layout_cache`, `lod`, `nine_patch`, `object_pool`, `path`, `pattern`, `text_layout`, `texture_cache`, `bitmap_font`
- **Features**:
  - Blend modes (standard CSS blend modes)
  - Box blur with adaptive quality (LOD-based degradation)
  - Glyph atlas / texture caching
  - Nine-patch border image support
  - Path rasterization
  - Pattern fill
  - Object pool for buffer reuse
  - Dirty rect optimization
- **No GPU acceleration**
- **No Skia dependency** — fully custom implementation
- **Text**: Custom bitmap font + glyph atlas (no HarfBuzz, no FreeType)
- **No OOP raster** — single process

### Gap Analysis
| Feature | Chromium | LiquiDE | Gap |
|---|---|---|---|
| GPU rasterization | Yes (Ganesh/Graphite) | No | **Major gap** |
| Skia integration | Core dependency | None (custom) | **Architectural choice** |
| Multi-threaded raster | Yes (worker pool) | No | **Performance gap** |
| Subpixel text rendering | Yes (FreeType + HarfBuzz) | No (bitmap font) | **Quality gap** |
| Image decode pipeline | Full (async, GPU upload, cache) | Basic (image_decode module) | **Partial** |
| Color management | Full ICC/HDR/wide gamut | Basic sRGB | **Missing** |
| OOP rasterization | Yes (security boundary) | N/A | **Not needed (single process)** |
| Blur implementation | Gaussian (Skia) | Box blur with adaptive LOD | **Different approach** |
| Texture caching | GPU texture management | CPU texture_cache + object_pool | **CPU-only equivalent** |
| Dirty rect optimization | Via damage tracking + tiling | dirty_rects module | **Present** |
| Custom modules | N/A (Skia handles all) | 23 specialized modules | **LiquiDE has fine control** |

---

## 7. Pipeline & Threading Architecture

### Chromium
```
Main Thread (Blink):
  DOM → Style → Layout → PrePaint → Paint → Commit ──────┐
                                                          │ (blocked)
Compositor Thread (cc):                                   ▼
  BeginImplFrame → Commit → Pending Tree → [Raster] → Activate → Active Tree → Draw
                                              │
Raster Threads (N workers):                   ▼
  TileManager → TaskGraph → PaintRecord → Bitmap/Texture
                                              │
GPU Process:                                  ▼
  Command Buffer → Skia GPU → Display → Swap
```

- **4 thread types** with message passing and blocking commits
- **Scheduler-driven**: `cc::Scheduler` decides when to commit, activate, draw
- **Pipelining**: Main thread can start next frame while compositor rasters previous
- **Latency management**: High/low latency modes, deadline-based frame dropping
- **Process isolation**: Renderer process sandboxed, GPU process separate

### LiquiDE
```
Single Thread:
  DOM (dirty check) → Style → Layout → Paint → Bridge → Scene → Render
         │              │        │        │        │        │       │
    Document       StyleEngine  Layout  Painter  Pipeline  Comp  CPU Rasterizer
    (788 lines)    (990 lines)  Engine  (440 ln)  Bridge  Scene    (3100+ lines)
                                                          Graph
```

- **Single thread** — entire pipeline runs synchronously
- **Render-on-demand** — dirty flag triggers full pipeline run
- **No pipelining** — each frame must complete before next starts
- **No process isolation** — compositor and renderer in same process
- **Advantage**: Zero latency between stages, no commit overhead, simpler debugging

### Gap Analysis
| Aspect | Chromium | LiquiDE | Notes |
|---|---|---|---|
| Parallelism | 4+ threads | 1 thread | Chromium can overlap stages |
| Frame pipelining | Yes | No | Chromium starts frame N+1 while N rasters |
| Input latency | Compositor handles scroll/pinch without main thread | Main thread handles all | Chromium more responsive during load |
| Complexity | High (thread safety, commit protocol, scheduling) | Low (sequential) | LiquiDE simpler to reason about |
| Debugging | Complex (cross-thread state) | Straightforward | LiquiDE advantage |

---

## 8. DOM

### Chromium (Blink)
- Full HTML5 DOM implementation with hundreds of element types
- Shadow DOM v1, Custom Elements v2, `<template>`, `<slot>`
- `MutationObserver`, `IntersectionObserver`, `ResizeObserver`, `PerformanceObserver`
- Range, Selection, TreeWalker, NodeIterator
- Full event model with capture/bubble/passive

### LiquiDE
- Custom lightweight DOM — 788-line `Document` with `NodeData` (5 variants: Element, Text, Image, Surface, ShadowRoot)
- 16 pseudo-state flags, dirty tracking
- Mutation observers (basic)
- No HTML parsing — DOM built programmatically
- No Shadow DOM, no Custom Elements
- Designed as a shell/desktop compositor interface, not a web browser

---

## 9. What LiquiDE Has That Chromium Doesn't

LiquiDE is purpose-built as a **desktop shell compositor** rather than a web browser engine, giving it unique capabilities:

| Feature | Description |
|---|---|
| **Glass/Blur effects** | Native `Glass`, `BlurBackdrop`, `Tint` scene node kinds for desktop compositor effects |
| **Desktop shell integration** | `Workspace`, `LockScreen`, `CrashScreen`, `Cursor` scene nodes |
| **DegradationController** | Adaptive quality reduction under load (LOD-based blur, frame skipping) |
| **Domain-specific scene graph** | 31 semantic node types vs Chromium's generic layer abstraction |
| **Custom shell extensions** | CSS-like properties for `shell-blur-radius`, `shell-tint-color`, `shell-surface-*` |
| **Lightweight footprint** | ~70 crates vs Chromium's millions of lines; compiles in seconds |
| **No web compat baggage** | No need for quirks mode, legacy CSS hacks, backwards compatibility |

---

## 10. Priority Recommendations

### High Priority (Correctness)
1. **Float layout** — Required for any CSS 2.1 block formatting context compliance
2. **Full margin collapsing** — Parent-child and empty-block collapsing beyond adjacent siblings
3. **`position: sticky`** — Widely used in modern UIs
4. **Grid spanning / implicit grid** — Current grid is too simplified for real layouts
5. **Inline BiDi / shaping** — Required for internationalized text

### Medium Priority (Feature Coverage)
6. **`@layer` cascade layers** — Modern CSS architecture feature
7. **`@container` queries** — Essential for component-based design
8. **Table layout** — Still widely used
9. **Multicolumn layout** — Useful for document-style content
10. **Writing modes** — Required for CJK and RTL support

### High Priority (Performance)
11. **Paint invalidation** — Avoid full repaint; implement dirty-bit incremental painting
12. **Display item caching** — Skip unchanged items between frames
13. **Fragment caching** — Avoid relayout when constraints unchanged
14. **Style invalidation sets** — Targeted restyle instead of full tree restyle
15. **GPU rasterization** — Or Skia integration for hardware acceleration

### Low Priority (Polish)
16. Scrollbar compositing
17. `:has()`, `:is()`, `:where()` selectors
18. `@scope` support
19. CSS nesting
20. Anchor positioning

---

## Architectural Comparison Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CHROMIUM                                    │
│                                                                     │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐      │
│   │  HTML     │   │  CSS     │   │  Style   │   │ LayoutNG │      │
│   │  Parser   │──▶│  Parser  │──▶│ Resolver │──▶│ (Fragment│      │
│   │ (Blink)  │   │ (Blink)  │   │ (Bloom+  │   │  caching)│      │
│   └──────────┘   └──────────┘   │ Inval.Set│   └────┬─────┘      │
│                                  └──────────┘        │             │
│                                                      ▼             │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐      │
│   │  Display │   │  Surface │   │  cc/     │   │  PrePaint│      │
│   │ Compositor│◀──│ Aggregator│◀──│  (Tiles, │◀──│  Paint   │      │
│   │ (viz)    │   │          │   │  Sched.) │   │  Chunks  │      │
│   └──────────┘   └──────────┘   └──────────┘   └──────────┘      │
│        │                                                           │
│        ▼                                                           │
│   ┌──────────┐                                                     │
│   │  Skia    │  GPU/Software rendering                             │
│   │ (Ganesh/ │                                                     │
│   │ Graphite)│                                                     │
│   └──────────┘                                                     │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                          LiquiDE                                    │
│                                                                     │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐      │
│   │  DOM     │   │lightningcss  │ StyleEngine│   │  Layout  │      │
│   │ Document │──▶│  (CSS    │──▶│ (Cascade │──▶│  Engine  │      │
│   │ (788 ln) │   │  Parser) │   │  Map)    │   │ (Blk/Flx/│      │
│   └──────────┘   └──────────┘   └──────────┘   │  Grid)   │      │
│                                                  └────┬─────┘      │
│                                                       ▼            │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐      │
│   │  CPU     │   │  Scene   │   │  Pipeline│   │  Painter │      │
│   │ Rasterizer│◀──│  Graph   │◀──│  Bridge  │◀──│ (Display │      │
│   │ (23 mods)│   │ (31 kinds│   │ (Prop.   │   │  List)   │      │
│   └──────────┘   │  Damage) │   │  Trees)  │   └──────────┘      │
│                  └──────────┘   └──────────┘                       │
│                                                                     │
│   Unique:  Glass │ BlurBackdrop │ Tint │ DegradationController     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Summary Statistics

| Metric | Chromium | LiquiDE | Ratio |
|---|---|---|---|
| CSS longhands | ~580 | 118 | 20% |
| CSS shorthands | ~230 | 28 | 12% |
| ComputedStyle fields | ~500+ | 111 | 22% |
| Layout algorithms | 8+ | 5 (3 partial) | 38% |
| Display item types | ~40 | 38 | 95% |
| Property tree types | 4 | 4 | 100% |
| Scene/Layer types | ~10 generic | 31 domain-specific | Different design |
| Threads | 4+ | 1 | — |
| Rasterizer | Skia (GPU+CPU) | Custom CPU (3100+ lines) | Different tradeoff |
| Codebase size | ~8M lines (renderer) | ~70 crates | ~1-2% of Chromium |
