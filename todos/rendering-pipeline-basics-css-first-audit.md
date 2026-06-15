# Rendering Pipeline Basics and CSS-First Shell Audit

Date: 2026-06-15

Scope:
- Review only. No source files were changed during this pass.
- First priority: why the shell feels janky, jumbled, or flickery.
- Target architecture: shell visuals should be driven by DOM, CSS, computed style, layout, paint, and scene bridge data, with Rust supplying state and platform surfaces.
- Focus areas: shell scene assembly, CSS pipeline ordering, style/layout invalidation, scene identity, visual effects, damage, blur, and renderer performance basics.

Severity legend:
- Critical: breaks CSS-first ownership or can directly produce stale, missing, or visibly wrong frames.
- High: major jank, flicker, performance, or completeness risk under ordinary shell use.
- Medium: important correctness/completeness gap that will block advanced CSS or predictable performance.
- Low: cleanup or future risk; not the first blocker.

## Pipeline Map

Current visible path:
1. Shell state is synchronized into DOM/templates from `Shell::build_scene`.
2. `DesktopPipeline` runs style, layout, paint, and `display_list_to_scene`.
3. `Shell::build_scene` mixes CSS pipeline nodes with manual Rust-built workspace, windows, dialogs, lockscreen, overview, loading, and fallbacks.
4. The compositor flattens/sorts scene nodes.
5. The session render thread builds damage, renders with the CPU renderer, and presents tiles/framebuffer output.

Target path:
1. Shell state updates DOM attributes, classes, custom properties, and semantic data.
2. One authoritative CSS engine computes all shell chrome, overlays, window decorations, and shell app placeholders.
3. Layout/paint/property trees preserve CSS semantics through the scene bridge.
4. Stable scene identities and precise damage feed renderer caches.
5. Renderer quality degrades gracefully without visual popping or stale effects.

## Immediate Priority Order

- [ ] P0: Move shell windows, overlays, and placeholder app content into DOM/CSS.
- [ ] P0: Fix CSS animation/transition pipeline ordering and repaint invalidation.
- [ ] P0: Make scene node identity stable across frames.
- [ ] P0: Preserve scoped CSS effects instead of flattening filters/masks/blends as standalone nodes.
- [ ] P1: Add a container-query feedback loop.
- [ ] P1: Make scene building pure: no wall-clock mutation or manager ticking during render.
- [ ] P1: Replace broad DOM/template churn with keyed reconciliation.
- [ ] P1: Unify damage, blur cache validation, and adaptive renderer timing.

## Critical

### TODO 1: Make windows DOM/CSS-driven

Finding:
Windows are the largest manual exception to CSS-first rendering. The code builds workspace, shadows, titlebars, controls, content, and placeholder apps directly in Rust.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:347` documents windows as the manual exception.
- `crates/liquide-shell/src/shell/scene.rs:465` injects the manual workspace.
- `crates/liquide-shell/src/shell/scene.rs:704` through `:889` manually builds windows, shadows, titlebars, controls, and content.
- `crates/liquide-shell/src/shell/scene.rs:921` through `:1138` hardcodes placeholder app content for Settings, Terminal, Files, Browser, and Calculator.
- `crates/liquide-shell/src/shell/windows.rs:597` notes focused class sync is not done because windows bypass CSS.

Impact:
The shell cannot be 100% CSS-driven while the most visible surface ignores DOM structure, CSS selectors, layout, pseudo states, media/container rules, and theme variables. It also creates duplicate ownership for window focus, geometry, decoration state, and app content.

Remediation:
- [ ] Represent windows under a DOM `workspace-container` or equivalent layer.
- [ ] Sync window state as attributes/classes/custom properties: focused, minimized, maximized, dragging, resizing, app id, title, geometry, workspace, z-order.
- [ ] Render titlebars, controls, shadows, focus rings, placeholders, and empty app states through templates and CSS.
- [ ] Reserve manual scene nodes only for real app surface embedding if the app content buffer cannot yet be DOM/CSS.
- [ ] Add tests that a focused window produces CSS class/attribute state and that selector-based window styling reaches the scene.

### TODO 2: Move dialogs, lockscreen, and overview into DOM/CSS overlays

Finding:
Dialog, lockscreen, and overview surfaces still bypass templates/CSS and are painted from fixed Rust scene nodes.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:474` through `:502` appends manual overlay surfaces.
- `crates/liquide-shell/src/shell/scene.rs:516` builds dialog overlay geometry and controls in Rust.
- `crates/liquide-shell/src/shell/scene.rs:600` builds lockscreen overlay.
- `crates/liquide-shell/src/shell/scene.rs:641` builds overview overlay.
- Existing assets already point the other way: `assets/templates/components/dialog.html:17` and `assets/themes/components.css:244`.

