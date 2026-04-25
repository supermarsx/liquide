# CSS 2.1 & CSS 3 Specification Gap Analysis — LiquiDE Engine

Generated: 2026-03-07 | Updated: 2026-04-24 after t13 CSS conformance reset

This document still carries March audit-era estimates for untouched sections. The selector, shorthand, supports/media, import, scope, custom-property, and transition claims below were corrected to match the post-t13 tested surface.

---

## Executive Summary

| Specification Area | Coverage | Grade | Key Gaps |
|---|---|---|---|
| **CSS 2.1 Core** | ~92% | A | Counters not rendered, z-index stacking incomplete, paged media NYI |
| **CSS Selectors Level 3/4** | ~82% | B | :has() remains partial, :link/:any-link missing, ::backdrop and column combinator missing |
| **CSS Cascade Level 4/5** | ~78% | B- | revert still partial, @supports selector() missing, advanced media features still sparse |
| **CSS Flexbox Level 1** | ~95% | A | flex-flow shorthand missing, align-content missing space-evenly |
| **CSS Grid Level 2** | ~80% | B | grid/grid-template shorthands, subgrid incomplete, intrinsic heuristics |
| **CSS Multi-column Level 1** | ~85% | B | column-rule-* not parsed, balance algorithm simplified |
| **CSS Backgrounds & Borders 3** | ~72% | C+ | Layered backgrounds preserved, but repeat/position/size wiring remains incomplete |
| **CSS Colors Level 4** | ~90% | A- | lab()/lch() missing, currentcolor resolves to black |
| **CSS Transforms Level 1/2** | ~75% | B- | All 3D transforms missing (translate3d, rotate3d, matrix3d, perspective) |
| **CSS Transitions Level 1** | ~50% | C- | Limited runtime interpolation exists for the current numeric subset; broad property coverage is still missing |
| **CSS Animations Level 1** | ~40% | D | @keyframes parsed; liquide-animation crate exists but not integrated |
| **CSS Filters Level 1** | ~95% | A | All filters + backdrop-filter, SIMD-accelerated |
| **CSS Masking Level 1** | ~50% | D+ | clip-path works; mask-* properties stored but no renderer |
| **CSS Compositing Level 1** | ~95% | A | All 16 blend modes, SIMD-accelerated |
| **CSS Text Level 3/4** | ~90% | A- | hanging-punctuation NYI, text-justify stored but not applied |
| **CSS Text Decoration 3** | ~90% | A- | text-emphasis stored but not rendered |
| **CSS Fonts Level 3/4** | ~75% | B- | font-feature/variation-settings stored but no shaping integration |
| **CSS Writing Modes 3** | ~70% | C+ | text-orientation not parsed, direction not used in inline layout |
| **CSS Logical Properties 1** | ~95% | A | Full resolution to physical values |
| **CSS Overflow Level 3** | ~70% | C+ | No scrollbar rendering, scroll containers don't affect layout |
| **CSS Containment Level 2** | ~60% | C | contain property parsed but not enforced in layout |
| **CSS Scroll Snap Level 1** | ~30% | F | Properties parsed, no snap enforcement |
| **CSS User Interface 3** | ~80% | B | cursor full, outline defined but no renderer, user-select no selection rendering |

**Overall: ~75% CSS2/3 coverage**

---

## CSS 2.1 SPECIFICATION (Chapters 4–18)

### Chapter 4: Syntax and Basic Data Types
| Feature | Status | Location |
|---|---|---|
| Length units (px, em, rem, ex, ch, pt, cm, mm, in, vw, vh, vmin, vmax) | ✅ Full | theme-css/parser/values.rs:14-146 |
| CSS4 units (dvw, dvh, svw, svh, lvw, lvh, cqw, cqh, rlh, lh) | ✅ Full | theme-css/parser/values.rs:14-146 |
| Percentages | ✅ Full | dimension.rs |
| Colors (hex, rgb, rgba, named, hsl, hwb) | ✅ Full | theme-css/parser/values.rs:158-175 |
| calc(), min(), max(), clamp() | ✅ Full | theme-css/parser/math_expr.rs |
| inherit, initial, unset keywords | ✅ Full | engine/apply.rs:24-43 |
| Shorthand expansion | ⚠️ Partial | Common canonical shorthands are preserved; transition/animation/mask/border-image/offset remain incomplete |

