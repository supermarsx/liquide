# UI Toolkit Stance

**Status**: Canonical — adopted April 22, 2026 (task t9, Phase 1).
**Scope**: Internal guidance for crates that build user-facing surfaces
(windows, dialogs, widgets, shell chrome, built-in applications).

## Canonical Toolkits

Liquide has converged on two complementary content paths. All **new** code
targeting on-screen UI MUST choose one of them; the choice is driven by
whether the surface is built from code-defined widgets or from declarative
DOM/template content.

### 1. Retained-mode widgets (native UI surface)

Use the trio:

- **`liquide-ui-core`** — widget tree, layout primitives, event dispatch,
  painting traits. The foundation layer.
- **`liquide-ui-widgets`** — concrete widget implementations (buttons,
  labels, panels, lists, inputs, etc.) built on `liquide-ui-core`.
- **`liquide-ui-window`** — window-scoped integration: a widget tree hosted
  inside a compositor window, with frame/tick driving and input routing.

Target for: shell chrome, dialogs, panels, small built-in apps whose UI is
expressed in Rust code rather than markup.

### 2. DOM / template content path

Use **`liquide-components`** for UI built from declarative templates
(HTML-like markup, CSS styling, DOM tree, event attributes). This path
plugs into the existing style/layout/paint pipeline through the DOM and
style-system crates and is appropriate for content-heavy or markup-driven
surfaces (e.g. help viewer, assistance/onboarding, rich notification
bodies).

Retained widgets (path 1) and DOM content (path 2) can coexist within the
same window: a `liquide-ui-window` widget may embed a DOM render surface,
and `liquide-components` content may be framed by widget chrome.

## Deprecated

**`liquide-ui`** is deprecated. It predates the split into
`liquide-ui-core` / `liquide-ui-widgets` / `liquide-ui-window` and
overlaps them in scope. As of Phase 1 of task t9, the crate root carries
a `#![deprecated(...)]` attribute; existing consumers will still compile
but will emit deprecation warnings across the workspace build. These
warnings are **intentional** — they surface every remaining consumer so
that task t10 can migrate them in a single coordinated pass.

Do **not**:

- Add new modules, widgets, or public items to `liquide-ui`.
- Introduce new consumers of `liquide-ui` from other crates.
- Suppress the deprecation warning at consumer sites.

## Migration Guidance

Consumer migration from `liquide-ui` to `liquide-ui-core`/`-widgets`/
`-window` is **deferred to task t10**. Phase 1 deliberately does not
rewrite call sites. If you are working in a crate that currently imports
from `liquide-ui`, leave the imports as-is for now; t10 will enumerate
them from the deprecation warnings, choose the corresponding item in the
canonical trio (or in `liquide-components` when the call site is really a
DOM surface), and migrate crate-by-crate. The only action for Phase 1 is:
do not grow the deprecation surface.