Impact:
Critical shell states do not share the same CSS capabilities as normal chrome. This guarantees visual drift, missing theme behavior, and awkward transitions between DOM/CSS surfaces and manual overlays.

Remediation:
- [ ] Model dialog, lockscreen, overview, and loading as DOM overlay roots.
- [ ] Move layout, colors, glass, transitions, and button visuals to CSS.
- [ ] Keep Rust responsible only for state, focus routing, input actions, and secure lockscreen policy.
- [ ] Add visual wiring tests proving overlay styles come from CSS selectors.

### TODO 3: Fix animation and transition invalidation

Finding:
The pipeline ticks animations/transitions after layout, then decides whether to repaint without considering `animations_active`.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:193` through `:242` computes layout before animation values are applied.
- `crates/liquide-shell/src/pipeline/stages.rs:260` through `:276` ticks transitions/animations and mutates styles.
- `crates/liquide-shell/src/pipeline/stages.rs:278` through `:292` sets `recompute_paint = recompute_layout || has_paint_work || self.last_display_list.is_none()`, without `animations_active`.
- `crates/liquide-shell/src/pipeline/animation_bridge.rs:109` through `:164` marks layout-affecting properties as transitionable, including width, height, margin, padding, top, left, flex-basis, gap, font-size, and line-height.

Impact:
Paint-only animations can update computed styles but reuse an old display list, so the visible frame can stick until unrelated dirtiness forces repaint. Layout-affecting animations are applied after layout, so the same frame can use stale geometry. This is a direct flicker/jank source and blocks advanced CSS motion.

Remediation:
- [ ] Classify animated property impact as layout, paint, or compositor-only.
- [ ] Recompute paint whenever paint-affecting animations are active.
- [ ] Rerun layout when layout-affecting animations are active, or restrict active shell animations to compositor-safe properties until this is fixed.
- [ ] Prefer property-tree/compositor updates for opacity, transform, and filter where possible.
- [ ] Add frame-step tests for transition interpolation reaching display output.

### TODO 4: Make CSS scene node IDs stable across frames

Finding:
CSS pipeline scene node IDs are deliberately varied by frame, while renderer effect caches use `NodeId` as identity.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:336` through `:340` computes `next_scene_id = 1_000_000 + (frame_counter % 1000) * 100_000`.
- `crates/liquide-renderer-cpu/src/blur_worker.rs:68` through `:71` keys cache and pending work by `NodeId`.
- `crates/liquide-renderer-cpu/src/blur_worker.rs:213` through `:220` requires matching id and size for cached blur.
- `crates/liquide-renderer-cpu/src/renderer/effects.rs:386` through `:440` uses node id to request and reuse async backdrop blur.

Impact:
Glass and backdrop blur cannot reliably hit caches if CSS-generated node identities change every frame. The renderer falls back to tint-only output on misses, then may never see a stable id long enough to reuse the blur. This is a strong flicker and wasted-work candidate.

Remediation:
- [ ] Derive scene IDs from stable DOM node identity plus display item role/index.
- [ ] Add explicit generation/invalidation only when the element identity, role, or effect input changes.
- [ ] Keep renderer caches keyed by stable identity plus effect parameters and input-region generation.
- [ ] Add a test that unchanged CSS glass produces the same scene node id across frames.

### TODO 5: Preserve scoped CSS effects through the scene bridge

Finding:
CSS filters, masks, blend modes, and render layers are represented as scoped operations in scene types, but the bridge emits them as standalone flat nodes.

