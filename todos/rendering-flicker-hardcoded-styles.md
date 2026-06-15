# Rendering Flicker and Hardcoded Styling TODOs

Date: 2026-06-15

Scope:
- CSS-driven shell scene assembly.
- Theme loading from session startup through `Shell` and `DesktopPipeline`.
- CSS pipeline cache invalidation and viewport/theme mutation behavior.
- Decoration CSS selector usage against the real window template.
- Renderer-side flicker sources that remain after the shell/CSS fixes.

Severity legend:
- Critical: can show stale or wrong frames after a real theme/viewport/style mutation.
- High: visible theme/design bypass or likely flicker under normal desktop use.
- Medium: real drift risk, but narrower or dependent on timing/load.

## Completed In This Pass

- Removed the unconditional dark/blue/purple shell backdrop from `crates/liquide-shell/src/shell/scene.rs`. The desktop background now comes from the DOM/CSS `desktop-background` path instead of a manual fallback layer.
- Replaced hardcoded shell overlay colors for dialogs, overview, and lockscreen with values from the active `ShellTheme`.
- Changed the shell lockscreen overlay away from `SceneNodeKind::LockScreen` and into a themed `Background` scrim, so the shell path no longer depends on the renderer's hardcoded lockscreen veil.
- Invalidated `DesktopPipeline` cached output after stylesheet, theme, viewport, and color-scheme changes in `crates/liquide-shell/src/pipeline/stages.rs`.
- Reset transition and animation state on full theme replacement so old style state cannot bleed into a new theme frame.
- Wired `Shell::load_css_theme` and `load_default_css_theme` to replace the CSS desktop pipeline theme, not only the legacy `ShellTheme`/`StyleResolver` path.
- Removed the session startup double-add of theme CSS in `crates/liquide-session/src/desktop/mod.rs`.
- Updated decoration layout/style resolution to prefer the selectors used by `assets/templates/window.html`: `window-titlebar`, `close-button`, `maximize-button`, and `minimize-button`.
- Added regression coverage for immediate scene changes after `set_theme`, `add_stylesheet`, and `set_viewport`.

## Confirmed Flicker Origins Fixed Or Mitigated

### TODO R1: Stale CSS Pipeline Output After Runtime Mutations

Status: Fixed

Finding:
`DesktopPipeline` cached `last_styles`, `last_layout`, and `last_display_list`, but stylesheet/theme/viewport/color-scheme mutation methods did not clear those cached products.

Impact:
The renderer could reuse an old scene for one or more frames after a real style mutation, then snap to the new frame once another dirty path happened to run. This is a direct flicker/jump source during theme changes, monitor resize, and external theme loading.

Remediation:
- [x] Clear cached style/layout/display-list output after `add_stylesheet`.
- [x] Clear cached output after `set_theme`.
- [x] Clear cached output after actual viewport dimension changes.
- [x] Clear cached output after preferred color scheme changes.
- [x] Add regression tests proving output changes immediately without a DOM dirty change.

### TODO R2: Theme Loading Split Between Legacy StyleResolver And CSS Pipeline

Status: Fixed

Finding:
`Shell::load_css_theme` updated the parsed `ShellTheme` and legacy `StyleResolver`, but did not feed the loaded stylesheet into `DesktopPipeline`, which is the path that produces the CSS-driven desktop scene.

Impact:
The shell could believe a theme was loaded while the rendered CSS desktop scene was still using prior pipeline styles. Session startup then compensated by separately calling `shell.add_stylesheet`, creating split ownership and duplicate theme behavior.

Remediation:
- [x] Replace the current `css_pipeline` theme from `Shell::load_css_theme`.
- [x] Reset the CSS pipeline on `load_default_css_theme`.
- [x] Keep the session startup path single-sourced by removing the extra `shell.add_stylesheet`.

### TODO R3: Manual Default Backdrop Bypassed CSS

Status: Fixed

Finding:
`Shell::build_scene` always appended a manually painted backdrop with fixed dark, blue, purple, and white accent colors.