### Chapter 5: Selectors
| Feature | Status | Location |
|---|---|---|
| Type (E), Universal (*), Class (.c), ID (#id) | ✅ Full | selector.rs:152-169 |
| Descendant, Child (>), Adjacent (+), General sibling (~) | ✅ Full | selector.rs:20-30 |
| Attribute selectors (all 7 operators + case-insensitive) | ✅ Full | selector.rs:124-150 |
| :hover, :focus, :active, :visited | ✅ Full | selector.rs:35-38 |
| :first-child, :last-child | ✅ Full | selector.rs:41-42 |
| :lang() | ✅ Full | selector.rs:66 |
| ::before, ::after | ✅ Full | selector.rs:92-93, cascade.rs:339 |
| ::first-line, ::first-letter | ⚠️ Parsed only | selector.rs:94-95 — styles computed but not applied during layout |

### Chapter 6: Cascading and Inheritance
| Feature | Status | Location |
|---|---|---|
| Specificity (a,b,c) calculation | ✅ Full | specificity.rs:9-45 |
| Cascade order (UA, user, author, !important) | ✅ Full | cascade.rs |
| Inheritance (~25 properties) | ✅ Full | inheritance.rs:6-51 |
| Initial values for all properties | ✅ Full | computed/mod.rs:590-690 |

### Chapter 8: Box Model
| Feature | Status | Location |
|---|---|---|
| margin, border, padding (all 4 sides) | ✅ Full | computed/mod.rs:59-64 |
| Border styles (all 10: none through outset) | ✅ Full | computed/border.rs:8-19 |
| Border-width keywords (thin/medium/thick) | ✅ Full | theme-css/parser/properties.rs:213-240 |
| Margin collapsing (siblings) | ✅ Full | block.rs:284-288 |
| Margin collapsing (parent-child) | ✅ Full | block.rs:300-320 |
| Margin collapsing (empty blocks) | ✅ Full | block.rs:301-312 |
| Negative margins | ✅ Full | Dimension supports negative |
| box-sizing: content-box, border-box | ✅ Full | computed/display.rs:62-72 |

### Chapter 9: Visual Formatting Model
| Feature | Status | Location |
|---|---|---|
| display: block, inline, inline-block, none, list-item, table-*, flex, grid, contents, flow-root | ✅ Full | computed/display.rs:6-39 |
| position: static, relative, absolute, fixed, sticky | ✅ Full | computed/display.rs:47-60 |
| float: left, right, none | ✅ Full | float.rs + block.rs |
| clear: left, right, both, none | ✅ Full | block.rs:531-542 |
| Block formatting context (BFC) | ✅ Full | block.rs:284-320 |
| Anonymous block/inline boxes | ✅ Full | block.rs flush_inline_run |
| z-index property | ✅ Full | computed/mod.rs:94 |
| Stacking context ordering | ⚠️ Partial | z-index stored but paint doesn't fully sort by stacking context rules |

### Chapter 10: Visual Formatting Model Details
| Feature | Status | Location |
|---|---|---|
| Width/height computation (all box types) | ✅ Full | block.rs:50-300 |
| min-width, max-width, min-height, max-height | ✅ Full | computed/mod.rs:55-58, block.rs:50-85 |
| Line height (normal, number, length) | ✅ Full | computed/typography.rs:18-29 |
| vertical-align (all 8 CSS2.1 values + length) | ✅ Full | computed/typography.rs:161-177 |
| Replaced elements (img, video, canvas, svg, iframe) | ✅ Full | replaced.rs:16-175 |
| aspect-ratio | ✅ Full | computed/visual.rs:76-86 |

### Chapter 11: Visual Effects
| Feature | Status | Location |
|---|---|---|
| overflow: visible, hidden, scroll, auto | ✅ Full | computed/mod.rs:131-132 |
| visibility: visible, hidden, collapse | ✅ Full | computed/display.rs:75-85 |
| clip (deprecated) | ✅ Full | engine/apply.rs:1450 |

### Chapter 12: Generated Content
| Feature | Status | Location |
|---|---|---|
| content: strings, attr(), open-quote, close-quote | ✅ Full | engine/content.rs:14-150 |
| content: counter(), counters() | ⚠️ Partial | Parsed; counter registry exists but values not rendered in layout |
| counter-increment, counter-reset | ⚠️ Partial | Parsed; counter.rs has registry but not fully wired |
| list-style-type (all CSS2.1 types) | ✅ Full | computed/misc.rs:222-253 |
| list-style-position (inside, outside) | ✅ Full | computed/misc.rs |
| list-style-image | ❌ Missing | Parsed but silently ignored |
| quotes property | ⚠️ Partial | Parsed; dynamic nesting not implemented |

### Chapter 13: Paged Media
| Feature | Status | Location |
|---|---|---|
| page-break-before/after/inside (→ break-*) | ⚠️ Parsed only | apply.rs:1283-1307 — no paged layout |
| orphans, widows | ⚠️ Partial | Parsed; enforced in multicol only |

### Chapter 14: Colors and Backgrounds
| Feature | Status | Location |
|---|---|---|
| color property | ✅ Full | computed/mod.rs:106 |
| background-color | ✅ Full | painter/mod.rs:399 |
| background-image (gradients) | ✅ Full | painter/gradients.rs |
| background-image (URLs) | ⚠️ Partial | Parsed; no image fetch/decode pipeline |
| background-repeat | ⚠️ Partial | Stored but repeat rendering not wired |
| background-position | ⚠️ Partial | Stored but positioning not fully wired |
| background-attachment | ⚠️ Partial | Parsed; fixed not wired to compositor |

### Chapter 15: Fonts
| Feature | Status | Location |
|---|---|---|
| font-family (generic + specific) | ✅ Full | computed/mod.rs:107 |
| font-style: normal, italic, oblique | ✅ Full | computed/typography.rs:5-16 |
| font-weight: normal, bold, 100-900 | ✅ Full | computed/mod.rs:109 |
| font-size (keywords, length, percentage) | ✅ Full | computed/mod.rs:108 |
| font-variant | ⚠️ Partial | Parsed; not applied during text rendering |

### Chapter 16: Text
| Feature | Status | Location |
|---|---|---|
| text-indent | ✅ Full | computed/mod.rs:121 |
| text-align: left, right, center, justify, start, end | ✅ Full | computed/typography.rs:31-45 |
| text-decoration (line, style, color, thickness) | ✅ Full | renderer/text.rs:352-448 |
| letter-spacing, word-spacing | ✅ Full | computed/mod.rs:112-113 |
| text-transform: capitalize, uppercase, lowercase | ✅ Full | computed/typography.rs:78-90 |
| white-space (all 6 values) | ✅ Full | computed/typography.rs:105-118 |
| word-break: normal, break-all, keep-all | ✅ Full | computed/typography.rs:120-132 |

### Chapter 17: Tables
| Feature | Status | Location |
|---|---|---|
| Table display values (all 9) | ✅ Full | computed/display.rs:14-38 |
| table-layout: auto, fixed | ✅ Full | computed/border.rs:62-71 |
| border-collapse: collapse, separate | ✅ Full | computed/border.rs:49-59 |
| border-spacing | ✅ Full | computed/mod.rs:161 |
| caption-side: top, bottom | ✅ Full | computed/border.rs:85-95 |
| empty-cells: show, hide | ✅ Full | computed/border.rs:73-83 |
| colspan, rowspan | ✅ Full | table.rs:86-155 |

### Chapter 18: User Interface
| Feature | Status | Location |
|---|---|---|
| cursor (24 values) | ✅ Full | computed/visual.rs:5-34 |
| outline (width, style, color, offset) | ✅ Full | computed/mod.rs:189 |
| outline rendering | ⚠️ Partial | DisplayItem exists; renderer doesn't implement it |

---

## CSS 3 SPECIFICATIONS

### CSS Selectors Level 3 & 4

#### Pseudo-Classes (41 total)
| Pseudo-Class | Status | Notes |
|---|---|---|
| :hover, :focus, :active | ✅ Full | |
| :visited | ✅ Full | Via pseudo-state flag |
| :focus-within, :focus-visible | ✅ Full | |
| :first-child, :last-child, :only-child | ✅ Full | Text nodes correctly filtered |
| :first-of-type, :last-of-type, :only-of-type | ✅ Full | |
| :nth-child(An+B), :nth-last-child(An+B) | ✅ Full | |
| :nth-of-type(An+B), :nth-last-of-type(An+B) | ✅ Full | |
| :root, :empty, :target, :scope | ✅ Full | |
| :enabled, :disabled, :checked | ✅ Full | |
| :required, :optional | ✅ Full | |
| :read-only, :read-write | ✅ Full | |
| :placeholder-shown | ✅ Full | |
| :default, :indeterminate | ✅ Full | |
| :valid, :invalid | ⚠️ Partial | Only checks aria-invalid attribute |
| :in-range, :out-of-range | ⚠️ Partial | Basic min/max bounds check |
| :is(), :where(), :not() | ✅ Full | Correct specificity |
| :has() | ⚠️ Partial | Relative combinators and nested parsing are covered, but selector-list and invalidation edge cases remain |
| :lang() | ✅ Full | Prefix matching (en matches en-US) |
| :link, :any-link | ❌ Missing | |
| :dir() | ⚠️ Partial | Inherited direction matching is covered; broader scoping/shadow semantics remain |
| :user-valid, :user-invalid | ❌ Missing | |
| :autofill, :blank, :modal | ❌ Missing | |
| :current, :past, :future | ❌ Missing | Time-dimensional |
| :nth-child(An+B of S) | ❌ Missing | Level 4 of-selector syntax |

#### Pseudo-Elements
| Pseudo-Element | Status | Notes |
|---|---|---|
| ::before, ::after | ✅ Full | Computed and laid out |
| ::first-line, ::first-letter | ⚠️ Parsed | Styles computed but not applied in layout |
| ::marker | ✅ Full | Computed in cascade |
| ::placeholder, ::selection | ✅ Full | Computed in cascade |
| ::backdrop | ❌ Missing | |
| ::cue, ::part(), ::slotted() | ❌ Missing | |

#### Combinators
| Combinator | Status |
|---|---|
| Descendant (space), Child (>), Adjacent (+), General (~) | ✅ Full |
| Column (||) | ❌ Missing |

### CSS Cascade & Inheritance Level 4/5

| Feature | Status | Notes |
|---|---|---|
| Specificity (a,b,c) with :where()=0 | ✅ Full | |
| Cascade origins (UA, user, author, inline) | ✅ Full | |
| !important reversed order | ✅ Full | |
| inherit, initial, unset | ✅ Full | |
| revert | ⚠️ Partial | Simplified as unset |
| revert-layer | ⚠️ Partial | Lower-origin/layer fallback is covered; broader cascade-edge parity is still incomplete |
| @import | ⚠️ Partial | Import qualifiers are evaluated during load/reload; watcher coverage outside watched roots remains limited |
| @media | ⚠️ Partial | Viewport, color-scheme, reduced-motion, hover/pointer, and range/or syntax are covered |
| @supports | ⚠️ Partial | Declaration checks and fail-closed unknown forms are covered; selector() remains unsupported |
| @layer | ✅ Full | Layer ordering enforced |
| @scope | ⚠️ Partial | scope-start and scope-end bounds are enforced; full scoping semantics remain incomplete |
| @container | ⚠️ Partial | Size queries and nested container contents are covered; style queries are still missing |
| @property | ⚠️ Partial | syntax/inherits/initial-value are parsed and registered; typed runtime consumption remains limited |
| @keyframes | ⚠️ Partial | Parsed; no runtime evaluation |
| @font-face | ✅ Full | |
| @counter-style | ⚠️ Partial | Parsed; not used in rendering |
| @page | ⚠️ Parsed only | No print support |
| var() with fallback | ✅ Full | variables.rs:60-67 |

#### Media Features
| Feature | Status |
|---|---|
| width, min-width, max-width | ✅ Full |
| height, min-height, max-height | ✅ Full |
| prefers-color-scheme | ✅ Full |
| prefers-reduced-motion | ✅ Full |
| aspect-ratio, orientation | ❌ Missing |
| resolution, color, color-gamut | ❌ Missing |
| hover, pointer, any-hover, any-pointer | ✅ Full |
| prefers-contrast, forced-colors | ❌ Missing |

### CSS Flexible Box Layout Level 1

| Property | Status | Notes |
|---|---|---|
| display: flex / inline-flex | ✅ Full | |
| flex-direction (4 values) | ✅ Full | |
| flex-wrap (3 values) | ✅ Full | wrap-reverse fixed |
| flex-flow (shorthand) | ❌ Missing | Not expanded |
| order | ✅ Full | Sorting at flex.rs:512 |
| flex-grow, flex-shrink, flex-basis | ✅ Full | |
| flex (shorthand) | ✅ Full | |
| justify-content (6 values incl. space-evenly) | ✅ Full | |
| align-items (5 values) | ✅ Full | |
| align-self (6 values) | ✅ Full | |
| align-content | ⚠️ Partial | Missing space-evenly |
| gap, row-gap, column-gap | ✅ Full | |

**Algorithm compliance**: All 8 steps of CSS Flexbox §9 implemented. Min/max constraints applied during grow/shrink. align-content: stretch correctly expands line cross-sizes.

### CSS Grid Layout Level 2

| Property | Status | Notes |
|---|---|---|
| display: grid / inline-grid | ✅ Full | |
| grid-template-columns/rows | ✅ Full | px, %, fr, auto, minmax(), repeat(), fit-content() |
| grid-template-areas | ✅ Full | Named areas |
| grid-template (shorthand) | ❌ Missing | |
| grid (shorthand) | ❌ Missing | |
| grid-auto-columns/rows | ✅ Full | |
| grid-auto-flow: row, column, dense | ✅ Full | Dense back-fill implemented |
| grid-column/row start/end | ✅ Full | Line numbers, names, span |
| grid-area | ✅ Full | |
| gap | ✅ Full | |
| justify-items/self, align-items/self | ✅ Full | |
| justify-content, align-content | ✅ Full | |
| Subgrid | ⚠️ Partial | Enum exists; track inheritance not wired |
| repeat(auto-fill) | ✅ Full | |
| repeat(auto-fit) | ⚠️ Partial | Empty track collapsing implemented |
| min-content/max-content sizing | ⚠️ Heuristic | Uses fr-weight approximation (0.5/1.5) |

### CSS Multi-column Layout Level 1

| Property | Status | Notes |
|---|---|---|
| column-count, column-width, columns | ✅ Full | |
| column-gap | ✅ Full | |
| column-rule-color/style/width | ❌ Missing | Struct defined but not parsed |
| column-span: none, all | ✅ Full | |
| column-fill: auto, balance | ✅ Full | |
| break-before/after/inside | ✅ Full | |
| orphans, widows | ✅ Full | Enforced in multicol |

### CSS Backgrounds and Borders Level 3

| Property | Status | Notes |
|---|---|---|
| background-color | ✅ Full | |
| background-image (gradients) | ✅ Full | Linear, radial, conic |
| background-image (URLs) | ⚠️ Partial | Parsed; no image loading pipeline |
| background-repeat | ⚠️ Partial | Stored; repeat rendering not wired |
| background-position | ⚠️ Partial | Stored; not fully applied in paint |
| background-size | ⚠️ Partial | Stored as string; not resolved |
| background-clip (border/padding/content) | ✅ Full | text falls back to padding |
| background-origin | ⚠️ Partial | Resolved but positioning not wired |
| background-attachment: fixed | ⚠️ Flag only | Tracked; not wired to compositor |
| Multiple backgrounds | ⚠️ Partial | Layered images are preserved through parsing/expansion; full per-layer rendering is incomplete |
| background shorthand (full) | ⚠️ Partial | Raw shorthand text and layered image lists are preserved; full per-layer decomposition is incomplete |
| border-radius (all 4 corners) | ✅ Full | Single f32 per corner (not elliptical) |
| border-image (source, slice, width, outset, repeat) | ⚠️ Partial | Parsed; 9-slice rendering incomplete |
| box-shadow (offset, blur, spread, inset, multiple) | ✅ Full | Anti-aliased rendering |

### CSS Images Module Level 3

| Feature | Status | Notes |
|---|---|---|
| linear-gradient() | ✅ Full | Angle, stops, hints |
| radial-gradient() | ✅ Full | Circle + ellipse with separate rx/ry |
| conic-gradient() | ✅ Full | Angle, center, stops |
| repeating-linear/radial/conic-gradient() | ❌ Missing | |
| image-set() | ❌ Missing | |
| object-fit (fill, contain, cover, none, scale-down) | ✅ Full | |
| object-position | ⚠️ Partial | Stored; not applied in painter |
| image-rendering | ⚠️ Partial | Stored; not applied |

### CSS Color Module Level 4

| Format | Status | Notes |
|---|---|---|
| Named colors (148+) | ✅ Full | Via csscolorparser |
| Hex (#RGB, #RRGGBB, #RGBA, #RRGGBBAA) | ✅ Full | |
| rgb()/rgba() (modern + legacy syntax) | ✅ Full | |
| hsl()/hsla() | ✅ Full | |
| hwb() | ✅ Full | |
| oklab()/oklch() | ✅ Full | Explicit parsers |
| color() (srgb, display-p3, etc.) | ✅ Full | 8+ color spaces |
| color-mix() | ✅ Full | sRGB mixing |
| lab()/lch() | ❌ Missing | csscolorparser doesn't support |
| currentcolor | ⚠️ Partial | Resolves to black (no context) |
| transparent | ✅ Full | |
| opacity | ✅ Full | |

### CSS Transforms Level 1 & 2

| Feature | Status | Notes |
|---|---|---|
| translate(), translateX/Y() | ✅ Full | |
| scale(), scaleX/Y() | ✅ Full | |
| rotate() | ✅ Full | |
| skew(), skewX/Y() | ✅ Full | |
| matrix() | ✅ Full | 2D affine |
| Individual: translate, rotate, scale | ✅ Full | Composed in assemble.rs |
| transform-origin (2/3-value) | ✅ Full | |
| translate3d(), scale3d(), rotate3d() | ❌ Missing | No 3D variants |
| matrix3d() | ❌ Missing | |
| perspective() function | ❌ Missing | No 3D projection |
| perspective property | ⚠️ Parsed only | |
| transform-style: preserve-3d | ⚠️ Parsed only | No 3D context |
| backface-visibility | ⚠️ Parsed only | |

### CSS Transitions Level 1

| Feature | Status | Notes |
|---|---|---|
| transition-property | ⚠️ Partial | Explicit lists are parsed; `all` expands over the current numeric runtime subset |
| transition-duration | ⚠️ Partial | Parsed and consumed by the runtime transition manager |
| transition-timing-function | ✅ Full | TimingFunction enum complete |
| transition-delay | ⚠️ Partial | Parsed and consumed by the runtime transition manager |
| **Runtime scheduler** | ⚠️ Partial | Interpolation exists for the current numeric subset only |

### CSS Animations Level 1

| Feature | Status | Notes |
|---|---|---|
| @keyframes rule | ⚠️ Parsed | lightningcss handles parsing |
| animation-name through animation-play-state | ✅ Full | All properties stored with proper enums |
| animation shorthand | ⚠️ Partial | |
| **Runtime scheduler** | ❌ Missing | liquide-animation crate exists but not wired into pipeline |

### CSS Filter Effects Module Level 1

| Filter | Status | Notes |
|---|---|---|
| blur() | ✅ Full | SIMD-accelerated |
| brightness(), contrast() | ✅ Full | SIMD-accelerated |
| grayscale(), sepia(), saturate() | ✅ Full | SIMD-accelerated |
| hue-rotate(), invert(), opacity() | ✅ Full | |
| drop-shadow() | ✅ Full | |
| url() (SVG reference filter) | ⚠️ Parsed | No SVG filter support |
| backdrop-filter | ✅ Full | All functions, SIMD |

### CSS Masking Module Level 1

| Feature | Status | Notes |
|---|---|---|
| clip-path: inset() | ✅ Full | |
| clip-path: circle() | ✅ Full | SDF anti-aliasing |
| clip-path: ellipse() | ✅ Full | SDF anti-aliasing |
| clip-path: polygon() | ✅ Full | Winding number + SDF AA |
| clip-path: path() | ❌ Missing | |
| clip-path: url() | ❌ Missing | |
| mask-image through mask-composite | ⚠️ Parsed only | Properties stored; no rendering |

### CSS Compositing and Blending Level 1

| Feature | Status | Notes |
|---|---|---|
| mix-blend-mode (all 16 modes) | ✅ Full | SIMD-accelerated |
| isolation: auto, isolate | ✅ Full | |
| background-blend-mode | ✅ Full | |

### CSS Overflow Module Level 3

| Feature | Status | Notes |
|---|---|---|
| overflow, overflow-x, overflow-y (5 values) | ✅ Full | Stored; clip applied in painter |
| text-overflow: clip, ellipsis | ✅ Full | |
| overflow-clip-margin | ⚠️ Parsed only | |
| Scrollbar rendering | ❌ Missing | |
| Scroll containers in layout | ❌ Missing | overflow doesn't affect child sizing |

### CSS Fonts Module Level 3/4

| Feature | Status | Notes |
|---|---|---|
| @font-face | ✅ Full | |
| font-family, font-weight, font-style, font-size | ✅ Full | |
| font-stretch (9 values) | ✅ Full | |
| font-size-adjust | ⚠️ Parsed only | |
| font-variant (all sub-properties) | ⚠️ Parsed only | Not applied in shaping |
| font-feature-settings | ⚠️ Parsed only | No HarfBuzz integration |
| font-variation-settings | ⚠️ Parsed only | No variable font axis support |
| font-display | ❌ Missing | |
| line-height (normal, number, length) | ✅ Full | |

### CSS Text Module Level 3/4

| Feature | Status | Notes |
|---|---|---|
| text-transform (capitalize, uppercase, lowercase) | ✅ Full | |
| white-space (6 values incl. break-spaces) | ✅ Full | |
| tab-size | ✅ Full | |
| word-break (4 values) | ✅ Full | |
| overflow-wrap (3 values) | ✅ Full | |
| hyphens: none, manual, auto | ✅ Full | |
| text-align (6 values) | ✅ Full | |
| text-align-last | ✅ Full | |
| text-indent | ✅ Full | |
| letter-spacing, word-spacing | ✅ Full | |
| text-justify | ⚠️ Parsed only | Not applied in justify algorithm |
| white-space-collapse | ⚠️ Parsed only | |
| line-break (CJK) | ⚠️ Parsed only | |
| hanging-punctuation | ⚠️ Parsed only | |

### CSS Text Decoration Module Level 3

| Feature | Status | Notes |
|---|---|---|
| text-decoration-line (underline, overline, line-through) | ✅ Full | |
| text-decoration-style (solid, double, dotted, dashed, wavy) | ✅ Full | All rendered |
| text-decoration-color | ✅ Full | |
| text-decoration-thickness | ✅ Full | |
| text-underline-offset | ✅ Full | |
| text-underline-position | ✅ Full | |
| text-shadow (offset, blur, color, multiple) | ✅ Full | Rendered with offset |
| text-emphasis-style/color/position | ⚠️ Parsed only | No rendering |

### CSS Writing Modes Level 3/4

| Feature | Status | Notes |
|---|---|---|
| writing-mode (5 values) | ✅ Full | horizontal-tb, vertical-rl/lr, sideways-rl/lr |
| direction: ltr, rtl | ⚠️ Partial | Parsed; not used in inline layout |
| unicode-bidi (6 values) | ⚠️ Parsed only | |
| text-orientation | ❌ Missing | Enum defined; not parsed in apply.rs |

### CSS Logical Properties Level 1

| Feature | Status | Notes |
|---|---|---|
| inline-size, block-size, min-*, max-* | ✅ Full | Resolved via logical.rs |
| margin-inline-start/end, margin-block-start/end | ✅ Full | |
| padding-inline-*, padding-block-* | ✅ Full | |
| inset-inline-*, inset-block-* | ✅ Full | |
| border-inline-*, border-block-* (width) | ✅ Full | |
| border-*-radius (start-start, etc.) | ✅ Full | |

### CSS Box Alignment Module Level 3

| Feature | Status | Notes |
|---|---|---|
| justify-content (6 values) | ✅ Full | |
| align-content | ⚠️ Partial | Missing space-evenly |
| justify-items, align-items | ✅ Full | |
| justify-self, align-self | ✅ Full | |
| place-content, place-items, place-self | ✅ Full | |
| gap, row-gap, column-gap | ✅ Full | |
| self-start, self-end values | ❌ Missing | Enum exists but not parsed |

### CSS Positioned Layout Level 3

| Feature | Status | Notes |
|---|---|---|
| position: static, relative, absolute, fixed, sticky | ✅ Full | |
| top, right, bottom, left | ✅ Full | |
| inset (shorthand) | ✅ Full | |
| z-index | ✅ Full | |
| Sticky clamping | ✅ Full | positioned.rs:160-169 |
| Anchor positioning (position-anchor, position-area) | ⚠️ Partial | Properties exist; alignment not applied |

### CSS Containment Level 2

| Feature | Status | Notes |
|---|---|---|
| contain: none, strict, content, size, layout, style, paint | ✅ Full | Bitflags parsed |
| content-visibility: visible, auto, hidden | ✅ Full | |
| contain-intrinsic-size | ⚠️ Parsed only | No intrinsic sizing logic |
| **Layout containment enforcement** | ❌ Missing | contain: layout doesn't skip descendants |

### CSS Scroll Snap Level 1

| Feature | Status | Notes |
|---|---|---|
| scroll-snap-type, scroll-snap-align, scroll-snap-stop | ✅ Full | Parsed with enums |
| scroll-padding, scroll-margin | ✅ Full | Parsed |
| **Snap enforcement** | ❌ Missing | No scroll snap logic |

### CSS User Interface Level 3

| Feature | Status | Notes |
|---|---|---|
| cursor (23 values) | ✅ Full | Hardware cursor on Win32 |
| outline (width, style, color, offset) | ✅ Full | DisplayItem emitted |
| outline rendering | ❌ Missing | No renderer path |
| resize: none, both, horizontal, vertical | ✅ Full | |
| caret-color | ✅ Full | |
| user-select | ✅ Full | No selection rendering |
| pointer-events: auto, none | ✅ Full | Hit-test aware |
| appearance: none, auto | ⚠️ Parsed only | |

---

## PRIORITY FIX LIST

### Tier 1 — High Impact, Moderate Effort
1. **Animation/transition scheduler** — Wire liquide-animation into paint pipeline
2. **Background-image positioning/repeat/size** — Complete background rendering
3. **3D transforms** — Add translate3d, rotate3d, matrix3d, perspective projection
4. **Mask rendering** — Wire mask-* properties to compositor
5. **align-content: space-evenly** — Add to AlignContent enum
6. **text-orientation parsing** — Add to apply.rs
7. **direction in inline layout** — RTL text flow
8. **Outline rendering** — Add renderer path for outlines
9. **Scrollbar rendering** — Visual scroll indicators

### Tier 2 — Medium Impact
10. **Repeating gradients** — repeating-linear/radial/conic-gradient
11. **Multiple backgrounds** — Finish per-layer repeat/position/size/origin rendering
12. **Grid/grid-template shorthands** — Parse and expand
13. **flex-flow shorthand** — Parse and expand
14. **column-rule-* properties** — Parse and render
15. **Stacking context ordering** — Full z-index paint sort
16. **Subgrid track inheritance** — Wire parent track data
17. **Media features** — orientation, resolution, aspect-ratio, contrast/color-gamut, and broader system prefs
18. **currentcolor resolution** — Resolve to inherited color, not black
19. **Elliptical border-radius** — Separate x/y per corner

### Tier 3 — Low Impact / Edge Cases
20. **lab()/lch() colors** — Add parser
21. **revert / revert-layer parity** — close remaining cascade edge cases beyond the validated fallback paths
22. **Full @scope semantics** — extend beyond structural start/end boundary enforcement
23. **:link, :any-link, richer selector-list pseudos** — remaining selector surface
24. **::first-line/::first-letter application** — Complex partial styling
25. **Counter rendering** — Wire counter values to content
26. **Scroll snap enforcement** — Snap point logic
27. **font-feature/variation-settings** — HarfBuzz integration
28. **text-emphasis rendering** — Emphasis marks above/below text
29. **Containment enforcement** — Skip descendant layout for contain: layout
30. **border-image 9-slice** — Proper slice/repeat rendering
