# Complete CSS Longhand Properties Reference

**Sources:** MDN Web Docs CSS Reference, W3C All CSS Properties Index  
**Date:** February 2026  
**Total unique longhand properties:** ~370 (standard) + ~20 (-webkit- prefixed)

> Properties marked with `[S]` are **shorthands** (included for completeness but they expand to longhands).  
> Properties marked with `[L]` are **logical** (writing-mode-aware equivalents of physical properties).

---

## 1. Display & Box Generation

| Property | Type | Notes |
|----------|------|-------|
| display | longhand | Block/inline/flex/grid/table/none/contents |
| visibility | longhand | visible/hidden/collapse |
| content | longhand | For ::before/::after pseudo-elements |
| content-visibility | longhand | auto/hidden/visible (containment) |
| overlay | longhand | auto/none (top-layer rendering) |

---

## 2. Positioning

| Property | Type | Notes |
|----------|------|-------|
| position | longhand | static/relative/absolute/fixed/sticky |
| top | longhand | Physical offset |
| right | longhand | Physical offset |
| bottom | longhand | Physical offset |
| left | longhand | Physical offset |
| inset | `[S]` | Shorthand for top/right/bottom/left |
| inset-block | `[S][L]` | Shorthand for block-start/end |
| inset-block-start | `[L]` | Logical equivalent of top/bottom |
| inset-block-end | `[L]` | Logical equivalent of top/bottom |
| inset-inline | `[S][L]` | Shorthand for inline-start/end |
| inset-inline-start | `[L]` | Logical equivalent of left/right |
| inset-inline-end | `[L]` | Logical equivalent of left/right |
| z-index | longhand | Stacking order |
| float | longhand | left/right/none/inline-start/inline-end |
| clear | longhand | left/right/both/none/inline-start/inline-end |

### Anchor Positioning

| Property | Type | Notes |
|----------|------|-------|
| anchor-name | longhand | Names an anchor element |
| anchor-scope | longhand | Limits anchor visibility |
| position-anchor | longhand | Associates with named anchor |
| position-area | longhand | Positions relative to anchor |
| position-try | `[S]` | Shorthand for try-fallbacks + try-order |
| position-try-fallbacks | longhand | Fallback position strategies |
| position-try-order | longhand | Order to try fallbacks |
| position-visibility | longhand | Visibility when anchor missing |

---

## 3. Box Model — Sizing

| Property | Type | Notes |
|----------|------|-------|
| width | longhand | Physical width |
| height | longhand | Physical height |
| min-width | longhand | Minimum width |
| min-height | longhand | Minimum height |
| max-width | longhand | Maximum width |
| max-height | longhand | Maximum height |
| inline-size | `[L]` | Logical width |
| block-size | `[L]` | Logical height |
| min-inline-size | `[L]` | Logical min-width |
| min-block-size | `[L]` | Logical min-height |
| max-inline-size | `[L]` | Logical max-width |
| max-block-size | `[L]` | Logical max-height |
| box-sizing | longhand | content-box/border-box |
| aspect-ratio | longhand | auto / ratio |

### Intrinsic Sizing

| Property | Type | Notes |
|----------|------|-------|
| contain-intrinsic-size | `[S]` | Shorthand |
| contain-intrinsic-width | longhand | Placeholder width for content-visibility |
| contain-intrinsic-height | longhand | Placeholder height |
| contain-intrinsic-inline-size | `[L]` | Logical placeholder |
| contain-intrinsic-block-size | `[L]` | Logical placeholder |
| interpolate-size | longhand | Allow animating to/from intrinsic sizes |
| field-sizing | longhand | content/fixed (form controls) |

---

## 4. Box Model — Margin

| Property | Type | Notes |
|----------|------|-------|
| margin | `[S]` | Shorthand for all 4 sides |
| margin-top | longhand | Physical top margin |
| margin-right | longhand | Physical right margin |
| margin-bottom | longhand | Physical bottom margin |
| margin-left | longhand | Physical left margin |
| margin-block | `[S][L]` | Shorthand for block margins |
| margin-block-start | `[L]` | Logical top margin |
| margin-block-end | `[L]` | Logical bottom margin |
| margin-inline | `[S][L]` | Shorthand for inline margins |
| margin-inline-start | `[L]` | Logical left margin |
| margin-inline-end | `[L]` | Logical right margin |
| margin-trim | longhand | Trim child margins at container edges |

