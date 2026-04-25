# E2E Testing

## What Exists

LiquiDE now has three distinct end-to-end surfaces:

1. Shell e2e and integration coverage in `crates/liquide-shell/tests`.
2. Session e2e coverage in `crates/liquide-session/tests`.
3. Built-in app launch and scripted input coverage in `crates/liquide-e2e`.

The workspace root is a virtual workspace, so root-level `tests/e2e` and `tests/integration` files are inert and should not be treated as the canonical entry point.

## Primary Entry Points

Run the built-in app workspace e2e package:

```powershell
cargo test -p liquide-e2e -- --nocapture
```

Run only the built-in app launch smoke scenarios:

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

Run the existing shell and session e2e suites:

```powershell
cargo test -p liquide-shell --test integration_rendering --test e2e_viewport --test e2e_event_dispatch --test e2e_context_menu --test e2e_font_rendering
cargo test -p liquide-session --test e2e_full_integration --test e2e_scene_rendering --test e2e_window_management --test e2e_dock_tracking --test e2e_devtools --test e2e_alignment_tiling
```

## Design Notes

- `crates/liquide-e2e` exercises built-in apps through their real `AppBootstrap` path.
- The scenario layer uses `liquide-platform`'s standalone backend plus scripted `PlatformEvent` injection for deterministic resize, focus, keyboard, and mouse delivery.
- Terminal coverage runs through the explicit stub PTY path so the suite stays deterministic on Windows CI.
- Assertions intentionally focus on launch/runtime contracts, frame delivery, and present capture. Rich visual or widget-semantic assertions are deferred until the built-in apps expose less placeholder-heavy roots.

## CI

- `CI` now includes a `windows-latest` e2e job that runs `liquide-e2e` plus the validated shell and session e2e suites.
- `Nightly` mirrors that Windows e2e lane so the built-in app, shell, and session paths keep running outside Linux-only jobs.
- Linux workspace jobs remain intact and are not blocked on unrelated deferred targets outside the app e2e slice.
