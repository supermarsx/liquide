# E2E Testing

LiquiDE has four runnable end-to-end surfaces today:

1. Built-in app launch and scripted input coverage in `crates/liquide-e2e`.
2. Shell rendering, viewport, event, context menu, and font coverage in `crates/liquide-shell/tests`.
3. Session integration, scene, window-management, dock, devtools, and tiling coverage in `crates/liquide-session/tests`.
4. Renderer-focused smoke and bench compile gates covering CPU rendering, render-coordinator output, wgpu coverage contracts, stale-frame metadata, remote tile loopback, and no-run Criterion bench builds.

The workspace root is a virtual workspace. Root-level `tests/e2e` and `tests/integration` files are inert and should not be treated as the canonical entry point.

## Runner Quick Start

Use the manifest-driven runner for local and future CI e2e execution:

```powershell
pwsh -NoProfile -File scripts/e2e.ps1 -List
pwsh -NoProfile -File scripts/e2e.ps1 -Suite check
pwsh -NoProfile -File scripts/e2e.ps1 -Suite apps -ContinueOnFailure
pwsh -NoProfile -File scripts/e2e.ps1 -Suite shell,session -OutputDir target/e2e-shell-session
pwsh -NoProfile -File scripts/e2e.ps1 -Suite renderer -List
pwsh -NoProfile -File scripts/e2e.ps1 -Suite renderer -Tier smoke
```

Windows PowerShell 5 is supported when available:

```powershell
powershell -NoProfile -File scripts/e2e.ps1 -List
```

Useful filters and switches:

- `-Suite check,apps,shell,session,renderer` narrows the selected manifest entries by suite.
- `-Tier preflight,smoke,integration,bench-compile` narrows by tier.
- `-List` prints the selected id, suite, tier, platform, description, and command without running anything.
- `-ContinueOnFailure` runs all selected entries and exits 0 even when one or more entries fail.
- `-OutputDir` controls where logs and `summary.json` are written. Relative paths are resolved from the repository root.
- `-CargoTargetDir` controls the Cargo build output used by runner commands. It defaults to `target/e2e/cargo-target` so automated e2e runs do not collide with locked executables in the shared `target/debug` tree on Windows. Pass `-CargoTargetDir ""` to use Cargo's normal target directory.
- `-NoCapture` removes trailing cargo test `-- --nocapture` arguments from manifest entries that include them.

## Suites

- `check`: fast preflight compilation for the app harness, platform, built-in app crates, and `liquide-e2e` package.
- `apps`: Windows-oriented built-in app scenarios from `liquide-e2e`, including app launch, scripted input flow, and the aggregate workspace smoke.
- `shell`: platform-agnostic shell e2e and integration coverage.
- `session`: platform-agnostic desktop session e2e coverage.
- `renderer`: renderer-specific smoke coverage plus no-run Criterion benchmark compile gates. Use `-Tier smoke` for behavioral checks only, or `-Tier bench-compile` to compile renderer-related benches without running measurements.

Platform selection is automatic. Entries tagged `all` run on every host; Windows app entries are selected only on Windows.

## Output Artifacts

The runner writes artifacts under `target/e2e` by default:

- `target/e2e/logs/<id>.log`: streamed stdout/stderr for each command.
- `target/e2e/summary.json`: machine-readable execution summary with `generatedAt`, `root`, `selectedCount`, `failedCount`, and a `results` array containing `id`, `suite`, `tier`, `platforms`, `commandLine`, `exitCode`, `durationMs`, and `logPath`.

By default, Cargo artifacts for runner commands are isolated under `target/e2e/cargo-target`. This costs one extra compile the first time the runner is used, but it avoids the common Windows failure where a previous test process keeps `target/debug/*.exe` locked while a later command tries to rebuild it.

## CI/Nightly Integration

CI and nightly e2e workflows now use the manifest-driven runner for consistent command selection, platform filtering, and artifact output.

Canonical workflow invocation:

```powershell
pwsh -NoProfile -File scripts/e2e.ps1 -Suite check,apps,shell,session -CargoTargetDir ""
```

CI overrides the Cargo target directory to an empty string (`-CargoTargetDir ""`) to reuse the existing cached `target/` directory instead of the runner's default isolated `target/e2e/cargo-target` tree. This avoids redundant rebuilds in the hosted environment where lock collisions are not a concern.

Workflow artifacts contain the entire `target/e2e/` directory:

- `target/e2e/logs/*.log`: per-command stdout/stderr.
- `target/e2e/summary.json`: machine-readable execution summary.

Retention: 7 days for CI runs, 14 days for nightly runs.

Linux workspace jobs remain separate from this e2e lane and should not be blocked by unrelated deferred targets outside the e2e surface.

## Next Phases

1. ~~Wire CI and nightly jobs to call `scripts/e2e.ps1` instead of duplicating cargo command lists.~~ ✅ Complete (Apr 29 2026).
2. ~~Add frame-sequence and flicker assertions to catch missing presents, repeated stale frames, and unstable render cadence.~~ Complete in the renderer suite.
3. Add visual baselines for stable shell/session scenes once the renderer capture path can produce deterministic host-safe snapshots.
4. Expand app scenarios beyond Windows by moving host-specific assumptions behind manifest platform tags and scenario-level capability checks.
5. Add suite ownership metadata and trend reporting so failures can be routed by area and compared across nightly runs.

## Direct Cargo Fallbacks

Compile the e2e app surface:

```powershell
cargo check -p liquide-app-harness -p liquide-platform -p liquide-apps-files -p liquide-apps-settings -p liquide-apps-terminal -p liquide-apps-text-editor -p liquide-apps-software-center -p liquide-apps-task-manager -p liquide-e2e
```

Run the built-in app launch smoke scenarios:

```powershell
cargo test -p liquide-e2e --test windows_app_launch -- --nocapture
```

Run the scripted input-flow scenarios:

```powershell
cargo test -p liquide-e2e --test windows_input_flow -- --nocapture
```

Run the full built-in app aggregate smoke:

```powershell
cargo test -p liquide-e2e --test windows_full_workspace_smoke -- --nocapture
```

Run the shell and session e2e suites directly:

```powershell
cargo test -p liquide-shell --test integration_rendering --test e2e_viewport --test e2e_event_dispatch --test e2e_context_menu --test e2e_font_rendering
cargo test -p liquide-session --test e2e_full_integration --test e2e_scene_rendering --test e2e_window_management --test e2e_dock_tracking --test e2e_devtools --test e2e_alignment_tiling
```

Run the renderer-focused smoke checks directly:

```powershell
cargo test -p liquide-renderer-cpu --lib render_background
cargo test -p liquide-render-coordinator --test real_render_output
cargo test -p liquide-renderer-wgpu --lib scene_kind_coverage
cargo test -p liquide-session --lib t47_
cargo test -p liquide-session --test tile_loopback
```

Compile the renderer-related Criterion benches without running measurements:

```powershell
cargo bench --bench tile_encode --no-run
cargo bench --bench transport_throughput --no-run
cargo bench --bench compositor_render --no-run
```

## Design Notes

- `crates/liquide-e2e` exercises built-in apps through their real `AppBootstrap` path.
- The scenario layer uses `liquide-platform`'s standalone backend plus scripted `PlatformEvent` injection for deterministic resize, focus, keyboard, and mouse delivery.
- Terminal coverage runs through the explicit stub PTY path so the suite stays deterministic on Windows CI.
- Assertions currently focus on launch/runtime contracts, frame delivery, and present capture. Rich visual and widget-semantic assertions remain future work.