---

## 5. Box Model — Padding

| Property | Type | Notes |
|----------|------|-------|
| padding | `[S]` | Shorthand for all 4 sides |
| padding-top | longhand | Physical top padding |
| padding-right | longhand | Physical right padding |
| padding-bottom | longhand | Physical bottom padding |
| padding-left | longhand | Physical left padding |
| padding-block | `[S][L]` | Shorthand for block padding |
| padding-block-start | `[L]` | Logical top padding |
| padding-block-end | `[L]` | Logical bottom padding |
| padding-inline | `[S][L]` | Shorthand for inline padding |
| padding-inline-start | `[L]` | Logical left padding |
| padding-inline-end | `[L]` | Logical right padding |

---

## 6. Box Model — Border

### Border Width

| Property | Type | Notes |
|----------|------|-------|
| border | `[S]` | Mega shorthand |
| border-width | `[S]` | Shorthand for all border widths |
| border-top-width | longhand | |
| border-right-width | longhand | |
| border-bottom-width | longhand | |
| border-left-width | longhand | |
| border-block-width | `[S][L]` | |
| border-block-start-width | `[L]` | |
| border-block-end-width | `[L]` | |
| border-inline-width | `[S][L]` | |
| border-inline-start-width | `[L]` | |
| border-inline-end-width | `[L]` | |

### Border Style

| Property | Type | Notes |
|----------|------|-------|
| border-style | `[S]` | Shorthand for all border styles |
| border-top-style | longhand | |
| border-right-style | longhand | |
| border-bottom-style | longhand | |
| border-left-style | longhand | |
| border-block-style | `[S][L]` | |
| border-block-start-style | `[L]` | |
| border-block-end-style | `[L]` | |
| border-inline-style | `[S][L]` | |
| border-inline-start-style | `[L]` | |
| border-inline-end-style | `[L]` | |

### Border Color

| Property | Type | Notes |
|----------|------|-------|
| border-color | `[S]` | Shorthand for all border colors |
| border-top-color | longhand | |
| border-right-color | longhand | |
| border-bottom-color | longhand | |
| border-left-color | longhand | |
| border-block-color | `[S][L]` | |
| border-block-start-color | `[L]` | |
| border-block-end-color | `[L]` | |
| border-inline-color | `[S][L]` | |
| border-inline-start-color | `[L]` | |
| border-inline-end-color | `[L]` | |

### Border Side Shorthands

| Property | Type | Notes |
|----------|------|-------|
| border-top | `[S]` | width + style + color |
| border-right | `[S]` | |
| border-bottom | `[S]` | |
| border-left | `[S]` | |
| border-block | `[S][L]` | |
| border-block-start | `[S][L]` | |
| border-block-end | `[S][L]` | |
| border-inline | `[S][L]` | |
| border-inline-start | `[S][L]` | |
| border-inline-end | `[S][L]` | |

### Border Radius

| Property | Type | Notes |
|----------|------|-------|
| border-radius | `[S]` | Shorthand for 4 corners |
| border-top-left-radius | longhand | |
| border-top-right-radius | longhand | |
| border-bottom-right-radius | longhand | |
| border-bottom-left-radius | longhand | |
| border-start-start-radius | `[L]` | |
| border-start-end-radius | `[L]` | |
| border-end-start-radius | `[L]` | |
| border-end-end-radius | `[L]` | |

### Border Image

| Property | Type | Notes |
|----------|------|-------|
| border-image | `[S]` | Shorthand |
| border-image-source | longhand | URL/gradient for border image |
| border-image-slice | longhand | How to slice the image |
| border-image-width | longhand | Width of image border |
| border-image-outset | longhand | Outset from border box |
| border-image-repeat | longhand | stretch/repeat/round/space |

### Border Misc

| Property | Type | Notes |
|----------|------|-------|
| border-collapse | longhand | collapse/separate (tables) |
| border-spacing | longhand | Space between table cells |

---

## 7. Background