Evidence:
- `crates/liquide-compositor/src/scene/mod.rs:251`, `:258`, and `:288` define render layer, filter, and mask as child-scoped node kinds.
- `crates/liquide-shell/src/pipeline/scene_bridge.rs:139`, `:183`, `:254`, and `:285` emit these effects as standalone nodes.
- `crates/liquide-shell/src/pipeline/scene_bridge.rs:317` applies only opacity, clip, and transform state to ordinary renderable children.
- `crates/liquide-renderer-cpu/src/renderer/effects.rs:301`, `crates/liquide-renderer-cpu/src/renderer/mod.rs:1362`, and `:1426` apply effects to the current framebuffer.
- `crates/liquide-shell/src/pipeline/stages.rs:347` calls `display_list_to_scene` directly instead of submitting property trees.

Impact:
Filters and masks can affect pixels outside their intended subtree, or fail to isolate child content the way CSS expects. This breaks advanced CSS effects and can make shell layers visually bleed into each other.

Remediation:
- [ ] Preserve push/pop scopes from paint output into scene groups.
- [ ] Render affected child ranges into offscreen layers before applying filter, mask, blend, and isolation.
- [ ] Use property trees in the scene path or emit explicit grouped scene ranges/effect ids.
- [ ] Add pixel tests for `filter`, `mask-image`, `mix-blend-mode`, and isolation.

## High

### TODO 6: Add a real container-query feedback loop

Finding:
Container sizes are written only after layout, but container-query style resolution does not run a same-frame second pass.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:244` through `:258` records container sizes after layout.
- `crates/liquide-style-engine/src/engine/media.rs:858` falls back when sizes are missing.
- `crates/liquide-shell/src/pipeline/stages.rs:149` can return cached output on clean frames.
- `crates/liquide-layout/src/container_query.rs:46` has container size recording infrastructure.

Impact:
Initial `@container` results can be wrong, flash for a frame, or stay wrong if no later dirtiness forces another pass. This blocks advanced responsive shell layout.

Remediation:
- [ ] Treat unknown container size as deferred or false instead of viewport-equivalent success.
- [ ] Run style -> layout -> record container sizes -> restyle affected descendants -> relayout/repaint until stable or bounded.
- [ ] Track which descendants depend on which query containers.
- [ ] Add tests where a container query changes layout in the same frame.

### TODO 7: Make scene building pure and move ticking out of render

Finding:
`build_scene` and DOM sync mutate runtime state while producing a scene.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:354` through `:363` toggles cursor blink from `SystemTime::now()` inside scene build.
- `crates/liquide-shell/src/shell/scene.rs:367` through `:376` calls DOM sync and renders the CSS pipeline from `build_scene`.
- `crates/liquide-shell/src/shell/dom_sync.rs:90` through `:108` performs broad sync work.
- `crates/liquide-shell/src/shell/dom_sync.rs:917` advances the tooltip manager during DOM sync.
- `crates/liquide-shell/src/shell/dom_sync.rs:1065` and following lines update thread coordinator fallback state from DOM sync.

Impact:
Rendering is not a pure read of shell state. Multiple scene builds can change hover/tooltip/cursor/fallback state, making frame output order-dependent and harder to reason about in local, remote, or test rendering.

Remediation:
- [ ] Split update/tick and render phases.
- [ ] Advance cursor blink, tooltip animation, fallback coordination, and timers before render.
- [ ] Make `build_scene` consume an immutable snapshot.
- [ ] Add tests that repeated `build_scene` calls without a tick produce identical scene output.

### TODO 8: Replace coarse template replacement with keyed reconciliation

Finding:
Several template paths remove/destroy children and reparse HTML instead of reconciling stable keyed nodes.

Evidence:
- `crates/liquide-shell/src/shell/dom_sync.rs:985` through `:992` removes and destroys template children.
- `crates/liquide-shell/src/shell/dom_sync.rs:996` through `:1019` reapplies templates by clearing and parsing children.
- `crates/liquide-shell/src/shell/dom_sync.rs:1024` through `:1048` removes overlay templates and reparses them.

Impact:
Node identity churn creates unnecessary style/layout/paint work, breaks stable scene identity, and can reset transitions or hover/focus state during ordinary shell updates.

Remediation:
- [ ] Add keyed reconciliation for template output, using `data-key` or equivalent identity.
- [ ] Preserve DOM node ids for unchanged elements.
- [ ] Avoid wholesale clear/reparse for menus, overlays, notifications, and repeated shell chrome.
- [ ] Add tests proving unchanged template nodes retain identity across updates.

