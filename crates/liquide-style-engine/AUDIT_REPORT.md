# liquide-style-engine — Comprehensive Audit Report

**Scope:** All 13 source files in `crates/liquide-style-engine/src/`
**Total lines:** ~13,900

---

## 1. TODO / FIXME / HACK / Stub Comments

| # | File | Line | Description |
|---|------|------|-------------|
| 1 | engine.rs | ~4225 | `// Overrides the stub that only handled "auto"` — comment on `scrollbar-color` handler |
| 2 | engine.rs | ~5241 | `// consumed here so they are no longer stub-only` — mask longhands position/size/repeat/origin/clip/composite are read but not carried through `MaskSpec` |
| 3 | engine.rs | ~5943 | `// Counters: counter(name) / counters(name, sep) → placeholder` — `evaluate_content_value()` emits `[counter:name]` placeholder strings instead of resolving counters |
| 4 | engine.rs | ~6020 | `// Store as placeholder — layout will resolve against DOM` — `attr()` in `content` emits `[attr:name]` placeholder strings |
| 5 | shorthand.rs | ~788 | `// Transition is complex — just pass through for now; the engine handles it` — `expand_transition()` is a stub |
| 6 | shorthand.rs | ~793 | `// (no comment but body is stub)` — `expand_animation()` passes the whole value as `animation-name` only |
| 7 | selector.rs | ~878 | `// For simplicity, wrap first selector; full impl would use all` — `:not()` discards all selectors after the first in a comma-separated list |
| 8 | engine.rs | ~4253 | `// unset: inherited properties → inherit, non-inherited → initial / For simplicity, treat as initial` — `all: unset` and `all: revert` both reset to `default()`, ignoring inherited values |
| 9 | engine.rs | ~963 | `// Revert to the previous cascade origin's value / For now, simplified: act like unset` — `revert` / `revert-layer` keyword simplified to `unset` behavior |
| 10 | engine.rs | ~5260 | `// struct doesn't carry position/size/repeat/origin/clip/composite yet` — `MaskSpec` assembly discards mask-position/size/repeat/origin/clip/composite longhands |

---

## 2. Functions Returning Hardcoded / Dummy Data

| # | File | Line | Function / Location | Description |
|---|------|------|---------------------|-------------|
| 1 | engine.rs | ~5491 | `resolve_env_variable()` | All `env()` variables return hardcoded `"0px"` or `"100%"`. Safe-area insets, titlebar-area, keyboard insets are never read from the actual platform. |
| 2 | engine.rs | ~5753 | `evaluate_media_feature()` for `prefers-color-scheme` | Always matches `"light"` only — dark mode preference is hardcoded. |
| 3 | engine.rs | ~5714 | `parse_filter_px()` for `em`/`rem` | Hardcoded `* 16.0` approximation — ignores actual `base_font_size` / parent font size. |
| 4 | value_resolve.rs | ~114–128 | `length_unit_to_dimension()` | `Ex` approximated as `0.5 * em`; `Ch` approximated as `0.5 * font_size` — uses rough typographic estimates, not actual glyph metrics. |
| 5 | value_resolve.rs | ~130–133 | `length_unit_to_dimension()` | `Lh` approximated as `1.2 * em`, `Rlh` as `1.2 * 16.0` — ignores actual line-height. |
| 6 | value_resolve.rs | ~137–140 | `length_unit_to_dimension()` | Container query units (`Cqw`, `Cqh`, etc.) approximated as percentage — no actual container size lookup. |
| 7 | value_resolve.rs | ~142–145 | `length_unit_to_dimension()` | Dynamic viewport units (`Dvw`, `Dvh`, `Svw`, `Lvw`, etc.) all map to the static `Vw`/`Vh` — no distinction between dynamic/small/large viewports. |
| 8 | engine.rs | ~4141–4145 | `font` shorthand system fonts | System font keywords (`caption`, `icon`, `menu`, etc.) hardcode `font_size = 14.0` and `font_family = ["sans-serif"]` — no platform font lookup. |
| 9 | engine.rs | ~5263 | `assemble_mask()` — `image_id` | `mask-image` is parsed as `img.parse::<u64>().unwrap_or(0)` — always 0 for non-numeric mask-image references (e.g., `url(...)`, gradients). |
| 10 | engine.rs | ~5945 | `evaluate_content_value()` — counter() | Returns `[counter:name]` literal string — never resolves CSS counter state. |
| 11 | engine.rs | ~6008 | `evaluate_content_value()` — attr() | Returns `[attr:name]` literal string — never resolves the actual element attribute value. |