| Property | Type | Notes |
|----------|------|-------|
| background | `[S]` | Mega shorthand |
| background-color | longhand | |
| background-image | longhand | URL/gradient |
| background-position | `[S]` | Shorthand for x/y |
| background-position-x | longhand | |
| background-position-y | longhand | |
| background-size | longhand | cover/contain/length |
| background-repeat | longhand | repeat/no-repeat/space/round |
| background-origin | longhand | border-box/padding-box/content-box |
| background-clip | longhand | border-box/padding-box/content-box/text |
| background-attachment | longhand | scroll/fixed/local |
| background-blend-mode | longhand | normal/multiply/screen/overlay/... |

---

## 8. Box Decoration

| Property | Type | Notes |
|----------|------|-------|
| box-shadow | longhand | Shadow effect(s) |
| box-decoration-break | longhand | slice/clone (fragmentation) |
| outline | `[S]` | Shorthand |
| outline-width | longhand | |
| outline-style | longhand | |
| outline-color | longhand | |
| outline-offset | longhand | Gap between outline and border |
| opacity | longhand | 0–1 transparency |

---

## 9. Flexbox

| Property | Type | Notes |
|----------|------|-------|
| flex | `[S]` | Shorthand for grow/shrink/basis |
| flex-direction | longhand | row/column/row-reverse/column-reverse |
| flex-wrap | longhand | nowrap/wrap/wrap-reverse |
| flex-flow | `[S]` | Shorthand for direction + wrap |
| flex-grow | longhand | Growth factor |
| flex-shrink | longhand | Shrink factor |
| flex-basis | longhand | Initial main size |
| order | longhand | Visual order of flex/grid items |

---

## 10. Grid

| Property | Type | Notes |
|----------|------|-------|
| grid | `[S]` | Mega shorthand |
| grid-template | `[S]` | Shorthand for rows/columns/areas |
| grid-template-rows | longhand | Row track definitions |
| grid-template-columns | longhand | Column track definitions |
| grid-template-areas | longhand | Named grid areas |
| grid-auto-rows | longhand | Implicit row sizing |
| grid-auto-columns | longhand | Implicit column sizing |
| grid-auto-flow | longhand | row/column/dense |
| grid-row | `[S]` | Shorthand for row-start/end |
| grid-row-start | longhand | Placement start |
| grid-row-end | longhand | Placement end |
| grid-column | `[S]` | Shorthand for column-start/end |
| grid-column-start | longhand | Placement start |
| grid-column-end | longhand | Placement end |
| grid-area | `[S]` | Shorthand for row-start/column-start/row-end/column-end |

---

## 11. Alignment (Flex & Grid)

| Property | Type | Notes |
|----------|------|-------|
| align-content | longhand | Align content on cross axis |
| align-items | longhand | Default align-self for children |
| align-self | longhand | Override alignment for one item |
| justify-content | longhand | Distribute on main axis |
| justify-items | longhand | Default justify-self for children |
| justify-self | longhand | Override justification for one item |
| place-content | `[S]` | Shorthand: align-content + justify-content |
| place-items | `[S]` | Shorthand: align-items + justify-items |
| place-self | `[S]` | Shorthand: align-self + justify-self |
| gap | `[S]` | Shorthand for row-gap + column-gap |
| row-gap | longhand | Gap between rows |
| column-gap | longhand | Gap between columns |

---

## 12. Multi-Column Layout

| Property | Type | Notes |
|----------|------|-------|
| columns | `[S]` | Shorthand for width + count |
| column-width | longhand | Ideal column width |
| column-count | longhand | Number of columns |
| column-gap | longhand | (shared with grid gap) |
| column-rule | `[S]` | Shorthand |
| column-rule-width | longhand | |
| column-rule-style | longhand | |
| column-rule-color | longhand | |
| column-span | longhand | none/all |
| column-fill | longhand | auto/balance/balance-all |

---

## 13. Typography — Font