### TODO 9: Remove split CSS ownership between resolver and pipeline

Finding:
The shell uses both a legacy `StyleResolver` and the `DesktopPipeline` style engine. These can drift after dynamic stylesheet/theme changes.

Evidence:
- `crates/liquide-shell/src/shell/mod.rs:497` creates a `StyleResolver` from `ThemeEngine`.
- `crates/liquide-shell/src/shell/mod.rs:506` separately creates `DesktopPipeline`.
- `crates/liquide-shell/src/shell/scene.rs:393` still resolves decoration styles through the resolver path.
- `crates/liquide-shell/src/shell/devtools.rs:102` updates dynamic stylesheet state through the CSS pipeline path.

Impact:
Two style sources can disagree about what the shell should look like. CSS-first rendering should not require the shell to query a separate selector engine for parts of the same scene.

Remediation:
- [ ] Make the pipeline computed style/layout output authoritative for rendered shell visuals.
- [ ] Remove decoration style lookups from the legacy resolver path.
- [ ] If the resolver must remain temporarily, rebuild/invalidate it from the same stylesheet source as the pipeline.
- [ ] Add tests that dynamic stylesheet changes affect both DOM/CSS chrome and any remaining decoration consumers in the same frame.

### TODO 10: Replace inline geometry styles with CSS-driven positioning contracts

Finding:
Menus, tooltips, and devtools rows still receive inline positions or indentation from Rust/template values.

Evidence:
- `assets/templates/session-menu.html:12` uses inline `left` and `top`.
- `assets/templates/context-menu.html:16` uses inline `left` and `top`.
- `assets/templates/app-menu.html:15` uses inline `left` and `top`.
- `assets/templates/tooltip.html:12` uses inline `left` and `top`.
- `assets/templates/devtools.html:75` and `:157` use inline indentation.
- `assets/templates/devtools/tree-row.html:21` uses inline indentation.
- `assets/templates/devtools/elements-tab.html:24` uses inline indentation.
- `crates/liquide-shell/src/shell/dom_sync.rs:773`, `:819`, `:888`, and `:933` compute overlay positions in Rust.

Impact:
These paths bypass normal CSS layout, anchoring, logical properties, media/container behavior, and theme-level control. They also make remote/client scaling and high-DPI behavior more fragile.

Remediation:
- [ ] Prefer CSS anchor positioning/popover-style contracts where possible.
- [ ] If geometry must come from Rust, expose it as semantic custom properties on a layer root, not per-node inline styles.
- [ ] Centralize menu/tooltip placement so collision, viewport bounds, and directionality are consistent.
- [ ] Replace devtools indentation inline styles with classes, depth attributes, or CSS variables.

### TODO 11: Replace shell z-band/fullscreen-fill heuristics with explicit layer contracts

Finding:
CSS output is split into background and chrome by geometry heuristics, then z-orders are rewritten.

Evidence:
- `crates/liquide-shell/src/shell/scene.rs:407` through `:463` splits pipeline nodes into desktop background and chrome overlay.
- `crates/liquide-shell/src/shell/scene.rs:424` and `:455` rewrite z-order bands.
- Fullscreen `Background` or `GradientFill` nodes with area above a threshold are treated specially.

Impact:
Authoring a new fullscreen CSS effect can accidentally change layer classification. CSS stacking context and z-index are not the single source of truth.

Remediation:
- [ ] Introduce explicit DOM layer roots: desktop background, workspace, chrome, popover, modal, lockscreen, loading.
- [ ] Let CSS stacking operate inside those roots.
- [ ] Use a small explicit root-layer contract instead of area-based classification.
- [ ] Add tests for fullscreen backgrounds, fullscreen overlays, and z-index ordering.

### TODO 12: Unify renderer timing and adaptive LOD behavior

Finding:
`SoftwareRenderer` has two different `report_render_time` implementations. The render thread uses the trait path, which does less work than the inherent CPU method.

Evidence:
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:404` through `:424` updates blur state and LOD bias.
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:789` through `:797` only updates blur timing through the trait implementation.
- `crates/liquide-session/src/desktop/render_thread.rs:1319` calls `renderer.report_render_time(render_ms)` through `&mut dyn Renderer`.

