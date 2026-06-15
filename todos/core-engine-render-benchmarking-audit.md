# Core Engine Render Benchmarking Audit

Date: 2026-06-15

Scope:
- Review only. No source files were changed during this pass.
- Focus: how to measure real render time from shell/DOM tree to presented pixels.
- Target path: DOM/template sync -> style -> layout -> container query feedback -> animation resolution -> paint/display list -> scene bridge -> compositor flatten/damage -> CPU/GPU render -> tile encode/present.

Severity legend:
- Critical: current benchmark/telemetry numbers can mislead decisions about jank or SLO health.
- High: missing benchmark or instrumentation blocks diagnosis of real shell performance.
- Medium: benchmark exists but only covers a lower-level slice.

## Current Coverage Summary

Existing benchmarks are useful, but they do not answer "how long does the real shell take from tree to render?"

- Root workspace Criterion benches are registered at `Cargo.toml:505` through `:519`: `tile_encode`, `transport_throughput`, `compositor_render`, and `layout_cache`.
- `benches/layout_cache.rs` benchmarks synthetic layout only with a manually built `StyleMap`, not CSS cascade, paint, scene bridge, or renderer.
- `benches/compositor_render.rs` benchmarks synthetic scene submit/flatten/damage/present lifecycle, not DOM/CSS/style/layout/paint, and not real CPU renderer rasterization.
- `crates/liquide-shell/benches/shell_bench.rs` benchmarks shell data structures and window bookkeeping, not `Shell::build_scene()` or the CSS pipeline.
- `crates/liquide-renderer-cpu/benches/renderer_bench.rs` benchmarks primitive kernels such as blur, rect fill, blend, paths, and glyph blit, not full flat-scene rendering.
- `crates/liquide-bench/src/harness.rs:51` through `:102` simulates compositor timing data rather than timing the real compositor/render path.

## Critical

### TODO 1: Add a real tree-to-pixels benchmark suite

Finding:
There is no benchmark that starts from a real shell DOM/tree and measures the full render path into pixels.

Evidence:
- `crates/liquide-shell/src/pipeline/mod.rs:1` through `:14` defines the intended DOM -> Style -> Layout -> Paint -> SceneNode path.
- `crates/liquide-shell/src/shell/scene.rs:367` through `:376` runs DOM sync and `render_to_scene_with_output`.
- `crates/liquide-session/src/desktop/render_thread.rs:1196` through `:1304` then submits, flattens, damages, renders, and trims output.
- Existing registered benches at `Cargo.toml:505` through `:519` do not include a full shell/CSS/render benchmark.

Impact:
We cannot currently prove whether jank comes from DOM sync, selector matching, layout, paint, scene bridge, flattening, rasterization, blur, damage, tile encode, or present. Optimizations are likely to chase the wrong layer.

Remediation:
- [ ] Add a `shell_tree_to_pixels` Criterion bench.
- [ ] Build real shell fixtures from canonical assets/templates/themes.
- [ ] Measure cold frame, warm clean frame, style-dirty frame, layout-dirty frame, paint-dirty frame, animation frame, and resize frame.
- [ ] Render into a CPU framebuffer with real `SoftwareRenderer`.
- [ ] Record node counts, DOM node count, style count, layout box count, display item count, scene node count, flat node count, damaged tiles, rendered tiles, and output pixels touched.

### TODO 2: Replace simulated `liquide-bench` compositor timings with real measurements or label them as synthetic

Finding:
The top-level benchmark harness presents compositor/latency metrics, but it generates deterministic simulated values.

Evidence:
- `crates/liquide-bench/src/harness.rs:53` says the compositor suite simulates frame composition.
- `crates/liquide-bench/src/harness.rs:65` through `:100` records synthetic `compose_time`, `damage_compute_time`, `input_to_photon`, cursor latency, FPS, and first-frame values.
- `crates/liquide-bench/src/slo.rs:152` through `:156` defines LAN SLOs for input-to-photon, first frame, cursor, and FPS.

Impact:
These numbers cannot validate the real shell or renderer. They can pass while the real frame path is slow, flickering, or doing duplicate work.