| Property | Type | Notes |
|----------|------|-------|
| font | `[S]` | Mega shorthand |
| font-family | longhand | Font stack |
| font-size | longhand | Absolute/relative/length |
| font-size-adjust | longhand | Normalize x-height across fonts |
| font-weight | longhand | normal/bold/100-900 |
| font-style | longhand | normal/italic/oblique |
| font-stretch | longhand | ultra-condensed to ultra-expanded |
| font-variant | `[S]` | Shorthand for all variant sub-properties |
| font-variant-ligatures | longhand | |
| font-variant-caps | longhand | small-caps/petite-caps/etc. |
| font-variant-numeric | longhand | lining/oldstyle/tabular/etc. |
| font-variant-east-asian | longhand | jis78/simplified/etc. |
| font-variant-alternates | longhand | stylistic()/swash()/etc. |
| font-variant-position | longhand | sub/super |
| font-variant-emoji | longhand | text/emoji/unicode |
| font-feature-settings | longhand | OpenType features |
| font-variation-settings | longhand | Variable font axes |
| font-kerning | longhand | auto/normal/none |
| font-optical-sizing | longhand | auto/none |
| font-language-override | longhand | Override language system |
| font-palette | longhand | Color palettes in color fonts |
| font-synthesis | `[S]` | Shorthand |
| font-synthesis-weight | longhand | auto/none |
| font-synthesis-style | longhand | auto/none |
| font-synthesis-small-caps | longhand | auto/none |
| font-synthesis-position | longhand | auto/none |

---

## 14. Typography — Text

| Property | Type | Notes |
|----------|------|-------|
| color | longhand | Foreground/text color |
| line-height | longhand | Leading |
| letter-spacing | longhand | Tracking |
| word-spacing | longhand | Space between words |
| text-align | longhand | left/right/center/justify/start/end |
| text-align-last | longhand | Alignment of last line |
| text-indent | longhand | First-line indent |
| text-transform | longhand | uppercase/lowercase/capitalize/full-width |
| text-decoration | `[S]` | Shorthand |
| text-decoration-line | longhand | underline/overline/line-through |
| text-decoration-style | longhand | solid/double/dotted/dashed/wavy |
| text-decoration-color | longhand | |
| text-decoration-thickness | longhand | |
| text-decoration-skip-ink | longhand | auto/none/all |
| text-underline-offset | longhand | Offset from baseline |
| text-underline-position | longhand | auto/under/left/right |
| text-emphasis | `[S]` | Shorthand |
| text-emphasis-style | longhand | filled/open dot/circle/etc. |
| text-emphasis-color | longhand | |
| text-emphasis-position | longhand | over/under + left/right |
| text-shadow | longhand | Shadow effect(s) on text |
| text-overflow | longhand | clip/ellipsis |
| text-wrap | `[S]` | Shorthand for mode + style |
| text-wrap-mode | longhand | wrap/nowrap |
| text-wrap-style | longhand | auto/balance/pretty/stable |
| white-space | longhand | normal/nowrap/pre/pre-wrap/pre-line/break-spaces |
| white-space-collapse | longhand | collapse/preserve/preserve-breaks |
| word-break | longhand | normal/break-all/keep-all/break-word |
| overflow-wrap | longhand | normal/anywhere/break-word |
| line-break | longhand | auto/loose/normal/strict/anywhere |
| hyphens | longhand | none/manual/auto |
| hyphenate-character | longhand | auto/string |
| hyphenate-limit-chars | longhand | min-word/before/after |
| hanging-punctuation | longhand | none/first/last/force-end/allow-end |
| tab-size | longhand | Number or length |
| text-justify | longhand | auto/inter-word/inter-character/none |
| text-orientation | longhand | mixed/upright/sideways |
| text-combine-upright | longhand | none/all |
| text-rendering | longhand | auto/optimizeSpeed/optimizeLegibility/geometricPrecision |
| text-size-adjust | longhand | auto/none/percentage |
| text-autospace | longhand | ideograph-alpha/ideograph-numeric/etc. |
| text-spacing-trim | longhand | space-all/space-first/trim-start/etc. |
| text-box | `[S]` | Shorthand for edge + trim |
| text-box-edge | longhand | leading/text/cap/ex/etc. |
| text-box-trim | longhand | none/start/end/both |

---

## 15. Typography — Writing Modes

| Property | Type | Notes |
|----------|------|-------|
| writing-mode | longhand | horizontal-tb/vertical-rl/vertical-lr/sideways-rl/sideways-lr |
| direction | longhand | ltr/rtl |
| unicode-bidi | longhand | normal/embed/isolate/bidi-override/isolate-override/plaintext |

---

## 16. Typography — Lists