Impact:
Production adaptive quality behavior differs from the intended CPU renderer behavior. LOD bias may not update on the threaded presentation path, and blur toggling thresholds/cache handling differ by call site.

Remediation:
- [ ] Make the trait implementation delegate to the same shared CPU timing function.
- [ ] Keep one hysteresis policy for blur and LOD.
- [ ] Add a `Box<dyn Renderer>` test proving slow frames update `lod_stats().adaptive_bias`.

### TODO 13: Fix async blur cache validation and popping

Finding:
Backdrop blur is async, cache-keyed mostly by node id and size, and can fall back to tint-only output on misses. It can also present cached blur before validating that the backdrop input region is still current.

Evidence:
- `crates/liquide-renderer-cpu/src/blur_worker.rs:8` through `:10` documents first-frame tint-only fallback.
- `crates/liquide-renderer-cpu/src/blur_worker.rs:68` through `:71` keys cache and pending work by `NodeId`.
- `crates/liquide-renderer-cpu/src/blur_worker.rs:217` validates id and size.
- `crates/liquide-renderer-cpu/src/renderer/effects.rs:408` through `:440` blits cached output before snapshotting current framebuffer for a later request.
- `crates/liquide-renderer-cpu/src/renderer/effects.rs:427` through `:438` allocates and copies a backdrop snapshot on cache miss.

Impact:
Glass can pop between blur and tint, show stale blur when the backdrop changed, and allocate/copy heavily under identity churn.

Remediation:
- [ ] Include blur radius, effect parameters, and backdrop input generation in the cache key.
- [ ] Invalidate blur when damage intersects the blur input region.
- [ ] Reuse scratch buffers or pooled snapshots for blur jobs.
- [ ] Degrade blur quality smoothly instead of global on/off popping.

### TODO 14: Use one damage source and expand damage from actual effect bounds

Finding:
Damage and dirty-rect handling are split between session/compositor data and renderer-local managers. Renderer culling uses fixed padding.

Evidence:
- `crates/liquide-session/src/desktop/render_thread.rs:1272` through `:1284` clears damaged tiles only.
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:730` through `:771` culls nodes by damage bounds.
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:737` through `:748` uses fixed padding around damage.
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:1018` and `:1027` use renderer-local dirty blur checks.
- `crates/liquide-session/src/desktop/render_thread.rs:617` has `mark_rect_dirty`, but normal event redraws call full dirty paths such as `crates/liquide-session/src/desktop/event_loop.rs:217`.

Impact:
Partial rendering is only correct if all old and new pixels are included in damage. Fixed padding can miss large shadows, blur, transforms, masks, outlines, or filter outsets. Full dirty fallbacks hide the issue while hurting performance.

Remediation:
- [ ] Choose one authoritative damage pipeline.
- [ ] Expand damage by actual visual outsets: shadow blur/spread, backdrop blur radius, filter bounds, transforms, outlines, masks.
- [ ] Feed damage into blur cache invalidation.
- [ ] Add tests for moving shadows/blur across tile boundaries.
- [ ] Replace routine full-dirty event redraws with concrete rects where available.

## Medium

### TODO 15: Improve style dirtiness precision

Finding:
Any style invalidation tends to restyle broad subtrees and trigger layout, even when the computed style did not change or only paint changed.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:172` calls style invalidation.
- `crates/liquide-style-engine/src/engine/cascade.rs:198` restyles changed subtrees.
- `crates/liquide-shell/src/pipeline/stages.rs:194` treats style work as layout work.
- `crates/liquide-style-engine/src/impact.rs:7` notes staged impact is not driven by restyle.
- `crates/liquide-style-engine/src/engine/cascade.rs:394` marks `had_changes = true` without comparing old/new computed style.

Impact:
The shell does more layout/paint work than necessary and cannot reliably separate compositor-only changes from paint or layout changes.

Remediation:
- [ ] Diff old/new computed styles.
- [ ] Emit `StyleDiffSummary` with layout, paint, and compositor impacts.
- [ ] Skip layout for unchanged and paint-only style changes.
- [ ] Add perf tests for hover/class toggles on large shell DOMs.

### TODO 16: Index selector matching for advanced CSS

