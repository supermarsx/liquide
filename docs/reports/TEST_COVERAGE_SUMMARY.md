# Test Coverage Summary (Updated 2026-04-24)

## Current Coverage Health

- Shell e2e and integration rendering coverage is runnable through `crates/liquide-shell/tests`.
- Session e2e coverage is runnable through `crates/liquide-session/tests`.
- Workspace-level built-in app launch and scripted input coverage is now runnable through `cargo test -p liquide-e2e`.
- The Windows CI and nightly lanes now run `liquide-e2e` plus the validated shell and session e2e slices.

## Scope Notes

- The repository root is a virtual workspace, so `tests/e2e` and `tests/integration` under the root do not form a real runnable test lane.
- The authoritative entry point for built-in app end-to-end coverage is now `crates/liquide-e2e`.
- Shell and session e2e suites remain separate packages, but they are now part of the validated Windows gate.
- Full-workspace `--all-targets` and `--no-run` commands are still not the primary gate for this slice because unrelated targets outside the app e2e surface remain deferred.

## Confidence Areas

1. Built-in app launch through `AppBootstrap` on Windows for Files, Settings, Terminal, Text Editor, Software Center, and Task Manager.
2. Scripted resize, focus, key, and mouse delivery through the standalone platform backend.
3. Core shell scene build/render flow and viewport/event dispatch coverage.
4. Session-level window, dock, devtools, and scene flows.

## Remaining Coverage Gaps

- Shell-to-real-app process orchestration is still separate from the scripted built-in app path.
- Most built-in apps still expose placeholder-heavy UI roots, so current assertions focus on launch/runtime contracts rather than rich widget semantics.
- Native Win32 smoke beyond the standalone scripted backend remains a follow-on lane.
- Advanced CSS/compositor parity coverage remains strongest for baseline behavior and should keep expanding as those features mature.