| Property | Type | Notes |
|----------|------|-------|
| list-style | `[S]` | Shorthand |
| list-style-type | longhand | disc/circle/square/decimal/etc. |
| list-style-position | longhand | inside/outside |
| list-style-image | longhand | URL |
| counter-increment | longhand | Increment counter(s) |
| counter-reset | longhand | Reset counter(s) |
| counter-set | longhand | Set counter value |
| quotes | longhand | open-quote/close-quote strings |

---

## 17. Color & Appearance

| Property | Type | Notes |
|----------|------|-------|
| color | longhand | (also in typography) |
| accent-color | longhand | Tint color for form controls |
| color-scheme | longhand | light/dark/normal |
| forced-color-adjust | longhand | auto/none |
| print-color-adjust | longhand | economy/exact |
| appearance | longhand | none/auto |
| caret-color | longhand | Color of text cursor |
| caret | `[S]` | Shorthand for color + shape |
| caret-shape | longhand | auto/bar/block/underscore |

---

## 18. Overflow & Scrolling

| Property | Type | Notes |
|----------|------|-------|
| overflow | `[S]` | Shorthand for x + y |
| overflow-x | longhand | visible/hidden/clip/scroll/auto |
| overflow-y | longhand | |
| overflow-block | `[L]` | |
| overflow-inline | `[L]` | |
| overflow-clip-margin | longhand | Distance before clipping |
| overflow-anchor | longhand | auto/none (scroll anchoring) |
| overflow-wrap | longhand | (also in text) |
| scroll-behavior | longhand | auto/smooth |
| scroll-snap-type | longhand | none/x/y/block/inline/both mandatory/proximity |
| scroll-snap-align | longhand | none/start/end/center |
| scroll-snap-stop | longhand | normal/always |
| scroll-marker-group | longhand | none/before/after |
| scrollbar-width | longhand | auto/thin/none |
| scrollbar-color | longhand | auto/color pair |
| scrollbar-gutter | longhand | auto/stable/always |
| overscroll-behavior | `[S]` | Shorthand |
| overscroll-behavior-x | longhand | auto/contain/none |
| overscroll-behavior-y | longhand | |
| overscroll-behavior-block | `[L]` | |
| overscroll-behavior-inline | `[L]` | |

### Scroll Snap Margin (scroll-margin)

| Property | Type | Notes |
|----------|------|-------|
| scroll-margin | `[S]` | |
| scroll-margin-top | longhand | |
| scroll-margin-right | longhand | |
| scroll-margin-bottom | longhand | |
| scroll-margin-left | longhand | |
| scroll-margin-block | `[S][L]` | |
| scroll-margin-block-start | `[L]` | |
| scroll-margin-block-end | `[L]` | |
| scroll-margin-inline | `[S][L]` | |
| scroll-margin-inline-start | `[L]` | |
| scroll-margin-inline-end | `[L]` | |

### Scroll Snap Padding (scroll-padding)

| Property | Type | Notes |
|----------|------|-------|
| scroll-padding | `[S]` | |
| scroll-padding-top | longhand | |
| scroll-padding-right | longhand | |
| scroll-padding-bottom | longhand | |
| scroll-padding-left | longhand | |
| scroll-padding-block | `[S][L]` | |
| scroll-padding-block-start | `[L]` | |
| scroll-padding-block-end | `[L]` | |
| scroll-padding-inline | `[S][L]` | |
| scroll-padding-inline-start | `[L]` | |
| scroll-padding-inline-end | `[L]` | |

---

## 19. Transform

| Property | Type | Notes |
|----------|------|-------|
| transform | longhand | Transform function list |
| transform-origin | longhand | Origin point for transforms |
| transform-box | longhand | Reference box: content-box/border-box/fill-box/stroke-box/view-box |
| transform-style | longhand | flat/preserve-3d |
| translate | longhand | Individual translate transform |
| rotate | longhand | Individual rotate transform |
| scale | longhand | Individual scale transform |
| perspective | longhand | 3D perspective depth |
| perspective-origin | longhand | Vanishing point |
| backface-visibility | longhand | visible/hidden |

---

## 20. Transition

| Property | Type | Notes |
|----------|------|-------|
| transition | `[S]` | Shorthand |
| transition-property | longhand | Which properties to transition |
| transition-duration | longhand | How long |
| transition-timing-function | longhand | Easing function |
| transition-delay | longhand | Delay before start |
| transition-behavior | longhand | normal/allow-discrete |