Finding:
Selector support is broad, but matching is not indexed enough. Tag rules are indexed, while class/id/attribute-heavy selectors can fall into broad match buckets. `:has()` recursively scans relationships.

Evidence:
- `crates/liquide-style-engine/src/engine/mod.rs:66` indexes prepared sheets mainly by tag.
- `crates/liquide-style-engine/src/selector.rs:654` and `:1492` show recursive `:has()`/relationship matching paths.

Impact:
Advanced CSS selectors can become expensive in shell-scale DOMs, especially with frequent state/class churn.

Remediation:
- [ ] Add id, class, and attribute rule buckets.
- [ ] Track selector dependencies for invalidation.
- [ ] Cache and specially invalidate `:has()` dependencies.
- [ ] Add selector performance tests for large menus/window lists.

### TODO 17: Model CSS display as outer and inner display

Finding:
Inline flex/grid parsing exists, but layout routing appears to conflate outer and inner display behavior.

Evidence:
- `crates/liquide-style-engine/src/value_resolve.rs:247` parses `inline-flex` and `inline-grid`.
- `crates/liquide-layout/src/block.rs:1473` treats inline flex/grid as inline-level in one path.
- `crates/liquide-layout/src/block.rs:809` routes flex/grid before inline handling.
- `crates/liquide-layout/src/engine.rs:593` only special-cases `Display::Inline` in the generic router.

Impact:
Advanced shell UI using inline flex/grid can lay out differently from CSS expectations.

Remediation:
- [ ] Represent display as outer/inner values.
- [ ] Layout `inline-flex` and `inline-grid` as atomic inline boxes with inner flex/grid formatting.
- [ ] Add tests for inline flex/grid in text/toolbars/status chips.

### TODO 18: Retire or clearly scope `liquide-renderer-css`

Finding:
`liquide-renderer-css` is a narrow legacy bridge and does not look like a complete advanced CSS rendering path.

Evidence:
- `crates/liquide-renderer-css/src/resolver.rs:128` resolves only synthetic element/classes/pseudo/id inputs.
- `crates/liquide-renderer-css/src/resolver.rs:143` maps a small property subset.
- `crates/liquide-renderer-css/src/style.rs:78` defines many fields not fully mapped from the modern computed style pipeline.

Impact:
Maintaining two CSS-to-render style concepts increases confusion and makes completeness claims hard to verify.

Remediation:
- [ ] Replace it with a `ComputedStyle -> RenderStyle` adapter, or mark it as legacy/scoped.
- [ ] Add a compatibility table that says which path owns each CSS feature.

### TODO 19: Finish background and text fidelity in the scene bridge

Finding:
Some advanced CSS painting information is lost when converting display items to scene nodes.

Evidence:
- `crates/liquide-shell/src/pipeline/scene_bridge.rs:574` through `:589` notes background-size mode such as cover/contain is collapsed before scene output.
- `crates/liquide-shell/src/pipeline/scene_bridge.rs:829` through `:861` maps `TextRun` with fallback/default values for font style, spacing, line height, align, white-space, and shadows.

Impact:
CSS may compute the right value but render with degraded fidelity. Typography and background rendering can drift from authored CSS.

Remediation:
- [ ] Carry original background size/repeat/position mode through paint into scene nodes.
- [ ] Eliminate `TextRun` fallback or attach full computed text style to it.
- [ ] Add visual tests for cover/contain backgrounds, italic text, letter spacing, line height, shadows, and whitespace.

### TODO 20: Make template and asset routing canonical

Finding:
The shell has multiple template/root asset paths and workarounds for missing nested template support.

Evidence:
- `crates/liquide-shell/src/desktop_dom.rs:270` loads `assets/desktop.html` or embedded HTML.
- `assets/templates/desktop.html` is a separate richer template source.
- `crates/liquide-shell/src/shell/mod.rs:759` has CWD-only template registry behavior.
- `crates/liquide-shell/src/shell/dom_sync.rs:233` assembles statusbar structure in Rust because nested templates are unsupported.

Impact:
The runtime shell structure can drift from the intended asset/template tree. CSS-first design needs one canonical DOM/template source.

Remediation:
- [ ] Fix nested template support.
- [ ] Make `assets/templates` and `assets/themes` the single runtime source.
- [ ] Remove embedded/raw HTML mirror paths once canonical loading is reliable.