Remediation:
- [ ] Rename current suites to `synthetic-*` or clearly label output as simulated.
- [ ] Add real suites that call the compositor, renderer, encoder, and shell pipeline.
- [ ] Make CI SLOs depend on real measured suites for render-critical paths.
- [ ] Keep synthetic suites only for deterministic smoke tests or model validation.

### TODO 3: Add per-stage timing to `DesktopPipeline`

Finding:
The CSS pipeline has clear stage boundaries, but it does not expose timing or work-count breakdowns.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:132` starts `DesktopPipeline::run`.
- `crates/liquide-shell/src/pipeline/stages.rs:172` through `:190` performs style work.
- `crates/liquide-shell/src/pipeline/stages.rs:193` through `:242` performs layout work.
- `crates/liquide-shell/src/pipeline/stages.rs:244` through `:258` records container sizes.
- `crates/liquide-shell/src/pipeline/stages.rs:260` through `:276` performs animation/transition work.
- `crates/liquide-shell/src/pipeline/stages.rs:278` through `:292` performs paint or display-list reuse.
- `crates/liquide-shell/src/pipeline/stages.rs:341` through `:350` runs the pipeline and scene bridge.

Impact:
`render_ms` cannot be explained. A slow frame is just "slow", with no attribution to style, layout, paint, bridge, or cache reuse.

Remediation:
- [ ] Add a low-overhead `PipelineTimings` or `PipelineTrace` struct.
- [ ] Record `dom_sync_us`, `style_us`, `layout_us`, `container_query_us`, `animation_us`, `paint_us`, `scene_bridge_us`, and `total_tree_to_scene_us`.
- [ ] Record whether each stage was full, incremental, cached, or skipped.
- [ ] Attach stage counts: dirty nodes, restyled nodes, layout roots, layout boxes, display items, scene nodes, glass nodes.
- [ ] Make timings available to devtools, telemetry, and benchmarks without requiring logging.

### TODO 4: Stop overloading `render_ms`

Finding:
The threaded render path calculates both raster time and total render-thread time, but sends `total_ms` in a field named `render_ms`.

Evidence:
- `crates/liquide-session/src/desktop/render_thread.rs:1287` starts `t_render` just before renderer work.
- `crates/liquide-session/src/desktop/render_thread.rs:1315` computes `render_ms`.
- `crates/liquide-session/src/desktop/render_thread.rs:1316` computes `total_ms`.
- `crates/liquide-session/src/desktop/render_thread.rs:1330` through `:1337` stores `render_ms: total_ms`.
- `crates/liquide-session/src/desktop/render_thread.rs:847` adds `present_ms` to `frame.render_ms`, so total thread time is treated as render time and then combined with present time.

Impact:
Telemetry and logs blur together scene submit, flatten, damage, raster, trim, pixel copy, and render-thread overhead. This hides the true raster cost and can make present/frame totals confusing.

Remediation:
- [ ] Rename fields or split them: `render_thread_total_ms`, `raster_ms`, `flatten_ms`, `damage_ms`, `trim_ms`, `pixel_copy_ms`.
- [ ] Keep `present_ms` separate.
- [ ] Use the same names in logs, telemetry, devtools, and benchmark reports.

## High

### TODO 5: Wire telemetry-viewer frame breakdown to real session timings

Finding:
The telemetry viewer defines a useful stage-level frame model, but the live session telemetry uses a different aggregate model.

Evidence:
- `crates/liquide-telemetry-viewer/src/frame_stats.rs:9` through `:26` defines `FrameStats` with total, style, layout, paint, composite, raster, and idle timing fields.
- `crates/liquide-session/src/telemetry.rs:65` through `:122` tracks aggregate frame time and FPS only.
- `crates/liquide-telemetry-viewer/src/types.rs:25` through `:48` defines exported frame metrics without stage breakdown.
- `crates/liquide-telemetry-viewer/README.md:116` through `:120` describes file-based session telemetry export, but `rg` shows no session-side call to `export_telemetry`.

Impact:
The tool that should expose performance does not receive the detailed render pipeline data needed to debug jank.

Remediation:
- [ ] Unify session telemetry and telemetry-viewer data types or add an adapter.
- [ ] Export stage timings from the session process.
- [ ] Include p50/p95/p99 per stage and per workload.
- [ ] Add a debug overlay/devtools panel for current frame breakdown.

### TODO 6: Add shell scene-build benchmarks

Finding:
The shell benchmark file covers window management, focus, stats, history, and screen-time logic, but not real scene construction.

Evidence:
- `crates/liquide-shell/benches/shell_bench.rs` benchmarks `open_close_1000_windows`, `visible_windows_sort_500`, tiling, focus, history, stats, and screen time.
- `crates/liquide-shell/src/shell/scene.rs:354` starts `Shell::build_scene`.
- `crates/liquide-shell/src/shell/scene.rs:367` calls `sync_dom`.
- `crates/liquide-shell/src/shell/scene.rs:371` calls the CSS pipeline scene build.

Impact:
The shell can regress badly in actual rendering while existing shell benches stay green.

Remediation:
- [ ] Add benches for `Shell::build_scene()` with 0, 1, 10, 50, and 200 windows.
- [ ] Add scenarios for launcher open, context menu open, app menu open, notification stack, overview, lockscreen, dialog, devtools open, and cursor blink frame.
- [ ] Separate DOM sync time from CSS pipeline time and manual scene assembly time.
- [ ] Track scene node counts and cache hit/miss counters.

### TODO 7: Add real CPU renderer full-scene benchmarks

Finding:
Renderer benches currently measure low-level primitives, not full-scene render cost.

Evidence:
- `crates/liquide-renderer-cpu/benches/renderer_bench.rs` measures blur, fill rect, rounded rect, blend scanline, color roundtrip, path fill, and glyph blit.
- `crates/liquide-renderer-cpu/src/renderer/mod.rs:717` through `:722` exposes the renderer entry used for full flat-scene rendering.
- `crates/liquide-session/src/desktop/render_thread.rs:1300` calls `renderer.render(flat_nodes_buf, framebuf, &damage)`.

Impact:
Primitive numbers are useful, but they do not capture full-scene branch mix, node ordering, clipping, text, glass, shadow, damage culling, cache behavior, or partial tile rendering.

Remediation:
- [ ] Add renderer benches that feed realistic `FlatNode` scenes from shell fixtures.
- [ ] Benchmark full-frame and partial damage.
- [ ] Include scenes with text-heavy UI, glass/backdrop blur, gradients, shadows, transforms, masks, and images.
- [ ] Record nodes visited, nodes culled by damage, pixels touched, blur jobs submitted, cache hits, and allocations.

### TODO 8: Add display-list and scene-bridge benchmarks

Finding:
There is no direct benchmark for paint output size or conversion from display list to scene nodes.

Evidence:
- `crates/liquide-paint/src/painter/mod.rs:48` through `:72` paints layout into a `DisplayList`.
- `crates/liquide-shell/src/pipeline/scene_bridge.rs:25` converts display lists to compositor scene nodes.
- `crates/liquide-shell/src/pipeline/stages.rs:347` through `:350` bridges paint output into scene nodes during render.

Impact:
Scene bridge overhead, allocation churn, and item expansion are invisible. This matters because CSS-first design will move more surfaces through this bridge.

Remediation:
- [ ] Benchmark paint-only from precomputed style/layout.
- [ ] Benchmark scene-bridge-only from captured display lists.
- [ ] Track item expansion ratio: display items -> scene nodes.
- [ ] Track allocations and stable id generation cost.

### TODO 9: Benchmark incremental paths separately from cold paths

Finding:
The pipeline has cached fast paths and incremental relayout, but benchmarks do not cover the real invalidation matrix across style/layout/paint/render.

Evidence:
- `crates/liquide-shell/src/pipeline/stages.rs:149` through `:167` returns cached pipeline output when clean.
- `crates/liquide-shell/src/pipeline/stages.rs:194` sets layout recomputation based on style/layout dirtiness.
- `crates/liquide-style-engine/src/engine/cascade.rs:198` invalidates changed style subtrees.
- `benches/layout_cache.rs:103` through `:139` benchmarks incremental layout only, with synthetic dirty flags and a synthetic tree.

Impact:
Clean-frame performance, hover/class changes, menu movement, text edits, window drag, resize, and animation frames have different bottlenecks. A single full-layout benchmark cannot represent them.

Remediation:
- [ ] Define benchmark classes: cold, warm-clean, style-only, layout-only, paint-only, transform-only, animation, resize, and asset/theme reload.
- [ ] For each class, record stage skips and cache hits.
- [ ] Fail CI on regressions in both median and p95.

## Medium

### TODO 10: Add benchmark workloads that reflect the real shell

Finding:
Most current benchmarks use synthetic scenes or synthetic DOM/layout trees.

Evidence:
- `benches/layout_cache.rs:12` through `:26` describes a generated DOM tree.
- `benches/compositor_render.rs:12` through `:63` builds a synthetic scene directly from `SceneNode`s.

Impact:
Synthetic workloads are helpful for isolating algorithms, but they will miss shell-specific costs such as template replacement, selector shape, glass layers, text-heavy status areas, and manual/CSS scene mixing.

Remediation:
- [ ] Save canonical benchmark fixtures from real shell states.
- [ ] Include default desktop, many windows, menu-heavy, notification-heavy, devtools-heavy, glass-heavy, and remote-tile-heavy profiles.
- [ ] Use the same fixtures for benchmarks and visual regression tests.

### TODO 11: Add allocation and churn counters

Finding:
The current timing APIs do not expose allocation churn or identity churn, both of which are central to flicker/perf issues in the shell.

Evidence:
- `crates/liquide-shell/src/pipeline/mod.rs:87` through `:99` caches pipeline output behind `Arc`.
- `crates/liquide-shell/src/pipeline/stages.rs:170` through `:191` may clone cached styles when `Arc::try_unwrap` fails.
- `crates/liquide-session/src/desktop/render_thread.rs:1329` copies framebuffer pixels to a new `Vec` before sending a completed frame.

Impact:
Time-only benchmarks can hide memory churn that causes stutters, especially under remote rendering and async blur.

Remediation:
- [ ] Count DOM nodes created/destroyed per frame.
- [ ] Count style/layout/display-list/scene node allocations.
- [ ] Count framebuffer copies, blur snapshot allocations, and tile encode allocations.
- [ ] Include allocation counters in benchmark reports.

### TODO 12: Define stage budgets before optimizing

Finding:
The repo has SLOs for user-facing frame goals, but not a budget split for the render pipeline stages.

Evidence:
- `crates/liquide-bench/src/slo.rs:152` through `:156` defines broad LAN SLOs.
- No stage-level SLOs exist for style, layout, paint, bridge, raster, damage, encode, or present.

Impact:
Without budgets, a 16.67ms frame target does not tell engineers where time is allowed to go or which subsystem owns a regression.

Remediation:
- [ ] Define 60Hz, 120Hz, and remote-session budgets.
- [ ] Track p50, p95, and p99 per stage.
- [ ] Suggested starting budget for 60Hz local: tree-to-scene under 5ms p95, raster under 6ms p95, present/encode under 3ms p95, idle margin at least 2ms.
- [ ] Revisit budgets after real measurements land.

## Suggested Benchmark Commands After Implementation

- `cargo bench --bench shell_tree_to_pixels`
- `cargo bench -p liquide-shell --bench shell_bench`
- `cargo bench -p liquide-renderer-cpu --bench renderer_bench`
- `cargo bench --bench layout_cache`
- `cargo bench --bench compositor_render`
- `cargo run -p liquide-bench -- --suite real-render --profile lan`

## Notes From This Pass

- No tests or benchmarks were run. This pass audited benchmark coverage and instrumentation gaps only.
- This document complements `todos/rendering-pipeline-basics-css-first-audit.md`, which covers correctness and jank origins.