Impact:
The first visible desktop layer was partly theme-independent. During CSS scene churn, this fallback could visually compete with the CSS `desktop-background` and make theme changes look like a flash between two background systems.

Remediation:
- [x] Remove the hardcoded default backdrop.
- [x] Let CSS-provided background nodes be the source of desktop background styling.

### TODO R4: Decoration Selectors Did Not Match The Window Template

Status: Fixed

Finding:
Decoration layout queried legacy selectors like `titlebar` and `titlebar-button`, while the window template and theme CSS use `window-titlebar`, `close-button`, `maximize-button`, and `minimize-button`.

Impact:
Decoration hit geometry and button sizes could silently fall back to hardcoded defaults even when CSS supplied real dimensions. That causes render/hit-test drift and visible snapping when states change.

Remediation:
- [x] Resolve the template selectors first.
- [x] Keep legacy selectors as compatibility fallback.
- [x] Add a unit test proving the template selectors drive decoration layout.

## Remaining Flicker Risks

### TODO R5: CPU Renderer Adaptive Blur Can Pop Glass On And Off

Status: Open

Finding:
`crates/liquide-renderer-cpu/src/renderer/mod.rs` toggles `blur_enabled` based on moving render time averages. Glass, blur-backdrop, blur-cache, and lockscreen-style nodes all branch on that state.

Impact:
Under load, glass can abruptly switch between blurred and unblurred rendering, which reads as flicker or a glass "pop." This is separate from CSS correctness and needs renderer-level smoothing.

Remediation:
- [ ] Add hysteresis or minimum dwell time before toggling blur quality.
- [ ] Prefer quality degradation per effect over global blur enable/disable.
- [ ] Preserve/fade cached blur output during quality transitions.
- [ ] Emit debug counters/tracing when blur quality changes so flicker reports can be correlated to renderer state.

### TODO R6: Threaded Fallback Can Swap Scene Source If Main CSS Output Is Empty

Status: Open

Finding:
`Shell::build_scene` can use threaded scene nodes when the main CSS pipeline returns no nodes. That fallback is useful for resilience, but a transient empty CSS output can replace the visible scene source for a frame.

Impact:
If the main DOM/CSS path momentarily produces an empty scene during loading or mutation, the desktop can flash to the threaded fallback scene instead of retaining the last known-good CSS scene.

Remediation:
- [ ] Track last-good CSS scene output and reuse it for transient empty frames.
- [ ] Treat empty main CSS output as an observable warning when the DOM is non-empty.
- [ ] Add a test that simulates empty main output during a theme mutation and asserts no source swap.

### TODO R7: Cursor Blink Is Driven Inside `build_scene`

Status: Open

Finding:
The text caret blink toggles from wall-clock time inside `Shell::build_scene`.

Impact:
Extra scene builds can advance caret visibility outside the compositor's stable frame cadence. This is intended for text input, but it can be perceived as unrelated flicker in tests or tooling that calls `build_scene` repeatedly.

Remediation:
- [ ] Drive caret blink from the compositor frame tick or an explicit animation clock.
- [ ] Keep tests and snapshot paths on a deterministic clock.

### TODO R8: Manual Window/App Content Still Carries Styling Defaults

Status: Open

Finding:
Some manually assembled window/app placeholder content still uses scene-builder text defaults and fixed fallback values rather than resolved CSS text styles.

Impact:
Most shell chrome is moving through DOM/CSS, but these manual subtrees can still visually drift from the theme and can produce mismatched text metrics when app placeholder content changes.

Remediation:
- [ ] Move remaining placeholder app/window content into DOM templates where practical.
- [ ] Where manual scene nodes remain necessary, pass resolved CSS text/background styles into the scene builder.

## Verification

- [x] `cargo test -p liquide-shell --lib`
- [x] `cargo check -p liquide-session`
- [x] `cargo test -p liquide-visual-test --test wiring_audit`
- [x] `git diff --check`