---

## 3. Incomplete Match Arms / `unimplemented!()` / `todo!()`

| # | File | Line | Description |
|---|------|------|-------------|
| 1 | selector.rs | ~878 | `:not(A, B, C)` — only the first selector `A` is used; per Selectors Level 4, all must be tested. |
| 2 | engine.rs | ~4686 | `list-style-image` — silently discards `url()` values; only recognizes `"none"` keyword. |
| 3 | engine.rs | ~4005 | `list-style` shorthand — no handling of `url()` for `list-style-image`. Comment: `// Could be a url() for list-style-image or custom counter style`. |
| 4 | engine.rs | ~4787 | `speak` — empty handler; value is never stored. |
| 5 | engine.rs | ~4790–4793 | `position-try-fallbacks`, `position-visibility` — empty handler; values are never stored. |
| 6 | engine.rs | ~4796–4798 | `animation-range`, `animation-range-start`, `animation-range-end` — empty handler; values are never stored. |
| 7 | engine.rs | ~4801 | `baseline-shift` — empty handler; value is never stored. |
| 8 | engine.rs | ~4805 | Catch-all `_ => {}` — unknown properties are silently ignored with no logging/diagnostic. |
| 9 | value_resolve.rs | ~207–216 | `resolve_color()` — only handles `PropertyValue::Color(c)`. A `Keyword("red")` or `String("#ff0000")` or `String("rgb(255,0,0)")` returns `None`. |
| 10 | value_resolve.rs | ~45–76 | `try_parse_color()` — only parses hex colors (#rgb, #rrggbb, #rrggbbaa). Missing: `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`, `oklch()`, `oklab()`, named colors (`red`, `blue`, etc.), `transparent`, `currentcolor`. |
| 11 | cascade.rs | ~268–275 | `strip_important()` — only detects `!important` in `PropertyValue::Keyword` variant. A `PropertyValue::String("bold !important")` or other variants will not have `!important` stripped. |
| 12 | engine.rs | ~4253 | `all: unset` / `all: revert` — both execute `*style = ComputedStyle::default()`, not correctly propagating inherited values for inherited properties. |
| 13 | shorthand.rs | ~189–199 | `font-variant` shorthand expansion — broadcasts a single keyword to all sub-properties instead of parsing the multi-value shorthand per spec. |
| 14 | shorthand.rs | ~326–340 | `border-image` shorthand — only handles `"none"` or stores entire value as `border-image-source`; slices/width/outset/repeat sub-properties are never decomposed. |
| 15 | shorthand.rs | ~348–360 | `mask` shorthand — only handles `"none"` or stores entire value as `mask-image`; other sub-properties are never decomposed. |
| 16 | shorthand.rs | ~268–280 | `offset` shorthand — stores entire value as `offset-path`; distance/rotate/anchor/position are never decomposed. |
| 17 | engine.rs | ~4445–4452 | `border-block` shorthand — `groove`, `ridge`, `inset`, `outset` all mapped to `Solid` instead of their correct `BorderLineStyle` variants. |
| 18 | engine.rs | ~4526 | `border-inline` shorthand — same `groove/ridge/inset/outset → Solid` issue. |

---

## 4. CSS Properties Declared but Never Resolved / Applied

| # | File | Property | Description |
|---|------|----------|-------------|
| 1 | engine.rs:6055–6360 | Multiple | `consume_remaining_properties()` binds ~80 fields to `let _` — documenting dead fields. While comments claim downstream consumers exist, these fields are never transformed by the style engine itself. Key groups: |
| 2 | computed.rs | `speak` | Declared nowhere in `ComputedStyle` — the `speak` handler in engine.rs is empty. |
| 3 | computed.rs | `position_try_fallbacks`, `position_visibility` | No fields exist — handlers are empty. |
| 4 | computed.rs | `animation_range*` | No fields for `animation-range-start`/`animation-range-end` — handlers are empty. |
| 5 | computed.rs | `baseline_shift` | No field — handler is empty. |
| 6 | computed.rs | `list_style_image` | No field — `list-style-image` handler is a no-op. |
| 7 | engine.rs | mask-position/size/repeat/origin/clip/composite | Stored in `ComputedStyle` but `assemble_mask()` reads them as `let _` bindings without using them in the `MaskSpec` output. |
| 8 | engine.rs | `border-image-slice`, `border-image-width`, `border-image-outset`, `border-image-repeat` | Stored in `ComputedStyle` but `border-image` shorthand never decomposes into them; only `border-image-source` is populated. |
| 9 | engine.rs | `offset-distance`, `offset-rotate`, `offset-anchor`, `offset-position` | Stored in `ComputedStyle` but `offset` shorthand never decomposes into them; only `offset-path` is populated. |

---

## 5. Cascade / Specificity Gaps

| # | File | Line | Description |
|---|------|------|-------------|
| 1 | engine.rs | ~963–967 | **`revert` / `revert-layer` not implemented** — both fall back to `unset` behavior. Per CSS Cascading Level 5, `revert` should roll back to the previous origin and `revert-layer` to the previous cascade layer. |
| 2 | engine.rs | ~4247–4259 | **`all: revert` treated as `all: initial`** — `*style = ComputedStyle::default()` nukes inherited values; correct behavior for `all: unset` requires `inherit_from(parent)` for inherited properties. |
| 3 | cascade.rs | ~268–275 | **`strip_important()` only inspects `Keyword` variant** — `!important` on color, length, or string values is not detected, meaning those declarations won't participate in the important cascade level correctly. |
| 4 | engine.rs | (structural) | **No `@layer` conflict resolution between competing layers** — `layer_order` is tracked on `PreparedRule` but if two rules in different layers match with the same specificity, the engine relies only on `source_order` rather than the explicit layer ordering from `@layer`. `CascadePriority` does include `layer_order`, but it's always set to the global insertion order, not the `@layer` declaration order. |
| 5 | engine.rs | (structural) | **`:host` / `::slotted` styles don't participate in a separate shadow-origin cascade** — shadow DOM styles are scope-filtered but not placed in a distinct cascade origin as specified by CSS Scoping. |
| 6 | selector.rs | ~878 | **`:not()` selector list truncated** — discarding selectors 2..N from `:not(A, B)` can produce false matches; specificity is also computed from only the first selector rather than the most specific. |
| 7 | selector.rs | ~294 | **`:not()` specificity** — only uses the specificity of the first inner selector, not the maximum of all selectors in the list as specified by Selectors Level 4. |

---

## 6. Missing CSS Features

### 6a. Color Functions
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | value_resolve.rs | ~45–76 | **No `rgb()` / `rgba()` parsing** — only hex colors supported. |
| 2 | value_resolve.rs | ~45–76 | **No `hsl()` / `hsla()` parsing.** |
| 3 | value_resolve.rs | ~45–76 | **No `hwb()`, `oklch()`, `oklab()`, `lab()`, `lch()` color parsing** — CSS Color Level 4/5 functions missing. |
| 4 | value_resolve.rs | ~45–76 | **No named CSS colors** — `red`, `blue`, `transparent`, `currentColor` etc. not recognized by `try_parse_color()`. `resolve_color()` only handles `PropertyValue::Color`, so inline keyword colors in strings (e.g., `"red"`) are lost. |
| 5 | value_resolve.rs | ~207–216 | **`resolve_color()` doesn't delegate to `try_parse_color()`** for `Keyword` or `String` variants — only `PropertyValue::Color` is handled. |
| 6 | (entire crate) | — | **No `color-mix()` function support.** |
| 7 | (entire crate) | — | **No `color()` function support** (display-p3, srgb, etc.). |

### 6b. CSS Functions & Values
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | engine.rs | ~5430ff | **`env()` values are all hardcoded** — no platform integration for safe-area insets, viewport segments, etc. |
| 2 | value_resolve.rs | ~130–145 | **Container query units (`cqw`, `cqh`, etc.) approximated as percentage** — no actual container size resolution. |
| 3 | value_resolve.rs | ~142–145 | **Dynamic viewport units not distinguished** — `dvw`/`svw`/`lvw` all mapped to `vw`. |
| 4 | (entire crate) | — | **No `image-set()` function support.** |
| 5 | (entire crate) | — | **No `cross-fade()` function support.** |
| 6 | (entire crate) | — | **No `element()` function support.** |
| 7 | engine.rs | ~5945 | **CSS `counter()` / `counters()` not resolved** — output is placeholder string. |
| 8 | engine.rs | ~6008 | **CSS `attr()` not resolved** — output is placeholder string. |

### 6c. Selectors
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | selector.rs | ~878 | **`:not()` selector list only uses first selector.** |
| 2 | selector.rs | (structural) | **`:has()` only supports a single selector argument** — no comma-separated selector list. |
| 3 | selector.rs | (structural) | **No `:host-context()` pseudo-class.** |
| 4 | selector.rs | (structural) | **No `::part()` pseudo-element.** |
| 5 | selector.rs | (structural) | **No `::slotted()` pseudo-element** (only `:host` pseudo-class is partially supported). |
| 6 | selector.rs | (structural) | **No `::marker` pseudo-element.** |
| 7 | selector.rs | (structural) | **No `::backdrop` pseudo-element.** |
| 8 | selector.rs | (structural) | **No `::cue` / `::cue-region` pseudo-elements.** |
| 9 | selector.rs | (structural) | **No `::file-selector-button` pseudo-element.** |
| 10 | selector.rs | (structural) | **No `:dir()` pseudo-class.** |
| 11 | selector.rs | (structural) | **No `:is()` / `:where()` specificity correctly handles forgiving selector lists** — parsing failures silently produce empty vectors. |
| 12 | selector.rs | (structural) | **No `:defined`, `:any-link`, `:local-link`, `:target-within`, `:scope`, `:current`, `:past`, `:future` pseudo-classes.** |
| 13 | selector.rs | (structural) | **No `:playing` / `:paused` / `:seeking` / `:buffering` / `:stalled` media pseudo-classes.** |
| 14 | selector.rs | (structural) | **No `:modal`, `:fullscreen`, `:picture-in-picture`, `:autofill`, `:user-valid`, `:user-invalid` pseudo-classes.** |
| 15 | selector.rs | (structural) | **No `::highlight()` / `::spelling-error` / `::grammar-error` pseudo-elements.** |
| 16 | selector.rs | (structural) | **No namespace selectors** (`ns|element`). |

### 6d. At-Rules
| # | File | Description |
|---|------|-------------|
| 1 | engine.rs | **`@counter-style` not supported** — custom counter styles cannot be defined. |
| 2 | engine.rs | **`@page` / `@page` margin at-rules not supported.** |
| 3 | engine.rs | **`@namespace` not supported.** |
| 4 | engine.rs | **`@charset` not handled (but typically unnecessary).** |
| 5 | engine.rs | **`@scope` (CSS Cascading Level 6) not supported.** |
| 6 | engine.rs | **`@starting-style` not supported.** |
| 7 | engine.rs | **`@position-try` not supported** — anchor positioning fallbacks. |
| 8 | engine.rs | **`@view-transition` not supported.** |

### 6e. Media Query Features
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | engine.rs | ~5730–5760 | **Only 5 media features supported**: `min-width`, `max-width`, `min-height`, `max-height`, `prefers-color-scheme`. Missing: `orientation`, `aspect-ratio`, `resolution`, `color`, `color-gamut`, `pointer`, `hover`, `prefers-reduced-motion`, `prefers-contrast`, `forced-colors`, `scripting`, `prefers-reduced-data`, `prefers-reduced-transparency`, `display-mode`, `dynamic-range`, `video-dynamic-range`, `update`, `overflow-block/inline`. |
| 2 | engine.rs | ~5753 | **`prefers-color-scheme` always returns `"light"`** — no dark mode support. |
| 3 | engine.rs | ~5740 | **`or` combinator in media queries not supported** — only `and` and comma (or-list) are handled. |
| 4 | engine.rs | (structural) | **Range syntax for media queries not supported** — `(width > 768px)` / `(400px <= width <= 1200px)` not handled; only prefix `min-`/`max-` form. |

### 6f. Shorthand Decomposition
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | shorthand.rs | ~788 | **`transition` shorthand stub** — entire value passed as `transition-property`. Duration, timing-function, delay are not decomposed. |
| 2 | shorthand.rs | ~793 | **`animation` shorthand stub** — entire value passed as `animation-name`. Duration, timing-function, delay, iteration-count, direction, fill-mode, play-state are not decomposed. |
| 3 | shorthand.rs | ~728 | **`background` shorthand incomplete** — only color/gradient/none/transparent extracted; position, size, repeat, attachment, origin, clip, and multiple backgrounds not handled. |
| 4 | shorthand.rs | ~741 | **`font` shorthand path in `expand_shorthand`** is separate from the comprehensive parser in `engine.rs:apply_single_property` — the shorthand expander just passes the whole value as `font-size`, while the engine has a better parser. This dual-path risks inconsistency. |
| 5 | shorthand.rs | ~326 | **`border-image` shorthand** — only `"none"` or full value as `border-image-source`. |
| 6 | shorthand.rs | ~348 | **`mask` shorthand** — only `"none"` or full value as `mask-image`. |
| 7 | shorthand.rs | ~268 | **`offset` shorthand** — stores everything as `offset-path`. |

### 6g. Shadow DOM
| # | File | Line | Description |
|---|------|------|-------------|
| 1 | shadow_dom.rs | ~120–148 | **`property_inherits_across_boundary()` list diverges from `inheritance.rs`** — missing: `tab-size`, `hyphens`, `overflow-wrap`, `word-break`, `cursor`, `caret-color`, `image-rendering`, `text-rendering`, `text-underline-position`, `text-decoration-skip-ink`, `font-kerning`, `font-optical-sizing`, `font-feature-settings`, `font-variation-settings`, `pointer-events`, `color-scheme`, `forced-color-adjust`, `print-color-adjust`, `paint-order`, and all SVG inherited properties. |
| 2 | shadow_dom.rs | (structural) | **No `::part()` support** — `:host` is partially supported but exported parts cannot be styled. |
| 3 | shadow_dom.rs | (structural) | **No `::slotted()` CSS selector support** — `slotted_children()` helper exists but there's no corresponding pseudo-element in the selector engine. |

### 6h. Other Notable Gaps
| # | File | Description |
|---|------|-------------|
| 1 | engine.rs | **`@supports` selector function (`selector()`)** — `evaluate_supports_condition()` handles property checks but not `selector()` queries. |
| 2 | engine.rs | **Logical property shorthand two-value syntax** — `margin-inline: 10px 20px` (two-value) is not parsed; only single values. |
| 3 | engine.rs | **Multiple backgrounds** — only one background layer is supported. |
| 4 | engine.rs | **Multiple box-shadows** — `box-shadow` is stored as a single `BoxShadow` optional, not a list. |
| 5 | engine.rs | **Nesting (`&` selector)** — CSS Nesting Module not supported. |
| 6 | computed.rs | **`initial-letter`** — field exists but no known property handler in `apply_single_property()`. |
| 7 | engine.rs | **No `!important` on non-Keyword values** — per cascade.rs limitation, important declarations on Color/Length/String property values are not detected. |
| 8 | engine.rs | **No `@keyframes` at-rule processing** — animation keyframes are never parsed or stored. |
| 9 | engine.rs | **`currentColor` keyword not resolved** — when used as a color value in properties like `border-color`, `text-decoration-color`, `box-shadow`, it's not resolved to the element's computed `color`. |
| 10 | value_resolve.rs | **No relative length resolution against parent** — `em` units are converted to `px` at parse time, not at layout time relative to the parent's computed font-size. |

---

## Summary

| Category | Count |
|----------|-------|
| Stub / TODO / HACK comments | 10 |
| Hardcoded / dummy data | 11 |
| Incomplete match arms / missing handlers | 18 |
| Properties declared but never resolved | 9 groups |
| Cascade / specificity gaps | 7 |
| Missing CSS features | ~55+ individual items |

**Overall assessment:** The engine provides broad coverage (~200+ CSS properties, cascade levels, specificity, inheritance, variables, calc, container queries, shadow DOM) but has significant depth gaps in color parsing (hex-only), shorthand decomposition (`transition`/`animation`/`mask`/`border-image`/`offset` are stubs), media query features (5 of ~30+), `revert`/`revert-layer` semantics, selector coverage (missing many Level 4 pseudo-classes/elements), and `counter()`/`attr()` resolution (placeholder-only).