---

## 21. Animation

| Property | Type | Notes |
|----------|------|-------|
| animation | `[S]` | Shorthand |
| animation-name | longhand | @keyframes name |
| animation-duration | longhand | How long |
| animation-timing-function | longhand | Easing function |
| animation-delay | longhand | Delay before start |
| animation-iteration-count | longhand | Number or infinite |
| animation-direction | longhand | normal/reverse/alternate/alternate-reverse |
| animation-fill-mode | longhand | none/forwards/backwards/both |
| animation-play-state | longhand | running/paused |
| animation-composition | longhand | replace/add/accumulate |
| animation-timeline | longhand | auto/none/scroll()/view()/named |
| animation-range | `[S]` | Shorthand for start + end |
| animation-range-start | longhand | Start of animation attachment range |
| animation-range-end | longhand | End of animation attachment range |

### Scroll-Driven Animations (Timelines)

| Property | Type | Notes |
|----------|------|-------|
| scroll-timeline | `[S]` | Shorthand for name + axis |
| scroll-timeline-name | longhand | Named scroll timeline |
| scroll-timeline-axis | longhand | block/inline/x/y |
| view-timeline | `[S]` | Shorthand for name + axis |
| view-timeline-name | longhand | Named view timeline |
| view-timeline-axis | longhand | block/inline/x/y |
| view-timeline-inset | longhand | Inset for view progress |
| timeline-scope | longhand | Extend timeline scope to ancestors |

---

## 22. Filter & Blend

| Property | Type | Notes |
|----------|------|-------|
| filter | longhand | blur/brightness/contrast/grayscale/hue-rotate/invert/opacity/saturate/sepia/drop-shadow/url |
| backdrop-filter | longhand | Same functions applied to backdrop |
| mix-blend-mode | longhand | normal/multiply/screen/overlay/darken/lighten/... |
| isolation | longhand | auto/isolate (new stacking context) |

---

## 23. Clipping & Masking

| Property | Type | Notes |
|----------|------|-------|
| clip-path | longhand | Shape/URL/none |
| clip-rule | longhand | nonzero/evenodd |
| mask | `[S]` | Shorthand |
| mask-image | longhand | URL/gradient/none |
| mask-mode | longhand | alpha/luminance/match-source |
| mask-position | longhand | Position of mask layer |
| mask-size | longhand | Size of mask layer |
| mask-repeat | longhand | Repeat mask layer |
| mask-origin | longhand | Reference box for position |
| mask-clip | longhand | Painting area |
| mask-composite | longhand | add/subtract/intersect/exclude |
| mask-type | longhand | luminance/alpha (for SVG mask) |
| mask-border | `[S]` | Shorthand |
| mask-border-source | longhand | |
| mask-border-slice | longhand | |
| mask-border-width | longhand | |
| mask-border-outset | longhand | |
| mask-border-repeat | longhand | |
| mask-border-mode | longhand | |

---

## 24. Shape

| Property | Type | Notes |
|----------|------|-------|
| shape-outside | longhand | Float shape: circle/ellipse/polygon/inset/url |
| shape-margin | longhand | Margin around float shape |
| shape-image-threshold | longhand | Alpha threshold for image shapes |
| shape-rendering | longhand | auto/optimizeSpeed/crispEdges/geometricPrecision (SVG) |

---

## 25. Object Fit & Position (Replaced Elements)

| Property | Type | Notes |
|----------|------|-------|
| object-fit | longhand | fill/contain/cover/none/scale-down |
| object-position | longhand | Position of replaced content |
| object-view-box | longhand | Inset to adjust view of object |
| image-orientation | longhand | from-image/none |
| image-rendering | longhand | auto/smooth/high-quality/pixelated/crisp-edges |
| image-resolution | longhand | from-image/DPI |

---

## 26. Table

| Property | Type | Notes |
|----------|------|-------|
| table-layout | longhand | auto/fixed |
| caption-side | longhand | top/bottom |
| border-collapse | longhand | collapse/separate |
| border-spacing | longhand | Horizontal and vertical spacing |
| empty-cells | longhand | show/hide |
| vertical-align | longhand | baseline/sub/super/top/middle/bottom/length |

---

## 27. Containment