### TODO 21: Move hardcoded theme fallbacks into CSS/default assets

Finding:
Theme conversion and integration still include many Rust fallback colors/layout values.

Evidence:
- `crates/liquide-shell/src/theme_loader.rs` contains many `unwrap_or_else(|| Color::new(...))` defaults.
- `crates/liquide-shell/src/css_integration.rs` contains default layout/color values for decorations, dock, statusbar, launcher, notifications, and menus.

Impact:
Silent Rust fallbacks hide incomplete CSS and make it hard to verify that shell visuals are truly CSS-owned.

Remediation:
- [ ] Move defaults into canonical theme CSS variables/assets.
- [ ] Keep Rust fallbacks only for failsafe mode.
- [ ] Add a strict development check that required shell selectors/properties are present.
- [ ] Emit diagnostics when CSS completeness relies on fallback values.

### TODO 22: Avoid duplicate flatten/sort work on full-frame jobs

Finding:
The compositor flattens the scene on submit, then the session render job flattens the same scene again.

Evidence:
- `crates/liquide-compositor/src/compositor.rs:230` rebuilds `flat_cache`.
- `crates/liquide-session/src/desktop/render_thread.rs:1196` and `:1200` flatten for render jobs.
- `crates/liquide-compositor/src/scene/mod.rs:817` and `:859` sort children during flatten traversal.

Impact:
The renderer spends extra CPU on traversal and sorting, which worsens frame pacing under shell churn.

Remediation:
- [ ] Reuse `compositor.flat_scene()` after submit, or return the freshly flattened list.
- [ ] Add profiling assertions around flatten count per frame.

## Low

### TODO 23: Use actual display and remote cadence for frame budgets

Finding:
Frame timing still has a hardcoded refresh-rate TODO.

Evidence:
- `crates/liquide-shell/src/lib.rs:60` notes replacing hardcoded frame timing with `MonitorInfo::refresh_rate_hz`.

Impact:
Adaptive animation and renderer quality decisions may be tuned for the wrong cadence, especially on high-refresh displays, remote sessions, or throttled clients.

Remediation:
- [ ] Use monitor/client refresh information for frame budgets.
- [ ] Support remote cadence as a first-class timing source.
- [ ] Clamp and smooth cadence changes to avoid oscillation.

### TODO 24: Audit opt-in per-window render workers before enabling

Finding:
Chrome/content worker paths clear full buffers before marking partial damage. These appear opt-in or not on the main path today.

Evidence:
- `crates/liquide-render-thread/src/chrome_thread.rs:191` and `:198`.
- `crates/liquide-render-thread/src/content_thread.rs:193` and `:200`.
- `crates/liquide-session/src/desktop/window_render.rs:46` and `:102` suggest guarded/optional usage.

Impact:
If enabled without adjustment, partial damage could present cleared pixels outside the dirty region.

Remediation:
- [ ] Preserve pixels outside damage when using partial worker output.
- [ ] Or declare worker output full-frame and damage the full buffer after clear.
- [ ] Add a targeted worker-path visual test before enabling.

## Suggested Verification Work

- [ ] CSS ownership audit: assert no shell chrome surface is built manually except real app buffers.
- [ ] Stable identity test: unchanged DOM/CSS produces unchanged scene node ids.
- [ ] Animation test: paint-only transition updates display output every frame.
- [ ] Layout animation test: layout-affecting transition either relayouts or is explicitly unsupported.
- [ ] Container query test: container-driven style/layout resolves in one visible frame.
- [ ] Purity test: repeated `build_scene` with no tick produces identical output.
- [ ] Effect scoping pixel tests: filter, mask, blend mode, isolation.
- [ ] Damage tests: moving shadow, transform, and blur across tile boundaries.
- [ ] Renderer trait-object test: adaptive LOD changes through `&mut dyn Renderer`.

## Notes From This Pass

- Subagents were used for independent read-only slices: shell DOM/template ownership, CSS engine completeness/invalidation, and renderer/effects/performance.
- No tests were run in this pass because the request was review-only and no source behavior was changed.
- Existing broader audit remains at `todos/de-css-remote-shell-audit.md`.
- Existing focused flicker/hardcoded-style remediation notes remain at `todos/rendering-flicker-hardcoded-styles.md`.