| Property | Type | Notes |
|----------|------|-------|
| contain | longhand | none/strict/content/size/layout/style/paint |
| container | `[S]` | Shorthand for name + type |
| container-name | longhand | Names for @container queries |
| container-type | longhand | normal/size/inline-size |

---

## 28. Fragmentation (Print / Multi-Column)

| Property | Type | Notes |
|----------|------|-------|
| break-before | longhand | auto/avoid/always/page/column/region |
| break-after | longhand | |
| break-inside | longhand | auto/avoid/avoid-page/avoid-column |
| orphans | longhand | Min lines at bottom of page |
| widows | longhand | Min lines at top of page |
| page | longhand | Named page for @page |
| box-decoration-break | longhand | slice/clone |

---

## 29. Motion Path

| Property | Type | Notes |
|----------|------|-------|
| offset | `[S]` | Shorthand |
| offset-path | longhand | path()/ray()/url/shape |
| offset-distance | longhand | How far along path |
| offset-rotate | longhand | auto/angle |
| offset-anchor | longhand | Point within element |
| offset-position | longhand | Starting position |

---

## 30. Pointer & Interaction

| Property | Type | Notes |
|----------|------|-------|
| cursor | longhand | auto/default/pointer/text/move/... |
| pointer-events | longhand | auto/none/visiblePainted/... |
| touch-action | longhand | auto/none/pan-x/pan-y/manipulation/pinch-zoom |
| user-select | longhand | auto/none/text/all/contain |
| resize | longhand | none/both/horizontal/vertical/block/inline |
| will-change | longhand | auto/property-list |
| interactivity | longhand | auto/inert |

---

## 31. SVG Presentation Properties

These CSS properties apply to SVG elements:

| Property | Type | Notes |
|----------|------|-------|
| d | longhand | SVG path data |
| x | longhand | SVG x position |
| y | longhand | SVG y position |
| r | longhand | SVG circle radius |
| rx | longhand | SVG ellipse/rect x-radius |
| ry | longhand | SVG ellipse/rect y-radius |
| cx | longhand | SVG circle/ellipse center x |
| cy | longhand | SVG circle/ellipse center y |
| fill | longhand | Fill paint |
| fill-opacity | longhand | |
| fill-rule | longhand | nonzero/evenodd |
| stroke | longhand | Stroke paint |
| stroke-width | longhand | |
| stroke-dasharray | longhand | Dash pattern |
| stroke-dashoffset | longhand | Offset into dash pattern |
| stroke-linecap | longhand | butt/round/square |
| stroke-linejoin | longhand | miter/round/bevel |
| stroke-miterlimit | longhand | |
| stroke-opacity | longhand | |
| paint-order | longhand | fill/stroke/markers order |
| marker | `[S]` | Shorthand for start/mid/end |
| marker-start | longhand | |
| marker-mid | longhand | |
| marker-end | longhand | |
| alignment-baseline | longhand | auto/baseline/before-edge/text-before-edge/... |
| dominant-baseline | longhand | auto/ideographic/alphabetic/hanging/... |
| text-anchor | longhand | start/middle/end |
| color-interpolation | longhand | auto/sRGB/linearRGB |
| color-interpolation-filters | longhand | auto/sRGB/linearRGB |
| flood-color | longhand | SVG filter primitive |
| flood-opacity | longhand | SVG filter primitive |
| lighting-color | longhand | SVG filter primitive |
| stop-color | longhand | SVG gradient stop |
| stop-opacity | longhand | SVG gradient stop |
| vector-effect | longhand | none/non-scaling-stroke/non-scaling-size/... |
| baseline-source | longhand | auto/first/last |

---

## 32. View Transitions

| Property | Type | Notes |
|----------|------|-------|
| view-transition-name | longhand | Names element for view transition |
| view-transition-class | longhand | Groups view transition elements |

---

## 33. Reading Flow

| Property | Type | Notes |
|----------|------|-------|
| reading-flow | longhand | normal/flex-visual/flex-flow/grid-rows/grid-columns |
| reading-order | longhand | Override reading order of element |

---

## 34. Ruby

| Property | Type | Notes |
|----------|------|-------|
| ruby-position | longhand | over/under |
| ruby-align | longhand | start/center/space-between/space-around |
| ruby-overhang | longhand | auto/none |

---

## 35. Misc Properties

| Property | Type | Notes |
|----------|------|-------|
| all | longhand | Resets all properties |
| zoom | longhand | Zoom factor (legacy, now standard) |
| initial-letter | longhand | Drop cap size |
| line-height-step | longhand | Snap line-height to grid |
| line-clamp | longhand | Clamp to N visible lines |
| math-style | longhand | normal/compact (MathML) |
| math-depth | longhand | Script level (MathML) |
| math-shift | longhand | normal/compact (MathML) |
| speak-as | longhand | Speech synthesis |
| dynamic-range-limit | longhand | HDR color range limiting |

---

## 36. -webkit- Prefixed Properties (Widely Supported)

These are non-standard but widely supported in modern browsers:

| Property | Standard Equivalent | Notes |
|----------|-------------------|-------|
| -webkit-text-fill-color | (none) | Text fill color (overrides color) |
| -webkit-text-stroke | (none) `[S]` | Shorthand for stroke width + color |
| -webkit-text-stroke-width | (none) | Width of text stroke |
| -webkit-text-stroke-color | (none) | Color of text stroke |
| -webkit-line-clamp | line-clamp | Maximum visible lines |
| -webkit-background-clip | background-clip | (text value) |
| -webkit-text-security | (none) | disc/circle/square/none (password masking) |
| -webkit-appearance | appearance | |
| -webkit-tap-highlight-color | (none) | Touch highlight color |
| -webkit-overflow-scrolling | (none) | touch/auto (iOS momentum scroll) |
| -webkit-font-smoothing | (none) | none/antialiased/subpixel-antialiased |
| -webkit-backface-visibility | backface-visibility | |
| -webkit-transform | transform | |
| -webkit-transform-origin | transform-origin | |
| -webkit-transition | transition | |
| -webkit-animation | animation | |
| -webkit-filter | filter | |
| -webkit-backdrop-filter | backdrop-filter | |
| -webkit-mask | mask | |
| -webkit-mask-image | mask-image | |
| -webkit-mask-position | mask-position | |
| -webkit-mask-size | mask-size | |
| -webkit-mask-repeat | mask-repeat | |
| -webkit-mask-origin | mask-origin | |
| -webkit-mask-clip | mask-clip | |
| -webkit-mask-composite | mask-composite | |
| -webkit-user-select | user-select | |
| -webkit-box-reflect | (none) | Reflection below/above/left/right |
| -webkit-print-color-adjust | print-color-adjust | |
| -webkit-text-size-adjust | text-size-adjust | |

---

## Summary — Pure Longhand Count by Category

| Category | Longhand Count |
|----------|---------------|
| Display & Box Generation | 5 |
| Positioning (incl. Anchor) | 20 |
| Box Sizing | 21 |
| Margin | 10 |
| Padding | 10 |
| Border (width/style/color) | 36 |
| Border Radius | 8 |
| Border Image | 5 |
| Border Misc (collapse/spacing) | 2 |
| Background | 11 |
| Box Decoration (shadow/outline/opacity) | 6 |
| Flexbox | 7 |
| Grid | 11 |
| Alignment | 8 |
| Multi-Column | 7 |
| Font | 23 |
| Text & Typography | 45 |
| Writing Modes | 3 |
| Lists & Counters | 7 |
| Color & Appearance | 8 |
| Overflow & Scrolling | 35 |
| Transform | 10 |
| Transition | 5 |
| Animation | 12 |
| Scroll Timelines | 7 |
| Filter & Blend | 4 |
| Clipping & Masking | 17 |
| Shape | 4 |
| Object/Image | 5 |
| Table | 5 |
| Containment | 3 |
| Fragmentation | 6 |
| Motion Path | 5 |
| Pointer & Interaction | 7 |
| SVG Presentation | 30 |
| View Transitions | 2 |
| Reading Flow | 2 |
| Ruby | 3 |
| Misc | 10 |
| **TOTAL (standard longhands)** | **~370** |
| -webkit- prefixed | ~30 |
| **GRAND TOTAL** | **~400** |

---

*This list represents the CSS longhand properties supported by modern browsers as of early 2026. It was compiled from the MDN CSS Reference index, the W3C complete CSS properties list (741 entries including duplicates across specs), and filtered to those actually implemented in modern browsers.*
